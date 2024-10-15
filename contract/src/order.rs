use hodl_model::{
    lockup::{LockupClaim, LockupIndex},
    order::{OrderApi, OrderExecution, OrdersExecutionResult},
    Balance,
};
use near_sdk::{env, ext_contract, near, require, AccountId, Gas, Promise, PromiseOrValue, PromiseResult};

use crate::{internal::assert_enough_gas, Contract, ContractExt, FtTransferPromise, GAS_FOR_FT_TRANSFER};

const GAS_FOR_AFTER_FT_TRANSFER: Gas = Gas::from_tgas(50);

#[near]
impl OrderApi for Contract {
    fn reset_execution_status(&mut self) {
        self.assert_deposit_whitelist(&env::predecessor_account_id());
        self.is_executing = false;
    }

    fn get_orders(&self, account_id: AccountId) -> Vec<LockupClaim> {
        self.orders.get(&account_id).unwrap_or_default()
    }

    fn authorize(
        &mut self,
        account_ids: Vec<AccountId>,
        percentage: Option<u32>,
    ) -> PromiseOrValue<OrdersExecutionResult> {
        self.assert_deposit_whitelist(&env::predecessor_account_id());

        let percentage = unwrap_percentage(percentage);

        let mut transfer_promise: Option<Promise> = None;
        let mut orders = Vec::<OrderExecution>::new();

        for account_id in account_ids {
            let order_execution = self.execute_order(account_id.clone(), percentage);
            let total_approved = order_execution.total_approved;

            orders.push(order_execution);
            transfer_promise = match transfer_promise {
                None => Some(self.do_transfer(account_id.clone(), total_approved)),
                Some(promise) => Some(promise.and(self.do_transfer(account_id.clone(), total_approved))),
            };

            self.orders.remove(&account_id);
        }

        if let Some(promise) = transfer_promise {
            assert_enough_gas(
                GAS_FOR_FT_TRANSFER
                    .checked_mul(orders.len() as _)
                    .unwrap()
                    .checked_add(GAS_FOR_AFTER_FT_TRANSFER)
                    .unwrap(),
            );

            self.is_executing = true;

            promise
                .then(
                    oder_callback::ext(env::current_account_id())
                        .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
                        .on_orders_executed(orders),
                )
                .into()
        } else {
            PromiseOrValue::Value(OrdersExecutionResult::default())
        }
    }

    fn buy(&mut self, account_ids: Vec<AccountId>, percentage: Option<u32>) -> Vec<OrderExecution> {
        self.assert_deposit_whitelist(&env::predecessor_account_id());

        let percentage = unwrap_percentage(percentage);

        let mut result = Vec::<OrderExecution>::new();

        for account_id in account_ids {
            let order_execution = self.execute_order(account_id.clone(), percentage);

            result.push(order_execution);
            self.orders.remove(&account_id).expect("Couldn't delete orders");
        }

        result
    }

    fn revoke(&mut self, index: LockupIndex) {
        require!(
            !self.is_executing,
            "Cannot revoke an order while other orders are being executed"
        );

        let account_id = env::predecessor_account_id();
        let mut orders = self.orders.get(&account_id).expect("Account orders not found");

        let order_index = orders
            .iter()
            .position(|order| order.index == index)
            .expect("No order for this lockup");
        let order = orders.remove(order_index);

        self.orders.insert(&account_id, &orders);

        let mut lockup = self.lockups.get(u64::from(index)).expect("Lockup not found");
        lockup.claimed_balance -= order.claim_amount.0;
        self.lockups.replace(u64::from(index), &lockup);
    }
}

fn unwrap_percentage(percentage: Option<u32>) -> u32 {
    let percentage = percentage.unwrap_or(10000);
    require!(
        (0..=10000).contains(&percentage),
        "Percentage is out of range [0 .. 10000]"
    );
    percentage
}

fn calculate_percentage(value: u128, percentage: u32) -> u128 {
    value
        .checked_mul(percentage as u128)
        .expect("Failed to multiply")
        .checked_div(10000)
        .expect("Failed to divide")
}

impl Contract {
    fn execute_order(&mut self, account_id: AccountId, percentage: u32) -> OrderExecution {
        let mut order_execution = OrderExecution::new(account_id.clone());

        let account_orders = self.orders.get(&account_id).expect("Account not found");
        for order in account_orders {
            let requested_amount = order.claim_amount.0;
            let approved_amount = calculate_percentage(requested_amount, percentage);
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
            Some(format!("Authorize claimed {amount} balance from {receiver_id}")),
        )
    }
}

#[ext_contract(oder_callback)]
#[allow(dead_code)] // false positive
trait OrderCallback {
    fn on_orders_executed(&mut self, orders: Vec<OrderExecution>) -> PromiseOrValue<OrdersExecutionResult>;
}

#[near]
impl OrderCallback for Contract {
    #[private]
    fn on_orders_executed(&mut self, orders: Vec<OrderExecution>) -> PromiseOrValue<OrdersExecutionResult> {
        self.is_executing = false;

        let mut execution_result = OrdersExecutionResult::default();
        for (index, order) in orders.iter().enumerate() {
            let tx_result = env::promise_result(index as _);

            if tx_result == PromiseResult::Failed {
                self.refund_order(order.clone());
                execution_result.rejected.push(order.account_id.clone());
            } else {
                execution_result
                    .approved
                    .insert(order.account_id.clone(), order.total_approved);
            }

            // region cleanup
            let mut account_lockup_indices = self
                .account_lockups
                .get(&order.account_id)
                .expect("Cannot find lockups for account");

            for index in order.details.keys() {
                let lockup = self.lockups.get(u64::from(*index)).expect("Cannot find lockup");
                if lockup.claimed_balance == lockup.schedule.total_balance() {
                    account_lockup_indices.remove(index);
                }
            }

            self.internal_save_account_lockups(&order.account_id, account_lockup_indices);
            // endregion
        }

        PromiseOrValue::Value(execution_result)
    }
}

impl Contract {
    fn refund_order(&mut self, order: OrderExecution) {
        for (index, (amount, _)) in order.details {
            self.refund(index, amount);
        }
    }

    fn refund(&mut self, index: LockupIndex, amount: Balance) {
        let mut lockup = self.lockups.get(u64::from(index)).expect("Lockup not found");
        lockup.claimed_balance -= amount;
        self.lockups.replace(u64::from(index), &lockup);
    }
}
