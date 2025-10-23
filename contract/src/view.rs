use std::collections::HashSet;

use hodl_model::{
    api::LockupViewApi,
    lockup::{LockupClaim, LockupIndex, LockupView},
};
use near_sdk::{json_types::U128, near};

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

    fn get_orders(&self) -> Vec<(AccountId, Vec<LockupClaim>)> {
        let account_ids: HashSet<AccountId> = self.lockups.iter().map(|l| l.account_id).collect();

        account_ids
            .iter()
            .filter_map(|account_id| {
                if let Some(order) = self.orders.get(account_id) {
                    Some((account_id.clone(), order))
                } else {
                    None
                }
            })
            .collect()
    }

    fn get_total_orders_amount(&self) -> U128 {
        self.get_orders()
            .iter()
            .flat_map(|(_, orders)| orders)
            .map(|o| o.claim_amount.0)
            .sum::<u128>()
            .into()
    }

    fn get_total_unclaimed_amount(&self) -> U128 {
        self.lockups
            .iter()
            .map(|l| l.schedule.total_balance() - l.claimed_balance.0)
            .sum::<u128>()
            .into()
    }
}
