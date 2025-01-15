#![cfg(test)]

use anyhow::Result;
use hodl_model::{api::LockupViewApiIntegration, ft_message::FtMessage, lockup::LockupCreate, schedule::Schedule};
use near_sdk::{serde_json::to_string, NearToken};
use nitka::misc::ToNear;
use sweat_model::{FungibleTokenCoreIntegration, StorageManagementIntegration, SweatApiIntegration};

use crate::context::{prepare_contract, Context, IntegrationContext};

#[tokio::test]
async fn migration() -> Result<()> {
    let mut context = prepare_contract().await?;

    create_lockups(&mut context).await?;

    dbg!(context.lockup().get_num_lockups().await?);

    Ok(())
}

async fn create_lockups(context: &mut Context) -> Result<()> {
    let manager = context.manager().await?;

    context
        .ft_contract()
        .tge_mint(&manager.to_near(), 1_000_000_000.into())
        .await?;

    for i in 0..20 {
        let account = context.account(&format!("bob_{i}")).await?;

        context
            .ft_contract()
            .storage_deposit(account.to_near().into(), None)
            .await?;

        let message = FtMessage::LockupCreate(LockupCreate {
            account_id: account.to_near(),
            schedule: Schedule::new_unlocked(100),
            vesting_schedule: None,
        });

        context
            .ft_contract()
            .ft_transfer_call(
                context.lockup().contract.id().clone(),
                100.into(),
                None,
                to_string(&message).unwrap(),
            )
            .deposit(NearToken::from_yoctonear(1))
            .with_user(&manager)
            .await?;
    }

    Ok(())
}
