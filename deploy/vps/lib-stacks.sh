#!/usr/bin/env bash
# Helpers shared by 40-stacks.sh and 91-verify-stacks.sh. Sourced AFTER lib.sh, never executed.
# Both scripts run as the deploy user (group docker), not as root: "docker stack deploy"
# substitutes ${VAR} in the stack files from the environment, and sudo would reset it.
# shellcheck shell=bash

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  echo "lib-stacks.sh is a library; source it after lib.sh" >&2
  exit 2
fi

# The stack files bind-mount /opt/betula/config/... by absolute path (the contract), so the
# scripts only make sense for the copy that lives there.
BETULA_ROOT="/opt/betula"
STACKS_DIR="${BETULA_ROOT}/stacks"
CONFIG_DIR="${BETULA_ROOT}/config"

# Literal in the stack files (placeholder.yml, edge.www.yml, betula.example.yml); not configurable.
SITE_HOST="betula.app"
WWW_HOST="www.betula.app"
DEFAULT_GRAFANA_HOST="grafana.betula.app"

# Set by resolves_here: one line that says why (or where to).
RESOLVE_DETAIL=""

# ---------------------------------------------------------------- docker

require_swarm_manager() {
  have_cmd docker || die "docker is not installed (run vps/30-docker.sh first)"
  local state
  state="$(docker info --format '{{.Swarm.LocalNodeState}}/{{.Swarm.ControlAvailable}}' 2>/dev/null)" ||
    die "cannot talk to the Docker daemon as $(id -un): group docker only applies to logins made after 30-docker.sh - open a new ssh session"
  [[ "${state}" == "active/true" ]] || die "this node is not a swarm manager (state: ${state}); run vps/30-docker.sh"
}

secret_exists() { docker secret inspect "$1" >/dev/null 2>&1; }

stack_exists() {
  local stacks
  stacks="$(docker stack ls --format '{{.Name}}')"
  grep -qxF -- "$1" <<<"${stacks}"
}

# stack_services STACK -> full service names, one per line.
stack_services() {
  docker stack services "$1" --format '{{.Name}}' 2>/dev/null | sort
}

# service_label SERVICE LABEL -> value of a service label (deploy.labels); empty when unset.
service_label() {
  docker service inspect "$1" --format "{{index .Spec.Labels \"$2\"}}" 2>/dev/null || true
}

# service_state SERVICE -> "ok 1/1" | "wait <why>" | "failed <why>"
#   ok      every desired task is RUNNING (with a healthcheck, swarm reports "running" only once
#           the container is healthy; before that the task is "starting") and no update is in flight
#   wait    still converging
#   failed  swarm gave up by itself (update paused or rolled back): waiting longer will not help
service_state() {
  local svc=$1 line replicas running desired update
  line="$(docker service ls --filter "name=${svc}" --format '{{.Name}} {{.Replicas}}')"
  # --filter name= is a prefix match (edge_traefik would also match edge_traefik2).
  replicas="$(awk -v n="${svc}" '$1 == n { print $2 }' <<<"${line}")"
  if ! [[ "${replicas}" =~ ^([0-9]+)/([0-9]+) ]]; then
    printf 'wait no such service (yet)'
    return 0
  fi
  running="${BASH_REMATCH[1]}"
  desired="${BASH_REMATCH[2]}"
  update="$(docker service inspect "${svc}" --format '{{if .UpdateStatus}}{{.UpdateStatus.State}}{{end}}' 2>/dev/null || true)"
  case "${update}" in
    paused | rollback_paused | rollback_completed)
      printf 'failed update %s (%s running)' "${update}" "${replicas}"
      return 0
      ;;
    updating | rollback_started)
      printf 'wait update %s (%s running)' "${update}" "${replicas}"
      return 0
      ;;
  esac
  if [[ "${running}" -eq "${desired}" ]]; then
    printf 'ok %s' "${replicas}"
  else
    printf 'wait %s running' "${replicas}"
  fi
}

