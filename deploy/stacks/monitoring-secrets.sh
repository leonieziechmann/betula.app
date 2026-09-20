#!/usr/bin/env bash
# Swarm secrets of the monitoring stack, created ON the server. Run as the deploy user
# (docker group); no root needed.
# vps/40-stacks.sh creates a missing grafana-admin-password by itself (random, written once to
# /root/betula-initial-credentials.txt). This script is for what needs operator input: your own
# initial admin password (run it BEFORE 40-stacks.sh; both leave an existing secret alone), the
# SMTP secrets, a notification token, and changing the admin password later.
#
#   monitoring-secrets.sh status                 which secrets exist (names only)
#   monitoring-secrets.sh set <name>              value = first line of stdin; hidden prompt on a terminal
#   monitoring-secrets.sh random <name>           value = 48 random bytes (base64); nobody ever sees it
#   monitoring-secrets.sh reset-admin-password    new Grafana password from stdin -> grafana cli in the
#                                                 running container (the secret is only read at first start)
#
# Names: grafana-admin-password (monitoring.yml), grafana-smtp-host, grafana-smtp-user,
# grafana-smtp-password, grafana-smtp-from (monitoring.smtp.yml), grafana-ntfy-url,
# grafana-webhook-token, grafana-telegram-bot-token (contact-points.yml); "-v2", "-v3", ... may
# follow for a rotation - swarm secrets are immutable, see monitoring.smtp.yml.
#
#   laptop:  <password manager CLI> | ssh betula /opt/betula/stacks/monitoring-secrets.sh set grafana-admin-password
#            ssh -t betula /opt/betula/stacks/monitoring-secrets.sh set grafana-smtp-password      (hidden prompt)
#
# Why a script instead of "docker secret create <name> -": typed at a terminal that command echoes
# the value (into every session recording), and a value passed as an argument lands in the shell
# history and in ps. Here it only travels stdin -> shell variable -> docker's stdin.
# Idempotent: an existing secret is never touched and its input is not even read.

set -Eeuo pipefail

SCRIPT="$(basename "$0")"
ADMIN_PASSWORD_MIN=12   # it guards a public login page

_ts() { date -u +%Y-%m-%dT%H:%M:%SZ; }
log() { printf '%s [%s] %s\n' "$(_ts)" "${SCRIPT}" "$*"; }
die() {
  printf '%s [%s] FATAL: %s\n' "$(_ts)" "${SCRIPT}" "$*" >&2
  exit 1
}
trap 'die "line ${LINENO}: \"${BASH_COMMAND}\" failed"' ERR

usage() {
  # The header of this file is the documentation.
  sed -n '2,/^$/{ /^#/!d; s/^# \{0,1\}//; p; }' "$0"
}

KNOWN_NAMES=(
  grafana-admin-password
  grafana-smtp-host
  grafana-smtp-user
  grafana-smtp-password
  grafana-smtp-from
  # Alternative notification channels, see grafana/provisioning/alerting/contact-points.yml.
  grafana-ntfy-url
  grafana-webhook-token
  grafana-telegram-bot-token
)

# A typo must not create a stray secret that no stack file reads.
check_name() {
  local name=$1 base known
  base="${name}"
  if [[ "${name}" =~ ^(.+)-v[0-9]+$ ]]; then
    base="${BASH_REMATCH[1]}"
  fi
  for known in "${KNOWN_NAMES[@]}"; do
    [[ "${base}" == "${known}" ]] && return 0
  done
  die "unknown secret name '${name}' (known: ${KNOWN_NAMES[*]}, optionally with -v<N>)"
}

require_swarm() {
  command -v docker >/dev/null 2>&1 || die "docker is not installed (run vps/30-docker.sh first)"
  local state
  state="$(docker info --format '{{.Swarm.LocalNodeState}}/{{.Swarm.ControlAvailable}}' 2>/dev/null)" ||
    die "cannot talk to the Docker daemon (is $(id -un) in the docker group?)"
  [[ "${state}" == "active/true" ]] || die "this node is not a swarm manager (state: ${state})"
}

secret_exists() { docker secret inspect "$1" >/dev/null 2>&1; }

