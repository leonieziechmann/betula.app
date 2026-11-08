#!/usr/bin/env bash
# netmatrix.sh - the networks of vps/files/networks.conf in a local one-node swarm, with stand-ins,
# checked the way the server is checked. Run as root (the probes enter network namespaces), on a
# throwaway machine with Docker in swarm mode: never on the server. Needs python3 with PyYAML
# (python3-yaml), nsenter, nft, curl and jq; pulls traefik, the socket proxy, Prometheus, Alloy
# and alpine.
#
#   sudo bash deploy/test/netmatrix.sh          # up, check, down (KEEP=1: leave it running)
#   sudo bash deploy/test/netmatrix.sh up       # only build it
#   sudo bash deploy/test/netmatrix.sh check    # only check what runs
#   sudo bash deploy/test/netmatrix.sh down     # remove everything it made
#
# What it does:
#   render   every stack file with every override "docker stack config" can merge: what the
#            scripts deploy on the server must at least load
#   up       the host's networks, made by the code of vps/30-docker.sh (ensure_host_networks); the
#            stacks edge (+www), placeholder, monitoring (+public), cortex, the instances betula
#            (+cortex) and canary (+cortex +egress), and a stand-in of phloem (netmatrix/phloem.yml),
#            each rendered from the real file and changed by netmatrix/transform.py: real
#            Traefik, socket proxy, Prometheus, scraper and Alloy (the part of config.alloy that
#            reads Phloem's file), stand-ins for the rest; Phloem's firewall (phloem/deploy/firewall.nft)
#            loaded into its stand-in, and the guard run once; the scraper's targets written once
#            (vps/files/betula-scraper-targets.sh, which the server runs every minute)
#   check    vps/91-verify-stacks.sh netmatrix phloem (no FAIL), the routes through Traefik
#            (per site, /sync/v1/ above Folia, sync-ratelimit), folia_status of lib-stacks.sh,
#            what Prometheus has (up of every job, phloem_up, the guard); then two mutations
#            that the checks must catch: a web server on a network too many, Phloem without its
#            firewall; and an instance removed (canary), whose Radix must leave the scraper's
#            targets instead of staying a target that is down (deployed again afterwards)
#   down     the stacks, the networks of the plan, /run/betula/phloem, the work directory
#
# The stack names are the real ones (edge, monitoring, ...), as the plan names services by them:
# it refuses to start where such a stack runs that it did not make itself.
# Environment: NETMATRIX_DIR (work directory, default /tmp/betula-netmatrix), KEEP=1.
set -Eeuo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPLOY_DIR="$(cd "${TEST_DIR}/.." && pwd)"
# Phloem's own firewall, as `phloem start` loads it (the stand-in has none of its own).
PHLOEM_FIREWALL="${DEPLOY_DIR}/../phloem/deploy/firewall.nft"
# shellcheck source=../vps/lib.sh
. "${DEPLOY_DIR}/vps/lib.sh"
# shellcheck source=../vps/lib-stacks.sh
. "${DEPLOY_DIR}/vps/lib-stacks.sh"
# shellcheck source=../vps/lib-networks.sh
. "${DEPLOY_DIR}/vps/lib-networks.sh"
betula_init --tmp

NETMATRIX_DIR="${NETMATRIX_DIR:-/tmp/betula-netmatrix}"
export NETMATRIX_DIR
STACKS=(edge placeholder monitoring cortex betula canary phloem)
LABEL="app.betula.netmatrix"
CHECKS_FAILED=0

ok() { printf 'PASS  %s\n' "$*"; }
bad() {
  CHECKS_FAILED=$((CHECKS_FAILED + 1))
  printf 'FAIL  %s\n' "$*"
}

# ---------------------------------------------------------------- preconditions

preflight() {
  require_root
  require_cmd docker python3 nsenter nft curl jq awk
  python3 -c 'import yaml' 2>/dev/null || die "python3 has no yaml module (apt-get install python3-yaml)"
  require_swarm_manager
  [[ ! -d /opt/betula/vps ]] || die "/opt/betula/vps exists: this looks like the server. The harness replaces its stacks; run it somewhere else"
  local stack svc
  for stack in "${STACKS[@]}" phloem-canary; do
    stack_exists "${stack}" || continue
    while IFS= read -r svc; do
      [[ -n "${svc}" ]] || continue
      [[ "$(service_label "${svc}" "${LABEL}")" == "1" ]] ||
        die "stack ${stack} runs here and was not made by this harness (${svc} has no label ${LABEL}): not touching it"
    done < <(stack_services "${stack}")
  done
}

