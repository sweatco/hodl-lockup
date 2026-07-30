use std::{
    collections::{HashMap, HashSet},
    convert::Into,
    str::FromStr,
};

use hodl_model::{
    api::LockupApi,
    draft::{Draft, DraftGroup, DraftGroupIndex, DraftIndex},
    lockup::{Lockup, LockupClaim, LockupIndex},
    schedule::Schedule,
    termination::{TerminationConfig, VestingConditions},
    util::current_timestamp_sec,
    TimestampSec, TokenAccountId, WrappedBalance,
};
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::{
    assert_one_yocto,
    collections::{LookupMap, UnorderedMap, UnorderedSet, Vector},
    env::{self, panic_str},
    ext_contract, is_promise_success,
    json_types::{Base58CryptoHash, U128},
    log, near, near_bindgen, require,
    serde::Serialize,
    serde_json, AccountId, BorshStorageKey, Gas, NearToken, PanicOnDefault, Promise, PromiseOrValue,
};
use near_self_update_proc::SelfUpdate;

pub mod callbacks;
pub mod event;
pub mod ft_token_receiver;
pub mod internal;

mod issue;
mod migration;
mod order;
pub mod view;

use crate::{
    callbacks::{ext_self, SelfCallbacks},
    event::{
        emit, EventKind, FtLockupAddToDepositWhitelist, FtLockupAddToDraftOperatorsWhitelist, FtLockupClaimLockup,
        FtLockupCreateDraft, FtLockupCreateDraftGroup, FtLockupCreateLockup, FtLockupDeleteDraft,
        FtLockupDiscardDraftGroup, FtLockupFundDraftGroup, FtLockupNew, FtLockupRemoveFromDepositWhitelist,
        FtLockupRemoveFromDraftOperatorsWhitelist, FtLockupTerminateLockup, FtLockupUpdateOrder,
    },
    serde_json::json,
};

pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const GAS_FOR_FT_TRANSFER: Gas = Gas::from_gas(15_000_000_000_000);
const GAS_FOR_AFTER_FT_TRANSFER: Gas = Gas::from_gas(20_000_000_000_000);
const GAS_EXT_CALL_COST: Gas = Gas::from_gas(10_000_000_000_000);
const GAS_MIN_FOR_CONVERT: Gas = Gas::from_gas(15_000_000_000_000);

const DEFAULT_BENEFICIARY_ID: &str = "grants.sweat";

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

    pub orders: LookupMap<AccountId, Vec<LockupClaim>>,
    pub is_executing: bool,
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
    Orders,
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

