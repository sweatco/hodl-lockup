use std::collections::HashMap;

use near_sdk::{near, AccountId, PromiseOrValue};

use crate::{
    lockup::{LockupClaim, LockupIndex},
    Balance,
};

pub trait OrderApi {
    fn reset_execution_status(&mut self);
    fn get_orders(&self, account_id: AccountId) -> Vec<LockupClaim>;
    /// `percentage` is given with 2 decimal places (e.g., 1550 for 15.50%)
    fn authorize(
        &mut self,
        account_ids: Vec<AccountId>,
        percentage: Option<u32>,
    ) -> PromiseOrValue<OrdersExecutionResult>;
    /// `percentage` is given with 2 decimal places (e.g., 1550 for 15.50%)
    fn buy(&mut self, account_ids: Vec<AccountId>, percentage: Option<u32>) -> Vec<OrderExecution>;
    fn revoke(&mut self, index: LockupIndex);
}

#[near(serializers=[json])]
#[derive(Clone)]
pub struct OrderExecution {
    pub account_id: AccountId,
    pub total_approved: Balance,
    pub details: HashMap<LockupIndex, (Balance, Balance)>, // (Balance approved, Balance refund)
}

impl OrderExecution {
    pub fn new(account_id: AccountId) -> Self {
        Self {
            account_id,
            total_approved: 0,
            details: HashMap::new(),
        }
    }

    pub fn add(&mut self, index: LockupIndex, approved: Balance, refund: Balance) {
        self.details.insert(index, (approved, refund));
        self.total_approved += approved;
    }
}

#[near(serializers=[json])]
#[derive(Default)]
pub struct OrdersExecutionResult {
    pub approved: HashMap<AccountId, Balance>,
    pub rejected: Vec<AccountId>,
}
