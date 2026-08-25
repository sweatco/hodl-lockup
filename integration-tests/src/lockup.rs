#![cfg(test)]

use anyhow::Result;
use hodl_model::{
    api::RoleAssignments,
    lockup::{LockupIndex, LockupView},
};
use near_sdk::json_types::U128;
use near_workspaces::{AccountId, Contract};
use serde_json::json;

pub async fn new(
    lockup: &Contract,
    token_account_id: &AccountId,
    super_admin_account_id: &AccountId,
    roles: &RoleAssignments,
) -> Result<()> {
    lockup
        .call("new")
        .args_json(json!({
            "token_account_id": token_account_id,
            "super_admin_account_id": super_admin_account_id,
            "roles": roles,
        }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;
    Ok(())
}

pub async fn issue(
    lockup: &Contract,
    caller: &near_workspaces::Account,
    issue_date: u32,
    amounts: &[(AccountId, U128)],
) -> Result<()> {
    caller
        .call(lockup.id(), "issue")
        .args_json(json!({ "issue_date": issue_date, "amounts": amounts }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;
    Ok(())
}

pub async fn get_num_lockups(lockup: &Contract) -> Result<u32> {
    Ok(lockup.view("get_num_lockups").await?.json()?)
}

pub async fn get_lockup(lockup: &Contract, index: LockupIndex) -> Result<Option<LockupView>> {
    Ok(lockup
        .view("get_lockup")
        .args_json(json!({ "index": index }))
        .await?
        .json()?)
}
