#!/bin/bash
set -eox pipefail

echo ">> Building contract"

rustup target add wasm32-unknown-unknown
cd contract
cargo near build non-reproducible-wasm --out-dir ../res