# ---------------------------------------------------------------- render

# render STACK FILE... -> what "docker stack config" makes of the files (dies when it cannot).
render() {
  local stack=$1 file
  shift
  local -a args=()
  for file in "$@"; do args+=(-c "${DEPLOY_DIR}/stacks/${file}"); done
  docker stack config "${args[@]}" 2>"${BETULA_TMPDIR}/render.err" ||
    die "docker stack config cannot merge $*: $(cat "${BETULA_TMPDIR}/render.err")"
}

render_all() {
  step "Every stack file with its overrides loads"
  local combo
  local -a files
  export ACME_CASERVER="https://127.0.0.1:9/directory" GRAFANA_HOST=grafana.betula.app MONITORING_CONFIG_REV=netmatrix
  export CORTEX_IMAGE=betula-cortex:netmatrix CORTEX_REV=netmatrix PLACEHOLDER_CONFIG_REV=netmatrix
  export STACK_NAME=betula APP_HOST=betula.app FOLIA_ACCESS_GATE=on RADIX_IMAGE=betula-radix:netmatrix FOLIA_IMAGE=betula-folia:netmatrix ROUTER_PRIORITY=0
  export RADIX_EMBED_MODEL=/models/x FOLIA_SEMANTIC_MODEL=/models/y FOLIA_SEMANTIC_PASSAGE_MODEL=x
  for combo in \
    "edge.yml" "edge.yml edge.www.yml" "edge.yml edge.www.yml edge.legacy.yml" \
    "placeholder.yml" \
    "monitoring.yml" "monitoring.yml monitoring.public.yml monitoring.smtp.yml monitoring.notify.example.yml" \
    "cortex.yml" \
    "betula.yml" "betula.yml betula.gemini.yml betula.models.yml betula.cortex.yml betula.egress.yml" \
    "betula.yml betula.cortex.yml betula.cortex-offline.yml" "betula.yml betula.offline.yml"; do
    read -r -a files <<<"${combo}"
    render x "${files[@]}" >/dev/null
    ok "docker stack config: ${combo}"
  done
  # What the legacy file does: Traefik keeps every planned network and gets the old one.
  if render x edge.yml edge.legacy.yml | python3 -c '
import sys, yaml
nets = set(yaml.safe_load(sys.stdin)["services"]["traefik"]["networks"])
sys.exit(0 if {"edge", "out", "edge_metrics", "phloem_in", "edge_betula"} <= nets else 1)'; then
    ok "edge.legacy.yml adds the old network edge to Traefik and keeps the planned ones"
  else
    bad "edge.yml + edge.legacy.yml: Traefik's networks are not the planned ones plus edge"
  fi
  # A stack file and the plan agree: every service the plan puts on a network declares it.
  if render x betula.yml | python3 -c '
import sys, yaml
doc = yaml.safe_load(sys.stdin)
f = doc["services"]["folia"]["networks"]; r = doc["services"]["radix"]["networks"]
names = {k: (v or {}).get("name", "betula_" + k) for k, v in doc["networks"].items()}
sys.exit(0 if {names[n] for n in f} == {"edge_betula", "betula_snapshot"} and {names[n] for n in r} == {"cortex_betula", "betula_snapshot"} else 1)'; then
    ok "betula.yml for betula: Folia on edge_betula and betula_snapshot, Radix on cortex_betula and betula_snapshot"
  else
    bad "betula.yml for betula does not put Folia and Radix on the networks of the plan"
  fi
}

# ---------------------------------------------------------------- up

workdir() {
  step "Work directory ${NETMATRIX_DIR}"
  rm -rf -- "${NETMATRIX_DIR}"
  mkdir -p "${NETMATRIX_DIR}/standin" "${NETMATRIX_DIR}/journal" "${NETMATRIX_DIR}/stacks" "${NETMATRIX_DIR}/scraper"
  cp "${TEST_DIR}/netmatrix/standin.sh" "${NETMATRIX_DIR}/standin/standin.sh"
  cp -a "${DEPLOY_DIR}/config" "${NETMATRIX_DIR}/config"
  # Traefik's default certificate instead of none: without ACME there is no certificate, and the
  # routes are what is tested here, not TLS.
  sed -i 's/^\([[:space:]]*sniStrict:\).*/\1 false/' "${NETMATRIX_DIR}/config/traefik/dynamic/tls.yml"
  # Alloy: the part of config.alloy that reads Phloem's file and the guard's, and its remote write.
  {
    sed -n '/^\/\/ -* Phloem$/,/^\/\/ -* container metrics$/p' "${DEPLOY_DIR}/config/monitoring/alloy/config.alloy"
    sed -n '/^prometheus.remote_write "prometheus" {$/,/^}$/p' "${DEPLOY_DIR}/config/monitoring/alloy/config.alloy"
  } >"${NETMATRIX_DIR}/config/monitoring/alloy/config.alloy"
  local stack
  for stack in phloem phloem-canary; do
    install -d -m 0755 -o 65532 -g 65532 "/run/betula/phloem/${stack}/stats"
  done
  install -d -m 0755 /run/betula/metrics
}

