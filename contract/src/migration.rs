use std::collections::HashMap;

use hodl_model::{lockup::Lockup, ONE_DAY_SEC, ONE_YEAR_SEC};
use near_sdk::{
    env::{self, panic_str},
    ext_contract, is_promise_success,
    json_types::U128,
    near, require,
    serde_json::{self, json},
    AccountId, Gas, NearToken, Promise, PromiseOrValue,
};

use crate::{
    event::{emit, EventKind, FtLockupUpdateContract},
    Contract, ContractExt, GAS_FOR_FT_TRANSFER,
};

#[near]
impl Contract {
    #[private]
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        emit(EventKind::FtLockupUpdateContract(FtLockupUpdateContract {}));

        let old_state: Contract = env::state_read().expect("Failed to read old state");

        old_state
    }

    pub fn transfer_accounts(&mut self, contract_id: AccountId, account_ids: Vec<AccountId>) -> PromiseOrValue<()> {
        self.assert_deposit_whitelist(&env::predecessor_account_id());

        require!(
            !self.is_executing,
            "Cannot transfer accounts while orders are being executed"
        );
        self.is_executing = true;

        let mut total_amount = 0;
        let mut transfer_args = vec![];

        for account_id in account_ids.iter() {
            let lockup_indices = self
                .account_lockups
                .get(&account_id)
                .unwrap_or_else(|| panic_str(format!("Account {account_id} is not found!").as_str()));
            let orders: HashMap<u32, u128> = self
                .orders
                .remove(&account_id)
                .unwrap_or_default()
                .iter()
                .map(|order| (order.index, order.claim_amount.0))
                .collect();

            for index in lockup_indices {
                let lockup = self
                    .lockups
                    .get(index.into())
                    .unwrap_or_else(|| panic_str(format!("No lockup at index {index}!").as_str()));

                let issued_at = lockup
                    .schedule
                    .0
                    .first()
                    .unwrap_or_else(|| panic_str("Empty schedule"))
                    .timestamp
                    - ONE_YEAR_SEC;
                let total = lockup.schedule.total_balance();
                let claimed = lockup.claimed_balance.0 - orders.get(&index).map_or(0, |order_amount| *order_amount);

                transfer_args.push((account_id.clone(), issued_at, U128(total), U128(claimed)));
                total_amount += total - claimed;
            }
        }

        let msg = json!({
            "type": "migrate",
            "data": transfer_args,
        })
        .to_string();
        let args = json!({
            "receiver_id": contract_id,
            "amount": U128(total_amount),
            "msg": msg,
        });

        Promise::new(self.token_account_id.clone())
            .function_call(
                "ft_transfer_call".to_string(),
                serde_json::to_vec(&args).unwrap_or_else(|_| panic_str("Failed to serialize args")),
                NearToken::from_yoctonear(1),
                Gas::from_tgas(200),
            )
            .then(ext_self::ext(env::current_account_id()).on_migrated(account_ids))
            .into()
    }
}

#[ext_contract(ext_self)]
pub trait MigrationCallback {
    fn on_migrated(&mut self, account_ids: Vec<AccountId>);
}

#[near]
impl MigrationCallback for Contract {
    fn on_migrated(&mut self, account_ids: Vec<AccountId>) {
        self.is_executing = false;

        if !is_promise_success() {
            return;
        }

        for account_id in account_ids.iter() {
            let indices = self
                .account_lockups
                .remove(account_id)
                .unwrap_or_else(|| panic_str(format!("Account {account_id} is not found!").as_str()));

            for index in indices {
                self.lockups
                    .replace(index as u64, &Lockup::new_unlocked(account_id.clone(), 0));
            }
        }
    }
}
