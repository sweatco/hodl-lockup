#!/bin/bash
set -eox pipefail

echo ">> Building utils contract"

rustup target add wasm32-unknown-unknown
cargo build -p utils --target wasm32-unknown-unknown --profile=contract

cp ./target/wasm32-unknown-unknown/contract/utils.wasm res/utils.wasm
