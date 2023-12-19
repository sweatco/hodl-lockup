#![cfg(test)]

use integration_utils::{contract_call::ContractCall, integration_contract::IntegrationContract, misc::ToNear};
use model::{
    draft::{Draft, DraftGroupIndex, DraftGroupView, DraftIndex, DraftView},
    lockup::{LockupIndex, LockupView},
    lockup_api::LockupApiIntegration,
    schedule::Schedule,
    update::UpdateApiIntegration,
    view_api::LockupViewApiIntegration,
    TimestampSec, WrappedBalance,
};
use near_sdk::{json_types::Base58CryptoHash, serde_json::json, AccountId};
use near_workspaces::Contract;

pub struct LockupContract<'a> {
    contract: &'a Contract,
}

impl LockupApiIntegration for LockupContract<'_> {
    fn new(
        &self,
        token_account_id: AccountId,
        deposit_whitelist: Vec<AccountId>,
        draft_operators_whitelist: Option<Vec<AccountId>>,
        manager: AccountId,
    ) -> ContractCall<()> {
        self.make_call("new")
            .args_json(json!({
                "token_account_id": token_account_id,
                "deposit_whitelist": deposit_whitelist,
                "draft_operators_whitelist": draft_operators_whitelist,
                "manager": manager
            }))
            .unwrap()
    }

    fn claim(&mut self, amounts: Option<Vec<(LockupIndex, Option<WrappedBalance>)>>) -> ContractCall<WrappedBalance> {
        self.make_call("claim")
            .args_json(json!({
                "amounts": amounts
            }))
            .unwrap()
    }

    fn terminate(
        &mut self,
        lockup_index: LockupIndex,
        hashed_schedule: Option<Schedule>,
        termination_timestamp: Option<TimestampSec>,
    ) -> ContractCall<WrappedBalance> {
        self.make_call("terminate")
            .args_json(json!({
                "lockup_index": lockup_index,
                "hashed_schedule": hashed_schedule,
                "termination_timestamp": termination_timestamp
            }))
            .unwrap()
    }

    fn add_to_deposit_whitelist(
        &mut self,
        account_id: Option<AccountId>,
        account_ids: Option<Vec<AccountId>>,
    ) -> ContractCall<()> {
        self.make_call("add_to_deposit_whitelist")
            .args_json(json!({
                "account_id": account_id,
                "account_ids": account_ids,
            }))
            .unwrap()
    }

    fn remove_from_deposit_whitelist(
        &mut self,
        account_id: Option<AccountId>,
        account_ids: Option<Vec<AccountId>>,
    ) -> ContractCall<()> {
        self.make_call("remove_from_deposit_whitelist")
            .args_json(json!({
                "account_id": account_id,
                "account_ids": account_ids,
            }))
            .unwrap()
    }

    fn add_to_draft_operators_whitelist(&mut self, account_ids: Vec<AccountId>) -> ContractCall<()> {
        self.make_call("add_to_draft_operators_whitelist")
            .args_json(json!({
                "account_ids": account_ids
            }))
            .unwrap()
    }

    fn remove_from_draft_operators_whitelist(&mut self, account_ids: Vec<AccountId>) -> ContractCall<()> {
        self.make_call("remove_from_draft_operators_whitelist")
            .args_json(json!({
                "account_ids": account_ids
            }))
            .unwrap()
    }

    fn create_draft_group(&mut self) -> ContractCall<DraftGroupIndex> {
        self.make_call("create_draft_group")
    }

    fn create_draft(&mut self, draft: Draft) -> ContractCall<DraftIndex> {
        self.make_call("create_draft")
            .args_json(json!({
                "draft": draft
            }))
            .unwrap()
    }

    fn create_drafts(&mut self, drafts: Vec<Draft>) -> ContractCall<Vec<DraftIndex>> {
        self.make_call("create_drafts")
            .args_json(json!({
                "drafts": drafts
            }))
            .unwrap()
    }

    fn convert_draft(&mut self, draft_id: DraftIndex) -> ContractCall<LockupIndex> {
        self.make_call("convert_draft")
            .args_json(json!({
                "draft_id": draft_id
            }))
            .unwrap()
    }

    fn discard_draft_group(&mut self, draft_group_id: DraftGroupIndex) -> ContractCall<()> {
        self.make_call("discard_draft_group")
            .args_json(json!({
                "draft_group_id": draft_group_id
            }))
            .unwrap()
    }

    fn delete_drafts(&mut self, draft_ids: Vec<DraftIndex>) -> ContractCall<()> {
        self.make_call("delete_drafts")
            .args_json(json!({
                "draft_ids": draft_ids
            }))
            .unwrap()
    }
}