networks_up() {
  step "The host's networks, as vps/30-docker.sh makes them"
  netplan_read
  ensure_host_networks
}

# deploy STACK VAR=VALUE... -- FILE... - the files of stacks/ rendered with the variables,
# changed by transform.py, deployed.
deploy() {
  local stack=$1 out file
  shift
  local -a vars=() args=()
  while [[ "$1" != "--" ]]; do
    vars+=("$1")
    shift
  done
  shift
  for file in "$@"; do args+=(-c "${DEPLOY_DIR}/stacks/${file}"); done
  out="${NETMATRIX_DIR}/stacks/${stack}.yml"
  env "${vars[@]}" docker stack config "${args[@]}" | python3 "${TEST_DIR}/netmatrix/transform.py" "${stack}" "${NETMATRIX_DIR}" >"${out}"
  docker stack deploy --detach=true --resolve-image never -c "${out}" "${stack}" >/dev/null
  log "deployed ${stack} ($*)"
}

deploy_canary() {
  deploy canary STACK_NAME=canary APP_HOST=canary.betula.app FOLIA_ACCESS_GATE=on RADIX_IMAGE=alpine:3.22 FOLIA_IMAGE=alpine:3.22 ROUTER_PRIORITY=0 -- betula.yml betula.cortex.yml betula.egress.yml
}

