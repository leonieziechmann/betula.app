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

# Literal in the stack files (placeholder.yml, edge.www.yml); not configurable. The application's
# own host name is not literal: every instance has one (stacks/<instance>.env, load_instance).
SITE_HOST="betula.app"
WWW_HOST="www.betula.app"
DEFAULT_GRAFANA_HOST="grafana.betula.app"

# Set by resolves_here: one line that says why (or where to).
RESOLVE_DETAIL=""

# Seconds wait_for_stack waits per stack (the first run pulls about 1 GB of images).
CONVERGE_TIMEOUT="${CONVERGE_TIMEOUT:-600}"
# Stacks wait_for_stack gave up on; the caller decides what that means for its exit code.
FAILED_STACKS=()
# Extra arguments of "docker stack deploy" (50-app.sh: --resolve-image never).
STACK_DEPLOY_ARGS=()

# Set by load_instance: one instance of the application (stacks/<instance>.env).
INSTANCE_STACK=""
INSTANCE_HOST=""
INSTANCE_GATE=""
INSTANCE_CRAWL=""

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

# ---------------------------------------------------------------- deploying

# require_bind_sources FILE... - every host path the files bind-mount must exist before swarm sees them.
require_bind_sources() {
  local path missing=0
  while IFS= read -r path; do
    [[ -n "${path}" ]] || continue
    if [[ ! -e "${path}" ]]; then
      warn "bind source ${path} does not exist"
      missing=$((missing + 1))
    fi
  done < <(yaml_bind_sources "$@")
  [[ "${missing}" -eq 0 ]] ||
    die "${missing} bind source(s) missing: swarm would reject the task. Run deploy/sync.sh (config files) or vps/10-base.sh (/var/log/journal) first"
}

# require_secrets FILE... - same for the external secrets the files name.
require_secrets() {
  local name
  while IFS= read -r name; do
    [[ -n "${name}" ]] || continue
    secret_exists "${name}" ||
      die "swarm secret ${name} does not exist. Create it with the value on stdin, never in argv: <password manager CLI> | ssh betula docker secret create ${name} -   (secrets of the monitoring stack: stacks/monitoring-secrets.sh set ${name})"
  done < <(yaml_external_secrets "$@")
}

# deploy_stack STACK FILE...
deploy_stack() {
  local stack=$1 file
  shift
  local -a args=()
  for file in "$@"; do
    [[ -f "${file}" ]] || die "stack file missing: ${file} (run deploy/sync.sh)"
    assert_lf "${file}"
    args+=(-c "${file}")
  done
  require_bind_sources "$@"
  require_secrets "$@"
  log "docker stack deploy ${stack}: $(printf '%s ' "${@##*/}")"
  # --detach=true: return at once; wait_for_stack below waits with a timeout and explains failures.
  # --prune: a service that left the files leaves the swarm as well.
  # (the "+" form: an empty array is an "unbound variable" for older bash versions)
  docker stack deploy --detach=true --prune ${STACK_DEPLOY_ARGS[@]+"${STACK_DEPLOY_ARGS[@]}"} "${args[@]}" "${stack}"
}

# wait_for_stack STACK - until every service is converged (see service_state), or the timeout.
# A stack that does not get there is added to FAILED_STACKS, with the reason on stderr.
wait_for_stack() {
  local stack=$1 svc state streak=0 deadline last="" summary
  local -a pending=() failed=()
  deadline=$((SECONDS + CONVERGE_TIMEOUT))
  # Give the orchestrator a moment to turn the new spec into an update; "1/1, no update" read
  # too early would describe the state before this deploy.
  sleep 3
  while :; do
    pending=()
    failed=()
    summary=""
    while IFS= read -r svc; do
      [[ -n "${svc}" ]] || continue
      state="$(service_state "${svc}")"
      summary+="${svc#"${stack}"_}: ${state#* }; "
      case "${state}" in
        ok\ *) ;;
        failed\ *) failed+=("${svc}") ;;
        *) pending+=("${svc}") ;;
      esac
    done < <(stack_services "${stack}")
    if [[ "${summary}" != "${last}" ]]; then
      log "${stack}: ${summary:-no services yet}"
      last="${summary}"
    fi
    if [[ "${#failed[@]}" -gt 0 ]]; then
      break
    fi
    if [[ -n "${summary}" && "${#pending[@]}" -eq 0 ]]; then
      # Twice in a row, 5 s apart: a task that dies right after its start does not count.
      streak=$((streak + 1))
      if [[ "${streak}" -ge 2 ]]; then
        log "${stack}: converged"
        return 0
      fi
    else
      streak=0
    fi
    if [[ "${SECONDS}" -ge "${deadline}" ]]; then
      warn "${stack}: not converged after ${CONVERGE_TIMEOUT} s"
      break
    fi
    sleep 5
  done

  for svc in ${failed[@]+"${failed[@]}"} ${pending[@]+"${pending[@]}"}; do
    printf '\n---- docker service ps --no-trunc %s\n' "${svc}" >&2
    docker service ps --no-trunc "${svc}" >&2 || true
    printf -- '---- docker service logs --tail 15 %s\n' "${svc}" >&2
    docker service logs --no-task-ids --tail 15 "${svc}" >&2 2>&1 || true
  done
  FAILED_STACKS+=("${stack}")
  return 0
}

