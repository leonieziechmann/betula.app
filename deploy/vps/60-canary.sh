#!/usr/bin/env bash
# 60-canary.sh - canary follows master by itself: what vps/canary-agent.sh needs, and the timer
# that starts it every two minutes (README.md section 12). As root:
#
#   sudo bash /opt/betula/vps/60-canary.sh              # install or update; changes nothing else
#   <password manager CLI> | ssh betula sudo bash /opt/betula/vps/60-canary.sh token
#                                                       # store the GitHub token, then follow master
#   ssh -t betula sudo bash /opt/betula/vps/60-canary.sh token        (hidden prompt instead)
#   sudo bash /opt/betula/vps/60-canary.sh off          # stop following master; canary stays as it is
#   sudo bash /opt/betula/vps/60-canary.sh on           # follow again
#
# The token: a fine-grained personal access token of the repository's owner, for the repository
# leonieziechmann/betula.app ONLY, with the one permission "Actions: Read-only" (GitHub adds
# "Metadata: Read-only" by itself). It can list the runs of the workflow and download their
# artifacts; it cannot read the code, push, or change anything. Before it is stored it is tried
# against the API, and a token that can read the code is refused. It is kept encrypted with this
# host's key (systemd-creds) in /etc/credstore.encrypted/betula-canary-github-token and reaches
# nobody but betula-canary.service, in clear only in that service's credentials directory. It
# travels stdin -> shell variable -> stdin of systemd-creds: never an argument, never a file in clear.
#
# Installs: sqlite3 (the agent's read-only copy of the public site's database), the units
# betula-canary.service and .timer, the state directory /var/lib/betula-canary. Idempotent; a
# re-run never turns the timer on or off (only "token", "on" and "off" do), but restarts it when
# a unit changed.
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
# shellcheck source=lib-stacks.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-stacks.sh"
betula_init --tmp
require_root
require_ubuntu

UNITS=(betula-canary.service betula-canary.timer)
CREDSTORE="/etc/credstore.encrypted"
TOKEN_FILE="${CREDSTORE}/betula-canary-github-token"
# The name the service asks for (LoadCredentialEncrypted= in betula-canary.service); systemd-creds
# embeds it and refuses the file under any other name.
TOKEN_NAME="github-token"
STATE_DIR="/var/lib/betula-canary"
API="https://api.github.com"

usage() {
  die "usage: $(basename "$0") [token | on | off]"
}

install_units() {
  step "sqlite3, units, state directory"
  local unit changed=0
  apt_install sqlite3
  for unit in "${UNITS[@]}"; do
    install_file "${BETULA_FILES_DIR}/${unit}" "/etc/systemd/system/${unit}" 0644
    changed=$((changed + INSTALL_CHANGED))
  done
  # StateDirectory= would create it at the first run; now, so that "canary-agent.sh status" and
  # "deploy" work by hand before that.
  install -d -m 0700 -o "${DEPLOY_USER}" -g "${DEPLOY_USER}" "${STATE_DIR}"
  install -d -m 0700 -o root -g root "${CREDSTORE}"
  if [[ "${changed}" -gt 0 ]]; then
    systemctl daemon-reload
    # A timer that is already waiting keeps the schedule it was started with.
    if unit_active betula-canary.timer; then
      systemctl restart betula-canary.timer
      log "betula-canary.timer restarted with the new units"
    fi
  fi
}

# Sets TOKEN from the first line of stdin (hidden prompt on a terminal). Never prints it.
TOKEN=""
read_token() {
  local cr=$'\r'
  if [[ -t 0 ]]; then
    printf 'GitHub token (fine-grained, Actions: Read-only; input is hidden): ' >&2
    IFS= read -rs TOKEN || true
    printf '\n' >&2
  else
    IFS= read -r TOKEN || true # "|| true": a last line without a newline still counts
  fi
  TOKEN="${TOKEN%"${cr}"}"
  TOKEN="${TOKEN//[[:space:]]/}"
  [[ -n "${TOKEN}" ]] || die "no token on stdin"
  if [[ "${TOKEN}" == ghp_* ]]; then
    die "that is a classic token: it cannot be limited to one repository and one permission. Create a fine-grained one (github_pat_...): repository ${CANARY_REPO} only, Actions: Read-only"
  fi
  [[ "${TOKEN}" =~ ^github_pat_[A-Za-z0-9_]{20,255}$ ]] || die "that is not a fine-grained personal access token (github_pat_...)"
}

