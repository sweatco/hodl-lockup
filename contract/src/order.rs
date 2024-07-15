use std::collections::HashMap;

use hodl_model::{
    lockup::LockupIndex,
    order::{OrderApi, OrderExecution},
    Balance,
};
use near_sdk::{env, ext_contract, is_promise_success, near, AccountId, Gas, Promise, PromiseOrValue};

use crate::{Contract, ContractExt, FtTransferPromise};

const GAS_FOR_AFTER_FT_TRANSFER: Gas = Gas::from_gas(20_000_000_000_000);

#[near]
impl OrderApi for Contract {
    fn authorize(
        &mut self,
        account_ids: Vec<AccountId>,
        percentage: Option<f32>,
    ) -> PromiseOrValue<Vec<OrderExecution>> {
        // TODO: add assert for caller id

        let percentage = percentage.unwrap_or(1.0);
        assert!(
            0.0 <= percentage && percentage <= 1.0,
            "Percentage is out of range [0.0 .. 1.0]"
        );

        if let Some((head, tail)) = account_ids.split_first() {
            self.authorize_order(head.clone(), percentage, tail.to_vec(), vec![])
        } else {
            PromiseOrValue::Value(vec![])
        }
    }

    fn buy(&mut self, account_ids: Vec<AccountId>, percentage: Option<f32>) -> Vec<OrderExecution> {
        // TODO: add assert for caller id

        let percentage = percentage.unwrap_or(1.0);
        assert!(
            0.0 <= percentage && percentage <= 1.0,
            "Percentage is out of range [0.0 .. 1.0]"
        );

        let mut result = Vec::<OrderExecution>::new();

        for account_id in account_ids {
            let mut results = HashMap::<LockupIndex, (Balance, Balance)>::new();

            let account_orders = self.orders.get(&account_id).expect("Account not found");
            for order in account_orders {
                let requested_amount = order.claim_amount.0;
                let approved_amount = (requested_amount as f32 * percentage) as u128;
                let refund_amount = requested_amount - approved_amount;

                if approved_amount > requested_amount {
                    let mut lockup = self.lockups.get(order.index as _).expect("Lockup not found");
                    lockup.claimed_balance -= refund_amount;
                    self.lockups.replace(order.index as _, &lockup);
                }

                results.insert(order.index as _, (approved_amount, refund_amount));
            }

            result.push(OrderExecution {
                account_id: account_id.clone(),
                results,
            });
            self.orders.remove(&account_id).expect("Couldn't delete orders");
        }

        result
    }
}

impl Contract {
    fn authorize_order(
        &mut self,
        account_id: AccountId,
        percentage: f32,
        tail: Vec<AccountId>,
        result_accumulator: Vec<OrderExecution>,
    ) -> PromiseOrValue<Vec<OrderExecution>> {
        let mut order_execution = OrderExecution::empty(account_id.clone());
        let mut amount_to_transfer = 0;

        let account_orders = self.orders.get(&account_id).expect("Account not found");
        for order in account_orders {
            let requested_amount = order.claim_amount.0;
            let approved_amount = (requested_amount as f32 * percentage) as u128;
            let refund_amount = requested_amount - approved_amount;

            if approved_amount > requested_amount {
                let mut lockup = self.lockups.get(order.index as _).expect("Lockup not found");
                lockup.claimed_balance -= refund_amount;
                self.lockups.replace(order.index as _, &lockup);
            }

            order_execution
                .results
                .insert(order.index, (approved_amount, refund_amount));
            amount_to_transfer += approved_amount;
        }

        if amount_to_transfer > 0 {
            Promise::new(self.token_account_id.clone())
                .ft_transfer(
                    &account_id,
                    amount_to_transfer,
                    Some(format!(
                        "Claiming unlocked {} balance from {}",
                        amount_to_transfer,
                        env::current_account_id()
                    )),
                )
                .then(
                    oder_callback::ext(env::current_account_id())
                        .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
                        .on_order_authorized(order_execution, percentage, tail, result_accumulator),
                )
                .into()
        } else if let Some((head, tail)) = tail.split_first() {
            self.authorize_order(head.clone(), percentage, tail.to_vec(), result_accumulator)
        } else {
            PromiseOrValue::Value(result_accumulator)
        }
    }
}

#[ext_contract(oder_callback)]
trait OrderCallback {
    fn on_order_authorized(
        &mut self,
        order_execution: OrderExecution,
        percentage: f32,
        tail: Vec<AccountId>,
        result_accumulator: Vec<OrderExecution>,
    ) -> PromiseOrValue<Vec<OrderExecution>>;
}

#[near]
impl OrderCallback for Contract {
    #[private]
    fn on_order_authorized(
        &mut self,
        order_execution: OrderExecution,
        percentage: f32,
        tail: Vec<AccountId>,
        result_accumulator: Vec<OrderExecution>,
    ) -> PromiseOrValue<Vec<OrderExecution>> {
        let mut result = Vec::from(result_accumulator);

        if is_promise_success() {
            for (index, (balance, _)) in order_execution.results {
                if let Some(mut lockup) = self.lockups.get(index as _) {
                    lockup.claimed_balance -= balance;
                    self.lockups.replace(index as _, &lockup);
                }
            }

            result.push(OrderExecution::empty(order_execution.account_id.clone()));

            self.orders
                .remove(&order_execution.account_id)
                .expect("Cannot remove order");
        } else {
            result.push(order_execution)
        }

        if let Some((head, tail)) = tail.split_first() {
            self.authorize_order(head.clone(), percentage, tail.to_vec(), result)
        } else {
            PromiseOrValue::Value(result)
        }
    }
}
