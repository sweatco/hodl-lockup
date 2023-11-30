use std::{env, fs};

pub fn load_wasm(wasm_path: &str) -> anyhow::Result<Vec<u8>> {
    let current_dir = env::current_dir()?;
    let wasm_filepath = fs::canonicalize(current_dir.join(wasm_path))?;
    let data = fs::read(wasm_filepath)?;
    Ok(data)
}
