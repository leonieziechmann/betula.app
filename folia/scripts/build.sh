#!/usr/bin/env bash
# Builds the minimal version's two bundles into folia/site/pkg: the UI (folia-app) and the data
# worker (folia-worker), each in a cargo run of its own (leptos's csr must not meet the server's
# ssr), then the server.
#   folia/scripts/build.sh           size first (wasm-release)
#   folia/scripts/build.sh --dev     seconds, not minutes (wasm-dev)
set -euo pipefail
cd "$(dirname "$0")/.."
PROFILE=wasm-release
FLAGS=(--remove-name-section --remove-producers-section)
[ "${1:-}" = "--dev" ] && { PROFILE=wasm-dev; FLAGS=(); }
mkdir -p site/pkg
cargo build -p folia-app --target wasm32-unknown-unknown --profile "$PROFILE"
wasm-bindgen --target web --no-typescript ${FLAGS[@]+"${FLAGS[@]}"} --out-dir site/pkg --out-name folia_app "target/wasm32-unknown-unknown/$PROFILE/folia_app.wasm"
cargo build -p folia-worker --target wasm32-unknown-unknown --profile "$PROFILE"
wasm-bindgen --target no-modules --no-typescript ${FLAGS[@]+"${FLAGS[@]}"} --out-dir site/pkg --out-name folia_worker "target/wasm32-unknown-unknown/$PROFILE/folia_worker.wasm"
cargo build -p folia-next-server
ls -la site/pkg
