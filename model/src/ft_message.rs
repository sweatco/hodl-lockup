use near_sdk::serde::{Deserialize, Serialize};

use crate::{draft::DraftGroupIndex, lockup::LockupCreate};

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct DraftGroupFunding {
    pub draft_group_id: DraftGroupIndex,
    // use remaining gas to try converting drafts
    pub try_convert: Option<bool>,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[serde(untagged)]
pub enum FtMessage {
    LockupCreate(LockupCreate),
    DraftGroupFunding(DraftGroupFunding),
}
