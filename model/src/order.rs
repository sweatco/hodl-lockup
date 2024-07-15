use std::collections::HashMap;

use near_sdk::{near, AccountId, PromiseOrValue};

use crate::{lockup::LockupIndex, Balance};

pub trait OrderApi {
    fn authorize(
        &mut self,
        account_ids: Vec<AccountId>,
        percentage: Option<f32>,
    ) -> PromiseOrValue<Vec<OrderExecution>>;
    fn buy(&mut self, account_ids: Vec<AccountId>, percentage: Option<f32>) -> Vec<OrderExecution>;
}

#[near(serializers=[json])]
pub struct OrderExecution {
    pub account_id: AccountId,
    pub results: HashMap<LockupIndex, (Balance, Balance)>, // (Balance approved, Balance refund)
}

impl OrderExecution {
    pub fn empty(account_id: AccountId) -> Self {
        Self {
            account_id,
            results: HashMap::new(),
        }
    }
}
