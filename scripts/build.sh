#!/bin/bash
set -eox pipefail

echo ">> Building contract"

rustup target add wasm32-unknown-unknown
cargo build -p hodl-lockup --target wasm32-unknown-unknown --profile=contract

cp ./target/wasm32-unknown-unknown/contract/hodl_lockup.wasm res/hodl_lockup.wasm
