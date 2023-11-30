use std::{env, fs};

use anyhow::Result;
use async_trait::async_trait;
use near_workspaces::Account;

pub fn load_wasm(wasm_path: &str) -> anyhow::Result<Vec<u8>> {
    let current_dir = env::current_dir()?;
    let wasm_filepath = fs::canonicalize(current_dir.join(wasm_path))?;
    let data = fs::read(wasm_filepath)?;
    Ok(data)
}

#[async_trait]
pub trait AccountExtension {
    async fn near_balance(&self) -> Result<u128>;
}

#[async_trait]
impl AccountExtension for Account {
    async fn near_balance(&self) -> Result<u128> {
        Ok(self.view_account().await?.balance.as_near())
    }
}