stacks_up() {
  step "The stacks, with stand-ins"
  local common=(ACME_CASERVER=https://127.0.0.1:9/directory MONITORING_CONFIG_REV=netmatrix PLACEHOLDER_CONFIG_REV=netmatrix)
  deploy edge "${common[@]}" -- edge.yml edge.www.yml
  deploy placeholder "${common[@]}" -- placeholder.yml
  deploy monitoring "${common[@]}" GRAFANA_HOST=grafana.betula.app -- monitoring.yml monitoring.public.yml
  deploy cortex CORTEX_IMAGE=alpine:3.22 CORTEX_REV=netmatrix -- cortex.yml
  deploy betula STACK_NAME=betula APP_HOST=betula.app FOLIA_ACCESS_GATE=on RADIX_IMAGE=alpine:3.22 FOLIA_IMAGE=alpine:3.22 ROUTER_PRIORITY=0 -- betula.yml betula.cortex.yml
  deploy_canary
  env STACK_NAME=phloem APP_HOST=betula.app NETMATRIX_DIR="${NETMATRIX_DIR}" docker stack deploy --detach=true -c "${TEST_DIR}/netmatrix/phloem.yml" phloem >/dev/null
  log "deployed phloem (netmatrix/phloem.yml, the stand-in)"
  local stack
  for stack in "${STACKS[@]}"; do
    wait_for_stack "${stack}"
  done
  [[ "${#FAILED_STACKS[@]}" -eq 0 ]] || die "not converged: ${FAILED_STACKS[*]}"
}

phloem_firewall_up() {
  step "Phloem's firewall into its stand-in, and the guard once"
  local cid pid
  cid="$(service_container phloem_sync)"
  [[ -n "${cid}" ]] || die "no container of phloem_sync"
  pid="$(container_pid "${cid}")"
  # What "phloem start" does at every start; a task swarm replaced since has none yet.
  if nsenter -t "${pid}" -n -- nft list table inet phloem >/dev/null 2>&1; then
    log "table inet phloem is in ${cid:0:12} already"
  else
    nsenter -t "${pid}" -n -- nft -f "${PHLOEM_FIREWALL}"
    log "table inet phloem loaded into ${cid:0:12}"
  fi
  bash "${DEPLOY_DIR}/vps/files/betula-phloem-guard.sh"
  sed 's/^/  /' /run/betula/metrics/phloem-guard.prom | grep -v '^  #'
}

# The scraper's targets, as betula-scraper-targets writes them on the server (every minute there),
# into the directory the harness's scraper mounts (transform.py).
scraper_targets() {
  BETULA_SCRAPER_TARGETS_DIR="${NETMATRIX_DIR}/scraper" bash "${DEPLOY_DIR}/vps/files/betula-scraper-targets.sh"
  log "the scraper's targets: $(cat "${NETMATRIX_DIR}/scraper/radix.json")"
}

# ---------------------------------------------------------------- check

# prom QUERY -> the first value Prometheus has for it (from inside its container).
prom() {
  local cid
  cid="$(service_container monitoring_prometheus)"
  docker exec "${cid}" wget -q -T 10 -O - "http://127.0.0.1:9090/api/v1/query?query=$(jq -rn --arg v "$1" '$v | @uri')" 2>/dev/null |
    jq -r '.data.result[0].value[1] // empty'
}

# through_traefik HOST PATH -> "<status> <body>" of https://HOST:18443PATH (Traefik's ports in the
# harness), the body on one line.
through_traefik() {
  local body="${BETULA_TMPDIR}/body" code text=""
  rm -f -- "${body}"
  code="$(curl -sk --noproxy '*' --max-time 10 --resolve "$1:18443:127.0.0.1" -o "${body}" -w '%{http_code}' "https://$1:18443$2" 2>/dev/null || true)"
  if [[ -s "${body}" ]]; then text="$(head -c 200 "${body}" | tr '\n' ' ')"; fi
  printf '%s %s' "${code:-000}" "${text% }"
}

# foreign_stacks -> the stacks here that this harness did not make, as an extended regex of their
# names (nothing when there are none). On a shared test machine somebody else's stack is no
# network of the plan, and 91-verify-stacks.sh says so, as it should; the harness only does not
# count those FAILs as its own.
foreign_stacks() {
  local stack svc foreign=""
  while IFS= read -r stack; do
    [[ -n "${stack}" ]] || continue
    while IFS= read -r svc; do
      if [[ -n "${svc}" && "$(service_label "${svc}" "${LABEL}")" != "1" ]]; then
        foreign+="${foreign:+|}${stack}"
        break
      fi
    done < <(stack_services "${stack}")
  done < <(docker stack ls --format '{{.Name}}' 2>/dev/null)
  printf '%s' "${foreign}"
}

# verify SECTION... -> what 91-verify-stacks.sh says (stdout); returns the number of its FAILs that
# are not about a foreign stack (at most 255).
verify() {
  local out foreign mine
  out="$(bash "${DEPLOY_DIR}/vps/91-verify-stacks.sh" "$@" 2>&1 || true)"
  printf '%s\n' "${out}"
  foreign="$(foreign_stacks)"
  if [[ -n "${foreign}" ]]; then
    mine="$(grep '^FAIL' <<<"${out}" | grep -cvE "(^|[^a-z0-9_-])(${foreign})[_-]" || true)"
  else
    mine="$(grep -c '^FAIL' <<<"${out}" || true)"
  fi
  [[ "${mine}" -le 255 ]] || mine=255
  return "${mine}"
}

check_verify() {
  step "vps/91-verify-stacks.sh netmatrix phloem"
  local n=0 foreign
  verify netmatrix phloem || n=$?
  foreign="$(foreign_stacks)"
  if [[ "${n}" -eq 0 ]]; then
    ok "91-verify-stacks.sh netmatrix phloem: no FAIL${foreign:+ but about the stacks of somebody else (${foreign//|/, })}"
  else
    bad "91-verify-stacks.sh netmatrix phloem: ${n} FAIL (above)"
  fi
}

check_routes() {
  step "Through Traefik"
  local out host path want code n429 host_path_want path_want
  for host_path_want in "betula.app /livez betula_folia" "canary.betula.app /livez canary_folia" \
    "betula.app /sync/v1/ws phloem_sync" "grafana.betula.app /login monitoring_grafana"; do
    read -r host path want <<<"${host_path_want}"
    out="$(through_traefik "${host}" "${path}")"
    if [[ "${out}" == "200 ${want} ${path}"* ]]; then ok "https://${host}${path} -> ${out}"; else
      bad "https://${host}${path} -> '${out}', wanted 200 from ${want}"
    fi
  done
  out="$(through_traefik www.betula.app /x)"
  if [[ "${out}" == 30[18]* ]]; then ok "https://www.betula.app/x -> ${out%% *} (the redirect of edge.www.yml)"; else bad "https://www.betula.app/x -> '${out}'"; fi
  # 200 requests, 20 at a time: well above 20 a second and the burst of 60.
  n429="$(seq 1 200 | xargs -P 20 -I{} curl -sk --noproxy '*' -o /dev/null --max-time 5 --resolve betula.app:18443:127.0.0.1 -w '%{http_code}\n' https://betula.app:18443/sync/v1/x 2>/dev/null | grep -c '^429$' || true)"
  if [[ "${n429}" -gt 0 ]]; then ok "sync-ratelimit: ${n429} of 200 requests, 20 at a time, got 429"; else bad "sync-ratelimit: none of 200 requests, 20 at a time, got 429"; fi
  for path_want in "/livez 200" "/healthz 200" "/api/status 401" "/api/db 401"; do
    read -r path want <<<"${path_want}"
    code="$(folia_status betula "${path}")"
    if [[ "${code}" == "${want}" ]]; then ok "folia_status betula ${path} -> ${code} (from Traefik's container)"; else
      bad "folia_status betula ${path} -> '${code}', wanted ${want}"
    fi
  done
}

check_prometheus() {
  step "What Prometheus has (after a minute of scrapes and remote writes)"
  local query value
  for _ in $(seq 1 24); do
    [[ "$(prom 'min(up{job="traefik"})')" == "1" && "$(prom 'count(up{job="radix"} == 1)')" == "2" && -n "$(prom 'max(phloem_up)')" && -n "$(prom 'max(betula_phloem_guard_firewall)')" ]] && break
    sleep 5
  done
  for query in 'min(up{job="scraper"})' 'min(up{job="traefik"})' 'min(up{job="radix", stack="betula"})' \
    'min(up{job="radix", stack="canary"})' 'count(up{job="cortex"} == 1)' 'min(up{job="alloy"})' \
    'max(phloem_up{job="phloem", stack="phloem"})' 'max(betula_phloem_guard_firewall{stack="phloem"})' \
    'max(betula_phloem_guard_containers{stack="phloem"})'; do
    value="$(prom "${query}")"
    case "${query}:${value}" in
      count*:2 | *:1) ok "${query} = ${value}" ;;
      *) bad "${query} = ${value:-absent}" ;;
    esac
  done
  value="$(prom 'max(betula_phloem_guard_privileged{stack="phloem"})')"
  if [[ "${value}" == "0" ]]; then ok "betula_phloem_guard_privileged{stack=\"phloem\"} = 0"; else bad "betula_phloem_guard_privileged = ${value:-absent}"; fi
}

