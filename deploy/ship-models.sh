#!/usr/bin/env bash
# ship-models.sh - bring the models of deploy/models.lock into the server's model store
# (README.md section 13). Runs on the workstation (Git Bash on Windows, Linux, macOS); deploy/ship.sh
# runs it before every deploy, and it is the one step after a change of models.lock:
#
#   SSH_TARGET=betula bash deploy/ship-models.sh
#
# Only what the store lacks goes over the wire: the server is asked first (vps/models.sh missing),
# by sha256 and size. A model the store lacks is taken from MODELS_DIR (default: models/ in the
# repository's root, which git ignores) under the file name the lock gives it, checked against the
# lock's sha256 here and once more on the server before it gets its name there. Nothing that is in
# the store is ever overwritten.
#
# The models are not built here: semantic/README.md says how (pack.py, poc/semantic-search).
#
# Environment:
#   SSH_TARGET, SSH_OPTS, DEPLOY_HOST, DEPLOY_USER   as in deploy/sync.sh
#   MODELS_DIR         where the model files are (default: models/ in the repository's root)
#   SHIP_MODELS_SYNC=0 do not run deploy/sync.sh first (ship.sh has just done it)
set -Eeuo pipefail

SCRIPT="$(basename "$0")"
DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${DEPLOY_DIR}/.." && pwd)"
LOCK="${DEPLOY_DIR}/models.lock"
MODELS_DIR="${MODELS_DIR:-${REPO_DIR}/models}"

log() { printf '[%s] %s\n' "${SCRIPT}" "$*"; }
die() {
  printf '[%s] FATAL: %s\n' "${SCRIPT}" "$*" >&2
  exit 1
}
trap 'die "line ${LINENO}: \"${BASH_COMMAND}\" failed"' ERR

DEPLOY_HOST="${DEPLOY_HOST:-betula.app}"
DEPLOY_USER="${DEPLOY_USER:-deploy}"
SSH_TARGET="${SSH_TARGET:-${DEPLOY_USER}@${DEPLOY_HOST}}"
[[ "${SSH_TARGET}" != -* ]] || die "SSH_TARGET must not start with '-'"
ssh_opts=()
if [[ -n "${SSH_OPTS:-}" ]]; then
  read -r -a ssh_opts <<<"${SSH_OPTS}"
fi
remote() { ssh ${ssh_opts[@]+"${ssh_opts[@]}"} "${SSH_TARGET}" "$@"; }
export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*'

for tool in ssh stat; do
  command -v "${tool}" >/dev/null 2>&1 || die "required command not found: ${tool}"
done
# sha256 FILE -> its sha256 (sha256sum on Linux and in Git Bash, shasum on macOS)
sha256() {
  local out
  if command -v sha256sum >/dev/null 2>&1; then
    out="$(sha256sum "$1")"
  else
    out="$(shasum -a 256 "$1")"
  fi
  printf '%s' "${out%% *}"
}
size_of() { stat -c %s "$1" 2>/dev/null || stat -f %z "$1"; }

# The lock, as the server's lib-stacks.sh reads it: "<role> <sha256> <bytes> <file>".
[[ -f "${LOCK}" ]] || die "${LOCK} does not exist"
declare -a SUMS=() BYTES=() NAMES=()
while IFS= read -r line || [[ -n "${line}" ]]; do
  line="${line%$'\r'}"
  [[ "${line}" =~ ^[[:space:]]*(#|$) ]] && continue
  read -r role sum bytes name extra <<<"${line}"
  [[ "${role}" =~ ^(passage|query)$ && "${sum}" =~ ^[0-9a-f]{64}$ && "${bytes}" =~ ^[1-9][0-9]*$ && -n "${name}" && -z "${extra:-}" ]] ||
    die "models.lock: '${line}' is not '<passage|query> <sha256> <bytes> <file>'"
  SUMS+=("${sum}") BYTES+=("${bytes}") NAMES+=("${name}")
done <"${LOCK}"
[[ "${#SUMS[@]}" -eq 2 ]] || die "models.lock names ${#SUMS[@]} models, not the pair (a passage and a query model)"

if [[ "${SHIP_MODELS_SYNC:-1}" != "0" ]]; then
  log "syncing deploy/ first (the server needs vps/models.sh and models.lock)"
  SSH_TARGET="${SSH_TARGET}" SSH_OPTS="${SSH_OPTS:-}" bash "${DEPLOY_DIR}/sync.sh"
fi

wanted=""
for i in "${!SUMS[@]}"; do
  wanted+=" ${SUMS[$i]}:${BYTES[$i]}"
done
missing="$(remote "bash /opt/betula/vps/models.sh missing${wanted}")"
if [[ -z "${missing}" ]]; then
  log "the server's model store holds both models of models.lock"
  exit 0
fi

for i in "${!SUMS[@]}"; do
  grep -qxF -- "${SUMS[$i]}" <<<"${missing}" || continue
  file="${MODELS_DIR}/${NAMES[$i]}"
  [[ -f "${file}" ]] ||
    die "the server lacks ${NAMES[$i]} (${SUMS[$i]:0:16}), and there is no ${file} to send (MODELS_DIR names another directory; semantic/README.md says how the models are made)"
  [[ "$(size_of "${file}")" == "${BYTES[$i]}" && "$(sha256 "${file}")" == "${SUMS[$i]}" ]] ||
    die "${file} is not the model models.lock names (${SUMS[$i]:0:16}, ${BYTES[$i]} bytes): another build of it? The lock and the file go together"
  log "sending ${NAMES[$i]} (${SUMS[$i]:0:16}, $((BYTES[$i] / 1000000)) MB)"
  remote "bash /opt/betula/vps/models.sh receive ${SUMS[$i]} ${BYTES[$i]}" <"${file}"
done
missing="$(remote "bash /opt/betula/vps/models.sh missing${wanted}")"
[[ -z "${missing}" ]] || die "the server's store still lacks: ${missing//$'\n'/ }"
log "done: the server's model store holds both models of models.lock (the next deploy of an instance runs them)"
