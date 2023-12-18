#![cfg(test)]

use async_trait::async_trait;
use integration_utils::{integration_contract::IntegrationContract, misc::ToNear};
use model::{
    adjust_api::AdjustApiIntegration,
    ft_token_receiver::FtMessage,
    lockup::LockupCreate,
    lockup_api::LockupApiIntegration,
    schedule::{Checkpoint, Schedule},
    view_api::LockupViewApiIntegration,
};
use near_sdk::{json_types::U128, serde_json, serde_json::json, AccountId, Balance};
use near_workspaces::types::NearToken;
use sweat_integration::SweatFt;
use sweat_model::{FungibleTokenCoreIntegration, StorageManagementIntegration, SweatApiIntegration};

use crate::{
    common::log_result,
    context::{prepare_contract, IntegrationContext},
};

#[tokio::test]
async fn test_adjust() -> anyhow::Result<()> {
    let mut context = prepare_contract().await?;

    let alice = context.alice().await?;
    let bob = context.bob().await?;
    let manager = context.manager().await?;

    let total_amount: Balance = 1_000_000_000_000_000;

    context
        .ft_contract()
        .with_user(&manager)
        .storage_deposit(Some(alice.to_near()), None)
        .await?;
    context
        .ft_contract()
        .with_user(&manager)
        .storage_deposit(Some(bob.to_near()), None)
        .await?;

    context
        .ft_contract()
        .with_user(context.ft_contract().contract().as_account())
        .tge_mint(&manager.to_near(), U128(total_amount * 2))
        .await?;

    let now = (context.lockup().block_timestamp_ms().await?) / 1_000;

    let lockup_term = 2 * 60 * 60;
    let cliff_end_timestamp = now + 365 * 24 * 60 * 60;
    let lockup = FtMessage::LockupCreate(LockupCreate {
        account_id: alice.to_near(),
        schedule: Schedule(vec![
            Checkpoint {
                timestamp: cliff_end_timestamp as _,
                balance: 0,
            },
            Checkpoint {
                timestamp: (cliff_end_timestamp + lockup_term) as _,
                balance: total_amount,
            },
        ]),
        vesting_schedule: None,
        is_adjustable: true,
    });
    let lockup_json = serde_json::to_string(&lockup).unwrap();

    context
        .ft_contract()
        .with_user(&manager)
        .ft_transfer_call_raw(
            context.lockup().contract().as_account().to_near(),
            U128(total_amount),
            None,
            lockup_json,
        )
        .await?;

    let alice_lockups = context.lockup().get_account_lockups(alice.to_near()).await?;
    let (index, lockup) = alice_lockups.first().expect("Must be a single index");
    let lockup_balance_before_recall = lockup.total_balance;
    let manager_balance_before_recall = context.ft_contract().ft_balance_of(manager.to_near()).await?.0;

    context
        .lockup()
        .with_user(&manager)
        .adjust(manager.to_near(), *index)
        .await?;

    let alice_lockups = context.lockup().get_account_lockups(alice.to_near()).await?;
    let lockup_balance_after_recall = alice_lockups.first().expect("Must be a single index").1.total_balance;
    let manager_balance_after_recall = context.ft_contract().ft_balance_of(manager.to_near()).await?.0;

    let lockup_balance_diff = lockup_balance_before_recall - lockup_balance_after_recall;
    let manager_balance_diff = manager_balance_after_recall - manager_balance_before_recall;

    assert_eq!(lockup_balance_diff, manager_balance_diff);

    Ok(())
}

#[tokio::test]
async fn test_revoke() -> anyhow::Result<()> {
    let mut context = prepare_contract().await?;

    let alice = context.alice().await?;
    let bob = context.bob().await?;
    let manager = context.manager().await?;

    let total_amount: Balance = 1_000_000_000_000_000;

    context
        .ft_contract()
        .with_user(&manager)
        .storage_deposit(Some(alice.to_near()), None)
        .await?;
    context
        .ft_contract()
        .with_user(&manager)
        .storage_deposit(Some(bob.to_near()), None)
        .await?;

    context
        .ft_contract()
        .with_user(context.ft_contract().contract().as_account())
        .tge_mint(&manager.to_near(), U128(total_amount * 2))
        .await?;

    let now = (context.lockup().block_timestamp_ms().await?) / 1_000;

    let lockup_term = 2 * 60 * 60;
    let cliff_end_timestamp = now + 365 * 24 * 60 * 60;
    let lockup = FtMessage::LockupCreate(LockupCreate {
        account_id: alice.to_near(),
        schedule: Schedule(vec![
            Checkpoint {
                timestamp: cliff_end_timestamp as _,
                balance: 0,
            },
            Checkpoint {
                timestamp: (cliff_end_timestamp + lockup_term) as _,
                balance: total_amount,
            },
        ]),
        vesting_schedule: None,
        is_adjustable: true,
    });
    let lockup_json = serde_json::to_string(&lockup).unwrap();

    context
        .ft_contract()
        .with_user(&manager)
        .ft_transfer_call_raw(
            context.lockup().contract().as_account().to_near(),
            U128(total_amount),
            None,
            lockup_json,
        )
        .await?;

    let alice_lockups = context.lockup().get_account_lockups(alice.to_near()).await?;
    let (index, _) = alice_lockups.first().expect("Must be a single index");
    let manager_balance_before_recall = context.ft_contract().ft_balance_of(manager.to_near()).await?.0;

    context
        .lockup()
        .with_user(&manager)
        .revoke(manager.to_near(), vec![*index])
        .await?;

    let alice_lockups = context.lockup().get_account_lockups(alice.to_near()).await?;
    assert!(alice_lockups.is_empty());

    let manager_balance_after_recall = context.ft_contract().ft_balance_of(manager.to_near()).await?.0;

    let manager_balance_diff = manager_balance_after_recall - manager_balance_before_recall;

    assert_eq!(total_amount, manager_balance_diff);

    Ok(())
}

#[async_trait]
trait FtContractHelper {
    async fn ft_transfer_call_raw(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> anyhow::Result<U128>;
}

#[async_trait]
impl FtContractHelper for SweatFt<'_> {
    async fn ft_transfer_call_raw(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> anyhow::Result<U128> {
        println!(
            "▶️ Transfer {:?} fungible tokens to {} with message: {}",
            amount, receiver_id, msg
        );

        let args = json!({
            "receiver_id": receiver_id,
            "amount": amount,
            "memo": memo,
            "msg": msg.to_string(),
        });

        let result = self
            .user_account()
            .unwrap()
            .call(self.contract().id(), "ft_transfer_call")
            .args_json(args)
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await?;

        log_result(result.clone());

        Ok(result.json()?)
    }
}
