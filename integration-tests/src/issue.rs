#![cfg(test)]

use std::str::FromStr;

use anyhow::Result;
use hodl_model::{
    api::{IssueApiIntegration, LockupViewApiIntegration},
    ONE_YEAR_SEC,
};
use near_sdk::AccountId;
use nitka::near_sdk::json_types::U128;
use rand::{distributions::Alphanumeric, Rng};
use sweat_model::FungibleTokenCoreIntegration;

use crate::context::{prepare_contract, IntegrationContext};

#[tokio::test]
async fn issue_lockups() -> Result<()> {
    const LOCKUPS_COUNT: usize = 70;
    const LOCKUP_AMOUNT: u128 = 100_000_000 * 10u128.pow(18);
    const ISSUE_DATE: u32 = 1725210000;

    let mut context = prepare_contract().await?;
    let manager = context.manager().await?;

    context
        .ft_contract()
        .ft_transfer(
            context.lockup().contract.id().clone(),
            (LOCKUP_AMOUNT * LOCKUPS_COUNT as u128).into(),
            None,
        )
        .with_user(&manager)
        .await?;

    let amounts: Vec<(AccountId, U128)> = (0..LOCKUPS_COUNT)
        .map(|_| (generate_account_id(), U128(LOCKUP_AMOUNT)))
        .collect();
    context
        .lockup()
        .issue(ISSUE_DATE, amounts.clone())
        .with_user(&manager)
        .await?;

    assert_eq!(LOCKUPS_COUNT as u64, context.lockup().get_num_lockups().await? as u64);

    let lockup = context.lockup().get_lockup(0).await?.unwrap();
    assert_eq!(LOCKUP_AMOUNT, lockup.schedule.total_balance());

    let lockup = context.lockup().get_lockup(10).await?.unwrap();
    assert_eq!(LOCKUP_AMOUNT, lockup.schedule.total_balance());
    assert_eq!(ISSUE_DATE + ONE_YEAR_SEC, lockup.schedule.0.first().unwrap().timestamp);
    assert_eq!(
        ISSUE_DATE + 4 * ONE_YEAR_SEC,
        lockup.schedule.0.last().unwrap().timestamp
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
