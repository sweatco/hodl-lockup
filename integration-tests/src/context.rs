#![cfg(test)]

use anyhow::Result;
use async_trait::async_trait;
use integration_utils::integration_contract::IntegrationContract;
use near_workspaces::Account;

use crate::lockup_interface::{LockupContract, LOCKUP_CONTRACT};

pub type Context = integration_utils::context::Context<near_workspaces::network::Sandbox>;

#[async_trait]
pub trait IntegrationContext {
    async fn manager(&mut self) -> Result<Account>;
    async fn alice(&mut self) -> Result<Account>;
    async fn fee(&mut self) -> Result<Account>;
    fn lockup(&self) -> LockupContract<'_>;
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
}

pub(crate) async fn prepare_contract() -> Result<Context> {
    let context = Context::new(&[LOCKUP_CONTRACT], "build-integration".into()).await?;
    //context.lockup().new().await?;
    Ok(context)
}
