#![cfg(test)]

use anyhow::Result;
use integration_utils::misc::ToNear;
use model::update::UpdateApiIntegration;

use crate::{
    context::{prepare_contract, IntegrationContext},
    utils::load_wasm,
};

#[tokio::test]
async fn simple_update() -> Result<()> {
    let mut context = prepare_contract().await?;

    let bob = context.bob().await?;
    let alice = context.alice().await?;

    context.lockup().set_multisig(alice.to_near()).call().await?;

    let wasm = load_wasm("../res/hodl_lockup.wasm")?;

    context
        .lockup()
        .update_contract(wasm.clone())
        .with_user(&alice)
        .call()
        .await?;

    let Err(error) = context.lockup().update_contract(wasm).with_user(&bob).call().await else {
        panic!("Update with non multisig account should fail");
    };

    assert!(error
        .to_string()
        .contains("Only multisig account can update the contract"));

    Ok(())
}