#[near]
impl LockupApi for Contract {
    #[allow(clippy::wrong_self_convention)]
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
            orders: LookupMap::new(StorageKey::Orders),
            is_executing: false,
        }
    }

    fn claim(&mut self, amounts: Option<Vec<(LockupIndex, Option<WrappedBalance>)>>) -> Vec<LockupClaim> {
        require!(
            !self.is_executing,
            "Cannot place new order while other orders are being executed"
        );

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
                            (unlocked_balance - lockup.claimed_balance.0).into()
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
                    let amount: WrappedBalance = (unlocked_balance - lockup.claimed_balance.0).into();

                    (*lockup_id, amount)
                })
                .collect();
            (amounts, lockups_by_id)
        };

        if !self.orders.contains_key(&account_id) {
            self.orders.insert(&account_id, &vec![]);
        }

        let mut account_orders = self.orders.get(&account_id).unwrap();

        let mut orders_index = HashMap::<LockupIndex, usize>::new();
        for (index, order) in account_orders.iter().enumerate() {
            orders_index.insert(order.index, index);
        }

        let mut lockup_claims = vec![];
        for (lockup_index, lockup_claim_amount) in claim_amounts {
            let lockup = lockups_by_id.get_mut(&lockup_index).unwrap();
            let lockup_claim = lockup.claim(lockup_index, lockup_claim_amount.0);

            if lockup_claim.claim_amount.0 > 0 {
                self.lockups.replace(u64::from(lockup_index), lockup);
                lockup_claims.push(lockup_claim.clone());

                if let Some(i) = orders_index.get(&lockup_claim.index) {
                    let order = account_orders.get_mut(*i).expect("Order not found");
                    order.claim_amount.0 += lockup_claim.claim_amount.0;
                } else {
                    account_orders.push(lockup_claim);
                }
            }
        }
        self.orders.insert(&account_id, &account_orders);

        emit(EventKind::UpdateOrders(
            account_orders
                .iter()
                .map(|order| FtLockupUpdateOrder {
                    id: order.index,
                    amount: order.claim_amount.into(),
                })
                .collect(),
        ));

        lockup_claims
    }

    #[payable]
    fn terminate(
        &mut self,
        lockup_index: LockupIndex,
        _hashed_schedule: Option<Schedule>,
        termination_timestamp: Option<TimestampSec>,
    ) -> WrappedBalance {
        assert_one_yocto();
        self.assert_deposit_whitelist(&env::predecessor_account_id());

        let current_timestamp = current_timestamp_sec();
        let termination_timestamp = termination_timestamp.unwrap_or(current_timestamp);

        // 1. Find a Lockup
        let mut lockup = self.lockups.get(u64::from(lockup_index)).expect("Lockup not found");

        // 2. Find a corresponding Order
        let mut orders = self.orders.get(&lockup.account_id).unwrap_or_default();
        let order_position = orders.iter().position(|o| o.index == lockup_index);
        let mut original_claim_amount = 0;

        // 3. Revert claimed amount (lockup.claimed_amount -= order.amount)
        if let Some(position) = order_position {
            original_claim_amount = orders[position].claim_amount.0;
            lockup.claimed_balance.0 = lockup
                .claimed_balance
                .0
                .checked_sub(original_claim_amount)
                .expect("Order claim exceeds lockup claimed balance");
        }

        // 4. Terminate the Lockup
        let unvested_balance = lockup.terminate(termination_timestamp);

        // 5. Adjust the Order:
        //      - if lockup.claimed_amount + order.amount <= lockup.total_amount -> leave it as is
        //      - else -> order.amount = lockup.total_amount - lockup.claimed_amount
        if let Some(position) = order_position {
            let total_balance = lockup.schedule.total_balance();
            let adjusted_order_amount = if lockup.claimed_balance.0 + original_claim_amount > total_balance {
                total_balance.saturating_sub(lockup.claimed_balance.0)
            } else {
                original_claim_amount
            };

            let order = orders
                .get_mut(position)
                .unwrap_or_else(|| panic_str("Order is not found"));
            order.claim_amount = adjusted_order_amount.into();
            lockup.claimed_balance.0 = lockup
                .claimed_balance
                .0
                .checked_add(adjusted_order_amount)
                .expect("Adjusting order overflowed claimed balance");
            order.is_final = lockup.schedule.total_balance() == lockup.claimed_balance.0;

            self.orders.insert(&lockup.account_id, &orders);
        }

        // 6. Clean up
        self.lockups.replace(u64::from(lockup_index), &lockup);
        if lockup.schedule.total_balance() == 0 {
            self.remove_lockup(lockup_index);
        }

        // 7. Emit an Evnt
        let event = FtLockupTerminateLockup {
            id: lockup_index,
            termination_timestamp,
            unvested_balance: unvested_balance.into(),
        };
        emit(EventKind::FtLockupTerminateLockup(vec![event]));

        unvested_balance.into()
    }

    #[private]
    fn clear_accounts(&mut self, account_ids: Vec<AccountId>) -> U128 {
        self.assert_deposit_whitelist(&env::predecessor_account_id());
        require!(
            !self.is_executing,
            "Cannot clear accounts while orders are being executed"
        );

        let mut cleared_amount = 0u128;
        let mut cleared_accounts = HashSet::new();

        for account_id in account_ids {
            if !cleared_accounts.insert(account_id.clone()) {
                continue;
            }

            let indices = self
                .account_lockups
                .remove(&account_id)
                .unwrap_or_else(|| panic_str(format!("Account {account_id} is not found!").as_str()));

            self.orders.remove(&account_id);

            for index in indices {
                let lockup = self
                    .lockups
                    .get(u64::from(index))
                    .unwrap_or_else(|| panic_str(format!("No lockup at index {index}!").as_str()));
                cleared_amount = cleared_amount
                    .checked_add(lockup.schedule.total_balance() - lockup.claimed_balance.0)
                    .expect("Cleared lockups amount overflow");

                // Vector indices are stable and cannot be removed individually. Preserve the
                // claimed amount in a fully claimed tombstone after unlinking the account.
                // Its total and claimed balances match, so it can never be claimed again.
                let claimed_balance = lockup.claimed_balance.0;
                let mut cleared_lockup = Lockup::new_unlocked(account_id.clone(), claimed_balance);
                cleared_lockup.claimed_balance = claimed_balance.into();
                self.lockups.replace(u64::from(index), &cleared_lockup);
            }
        }

        cleared_amount.into()
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
}