# try_token PATH -> the HTTP status GitHub answers to GET PATH with the token (000: no answer).
try_token() {
  local headers code
  headers="$(betula_tmpfile)"
  # A header file, not an argument: argv is visible to every user of the machine.
  printf 'Authorization: Bearer %s\nAccept: application/vnd.github+json\nX-GitHub-Api-Version: 2022-11-28\nUser-Agent: betula-canary-setup\n' "${TOKEN}" >"${headers}"
  code="$(curl --proto '=https' --tlsv1.2 -sS --max-time 30 -H @"${headers}" -o /dev/null -w '%{http_code}' "${API}$1" 2>/dev/null)" || true
  rm -f -- "${headers}"
  printf '%s' "${code:-000}"
}

store_token() {
  step "GitHub token"
  local code
  require_cmd curl systemd-creds
  read_token
  code="$(try_token "/repos/${CANARY_REPO}/actions/runs?per_page=1")"
  case "${code}" in
    200) log "the token may read the workflow runs of ${CANARY_REPO}" ;;
    401) die "GitHub does not accept the token (401): mistyped, expired or revoked" ;;
    403 | 404) die "the token may not read the Actions of ${CANARY_REPO} (${code}): give it that repository and \"Actions: Read-only\"" ;;
    *) die "no usable answer from GitHub (${code}); nothing was stored" ;;
  esac
  # Least privilege, checked: the agent needs the Actions, not the code.
  code="$(try_token "/repos/${CANARY_REPO}/contents/flake.nix")"
  if [[ "${code}" == "200" ]]; then
    die "the token can read the code of ${CANARY_REPO} as well: it only needs \"Actions: Read-only\" (and the Metadata GitHub adds). Create one without \"Contents\"; nothing was stored"
  fi
  log "it cannot read the code (${code}), as it should"
  # printf is a shell builtin: the token is never an argument of a process.
  printf '%s' "${TOKEN}" | systemd-creds encrypt --name="${TOKEN_NAME}" - "${TOKEN_FILE}.new"
  chmod 0600 "${TOKEN_FILE}.new"
  mv -f -- "${TOKEN_FILE}.new" "${TOKEN_FILE}"
  TOKEN=""
  log "stored encrypted in ${TOKEN_FILE} (only betula-canary.service sees it in clear)"
}

timer_on() {
  step "Follow master"
  [[ -s "${TOKEN_FILE}" ]] || die "no GitHub token yet: <password manager CLI> | ssh betula sudo bash ${BETULA_ROOT}/vps/$(basename "$0") token"
  systemctl enable --now betula-canary.timer >/dev/null 2>&1
  log "betula-canary.timer is on: a new build of master reaches https://canary.betula.app within minutes of its end (next look: $(systemctl show betula-canary.timer --property=NextElapseUSecRealtime --value))"
  log "watch it: journalctl -fu betula-canary.service; what canary runs: bash ${BETULA_ROOT}/vps/canary-agent.sh status (as ${DEPLOY_USER})"
}

timer_off() {
  step "Stop following master"
  if unit_exists betula-canary.timer; then
    systemctl disable --now betula-canary.timer >/dev/null 2>&1
  fi
  if unit_active betula-canary.service; then
    log "a run is in progress; it is finished, not interrupted (journalctl -fu betula-canary.service)"
  fi
  log "betula-canary.timer is off: canary stays as it is. Back on: sudo bash ${BETULA_ROOT}/vps/$(basename "$0") on"
}

# ---------------------------------------------------------------- main

[[ "$#" -le 1 ]] || usage
case "${1:-}" in
  "")
    install_units
    if unit_active betula-canary.timer; then
      log "betula-canary.timer is on (sudo bash ${BETULA_ROOT}/vps/$(basename "$0") off stops it)"
    elif [[ -s "${TOKEN_FILE}" ]]; then
      log "betula-canary.timer is off: sudo bash ${BETULA_ROOT}/vps/$(basename "$0") on"
    else
      log "next: a GitHub token (README.md section 12): <password manager CLI> | ssh betula sudo bash ${BETULA_ROOT}/vps/$(basename "$0") token"
    fi
    ;;
  token)
    install_units
    store_token
    timer_on
    ;;
  on)
    install_units
    timer_on
    ;;
  off) timer_off ;;
  *) usage ;;
esac
