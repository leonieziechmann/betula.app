#!/usr/bin/env bash
# The web tier while working on it, in one command: the browser app built for localhost when it
# is stale (build-client.sh --dev), and the server with app/assets live — an edit of the
# stylesheet, a script or an SVG is there with the next reload, without a build (--live-assets).
#
#   bash scripts/dev.sh                  build what is stale, serve on http://127.0.0.1:8080
#   bash scripts/dev.sh --watch          and build again and restart when Rust code changes
#   bash scripts/dev.sh sizes            what ships, file by file (folia assets)
#   bash scripts/dev.sh -- --addr …      the server's own flags after `--`
#
# The snapshot comes from Radix as always (README.md, „Web tier"). The page cache is not warmed
# (FOLIA_WARM_CACHE): the warm-up renders all 5,000 pages after every start, which here is every
# restart. What ships is built as before, minified: `cargo build`, Nix (docs/frontend.md §3).
set -euo pipefail
cd "$(dirname "$0")/.."

WATCH=0
case "${1:-}" in
  sizes) exec cargo run -q -p folia-server -- assets ;;
  --watch) WATCH=1; shift ;;
esac
SERVER_ARGS=()
case "${1:-}" in
  "") ;;
  --) shift; SERVER_ARGS=("$@") ;;
  *) echo "usage: $0 [--watch | sizes] [-- server flags]" >&2; exit 2 ;;
esac

# Where cargo builds (scripts/build-cache.sh may point it outside the checkout).
TARGET_DIR=$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
  | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p' | tr -s '\134' '/')
[ -n "$TARGET_DIR" ] || TARGET_DIR="${CARGO_TARGET_DIR:-target}"

# The Rust code of the browser app, and of the server besides. What app/assets holds is live.
CLIENT_SOURCES=(app/src catalog/src client/src client/js pack/src Cargo.toml Cargo.lock app/Cargo.toml catalog/Cargo.toml client/Cargo.toml pack/Cargo.toml)
SOURCES=("${CLIENT_SOURCES[@]}" server/src server/build server/Cargo.toml)

build_client() {
  if bash scripts/build-client.sh --dev >/dev/null; then
    echo "dev: browser app built"
  else
    echo "dev: the browser app does not build (bash scripts/build-client.sh --dev); the site stays server-rendered" >&2
  fi
}

PID=""
start() {
  local server="$TARGET_DIR/debug/folia"
  [ -f "$server.exe" ] && server="$server.exe"
  FOLIA_WARM_CACHE="${FOLIA_WARM_CACHE:-off}" "$server" --live-assets app/assets ${SERVER_ARGS[@]+"${SERVER_ARGS[@]}"} &
  PID=$!
}
stop() {
  if [ -n "$PID" ]; then
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
    PID=""
  fi
}
# Made before the first build: what is saved while it runs is the watch's first round.
STAMP=$(mktemp)
trap 'stop; exit 0' INT TERM
trap 'stop; rm -f "$STAMP"' EXIT

if [ ! -f site/pkg/folia_client_bg.wasm ] || [ -n "$(find "${CLIENT_SOURCES[@]}" -newer site/pkg/folia_client_bg.wasm -type f -print -quit 2>/dev/null)" ]; then
  build_client
fi
cargo build -p folia-server
start
if [ "$WATCH" = 0 ]; then
  wait "$PID"
  exit
fi

echo "dev: watching the Rust code; app/assets needs no build (Ctrl+C ends)"
FAILED=0
while sleep 1; do
  changed=$(find "${SOURCES[@]}" -newer "$STAMP" -type f -print 2>/dev/null || true)
  [ -n "$changed" ] || continue
  touch "$STAMP"
  client=0 server=$FAILED
  while read -r path; do
    case "$path" in
      client/*) client=1 ;;
      server/*) server=1 ;;
      *) client=1 server=1 ;;
    esac
  done <<< "$changed"
  # The browser app first, while the server still answers; then the server, which Windows does
  # not let a build overwrite while it runs.
  [ "$client" = 1 ] && build_client
  if [ "$server" = 1 ]; then
    stop
    if cargo build -p folia-server; then
      FAILED=0
      start
    else
      FAILED=1
      echo "dev: the server does not build; waiting for the next change" >&2
    fi
  fi
done
