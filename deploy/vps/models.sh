#!/usr/bin/env bash
# models.sh - the model store of the semantic search (README.md section 13): one file per model in
# /var/lib/betula/models, named by the sha256 of its content, written once and never changed. Every
# instance and every release reads it (read-only, stacks/betula.models.yml); deploy/models.lock says
# which two models run. Run on the server as the deploy user, without sudo:
#
#   bash /opt/betula/vps/models.sh status                 # the lock, the store, which service runs what
#   bash /opt/betula/vps/models.sh missing [<sha256>:<bytes>...]
#                                                        # what the store lacks of these (default: the
#                                                        # lock's), one sha256 a line
#   ... | bash /opt/betula/vps/models.sh receive <sha256> <bytes>
#                                                        # a model on stdin into the store
#   bash /opt/betula/vps/models.sh prune                  # removes what neither the lock nor a
#                                                        # service names
#
# deploy/ship-models.sh runs "missing" and "receive" from the workstation; deploy/ship.sh runs it
# before every deploy. A file arrives in a temporary file of the store's own directory, is checked
# against its size and its sha256, and only then gets its name, in one rename: a name in the store
# always stands for the whole, right content, whatever broke off on the way.
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
# shellcheck source=lib-stacks.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-stacks.sh"
betula_init

usage() {
  die "usage: $(basename "$0") status | missing [<sha256>:<bytes>...] | receive <sha256> <bytes> | prune"
}

require_store() {
  [[ -d "${MODEL_STORE}" ]] ||
    die "there is no model store ${MODEL_STORE}: sudo bash ${BETULA_ROOT}/vps/10-base.sh makes it (or once: sudo install -d -m 0755 -o ${DEPLOY_USER} -g ${DEPLOY_USER} ${MODEL_STORE})"
  [[ -w "${MODEL_STORE}" ]] || die "${MODEL_STORE} is not writable for $(id -un): it belongs to ${DEPLOY_USER} (sudo chown ${DEPLOY_USER}: ${MODEL_STORE})"
}

# human BYTES -> "34.6 MB"
human() { awk -v b="$1" 'BEGIN { printf "%.1f MB", b / 1000000 }'; }

cmd_missing() {
  local pair sum bytes
  local -a wanted=("$@")
  if [[ "${#wanted[@]}" -eq 0 ]]; then
    read_model_lock
    wanted=("${MODEL_PASSAGE}:${MODEL_PASSAGE_BYTES}" "${MODEL_QUERY}:${MODEL_QUERY_BYTES}")
  fi
  for pair in "${wanted[@]}"; do
    [[ "${pair}" =~ ^([0-9a-f]{64}):([1-9][0-9]{0,11})$ ]] || die "'${pair}' is not <sha256>:<bytes>"
    sum="${BASH_REMATCH[1]}" bytes="${BASH_REMATCH[2]}"
    model_intact "${sum}" "${bytes}" || printf '%s\n' "${sum}"
  done
}

# The temporary file of a receive, removed on the way out unless it became a model.
RECEIVE_PART=""
remove_part() { [[ -z "${RECEIVE_PART}" ]] || rm -f -- "${RECEIVE_PART}"; }

cmd_receive() {
  local sum=$1 bytes=$2 have got part
  [[ "${sum}" =~ ^[0-9a-f]{64}$ ]] || die "'${sum}' is not a sha256 (64 hex digits)"
  [[ "${bytes}" =~ ^[1-9][0-9]{0,11}$ ]] || die "'${bytes}' is not a number of bytes"
  [[ ! -t 0 ]] || die "the model has to come on stdin"
  require_store
  if model_intact "${sum}" "${bytes}"; then
    cat >/dev/null
    log "${sum:0:16} is in the store already"
    return 0
  fi
  # In the store's own directory, so that the rename below is one step of one file system. A
  # leftover of a broken upload starts with a dot and is never a model's name.
  RECEIVE_PART="$(mktemp "${MODEL_STORE}/.receive.XXXXXXXX")"
  add_exit_hook remove_part
  part="${RECEIVE_PART}"
  # head -c: never more than announced, so a wrong sender cannot fill the disk.
  head -c "$((bytes + 1))" >"${part}"
  have="$(stat -c %s "${part}")"
  [[ "${have}" == "${bytes}" ]] || die "${sum:0:16}: ${have} bytes arrived, ${bytes} were announced; nothing was stored"
  got="$(sha256sum "${part}")"
  got="${got%% *}"
  [[ "${got}" == "${sum}" ]] || die "${sum:0:16}: what arrived has the sha256 ${got}; nothing was stored"
  chmod 0444 "${part}"
  mv -f -- "${part}" "${MODEL_STORE}/${sum}"
  log "${sum:0:16} stored ($(human "${bytes}"))"
}

