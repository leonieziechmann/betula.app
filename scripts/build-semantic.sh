#!/usr/bin/env bash
# Builds the Web Worker of the semantic search (semantic/, feature `worker`) into site/pkg, next
# to the browser app, with its JavaScript:
#
#   site/pkg/semantic.simd.wasm      WebAssembly SIMD: every browser of the last years
#   site/pkg/semantic.relaxed.wasm   + relaxed SIMD (Chrome, Edge, Firefox): its int8 dot product
#                                    is a third faster; js/worker.js picks the build by feature test
#   site/pkg/semantic-worker.js      the worker
#   site/pkg/semantic.js             what a page imports (`Semantic`)
#   internal/embed/semantic.wasm     the SIMD build again, for Radix, which embeds it (go:embed) and
#                                    runs it in wazero to compute the modules' vectors: the same
#                                    code as the search's, so Radix's vectors are the crate's bits.
#                                    It is committed (Radix builds with Go alone); build and commit
#                                    it again with every change of semantic/src.
#
# No wasm-bindgen: the worker calls the module's few exports itself (semantic/src/wasm.rs).
# The model and the index are not built here: semantic/README.md says where they come from.
set -euo pipefail
cd "$(dirname "$0")/.."

TARGET_DIR=$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
  | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p' | tr -s '\134' '/')
[ -n "$TARGET_DIR" ] || TARGET_DIR="${CARGO_TARGET_DIR:-target}"

mkdir -p site/pkg
for build in simd relaxed; do
  features="+simd128"
  [ "$build" = relaxed ] && features="+simd128,+relaxed-simd"
  # Exactly these flags (CARGO_ENCODED_RUSTFLAGS outranks every other source of flags), and in a
  # directory of their own, so that the two builds do not overwrite each other's artifacts.
  CARGO_ENCODED_RUSTFLAGS="-Ctarget-feature=$features" cargo rustc -p folia-semantic --lib --features worker \
    --crate-type cdylib --target wasm32-unknown-unknown --profile wasm-release --target-dir "$TARGET_DIR/semantic-$build"
  cp "$TARGET_DIR/semantic-$build/wasm32-unknown-unknown/wasm-release/semantic.wasm" "site/pkg/semantic.$build.wasm"
done
cp site/pkg/semantic.simd.wasm internal/embed/semantic.wasm
cp semantic/js/worker.js site/pkg/semantic-worker.js
cp semantic/js/semantic.js site/pkg/semantic.js
ls -la site/pkg/semantic* internal/embed/semantic.wasm
