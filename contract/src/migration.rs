use model::migration::ContractDeprecated;

use crate::*;

#[near_bindgen]
impl Contract {
    #[private]
    #[init(ignore_state)]
    pub fn migrate(adjustable_account_ids: HashSet<AccountId>) -> Self {
        let old_state: ContractDeprecated = env::state_read().expect("Failed to read old state");
        let mut lockups: Vector<Lockup> = Vector::new(StorageKey::Lockups);

        for lockup in old_state.lockups.iter() {
            let modified_lockup = Lockup {
                account_id: lockup.account_id.clone(),
                schedule: lockup.schedule.clone(),
                claimed_balance: lockup.claimed_balance,
                termination_config: lockup.termination_config.clone(),
                is_adjustable: adjustable_account_ids.contains(&lockup.account_id),
            };
            lockups.push(&modified_lockup);
        }

        Contract {
            token_account_id: old_state.token_account_id,
            lockups,
            account_lockups: old_state.account_lockups,
            deposit_whitelist: old_state.deposit_whitelist,
            draft_operators_whitelist: old_state.draft_operators_whitelist,
            next_draft_id: old_state.next_draft_id,
            drafts: old_state.drafts,
            next_draft_group_id: old_state.next_draft_group_id,
            draft_groups: old_state.draft_groups,
            multisig: old_state.multisig,
        }
    }
}
