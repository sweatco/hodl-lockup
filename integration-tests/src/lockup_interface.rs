#![cfg(test)]

use anyhow::Result;
use async_trait::async_trait;
use integration_utils::integration_contract::IntegrationContract;
use model::{
    draft::{Draft, DraftGroupIndex, DraftIndex},
    lockup::LockupIndex,
    lockup_api::LockupApiIntegration,
    schedule::Schedule,
    TimestampSec, WrappedBalance,
};
use near_sdk::AccountId;
use near_workspaces::{Account, Contract};
use serde_json::json;

pub const LOCKUP_CONTRACT: &str = "hodl_lockup";

pub struct LockupContract<'a> {
    account: Option<Account>,
    contract: &'a Contract,
}

#[async_trait]
impl LockupApiIntegration for LockupContract<'_> {
    async fn new(
        &self,
        token_account_id: AccountId,
        deposit_whitelist: Vec<AccountId>,
        draft_operators_whitelist: Option<Vec<AccountId>>,
    ) -> Result<()> {
        self.call_contract(
            "new",
            json!({
                "token_account_id": token_account_id,
                "deposit_whitelist": deposit_whitelist,
                "draft_operators_whitelist": draft_operators_whitelist
            }),
        )
        .await
    }

    async fn claim(&mut self, amounts: Option<Vec<(LockupIndex, Option<WrappedBalance>)>>) -> Result<WrappedBalance> {
        self.call_contract(
            "claim",
            json!({
                "amounts": amounts
            }),
        )
        .await
    }

    async fn terminate(
        &mut self,
        lockup_index: LockupIndex,
        hashed_schedule: Option<Schedule>,
        termination_timestamp: Option<TimestampSec>,
    ) -> Result<WrappedBalance> {
        self.call_contract(
            "terminate",
            json!({
                "lockup_index": lockup_index,
                "hashed_schedule": hashed_schedule,
                "termination_timestamp": termination_timestamp
            }),
        )
        .await
    }

    async fn add_to_deposit_whitelist(
        &mut self,
        account_id: Option<AccountId>,
        account_ids: Option<Vec<AccountId>>,
    ) -> Result<()> {
        self.call_contract(
            "add_to_deposit_whitelist",
            json!({
                "account_id": account_id,
                "account_ids": account_ids,
            }),
        )
        .await
    }

    async fn remove_from_deposit_whitelist(
        &mut self,
        account_id: Option<AccountId>,
        account_ids: Option<Vec<AccountId>>,
    ) -> Result<()> {
        self.call_contract(
            "remove_from_deposit_whitelist",
            json!({
                "account_id": account_id,
                "account_ids": account_ids,
            }),
        )
        .await
    }

    async fn add_to_draft_operators_whitelist(&mut self, account_ids: Vec<AccountId>) -> Result<()> {
        self.call_contract(
            "add_to_draft_operators_whitelist",
            json!({
                "account_ids": account_ids
            }),
        )
        .await
    }

    async fn remove_from_draft_operators_whitelist(&mut self, account_ids: Vec<AccountId>) -> Result<()> {
        self.call_contract(
            "remove_from_draft_operators_whitelist",
            json!({
                "account_ids": account_ids
            }),
        )
        .await
    }

    async fn create_draft_group(&mut self) -> Result<DraftGroupIndex> {
        self.call_contract("create_draft_group", ()).await
    }

    async fn create_draft(&mut self, draft: Draft) -> Result<DraftIndex> {
        self.call_contract(
            "create_draft",
            json!({
                "draft": draft
            }),
        )
        .await
    }

    async fn create_drafts(&mut self, drafts: Vec<Draft>) -> Result<Vec<DraftIndex>> {
        self.call_contract(
            "create_drafts",
            json!({
                "drafts": drafts
            }),
        )
        .await
    }

    async fn convert_draft(&mut self, draft_id: DraftIndex) -> Result<LockupIndex> {
        self.call_contract(
            "convert_draft",
            json!({
                "draft_id": draft_id
            }),
        )
        .await
    }

    async fn discard_draft_group(&mut self, draft_group_id: DraftGroupIndex) -> Result<()> {
        self.call_contract(
            "discard_draft_group",
            json!({
                "draft_group_id": draft_group_id
            }),
        )
        .await
    }

    async fn delete_drafts(&mut self, draft_ids: Vec<DraftIndex>) -> Result<()> {
        self.call_contract(
            "delete_drafts",
            json!({
                "draft_ids": draft_ids
            }),
        )
        .await
    }
}

impl<'a> IntegrationContract<'a> for LockupContract<'a> {
    fn with_contract(contract: &'a Contract) -> Self {
        Self {
            contract,
            account: None,
        }
    }

    fn with_user(mut self, account: &Account) -> Self {
        self.account = account.clone().into();
        self
    }

    fn user_account(&self) -> Account {
        self.account
            .as_ref()
            .expect("Set account with `user` method first")
            .clone()
    }

    fn contract(&self) -> &'a Contract {
        self.contract
    }
}
