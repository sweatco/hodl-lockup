#![cfg(test)]

use anyhow::Result;
use async_trait::async_trait;
use helper_contract::{
    api::HelperApiIntegration,
    interface::{HelperContract, HELPER_CONTRACT},
};
use integration_utils::{integration_contract::IntegrationContract, misc::ToNear};
use model::lockup_api::LockupApiIntegration;
use multisig_integration::{Multisig, MULTISIG};
use multisig_model::api::MultisigApiIntegration;
use near_workspaces::{types::NearToken, Account};
use sweat_integration::{SweatFt, FT_CONTRACT};
use sweat_model::{StorageManagementIntegration, SweatApiIntegration};

use crate::lockup_interface::{GetContractAccount, LockupContract};

pub const LOCKUP_CONTRACT: &str = "hodl_lockup";

pub type Context = integration_utils::context::Context<near_workspaces::network::Sandbox>;

#[async_trait]
pub trait IntegrationContext {
    async fn manager(&mut self) -> Result<Account>;
    async fn bob(&mut self) -> Result<Account>;
    async fn alice(&mut self) -> Result<Account>;
    fn lockup(&self) -> LockupContract<'_>;
    fn multisig(&self) -> Multisig<'_>;
    fn ft_contract(&self) -> SweatFt<'_>;
    fn helper(&self) -> HelperContract<'_>;
}

#[async_trait]
impl IntegrationContext for Context {
    async fn manager(&mut self) -> Result<Account> {
        let name = "manager";

        if !self.accounts.contains_key(name) {
            let root_account = self.worker.dev_create_account().await?;

            let account = root_account
                .create_subaccount(name)
                .initial_balance(NearToken::from_near(50))
                .transact()
                .await?
                .into_result()?;

            self.accounts.insert(name.to_string(), account);
        }

        Ok(self.accounts.get(name).unwrap().clone())
    }

    async fn bob(&mut self) -> Result<Account> {
        self.account("bob").await
    }

    async fn alice(&mut self) -> Result<Account> {
        self.account("alice").await
    }

    fn lockup(&self) -> LockupContract<'_> {
        LockupContract::with_contract(&self.contracts[LOCKUP_CONTRACT])
    }

    fn multisig(&self) -> Multisig<'_> {
        Multisig::with_contract(&self.contracts[MULTISIG])
    }

    fn ft_contract(&self) -> SweatFt<'_> {
        SweatFt::with_contract(&self.contracts[FT_CONTRACT])
    }

    fn helper(&self) -> HelperContract<'_> {
        HelperContract::new(&self.contracts[HELPER_CONTRACT])
    }
}

pub(crate) async fn prepare_contract() -> Result<Context> {
    let mut context = Context::new(
        &[LOCKUP_CONTRACT, MULTISIG, FT_CONTRACT, HELPER_CONTRACT],
        "build-integration".into(),
    )
    .await?;

    let manager = context.manager().await?;

    context
        .ft_contract()
        .new(".u.sweat.testnet".to_string().into())
        .call()
        .await?;
    context.ft_contract().add_oracle(&manager.to_near()).call().await?;

    context.multisig().new(0).call().await?;

    context
        .ft_contract()
        .tge_mint(&manager.to_near(), NearToken::from_near(100).as_near().into())
        .call()
        .await?;

    context
        .ft_contract()
        .storage_deposit(context.lockup().contract_account().into(), None)
        .call()
        .await?;

    context
        .lockup()
        .new(
            context.ft_contract().contract_account(),
            vec![manager.to_near()],
            Some(vec![manager.to_near()]),
            context.multisig().contract().as_account().to_near(),
        )
        .call()
        .await?;

    context.helper().new().result().await?;

    Ok(context)
}
