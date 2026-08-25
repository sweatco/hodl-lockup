#![cfg(test)]

use std::{collections::HashMap, path::PathBuf};

use anyhow::{anyhow, Result};
use near_workspaces::{network::Sandbox, types::NearToken, Account, Contract, Worker};

use crate::ft;

const INITIAL_USER_BALANCE: NearToken = NearToken::from_near(50);

const LOCKUP_WASM_ENV: &str = "HODL_LOCKUP_WASM";
const FT_WASM_ENV: &str = "SWEAT_WASM";

pub struct Context {
    // Held to keep the sandbox alive for the test's lifetime.
    pub worker: Worker<Sandbox>,
    pub lockup: Contract,
    pub ft: Contract,
    accounts: HashMap<String, Account>,
}

impl Context {
    pub async fn account(&mut self, name: &str) -> Result<Account> {
        if !self.accounts.contains_key(name) {
            let root = self.worker.root_account()?;
            let account = root
                .create_subaccount(name)
                .initial_balance(INITIAL_USER_BALANCE)
                .transact()
                .await?
                .into_result()?;
            self.accounts.insert(name.to_string(), account);
        }

        Ok(self.accounts.get(name).unwrap().clone())
    }

    pub async fn manager(&mut self) -> Result<Account> {
        self.account("manager").await
    }
}

/// A booted sandbox with the SWEAT token and the hodl-lockup contract
/// deployed and wired together: `manager` is a test account granted every
/// ACL role.
pub async fn prepare_contract() -> Result<Context> {
    let worker = near_workspaces::sandbox().await?;

    let ft = deploy(&worker, ft_wasm_path()).await?;
    let lockup = deploy(&worker, lockup_wasm_path()).await?;

    let mut context = Context {
        worker,
        lockup,
        ft,
        accounts: HashMap::new(),
    };

    let manager = context.manager().await?;

    ft::new(&context.ft, ".u.sweat.testnet").await?;
    ft::add_oracle(&context.ft, manager.id()).await?;
    ft::tge_mint(&context.ft, manager.id(), 999_999_000_000_000 * 10u128.pow(18)).await?;
    ft::storage_deposit(&context.ft, context.lockup.id()).await?;

    crate::lockup::new(
        &context.lockup,
        context.ft.id(),
        manager.id(),
        &vec![
            ("DepositManager".to_string(), vec![manager.id().clone()]),
            ("StagingManager".to_string(), vec![manager.id().clone()]),
            ("UpgradeManager".to_string(), vec![manager.id().clone()]),
        ],
    )
    .await?;

    Ok(context)
}

fn wasm_path(env_var: &str, default: PathBuf) -> PathBuf {
    std::env::var_os(env_var).map(PathBuf::from).unwrap_or(default)
}

fn repo_path(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("res").join(file)
}

fn ft_wasm_path() -> PathBuf {
    wasm_path(FT_WASM_ENV, repo_path("sweat.wasm"))
}

/// Built with `make build-integration` (`--features integration-test`), which
/// writes over the same `res/hodl_lockup.wasm` the production build uses.
fn lockup_wasm_path() -> PathBuf {
    wasm_path(LOCKUP_WASM_ENV, repo_path("hodl_lockup.wasm"))
}

async fn deploy(worker: &Worker<Sandbox>, path: PathBuf) -> Result<Contract> {
    let bytes = std::fs::read(&path).map_err(|e| {
        anyhow!(
            "failed to read WASM at {} — did you run `make build-integration`? ({e})",
            path.display()
        )
    })?;
    Ok(worker.dev_deploy(&bytes).await?)
}