# ---------------------------------------------------------------- stack files

# yaml_bind_sources FILE... -> absolute host paths the files bind-mount, one per line.
# Both notations the stack files use: "- /host/path:/container/path[:ro]" and "source: /host/path".
# (Named volumes do not start with "/".) Swarm rejects a task whose bind source is missing
# ("bind source path does not exist"), and a rejected task is retried forever.
yaml_bind_sources() {
  sed -n -E \
    -e 's/^[[:space:]]*-[[:space:]]+"?(\/[^:"[:space:]]*):\/.*$/\1/p' \
    -e 's/^[[:space:]]*source:[[:space:]]*"?(\/[^"#[:space:]]*).*$/\1/p' \
    "$@" | sort -u
}

# yaml_external_secrets FILE... -> names under the top-level "secrets:" key, one per line.
yaml_external_secrets() {
  awk '
    FNR == 1 { on = 0 }
    /^secrets:[[:space:]]*$/ { on = 1; next }
    /^[^[:space:]#]/ { on = 0 }
    on && /^  [A-Za-z0-9_.-]+:/ { name = $1; sub(/:.*$/, "", name); print name }
  ' "$@" | sort -u
}

# ---------------------------------------------------------------- DNS

# This machine's own global addresses (IPv4 and IPv6), without Docker's bridges.
# PUBLIC_ADDRESSES="a b" overrides the detection (a host behind 1:1 NAT does not carry its public address).
local_public_addresses() {
  if [[ -n "${PUBLIC_ADDRESSES:-}" ]]; then
    tr ' ' '\n' <<<"${PUBLIC_ADDRESSES}" | sed '/^$/d'
    return 0
  fi
  local family
  for family in -4 -6; do
    ip -o "${family}" addr show scope global 2>/dev/null |
      awk '$2 !~ /^(docker|br-|veth|vx-|ov-)/ { sub(/\/.*$/, "", $4); print tolower($4) }'
  done
}

# host_addresses NAME -> every A and AAAA address of NAME, one per line (nothing when it does not resolve).
host_addresses() {
  local name=$1 out=""
  if have_cmd dig; then
    # +short prints CNAME targets as well; only lines that are addresses survive the filter.
    out="$(dig +short +time=3 +tries=2 A "${name}" 2>/dev/null || true)"
    out+=$'\n'"$(dig +short +time=3 +tries=2 AAAA "${name}" 2>/dev/null || true)"
  else
    out="$(getent ahosts "${name}" 2>/dev/null | awk '{ print $1 }' || true)"
  fi
  grep -E '^([0-9]{1,3}(\.[0-9]{1,3}){3}|[0-9A-Fa-f:]*:[0-9A-Fa-f:]*)$' <<<"${out}" | tr 'A-F' 'a-f' | sort -u || true
}

# resolves_here NAME - true when NAME resolves and EVERY address is one of this machine's.
# Every, not any: Let's Encrypt prefers an AAAA record when one exists, so a name whose A record
# points here and whose AAAA record points elsewhere fails validation - and failed validations
# are what the rate limit counts (5 per host name per hour).
resolves_here() {
  local name=$1 addrs mine addr
  RESOLVE_DETAIL=""
  addrs="$(host_addresses "${name}")"
  if [[ -z "${addrs}" ]]; then
    RESOLVE_DETAIL="${name} does not resolve (no A or AAAA record)"
    return 1
  fi
  mine="$(local_public_addresses)"
  for addr in ${addrs}; do
    if ! grep -qxF -- "${addr}" <<<"${mine}"; then
      RESOLVE_DETAIL="${name} resolves to ${addr}, which is not an address of this machine (${mine//$'\n'/ })"
      return 1
    fi
  done
  RESOLVE_DETAIL="${name} -> ${addrs//$'\n'/ }"
  return 0
}