# mutation WHAT CMD... - CMD breaks something on purpose; 91-verify-stacks.sh must FAIL on it.
expect_fail() {
  local what=$1 out n=0 foreign
  shift
  out="$(verify "$@")" || n=$?
  foreign="$(foreign_stacks)"
  if [[ "${n}" -gt 0 ]]; then
    ok "${what}: caught, ${n} FAIL:"
    grep '^FAIL' <<<"${out}" | { if [[ -n "${foreign}" ]]; then grep -vE "(^|[^a-z0-9_-])(${foreign})[_-]"; else cat; fi; } |
      cut -c1-200 | sed 's/^/        /'
  else
    bad "${what}: 91-verify-stacks.sh $* saw nothing"
  fi
}

check_mutations() {
  step "Mutations the checks must catch"
  local cid pid
  # The web server next to Prometheus (monitoring_query) and with a way out (monitoring_notify).
  docker service update --detach=false --quiet --network-add monitoring_query --network-add monitoring_notify betula_folia >/dev/null
  expect_fail "betula_folia on monitoring_query and monitoring_notify as well" netmatrix
  docker service update --detach=false --quiet --network-rm monitoring_query --network-rm monitoring_notify betula_folia >/dev/null
  cid="$(service_container phloem_sync)"
  pid="$(container_pid "${cid}")"
  nsenter -t "${pid}" -n -- nft delete table inet phloem
  expect_fail "Phloem without its firewall" phloem
  nsenter -t "${pid}" -n -- nft -f "${PHLOEM_FIREWALL}"
  if verify netmatrix phloem >/dev/null; then
    ok "after both are undone: netmatrix and phloem have no FAIL again"
  else
    bad "after both are undone, 91-verify-stacks.sh netmatrix phloem still FAILs"
  fi
}

