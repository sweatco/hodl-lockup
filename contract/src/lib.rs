use std::{
    collections::{HashMap, HashSet},
    convert::Into,
};

use hodl_model::{
    api::LockupApi,
    lockup::{Lockup, LockupClaim, LockupIndex},
    schedule::Schedule,
    util::current_timestamp_sec,
    TimestampSec, TokenAccountId, WrappedBalance,
};
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_plugins::{access_control, access_control_any, AccessControlRole, AccessControllable, Upgradable};
use near_sdk::{
    assert_one_yocto,
    borsh::BorshDeserialize,
    collections::{LookupMap, Vector},
    env::{self, panic_str},
    ext_contract, is_promise_success,
    json_types::U128,
    log, near, near_bindgen, require,
    serde::Serialize,
    serde_json, AccountId, BorshStorageKey, Gas, NearToken, PanicOnDefault, Promise, PromiseOrValue,
};
use strum::EnumIter;

pub mod callbacks;
pub mod event;
pub mod ft_token_receiver;
pub mod internal;

mod issue;
mod migration;
mod order;
pub mod view;

use crate::{
    event::{emit, EventKind, FtLockupClaimLockup, FtLockupCreateLockup, FtLockupNew, FtLockupTerminateLockup, FtLockupUpdateOrder},
    serde_json::json,
};

pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const GAS_FOR_FT_TRANSFER: Gas = Gas::from_gas(15_000_000_000_000);

/// Roles for `near_plugins`' `AccessControllable`. `StagingManager`/`UpgradeManager`
/// are deliberately separate from `DepositManager`: code-deployment is a more
/// dangerous capability than the operational roles and must not come bundled.
///
/// Must live in the same module as the `#[access_control]`-annotated struct:
/// the `AccessControlRole` derive generates a private `RoleFlags` type that
/// the `access_control` expansion refers to by name.
#[near(serializers = [json])]
#[derive(AccessControlRole, Copy, Clone, Debug, PartialEq, Eq, Hash, EnumIter)]
pub enum Roles {
    /// Old `deposit_whitelist`: full admin — issue lockups, terminate,
    /// execute/reset orders, transfer_accounts.
    DepositManager,
    /// `Upgradable::up_stage_code`.
    StagingManager,
    /// `Upgradable::up_deploy_code` + staging-duration management.
    UpgradeManager,
}

pub use hodl_model::api::RoleAssignments;

#[near(contract_state)]
#[derive(PanicOnDefault, Upgradable)]
#[access_control(role_type(Roles))]
#[upgradable(access_control_roles(
    code_stagers(Roles::StagingManager),
    code_deployers(Roles::UpgradeManager),
    duration_initializers(Roles::UpgradeManager),
    duration_update_stagers(Roles::UpgradeManager),
    duration_update_appliers(Roles::UpgradeManager),
))]
pub struct Contract {
    pub token_account_id: TokenAccountId,

    pub lockups: Vector<Lockup>,

    pub account_lockups: LookupMap<AccountId, HashSet<LockupIndex>>,

    pub orders: LookupMap<AccountId, Vec<LockupClaim>>,
    pub is_executing: bool,
}

#[near(serializers=[borsh, json])]
#[derive(BorshStorageKey)]
pub(crate) enum StorageKey {
    Lockups,
    AccountLockups,
    Orders,
}

impl Contract {
    /// One-shot ACL bootstrap for `new`/`migrate`: the predecessor (the
    /// contract's own account, forced by `#[private]`) becomes a temporary
    /// super-admin so it can perform the grants — a fresh ACL has no admins,
    /// and `acl_grant_role`/`acl_transfer_super_admin` authorize by
    /// predecessor — then hands super-admin off to `super_admin`, retaining
    /// no power itself. Every step is `require!`d: a silent ACL failure must
    /// abort the whole transaction, never complete init with a
    /// misconfigured ACL.
    pub(crate) fn init_authority(&mut self, super_admin: AccountId, roles: RoleAssignments) {
        require!(
            self.acl_init_super_admin(env::predecessor_account_id()),
            "ACL bootstrap failed: super admin is already initialized"
        );
        self.grant_role_assignments(roles);
        require!(
            self.acl_transfer_super_admin(super_admin).is_some(),
            "ACL bootstrap failed: could not transfer super admin"
        );
    }

    pub(crate) fn grant_role_assignments(&mut self, roles: RoleAssignments) {
        for (role, account_ids) in roles {
            for account_id in account_ids {
                require!(
                    self.acl_grant_role(role.clone(), account_id).is_some(),
                    "ACL bootstrap failed: could not grant role"
                );
            }
        }
    }
}

#[near]
impl LockupApi for Contract {
    #[allow(clippy::wrong_self_convention)]
    #[init]
    #[private]
    fn new(token_account_id: AccountId, super_admin_account_id: AccountId, roles: RoleAssignments) -> Self {
        let mut contract = Self {
            lockups: Vector::new(StorageKey::Lockups),
            account_lockups: LookupMap::new(StorageKey::AccountLockups),
            token_account_id: token_account_id.clone(),
            orders: LookupMap::new(StorageKey::Orders),
            is_executing: false,
        };

        emit(EventKind::FtLockupNew(FtLockupNew { token_account_id }));

        contract.init_authority(super_admin_account_id, roles);

        contract
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

    #[access_control_any(roles(Roles::DepositManager))]
    #[payable]
    fn terminate(
        &mut self,
        lockup_index: LockupIndex,
        _hashed_schedule: Option<Schedule>,
        termination_timestamp: Option<TimestampSec>,
    ) -> WrappedBalance {
        assert_one_yocto();

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
    fn test_terminate_without_orders_leaves_lockup_state() {
        let manager = manager();
        set_context(&contract_account(), 0, GENESIS_TIMESTAMP_SEC);

        let mut contract = Contract::new(
            token_account(),
            manager.clone(),
            vec![(Roles::DepositManager.into(), vec![manager.clone()])],
        );

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
        set_context(&contract_account(), 0, GENESIS_TIMESTAMP_SEC);

        let mut contract = Contract::new(
            token_account(),
            manager.clone(),
            vec![(Roles::DepositManager.into(), vec![manager.clone()])],
        );

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
        set_context(&contract_account(), 0, GENESIS_TIMESTAMP_SEC);

        let mut contract = Contract::new(
            token_account(),
            manager.clone(),
            vec![(Roles::DepositManager.into(), vec![manager.clone()])],
        );

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
        set_context(&contract_account(), 0, GENESIS_TIMESTAMP_SEC);

        let mut contract = Contract::new(
            token_account(),
            manager.clone(),
            vec![(Roles::DepositManager.into(), vec![manager.clone()])],
        );

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
        set_context(&contract_account(), 0, GENESIS_TIMESTAMP_SEC);

        let mut contract = Contract::new(
            token_account(),
            manager.clone(),
            vec![(Roles::DepositManager.into(), vec![manager.clone()])],
        );

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
        set_context(&contract_account(), 0, GENESIS_TIMESTAMP_SEC);

        let mut contract = Contract::new(
            token_account(),
            manager.clone(),
            vec![(Roles::DepositManager.into(), vec![manager.clone()])],
        );

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
