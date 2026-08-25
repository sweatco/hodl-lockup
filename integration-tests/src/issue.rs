#![cfg(test)]

use anyhow::Result;
use hodl_model::ONE_YEAR_SEC;
use near_sdk::json_types::U128;
use near_workspaces::AccountId;
use rand::{distributions::Alphanumeric, Rng};

use crate::{context::prepare_contract, ft, lockup};

#[tokio::test]
async fn issue_lockups() -> Result<()> {
    const LOCKUPS_COUNT: usize = 70;
    const LOCKUP_AMOUNT: u128 = 100_000_000 * 10u128.pow(18);
    const ISSUE_DATE: u32 = 1725210000;

    let mut context = prepare_contract().await?;
    let manager = context.manager().await?;

    ft::ft_transfer(
        &context.ft,
        &manager,
        context.lockup.id(),
        LOCKUP_AMOUNT * LOCKUPS_COUNT as u128,
    )
    .await?;

    let amounts: Vec<(AccountId, U128)> = (0..LOCKUPS_COUNT)
        .map(|_| (generate_account_id(), U128(LOCKUP_AMOUNT)))
        .collect();
    lockup::issue(&context.lockup, &manager, ISSUE_DATE, &amounts).await?;

    assert_eq!(LOCKUPS_COUNT as u32, lockup::get_num_lockups(&context.lockup).await?);

    let lockup_view = lockup::get_lockup(&context.lockup, 0).await?.unwrap();
    assert_eq!(LOCKUP_AMOUNT, lockup_view.schedule.total_balance());

    let lockup_view = lockup::get_lockup(&context.lockup, 10).await?.unwrap();
    assert_eq!(LOCKUP_AMOUNT, lockup_view.schedule.total_balance());
    assert_eq!(
        ISSUE_DATE + ONE_YEAR_SEC,
        lockup_view.schedule.0.first().unwrap().timestamp
    );
    assert_eq!(
        ISSUE_DATE + 4 * ONE_YEAR_SEC,
        lockup_view.schedule.0.last().unwrap().timestamp
    );

    Ok(())
}

fn generate_account_id() -> AccountId {
    AccountId::try_from(
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect::<String>()
            .to_lowercase(),
    )
    .unwrap()
}