# ---------------------------------------------------------------- instances of the application

# instance_names -> the instances that have a file stacks/<instance>.env, one per line.
instance_names() {
  local file
  for file in "${STACKS_DIR}"/*.env; do
    [[ -f "${file}" ]] || continue
    file="${file##*/}"
    printf '%s\n' "${file%.env}"
  done
}

# load_instance NAME - reads stacks/NAME.env into INSTANCE_STACK, INSTANCE_HOST, INSTANCE_GATE and
# INSTANCE_CRAWL. The file is read, never sourced: a value is data, whatever it looks like.
load_instance() {
  local name=$1 file line key value
  file="${STACKS_DIR}/${name}.env"
  [[ "${name}" =~ ^[a-z][a-z0-9-]{0,30}$ ]] || die "'${name}' is not a name for an instance (lower case letters, digits, '-')"
  [[ -f "${file}" ]] || die "there is no instance '${name}': ${file} does not exist (known: $(instance_names | tr '\n' ' '))"
  assert_lf "${file}"
  INSTANCE_STACK=""
  INSTANCE_HOST=""
  INSTANCE_GATE="on"
  INSTANCE_CRAWL="on"
  while IFS= read -r line || [[ -n "${line}" ]]; do
    [[ -n "${line}" && "${line}" != \#* ]] || continue
    [[ "${line}" == *=* ]] || die "${file}: '${line}' is not NAME=value"
    key="${line%%=*}"
    value="${line#*=}"
    case "${key}" in
      STACK_NAME) INSTANCE_STACK="${value}" ;;
      APP_HOST) INSTANCE_HOST="${value}" ;;
      FOLIA_ACCESS_GATE) INSTANCE_GATE="${value}" ;;
      RADIX_CRAWL) INSTANCE_CRAWL="${value}" ;;
      *) die "${file}: unknown setting ${key} (known: STACK_NAME, APP_HOST, FOLIA_ACCESS_GATE, RADIX_CRAWL)" ;;
    esac
  done <"${file}"
  [[ "${INSTANCE_STACK}" == "${name}" ]] || die "${file}: STACK_NAME is '${INSTANCE_STACK}', the file says '${name}'; they have to agree"
  case "${INSTANCE_STACK}" in
    edge | placeholder | monitoring) die "${file}: '${INSTANCE_STACK}' is the name of another stack" ;;
  esac
  [[ "${INSTANCE_HOST}" =~ ^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,}$ ]] || die "${file}: APP_HOST '${INSTANCE_HOST}' is not a host name"
  [[ "${INSTANCE_GATE}" == "on" || "${INSTANCE_GATE}" == "off" ]] || die "${file}: FOLIA_ACCESS_GATE is '${INSTANCE_GATE}', not on or off"
  [[ "${INSTANCE_CRAWL}" == "on" || "${INSTANCE_CRAWL}" == "off" ]] || die "${file}: RADIX_CRAWL is '${INSTANCE_CRAWL}', not on or off"
}

# Blue-green: two instances whose files name the same APP_HOST are two colours of one site
# (canary.env and canary-green.env). Each is a stack of its own, with its own volumes and its own
# router for the host. Traefik sends the host to the router with the higher priority, and only to
# a service with a running task (swarm says "running" once the healthcheck passed): the priority
# decides which colour is live, and the other one is the rollback that also catches the traffic
# while the live one has no healthy task. 50-app.sh never moves the traffic (a stack that runs
# keeps its priority, a new one next to a sibling starts at STANDBY_PRIORITY); 55-switch.sh does.
#
# Traefik's default priority is the length of the rule (25 for Host(`canary.betula.app`)), and 0 in
# the label means that default. STANDBY_PRIORITY is below every default (the shortest rule,
# Host(`a.bc`), has 12) and above the placeholder's 1.
STANDBY_PRIORITY=2

