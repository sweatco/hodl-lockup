use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    json_types::U128,
    near,
    serde::{Deserialize, Serialize},
    AccountId,
};

use crate::{
    schedule::Schedule,
    termination::{TerminationConfig, VestingConditions},
    util::current_timestamp_sec,
    Balance, TimestampSec, WrappedBalance,
};

pub type LockupIndex = u32;

#[near(serializers=[borsh, json])]
#[derive(Debug, PartialEq, Clone)]
pub struct LockupClaim {
    pub index: LockupIndex,
    pub claim_amount: WrappedBalance,
    pub is_final: bool,
}

#[near(serializers=[borsh, json])]
#[derive(Debug, PartialEq, Clone)]
pub struct Lockup {
    pub account_id: AccountId,
    pub schedule: Schedule,

    #[serde(default)]
    pub claimed_balance: WrappedBalance,
    /// An optional configuration that allows vesting/lockup termination.
    pub termination_config: Option<TerminationConfig>,
}

impl Lockup {
    pub fn new_unlocked_since(account_id: AccountId, total_balance: Balance, timestamp: TimestampSec) -> Self {
        Self {
            account_id,
            schedule: Schedule::new_unlocked_since(total_balance, timestamp),
            claimed_balance: U128(0),
            termination_config: None,
        }
    }

    pub fn new_unlocked(account_id: AccountId, total_balance: Balance) -> Self {
        Self::new_unlocked_since(account_id, total_balance, 1)
    }

    pub fn claim(&mut self, index: LockupIndex, claim_amount: Balance) -> LockupClaim {
        let balance_claimed_new = self
            .claimed_balance
            .0
            .checked_add(claim_amount)
            .expect("attempt to add with overflow");
        assert!(
            balance_claimed_new <= self.schedule.total_balance(),
            "Trying to claim more than the total balance"
        );
        self.claimed_balance = U128(balance_claimed_new);
        LockupClaim {
            index,
            claim_amount: U128(claim_amount),
            is_final: balance_claimed_new == self.schedule.total_balance(),
        }
    }

    pub fn assert_new_valid(&self, total_balance: Balance) {
        assert_eq!(
            self.claimed_balance.0, 0,
            "The initial lockup claimed balance should be 0"
        );
        self.schedule.assert_valid(total_balance);

        if let Some(termination_config) = &self.termination_config {
            match &termination_config.vesting_schedule {
                VestingConditions::SameAsLockupSchedule => {
                    // Ok, using lockup schedule.
                }
                VestingConditions::Hash(_hash) => {
                    // Ok, using unknown hash. Can't verify.
                }
                VestingConditions::Schedule(schedule) => {
                    schedule.assert_valid(total_balance);
                    self.schedule.assert_valid_termination_schedule(schedule);
                }
            }
        }
    }
}

#[near(serializers=[borsh, json])]
#[derive(Debug, PartialEq, Clone)]
pub struct LockupCreate {
    pub account_id: AccountId,
    pub schedule: Schedule,
    pub vesting_schedule: Option<VestingConditions>,
}

#[cfg(not(target_arch = "wasm32"))]
impl LockupCreate {
    pub fn new_unlocked(account_id: AccountId, total_balance: Balance) -> Self {
        Self {
            account_id,
            schedule: Schedule::new_unlocked(total_balance),
            vesting_schedule: None,
        }
    }
}

impl LockupCreate {
    pub fn into_lockup(&self, payer_id: &AccountId) -> Lockup {
        Lockup {
            account_id: self.account_id.clone(),
            schedule: self.schedule.clone(),
            claimed_balance: U128(0),
            termination_config: self.vesting_schedule.clone().map(|vesting_schedule| TerminationConfig {
                beneficiary_id: payer_id.clone(),
                vesting_schedule,
            }),
        }
    }
}

#[near(serializers=[json])]
#[derive(Debug, PartialEq)]
pub struct LockupView {
    pub account_id: AccountId,
    pub schedule: Schedule,

    #[serde(default)]
    pub claimed_balance: WrappedBalance,
    /// An optional configuration that allows vesting/lockup termination.
    pub termination_config: Option<TerminationConfig>,

    pub total_balance: WrappedBalance,
    pub unclaimed_balance: WrappedBalance,
    /// The current timestamp
    pub timestamp: TimestampSec,
}

impl From<Lockup> for LockupView {
    fn from(lockup: Lockup) -> Self {
        let total_balance = U128(lockup.schedule.total_balance());
        let timestamp = current_timestamp_sec();
        let unclaimed_balance = lockup
            .schedule
            .unlocked_balance(timestamp)
            .saturating_sub(lockup.claimed_balance.0);
        let Lockup {
            account_id,
            schedule,
            claimed_balance,
            termination_config,
        } = lockup;
        Self {
            account_id,
            schedule,
            claimed_balance,
            termination_config,
            total_balance,
            unclaimed_balance: U128(unclaimed_balance),
            timestamp,
        }
    }
}

#[near(serializers=[json])]
#[derive(Debug, PartialEq)]
pub struct LockupCreateView {
    pub account_id: AccountId,
    pub schedule: Schedule,
    pub vesting_schedule: Option<VestingConditions>,

    pub claimed_balance: WrappedBalance,
    pub total_balance: WrappedBalance,
    pub unclaimed_balance: WrappedBalance,
    /// The current timestamp
    pub timestamp: TimestampSec,
}

impl From<LockupCreate> for LockupCreateView {
    fn from(lockup_create: LockupCreate) -> Self {
        let total_balance = U128(lockup_create.schedule.total_balance());
        let timestamp = current_timestamp_sec();
        let unclaimed_balance = U128(lockup_create.schedule.unlocked_balance(timestamp));
        let LockupCreate {
            account_id,
            schedule,
            vesting_schedule,
        } = lockup_create;
        Self {
            account_id,
            schedule,
            vesting_schedule,
            claimed_balance: U128(0),
            total_balance,
            unclaimed_balance,
            timestamp,
        }
    }
}
