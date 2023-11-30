use model::update::UpdateApi;
use near_sdk::{env, near_bindgen, AccountId, Promise, PromiseOrValue};

use crate::{Contract, ContractExt};

#[near_bindgen]
impl UpdateApi for Contract {
    #[payable]
    fn update_contract(&mut self) -> PromiseOrValue<()> {
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
            .into()
    }

    #[private]
    fn set_multisig(&mut self, multisig: AccountId) {
        self.miltisig = multisig.into();
    }
}
