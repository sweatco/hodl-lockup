use near_sdk::{env, near_bindgen, AccountId, Promise};

use crate::{Contract, ContractExt};

#[near_bindgen]
impl Contract {
    #[payable]
    pub fn update_contract(&mut self) -> Promise {
        let Some(ref multisig) = self.miltisig else {
            env::panic_str("Multisig account is not set. Operation is impossible");
        };

        assert_eq!(
            env::predecessor_account_id(),
            *multisig,
            "Only multisig account can update the contract"
        );

        env::log_str("Skogo4");

        let code = env::input().expect("Error: No input");

        Promise::new(env::current_account_id())
            .deploy_contract(code)
            .as_return()
    }

    #[private]
    pub fn set_multisig(&mut self, multisig: AccountId) {
        self.miltisig = multisig.into();
    }
}
