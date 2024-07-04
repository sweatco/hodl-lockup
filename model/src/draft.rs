use std::collections::HashSet;

use near_sdk::{
    env, near,
    serde::{Deserialize, Serialize},
    AccountId,
};

use crate::{
    lockup::{LockupCreate, LockupCreateView},
    util::u128_dec_format,
    Balance,
};

pub type DraftGroupIndex = u32;
pub type DraftIndex = u32;

#[near(serializers=[borsh, json])]
#[derive(Debug, PartialEq, Clone)]
pub struct Draft {
    pub draft_group_id: DraftGroupIndex,
    pub lockup_create: LockupCreate,
}

impl Draft {
    pub fn total_balance(&self) -> Balance {
        self.lockup_create.schedule.total_balance()
    }

    pub fn assert_new_valid(&self) {
        let amount = self.lockup_create.schedule.total_balance();
        // any valid near account id will work fine here as a parameter
        self.lockup_create
            .into_lockup(&env::predecessor_account_id())
            .assert_new_valid(amount);
    }
}

#[near(serializers=[borsh, json])]
#[derive(Default)]
pub struct DraftGroup {
    pub total_amount: Balance,
    pub payer_id: Option<AccountId>,
    pub draft_indices: HashSet<DraftIndex>,
    pub discarded: bool,
}

impl DraftGroup {
    pub fn assert_can_add_draft(&self) {
        assert!(!self.discarded, "cannot add draft, draft group is discarded");
        assert!(self.payer_id.is_none(), "cannot add draft, group already funded");
    }

    pub fn assert_can_convert_draft(&self) {
        assert!(!self.discarded, "cannot convert draft, draft group is discarded");
        assert!(self.payer_id.is_some(), "cannot convert draft from not funded group");
    }

    pub fn assert_can_fund(&self) {
        assert!(!self.discarded, "cannot fund draft, draft group is discarded");
        assert!(self.payer_id.is_none(), "draft group already funded");
    }

    pub fn fund(&mut self, payer_id: &AccountId) {
        self.assert_can_fund();
        self.payer_id = Some(payer_id.clone());
    }

    pub fn assert_can_discard(&mut self) {
        assert!(!self.discarded, "cannot discard, draft group already discarded");
        assert!(self.payer_id.is_none(), "cannot discard, draft group already funded");
    }

    pub fn discard(&mut self) {
        self.assert_can_discard();
        self.discarded = true;
    }

    pub fn assert_can_delete_draft(&mut self) {
        assert!(self.discarded, "cannot delete draft, draft group is not discarded");
        assert!(
            self.payer_id.is_none(),
            "cannot delete draft, draft group already funded"
        );
    }
}

#[near(serializers=[borsh, json])]
pub struct DraftGroupView {
    #[serde(with = "u128_dec_format")]
    pub total_amount: Balance,
    pub payer_id: Option<AccountId>,
    pub draft_indices: Vec<DraftIndex>,
    pub discarded: bool,
    pub funded: bool,
}

impl From<DraftGroup> for DraftGroupView {
    fn from(draft_group: DraftGroup) -> Self {
        Self {
            total_amount: draft_group.total_amount,
            payer_id: draft_group.payer_id.clone(),
            draft_indices: draft_group.draft_indices.into_iter().collect(),
            discarded: draft_group.discarded,
            funded: draft_group.payer_id.is_some(),
        }
    }
}

#[derive(Serialize, Debug, PartialEq, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct DraftView {
    pub draft_group_id: DraftGroupIndex,
    pub lockup_create: LockupCreateView,
}

impl From<Draft> for DraftView {
    fn from(draft: Draft) -> Self {
        Self {
            draft_group_id: draft.draft_group_id,
            lockup_create: draft.lockup_create.into(),
        }
    }
}
