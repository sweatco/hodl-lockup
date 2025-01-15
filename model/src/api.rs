use near_sdk::{json_types::Base58CryptoHash, PromiseOrValue};
#[cfg(not(feature = "integration-test"))]
use near_sdk::{json_types::U128, AccountId};
#[cfg(feature = "integration-test")]
use nitka::near_sdk::{json_types::U128, AccountId};
use nitka_proc::make_integration_version;

use crate::{
    draft::{Draft, DraftGroupIndex, DraftGroupView, DraftIndex, DraftView},
    lockup::{LockupClaim, LockupIndex, LockupView},
    schedule::Schedule,
    TimestampSec, WrappedBalance,
};

#[cfg(feature = "integration-test")]
pub struct HodlContract<'a> {
    pub contract: &'a near_workspaces::Contract,
}

#[make_integration_version]
pub trait IssueApi {
    fn issue(&mut self, issue_date: TimestampSec, amounts: Vec<(AccountId, U128)>);
}

#[make_integration_version]
pub trait LockupApi {
    fn new(
        token_account_id: AccountId,
        deposit_whitelist: Vec<AccountId>,
        draft_operators_whitelist: Option<Vec<AccountId>>,
        manager: AccountId,
    ) -> Self;

    fn claim(&mut self, amounts: Option<Vec<(LockupIndex, Option<WrappedBalance>)>>) -> Vec<LockupClaim>;

    fn terminate(
        &mut self,
        lockup_index: LockupIndex,
        hashed_schedule: Option<Schedule>,
        termination_timestamp: Option<TimestampSec>,
    ) -> PromiseOrValue<WrappedBalance>;

    // preserving both options for API compatibility
    fn add_to_deposit_whitelist(&mut self, account_id: Option<AccountId>, account_ids: Option<Vec<AccountId>>);

    // preserving both options for API compatibility
    fn remove_from_deposit_whitelist(&mut self, account_id: Option<AccountId>, account_ids: Option<Vec<AccountId>>);

    fn add_to_draft_operators_whitelist(&mut self, account_ids: Vec<AccountId>);

    fn remove_from_draft_operators_whitelist(&mut self, account_ids: Vec<AccountId>);

    fn create_draft_group(&mut self) -> DraftGroupIndex;

    fn create_draft(&mut self, draft: Draft) -> DraftIndex;

    fn create_drafts(&mut self, drafts: Vec<Draft>) -> Vec<DraftIndex>;

    fn convert_draft(&mut self, draft_id: DraftIndex) -> LockupIndex;

    fn discard_draft_group(&mut self, draft_group_id: DraftGroupIndex);

    fn delete_drafts(&mut self, draft_ids: Vec<DraftIndex>);
}

#[make_integration_version]
pub trait LockupViewApi {
    fn get_token_account_id(&self) -> AccountId;

    fn get_account_lockups(&self, account_id: AccountId) -> Vec<(LockupIndex, LockupView)>;

    fn get_lockup(&self, index: LockupIndex) -> Option<LockupView>;
    fn get_lockups(&self, indices: Vec<LockupIndex>) -> Vec<(LockupIndex, LockupView)>;

    fn get_num_lockups(&self) -> u32;

    fn get_lockups_paged(
        &self,
        from_index: Option<LockupIndex>,
        limit: Option<LockupIndex>,
    ) -> Vec<(LockupIndex, LockupView)>;

    fn get_deposit_whitelist(&self) -> Vec<AccountId>;

    fn get_draft_operators_whitelist(&self) -> Vec<AccountId>;

    fn hash_schedule(&self, schedule: Schedule) -> Base58CryptoHash;

    fn validate_schedule(
        &self,
        schedule: Schedule,
        total_balance: WrappedBalance,
        termination_schedule: Option<Schedule>,
    );

    fn get_next_draft_group_id(&self) -> DraftGroupIndex;

    fn get_next_draft_id(&self) -> DraftGroupIndex;

    fn get_num_draft_groups(&self) -> u32;

    fn get_draft_group(&self, index: DraftGroupIndex) -> Option<DraftGroupView>;

    fn get_draft_groups_paged(
        &self,
        // not the draft_id, but internal index used inside the LookupMap struct
        from_index: Option<DraftGroupIndex>,
        to_index: Option<DraftGroupIndex>,
    ) -> Vec<(DraftGroupIndex, DraftGroupView)>;

    fn get_draft(&self, index: DraftIndex) -> Option<DraftView>;

    fn get_drafts(&self, indices: Vec<DraftIndex>) -> Vec<(DraftIndex, DraftView)>;

    fn get_version(&self) -> String;
}
