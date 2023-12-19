use std::collections::HashSet;

use near_sdk::{
    borsh,
    borsh::{BorshDeserialize, BorshSerialize},
    collections::{LookupMap, UnorderedMap, UnorderedSet, Vector},
    near_bindgen, AccountId,
};

use crate::{
    draft::{Draft, DraftGroup, DraftGroupIndex, DraftIndex},
    lockup::{Lockup, LockupIndex},
    TokenAccountId,
};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct OldState {
    pub token_account_id: TokenAccountId,

    pub lockups: Vector<Lockup>,

    pub account_lockups: LookupMap<AccountId, HashSet<LockupIndex>>,

    /// Account IDs that can create new lockups.
    pub deposit_whitelist: UnorderedSet<AccountId>,

    pub next_draft_id: DraftIndex,
    pub drafts: LookupMap<DraftIndex, Draft>,
    pub next_draft_group_id: DraftGroupIndex,
    pub draft_groups: UnorderedMap<DraftGroupIndex, DraftGroup>,
}
