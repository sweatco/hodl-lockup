#![cfg(test)]

use anyhow::Result;
use async_trait::async_trait;
use hodl_model::api::{HodlContract, LockupApiIntegration};
use near_workspaces::{types::NearToken, Account};
use nitka::misc::ToNear;
use sweat_model::{StorageManagementIntegration, SweatApiIntegration, SweatContract};

pub const LOCKUP_CONTRACT: &str = "hodl_lockup";
pub const FT_CONTRACT: &str = "sweat";

pub type Context = nitka::context::Context<near_workspaces::network::Sandbox>;

#[async_trait]
pub trait IntegrationContext {
    async fn manager(&mut self) -> Result<Account>;
    async fn bob(&mut self) -> Result<Account>;
    async fn alice(&mut self) -> Result<Account>;
    fn lockup(&self) -> HodlContract<'_>;
    fn ft_contract(&self) -> SweatContract<'_>;
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

    fn lockup(&self) -> HodlContract<'_> {
        HodlContract {
            contract: &self.contracts[LOCKUP_CONTRACT],
        }
    }

    fn ft_contract(&self) -> SweatContract<'_> {
        SweatContract {
            contract: &self.contracts[FT_CONTRACT],
        }
    }
}

pub(crate) async fn prepare_contract() -> Result<Context> {
    let mut context = Context::new(&[LOCKUP_CONTRACT, FT_CONTRACT], true, "build-integration".into()).await?;

    let manager = context.manager().await?;

    context.ft_contract().new(".u.sweat.testnet".to_string().into()).await?;
    context.ft_contract().add_oracle(&manager.to_near()).await?;

    context
        .ft_contract()
        .tge_mint(&manager.to_near(), (999_999_000_000_000 * 10u128.pow(18)).into())
        .await?;

    context
        .ft_contract()
        .storage_deposit(Some(context.lockup().contract.id().clone()), None)
        .await?;

    context
        .lockup()
        .new(
            context.ft_contract().contract.id().clone(),
            manager.to_near(),
            vec![
                ("DepositManager".to_string(), vec![manager.to_near()]),
                ("StagingManager".to_string(), vec![manager.to_near()]),
                ("UpgradeManager".to_string(), vec![manager.to_near()]),
            ],
        )
        .await?;

    Ok(context)
}
