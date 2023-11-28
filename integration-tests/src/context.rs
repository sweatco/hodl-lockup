#![cfg(test)]

use anyhow::Result;
use async_trait::async_trait;
use integration_utils::{integration_contract::IntegrationContract, misc::ToNear};
use model::lockup_api::LockupApiIntegration;
use multisig_integration::{Multisig, MULTISIG};
use multisig_model::api::MultisigApiIntegration;
use near_workspaces::Account;
use sweat_integration::{SweatFt, FT_CONTRACT};
use sweat_model::SweatApiIntegration;

use crate::lockup_interface::{GetContractAccount, LockupContract, LOCKUP_CONTRACT};

pub type Context = integration_utils::context::Context<near_workspaces::network::Testnet>;

#[async_trait]
pub trait IntegrationContext {
    async fn manager(&mut self) -> Result<Account>;
    async fn bob(&mut self) -> Result<Account>;
    async fn alice(&mut self) -> Result<Account>;
    fn lockup(&self) -> LockupContract<'_>;
    fn multisig(&self) -> Multisig<'_>;
    fn ft_contract(&self) -> SweatFt<'_>;
}

#[async_trait]
impl IntegrationContext for Context {
    async fn manager(&mut self) -> Result<Account> {
        self.account("manager").await
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
}

pub(crate) async fn prepare_contract() -> Result<Context> {
    let mut context = Context::new(&[LOCKUP_CONTRACT, MULTISIG, FT_CONTRACT], "build-integration".into()).await?;

    let manager = context.manager().await?;

    context.ft_contract().new(".u.sweat.testnet".to_string().into()).await?;
    context.ft_contract().add_oracle(&manager.to_near()).await?;

    context.multisig().new(0).await?;

    context
        .lockup()
        .new(
            context.ft_contract().contract_account(),
            vec![manager.to_near()],
            Some(vec![manager.to_near()]),
        )
        .await?;

    Ok(context)
}