impl Contract {
    pub(crate) fn remove_lockup(&mut self, lockup_index: u32) {
        let lockup = self
            .lockups
            .get(lockup_index as u64)
            .unwrap_or_else(|| panic_str("Cannot find lockup"));

        let mut indices = self.account_lockups.get(&lockup.account_id).unwrap_or_default();
        indices.remove(&lockup_index);
        self.internal_save_account_lockups(&lockup.account_id, indices);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    use hodl_model::{
        api::LockupViewApi,
        lockup::{Lockup, LockupClaim},
        schedule::{Checkpoint, Schedule},
        termination::{TerminationConfig, VestingConditions},
        TimestampSec,
    };
    use near_sdk::{test_utils::VMContextBuilder, testing_env, AccountId};

    const ONE_YEAR_SEC: u32 = 31_536_000;
    const GENESIS_TIMESTAMP_SEC: u32 = 0;

    fn alice() -> AccountId {
        AccountId::from_str("alice.near").unwrap()
    }

    fn beneficiary() -> AccountId {
        AccountId::from_str("beneficiary.near").unwrap()
    }

    fn manager() -> AccountId {
        AccountId::from_str("manager.near").unwrap()
    }

    fn contract_account() -> AccountId {
        AccountId::from_str("contract.near").unwrap()
    }

    fn token_account() -> AccountId {
        AccountId::from_str("token.near").unwrap()
    }

    fn to_nanos(timestamp_sec: TimestampSec) -> u64 {
        (timestamp_sec as u64) * 1_000_000_000
    }

    fn set_context(predecessor: &AccountId, attached_deposit: u128, timestamp_sec: TimestampSec) {
        let mut builder = VMContextBuilder::new();
        builder.current_account_id(contract_account());
        builder.predecessor_account_id(predecessor.clone());
        builder.signer_account_id(predecessor.clone());
        builder.attached_deposit(NearToken::from_yoctonear(attached_deposit));
        builder.account_balance(NearToken::from_yoctonear(10u128.pow(26)));
        builder.block_timestamp(to_nanos(timestamp_sec));
        testing_env!(builder.build());
    }

    fn sample_lockup(
        account_id: AccountId,
        total_balance: u128,
        claimed_balance: u128,
        finish_timestamp: TimestampSec,
    ) -> Lockup {
        let schedule = Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC,
                balance: 0.into(),
            },
            Checkpoint {
                timestamp: finish_timestamp,
                balance: total_balance.into(),
            },
        ]);

        Lockup {
            account_id,
            schedule,
            claimed_balance: claimed_balance.into(),
            termination_config: Some(TerminationConfig {
                beneficiary_id: beneficiary(),
                vesting_schedule: VestingConditions::SameAsLockupSchedule,
            }),
        }
    }

    #[test]
    fn test_clear_accounts_removes_lockups_and_returns_unclaimed_amount() {
        let manager = manager();
        set_context(&manager, 0, GENESIS_TIMESTAMP_SEC);

        let mut contract = Contract::new(token_account(), vec![manager.clone()], None, manager.clone());
        let alice = alice();
        let beneficiary = beneficiary();
        let alice_first = contract.internal_add_lockup(&sample_lockup(
            alice.clone(),
            1_000_000,
            250_000,
            GENESIS_TIMESTAMP_SEC + 10,
        ));
        let alice_second = contract.internal_add_lockup(&sample_lockup(
            alice.clone(),
            400_000,
            400_000,
            GENESIS_TIMESTAMP_SEC + 10,
        ));
        let beneficiary_lockup = contract.internal_add_lockup(&sample_lockup(
            beneficiary.clone(),
            600_000,
            100_000,
            GENESIS_TIMESTAMP_SEC + 10,
        ));
        contract.orders.insert(
            &alice,
            &vec![LockupClaim {
                index: alice_first,
                claim_amount: 250_000.into(),
                is_final: false,
            }],
        );

        let cleared = contract.clear_accounts(vec![alice.clone(), alice.clone()]);

        assert_eq!(cleared.0, 750_000);
        assert!(contract.account_lockups.get(&alice).is_none());
        assert!(contract.orders.get(&alice).is_none());
        assert_eq!(
            contract
                .lockups
                .get(u64::from(alice_first))
                .unwrap()
                .schedule
                .total_balance(),
            250_000
        );
        assert_eq!(
            contract.lockups.get(u64::from(alice_first)).unwrap().claimed_balance.0,
            250_000
        );
        assert_eq!(
            contract
                .lockups
                .get(u64::from(alice_second))
                .unwrap()
                .schedule
                .total_balance(),
            400_000
        );
        assert_eq!(
            contract.lockups.get(u64::from(alice_second)).unwrap().claimed_balance.0,
            400_000
        );
        assert_eq!(contract.get_total_unclaimed_amount().0, 500_000);
        assert!(contract
            .account_lockups
            .get(&beneficiary)
            .unwrap()
            .contains(&beneficiary_lockup));
    }

    #[test]
    fn test_terminate_without_orders_leaves_lockup_state() {
        let manager = manager();
        set_context(&manager, 0, GENESIS_TIMESTAMP_SEC);

        let mut contract = Contract::new(token_account(), vec![manager.clone()], None, manager.clone());

        let account = alice();
        let total_balance = 1_000_000;
        let finish_timestamp = GENESIS_TIMESTAMP_SEC + 10;
        let lockup_index =
            contract.internal_add_lockup(&sample_lockup(account.clone(), total_balance, 0, finish_timestamp));

        let termination_timestamp = GENESIS_TIMESTAMP_SEC + 5;
        set_context(&manager, 1, termination_timestamp);

        let unvested = contract.terminate(lockup_index, None, Some(termination_timestamp)).0;

        assert_eq!(unvested, 500_000);

        let stored_lockup = contract.lockups.get(u64::from(lockup_index)).unwrap();
        assert_eq!(stored_lockup.schedule.total_balance(), 500_000);
        assert_eq!(stored_lockup.claimed_balance.0, 0);
        assert!(contract.orders.get(&account).is_none());

        let indices = contract.account_lockups.get(&account).unwrap();
        assert!(indices.contains(&lockup_index));
    }

    #[test]
    fn test_terminate_preserves_order_when_within_total() {
        let manager = manager();
        set_context(&manager, 0, GENESIS_TIMESTAMP_SEC);

        let mut contract = Contract::new(token_account(), vec![manager.clone()], None, manager.clone());

        let account = alice();
        let total_balance = 1_000_000;
        let claim_amount = 200_000;
        let finish_timestamp = GENESIS_TIMESTAMP_SEC + 10;

        let lockup_index = contract.internal_add_lockup(&sample_lockup(
            account.clone(),
            total_balance,
            claim_amount,
            finish_timestamp,
        ));

        contract.orders.insert(
            &account,
            &vec![LockupClaim {
                index: lockup_index,
                claim_amount: claim_amount.into(),
                is_final: false,
            }],
        );

        let termination_timestamp = GENESIS_TIMESTAMP_SEC + 8;
        set_context(&manager, 1, termination_timestamp);

        let unvested = contract.terminate(lockup_index, None, Some(termination_timestamp)).0;

        assert_eq!(unvested, 200_000);

        let stored_orders = contract.orders.get(&account).unwrap();
        assert_eq!(stored_orders.len(), 1);
        assert_eq!(stored_orders[0].claim_amount.0, claim_amount);
        assert!(!stored_orders[0].is_final);

        let stored_lockup = contract.lockups.get(u64::from(lockup_index)).unwrap();
        assert_eq!(stored_lockup.schedule.total_balance(), 800_000);
    }

    #[test]
    fn test_terminate_trims_order_to_remaining_total() {
        let manager = manager();
        set_context(&manager, 0, GENESIS_TIMESTAMP_SEC);

        let mut contract = Contract::new(token_account(), vec![manager.clone()], None, manager.clone());

        let account = alice();
        let total_balance = 1_000_000;
        let claim_amount = 900_000;
        let finish_timestamp = GENESIS_TIMESTAMP_SEC + 10;

        let lockup_index = contract.internal_add_lockup(&sample_lockup(
            account.clone(),
            total_balance,
            claim_amount,
            finish_timestamp,
        ));

        contract.orders.insert(
            &account,
            &vec![LockupClaim {
                index: lockup_index,
                claim_amount: claim_amount.into(),
                is_final: false,
            }],
        );

        let termination_timestamp = GENESIS_TIMESTAMP_SEC + 6;
        set_context(&manager, 1, termination_timestamp);

        let unvested = contract.terminate(lockup_index, None, Some(termination_timestamp)).0;

        assert_eq!(unvested, 400_000);

        let stored_orders = contract.orders.get(&account).unwrap();
        assert_eq!(stored_orders.len(), 1);
        assert_eq!(stored_orders[0].claim_amount.0, 600_000);
        assert!(stored_orders[0].is_final);

        let stored_lockup = contract.lockups.get(u64::from(lockup_index)).unwrap();
        assert_eq!(stored_lockup.schedule.total_balance(), 600_000);
    }

    #[test]
    fn test_terminate_zeroes_order_and_removes_lockup_when_unvested() {
        let manager = manager();
        set_context(&manager, 0, GENESIS_TIMESTAMP_SEC);

        let mut contract = Contract::new(token_account(), vec![manager.clone()], None, manager.clone());

        let account = alice();
        let total_balance = 1_000_000;
        let claim_amount = 300_000;
        let finish_timestamp = GENESIS_TIMESTAMP_SEC + 10;

        let lockup_index = contract.internal_add_lockup(&sample_lockup(
            account.clone(),
            total_balance,
            claim_amount,
            finish_timestamp,
        ));

        contract.orders.insert(
            &account,
            &vec![LockupClaim {
                index: lockup_index,
                claim_amount: claim_amount.into(),
                is_final: false,
            }],
        );

        let termination_timestamp = GENESIS_TIMESTAMP_SEC;
        set_context(&manager, 1, termination_timestamp);

        let unvested = contract.terminate(lockup_index, None, Some(termination_timestamp)).0;

        assert_eq!(unvested, total_balance);

        let stored_orders = contract.orders.get(&account).unwrap();
        assert_eq!(stored_orders.len(), 1);
        assert_eq!(stored_orders[0].claim_amount.0, 0);
        assert!(stored_orders[0].is_final);

        assert!(contract.account_lockups.get(&account).is_none());
    }

    #[test]
    fn test_terminate_updates_only_matching_order() {
        let manager = manager();
        set_context(&manager, 0, GENESIS_TIMESTAMP_SEC);

        let mut contract = Contract::new(token_account(), vec![manager.clone()], None, manager.clone());

        let account = alice();
        let finish_timestamp = GENESIS_TIMESTAMP_SEC + 10;

        let primary_claim = 700_000;
        let secondary_claim = 300_000;

        let primary_index = contract.internal_add_lockup(&sample_lockup(
            account.clone(),
            1_000_000,
            primary_claim,
            finish_timestamp,
        ));

        let secondary_index = contract.internal_add_lockup(&sample_lockup(
            account.clone(),
            500_000,
            secondary_claim,
            finish_timestamp,
        ));

        contract.orders.insert(
            &account,
            &vec![
                LockupClaim {
                    index: primary_index,
                    claim_amount: primary_claim.into(),
                    is_final: false,
                },
                LockupClaim {
                    index: secondary_index,
                    claim_amount: secondary_claim.into(),
                    is_final: false,
                },
            ],
        );

        let termination_timestamp = GENESIS_TIMESTAMP_SEC + 4;
        set_context(&manager, 1, termination_timestamp);

        let unvested = contract.terminate(primary_index, None, Some(termination_timestamp)).0;

        assert_eq!(unvested, 600_000);

        let stored_orders = contract.orders.get(&account).unwrap();
        assert_eq!(stored_orders.len(), 2);

        let primary_order = stored_orders.iter().find(|o| o.index == primary_index).unwrap();
        assert_eq!(primary_order.claim_amount.0, 400_000);
        assert!(primary_order.is_final);

        let secondary_order = stored_orders.iter().find(|o| o.index == secondary_index).unwrap();
        assert_eq!(secondary_order.claim_amount.0, secondary_claim);
        assert!(!secondary_order.is_final);

        let stored_primary = contract.lockups.get(u64::from(primary_index)).unwrap();
        assert_eq!(stored_primary.schedule.total_balance(), 400_000);

        let stored_secondary = contract.lockups.get(u64::from(secondary_index)).unwrap();
        assert_eq!(stored_secondary.schedule.total_balance(), 500_000);
    }

    #[test]
    fn test_terminate_subsequent_lockup_does_not_double_existing_order() {
        let manager = manager();
        set_context(&manager, 0, GENESIS_TIMESTAMP_SEC);

        let mut contract = Contract::new(token_account(), vec![manager.clone()], None, manager.clone());

        let account = alice();
        let finish_timestamp = GENESIS_TIMESTAMP_SEC + 10;

        let first_index = contract.internal_add_lockup(&sample_lockup(account.clone(), 1_000_000, 0, finish_timestamp));

        set_context(&account, 0, GENESIS_TIMESTAMP_SEC + 2);
        contract.claim(Some(vec![(first_index, Some(900_000u128.into()))]));

        let orders = contract.orders.get(&account).unwrap();
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].claim_amount.0, 900_000);
        assert!(!orders[0].is_final);

        set_context(&manager, 1, GENESIS_TIMESTAMP_SEC + 6);
        contract.terminate(first_index, None, Some(GENESIS_TIMESTAMP_SEC + 6));

        let orders = contract.orders.get(&account).unwrap();
        let first_order = orders.iter().find(|o| o.index == first_index).unwrap();
        assert_eq!(first_order.claim_amount.0, 600_000);
        assert!(first_order.is_final);

        let second_index = contract.internal_add_lockup(&sample_lockup(account.clone(), 200_000, 0, finish_timestamp));

        set_context(&account, 0, GENESIS_TIMESTAMP_SEC + 7);
        contract.claim(Some(vec![(second_index, Some(50_000u128.into()))]));

        let orders = contract.orders.get(&account).unwrap();
        assert_eq!(orders.len(), 2);
        let first_order = orders.iter().find(|o| o.index == first_index).unwrap();
        assert_eq!(first_order.claim_amount.0, 600_000);
        assert!(first_order.is_final);
        let second_order = orders.iter().find(|o| o.index == second_index).unwrap();
        assert_eq!(second_order.claim_amount.0, 50_000);
        assert!(!second_order.is_final);

        set_context(&manager, 1, GENESIS_TIMESTAMP_SEC + 9);
        contract.terminate(second_index, None, Some(GENESIS_TIMESTAMP_SEC + 9));

        let orders = contract.orders.get(&account).unwrap();
        let first_order = orders.iter().find(|o| o.index == first_index).unwrap();
        assert_eq!(first_order.claim_amount.0, 600_000);
        assert!(first_order.is_final);
        let second_order = orders.iter().find(|o| o.index == second_index).unwrap();
        assert_eq!(second_order.claim_amount.0, 50_000);
        assert!(!second_order.is_final);
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
                balance: 0.into(),
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + 4 * ONE_YEAR_SEC,
                balance: total_balance.into(),
            },
        ]);

        let vesting_schedule = Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: 0.into(),
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + 2 * ONE_YEAR_SEC,
                balance: total_balance.into(),
            },
        ]);

        // At 1.5 years, user claims tokens.
        // Vested: 500_000 (1/2 of total, since vesting is 1 year linear)
        // Unlocked: 375_000 (1.5 / 4 years)
        // Claimable is min(500_000, 375_000) = 375_000
        let claimed_balance = 375_000.into();
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
        assert_eq!(
            claimed_balance,
            lockup.schedule.unlocked_balance(claim_timestamp).into()
        );

        // Now, terminate retroactively at 1 year.
        // At this point, vested_balance is 0 according to vesting_schedule.
        let termination_timestamp = GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC;

        // Since vested_balance (0) < claimed_balance (375_000), the logic should trigger.
        // The new vested_balance will be claimed_balance (375_000).
        // The new termination_timestamp will be calculated via `lockup_schedule.get_vesting_timestamp_for_amount(0)`,
        // which corresponds to GENESIS_TIMESTAMP_SEC.

        let unvested_balance = lockup.terminate(termination_timestamp);

        // unvested = total - new_vested = 1,000,000 - 375_000 = 625_000
        assert_eq!(unvested_balance, 625_000);

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
                balance: 0.into(),
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + 4 * ONE_YEAR_SEC,
                balance: total_balance.into(),
            },
        ]);

        let mut lockup = Lockup {
            account_id: alice(),
            schedule: lockup_schedule.clone(),
            claimed_balance: 0.into(), // No claims
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

        let unvested_balance = lockup.terminate(termination_timestamp);

        // unvested = total - vested = 15,000,000 - 2,500,000 = 12,500,000
        assert_eq!(unvested_balance, 12_500_000);

        // Check the lockup state after termination
        assert_eq!(lockup.schedule.total_balance(), vested_balance);
        let final_timestamp = lockup.schedule.0.last().unwrap().timestamp;
        assert_eq!(final_timestamp, termination_timestamp);
        assert_eq!(lockup.claimed_balance.0, 0);
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
                balance: 0.into(),
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + 4 * ONE_YEAR_SEC,
                balance: total_balance.into(),
            },
        ]);

        let mut lockup = Lockup {
            account_id: alice(),
            schedule: lockup_schedule.clone(),
            claimed_balance: 0.into(), // No claims
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
        let unvested_balance = lockup.terminate(termination_timestamp);

        // unvested = total - vested = 35,000,000 - 0 = 35,000,000
        assert_eq!(unvested_balance, 35_000_000);

        // Check the lockup state after termination
        assert_eq!(lockup.schedule.total_balance(), vested_balance);
        let final_timestamp = lockup.schedule.0.last().unwrap().timestamp;
        assert_eq!(final_timestamp, lockup.schedule.0.first().unwrap().timestamp + 1);
        assert_eq!(lockup.claimed_balance.0, 0);
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
                balance: 0.into(),
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + 4 * ONE_YEAR_SEC,
                balance: total_balance.into(),
            },
        ]);

        let lockup = Lockup {
            account_id: alice(),
            schedule: lockup_schedule.clone(),
            claimed_balance: 0.into(),
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
