#!/usr/bin/env bash
# The browser app against a reactive_graph that tells of R16 (docs/folia/frontend.md, R16): an
# effect that misses the change of a memo it reads, because it checked a memo made of that one
# first, says so in the browser's console with the places of both memos,
#
#   BETULA-MISSED an effect missed a change of the memo at crates/home/src/home/today.rs:157:19 (checked the memo at crates/home/src/home/today.rs:174:17 first)
#
# The patch (scripts/r16/reactive_graph-0.2.14.patch) goes onto a copy of the crate in the target
# directory, the build takes it by `--config patch.crates-io…`, and the app lands in target/r16/site
# beside the one of build-client.sh: nothing of the repository changes (Cargo.lock is put back).
# It builds into the dev profile's directory, so the next `build-client.sh --dev` builds
# reactive_graph and what uses it again.
#
#   bash scripts/r16.sh
#   target/debug/folia --live-assets assets --site-root target/r16/site     (the server as dev.sh starts it)
#   cd e2e && node --import ./r16.mjs study.mjs                               (each miss on stderr, and in $R16_LOG)
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(sed -n '/^name = "reactive_graph"$/{n;s/^version = "\(.*\)"$/\1/p;}' Cargo.lock)
[ "$VERSION" = "0.2.14" ] || { echo "r16: the patch is for reactive_graph 0.2.14, Cargo.lock has ${VERSION:-none}" >&2; exit 1; }
SOURCE=$(cargo metadata --format-version 1 2>/dev/null | grep -o '"manifest_path":"[^"]*/reactive_graph-0\.2\.14/Cargo\.toml"' | head -1 | sed 's/^"manifest_path":"//; s#/Cargo\.toml"$##' | tr -s '\134' '/')
[ -d "$SOURCE" ] || { echo "r16: reactive_graph 0.2.14 is not in cargo's registry (cargo fetch)" >&2; exit 1; }
TARGET_DIR=$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
  | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p' | tr -s '\134' '/')
[ -n "$TARGET_DIR" ] || TARGET_DIR="${CARGO_TARGET_DIR:-target}"
WORK="$TARGET_DIR/r16"

rm -rf "$WORK/reactive_graph"
mkdir -p "$WORK/site/pkg"
cp -r "$SOURCE" "$WORK/reactive_graph"
patch -s -d "$WORK/reactive_graph" -p1 < scripts/r16/reactive_graph-0.2.14.patch

cp Cargo.lock "$WORK/Cargo.lock"
trap 'cp "$WORK/Cargo.lock" Cargo.lock' EXIT
cargo build -p folia-client -p folia-worker --target wasm32-unknown-unknown --profile wasm-dev \
  --config "patch.crates-io.reactive_graph.path=\"$WORK/reactive_graph\""

WB="${WASM_BINDGEN:-$(command -v wasm-bindgen || true)}"
[ -n "$WB" ] || { echo "r16: wasm-bindgen 0.2.128 not found (as build-client.sh)" >&2; exit 1; }
"$WB" --target web --no-typescript --out-dir "$WORK/site/pkg" --out-name folia_client "$TARGET_DIR/wasm32-unknown-unknown/wasm-dev/folia_client.wasm"
"$WB" --target web --no-typescript --out-dir "$WORK/site/pkg" --out-name folia_worker "$TARGET_DIR/wasm32-unknown-unknown/wasm-dev/folia_worker.wasm"
cp crates/worker/js/data-worker.js "$WORK/site/pkg/data-worker.js"
# The semantic search's worker as build-client.sh made it (it has no reactive graph).
cp site/pkg/semantic* "$WORK/site/pkg/" 2>/dev/null || echo "r16: without the semantic search (bash scripts/build-client.sh makes it)" >&2
echo "r16: the app is in $WORK/site (folia --site-root $WORK/site)"
