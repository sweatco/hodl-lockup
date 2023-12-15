use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    json_types::Base58CryptoHash,
    serde::{Deserialize, Serialize},
    AccountId, Balance, CryptoHash,
};

use crate::{lockup::Lockup, schedule::Schedule, TimestampSec};

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub enum VestingConditions {
    SameAsLockupSchedule,
    Hash(Base58CryptoHash),
    Schedule(Schedule),
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct TerminationConfig {
    /// The account ID who paid for the lockup creation
    /// and will receive unvested balance upon termination
    pub beneficiary_id: AccountId,
    /// An optional vesting schedule
    pub vesting_schedule: VestingConditions,
}
