#!/usr/bin/env bash
# Builds the browser app into site/pkg (served by Folia under /pkg) with the Web Worker of the
# catalog's search (folia/crates/client/js/search-worker.js), and the Web Worker of the semantic search next to
# it (scripts/build-semantic.sh).
# Needs the wasm32-unknown-unknown target and wasm-bindgen-cli 0.2.128 (on PATH, in
# $WASM_BINDGEN, or the copy Trunk keeps in its cache).
#
#   scripts/build-client.sh          the bundle that ships: profile wasm-release, size first
#   scripts/build-client.sh --dev    while working on the app: profile wasm-dev, seconds not minutes
#
# --dev drops fat LTO, `opt-level = "z"` and the single codegen unit, which is what makes the
# release bundle small and its build slow, and builds with the flags of .cargo/config.toml
# (--cfg erase_components): editing a page and rebuilding goes from 2 min 18 s to 11 s.
# It keeps the names of the bundle's functions for the debugger (a panic's stack trace names
# them), which the bundle that ships leaves out: they were 29 of its 33.8 MB, and 0.3 of the
# 1.7 MB a browser downloaded in brotli. So --dev is for localhost only -- never deploy it.
set -euo pipefail
cd "$(dirname "$0")/.."

PROFILE=wasm-release
BINDGEN_FLAGS=(--remove-name-section --remove-producers-section)
case "${1:-}" in
  --dev) PROFILE=wasm-dev; BINDGEN_FLAGS=() ;;
  # The bundle that ships as Nix builds it: without the flags scripts/build-cache.sh writes into
  # .cargo/config.toml for the builds while working (--cfg erase_components). An empty
  # CARGO_ENCODED_RUSTFLAGS outranks every other source of flags.
  "") export CARGO_ENCODED_RUSTFLAGS= ;;
  *) echo "usage: $0 [--dev]" >&2; exit 2 ;;
esac

cargo build -p folia-client --target wasm32-unknown-unknown --profile "$PROFILE"

# Ask cargo where the bundle landed instead of guessing: scripts/build-cache.sh points
# build.target-dir at this worktree's own cache outside it, and a config file, an exported
# CARGO_TARGET_DIR and the plain target/ all resolve differently. cargo knows which won.
# tr squeezes the doubled backslashes of the JSON string back into path separators.
TARGET_DIR=$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
  | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p' | tr -s '\134' '/')
[ -n "$TARGET_DIR" ] || TARGET_DIR="${CARGO_TARGET_DIR:-target}"

WB="${WASM_BINDGEN:-$(command -v wasm-bindgen || true)}"
if [ -z "$WB" ]; then
  for candidate in "${LOCALAPPDATA:-}/trunkrs/trunk/cache/wasm-bindgen-0.2.128/wasm-bindgen.exe" "$HOME/.cache/trunk/wasm-bindgen-0.2.128/wasm-bindgen"; do
    [ -x "$candidate" ] && WB="$candidate" && break
  done
fi
[ -n "$WB" ] || { echo "wasm-bindgen 0.2.128 not found (cargo install wasm-bindgen-cli --version 0.2.128)" >&2; exit 1; }

mkdir -p site/pkg
"$WB" --target web --no-typescript ${BINDGEN_FLAGS[@]+"${BINDGEN_FLAGS[@]}"} --out-dir site/pkg --out-name folia_client "$TARGET_DIR/wasm32-unknown-unknown/$PROFILE/folia_client.wasm"
# The catalog's search worker, which runs the same bundle on a copy of the catalog of its own.
cp crates/client/js/search-worker.js site/pkg/search-worker.js

# The semantic search's worker: release builds only, for both (it has no dev build of its own).
bash scripts/build-semantic.sh >/dev/null
ls -la site/pkg
