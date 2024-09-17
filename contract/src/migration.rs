use near_sdk::{env, near_bindgen};

use crate::{
    event::{emit, EventKind, FtLockupUpdateContract},
    Contract, ContractExt,
};

#[near_bindgen]
impl Contract {
    #[private]
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        emit(EventKind::FtLockupUpdateContract(FtLockupUpdateContract {}));

        let old_state: Contract = env::state_read().expect("Failed to read old state");

        old_state
    }
}
