use hodl_model::{
    lockup::{LockupClaim, LockupIndex},
    order::{OrderApi, OrderExecution, OrdersExecution},
    Balance,
};
use near_sdk::{env, ext_contract, near, AccountId, Gas, Promise, PromiseOrValue, PromiseResult};

use crate::{Contract, ContractExt, FtTransferPromise};

const GAS_FOR_AFTER_FT_TRANSFER: Gas = Gas::from_gas(20_000_000_000_000);

#[near]
impl OrderApi for Contract {
    fn get_orders(&self, account_id: AccountId) -> Vec<LockupClaim> {
        self.orders.get(&account_id).unwrap_or_default()
    }

    fn authorize(&mut self, account_ids: Vec<AccountId>, percentage: Option<f32>) -> PromiseOrValue<OrdersExecution> {
        self.assert_deposit_whitelist(&env::predecessor_account_id());

        let percentage = percentage.unwrap_or(1.0);
        assert!(
            (0.0..=1.0).contains(&percentage),
            "Percentage is out of range [0.0 .. 1.0]"
        );

        let mut transfer_promise: Option<Promise> = Option::None;
        let mut result = OrdersExecution::default();

        for account_id in account_ids {
            let order = self.authorize_order(account_id.clone(), percentage);
            let total_approved = order.total_approved;

            if total_approved == 0 {
                result.declined.push(order);
                continue;
            }

            result.authorized.push(order);
            transfer_promise = match transfer_promise {
                None => Some(self.do_transfer(account_id, total_approved)),
                Some(promise) => Some(promise.and(self.do_transfer(account_id, total_approved))),
            };
        }

        if let Some(promise) = transfer_promise {
            promise
                .then(
                    oder_callback::ext(env::current_account_id())
                        .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
                        .on_orders_executed(result),
                )
                .into()
        } else {
            PromiseOrValue::Value(result)
        }
    }

    fn buy(&mut self, account_ids: Vec<AccountId>, percentage: Option<f32>) -> Vec<OrderExecution> {
        self.assert_deposit_whitelist(&env::predecessor_account_id());

        let percentage = percentage.unwrap_or(1.0);
        assert!(
            (0.0..=1.0).contains(&percentage),
            "Percentage is out of range [0.0 .. 1.0]"
        );

        let mut result = Vec::<OrderExecution>::new();

        for account_id in account_ids {
            let mut order_execution = OrderExecution::empty(account_id.clone());

            let account_orders = self.orders.get(&account_id).expect("Account not found");
            for order in account_orders {
                let requested_amount = order.claim_amount.0;
                let approved_amount = (requested_amount as f32 * percentage) as u128;
                let refund_amount = requested_amount - approved_amount;

                if approved_amount > requested_amount {
                    self.refund(order.index, refund_amount);
                }

                order_execution.add(order.index, approved_amount, refund_amount);
            }

            result.push(order_execution);
            self.orders.remove(&account_id).expect("Couldn't delete orders");
        }

        result
    }
}

impl Contract {
    fn authorize_order(&mut self, account_id: AccountId, percentage: f32) -> OrderExecution {
        let mut order_execution = OrderExecution::empty(account_id.clone());

        let account_orders = self.orders.get(&account_id).expect("Account not found");
        for order in account_orders {
            let requested_amount = order.claim_amount.0;
            let approved_amount = (requested_amount as f32 * percentage) as u128;
            let refund_amount = requested_amount - approved_amount;

            if approved_amount > requested_amount {
                self.refund(order.index, refund_amount);
            }

            order_execution.add(order.index, approved_amount, refund_amount);
        }

        order_execution
    }
}

impl Contract {
    fn do_transfer(&self, receiver_id: AccountId, amount: Balance) -> Promise {
        Promise::new(self.token_account_id.clone()).ft_transfer(
            &receiver_id,
            amount,
            Some(format!("Authorize claimed {} balance from {}", amount, receiver_id)),
        )
    }
}

#[ext_contract(oder_callback)]
trait OrderCallback {
    fn on_orders_executed(&mut self, orders: OrdersExecution) -> PromiseOrValue<OrdersExecution>;
}

#[near]
impl OrderCallback for Contract {
    #[private]
    fn on_orders_executed(&mut self, orders: OrdersExecution) -> PromiseOrValue<OrdersExecution> {
        for (index, order) in orders.authorized.iter().enumerate() {
            let result = env::promise_result(index as _);

            if result == PromiseResult::Failed {
                self.refund_order(order.clone());
            }
        }

        PromiseOrValue::Value(orders)
    }
}

impl Contract {
    fn refund_order(&mut self, order: OrderExecution) {
        for (index, (amount, _)) in order.results {
            self.refund(index, amount);
        }
    }

    fn refund(&mut self, index: LockupIndex, amount: Balance) {
        let mut lockup = self.lockups.get(index as _).expect("Lockup not found");
        lockup.claimed_balance -= amount;
        self.lockups.replace(index as _, &lockup);
    }
}
