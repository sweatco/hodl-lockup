#![cfg(test)]

use anyhow::Result;
use integration_utils::{integration_contract::IntegrationContract, misc::ToNear};
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

    context.lockup().set_multisig(alice.to_near()).await?;

    let wasm = load_wasm("../res/hodl_lockup.wasm")?;

    context.lockup().with_user(&alice).update_contract(wasm.clone()).await?;

    let Err(error) = context.lockup().with_user(&bob).update_contract(wasm).await else {
        panic!("Update with non multisig account should fail");
    };

    assert!(error
        .to_string()
        .contains("Only multisig account can update the contract"));

    Ok(())
}
