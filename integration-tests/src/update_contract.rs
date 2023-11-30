#![cfg(test)]

use std::{env, fs, str::FromStr};

use anyhow::Result;
use integration_utils::integration_contract::IntegrationContract;
use model::update::UpdateApiIntegration;
use multisig_integration::Multisig;
use multisig_model::{
    api::{MultisigApiIntegration, MultisigViewIntegration},
    data::{FunctionCallPermission, MultiSigRequest, MultiSigRequestAction},
};
use near_sdk::json_types::U128;
use near_workspaces::{
    types::{AccountDetails, Gas, KeyType, SecretKey},
    Account, AccountId,
};

use crate::{
    context::{prepare_contract, Context, IntegrationContext},
    lockup_interface::GetContractAccount,
};

#[tokio::test]
async fn update_contract() -> Result<()> {
    println!("👷🏽 Run update contract test");

    let mut context = prepare_contract().await?;

    let lockup_account_id = context.lockup().contract_account();
    let multisig_account_id = context.multisig().contract_account();

    dbg!(&multisig_account_id);
    dbg!(&lockup_account_id);

    context.lockup().set_multisig(multisig_account_id.clone()).await?;

    dbg!(context.lockup().contract().view_access_keys().await?);

    assert_eq!(0, context.multisig().get_num_confirmations().await?);

    let signers = [
        SecretKey::from_random(KeyType::ED25519),
        SecretKey::from_random(KeyType::ED25519),
        SecretKey::from_random(KeyType::ED25519),
    ];

    let signers_accounts: Vec<_> = signers
        .iter()
        .map(|key| {
            Account::from_secret_key(
                AccountId::from_str(&multisig_account_id.to_string()).unwrap(),
                key.clone(),
                &mut context.worker,
            )
        })
        .collect();

    dbg!(&signers_accounts);

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

    update_with_multisig(&mut context, &signers_accounts, &wasm).await?;

    let update_request = context
        .multisig()
        .with_user(&signers_accounts[0])
        .add_request_and_confirm(MultiSigRequest {
            receiver_id: context.lockup().contract_account(),
            actions: vec![MultiSigRequestAction::FunctionCall {
                method_name: "update_contract_kok".to_string(),
                args: vec![].into(), //wasm.into(),
                deposit: U128(10000000000000000000000000000000000),
                gas: Gas::from_tgas(300).as_gas().into(),
            }],
        })
        .await?;

    let confirmations = context.multisig().get_confirmations(update_request).await?;

    assert_eq!(1, confirmations.len());

    let account_view: AccountDetails = signers_accounts[1].view_account().await?;

    dbg!(&account_view);

    let confirmed = context
        .multisig()
        .with_user(&signers_accounts[1])
        .confirm(update_request)
        .await?;

    dbg!(confirmed);

    let confirmations = context.multisig().get_confirmations(update_request).await?;

    assert_eq!(2, confirmations.len());

    Ok(())
}

async fn update_with_multisig(context: &mut Context, signers_accounts: &[Account], wasm: &[u8]) -> Result<()> {
    let update_request = context
        .multisig()
        .with_user(&signers_accounts[0])
        .add_request_and_confirm(MultiSigRequest {
            receiver_id: context.lockup().contract_account(),
            actions: vec![MultiSigRequestAction::DeployContract {
                code: wasm.to_vec().into(),
            }],
        })
        .await?;

    let confirmed = context
        .multisig()
        .with_user(&signers_accounts[1])
        .confirm(update_request)
        .await?;

    dbg!(&confirmed);

    Ok(())
}

async fn add_keys(mut multisig: Multisig<'_>, signers: &[SecretKey]) -> Result<()> {
    let multisig_keys_actions: Vec<_> = signers
        .into_iter()
        .map(|key| MultiSigRequestAction::AddKey {
            public_key: to_near_pk(key.public_key()),
            permission: FunctionCallPermission {
                allowance: U128(18376705513618804849920000).into(),
                receiver_id: multisig.contract_account(),
                method_names: ["add_request", "add_request_and_confirm", "delete_request", "confirm"]
                    .into_iter()
                    .map(Into::into)
                    .collect(),
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

fn load_wasm(wasm_path: &str) -> Result<Vec<u8>> {
    let current_dir = env::current_dir()?;
    let wasm_filepath = fs::canonicalize(current_dir.join(wasm_path))?;
    let data = fs::read(wasm_filepath)?;
    Ok(data)
}

fn to_near_pk(key: near_workspaces::types::PublicKey) -> near_sdk::PublicKey {
    near_sdk::PublicKey::from_str(&key.to_string()).unwrap()
}
