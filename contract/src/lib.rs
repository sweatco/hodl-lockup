use std::{
    collections::{HashMap, HashSet},
    convert::Into,
};

use hodl_model::{
    draft::{Draft, DraftGroup, DraftGroupIndex, DraftIndex},
    lockup::{Lockup, LockupIndex},
    lockup_api::LockupApi,
    schedule::Schedule,
    termination::TerminationConfig,
    util::current_timestamp_sec,
    view_api::LockupViewApi,
    TimestampSec, TokenAccountId, WrappedBalance,
};
// use near_contract_standards::fungible_token::core_impl::ext_fungible_token;
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::{
    assert_one_yocto,
    collections::{LookupMap, UnorderedMap, UnorderedSet, Vector},
    env, ext_contract, is_promise_success,
    json_types::{Base58CryptoHash, U128},
    log, near, near_bindgen,
    serde::Serialize,
    serde_json, AccountId, BorshStorageKey, Gas, NearToken, PanicOnDefault, Promise, PromiseOrValue,
};
use near_self_update_proc::SelfUpdate;

pub mod callbacks;
pub mod event;
pub mod ft_token_receiver;
pub mod internal;

mod migration;
pub mod view;

use crate::{
    callbacks::{ext_self, SelfCallbacks},
    event::{
        emit, EventKind, FtLockupAddToDepositWhitelist, FtLockupAddToDraftOperatorsWhitelist, FtLockupClaimLockup,
        FtLockupCreateDraft, FtLockupCreateDraftGroup, FtLockupCreateLockup, FtLockupDeleteDraft,
        FtLockupDiscardDraftGroup, FtLockupFundDraftGroup, FtLockupNew, FtLockupRemoveFromDepositWhitelist,
        FtLockupRemoveFromDraftOperatorsWhitelist, FtLockupTerminateLockup,
    },
    serde_json::json,
};

pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const GAS_FOR_FT_TRANSFER: Gas = Gas::from_gas(15_000_000_000_000);
const GAS_FOR_AFTER_FT_TRANSFER: Gas = Gas::from_gas(20_000_000_000_000);
const GAS_EXT_CALL_COST: Gas = Gas::from_gas(10_000_000_000_000);
const GAS_MIN_FOR_CONVERT: Gas = Gas::from_gas(15_000_000_000_000);

#[near(contract_state)]
#[derive(PanicOnDefault, SelfUpdate)]
pub struct Contract {
    pub token_account_id: TokenAccountId,

    pub lockups: Vector<Lockup>,

    pub account_lockups: LookupMap<AccountId, HashSet<LockupIndex>>,

    /// account ids that can perform all actions:
    /// - manage deposit_whitelist
    /// - manage drafts, draft_groups
    /// - create lockups, terminate lockups, fund draft_groups
    pub deposit_whitelist: UnorderedSet<AccountId>,

    /// account ids that can perform all actions on drafts:
    /// - manage drafts, draft_groups
    pub draft_operators_whitelist: UnorderedSet<AccountId>,

    pub next_draft_id: DraftIndex,
    pub drafts: LookupMap<DraftIndex, Draft>,
    pub next_draft_group_id: DraftGroupIndex,
    pub draft_groups: UnorderedMap<DraftGroupIndex, DraftGroup>,

    /// The account ID authorized to perform sensitive operations on the contract.
    pub manager: AccountId,
}

#[near(serializers=[borsh, json])]
#[derive(BorshStorageKey)]
pub(crate) enum StorageKey {
    Lockups,
    AccountLockups,
    DepositWhitelist,
    DraftOperatorsWhitelist,
    Drafts,
    DraftGroups,
}

impl Contract {
    fn assert_account_can_update(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.manager,
            "Only the manager can update the code"
        );
    }
}

#[near_bindgen]
impl LockupApi for Contract {
    #[init]
    fn new(
        token_account_id: AccountId,
        deposit_whitelist: Vec<AccountId>,
        draft_operators_whitelist: Option<Vec<AccountId>>,
        manager: AccountId,
    ) -> Self {
        let mut deposit_whitelist_set = UnorderedSet::new(StorageKey::DepositWhitelist);
        deposit_whitelist_set.extend(deposit_whitelist.clone().into_iter().map(Into::into));
        let mut draft_operators_whitelist_set = UnorderedSet::new(StorageKey::DraftOperatorsWhitelist);
        draft_operators_whitelist_set.extend(
            draft_operators_whitelist
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(Into::into),
        );
        emit(EventKind::FtLockupNew(FtLockupNew {
            token_account_id: token_account_id.clone(),
        }));
        emit(EventKind::FtLockupAddToDepositWhitelist(
            FtLockupAddToDepositWhitelist {
                account_ids: deposit_whitelist.into_iter().map(Into::into).collect(),
            },
        ));
        emit(EventKind::FtLockupAddToDraftOperatorsWhitelist(
            FtLockupAddToDraftOperatorsWhitelist {
                account_ids: draft_operators_whitelist
                    .unwrap_or_default()
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            },
        ));
        Self {
            lockups: Vector::new(StorageKey::Lockups),
            account_lockups: LookupMap::new(StorageKey::AccountLockups),
            token_account_id,
            deposit_whitelist: deposit_whitelist_set,
            draft_operators_whitelist: draft_operators_whitelist_set,
            next_draft_id: 0,
            drafts: LookupMap::new(StorageKey::Drafts),
            next_draft_group_id: 0,
            draft_groups: UnorderedMap::new(StorageKey::DraftGroups),
            manager,
        }
    }

