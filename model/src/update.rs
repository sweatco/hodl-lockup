use integration_trait::make_integration_version;
use near_sdk::{AccountId, PromiseOrValue};

#[make_integration_version]
pub trait UpdateApi {
    #[update]
    fn update_contract(&mut self) -> PromiseOrValue<()>;
    fn set_multisig(&mut self, multisig: AccountId);
}
