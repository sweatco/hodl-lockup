#![cfg(test)]

use anyhow::Result;
use async_trait::async_trait;
use integration_utils::integration_contract::IntegrationContract;
use multisig_integration::{Multisig, MULTISIG};
use near_workspaces::Account;
use sweat_integration::{SweatFt, FT_CONTRACT};

use crate::lockup_interface::{LockupContract, LOCKUP_CONTRACT};

pub type Context = integration_utils::context::Context<near_workspaces::network::Sandbox>;

#[async_trait]
pub trait IntegrationContext {
    async fn manager(&mut self) -> Result<Account>;
    async fn alice(&mut self) -> Result<Account>;
    async fn fee(&mut self) -> Result<Account>;
    fn lockup(&self) -> LockupContract<'_>;
    fn multisig(&self) -> Multisig<'_>;
    fn ft_contract(&self) -> SweatFt<'_>;
}

#[async_trait]
impl IntegrationContext for Context {
    async fn manager(&mut self) -> Result<Account> {
        self.account("manager").await
    }

    async fn alice(&mut self) -> Result<Account> {
        self.account("alice").await
    }

    async fn fee(&mut self) -> Result<Account> {
        self.account("fee").await
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
    let context = Context::new(&[LOCKUP_CONTRACT, MULTISIG, FT_CONTRACT], "build-integration".into()).await?;
    //context.lockup().new().await?;
    Ok(context)
}