    fn claim(&mut self, amounts: Option<Vec<(LockupIndex, Option<WrappedBalance>)>>) -> PromiseOrValue<WrappedBalance> {
        let account_id = env::predecessor_account_id();

        let (claim_amounts, mut lockups_by_id) = if let Some(amounts) = amounts {
            let lockups_by_id: HashMap<LockupIndex, Lockup> = self
                .internal_get_account_lockups_by_id(&account_id, &amounts.iter().map(|x| x.0).collect())
                .into_iter()
                .collect();
            let amounts: HashMap<LockupIndex, WrappedBalance> = amounts
                .into_iter()
                .map(|(lockup_id, amount)| {
                    (
                        lockup_id,
                        if let Some(amount) = amount {
                            amount
                        } else {
                            let lockup = lockups_by_id.get(&lockup_id).expect("lockup not found");
                            let unlocked_balance = lockup.schedule.unlocked_balance(current_timestamp_sec());
                            (unlocked_balance - lockup.claimed_balance).into()
                        },
                    )
                })
                .collect();
            (amounts, lockups_by_id)
        } else {
            let lockups_by_id: HashMap<LockupIndex, Lockup> =
                self.internal_get_account_lockups(&account_id).into_iter().collect();
            let amounts: HashMap<LockupIndex, WrappedBalance> = lockups_by_id
                .iter()
                .map(|(lockup_id, lockup)| {
                    let unlocked_balance = lockup.schedule.unlocked_balance(current_timestamp_sec());
                    let amount: WrappedBalance = (unlocked_balance - lockup.claimed_balance).into();

                    (*lockup_id, amount)
                })
                .collect();
            (amounts, lockups_by_id)
        };

        let account_id = env::predecessor_account_id();
        let mut lockup_claims = vec![];
        let mut total_claim_amount = 0;
        for (lockup_index, lockup_claim_amount) in claim_amounts {
            let lockup = lockups_by_id.get_mut(&lockup_index).unwrap();
            let lockup_claim = lockup.claim(lockup_index, lockup_claim_amount.0);

            if lockup_claim.claim_amount.0 > 0 {
                log!("Claiming {} form lockup #{}", lockup_claim.claim_amount.0, lockup_index);
                total_claim_amount += lockup_claim.claim_amount.0;
                self.lockups.replace(u64::from(lockup_index), lockup);
                lockup_claims.push(lockup_claim);
            }
        }
        log!("Total claim {}", total_claim_amount);

        if total_claim_amount > 0 {
            Promise::new(self.token_account_id.clone())
                .ft_transfer(
                    &account_id,
                    total_claim_amount,
                    Some(format!(
                        "Claiming unlocked {} balance from {}",
                        total_claim_amount,
                        env::current_account_id()
                    )),
                )
                .then(
                    ext_self::ext(env::current_account_id())
                        .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
                        .after_ft_transfer(account_id, lockup_claims),
                )
                .into()
        } else {
            PromiseOrValue::Value(0.into())
        }
    }

