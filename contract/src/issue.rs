use hodl_model::{
    api::IssueApi,
    lockup::Lockup,
    schedule::{Checkpoint, Schedule},
    termination::{TerminationConfig, VestingConditions},
    TimestampSec, ONE_YEAR_SEC,
};
use near_sdk::{env, json_types::U128, near_bindgen, AccountId};

use crate::{
    event::{emit, EventKind, FtLockupCreateLockup},
    Contract, ContractExt,
};

#[near_bindgen]
impl IssueApi for Contract {
    fn issue(&mut self, issue_date: TimestampSec, amounts: Vec<(AccountId, U128)>) {
        let issuer_id = env::predecessor_account_id();
        self.assert_deposit_whitelist(&issuer_id);

        let cliff_end_date = issue_date + ONE_YEAR_SEC;
        let lockup_end_date = cliff_end_date + 3 * ONE_YEAR_SEC;
        let termination_config = TerminationConfig {
            beneficiary_id: issuer_id,
            vesting_schedule: VestingConditions::SameAsLockupSchedule,
        };

        let mut events: Vec<FtLockupCreateLockup> = vec![];

        for (account_id, amount) in amounts {
            let lockup = Lockup {
                account_id,
                schedule: Schedule(vec![
                    Checkpoint {
                        timestamp: cliff_end_date,
                        balance: 0.into(),
                    },
                    Checkpoint {
                        timestamp: lockup_end_date,
                        balance: amount,
                    },
                ]),
                claimed_balance: 0.into(),
                termination_config: Some(termination_config.clone()),
            };

            let index = self.internal_add_lockup(&lockup);
            events.push((index, lockup, None).into());
        }

        emit(EventKind::FtLockupCreateLockup(events));
    }
}
