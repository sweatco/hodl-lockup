use model::{
    lockup::Lockup,
    schedule::Schedule,
    termination::{TerminationConfig, VestingConditions},
    TimestampSec,
};
use near_sdk::{AccountId, Balance, CryptoHash};

pub(crate) trait TerminableLockup {
    fn terminate(
        &mut self,
        hashed_schedule: Option<Schedule>,
        termination_timestamp: TimestampSec,
    ) -> (Balance, AccountId);

    fn make_terminable(&mut self, beneficiary_id: AccountId);
}

impl TerminableLockup for Lockup {
    fn terminate(
        &mut self,
        hashed_schedule: Option<Schedule>,
        termination_timestamp: TimestampSec,
    ) -> (Balance, AccountId) {
        let termination_config = self.termination_config.take().expect("No termination config");
        let total_balance = self.schedule.total_balance();
        let vested_balance = match &termination_config.vesting_schedule {
            VestingConditions::SameAsLockupSchedule => &self.schedule,
            VestingConditions::Hash(hash) => {
                let schedule = hashed_schedule
                    .as_ref()
                    .expect("Revealed schedule required for the termination");
                let hash: CryptoHash = (*hash).into();
                assert_eq!(hash, schedule.hash(), "The revealed schedule hash doesn't match");
                schedule.assert_valid(total_balance);
                self.schedule.assert_valid_termination_schedule(schedule);
                schedule
            }
            VestingConditions::Schedule(schedule) => schedule,
        }
        .unlocked_balance(termination_timestamp);
        let unvested_balance = total_balance - vested_balance;
        if unvested_balance > 0 {
            self.schedule.terminate(vested_balance, termination_timestamp);
        }
        (unvested_balance, termination_config.beneficiary_id)
    }

    fn make_terminable(&mut self, beneficiary_id: AccountId) {
        self.termination_config = Some(TerminationConfig {
            beneficiary_id,
            vesting_schedule: VestingConditions::SameAsLockupSchedule,
        });
    }
}