    #[payable]
    fn terminate(
        &mut self,
        lockup_index: LockupIndex,
        hashed_schedule: Option<Schedule>,
        termination_timestamp: Option<TimestampSec>,
    ) -> PromiseOrValue<WrappedBalance> {
        assert_one_yocto();
        self.assert_deposit_whitelist(&env::predecessor_account_id());
        let mut lockup = self.lockups.get(u64::from(lockup_index)).expect("Lockup not found");
        let current_timestamp = current_timestamp_sec();
        let termination_timestamp = termination_timestamp.unwrap_or(current_timestamp);

        let (unvested_balance, beneficiary_id) = lockup.terminate(hashed_schedule, termination_timestamp);
        self.lockups.replace(u64::from(lockup_index), &lockup);

        // no need to store empty lockup
        if lockup.schedule.total_balance() == 0 {
            let mut indices = self.account_lockups.get(&lockup.account_id).unwrap_or_default();
            indices.remove(&lockup_index);
            self.internal_save_account_lockups(&lockup.account_id, indices);
        }

        let event = FtLockupTerminateLockup {
            id: lockup_index,
            termination_timestamp,
            unvested_balance: unvested_balance.into(),
        };
        emit(EventKind::FtLockupTerminateLockup(vec![event]));

        if unvested_balance > 0 {
            Promise::new(self.token_account_id.clone())
                .ft_transfer(
                    &beneficiary_id.clone(),
                    unvested_balance,
                    Some(format!("Terminated lockup #{lockup_index}")),
                )
                .then(
                    ext_self::ext(env::current_account_id())
                        .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
                        .after_lockup_termination(beneficiary_id, unvested_balance.into()),
                )
                .into()
        } else {
            PromiseOrValue::Value(0.into())
        }
    }

    // preserving both options for API compatibility
    #[payable]
    fn add_to_deposit_whitelist(&mut self, account_id: Option<AccountId>, account_ids: Option<Vec<AccountId>>) {
        assert_one_yocto();
        self.assert_deposit_whitelist(&env::predecessor_account_id());
        let account_ids = if let Some(account_ids) = account_ids {
            account_ids
        } else {
            vec![account_id.expect("expected either account_id or account_ids")]
        };
        for account_id in &account_ids {
            self.deposit_whitelist.insert(account_id);
        }
        emit(EventKind::FtLockupAddToDepositWhitelist(
            FtLockupAddToDepositWhitelist {
                account_ids: account_ids.into_iter().map(Into::into).collect(),
            },
        ));
    }

    // preserving both options for API compatibility
    #[payable]
    fn remove_from_deposit_whitelist(&mut self, account_id: Option<AccountId>, account_ids: Option<Vec<AccountId>>) {
        assert_one_yocto();
        self.assert_deposit_whitelist(&env::predecessor_account_id());
        let account_ids = if let Some(account_ids) = account_ids {
            account_ids
        } else {
            vec![account_id.expect("expected either account_id or account_ids")]
        };
        for account_id in &account_ids {
            self.deposit_whitelist.remove(account_id);
        }
        assert!(
            !self.deposit_whitelist.is_empty(),
            "cannot remove all accounts from deposit whitelist",
        );
        emit(EventKind::FtLockupRemoveFromDepositWhitelist(
            FtLockupRemoveFromDepositWhitelist {
                account_ids: account_ids.into_iter().map(Into::into).collect(),
            },
        ));
    }

    #[payable]
    fn add_to_draft_operators_whitelist(&mut self, account_ids: Vec<AccountId>) {
        assert_one_yocto();
        self.assert_deposit_whitelist(&env::predecessor_account_id());
        for account_id in &account_ids {
            self.draft_operators_whitelist.insert(account_id);
        }
        emit(EventKind::FtLockupAddToDraftOperatorsWhitelist(
            FtLockupAddToDraftOperatorsWhitelist {
                account_ids: account_ids.into_iter().map(Into::into).collect(),
            },
        ));
    }

    #[payable]
    fn remove_from_draft_operators_whitelist(&mut self, account_ids: Vec<AccountId>) {
        assert_one_yocto();
        self.assert_deposit_whitelist(&env::predecessor_account_id());
        for account_id in &account_ids {
            self.draft_operators_whitelist.remove(account_id);
        }
        emit(EventKind::FtLockupRemoveFromDraftOperatorsWhitelist(
            FtLockupRemoveFromDraftOperatorsWhitelist {
                account_ids: account_ids.into_iter().map(Into::into).collect(),
            },
        ));
    }

    fn create_draft_group(&mut self) -> DraftGroupIndex {
        self.assert_draft_operators_whitelist(&env::predecessor_account_id());

        let index = self.next_draft_group_id;
        self.next_draft_group_id += 1;
        assert!(
            self.draft_groups.insert(&index, &DraftGroup::default()).is_none(),
            "Invariant"
        );
        emit(EventKind::FtLockupCreateDraftGroup(vec![FtLockupCreateDraftGroup {
            id: index,
        }]));

        index
    }

    fn create_draft(&mut self, draft: Draft) -> DraftIndex {
        self.create_drafts(vec![draft])[0]
    }

