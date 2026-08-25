use near_sdk::{json_types::U128, AccountId};

pub mod api;
pub mod lockup;
pub mod order;
pub mod schedule;
pub mod termination;
pub mod util;

pub const ONE_DAY_SEC: TimestampSec = 24 * 60 * 60;
pub const ONE_YEAR_SEC: TimestampSec = 365 * ONE_DAY_SEC;

pub type WrappedBalance = U128;
pub type TimestampSec = u32;
pub type TokenAccountId = AccountId;
pub type Balance = u128;

pub mod u256 {
    #![allow(clippy::doc_markdown)]
    #![allow(clippy::assign_op_pattern)]

    uint::construct_uint! {
        pub struct U256(4);
    }
}
