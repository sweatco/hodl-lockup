use model::migration::OldState;
use near_sdk::{collections::UnorderedSet, env, env::log_str, near_bindgen, AccountId};

use crate::{Contract, ContractExt, StorageKey};

#[near_bindgen]
impl Contract {
    #[private]
    #[init(ignore_state)]
    pub fn migrate(manager: AccountId) -> Self {
        log_str("Migrate");

        let old_state: OldState = env::state_read().expect("Failed to read old state");

        Contract {
            token_account_id: old_state.token_account_id,
            lockups: old_state.lockups,
            account_lockups: old_state.account_lockups,
            deposit_whitelist: old_state.deposit_whitelist,
            draft_operators_whitelist: UnorderedSet::new(StorageKey::DraftOperatorsWhitelist),
            next_draft_id: old_state.next_draft_id,
            drafts: old_state.drafts,
            next_draft_group_id: old_state.next_draft_group_id,
            draft_groups: old_state.draft_groups,
            manager,
        }
    }
}