# Sets VALUE from the first line of stdin. Never prints it.
VALUE=""
read_value() {
  local what=$1 cr=$'\r'
  VALUE=""
  if [[ -t 0 ]]; then
    printf '%s (input is hidden): ' "${what}" >&2
    IFS= read -rs VALUE || true
    printf '\n' >&2
  else
    IFS= read -r VALUE || true   # "|| true": a last line without a newline still counts
  fi
  VALUE="${VALUE%"${cr}"}"       # piped from a Windows file
  [[ -n "${VALUE}" ]] || die "no value on stdin for ${what}"
}

create_secret() {
  local name=$1
  # printf is a shell builtin: the value is never an argument of a process.
  printf '%s' "${VALUE}" | docker secret create --label app.betula.stack=monitoring "${name}" - >/dev/null
  VALUE=""
  log "created: ${name}"
}

cmd_status() {
  local name
  for name in "${KNOWN_NAMES[@]}"; do
    if secret_exists "${name}"; then
      log "exists:  ${name}"
    elif [[ "${name}" == "grafana-admin-password" ]]; then
      log "MISSING: ${name} (monitoring.yml cannot be deployed without it; vps/40-stacks.sh creates it)"
    else
      log "missing: ${name} (optional)"
    fi
  done
}

cmd_set() {
  local name=$1
  check_name "${name}"
  if secret_exists "${name}"; then
    if [[ "${name}" == grafana-admin-password* ]]; then
      log "unchanged: ${name} already exists (to change the password: ${SCRIPT} reset-admin-password)"
    else
      log "unchanged: ${name} already exists (swarm secrets are immutable; rotate with a -v<N> name)"
    fi
    return 0
  fi
  read_value "${name}"
  if [[ "${name}" == grafana-admin-password* && "${#VALUE}" -lt "${ADMIN_PASSWORD_MIN}" ]]; then
    VALUE=""
    die "${name}: shorter than ${ADMIN_PASSWORD_MIN} characters; nothing created"
  fi
  create_secret "${name}"
}

cmd_random() {
  local name=$1
  check_name "${name}"
  if secret_exists "${name}"; then
    log "unchanged: ${name} already exists"
    return 0
  fi
  # head reads exactly 48 bytes and base64 reads to EOF: no SIGPIPE under pipefail. 48 bytes give
  # 64 characters without padding.
  VALUE="$(head -c 48 /dev/urandom | base64 | tr -d '\n')"
  [[ "${#VALUE}" -eq 64 ]] || die "could not read random data"
  create_secret "${name}"
  if [[ "${name}" == grafana-admin-password* ]]; then
    log "nobody knows this password: set yours after the first deploy with '${SCRIPT} reset-admin-password'"
  fi
}

cmd_reset_admin_password() {
  local cid
  cid="$(docker ps -q --filter 'label=com.docker.swarm.service.name=monitoring_grafana' --filter 'health=healthy' | head -n 1)" || true
  [[ -n "${cid}" ]] || die "no healthy monitoring_grafana container on this node (docker stack services monitoring)"
  read_value "new Grafana admin password"
  if [[ "${#VALUE}" -lt "${ADMIN_PASSWORD_MIN}" ]]; then
    VALUE=""
    die "shorter than ${ADMIN_PASSWORD_MIN} characters; nothing changed"
  fi
  # grafana cli exits 0 even when it finds no admin user, so its answer is checked. It never
  # repeats its input; the captured text is safe to show.
  local out
  out="$(printf '%s\n' "${VALUE}" | docker exec -i "${cid}" grafana cli admin reset-admin-password --password-from-stdin 2>&1)" || {
    VALUE=""
    die "grafana cli failed: ${out}"
  }
  VALUE=""
  [[ "${out}" == *"changed successfully"* ]] || die "grafana cli did not confirm the change: ${out}"
  log "Grafana admin password changed (the swarm secret is only the initial password and stays as it is)"
}

main() {
  local cmd=${1:-}
  case "${cmd}" in
    status)
      [[ $# -eq 1 ]] || die "usage: ${SCRIPT} status"
      require_swarm
      cmd_status
      ;;
    set | random)
      [[ $# -eq 2 ]] || die "usage: ${SCRIPT} ${cmd} <name>"
      require_swarm
      "cmd_${cmd}" "$2"
      ;;
    reset-admin-password)
      [[ $# -eq 1 ]] || die "usage: ${SCRIPT} reset-admin-password"
      require_swarm
      cmd_reset_admin_password
      ;;
    -h | --help | help)
      usage
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
}

main "$@"