impl<'a> LockupViewApiIntegration for LockupContract<'a> {
    fn get_token_account_id(&self) -> ContractCall<AccountId> {
        self.make_call("get_token_account_id")
    }

    fn get_account_lockups(&self, account_id: AccountId) -> ContractCall<Vec<(LockupIndex, LockupView)>> {
        self.make_call("get_account_lockups")
            .args_json(json!({
                "account_id": account_id
            }))
            .unwrap()
    }

    fn get_lockup(&self, index: LockupIndex) -> ContractCall<Option<LockupView>> {
        self.make_call("get_lockup")
            .args_json(json!({
                "index": index
            }))
            .unwrap()
    }

    fn get_lockups(&self, indices: Vec<LockupIndex>) -> ContractCall<Vec<(LockupIndex, LockupView)>> {
        self.make_call("get_lockups")
            .args_json(json!({
                "indices": indices
            }))
            .unwrap()
    }

    fn get_num_lockups(&self) -> ContractCall<u32> {
        self.make_call("get_num_lockups")
    }

    fn get_lockups_paged(
        &self,
        from_index: Option<LockupIndex>,
        limit: Option<LockupIndex>,
    ) -> ContractCall<Vec<(LockupIndex, LockupView)>> {
        self.make_call("get_lockups_paged")
            .args_json(json!({
                "from_index": from_index,
                "limit": limit
            }))
            .unwrap()
    }

    fn get_deposit_whitelist(&self) -> ContractCall<Vec<AccountId>> {
        self.make_call("get_deposit_whitelist")
    }

    fn get_draft_operators_whitelist(&self) -> ContractCall<Vec<AccountId>> {
        self.make_call("get_draft_operators_whitelist")
    }

    fn hash_schedule(&self, schedule: Schedule) -> ContractCall<Base58CryptoHash> {
        self.make_call("hash_schedule")
            .args_json(json!({
                "schedule": schedule
            }))
            .unwrap()
    }

    fn validate_schedule(
        &self,
        schedule: Schedule,
        total_balance: WrappedBalance,
        termination_schedule: Option<Schedule>,
    ) -> ContractCall<()> {
        self.make_call("validate_schedule")
            .args_json(json!({
                "schedule": schedule,
                "total_balance": total_balance,
                "termination_schedule": termination_schedule,
            }))
            .unwrap()
    }

    fn get_next_draft_group_id(&self) -> ContractCall<DraftGroupIndex> {
        self.make_call("get_next_draft_group_id")
    }

    fn get_next_draft_id(&self) -> ContractCall<DraftGroupIndex> {
        self.make_call("get_next_draft_id")
    }

    fn get_num_draft_groups(&self) -> ContractCall<u32> {
        self.make_call("get_num_draft_groups")
    }

    fn get_draft_group(&self, index: DraftGroupIndex) -> ContractCall<Option<DraftGroupView>> {
        self.make_call("get_draft_group")
            .args_json(json!({
                "index": index
            }))
            .unwrap()
    }

    fn get_draft_groups_paged(
        &self,
        from_index: Option<DraftGroupIndex>,
        to_index: Option<DraftGroupIndex>,
    ) -> ContractCall<Vec<(DraftGroupIndex, DraftGroupView)>> {
        self.make_call("get_draft_groups_paged")
            .args_json(json!({
                "from_index": from_index,
                "to_index": to_index
            }))
            .unwrap()
    }

    fn get_draft(&self, index: DraftIndex) -> ContractCall<Option<DraftView>> {
        self.make_call("get_draft")
            .args_json(json!({
                "index": index
            }))
            .unwrap()
    }

    fn get_drafts(&self, indices: Vec<DraftIndex>) -> ContractCall<Vec<(DraftIndex, DraftView)>> {
        self.make_call("get_drafts")
            .args_json(json!({
                "indices": indices
            }))
            .unwrap()
    }

    fn get_version(&self) -> ContractCall<String> {
        self.make_call("get_version")
    }
}

impl<'a> UpdateApiIntegration for LockupContract<'a> {
    fn update_contract(&mut self, code: Vec<u8>) -> ContractCall<()> {
        self.make_call("update_contract").args(code)
    }

    fn set_multisig(&mut self, multisig: AccountId) -> ContractCall<()> {
        self.make_call("set_multisig")
            .args_json(json!({
                "multisig": multisig
            }))
            .unwrap()
    }
}

impl<'a> IntegrationContract<'a> for LockupContract<'a> {
    fn with_contract(contract: &'a Contract) -> Self {
        Self { contract }
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
