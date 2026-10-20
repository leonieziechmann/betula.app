#!/usr/bin/env bash
# The cloud's prebuild: the tools the builds need, and folia's and Radix's builds themselves, made
# before anyone asks for them (docs/folia/frontend.md §3, „Build times", „In the cloud").
#
#   bash .claude/hooks/prebuild.sh session-start
#       the SessionStart hook (.claude/settings.json): in a cloud session, the prebuild in the
#       background, at the lowest priority; anywhere else nothing
#   bash .claude/hooks/prebuild.sh setup
#       the setup script of the cloud environment: the prebuild in the foreground, stopped after
#       PREBUILD_BUDGET seconds (240), so that the environment's cache keeps what it made
#   bash .claude/hooks/prebuild.sh run
#       the prebuild in the foreground, without a limit
#
# The steps, each over in a moment when its work is done already:
#   1. the wasm32-unknown-unknown target, and wasm-bindgen at the version of folia/Cargo.lock: the
#      release's binary, checked against the sum below (another version is built from crates.io)
#   2. the Go toolchain radix/go.mod asks for, and the modules of radix/ and cortex/
#   3. the crates of folia/Cargo.lock
#   4. in folia/: cargo build, and the browser app as build-client.sh --dev builds it; with the
#      flags of folia/.cargo/config.toml, into folia/target
#   5. Radix and Cortex with their tests, compiled (go test -run '^$')
# Not cargo test --no-run (1 min 34 s): it builds into target/debug as cargo build does, and a
# cargo build of the session would wait for it, nothing left to do, in the minutes in which the
# first edits are built (84 s, measured 2026-10-20). The first cargo test pays it instead.
# Nothing it writes is kept by git: cargo runs --locked, and everything lands in target/ and the
# caches of cargo, rustup and Go. A cargo command of the session that builds into the directory of
# a running step (the same profile and target) waits for its lock and then finds that work done.
set -uo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root" || exit 0

# wasm-bindgen's releases: the sha256 of wasm-bindgen-<version>-x86_64-unknown-linux-musl.tar.gz
# (github.com/wasm-bindgen/wasm-bindgen/releases, the .sha256sum next to it). A new version of the
# crate in folia/Cargo.lock without its line here is built with cargo install (two minutes).
wasm_bindgen_sum() {
  case "$1" in
    0.2.128) echo b51f0208fdff83515a787bd8ab9ac5865ed84dabb66d0c709957bb59793c645f ;;
  esac
}

log() { printf '%s prebuild: %s\n' "$(date +%T)" "$*"; }

deadline=""
# step NAME COMMAND...: one step, cut off where the budget ends (setup); its outcome and time
# logged.
step() {
  local name=$1 t0=$SECONDS rc
  shift
  if [ -n "$deadline" ]; then
    local rest=$((deadline - SECONDS))
    if [ "$rest" -le 5 ]; then
      log "$name: left out, the budget is spent"
      return 0
    fi
    # timeout runs programs, not functions: a function of this script runs in a bash of its own.
    declare -F "$1" >/dev/null && set -- bash -c '"$@"' bash "$@"
    # The whole process group gets the signal, rustc too; what cargo finished stays built.
    timeout --signal=INT --kill-after=15 "$rest" "$@"
    rc=$?
  else
    "$@"
    rc=$?
  fi
  if [ "$rc" = 0 ]; then
    log "$name: done in $((SECONDS - t0)) s"
  else
    log "$name: failed or cut off (exit $rc) after $((SECONDS - t0)) s"
  fi
  return "$rc"
}

wasm_target() {
  rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown ||
    rustup target add wasm32-unknown-unknown
}

