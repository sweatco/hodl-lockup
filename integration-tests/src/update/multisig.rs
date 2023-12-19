#![cfg(test)]

use std::str::FromStr;

use anyhow::Result;
use model::{update::UpdateApiIntegration, view_api::LockupViewApiIntegration};
use multisig_integration::Multisig;
use multisig_model::{
    api::{MultisigApiIntegration, MultisigViewIntegration},
    data::{FunctionCallPermission, MultiSigRequest, MultiSigRequestAction},
};
use near_sdk::json_types::U128;
use near_workspaces::{
    types::{Gas, KeyType, NearToken, SecretKey},
    Account, AccountId,
};

use crate::{
    context::{prepare_contract, Context, IntegrationContext},
    lockup_interface::GetContractAccount,
    utils::{load_wasm, AccountExtension},
};

#[tokio::test]
async fn update_contract() -> Result<()> {
    println!("👷🏽 Run update contract test");

    let mut context = prepare_contract().await?;

    assert_eq!("1.0.0", context.lockup().get_version().call().await?);

    let multisig_account_id = context.multisig().contract_account();

    context
        .lockup()
        .set_multisig(multisig_account_id.clone())
        .call()
        .await?;

    assert_eq!(0, context.multisig().get_num_confirmations().call().await?);

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

    add_multisig_keys(context.multisig(), &signers).await?;

    context
        .multisig()
        .add_request_and_confirm(MultiSigRequest {
            receiver_id: context.multisig().contract_account(),
            actions: vec![MultiSigRequestAction::SetNumConfirmations { num_confirmations: 2 }],
        })
        .call()
        .await?;

    assert_eq!(2, context.multisig().get_num_confirmations().call().await?);

    let wasm = load_wasm("../res/hodl_lockup.wasm")?;

    update_multisig_with_method_call(&mut context, &signers_accounts, &wasm).await?;

    assert_eq!(
        context.ft_contract().contract_account(),
        context.lockup().get_token_account_id().call().await?
    );

    assert_eq!("1.2.0", context.lockup().get_version().call().await?);

    Ok(())
}

pub async fn update_multisig_with_method_call(
    context: &mut Context,
    signers_accounts: &[Account],
    wasm: &[u8],
) -> Result<()> {
    println!("👷🏽 update_with_method_call");

    dbg!(signers_accounts[0].near_balance().await?);

    let update_request = context
        .multisig()
        .add_request_and_confirm(MultiSigRequest {
            receiver_id: context.lockup().contract_account(),
            actions: vec![MultiSigRequestAction::FunctionCall {
                method_name: "update_contract".to_string(),
                args: wasm.to_vec().into(),
                deposit: U128(1),
                gas: Gas::from_tgas(250).as_gas().into(),
            }],
        })
        .with_user(&signers_accounts[0])
        .call()
        .await?;

    let confirmations = context.multisig().get_confirmations(update_request).call().await?;

    assert_eq!(1, confirmations.len());

    dbg!(signers_accounts[1].near_balance().await?);

    // context.multisig().get

    context
        .multisig()
        .confirm(update_request)
        .with_user(&signers_accounts[1])
        .call()
        .await?;

    assert!(context.multisig().list_request_ids().call().await?.is_empty());

    println!("👷🏽 update_with_method_call: OK");

    Ok(())
}

pub async fn add_multisig_keys(mut multisig: Multisig<'_>, signers: &[SecretKey]) -> Result<()> {
    let multisig_keys_actions: Vec<_> = signers
        .into_iter()
        .map(|key| MultiSigRequestAction::AddKey {
            public_key: to_near_pk(key.public_key()),
            permission: FunctionCallPermission {
                allowance: U128(NearToken::from_near(5).as_yoctonear()).into(),
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
        .call()
        .await?;

    Ok(())
}

fn to_near_pk(key: near_workspaces::types::PublicKey) -> near_sdk::PublicKey {
    near_sdk::PublicKey::from_str(&key.to_string()).unwrap()
}
