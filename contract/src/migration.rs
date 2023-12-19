use near_sdk::{AccountId, collections::UnorderedSet, env, near_bindgen};

use model::migration::OldState;

use crate::{Contract, ContractExt, StorageKey};
use crate::event::{emit, EventKind, FtLockupUpdateContract};

#[near_bindgen]
impl Contract {
    #[private]
    #[init(ignore_state)]
    pub fn migrate(manager: AccountId) -> Self {
        emit(EventKind::FtLockupUpdateContract(FtLockupUpdateContract {}));

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