    fn create_drafts(&mut self, drafts: Vec<Draft>) -> Vec<DraftIndex> {
        self.assert_draft_operators_whitelist(&env::predecessor_account_id());
        let mut draft_group_lookup: HashMap<DraftGroupIndex, DraftGroup> = HashMap::new();
        let mut events: Vec<FtLockupCreateDraft> = vec![];
        let draft_ids: Vec<DraftIndex> = drafts
            .into_iter()
            .map(|draft| {
                let draft_group = draft_group_lookup.entry(draft.draft_group_id).or_insert_with(|| {
                    self.draft_groups
                        .get(&draft.draft_group_id as _)
                        .expect("draft group not found")
                });
                draft_group.assert_can_add_draft();
                draft.assert_new_valid();

                let index = self.next_draft_id;
                self.next_draft_id += 1;
                assert!(self.drafts.insert(&index, &draft).is_none(), "Invariant");
                draft_group.total_amount = draft_group
                    .total_amount
                    .checked_add(draft.total_balance())
                    .expect("attempt to add with overflow");
                draft_group.draft_indices.insert(index);
                let event: FtLockupCreateDraft = (index, draft).into();
                events.push(event);

                index
            })
            .collect();

        emit(EventKind::FtLockupCreateDraft(events));
        for (draft_group_id, draft_group) in draft_group_lookup {
            self.draft_groups.insert(&draft_group_id as _, &draft_group);
        }

        draft_ids
    }

    fn convert_draft(&mut self, draft_id: DraftIndex) -> LockupIndex {
        self.convert_drafts(vec![draft_id])[0]
    }

    fn discard_draft_group(&mut self, draft_group_id: DraftGroupIndex) {
        self.assert_draft_operators_whitelist(&env::predecessor_account_id());

        let mut draft_group = self
            .draft_groups
            .get(&draft_group_id as _)
            .expect("draft group not found");
        draft_group.discard();

        if draft_group.draft_indices.is_empty() {
            self.draft_groups.remove(&draft_group_id as _);
        } else {
            self.draft_groups.insert(&draft_group_id as _, &draft_group);
        }

        emit(EventKind::FtLockupDiscardDraftGroup(vec![FtLockupDiscardDraftGroup {
            id: draft_group_id,
        }]));
    }

    fn delete_drafts(&mut self, draft_ids: Vec<DraftIndex>) {
        // no authorization required here since the draft group discard has been authorized
        let mut draft_group_lookup: HashMap<DraftGroupIndex, DraftGroup> = HashMap::new();
        let mut events: Vec<FtLockupDeleteDraft> = vec![];
        for draft_id in draft_ids {
            let draft = self.drafts.remove(&draft_id as _).expect("draft not found");
            let draft_group = draft_group_lookup.entry(draft.draft_group_id).or_insert_with(|| {
                self.draft_groups
                    .get(&draft.draft_group_id as _)
                    .expect("draft group not found")
            });

            draft_group.assert_can_delete_draft();
            let amount = draft.total_balance();
            assert!(draft_group.total_amount >= amount, "Invariant");
            draft_group.total_amount -= amount;

            assert!(draft_group.draft_indices.remove(&draft_id), "Invariant");

            let event = FtLockupDeleteDraft { id: draft_id };
            events.push(event);
        }

        emit(EventKind::FtLockupDeleteDraft(events));

        for (draft_group_id, draft_group) in draft_group_lookup {
            if draft_group.draft_indices.is_empty() {
                self.draft_groups.remove(&draft_group_id as _);
            } else {
                self.draft_groups.insert(&draft_group_id as _, &draft_group);
            }
        }
    }

    #[payable]
    fn edit(&mut self, index: LockupIndex, schedule: Option<Schedule>, termination_config: Option<TerminationConfig>) {
        self.assert_deposit_whitelist(&env::predecessor_account_id());
        assert_one_yocto();

        let mut lockup = self
            .lockups
            .get(index as _)
            .unwrap_or_else(|| panic!("No lockup found at index {index}"));

        if let Some(schedule) = schedule {
            lockup.schedule = schedule;
        }

        if termination_config.is_some() {
            lockup.termination_config = termination_config;
        }

        self.lockups.replace(index as _, &lockup);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hodl_model::lockup::Lockup;
    use hodl_model::schedule::{Checkpoint, Schedule};
    use hodl_model::termination::{TerminationConfig, VestingConditions};
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::{testing_env, AccountId, NearToken};
    use std::str::FromStr;

    fn get_context(predecessor_account_id: AccountId, attached_deposit: NearToken) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .predecessor_account_id(predecessor_account_id)
            .attached_deposit(attached_deposit)
            .current_account_id(accounts(0)); // Assuming accounts(0) is the contract itself
        builder
    }

