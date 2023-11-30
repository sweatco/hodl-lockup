use integration_trait::make_integration_version;
use near_sdk::{AccountId, Promise};

#[make_integration_version]
pub trait UpdateApi {
    #[update]
    fn update_contract(&mut self) -> Promise;
    fn set_multisig(&mut self, multisig: AccountId);
}
