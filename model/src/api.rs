#![allow(clippy::new_ret_no_self)]
#![allow(clippy::wrong_self_convention)]

use near_sdk::PromiseOrValue;
#[cfg(not(feature = "integration-test"))]
use near_sdk::{json_types::U128, AccountId};
#[cfg(feature = "integration-test")]
use nitka::near_sdk::{json_types::U128, AccountId};
use nitka_proc::make_integration_version;

use crate::{
    lockup::{LockupClaim, LockupIndex, LockupView},
    schedule::Schedule,
    TimestampSec, WrappedBalance,
};

#[cfg(feature = "integration-test")]
pub struct HodlContract<'a> {
    pub contract: &'a near_workspaces::Contract,
}

#[make_integration_version]
pub trait IssueApi {
    fn issue(&mut self, issue_date: TimestampSec, amounts: Vec<(AccountId, U128)>);
}

#[make_integration_version]
pub trait LockupApi {
    fn new(
        token_account_id: AccountId,
        deposit_whitelist: Vec<AccountId>,
        draft_operators_whitelist: Option<Vec<AccountId>>,
        manager: AccountId,
    ) -> Self;

    fn claim(&mut self, amounts: Option<Vec<(LockupIndex, Option<WrappedBalance>)>>) -> Vec<LockupClaim>;

    fn terminate(
        &mut self,
        lockup_index: LockupIndex,
        hashed_schedule: Option<Schedule>,
        termination_timestamp: Option<TimestampSec>,
    ) -> WrappedBalance;

    fn set_accounts_total_balances(&mut self, accounts_and_balances: Vec<(AccountId, U128)>);

    // preserving both options for API compatibility
    fn add_to_deposit_whitelist(&mut self, account_id: Option<AccountId>, account_ids: Option<Vec<AccountId>>);

    // preserving both options for API compatibility
    fn remove_from_deposit_whitelist(&mut self, account_id: Option<AccountId>, account_ids: Option<Vec<AccountId>>);
}

#[make_integration_version]
pub trait LockupViewApi {
    fn get_token_account_id(&self) -> AccountId;

    fn get_account_lockups(&self, account_id: AccountId) -> Vec<(LockupIndex, LockupView)>;

    fn get_lockup(&self, index: LockupIndex) -> Option<LockupView>;
    fn get_lockups(&self, indices: Vec<LockupIndex>) -> Vec<(LockupIndex, LockupView)>;

    fn get_num_lockups(&self) -> u32;

    fn get_lockups_paged(
        &self,
        from_index: Option<LockupIndex>,
        limit: Option<LockupIndex>,
    ) -> Vec<(LockupIndex, LockupView)>;

    fn get_deposit_whitelist(&self) -> Vec<AccountId>;

    fn get_version(&self) -> String;

    fn get_orders(&self) -> Vec<(AccountId, Vec<LockupClaim>)>;

    fn get_total_orders_amount(&self) -> U128;

    fn get_total_unclaimed_amount(&self) -> U128;
}
