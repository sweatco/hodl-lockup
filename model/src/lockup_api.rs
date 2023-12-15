use integration_trait::make_integration_version;
use near_sdk::{assert_one_yocto, env, json_types::U128, AccountId, PromiseOrValue};

use crate::{
    draft::{Draft, DraftGroupIndex, DraftIndex},
    lockup::{Lockup, LockupIndex},
    schedule::Schedule,
    TimestampSec, WrappedBalance,
};

#[make_integration_version]
pub trait LockupApi {
    fn new(
        token_account_id: AccountId,
        deposit_whitelist: Vec<AccountId>,
        draft_operators_whitelist: Option<Vec<AccountId>>,
    ) -> Self;

    fn claim(&mut self, amounts: Option<Vec<(LockupIndex, Option<WrappedBalance>)>>) -> PromiseOrValue<WrappedBalance>;

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

    fn make_lockup_terminable(&mut self, beneficiary_id: AccountId, lockup_index: LockupIndex);

    fn recall(&mut self, beneficiary_id: AccountId, lockup_index: LockupIndex) -> PromiseOrValue<()>;
}
