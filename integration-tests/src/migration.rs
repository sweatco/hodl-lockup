#![cfg(test)]

use anyhow::Result;
use integration_utils::misc::ToNear;
use model::{ft_message::FtMessage, lockup::LockupCreate, schedule::Schedule, view_api::LockupViewApiIntegration};
use near_sdk::serde_json::to_string;
use sweat_model::{FungibleTokenCoreIntegration, StorageManagementIntegration, SweatApiIntegration};

use crate::{
    context::{prepare_contract, Context, IntegrationContext},
    lockup_interface::GetContractAccount,
};

#[tokio::test]
async fn migration() -> Result<()> {
    let mut context = prepare_contract().await?;

    create_lockups(&mut context).await?;

    dbg!(context.lockup().get_num_lockups().call().await?);

    Ok(())
}

async fn create_lockups(context: &mut Context) -> Result<()> {
    let manager = context.manager().await?;

    context
        .ft_contract()
        .tge_mint(&manager.to_near(), 1_000_000_000.into())
        .call()
        .await?;

    for i in 0..20 {
        let account = context.account(&format!("bob_{i}")).await?;

        context
            .ft_contract()
            .storage_deposit(account.to_near().into(), None)
            .call()
            .await?;

        let message = FtMessage::LockupCreate(LockupCreate {
            account_id: account.to_near(),
            schedule: Schedule::new_unlocked(100),
            vesting_schedule: None,
        });

        context
            .ft_contract()
            .ft_transfer_call(
                context.lockup().contract_account(),
                100.into(),
                None,
                to_string(&message).unwrap(),
            )
            .with_user(&manager)
            .call()
            .await?;
    }

    Ok(())
}
