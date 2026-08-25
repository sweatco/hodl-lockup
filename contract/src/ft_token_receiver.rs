use hodl_model::lockup::LockupCreate;
use near_plugins::AccessControllable;
use near_sdk::require;

use crate::{
    emit, env, log, near_bindgen, serde_json, AccountId, Contract, ContractExt, EventKind, FtLockupCreateLockup,
    FungibleTokenReceiver, PromiseOrValue, Roles, U128,
};

#[near_bindgen]
impl FungibleTokenReceiver for Contract {
    fn ft_on_transfer(&mut self, sender_id: AccountId, amount: U128, msg: String) -> PromiseOrValue<U128> {
        assert_eq!(env::predecessor_account_id(), self.token_account_id, "Invalid token ID");
        let amount = amount.into();
        // `sender_id` is the FT sender, not `predecessor_account_id` (that's
        // the token contract), so `#[access_control_any]` can't gate this
        // directly.
        require!(
            self.acl_has_role(Roles::DepositManager.into(), sender_id.clone()),
            "Not in deposit whitelist"
        );

        let lockup_create: LockupCreate = serde_json::from_str(&msg).unwrap();
        let lockup = lockup_create.into_lockup(&sender_id);
        lockup.assert_new_valid(amount);
        let index = self.internal_add_lockup(&lockup);
        log!("Created new lockup for {} with index {}", lockup.account_id, index);
        let event: FtLockupCreateLockup = (index, lockup).into();
        emit(EventKind::FtLockupCreateLockup(vec![event]));

        PromiseOrValue::Value(0.into())
    }
}
