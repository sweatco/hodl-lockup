use near_sdk::{env, Timestamp};

use crate::TimestampSec;

pub fn nano_to_sec(timestamp: Timestamp) -> TimestampSec {
    (timestamp / 10u64.pow(9)).try_into().unwrap()
}

pub fn current_timestamp_sec() -> TimestampSec {
    nano_to_sec(env::block_timestamp())
}

pub mod u128_dec_format {
    use near_sdk::serde::{de, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(num: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&num.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?.parse().map_err(de::Error::custom)
    }
}
