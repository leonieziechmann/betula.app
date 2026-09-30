#!/usr/bin/env bash
# 55-switch.sh - blue-green: hand an instance's host name over to it, away from the other instance
# that is routed for the same name (two files stacks/<name>.env with the same APP_HOST; README.md
# section 4, vps/lib-stacks.sh). Run on the server as the deploy user, WITHOUT sudo:
#
#   bash /opt/betula/vps/55-switch.sh canary-green   # canary-green takes https://canary.betula.app over
#   bash /opt/betula/vps/55-switch.sh canary         # and gives it back: the rollback
#
# In this order; nothing is changed unless the step before succeeded:
#   1. The instance is deployed, its services are converged, its web server answers /livez and
#      /healthz, keeps /api/status closed while FOLIA_ACCESS_GATE=on, and its catalog is not older
#      than the schema its build reads. Asked directly on docker_gwbridge, past Traefik, which
#      still sends the host to the other one.
#   2. Its router gets a priority above every other router for the host. That is a label of the
#      service, not of its tasks: nothing restarts (with docker 29.8.0 or newer; raise_priority
#      says why). Traefik reads it the next time it looks at the swarm (every 15 s).
#   3. Requests to https://<host>/livez until Traefik's access log names this instance's router
#      three times in a row (up to SWITCH_TIMEOUT seconds).
# The other instance keeps running as it is: it is the rollback, and while this one has no healthy
# task Traefik sends the host's requests there. Once this one has proven itself, the other can go:
# docker stack rm <other> (its volumes stay).
#
# Environment (optional):
#   SWITCH_TIMEOUT=120   seconds step 3 waits for Traefik
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
# shellcheck source=lib-stacks.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-stacks.sh"
betula_init
require_ubuntu

SWITCH_TIMEOUT="${SWITCH_TIMEOUT:-120}"

usage() {
  die "usage: $(basename "$0") <instance>   (instances: $(instance_names | tr '\n' ' '))"
}

# running_tasks SERVICE -> the IDs of its tasks that are meant to run, on one line.
running_tasks() {
  docker service ps -q --filter desired-state=running "$1" 2>/dev/null | sort | tr '\n' ' '
}

# router_of_host -> the router Traefik's access log names for one request of ours to the host
# (empty while the line is not there).
router_of_host() {
  local marker="switch-${RANDOM}${RANDOM}"
  # -k: the certificate is not the question here (91-verify-stacks.sh checks it).
  curl -sS -k -o /dev/null --max-time 10 "https://${INSTANCE_HOST}/livez?${marker}" 2>/dev/null || true
  sleep 1
  docker service logs --since 2m edge_traefik 2>&1 | grep -F "${marker}" |
    sed -n 's/.*"RouterName":"\([^"]*\)".*/\1/p' | tail -n 1 || true
}

# ---------------------------------------------------------------- steps

preflight() {
  step "Is ${INSTANCE_STACK} ready to serve https://${INSTANCE_HOST}?"
  local svc state address path code
  if [[ "${EUID}" -eq 0 && -n "${SUDO_USER:-}" ]]; then
    die "run this as ${DEPLOY_USER} WITHOUT sudo"
  fi
  stack_exists "${INSTANCE_STACK}" ||
    die "instance ${INSTANCE_STACK} is not deployed (from the workstation: deploy/ship.sh ${INSTANCE_STACK})"
  [[ "$(service_label "${SERVICE}" "traefik.http.routers.${ROUTER}.rule")" == "Host(\`${INSTANCE_HOST}\`)" ]] ||
    die "the router of ${SERVICE} is not the one for ${INSTANCE_HOST} that ${INSTANCE_STACK}.env names: bash ${BETULA_ROOT}/vps/50-app.sh ${INSTANCE_STACK} first"
  while IFS= read -r svc; do
    [[ -n "${svc}" ]] || continue
    state="$(service_state "${svc}")"
    [[ "${state}" == ok\ * ]] || die "${svc} is not converged (${state#* }): docker service ps --no-trunc ${svc}"
    log "${svc}: ${state#* }"
  done < <(stack_services "${INSTANCE_STACK}")

  address="$(folia_address "${INSTANCE_STACK}")"
  [[ -n "${address}" ]] || die "no container of ${SERVICE} runs on this node (docker service ps --no-trunc ${SERVICE})"
  for path in /livez /healthz; do
    code="$(curl -sS -o /dev/null --max-time 10 -w '%{http_code}' "http://${address}:8080${path}" 2>/dev/null || true)"
    [[ "${code}" == "200" ]] ||
      die "${SERVICE} answers ${path} with ${code:-nothing} (asked directly at ${address}:8080): it has no catalog to show, or no answer from its Radix. docker service logs ${SERVICE}"
    log "${SERVICE} answers ${path} with 200 (asked directly at ${address}:8080)"
  done
  if [[ "${INSTANCE_GATE}" == "on" ]]; then
    code="$(curl -sS -o /dev/null --max-time 10 -w '%{http_code}' "http://${address}:8080/api/status" 2>/dev/null || true)"
    [[ "${code}" == "401" ]] ||
      die "${INSTANCE_STACK}.env says FOLIA_ACCESS_GATE=on, but ${SERVICE} answers /api/status with ${code:-nothing} without the password: it would open the site"
    log "closed testing is in force (/api/status -> 401 without the password)"
  fi
  if snapshot_outdated "${INSTANCE_STACK}"; then
    die "the catalog of ${SERVICE} is older than the schema its build reads (\"snapshot.outdated\" in its log): pages would fail. Build and export a new snapshot in ${INSTANCE_STACK}_radix first (docker exec <its radix container> /bin/radix build --db /data/radix.db, then /bin/radix export --db /data/radix.db --out /data/snapshot)"
  fi
  log "the catalog fits the build (no \"snapshot.outdated\" after the last snapshot it activated)"
}