    #[test]
    fn test_edit_lockup_schedule_and_termination_config() {
        let manager_account = accounts(0);
        let beneficiary_account = accounts(1);
        let token_account = accounts(2);

        let mut context = get_context(manager_account.clone(), NearToken::from_yoctonear(1));
        testing_env!(context.build());

        let mut contract = Contract::new(
            token_account.clone(),
            vec![manager_account.clone()], // manager is in deposit whitelist
            None,
            manager_account.clone(),
        );

        // Create an initial lockup
        let initial_schedule = Schedule(vec![
            Checkpoint {
                timestamp: 0,
                balance: 1000,
            },
            Checkpoint {
                timestamp: 100,
                balance: 1000,
            },
        ]);
        let initial_lockup = Lockup {
            account_id: beneficiary_account.clone(),
            schedule: initial_schedule.clone(),
            claimed_balance: 0,
            termination_config: None,
        };
        contract.lockups.push(&initial_lockup);
        let lockup_index = 0; // The first lockup pushed will have index 0

        // Define new schedule and termination config
        let new_schedule = Schedule(vec![
            Checkpoint {
                timestamp: 10,
                balance: 2000,
            },
            Checkpoint {
                timestamp: 120,
                balance: 2000,
            },
        ]);
        let new_termination_config = TerminationConfig {
            beneficiary_id: accounts(3),
            vesting_schedule: VestingConditions::SameAsLockupSchedule,
        };

        // Call the edit function
        contract.edit(
            lockup_index,
            Some(new_schedule.clone()),
            Some(new_termination_config.clone()),
        );

        // Assert the lockup was updated
        let updated_lockup = contract
            .lockups
            .get(lockup_index as _)
            .expect("Lockup not found after edit");
        assert_eq!(updated_lockup.schedule.0[0].timestamp, new_schedule.0[0].timestamp);
        assert_eq!(updated_lockup.schedule.0[0].balance, new_schedule.0[0].balance);
        assert_eq!(updated_lockup.schedule.0[1].timestamp, new_schedule.0[1].timestamp);
        assert_eq!(updated_lockup.schedule.0[1].balance, new_schedule.0[1].balance);

        assert!(updated_lockup.termination_config.is_some());
        let actual_termination_config = updated_lockup.termination_config.unwrap();
        assert_eq!(
            actual_termination_config.beneficiary_id,
            new_termination_config.beneficiary_id
        );
        assert_eq!(
            actual_termination_config.vesting_schedule,
            new_termination_config.vesting_schedule
        );
    }

    #[test]
    #[should_panic(expected = "Not in deposit whitelist")]
    fn test_edit_unauthorized() {
        let manager_account = accounts(0);
        let unauthorized_account = accounts(1);
        let token_account = accounts(2);

        let mut context = get_context(unauthorized_account.clone(), NearToken::from_yoctonear(1));
        testing_env!(context.build());

        let mut contract = Contract::new(
            token_account.clone(),
            vec![manager_account.clone()], // manager is in deposit whitelist
            None,
            manager_account.clone(),
        );

        // Create an initial lockup
        let initial_schedule = Schedule(vec![
            Checkpoint {
                timestamp: 0,
                balance: 1000,
            },
            Checkpoint {
                timestamp: 100,
                balance: 1000,
            },
        ]);
        let initial_lockup = Lockup {
            account_id: unauthorized_account.clone(),
            schedule: initial_schedule.clone(),
            claimed_balance: 0,
            termination_config: None,
        };
        contract.lockups.push(&initial_lockup);
        let lockup_index = 0;

        // Attempt to edit as unauthorized user
        contract.edit(lockup_index, None, None);
    }

    #[test]
    #[should_panic(expected = "No lockup found at index 999")]
    fn test_edit_non_existent_lockup() {
        let manager_account = accounts(0);
        let token_account = accounts(2);

        let mut context = get_context(manager_account.clone(), NearToken::from_yoctonear(1));
        testing_env!(context.build());

        let mut contract = Contract::new(
            token_account.clone(),
            vec![manager_account.clone()], // manager is in deposit whitelist
            None,
            manager_account.clone(),
        );

        // Attempt to edit a non-existent lockup
        contract.edit(999, None, None);
    }

    #[test]
    #[should_panic(expected = "Requires attached deposit of exactly 1 yoctoNEAR")]
    fn test_edit_no_attached_deposit() {
        let manager_account = accounts(0);
        let beneficiary_account = accounts(1);
        let token_account = accounts(2);

        let mut context = get_context(manager_account.clone(), NearToken::from_yoctonear(0)); // 0 yoctoNEAR
        testing_env!(context.build());

        let mut contract = Contract::new(
            token_account.clone(),
            vec![manager_account.clone()], // manager is in deposit whitelist
            None,
            manager_account.clone(),
        );

        // Create an initial lockup
        let initial_schedule = Schedule(vec![
            Checkpoint {
                timestamp: 0,
                balance: 1000,
            },
            Checkpoint {
                timestamp: 100,
                balance: 1000,
            },
        ]);
        let initial_lockup = Lockup {
            account_id: beneficiary_account.clone(),
            schedule: initial_schedule.clone(),
            claimed_balance: 0,
            termination_config: None,
        };
        contract.lockups.push(&initial_lockup);
        let lockup_index = 0;

        // Attempt to edit with no attached deposit
        contract.edit(lockup_index, None, None);
    }

