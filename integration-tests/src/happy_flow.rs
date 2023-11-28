#![cfg(test)]

use crate::context::prepare_contract;

#[tokio::test]
async fn happy_flow() -> anyhow::Result<()> {
    println!("👷🏽 Run happy flow test");

    let _context = prepare_contract().await?;

    Ok(())
}
