#![cfg(test)]

use anyhow::Result;
use hodl_model::{lockup::LockupCreate, schedule::Schedule};
use serde_json::to_string;

use crate::{
    context::{prepare_contract, Context},
    ft, lockup,
};

#[tokio::test]
async fn migration() -> Result<()> {
    let mut context = prepare_contract().await?;

    create_lockups(&mut context).await?;

    dbg!(lockup::get_num_lockups(&context.lockup).await?);

    Ok(())
}

async fn create_lockups(context: &mut Context) -> Result<()> {
    let manager = context.manager().await?;

    ft::tge_mint(&context.ft, manager.id(), 1_000_000_000).await?;

    for i in 0..20 {
        let account = context.account(&format!("bob_{i}")).await?;

        ft::storage_deposit(&context.ft, account.id()).await?;

        let message = LockupCreate {
            account_id: account.id().as_str().parse().unwrap(),
            schedule: Schedule::new_unlocked(100),
            vesting_schedule: None,
        };

        ft::ft_transfer_call(
            &context.ft,
            &manager,
            context.lockup.id(),
            100,
            &to_string(&message).unwrap(),
        )
        .await?;
    }

    Ok(())
}