cmd_status() {
  local file name sum role used
  read_model_lock
  require_store
  used="$(model_ids_in_use)"
  step "models.lock"
  local -a lock=("passage ${MODEL_PASSAGE} ${MODEL_PASSAGE_BYTES} ${MODEL_PASSAGE_NAME}" "query ${MODEL_QUERY} ${MODEL_QUERY_BYTES} ${MODEL_QUERY_NAME}")
  local entry bytes
  for entry in "${lock[@]}"; do
    read -r role sum bytes name <<<"${entry}"
    if model_intact "${sum}" "${bytes}"; then
      log "${role}  ${sum:0:16}  ${name} ($(human "${bytes}")): in the store, intact"
    else
      warn "${role}  ${sum:0:16}  ${name} ($(human "${bytes}")): NOT in the store, or damaged (from the workstation: SSH_TARGET=betula bash deploy/ship-models.sh)"
    fi
  done
  step "${MODEL_STORE}"
  for file in "${MODEL_STORE}"/*; do
    [[ -f "${file}" ]] || continue
    name="${file##*/}"
    [[ "${name}" =~ ^[0-9a-f]{64}$ ]] || {
      warn "${name}: no model's name (not written by this script)"
      continue
    }
    role=""
    [[ "${name}" == "${MODEL_PASSAGE}" ]] && role="the lock's passage model"
    [[ "${name}" == "${MODEL_QUERY}" ]] && role="the lock's query model"
    if grep -qxF -- "${name}" <<<"${used}"; then
      role="${role:+${role}, }in use"
    fi
    log "${name:0:16}  $(human "$(stat -c %s "${file}")")  ${role:-unused: prune removes it}"
  done
  step "Services"
  local svc env
  while IFS= read -r svc; do
    [[ "${svc}" == *_radix || "${svc}" == *_folia ]] || continue
    env="$(docker service inspect "${svc}" --format '{{range .Spec.TaskTemplate.ContainerSpec.Env}}{{println .}}{{end}}' 2>/dev/null || true)"
    sum="$(sed -n -E 's#^(RADIX_EMBED_MODEL|FOLIA_SEMANTIC_MODEL)=/models/([0-9a-f]{64})$#\2#p' <<<"${env}")"
    if [[ -z "${sum}" ]]; then
      log "${svc}: no model (without the semantic search)"
    elif [[ "${sum}" == "${MODEL_PASSAGE}" || "${sum}" == "${MODEL_QUERY}" ]]; then
      log "${svc}: ${sum:0:16}, the lock's"
    else
      log "${svc}: ${sum:0:16}, not the lock's (its next deploy brings the lock's)"
    fi
  done < <(docker service ls --format '{{.Name}}' 2>/dev/null | sort)
}

cmd_prune() {
  local file name used removed=0
  read_model_lock
  require_store
  used="$(model_ids_in_use)"
  for file in "${MODEL_STORE}"/* "${MODEL_STORE}"/.receive.*; do
    [[ -f "${file}" ]] || continue
    name="${file##*/}"
    if [[ "${name}" == .receive.* ]]; then
      # A broken upload; one still running is younger than an hour.
      if [[ -n "$(find "${file}" -mmin +60 2>/dev/null)" ]]; then
        rm -f -- "${file}"
        log "removed ${name} (an upload that broke off)"
      fi
      continue
    fi
    [[ "${name}" =~ ^[0-9a-f]{64}$ ]] || continue
    [[ "${name}" != "${MODEL_PASSAGE}" && "${name}" != "${MODEL_QUERY}" ]] || continue
    if grep -qxF -- "${name}" <<<"${used}"; then
      log "kept ${name:0:16}: a service runs it (the lock names another; that service's next deploy brings the lock's)"
      continue
    fi
    rm -f -- "${file}"
    removed=$((removed + 1))
    log "removed ${name:0:16}"
  done
  log "${removed} model(s) removed"
}

[[ "$#" -ge 1 ]] || usage
command="$1"
shift
case "${command}" in
  status) [[ "$#" -eq 0 ]] || usage; cmd_status ;;
  missing) cmd_missing "$@" ;;
  receive) [[ "$#" -eq 2 ]] || usage; cmd_receive "$1" "$2" ;;
  prune) [[ "$#" -eq 0 ]] || usage; cmd_prune ;;
  *) usage ;;
esac
