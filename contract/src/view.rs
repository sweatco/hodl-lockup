use hodl_model::{
    api::LockupViewApi,
    lockup::{LockupIndex, LockupView},
};
use near_sdk::near;

use crate::{AccountId, Contract, ContractExt, Into, VERSION};

#[near]
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

    fn get_version(&self) -> String {
        VERSION.into()
    }
}
