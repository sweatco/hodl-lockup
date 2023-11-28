use model::{
    draft::{DraftGroupIndex, DraftGroupView, DraftIndex, DraftView},
    lockup::{LockupIndex, LockupView},
    schedule::Schedule,
    view_api::LockupViewApi,
    WrappedBalance,
};

use crate::{near_bindgen, AccountId, Base58CryptoHash, Contract, ContractExt, Into, VERSION};

#[near_bindgen]
impl LockupViewApi for Contract {
    fn get_token_account_id(&self) -> AccountId {
        self.token_account_id.clone()
    }

    fn get_account_lockups(&self, account_id: AccountId) -> Vec<(LockupIndex, LockupView)> {
        self.internal_get_account_lockups(&account_id)
            .into_iter()
            .map(|(lockup_index, lockup)| (lockup_index, lockup.into()))
            .collect()
    }

    fn get_lockup(&self, index: LockupIndex) -> Option<LockupView> {
        self.lockups.get(u64::from(index)).map(Into::into)
    }

    fn get_lockups(&self, indices: Vec<LockupIndex>) -> Vec<(LockupIndex, LockupView)> {
        indices
            .into_iter()
            .filter_map(|index| self.get_lockup(index).map(|lockup| (index, lockup)))
            .collect()
    }

    fn get_num_lockups(&self) -> u32 {
        self.lockups.len().try_into().unwrap()
    }

    fn get_lockups_paged(
        &self,
        from_index: Option<LockupIndex>,
        limit: Option<LockupIndex>,
    ) -> Vec<(LockupIndex, LockupView)> {
        let from_index = from_index.unwrap_or(0);
        let limit = limit.unwrap_or(self.get_num_lockups());
        (from_index..std::cmp::min(self.get_num_lockups(), limit))
            .filter_map(|index| self.get_lockup(index).map(|lockup| (index, lockup)))
            .collect()
    }

    fn get_deposit_whitelist(&self) -> Vec<AccountId> {
        self.deposit_whitelist.to_vec()
    }

    fn get_draft_operators_whitelist(&self) -> Vec<AccountId> {
        self.draft_operators_whitelist.to_vec()
    }

    fn hash_schedule(&self, schedule: Schedule) -> Base58CryptoHash {
        schedule.hash().into()
    }

    fn validate_schedule(
        &self,
        schedule: Schedule,
        total_balance: WrappedBalance,
        termination_schedule: Option<Schedule>,
    ) {
        schedule.assert_valid(total_balance.0);
        if let Some(termination_schedule) = termination_schedule {
            termination_schedule.assert_valid(total_balance.0);
            schedule.assert_valid_termination_schedule(&termination_schedule);
        }
    }

    fn get_next_draft_group_id(&self) -> DraftGroupIndex {
        self.next_draft_group_id
    }

    fn get_next_draft_id(&self) -> DraftGroupIndex {
        self.next_draft_id
    }

    fn get_num_draft_groups(&self) -> u32 {
        self.draft_groups.len().try_into().unwrap()
    }

    fn get_draft_group(&self, index: DraftGroupIndex) -> Option<DraftGroupView> {
        self.draft_groups.get(&index as _).map(Into::into)
    }

    fn get_draft_groups_paged(
        &self,
        // not the draft_id, but internal index used inside the LookupMap struct
        from_index: Option<DraftGroupIndex>,
        to_index: Option<DraftGroupIndex>,
    ) -> Vec<(DraftGroupIndex, DraftGroupView)> {
        let from_index = from_index.unwrap_or(0);
        let to_index = to_index.unwrap_or(self.draft_groups.len().try_into().unwrap());
        let keys = self.draft_groups.keys_as_vector();
        let values = self.draft_groups.values_as_vector();
        (from_index..std::cmp::min(self.next_draft_group_id as _, to_index))
            .map(|index| {
                (
                    keys.get(u64::from(index)).unwrap(),
                    values.get(u64::from(index)).unwrap().into(),
                )
            })
            .collect()
    }

    fn get_draft(&self, index: DraftIndex) -> Option<DraftView> {
        self.drafts.get(&index as _).map(Into::into)
    }

    fn get_drafts(&self, indices: Vec<DraftIndex>) -> Vec<(DraftIndex, DraftView)> {
        indices
            .into_iter()
            .filter_map(|index| self.get_draft(index).map(|draft| (index, draft)))
            .collect()
    }

    fn get_version(&self) -> String {
        VERSION.into()
    }
}
