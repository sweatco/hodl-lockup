#![cfg(test)]

use anyhow::Result;
use helper_contract::api::HelperApiIntegration;

use crate::context::{prepare_contract, IntegrationContext};

#[tokio::test]
async fn helper_contract() -> Result<()> {
    let context = prepare_contract().await?;

    dbg!(context.helper().block_timestamp_ms().call().await?);

    Ok(())
}