    #[test]
    fn test_edit_only_schedule() {
        let manager_account = accounts(0);
        let beneficiary_account = accounts(1);
        let token_account = accounts(2);

        let mut context = get_context(manager_account.clone(), NearToken::from_yoctonear(1));
        testing_env!(context.build());

        let mut contract = Contract::new(
            token_account.clone(),
            vec![manager_account.clone()], // manager is in deposit whitelist
            None,
            manager_account.clone(),
        );

        // Create an initial lockup with a termination config
        let initial_schedule = Schedule(vec![
            Checkpoint {
                timestamp: 0,
                balance: 1000,
            },
            Checkpoint {
                timestamp: 100,
                balance: 1000,
            },
        ]);
        let initial_termination_config = TerminationConfig {
            beneficiary_id: accounts(5),
            vesting_schedule: VestingConditions::SameAsLockupSchedule,
        };
        let initial_lockup = Lockup {
            account_id: beneficiary_account.clone(),
            schedule: initial_schedule.clone(),
            claimed_balance: 0,
            termination_config: Some(initial_termination_config.clone()),
        };
        contract.lockups.push(&initial_lockup);
        let lockup_index = 0;

        // Define new schedule
        let new_schedule = Schedule(vec![
            Checkpoint {
                timestamp: 10,
                balance: 2000,
            },
            Checkpoint {
                timestamp: 120,
                balance: 2000,
            },
        ]);

        // Call the edit function, only updating schedule
        contract.edit(
            lockup_index,
            Some(new_schedule.clone()),
            None, // termination_config is None
        );

        // Assert the lockup was updated correctly
        let updated_lockup = contract
            .lockups
            .get(lockup_index as _)
            .expect("Lockup not found after edit");
        assert_eq!(updated_lockup.schedule.0[0].timestamp, new_schedule.0[0].timestamp);
        assert_eq!(updated_lockup.schedule.0[0].balance, new_schedule.0[0].balance);
        assert_eq!(updated_lockup.schedule.0[1].timestamp, new_schedule.0[1].timestamp);
        assert_eq!(updated_lockup.schedule.0[1].balance, new_schedule.0[1].balance);

        // Ensure termination_config remains unchanged
        assert!(updated_lockup.termination_config.is_some());
        let actual_termination_config = updated_lockup.termination_config.unwrap();
        assert_eq!(
            actual_termination_config.beneficiary_id,
            initial_termination_config.beneficiary_id
        );
        assert_eq!(
            actual_termination_config.vesting_schedule,
            initial_termination_config.vesting_schedule
        );
    }

    #[test]
    fn test_edit_only_termination_config() {
        let manager_account = accounts(0);
        let beneficiary_account = accounts(1);
        let token_account = accounts(2);

        let mut context = get_context(manager_account.clone(), NearToken::from_yoctonear(1));
        testing_env!(context.build());

        let mut contract = Contract::new(
            token_account.clone(),
            vec![manager_account.clone()], // manager is in deposit whitelist
            None,
            manager_account.clone(),
        );

        // Create an initial lockup with a schedule
        let initial_schedule = Schedule(vec![
            Checkpoint {
                timestamp: 0,
                balance: 1000,
            },
            Checkpoint {
                timestamp: 100,
                balance: 1000,
            },
        ]);
        let initial_lockup = Lockup {
            account_id: beneficiary_account.clone(),
            schedule: initial_schedule.clone(),
            claimed_balance: 0,
            termination_config: None, // No initial termination config
        };
        contract.lockups.push(&initial_lockup);
        let lockup_index = 0;

        // Define new termination config
        let new_termination_config = TerminationConfig {
            beneficiary_id: accounts(3),
            vesting_schedule: VestingConditions::SameAsLockupSchedule,
        };

        // Call the edit function, only updating termination config
        contract.edit(
            lockup_index,
            None, // schedule is None
            Some(new_termination_config.clone()),
        );

        // Assert the lockup was updated correctly
        let updated_lockup = contract
            .lockups
            .get(lockup_index as _)
            .expect("Lockup not found after edit");
        // Ensure schedule remains unchanged
        assert_eq!(updated_lockup.schedule.0[0].timestamp, initial_schedule.0[0].timestamp);
        assert_eq!(updated_lockup.schedule.0[0].balance, initial_schedule.0[0].balance);
        assert_eq!(updated_lockup.schedule.0[1].timestamp, initial_schedule.0[1].timestamp);
        assert_eq!(updated_lockup.schedule.0[1].balance, initial_schedule.0[1].balance);

        assert!(updated_lockup.termination_config.is_some());
        let actual_termination_config = updated_lockup.termination_config.unwrap();
        assert_eq!(
            actual_termination_config.beneficiary_id,
            new_termination_config.beneficiary_id
        );
        assert_eq!(
            actual_termination_config.vesting_schedule,
            new_termination_config.vesting_schedule
        );
    }

