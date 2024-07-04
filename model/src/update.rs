use near_sdk::{AccountId, PromiseOrValue};
use nitka::make_integration_version;

#[make_integration_version]
pub trait UpdateApi {
    #[update]
    fn update_contract(&mut self) -> PromiseOrValue<()>;
    fn set_multisig(&mut self, multisig: AccountId);
}
