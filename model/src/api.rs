#![allow(clippy::new_ret_no_self)]
#![allow(clippy::wrong_self_convention)]

use near_sdk::{json_types::U128, AccountId};

use crate::{
    lockup::{LockupClaim, LockupIndex, LockupView},
    schedule::Schedule,
    TimestampSec, WrappedBalance,
};

pub trait IssueApi {
    fn issue(&mut self, issue_date: TimestampSec, amounts: Vec<(AccountId, U128)>);
}

/// Role assignments for ACL bootstrap: pairs of role name (matching a
/// `Roles` variant in the `hodl-lockup` contract crate, e.g.
/// `"DepositManager"`) to the accounts that should hold it. Kept
/// string-keyed here (rather than an enum) because the `Roles` type must
/// stay local to the contract crate — the `near_plugins::AccessControlRole`
/// derive it uses generates a private companion type that the
/// `#[access_control]` macro on `Contract` resolves by bare name, which only
/// works if both live in the same module.
pub type RoleAssignments = Vec<(String, Vec<AccountId>)>;

pub trait LockupApi {
    fn new(token_account_id: AccountId, super_admin_account_id: AccountId, roles: RoleAssignments) -> Self;

    fn claim(&mut self, amounts: Option<Vec<(LockupIndex, Option<WrappedBalance>)>>) -> Vec<LockupClaim>;

    fn terminate(
        &mut self,
        lockup_index: LockupIndex,
        hashed_schedule: Option<Schedule>,
        termination_timestamp: Option<TimestampSec>,
    ) -> WrappedBalance;
}

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

    fn get_version(&self) -> String;

    fn get_orders(&self) -> Vec<(AccountId, Vec<LockupClaim>)>;

    fn get_total_orders_amount(&self) -> U128;

    fn get_total_unclaimed_amount(&self) -> U128;
}
