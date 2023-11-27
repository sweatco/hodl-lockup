use integration_trait::make_integration_version;
use near_sdk::{json_types::Base58CryptoHash, AccountId};

use crate::{
    draft::{DraftGroupIndex, DraftGroupView, DraftIndex, DraftView},
    lockup::{LockupIndex, LockupView},
    schedule::Schedule,
    WrappedBalance,
};

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
