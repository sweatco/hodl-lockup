#!/bin/bash
set -eox pipefail

rustup component add clippy

cargo clippy --all \
  -- \
  \
  -W clippy::all \
  -W clippy::pedantic \
  \
  -A clippy::module_name_repetitions \
  -A clippy::needless-pass-by-value \
  -A clippy::must-use-candidate \
  -A clippy::missing-panics-doc \
  -A clippy::missing-errors-doc \
  -A clippy::doc-markdown \
  \
  -D warnings
