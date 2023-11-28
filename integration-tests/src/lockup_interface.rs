#![cfg(test)]

use anyhow::Result;
use async_trait::async_trait;
use integration_utils::{integration_contract::IntegrationContract, misc::ToNear};
use model::{
    draft::{Draft, DraftGroupIndex, DraftIndex},
    lockup::LockupIndex,
    lockup_api::LockupApiIntegration,
    schedule::Schedule,
    TimestampSec, WrappedBalance,
};
use near_sdk::{serde_json::json, AccountId};
use near_workspaces::{Account, Contract};

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
        self.call(
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
        self.call(
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
        self.call(
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
        self.call(
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
        self.call(
            "remove_from_deposit_whitelist",
            json!({
                "account_id": account_id,
                "account_ids": account_ids,
            }),
        )
        .await
    }

    async fn add_to_draft_operators_whitelist(&mut self, account_ids: Vec<AccountId>) -> Result<()> {
        self.call(
            "add_to_draft_operators_whitelist",
            json!({
                "account_ids": account_ids
            }),
        )
        .await
    }

    async fn remove_from_draft_operators_whitelist(&mut self, account_ids: Vec<AccountId>) -> Result<()> {
        self.call(
            "remove_from_draft_operators_whitelist",
            json!({
                "account_ids": account_ids
            }),
        )
        .await
    }

    async fn create_draft_group(&mut self) -> Result<DraftGroupIndex> {
        self.call("create_draft_group", ()).await
    }

    async fn create_draft(&mut self, draft: Draft) -> Result<DraftIndex> {
        self.call(
            "create_draft",
            json!({
                "draft": draft
            }),
        )
        .await
    }

    async fn create_drafts(&mut self, drafts: Vec<Draft>) -> Result<Vec<DraftIndex>> {
        self.call(
            "create_drafts",
            json!({
                "drafts": drafts
            }),
        )
        .await
    }

    async fn convert_draft(&mut self, draft_id: DraftIndex) -> Result<LockupIndex> {
        self.call(
            "convert_draft",
            json!({
                "draft_id": draft_id
            }),
        )
        .await
    }

    async fn discard_draft_group(&mut self, draft_group_id: DraftGroupIndex) -> Result<()> {
        self.call(
            "discard_draft_group",
            json!({
                "draft_group_id": draft_group_id
            }),
        )
        .await
    }

    async fn delete_drafts(&mut self, draft_ids: Vec<DraftIndex>) -> Result<()> {
        self.call(
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

    fn with_user(&mut self, account: &Account) -> &mut Self {
        self.account = account.clone().into();
        self
    }

    fn user_account(&self) -> Option<Account> {
        self.account.clone()
    }

    fn contract(&self) -> &'a Contract {
        self.contract
    }
}

pub trait GetContractAccount {
    fn contract_account(&self) -> AccountId;
}

impl<'a, T: IntegrationContract<'a>> GetContractAccount for T {
    fn contract_account(&self) -> AccountId {
        self.contract().as_account().to_near()
    }
}
