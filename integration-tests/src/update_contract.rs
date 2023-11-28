#![cfg(test)]

use std::{env, fs, str::FromStr};

use anyhow::Result;
use base58::ToBase58;
use ed25519_dalek::SigningKey;
use integration_utils::integration_contract::IntegrationContract;
use multisig_integration::Multisig;
use multisig_model::{
    api::{MultisigApiIntegration, MultisigViewIntegration},
    data::{FunctionCallPermission, MultiSigRequest, MultiSigRequestAction},
};
use near_sdk::{borsh::BorshDeserialize, PublicKey};
use near_workspaces::{Account, Contract};
use rand::rngs::OsRng;

use crate::{
    context::{prepare_contract, Context, IntegrationContext},
    lockup_interface::GetContractAccount,
};

#[tokio::test]
async fn update_contract() -> anyhow::Result<()> {
    println!("👷🏽 Run update contract test");

    let context = prepare_contract().await?;

    assert_eq!(0, context.multisig().get_num_confirmations().await?);

    let signers = [new_pk()?, new_pk()?, new_pk()?];

    add_keys(context.multisig(), &signers).await?;

    context
        .multisig()
        .add_request_and_confirm(MultiSigRequest {
            receiver_id: context.multisig().contract_account(),
            actions: vec![MultiSigRequestAction::SetNumConfirmations { num_confirmations: 2 }],
        })
        .await?;

    assert_eq!(2, context.multisig().get_num_confirmations().await?);

    let wasm = load_wasm("../res/hodl_lockup.wasm")?;

    dbg!(context.multisig().contract_account());

    let update_request = context
        .multisig()
        .add_request_and_confirm(MultiSigRequest {
            receiver_id: context.lockup().contract_account(),
            actions: vec![MultiSigRequestAction::DeployContract { code: wasm.into() }],
        })
        .await?;

    let confirmations = context.multisig().get_confirmations(update_request).await?;

    assert_eq!(1, confirmations.len());

    dbg!(&confirmations);

    Ok(())
}

async fn add_keys(mut multisig: Multisig<'_>, signers: &[PublicKey]) -> Result<()> {
    let multisig_method_names = ["add_request", "add_request_and_confirm", "delete_request", "confirm"];

    let multisig_keys_actions: Vec<_> = signers
        .into_iter()
        .map(|key| MultiSigRequestAction::AddKey {
            public_key: key.clone(),
            permission: FunctionCallPermission {
                allowance: None,
                receiver_id: multisig.contract_account(),
                method_names: multisig_method_names.into_iter().map(Into::into).collect(),
            }
            .into(),
        })
        .collect();

    multisig
        .add_request_and_confirm(MultiSigRequest {
            receiver_id: multisig.contract_account(),
            actions: multisig_keys_actions,
        })
        .await?;

    Ok(())
}

fn new_pk() -> Result<PublicKey> {
    let signing_key = SigningKey::generate(&mut OsRng);
    Ok(PublicKey::from_str(
        &signing_key.verifying_key().as_bytes().to_vec().to_base58(),
    )?)
}

fn load_wasm(wasm_path: &str) -> Result<Vec<u8>> {
    let current_dir = env::current_dir()?;
    let wasm_filepath = fs::canonicalize(current_dir.join(wasm_path))?;
    let data = fs::read(wasm_filepath)?;
    Ok(data)
}
