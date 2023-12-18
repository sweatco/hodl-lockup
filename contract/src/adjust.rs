use near_sdk::require;

use crate::*;

#[near_bindgen]
impl AdjustApi for Contract {
    #[payable]
    fn adjust(&mut self, beneficiary_id: AccountId, lockup_index: LockupIndex) -> PromiseOrValue<()> {
        assert_one_yocto();
        self.assert_deposit_whitelist(&env::predecessor_account_id());

        let mut lockup = self.lockups.get(lockup_index as _).expect("Lockup not found");
        require!(lockup.is_adjustable, "Lockup is not adjustable");

        let now = current_timestamp_sec();
        let cliff_end = lockup.schedule.0.first().expect("Checkpoint is required").timestamp;

        if now >= cliff_end {
            panic_str("Cliff already ended");
        }

        let created_at = cliff_end - ONE_YEAR_SECONDS;
        let percent = (now - created_at) as f64 / ONE_YEAR_SECONDS as f64;

        let initial_schedule = lockup.schedule.clone();
        for checkpoint in lockup.schedule.0.iter_mut() {
            checkpoint.balance = (checkpoint.balance as f64 * percent) as _;
        }
        self.lockups.replace(lockup_index as _, &lockup);

        let balance_to_refund = initial_schedule.total_balance() - lockup.schedule.total_balance();

        if balance_to_refund > 0 {
            Promise::new(self.token_account_id.clone())
                .ft_transfer(
                    &beneficiary_id.clone(),
                    balance_to_refund,
                    Some(format!("Adjusted lockup #{lockup_index}")),
                )
                .then(
                    ext_self::ext(env::current_account_id())
                        .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
                        .after_lockup_adjustment(lockup_index, initial_schedule),
                )
                .into()
        } else {
            PromiseOrValue::Value(())
        }
    }

    #[payable]
    fn revoke(&mut self, beneficiary_id: AccountId, lockup_indices: Vec<LockupIndex>) -> PromiseOrValue<()> {
        assert_one_yocto();
        self.assert_deposit_whitelist(&env::predecessor_account_id());

        let mut total_balance = 0;
        let mut original_schedules: Vec<(LockupIndex, Schedule)> = vec![];

        for index in lockup_indices.clone() {
            let mut lockup = self.lockups.get(index as _).expect("Lockup not found");
            require!(lockup.is_adjustable, "Lockup is not adjustable");

            total_balance += lockup.schedule.total_balance();

            original_schedules.push((index, lockup.schedule));
            lockup.schedule = Schedule::new_unlocked(0);
            self.lockups.replace(index as _, &lockup);
        }

        if total_balance > 0 {
            Promise::new(self.token_account_id.clone())
                .ft_transfer(
                    &beneficiary_id.clone(),
                    total_balance,
                    Some(format!("Revoke for lockups {lockup_indices:?}")),
                )
                .then(
                    ext_self::ext(env::current_account_id())
                        .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
                        .after_lockups_revoke(original_schedules),
                )
                .into()
        } else {
            PromiseOrValue::Value(())
        }
    }
}

#[ext_contract(ext_self)]
pub trait AdjustmentSelfCallbacks {
    fn after_lockup_adjustment(&mut self, lockup_index: LockupIndex, schedule: Schedule);

    fn after_lockups_revoke(&mut self, original_schedules: Vec<(LockupIndex, Schedule)>);
}

#[near_bindgen]
impl AdjustmentSelfCallbacks for Contract {
    #[private]
    fn after_lockup_adjustment(&mut self, lockup_index: LockupIndex, schedule: Schedule) {
        if is_promise_success() {
            return;
        }

        let mut lockup = self.lockups.get(lockup_index as _).expect("Lockup not found");
        lockup.schedule = schedule;
    }

    fn after_lockups_revoke(&mut self, original_schedules: Vec<(LockupIndex, Schedule)>) {
        if is_promise_success() {
            for (index, _) in original_schedules {
                let mut lockup = self.lockups.get(index as _).expect("Lockup not found");

                let mut account_lockups = self
                    .account_lockups
                    .get(&lockup.account_id)
                    .expect("Account lockups not found");
                account_lockups.remove(&index);
                self.account_lockups.insert(&lockup.account_id, &account_lockups);

                lockup.account_id = env::current_account_id();
                self.lockups.replace(index as _, &lockup);
            }
        } else {
            for (index, schedule) in original_schedules {
                let mut lockup = self.lockups.get(index as _).expect("Lockup not found");
                lockup.schedule = schedule;
                self.lockups.replace(index as _, &lockup);
            }
        }
    }
}