    const ONE_YEAR_SEC: u32 = 31_536_000;
    const GENESIS_TIMESTAMP_SEC: u32 = 0;

    fn alice() -> AccountId {
        AccountId::from_str("alice.near").unwrap()
    }

    fn beneficiary() -> AccountId {
        AccountId::from_str("beneficiary.near").unwrap()
    }

    #[test]
    fn test_terminate_retroactively_adjusts_timestamp() {
        let mut builder = VMContextBuilder::new();
        builder.predecessor_account_id(alice());
        testing_env!(builder.build());

        let total_balance = 1_000_000;
        let lockup_schedule = Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC,
                balance: 0,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + 4 * ONE_YEAR_SEC,
                balance: total_balance,
            },
        ]);

        let vesting_schedule = Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: 0,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + 2 * ONE_YEAR_SEC,
                balance: total_balance,
            },
        ]);

        // At 1.5 years, user claims tokens.
        // Vested: 500_000 (1/2 of total, since vesting is 1 year linear)
        // Unlocked: 375_000 (1.5 / 4 years)
        // Claimable is min(500_000, 375_000) = 375_000
        let claimed_balance = 375_000;
        let mut lockup = Lockup {
            account_id: alice(),
            schedule: lockup_schedule.clone(),
            claimed_balance,
            termination_config: Some(TerminationConfig {
                beneficiary_id: beneficiary(),
                vesting_schedule: VestingConditions::Schedule(vesting_schedule.clone()),
            }),
        };

        let claim_timestamp = GENESIS_TIMESTAMP_SEC + 3 * ONE_YEAR_SEC / 2;
        assert_eq!(claimed_balance, lockup.schedule.unlocked_balance(claim_timestamp));

        // Now, terminate retroactively at 1 year.
        // At this point, vested_balance is 0 according to vesting_schedule.
        let termination_timestamp = GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC;

        // Since vested_balance (0) < claimed_balance (375_000), the logic should trigger.
        // The new vested_balance will be claimed_balance (375_000).
        // The new termination_timestamp will be calculated via `lockup_schedule.get_vesting_timestamp_for_amount(0)`,
        // which corresponds to GENESIS_TIMESTAMP_SEC.

        let (unvested_balance, beneficiary_id) = lockup.terminate(None, termination_timestamp);

        // unvested = total - new_vested = 1,000,000 - 375_000 = 625_000
        assert_eq!(unvested_balance, 625_000);
        assert_eq!(beneficiary_id, beneficiary());

        // Check the lockup state after termination
        // The schedule should be terminated with the new values.
        assert_eq!(lockup.schedule.total_balance(), 375_000);

        // The last checkpoint of the schedule should be at the *new* termination_timestamp.
        let final_timestamp = lockup.schedule.0.last().unwrap().timestamp;

        // The new timestamp is calculated with `get_vesting_timestamp_for_amount(0)` on the lockup schedule.
        // For a linear schedule, this should be the start of the schedule.
        let expected_new_timestamp = lockup_schedule.get_vesting_timestamp_for_amount(375_000);
        assert!(final_timestamp.abs_diff(expected_new_timestamp) <= 1);
        assert!(claim_timestamp > termination_timestamp);
    }

    #[test]
    fn test_terminate_no_claims() {
        let mut builder = VMContextBuilder::new();
        builder.predecessor_account_id(alice());
        testing_env!(builder.build());

        let total_balance = 15_000_000;
        let lockup_schedule = Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: 0,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + 4 * ONE_YEAR_SEC,
                balance: total_balance,
            },
        ]);

        let mut lockup = Lockup {
            account_id: alice(),
            schedule: lockup_schedule.clone(),
            claimed_balance: 0, // No claims
            termination_config: Some(TerminationConfig {
                beneficiary_id: beneficiary(),
                vesting_schedule: VestingConditions::SameAsLockupSchedule,
            }),
        };

        // Terminate at 1.5 years.
        let termination_timestamp = GENESIS_TIMESTAMP_SEC + 3 * ONE_YEAR_SEC / 2;

        // At this point, vested_balance is 2_500_000 (1/6 of total_balance, since vesting is 3 year linear from year 1 to 4).
        let vested_balance = lockup.schedule.unlocked_balance(termination_timestamp);
        assert_eq!(vested_balance, 2_500_000);

        let (unvested_balance, beneficiary_id) = lockup.terminate(None, termination_timestamp);

        // unvested = total - vested = 15,000,000 - 2,500,000 = 12,500,000
        assert_eq!(unvested_balance, 12_500_000);
        assert_eq!(beneficiary_id, beneficiary());

        // Check the lockup state after termination
        assert_eq!(lockup.schedule.total_balance(), vested_balance);
        let final_timestamp = lockup.schedule.0.last().unwrap().timestamp;
        assert_eq!(final_timestamp, termination_timestamp);
        assert_eq!(lockup.claimed_balance, 0);
    }

    #[test]
    fn test_terminate_retroactively_no_claims() {
        let mut builder = VMContextBuilder::new();
        builder.predecessor_account_id(alice());
        testing_env!(builder.build());

        let total_balance = 35_000_000;
        let lockup_schedule = Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: 0,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + 4 * ONE_YEAR_SEC,
                balance: total_balance,
            },
        ]);

        let mut lockup = Lockup {
            account_id: alice(),
            schedule: lockup_schedule.clone(),
            claimed_balance: 0, // No claims
            termination_config: Some(TerminationConfig {
                beneficiary_id: beneficiary(),
                vesting_schedule: VestingConditions::SameAsLockupSchedule,
            }),
        };

        // Terminate retroactively at 11 months.
        let termination_timestamp = GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 11 / 12;

        // At this point, vested_balance is 0 according to vesting_schedule.
        let vested_balance = lockup.schedule.unlocked_balance(termination_timestamp);
        assert_eq!(vested_balance, 0);

        // Since claimed_balance is 0, no special adjustment should happen.
        let (unvested_balance, beneficiary_id) = lockup.terminate(None, termination_timestamp);

        // unvested = total - vested = 35,000,000 - 0 = 35,000,000
        assert_eq!(unvested_balance, 35_000_000);
        assert_eq!(beneficiary_id, beneficiary());

        // Check the lockup state after termination
        assert_eq!(lockup.schedule.total_balance(), vested_balance);
        let final_timestamp = lockup.schedule.0.last().unwrap().timestamp;
        assert_eq!(final_timestamp, lockup.schedule.0.first().unwrap().timestamp + 1);
        assert_eq!(lockup.claimed_balance, 0);
    }

    #[test]
    pub fn test_vesting_timestamp_evaluation() {
        let mut builder = VMContextBuilder::new();
        builder.predecessor_account_id(alice());
        testing_env!(builder.build());

        let total_balance = 5_000_000_000_000_000_000_000_000;
        let lockup_schedule = Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: 0,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + 4 * ONE_YEAR_SEC,
                balance: total_balance,
            },
        ]);

        let lockup = Lockup {
            account_id: alice(),
            schedule: lockup_schedule.clone(),
            claimed_balance: 0,
            termination_config: Some(TerminationConfig {
                beneficiary_id: beneficiary(),
                vesting_schedule: VestingConditions::SameAsLockupSchedule,
            }),
        };

        let reference_timestamps = vec![
            GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC + 30 * 60 * 60,
            GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC + 70 * 30 * 60 * 60,
            GENESIS_TIMESTAMP_SEC + 2 * ONE_YEAR_SEC + 95 * 30 * 60 * 60 + 2,
            GENESIS_TIMESTAMP_SEC + 3 * ONE_YEAR_SEC + 30 * 60 + 7,
        ];

        for reference_timestamp in reference_timestamps {
            let reference_amount = lockup.schedule.unlocked_balance(reference_timestamp);
            assert_eq!(
                reference_timestamp,
                lockup.schedule.get_vesting_timestamp_for_amount(reference_amount)
            );
        }
    }
}

/// Amount of fungible tokens
pub type TokenAmount = u128;
trait FtTransferPromise {
    fn ft_transfer(self, receiver_id: &AccountId, amount: TokenAmount, memo: Option<String>) -> Promise;
}

impl FtTransferPromise for Promise {
    fn ft_transfer(self, receiver_id: &AccountId, amount: TokenAmount, memo: Option<String>) -> Promise {
        let args = serde_json::to_vec(&json!({
            "receiver_id": receiver_id,
            "amount": amount.to_string(),
            "memo": memo.unwrap_or_default(),
        }))
        .expect("Failed to serialize arguments");

        self.function_call(
            "ft_transfer".to_string(),
            args,
            NearToken::from_yoctonear(1),
            GAS_FOR_FT_TRANSFER,
        )
    }
}
