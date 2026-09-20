#!/usr/bin/env bash
# Builds the browser app into site/pkg (served by Folia under /pkg).
# Needs the wasm32-unknown-unknown target and wasm-bindgen-cli 0.2.128 (on PATH, in
# $WASM_BINDGEN, or the copy Trunk keeps in its cache).
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build -p folia-client --target wasm32-unknown-unknown --profile wasm-release

WB="${WASM_BINDGEN:-$(command -v wasm-bindgen || true)}"
if [ -z "$WB" ]; then
  for candidate in "${LOCALAPPDATA:-}/trunkrs/trunk/cache/wasm-bindgen-0.2.128/wasm-bindgen.exe" "$HOME/.cache/trunk/wasm-bindgen-0.2.128/wasm-bindgen"; do
    [ -x "$candidate" ] && WB="$candidate" && break
  done
fi
[ -n "$WB" ] || { echo "wasm-bindgen 0.2.128 not found (cargo install wasm-bindgen-cli --version 0.2.128)" >&2; exit 1; }

mkdir -p site/pkg
"$WB" --target web --no-typescript --out-dir site/pkg --out-name folia_client target/wasm32-unknown-unknown/wasm-release/folia_client.wasm
ls -la site/pkg
