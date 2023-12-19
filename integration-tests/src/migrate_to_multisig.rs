use std::str::FromStr;

use helper_contract::interface::GetContractAccount;
use integration_utils::{integration_contract::IntegrationContract, misc::ToNear};
use model::{ft_message::FtMessage, lockup::LockupCreate, schedule::Schedule, view_api::LockupViewApiIntegration};
use multisig_model::{
    api::MultisigApiIntegration,
    data::{MultiSigRequest, MultiSigRequestAction},
};
use near_sdk::serde_json::{json, to_string};
use near_workspaces::{
    types::{KeyType, SecretKey},
    Account, AccountId,
};
use sweat_model::{FungibleTokenCoreIntegration, StorageManagementIntegration, SweatApiIntegration};

use crate::{
    context::{prepare_contract, Context, IntegrationContext},
    update::multisig::{add_multisig_keys, update_multisig_with_method_call},
    utils::load_wasm,
};

#[tokio::test]
async fn migrate_to_multisig() -> anyhow::Result<()> {
    let mut context = prepare_contract().await?;

    create_lockups(&mut context).await?;

    let lockups_number_before = context.lockup().get_num_lockups().call().await?;

    dbg!(&lockups_number_before);

    let wasm = load_wasm("../res/hodl_lockup_1.2.0.wasm")?;

    let result = context.lockup().contract().as_account().deploy(&wasm).await?;

    let res = result.into_result()?;

    dbg!(&res);

    let multisig_account_id = context.multisig().contract_account();

    context
        .lockup()
        .make_call("migrate")
        .args_json(json!({ "manager": multisig_account_id }))
        .unwrap()
        .call()
        .await?;

    let lockups_number_after = context.lockup().get_num_lockups().call().await?;

    assert_eq!("1.2.0", context.lockup().get_version().call().await?);

    dbg!(&lockups_number_after);

    assert_eq!(lockups_number_before, lockups_number_after);

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

    let wasm = load_wasm("../res/hodl_lockup_1.3.0.wasm")?;

    update_multisig_with_method_call(&mut context, &signers_accounts, &wasm).await?;

    let lockups_number_after = context.lockup().get_num_lockups().call().await?;

    assert_eq!(lockups_number_before, lockups_number_after);

    assert_eq!("1.3.0", context.lockup().get_version().call().await?);

    Ok(())
}

async fn create_lockups(context: &mut Context) -> anyhow::Result<()> {
    let manager = context.manager().await?;

    context
        .ft_contract()
        .tge_mint(&manager.to_near(), 1_000_000_000.into())
        .call()
        .await?;

    for i in 0..5 {
        let account = context.account(&format!("bob_{i}")).await?;

        context
            .ft_contract()
            .storage_deposit(account.to_near().into(), None)
            .call()
            .await?;

        let message = FtMessage::LockupCreate(LockupCreate {
            account_id: account.to_near(),
            schedule: Schedule::new_unlocked(100),
            vesting_schedule: None,
        });

        context
            .ft_contract()
            .ft_transfer_call(
                context.lockup().contract_account(),
                100.into(),
                None,
                to_string(&message).unwrap(),
            )
            .with_user(&manager)
            .call()
            .await?;
    }

    Ok(())
}
