use std::collections::HashMap;

use hodl_model::{
    draft::{DraftGroup, DraftGroupIndex, DraftIndex},
    lockup::{Lockup, LockupClaim, LockupIndex},
    util::current_timestamp_sec,
    WrappedBalance,
};

use crate::{
    emit, ext_contract, is_promise_success, log, near_bindgen, AccountId, Contract, ContractExt, EventKind,
    FtLockupClaimLockup, FtLockupCreateLockup, Into,
};

#[ext_contract(ext_self)]
pub trait SelfCallbacks {
    fn after_ft_transfer(&mut self, account_id: AccountId, lockup_claims: Vec<LockupClaim>) -> WrappedBalance;

    fn after_lockup_termination(&mut self, account_id: AccountId, amount: WrappedBalance) -> WrappedBalance;

    fn convert_drafts(&mut self, draft_ids: Vec<DraftIndex>) -> Vec<LockupIndex>;
}

#[near_bindgen]
impl SelfCallbacks for Contract {
    #[private]
    fn after_ft_transfer(&mut self, account_id: AccountId, lockup_claims: Vec<LockupClaim>) -> WrappedBalance {
        let promise_success = is_promise_success();
        let mut total_balance = 0;
        if promise_success {
            let mut remove_indices = vec![];
            let mut events: Vec<FtLockupClaimLockup> = vec![];
            for LockupClaim {
                index,
                is_final,
                claim_amount,
            } in lockup_claims
            {
                if is_final {
                    remove_indices.push(index);
                }
                total_balance += claim_amount.0;
                let event = FtLockupClaimLockup {
                    id: index,
                    amount: claim_amount,
                };
                events.push(event);
            }
            if !remove_indices.is_empty() {
                let mut indices = self.account_lockups.get(&account_id).unwrap_or_default();
                for index in remove_indices {
                    indices.remove(&index);
                }
                self.internal_save_account_lockups(&account_id, indices);
            }
            emit(EventKind::FtLockupClaimLockup(events));
        } else {
            log!("Token transfer has failed. Refunding.");
            let mut modified = false;
            let mut indices = self.account_lockups.get(&account_id).unwrap_or_default();
            for LockupClaim {
                index, claim_amount, ..
            } in lockup_claims
            {
                if indices.insert(index) {
                    modified = true;
                }
                let mut lockup = self.lockups.get(u64::from(index)).unwrap();
                lockup.claimed_balance -= claim_amount.0;
                self.lockups.replace(u64::from(index), &lockup);
            }

            if modified {
                self.internal_save_account_lockups(&account_id, indices);
            }
        }
        total_balance.into()
    }

    #[private]
    fn after_lockup_termination(&mut self, account_id: AccountId, amount: WrappedBalance) -> WrappedBalance {
        if is_promise_success() {
            return amount;
        }

        log!("Lockup termination transfer has failed.");
        // There is no internal balance, so instead we create a new lockup.
        let lockup = Lockup::new_unlocked_since(account_id, amount.0, current_timestamp_sec());
        let lockup_index = self.internal_add_lockup(&lockup);
        let event: FtLockupCreateLockup = (lockup_index, lockup, None).into();
        emit(EventKind::FtLockupCreateLockup(vec![event]));
        0.into()
    }

    fn convert_drafts(&mut self, draft_ids: Vec<DraftIndex>) -> Vec<LockupIndex> {
        let mut draft_group_lookup: HashMap<DraftGroupIndex, DraftGroup> = HashMap::new();
        let mut events: Vec<FtLockupCreateLockup> = vec![];
        let lockup_ids: Vec<LockupIndex> = draft_ids
            .iter()
            .map(|draft_id| {
                let draft = self.drafts.remove(draft_id as _).expect("draft not found");
                let draft_group = draft_group_lookup.entry(draft.draft_group_id).or_insert_with(|| {
                    self.draft_groups
                        .get(&draft.draft_group_id as _)
                        .expect("draft group not found")
                });
                draft_group.assert_can_convert_draft();
                let payer_id = draft_group.payer_id.as_mut().expect("expected present payer_id");

                assert!(draft_group.draft_indices.remove(draft_id), "Invariant");
                let amount = draft.total_balance();
                assert!(draft_group.total_amount >= amount, "Invariant");
                draft_group.total_amount -= amount;

                let lockup = draft.lockup_create.into_lockup(payer_id);
                let index = self.internal_add_lockup(&lockup);

                let event: FtLockupCreateLockup = (index, lockup, Some(*draft_id)).into();
                events.push(event);

                index
            })
            .collect();

        emit(EventKind::FtLockupCreateLockup(events));

        for (draft_group_id, draft_group) in &draft_group_lookup {
            if draft_group.draft_indices.is_empty() {
                self.draft_groups.remove(draft_group_id as _);
            } else {
                self.draft_groups.insert(draft_group_id as _, draft_group);
            }
        }

        lockup_ids
    }
}
