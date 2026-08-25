use near_sdk::{json_types::Base58CryptoHash, near, AccountId};

use crate::{lockup::Lockup, schedule::Schedule, Balance, TimestampSec};

#[near(serializers=[borsh, json])]
#[derive(Clone, Debug, PartialEq)]
pub enum VestingConditions {
    SameAsLockupSchedule,
    Hash(Base58CryptoHash),
    Schedule(Schedule),
}

#[near(serializers=[borsh, json])]
#[derive(Debug, PartialEq, Clone)]
pub struct TerminationConfig {
    /// The account ID who paid for the lockup creation
    /// and will receive unvested balance upon termination
    pub beneficiary_id: AccountId,
    /// An optional vesting schedule
    pub vesting_schedule: VestingConditions,
}

impl Lockup {
    pub fn terminate(&mut self, termination_timestamp: TimestampSec) -> Balance {
        let total_balance = self.schedule.total_balance();
        let vested_balance = self.schedule.unlocked_balance(termination_timestamp);

        let (vested_balance, termination_timestamp) = if vested_balance >= self.claimed_balance.0 {
            (vested_balance, termination_timestamp)
        } else {
            let last_claim_timestamp = self.schedule.get_vesting_timestamp_for_amount(vested_balance);
            (self.claimed_balance.0, last_claim_timestamp)
        };

        let unvested_balance = total_balance - vested_balance;
        if unvested_balance > 0 {
            self.schedule.terminate(vested_balance, termination_timestamp);
        }

        unvested_balance
    }
}
