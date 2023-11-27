use near_sdk::{json_types::U128, AccountId};

pub mod draft;
pub mod lockup;
pub mod lockup_api;
pub mod schedule;
pub mod termination;
pub mod util;
pub mod view_api;

pub type WrappedBalance = U128;
pub type TimestampSec = u32;
pub type TokenAccountId = AccountId;

pub mod u256 {
    #![allow(clippy::doc_markdown)]
    #![allow(clippy::assign_op_pattern)]

    uint::construct_uint! {
        pub struct U256(4);
    }
}
