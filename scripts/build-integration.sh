#!/bin/bash
set -eox pipefail

echo ">> Building contract"

cargo near build non-reproducible-wasm --out-dir res --features integration-test --manifest-path contract/Cargo.toml