wasm_bindgen() {
  local want have sum dir name rc
  want=$(awk '$0 == "name = \"wasm-bindgen\"" { getline; gsub(/^version = "|"$/, ""); print; exit }' folia/Cargo.lock)
  [ -n "$want" ] || return 0
  have=$(wasm-bindgen --version 2>/dev/null | awk '{ print $2 }')
  [ "$have" = "$want" ] && return 0
  sum=$(wasm_bindgen_sum "$want")
  if [ -z "$sum" ] || [ "$(uname -sm)" != "Linux x86_64" ]; then
    log "wasm-bindgen $want: no checked binary, building it"
    cargo install --locked --quiet wasm-bindgen-cli --version "$want"
    return
  fi
  dir=$(mktemp -d)
  name="wasm-bindgen-$want-x86_64-unknown-linux-musl"
  curl -fsSL --max-time 120 -o "$dir/$name.tar.gz" \
      "https://github.com/wasm-bindgen/wasm-bindgen/releases/download/$want/$name.tar.gz" &&
    echo "$sum  $dir/$name.tar.gz" | sha256sum --check --quiet - &&
    tar -xzf "$dir/$name.tar.gz" -C "$dir" &&
    mkdir -p "${CARGO_HOME:-$HOME/.cargo}/bin" &&
    install -m 0755 "$dir/$name/wasm-bindgen" "${CARGO_HOME:-$HOME/.cargo}/bin/wasm-bindgen"
  rc=$?
  rm -rf "$dir"
  return "$rc"
}

go_modules() {
  command -v go >/dev/null || return 0
  # In radix/ first: go.mod's toolchain comes with it (GOTOOLCHAIN=auto).
  (cd radix && go mod download) && (cd cortex && go mod download)
}

go_build() {
  command -v go >/dev/null || return 0
  local m
  for m in radix cortex; do
    (cd "$m" && go build ./... && go test -count=1 -run '^$' ./... >/dev/null) || return 1
  done
}

export -f log wasm_bindgen_sum wasm_target wasm_bindgen go_modules go_build

# Every step, in the order a session is likely to ask for its work.
prebuild() {
  local t0=$SECONDS
  log "starts in $root${deadline:+, for at most $((deadline - SECONDS)) s}"
  step "wasm32 target" wasm_target
  step "wasm-bindgen" wasm_bindgen
  # Quick, and in the setup the only moment the module proxy is sure to be reachable: its
  # downloads lie on storage.googleapis.com.
  step "Go toolchain and modules" go_modules
  (
    cd folia || exit 1
    step "cargo fetch" cargo fetch --locked --quiet
    step "cargo build" cargo build --locked --quiet
    # target/wasm32-unknown-unknown/wasm-dev, a lock of its own: no build of the server waits for
    # it.
    step "browser app (wasm-dev)" cargo build --locked --quiet -p folia-client -p folia-worker \
      --target wasm32-unknown-unknown --profile wasm-dev
  )
  step "Radix and Cortex" go_build
  log "over after $((SECONDS - t0)) s"
}

case "${1:-}" in
  session-start)
    # A local session has its tools and its target/ already.
    [ "${CLAUDE_CODE_REMOTE:-}" = true ] || exit 0
    out=${PREBUILD_LOG:-/tmp/prebuild.log}
    # Detached and with all three streams of its own, so that the hook returns at once and the
    # session does not wait for it; nice and idle I/O, so that the session's own commands go first.
    setsid -f nice -n 19 ionice -c 3 -t bash "${BASH_SOURCE[0]}" run </dev/null >>"$out" 2>&1
    echo "A prebuild runs in the background of this cloud session" \
      "(.claude/hooks/prebuild.sh, log $out): the tools, then in folia/ cargo build and the" \
      "browser app's dev build, then Radix and Cortex. A cargo command may first wait for its" \
      "lock (\"Blocking waiting for file lock\"); let it wait, it then finds that work done."
    ;;
  setup)
    deadline=$((SECONDS + ${PREBUILD_BUDGET:-240}))
    prebuild
    # Never the reason a session does not start.
    exit 0
    ;;
  run)
    # One prebuild at a time: a second one (the session resumed while one runs) leaves it be.
    exec 9>"${TMPDIR:-/tmp}/prebuild.lock"
    flock -n 9 || { log "another prebuild runs; this one leaves it be"; exit 0; }
    prebuild
    ;;
  *)
    echo "usage: $0 session-start|setup|run" >&2
    exit 2
    ;;
esac