# The two colours of the canary that vps/canary-agent.sh alternates between when it brings a build
# of master there (README.md section 12): the new release goes to the one that does not serve, and
# the one that served is removed once the new one does. Both files have to say RADIX_CRAWL=off.
CANARY_COLOURS=(canary canary-green)
# Where those builds come from: the workflow .github/workflows/images.yml of this repository, runs
# for a push to master. vps/60-canary.sh checks the token against it, canary-agent.sh polls it.
CANARY_REPO="leonieziechmann/betula.app"
CANARY_WORKFLOW="images.yml"
CANARY_BRANCH="master"

# is_canary_colour NAME - true for the instances in CANARY_COLOURS.
is_canary_colour() {
  local colour
  for colour in "${CANARY_COLOURS[@]}"; do
    [[ "$1" == "${colour}" ]] && return 0
  done
  return 1
}

# app_stacks_for_host HOST -> every stack whose web server is routed for HOST, one per line. Read
# from the router labels stacks/betula.yml sets, so it tells what is deployed, not what is planned.
app_stacks_for_host() {
  local host=$1 svc stack
  while IFS= read -r svc; do
    [[ "${svc}" == *_folia ]] || continue
    stack="${svc%_folia}"
    if [[ "$(service_label "${svc}" "traefik.http.routers.${stack}-folia.rule")" == "Host(\`${host}\`)" ]]; then
      printf '%s\n' "${stack}"
    fi
  done < <(docker service ls --format '{{.Name}}' 2>/dev/null | sort)
  return 0
}

# router_priority STACK -> the priority Traefik gives the router of the stack's web server: its
# label, or where that is unset or 0, Traefik's default (the length of the rule).
router_priority() {
  local value rule
  value="$(service_label "$1_folia" "traefik.http.routers.$1-folia.priority")"
  if [[ "${value}" =~ ^[0-9]{1,9}$ && "${value}" -gt 0 ]]; then
    printf '%s' "$((10#${value}))"
    return 0
  fi
  rule="$(service_label "$1_folia" "traefik.http.routers.$1-folia.rule")"
  printf '%s' "${#rule}"
}

# app_stack_for_host HOST -> the stack Traefik sends HOST to: of the stacks routed for it, the one
# with the highest router priority (nothing when none is).
app_stack_for_host() {
  local host=$1 stack priority best="" best_priority=-1
  while IFS= read -r stack; do
    [[ -n "${stack}" ]] || continue
    priority="$(router_priority "${stack}")"
    if [[ "${priority}" -gt "${best_priority}" ]]; then
      best="${stack}"
      best_priority="${priority}"
    fi
  done < <(app_stacks_for_host "${host}")
  printf '%s' "${best}"
}

# folia_container STACK -> the ID of the container of the stack's web server that runs here.
folia_container() {
  docker ps -q --no-trunc --filter "label=com.docker.swarm.service.name=$1_folia" --filter status=running 2>/dev/null | head -n 1
}

# folia_address STACK -> the address of the stack's running web server on docker_gwbridge, where
# this host reaches it directly, past Traefik (nothing when no task of it runs here).
folia_address() {
  local cid address
  cid="$(folia_container "$1")"
  [[ -n "${cid}" ]] || return 0
  address="$(docker network inspect docker_gwbridge --format "{{with index .Containers \"${cid}\"}}{{.IPv4Address}}{{end}}" 2>/dev/null || true)"
  printf '%s' "${address%/*}"
}

# snapshot_outdated STACK - true when the catalog the stack's web server shows is older than the
# schema its build reads: its log says "snapshot.outdated" after the last "snapshot.activated".
# /healthz does not tell, and such a site fails on every page that needs the newer columns.
snapshot_outdated() {
  local cid last
  cid="$(folia_container "$1")"
  [[ -n "${cid}" ]] || return 1
  last="$(docker logs "${cid}" 2>&1 | grep -oE '"event":"snapshot\.(activated|outdated)"' | tail -n 1 || true)"
  [[ "${last}" == *outdated* ]]
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
