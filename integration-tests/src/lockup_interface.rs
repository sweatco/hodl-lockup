#![cfg(test)]

use anyhow::Result;
use async_trait::async_trait;
use integration_utils::{integration_contract::IntegrationContract, misc::ToNear};
use model::{
    adjust_api::AdjustApiIntegration,
    draft::{Draft, DraftGroupIndex, DraftGroupView, DraftIndex, DraftView},
    lockup::{LockupIndex, LockupView},
    lockup_api::LockupApiIntegration,
    schedule::Schedule,
    update::UpdateApiIntegration,
    view_api::{LockupViewApi, LockupViewApiIntegration},
    TimestampSec, WrappedBalance,
};
use near_sdk::{json_types::Base58CryptoHash, serde_json::json, AccountId, Timestamp};
use near_workspaces::{result::ExecutionFinalResult, types::NearToken, Account, Contract};

use crate::common::log_result;

pub const LOCKUP_CONTRACT: &str = "hodl_lockup";

pub struct LockupContract<'a> {
    account: Option<Account>,
    contract: &'a Contract,
}

impl LockupContract<'_> {
    pub(crate) async fn terminate_raw(
        &mut self,
        lockup_index: LockupIndex,
        hashed_schedule: Option<Schedule>,
        termination_timestamp: Option<TimestampSec>,
    ) -> Result<ExecutionFinalResult> {
        println!("▶️ terminate");

        let result = self
            .user_account()
            .expect("User account is required")
            .call(self.contract.id(), "terminate")
            .args_json(json!({
                "lockup_index": lockup_index,
                "hashed_schedule": hashed_schedule,
                "termination_timestamp": termination_timestamp
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await?;

        log_result(result.clone());

        Ok(result)
    }

    pub(crate) async fn block_timestamp_ms(&self) -> anyhow::Result<Timestamp> {
        println!("▶️ block_timestamp_ms");
        let result = self.contract.view("block_timestamp_ms").await?.json()?;
        println!("   ✅ {:?}", result);
        Ok(result)
    }
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
        let result = self
            .terminate_raw(lockup_index, hashed_schedule, termination_timestamp)
            .await?
            .json()?;

        Ok(result)
    }

    async fn add_to_deposit_whitelist(
        &mut self,
        account_id: Option<AccountId>,
        account_ids: Option<Vec<AccountId>>,
    ) -> Result<()> {
        println!("▶️ add_to_deposit_whitelist");

        let account = self.user_account().expect("User account is required");
        let result = account
            .call(self.contract.id(), "add_to_deposit_whitelist")
            .args_json(json!({
                "account_id": account_id,
                "account_ids": account_ids,
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await?;

        log_result(result.clone());

        Ok(result.json()?)
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

    async fn make_lockup_terminable(&mut self, beneficiary_id: AccountId, lockup_index: LockupIndex) -> Result<()> {
        println!("▶️ make_lockup_terminable");

        let account = self.user_account().expect("User account is required");
        let result = account
            .call(self.contract.id(), "make_lockup_terminable")
            .args_json(json!({
                "beneficiary_id": beneficiary_id,
                "lockup_index": lockup_index,
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await?;

        log_result(result.clone());

        Ok(())
    }
}

#[async_trait]
impl AdjustApiIntegration for LockupContract<'_> {
    async fn adjust(&mut self, beneficiary_id: AccountId, lockup_index: LockupIndex) -> Result<()> {
        println!("▶️ recall");

        let account = self.user_account().expect("User account is required");
        let result = account
            .call(self.contract.id(), "adjust")
            .args_json(json!({
                "beneficiary_id": beneficiary_id,
                "lockup_index": lockup_index,
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await?;

        log_result(result.clone());

        Ok(())
    }

    async fn revoke(&mut self, beneficiary_id: AccountId, lockup_indices: Vec<LockupIndex>) -> Result<()> {
        println!("▶️ revoke");

        let account = self.user_account().expect("User account is required");
        let result = account
            .call(self.contract.id(), "revoke")
            .args_json(json!({
                "beneficiary_id": beneficiary_id,
                "lockup_indices": lockup_indices,
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await?;

        log_result(result.clone());

        Ok(())
    }
}

#[async_trait]
impl LockupViewApiIntegration for LockupContract<'_> {
    async fn get_token_account_id(&self) -> Result<AccountId> {
        todo!()
    }

    async fn get_account_lockups(&self, account_id: AccountId) -> Result<Vec<(LockupIndex, LockupView)>> {
        self.call(
            "get_account_lockups",
            json!({
                "account_id": account_id
            }),
        )
        .await
    }

    async fn get_lockup(&self, index: LockupIndex) -> Result<Option<LockupView>> {
        todo!()
    }

    async fn get_lockups(&self, indices: Vec<LockupIndex>) -> Result<Vec<(LockupIndex, LockupView)>> {
        todo!()
    }

    async fn get_num_lockups(&self) -> Result<u32> {
        todo!()
    }

    async fn get_lockups_paged(
        &self,
        from_index: Option<LockupIndex>,
        limit: Option<LockupIndex>,
    ) -> Result<Vec<(LockupIndex, LockupView)>> {
        todo!()
    }

    async fn get_deposit_whitelist(&self) -> Result<Vec<AccountId>> {
        todo!()
    }

    async fn get_draft_operators_whitelist(&self) -> Result<Vec<AccountId>> {
        todo!()
    }

    async fn hash_schedule(&self, schedule: Schedule) -> Result<Base58CryptoHash> {
        todo!()
    }

    async fn validate_schedule(
        &self,
        schedule: Schedule,
        total_balance: WrappedBalance,
        termination_schedule: Option<Schedule>,
    ) -> Result<()> {
        todo!()
    }

    async fn get_next_draft_group_id(&self) -> Result<DraftGroupIndex> {
        todo!()
    }

    async fn get_next_draft_id(&self) -> Result<DraftGroupIndex> {
        todo!()
    }

    async fn get_num_draft_groups(&self) -> Result<u32> {
        todo!()
    }

    async fn get_draft_group(&self, index: DraftGroupIndex) -> Result<Option<DraftGroupView>> {
        todo!()
    }

    async fn get_draft_groups_paged(
        &self,
        from_index: Option<DraftGroupIndex>,
        to_index: Option<DraftGroupIndex>,
    ) -> Result<Vec<(DraftGroupIndex, DraftGroupView)>> {
        todo!()
    }

    async fn get_draft(&self, index: DraftIndex) -> Result<Option<DraftView>> {
        todo!()
    }

    async fn get_drafts(&self, indices: Vec<DraftIndex>) -> Result<Vec<(DraftIndex, DraftView)>> {
        todo!()
    }

    async fn get_version(&self) -> Result<String> {
        todo!()
    }
}

#[async_trait]
impl<'a> UpdateApiIntegration for LockupContract<'a> {
    async fn update_contract(&mut self, code: Vec<u8>) -> Result<()> {
        println!("▶️ update_contract");

        let result = self
            .user_account()
            .unwrap()
            .call(self.contract().id(), "update_contract")
            .args(code)
            .max_gas()
            .transact()
            .await?
            .into_result()?;

        println!("Result: {result:?}");

        Ok(())
    }

    async fn set_multisig(&mut self, multisig: AccountId) -> Result<()> {
        self.call(
            "set_multisig",
            json!({
                "multisig": multisig
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