raise_priority() {
  step "Router priority"
  local other priority best=0 own before after
  local -a others=()
  while IFS= read -r other; do
    [[ -n "${other}" && "${other}" != "${INSTANCE_STACK}" ]] || continue
    others+=("${other}")
    priority="$(router_priority "${other}")"
    if [[ "${priority}" -gt "${best}" ]]; then
      best="${priority}"
    fi
  done < <(app_stacks_for_host "${INSTANCE_HOST}")
  own="$(router_priority "${INSTANCE_STACK}")"
  if [[ "${#others[@]}" -eq 0 ]]; then
    log "no other stack is routed for ${INSTANCE_HOST}: there is nothing to switch from"
    return 0
  fi
  OTHERS=("${others[@]}")
  if [[ "${own}" -gt "${best}" ]]; then
    log "${ROUTER} has the highest priority for ${INSTANCE_HOST} already (${own}; ${others[*]}: ${best})"
    return 0
  fi
  before="$(running_tasks "${SERVICE}")"
  # --detach: a label of the service changes no task, so there is nothing to wait for, as long as
  # the CLI changes nothing else: "service update" hands swarm the whole spec back. Before docker
  # 29.8.0 it sorted the mounts on every call (docker/cli#7227), which moved Folia's tmpfs to the
  # front: a new task template for swarm, so the web server restarted, and Traefik sent the host
  # back to the other colour until it was healthy (a test swarm with 29.3.1). The switches on the
  # server (29.8.1) changed the label and nothing else (checked 2026-09-30). What an update
  # changed: .PreviousSpec against .Spec from the Engine API (curl -s --unix-socket
  # /var/run/docker.sock http://localhost/services/<name>), not from "docker service inspect",
  # which fills swarm's defaults into .Spec only (an empty DNSConfig, the rollback's Monitor).
  docker service update --detach=true --label-add "${PRIORITY_LABEL}=$((best + 1))" "${SERVICE}" >/dev/null
  log "${ROUTER}: priority ${own} -> $((best + 1)) (${others[*]}: ${best})"
  sleep 3
  after="$(running_tasks "${SERVICE}")"
  if [[ "${before}" != "${after}" ]]; then
    warn "the tasks of ${SERVICE} changed with the label (${before}-> ${after}): docker $(docker version --format '{{.Client.Version}}' 2>/dev/null || true) sent swarm more than the label (before 29.8.0 the CLI sorts the mounts), and swarm restarts the web server for it. Until the new task is healthy, Traefik sends https://${INSTANCE_HOST} to ${others[*]}; the next step waits for that"
  fi
}

wait_for_traefik() {
  step "Does Traefik send https://${INSTANCE_HOST} to ${INSTANCE_STACK}?"
  local router streak=0 deadline=$((SECONDS + SWITCH_TIMEOUT)) last=""
  while :; do
    router="$(router_of_host)"
    if [[ "${router}" != "${last}" ]]; then
      log "a request to https://${INSTANCE_HOST}/livez went to ${router:-<no line in the access log yet>}"
      last="${router}"
    fi
    if [[ "${router}" == "${ROUTER}@swarm" ]]; then
      streak=$((streak + 1))
      if [[ "${streak}" -ge 3 ]]; then
        return 0
      fi
    else
      streak=0
    fi
    if [[ "${SECONDS}" -ge "${deadline}" ]]; then
      die "after ${SWITCH_TIMEOUT} s Traefik still sends https://${INSTANCE_HOST} to ${router:-<unknown>}, not to ${ROUTER}@swarm. The priority label stays; look at: docker service logs --since 5m edge_traefik. Back to how it was: bash ${BETULA_ROOT}/vps/55-switch.sh ${OTHERS[0]:-<the other instance>}"
    fi
    sleep 2
  done
}

report() {
  step "Done"
  log "https://${INSTANCE_HOST} is served by ${INSTANCE_STACK} ($(docker service inspect "${SERVICE}" --format '{{.Spec.TaskTemplate.ContainerSpec.Image}}' 2>/dev/null | sed 's/@.*//'))"
  local other
  for other in ${OTHERS[@]+"${OTHERS[@]}"}; do
    log "${other} keeps running as the rollback: bash ${BETULA_ROOT}/vps/55-switch.sh ${other}. Once ${INSTANCE_STACK} has proven itself: docker stack rm ${other} (its volumes stay)"
  done
  log "next: bash ${BETULA_ROOT}/vps/91-verify-stacks.sh services app"
}

# ---------------------------------------------------------------- main

[[ "$#" -eq 1 ]] || usage
[[ "${SWITCH_TIMEOUT}" =~ ^[0-9]{1,4}$ ]] || die "SWITCH_TIMEOUT='${SWITCH_TIMEOUT}' is not a number of seconds"
require_cmd docker curl
require_swarm_manager
load_instance "$1"

SERVICE="${INSTANCE_STACK}_folia"
ROUTER="${INSTANCE_STACK}-folia"
PRIORITY_LABEL="traefik.http.routers.${ROUTER}.priority"
# The other stacks routed for the host (set by raise_priority).
OTHERS=()

preflight
raise_priority
wait_for_traefik
report
