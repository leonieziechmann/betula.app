#!/usr/bin/env bash
# The UI bundle split at its #[lazy] functions (docs/folia-refactor.md §6.7), into folia/site/split:
# rustc's binary with LTO, relocations and symbols → Leptos's splitter (wasm_split_cli_support,
# what cargo-leptos runs; `cargo install --locked wasm_split_cli_support --features build-binary`)
# → wasm-bindgen on the main part. Serve it with FOLIA_SPLIT=1.
set -euo pipefail
cd "$(dirname "$0")/.."
SPLIT="${WASM_SPLIT:-wasm-split-cli}"
RUSTFLAGS="-C link-arg=--emit-relocs" CARGO_TARGET_DIR=target/split \
  cargo build -p folia-app --bin folia-app --target wasm32-unknown-unknown --profile wasm-split
rm -rf site/split && mkdir -p site/split
"$SPLIT" target/split/wasm32-unknown-unknown/wasm-split/folia-app.wasm site/split --out-name folia_app
wasm-bindgen --target web --no-typescript --keep-lld-exports --remove-name-section --remove-producers-section --out-dir site/split --out-name folia_app site/split/folia_app.wasm
rm site/split/folia_app.wasm
ls -la site/split
