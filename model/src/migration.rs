use std::collections::HashSet;

use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::{LookupMap, UnorderedMap, UnorderedSet, Vector},
    near_bindgen,
    serde::Deserialize,
    AccountId, Balance, PanicOnDefault,
};

use crate::{
    draft::{Draft, DraftGroup, DraftGroupIndex, DraftIndex},
    lockup::LockupIndex,
    schedule::Schedule,
    termination::TerminationConfig,
    util::u128_dec_format,
    TokenAccountId,
};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct ContractDeprecated {
    pub token_account_id: TokenAccountId,

    pub lockups: Vector<LockupDeprecated>,

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

    pub multisig: Option<AccountId>,
}

#[derive(BorshDeserialize, BorshSerialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct LockupDeprecated {
    pub account_id: AccountId,
    pub schedule: Schedule,

    #[serde(default)]
    #[serde(with = "u128_dec_format")]
    pub claimed_balance: Balance,
    /// An optional configuration that allows vesting/lockup termination.
    pub termination_config: Option<TerminationConfig>,
}