# What the scraper did with discovery by DNS until 2026-11-08: on internal networks alone Docker's
# DNS answers a name it does not know with SERVFAIL, Prometheus kept the targets of such a name, and
# a removed colour stayed a target that is down. The list of betula-scraper-targets drops it, and
# the scraper ends its series.
check_removed_instance() {
  step "An instance removed: its Radix leaves the scraper's targets"
  local out value="" left
  out="$(docker exec "$(service_container monitoring_scraper)" nslookup tasks.canary-green_radix. 127.0.0.11 2>&1 || true)"
  log "Docker's DNS in the scraper, for a name it does not know: $(grep -m 1 -ioE 'SERVFAIL|NXDOMAIN|server failure|can.t find[^:]*' <<<"${out}" || printf 'no answer')"
  docker stack rm canary >/dev/null
  scraper_targets
  if grep -q 'canary_radix' "${NETMATRIX_DIR}/scraper/radix.json"; then
    bad "betula-scraper-targets still lists canary_radix after docker stack rm canary"
  fi
  # The scraper sees the rename at once; the series ends two scrapes (30 s each) later.
  for _ in $(seq 1 36); do
    value="$(prom 'count(up{job="radix", stack="canary"})')"
    [[ -n "${value}" ]] || break
    sleep 5
  done
  if [[ -z "${value}" ]]; then
    ok "up{job=\"radix\", stack=\"canary\"} is gone after docker stack rm canary (no target that is down)"
  else
    bad "up{job=\"radix\", stack=\"canary\"} is still there 3 minutes after docker stack rm canary: the scraper kept the target"
  fi
  value="$(prom 'min(up{job="radix", stack="betula"})')"
  if [[ "${value}" == "1" ]]; then ok "up{job=\"radix\", stack=\"betula\"} = 1 all the while"; else bad "up{job=\"radix\", stack=\"betula\"} = ${value:-absent} after canary was removed"; fi
  # Back as it was. A deploy of the same name trips over a network of it that is still there.
  for _ in $(seq 1 30); do
    left="$(docker ps -aq --filter label=com.docker.stack.namespace=canary)$(docker network ls -q --filter label=com.docker.stack.namespace=canary)"
    [[ -n "${left}" ]] || break
    sleep 2
  done
  deploy_canary
  wait_for_stack canary
  scraper_targets
}

# ---------------------------------------------------------------- down

down() {
  step "Down"
  local stack net svc mine
  for stack in "${STACKS[@]}" phloem-canary; do
    stack_exists "${stack}" || continue
    mine=1
    while IFS= read -r svc; do
      [[ -z "${svc}" || "$(service_label "${svc}" "${LABEL}")" == "1" ]] || mine=0
    done < <(stack_services "${stack}")
    if [[ "${mine}" -eq 1 ]]; then docker stack rm "${stack}" >/dev/null; else warn "stack ${stack} was not made by this harness: left alone"; fi
  done
  # Networks go once their tasks are gone.
  sleep 15
  netplan_read
  for net in $(netplan_host_networks) "${NETPLAN_NETS[@]}"; do
    docker network rm "${net}" >/dev/null 2>&1 || true
  done
  docker volume ls -q | grep -E '^(edge|monitoring|cortex|betula|canary|phloem)_' | xargs -r docker volume rm >/dev/null 2>&1 || true
  rm -rf -- /run/betula/phloem "${NETMATRIX_DIR}"
  rm -f -- /run/betula/metrics/phloem-guard.prom /run/betula/metrics/verify-probes.prom
  log "removed"
}

# ---------------------------------------------------------------- main

what="${1:-all}"
preflight
case "${what}" in
  up)
    render_all
    workdir
    networks_up
    stacks_up
    phloem_firewall_up
    scraper_targets
    ;;
  check)
    phloem_firewall_up
    scraper_targets
    check_verify
    check_routes
    check_prometheus
    check_mutations
    check_removed_instance
    ;;
  down) down ;;
  all)
    render_all
    workdir
    networks_up
    stacks_up
    phloem_firewall_up
    scraper_targets
    check_verify
    check_routes
    check_prometheus
    check_mutations
    check_removed_instance
    [[ "${KEEP:-0}" == "1" ]] || down
    ;;
  *) die "usage: $(basename "$0") [up|check|down|all]" ;;
esac
if [[ "${what}" == "check" || "${what}" == "all" ]]; then
  printf '\nNETMATRIX  %s\n' "$([[ "${CHECKS_FAILED}" -eq 0 ]] && printf 'everything as planned' || printf '%d check(s) FAILed' "${CHECKS_FAILED}")"
  [[ "${CHECKS_FAILED}" -eq 0 ]]
fi
