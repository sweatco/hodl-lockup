use integration_trait::make_integration_version;
use near_sdk::{AccountId, PromiseOrValue};

use crate::lockup::LockupIndex;

#[make_integration_version]
pub trait AdjustApi {
    fn adjust(&mut self, beneficiary_id: AccountId, lockup_index: LockupIndex) -> PromiseOrValue<()>;

    fn revoke(&mut self, beneficiary_id: AccountId, lockup_indices: Vec<LockupIndex>) -> PromiseOrValue<()>;
}
