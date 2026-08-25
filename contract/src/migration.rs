use std::collections::{HashMap, HashSet};

use hodl_model::{
    lockup::{Lockup, LockupClaim, LockupCreate, LockupIndex},
    Balance, TokenAccountId, ONE_YEAR_SEC,
};
use near_plugins::{access_control_any, AccessControllable};
use near_sdk::{
    collections::{LookupMap, UnorderedMap, UnorderedSet, Vector},
    env::{self, panic_str},
    ext_contract, is_promise_success,
    json_types::U128,
    near, require,
    serde_json::{self, json},
    AccountId, Gas, NearToken, Promise, PromiseOrValue,
};

use crate::{
    event::{emit, EventKind, FtLockupUpdateContract},
    Contract, ContractExt, RoleAssignments, Roles,
};

/// Mirrors the on-chain Borsh layout of `Contract` before `migrate` runs
/// (12 fields, including `deposit_whitelist`/`draft_operators_whitelist`/
/// `manager`, and the drafts feature's `next_draft_id`/`drafts`/
/// `next_draft_group_id`/`draft_groups` — the drafts feature is being
/// dropped in this same migration, since no create-draft methods were ever
/// shipped); Borsh deserializes by position, so field order must match the
/// deployed struct exactly.
/// Borsh-layout-only stand-ins for the drafts feature's types, which have
/// been removed from `hodl-model` — only needed here so `OldContract` can
/// parse the deployed contract's old bytes; nothing reads their contents.
#[near(serializers = [borsh])]
pub(crate) struct OldDraft {
    pub draft_group_id: u32,
    pub lockup_create: LockupCreate,
}

#[near(serializers = [borsh])]
pub(crate) struct OldDraftGroup {
    pub total_amount: Balance,
    pub payer_id: Option<AccountId>,
    pub draft_indices: HashSet<u32>,
    pub discarded: bool,
}

#[near(serializers = [borsh])]
pub(crate) struct OldContract {
    pub token_account_id: TokenAccountId,
    pub lockups: Vector<Lockup>,
    pub account_lockups: LookupMap<AccountId, HashSet<LockupIndex>>,
    pub deposit_whitelist: UnorderedSet<AccountId>,
    pub draft_operators_whitelist: UnorderedSet<AccountId>,
    pub next_draft_id: u32,
    pub drafts: LookupMap<u32, OldDraft>,
    pub next_draft_group_id: u32,
    pub draft_groups: UnorderedMap<u32, OldDraftGroup>,
    pub manager: AccountId,
    pub orders: LookupMap<AccountId, Vec<LockupClaim>>,
    pub is_executing: bool,
}

#[near]
impl Contract {
    /// One-shot migration for the already-deployed contract: drops the
    /// `deposit_whitelist`/`draft_operators_whitelist`/`manager` fields and
    /// the drafts feature's fields from the Borsh layout, and bootstraps
    /// `AccessControllable` storage. `old.deposit_whitelist`/
    /// `old.draft_operators_whitelist`/`old.manager` are deliberately never
    /// read — whoever runs this decides explicitly who holds which role via
    /// `roles`. `old.next_draft_id`/`old.drafts`/`old.next_draft_group_id`/
    /// `old.draft_groups` are also dropped: no create-draft methods were
    /// ever shipped, so these were never populated.
    ///
    /// Takes no `&self`: near-sdk must not auto-deserialize the new struct
    /// shape from the old on-chain bytes; `env::state_read` parses them
    /// against `OldContract` instead. `#[init(ignore_state)]` is required —
    /// a bare `#[init]` refuses to run when state already exists, and
    /// near-sdk forbids returning `Self` from a plain method. A second
    /// invocation after success still reverts: the new state doesn't parse
    /// as `OldContract`, and `init_authority` rejects an already-bootstrapped
    /// ACL.
    ///
    /// Delete this method and `OldContract` once it has run against the
    /// live contract.
    #[private]
    #[init(ignore_state)]
    pub fn migrate(super_admin: AccountId, roles: RoleAssignments) -> Self {
        let old: OldContract = env::state_read().expect("Failed to read old state");

        let mut contract = Self {
            token_account_id: old.token_account_id,
            lockups: old.lockups,
            account_lockups: old.account_lockups,
            orders: old.orders,
            is_executing: old.is_executing,
        };

        emit(EventKind::FtLockupUpdateContract(FtLockupUpdateContract {}));
        contract.init_authority(super_admin, roles);

        contract
    }

    #[access_control_any(roles(Roles::DepositManager))]
    pub fn transfer_accounts(&mut self, contract_id: AccountId, account_ids: Vec<AccountId>) -> PromiseOrValue<()> {
        require!(
            !self.is_executing,
            "Cannot transfer accounts while orders are being executed"
        );
        self.is_executing = true;

        let mut total_amount = 0;
        let mut transfer_args = vec![];

        for account_id in account_ids.iter() {
            let lockup_indices = self
                .account_lockups
                .get(&account_id)
                .unwrap_or_else(|| panic_str(format!("Account {account_id} is not found!").as_str()));
            let orders: HashMap<u32, u128> = self
                .orders
                .remove(&account_id)
                .unwrap_or_default()
                .iter()
                .map(|order| (order.index, order.claim_amount.0))
                .collect();

            for index in lockup_indices {
                let lockup = self
                    .lockups
                    .get(index.into())
                    .unwrap_or_else(|| panic_str(format!("No lockup at index {index}!").as_str()));

                let issued_at = lockup
                    .schedule
                    .0
                    .first()
                    .unwrap_or_else(|| panic_str("Empty schedule"))
                    .timestamp
                    - ONE_YEAR_SEC;
                let total = lockup.schedule.total_balance();
                let claimed = lockup.claimed_balance.0 - orders.get(&index).map_or(0, |order_amount| *order_amount);

                transfer_args.push((account_id.clone(), issued_at, U128(total), U128(claimed)));
                total_amount += total - claimed;
            }
        }

        let msg = json!({
            "type": "migrate",
            "data": transfer_args,
        })
        .to_string();
        let args = json!({
            "receiver_id": contract_id,
            "amount": U128(total_amount),
            "msg": msg,
        });

        Promise::new(self.token_account_id.clone())
            .function_call(
                "ft_transfer_call".to_string(),
                serde_json::to_vec(&args).unwrap_or_else(|_| panic_str("Failed to serialize args")),
                NearToken::from_yoctonear(1),
                Gas::from_tgas(200),
            )
            .then(ext_self::ext(env::current_account_id()).on_migrated(account_ids))
            .into()
    }
}

#[ext_contract(ext_self)]
#[allow(dead_code)] // false positive
pub trait MigrationCallback {
    fn on_migrated(&mut self, account_ids: Vec<AccountId>);
}

#[near]
impl MigrationCallback for Contract {
    fn on_migrated(&mut self, account_ids: Vec<AccountId>) {
        self.is_executing = false;

        if !is_promise_success() {
            return;
        }

        for account_id in account_ids.iter() {
            let indices = self
                .account_lockups
                .remove(account_id)
                .unwrap_or_else(|| panic_str(format!("Account {account_id} is not found!").as_str()));

            for index in indices {
                self.lockups
                    .replace(index as u64, &Lockup::new_unlocked(account_id.clone(), 0));
            }
        }
    }
}
