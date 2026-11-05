#!/usr/bin/env bash
# 49-phloem.sh - deploy an instance of Phloem, the service of Betula Sync (stacks/phloem.yml,
# docs/phloem/phloem.md), from an image that is loaded on this server already, and then try what
# it must not be able to do. Run on the server as the deploy user, WITHOUT sudo;
# deploy/ship-phloem.sh does that after it has built and loaded the image:
#
#   bash /opt/betula/vps/49-phloem.sh phloem-canary 2026-11-05-ab12cd3   # this release
#   bash /opt/betula/vps/49-phloem.sh phloem                             # the release that runs
#                                                     # now (after a change to stacks/phloem.yml
#                                                     # or stacks/phloem/phloem.env)
#
# Each instance is a stack of its own (phloem: https://betula.app, phloem-canary:
# https://canary.betula.app), with its own network, data and numbers, deployed on its own. In
# this order:
#   1. What it needs: its file stacks/phloem/<stack>.env; its network <stack>_in (overlay,
#      internal, not attachable: vps/30-docker.sh), on which Traefik is; its two file systems
#      (vps/47-phloem-host.sh); the image.
#   2. docker stack deploy <stack>. Swarm stops the old task before it starts the new one (one
#      writer on phloem.db): the devices hear 1001 and come back by themselves within seconds.
#      What swarm was told is checked (image, revision, the router), and waited for until it runs.
#   3. What is forbidden, tried. From inside: "phloem probe" (DNS, the internet, the metadata
#      service, IPv6, Traefik's address: all must be dropped, and the service has no privilege
#      left). From outside: https://<APP_HOST>/sync/v1/ reaches Phloem (its router, by Traefik's
#      access log), /healthz does not; its stats file is there and fresh. The monitoring is told
#      first that probes run: they set off "Phloem: its firewall drops connections" otherwise.
#   4. 91-verify-stacks.sh phloem: its networks, the probes from the host's side, what the
#      monitoring sees.
# A FAIL in 3 or 4 ends with exit 1: the release runs, but it should not stay. Roll back with the
# tag before it, which is still loaded (docker image ls betula-phloem):
#   bash /opt/betula/vps/49-phloem.sh <stack> <previous tag>
#
# Environment (all optional):
#   CONVERGE_TIMEOUT=600   seconds to wait for the task to run (its health check gives it 30 s)
#   PHLOEM_VERIFY=1        0: step 4 is skipped (a test swarm without the monitoring)
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
# shellcheck source=lib-stacks.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-stacks.sh"
# shellcheck source=lib-phloem.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-phloem.sh"
betula_init --tmp

# The service label that keeps the revision of what a deploy is made of (stack file and .env).
REV_LABEL="app.betula.phloem-rev"
TAG_PATTERN='^[A-Za-z0-9][A-Za-z0-9_.-]{0,100}$'
TRAEFIK="edge_traefik"
PHLOEM_VERIFY="${PHLOEM_VERIFY:-1}"
# Where the monitoring learns that probes run (91-verify-stacks.sh writes the same file).
PROBES_FILE="${BETULA_RUN_DIR}/metrics/verify-probes.prom"
FAILS=0

usage() {
  die "usage: bash $(basename "$0") <instance> [<tag>]   (instances: $(phloem_names | tr '\n' ' '); loaded: $(loaded_images))"
}

loaded_images() {
  docker image ls --format '{{.Repository}}:{{.Tag}}' betula-phloem 2>/dev/null | tr '\n' ' ' || true
}

ok() { log "ok: $*"; }
bad() {
  printf '%s [%s] FAIL: %s\n' "$(_ts)" "${BETULA_SCRIPT}" "$*" >&2
  FAILS=$((FAILS + 1))
}

# service_image SERVICE -> the image the service is given, without a digest (nothing when the
# service does not exist).
service_image() {
  local image
  image="$(docker service inspect "$1" --format '{{.Spec.TaskTemplate.ContainerSpec.Image}}' 2>/dev/null || true)"
  printf '%s' "${image%%@*}"
}

# running_tag -> the tag of the image the instance runs now (nothing when it does not exist).
running_tag() {
  local image
  image="$(service_image "${SERVICE}")"
  [[ "${image}" == *:* ]] || return 0
  printf '%s' "${image##*:}"
}

# file_services FILE -> the services FILE defines (the keys two spaces deep under "services:").
file_services() {
  awk '
    /^[^[:space:]#]/ { on = ($0 ~ /^services:[[:space:]]*$/); next }
    on && /^  [^[:space:]#]/ { name = $1; sub(/:.*$/, "", name); print name }
  ' "$1"
}

# ---------------------------------------------------------------- steps

preflight() {
  step "Preconditions of ${STACK}"
  local facts services svc
  if [[ "${EUID}" -eq 0 && -n "${SUDO_USER:-}" ]]; then
    die "run this as ${DEPLOY_USER} WITHOUT sudo: docker stack deploy reads PHLOEM_IMAGE and the rest from the environment, and sudo resets it"
  fi
  [[ "${BETULA_VPS_DIR}" == "${BETULA_ROOT}/vps" ]] ||
    die "this copy lives in ${BETULA_VPS_DIR}; the stack file is read from ${STACKS_DIR}, so run ${BETULA_ROOT}/vps/$(basename "$0")"
  [[ "${CONVERGE_TIMEOUT}" =~ ^[0-9]{1,5}$ ]] || die "CONVERGE_TIMEOUT='${CONVERGE_TIMEOUT}' is not a number of seconds"
  [[ "${PHLOEM_VERIFY}" == "0" || "${PHLOEM_VERIFY}" == "1" ]] || die "PHLOEM_VERIFY='${PHLOEM_VERIFY}' is not 0 or 1"
  require_cmd docker jq curl sha256sum findmnt awk
  require_swarm_manager
  log "instance ${STACK}: https://${PHLOEM_HOST}/sync/v1/, origins ${PHLOEM_ORIGIN_LIST}, budget ${PHLOEM_BUDGET} bytes"

  # Its one network: Traefik and it, nothing else can join (attachable false), and no way out
  # (internal true). Made by 30-docker.sh, never by a stack.
  facts="$(docker network inspect "${STACK}_in" --format '{{.Driver}} {{.Scope}} {{.Attachable}} {{.Internal}}' 2>/dev/null || true)"
  [[ "${facts}" == "overlay swarm false true" ]] ||
    die "network ${STACK}_in is missing or not what it has to be ('${facts:-missing}', wanted 'overlay swarm false true': internal, not attachable). sudo bash ${BETULA_ROOT}/vps/30-docker.sh makes it"
  [[ "$(service_networks "${TRAEFIK}")" == *"${STACK}_in true"* ]] ||
    die "${TRAEFIK} is not on ${STACK}_in, so it could not reach Phloem: bash ${BETULA_ROOT}/vps/40-stacks.sh (stacks/edge.yml names the network)"
  phloem_check_mounts "${STACK}" || die "${PHLOEM_MOUNT_DETAIL}"
  ok "its file systems: $(phloem_data_dir "${STACK}") (ext4 of its own) and $(phloem_stats_dir "${STACK}") (tmpfs of 1 MiB)"

  docker image inspect "${PHLOEM_IMAGE}" >/dev/null 2>&1 ||
    die "image ${PHLOEM_IMAGE} is not loaded on this server (deploy/ship-phloem.sh builds and loads it; loaded: $(loaded_images))"
  [[ -f "${PHLOEM_STACK_FILE}" ]] || die "stack file missing: ${PHLOEM_STACK_FILE} (run deploy/sync.sh)"
  assert_lf "${PHLOEM_STACK_FILE}"
  services="$(file_services "${PHLOEM_STACK_FILE}" | tr '\n' ' ')"
  [[ "${services}" == "sync " ]] ||
    die "${PHLOEM_STACK_FILE} has to define exactly the service sync, not: ${services:-none}"
  if stack_exists "${STACK}"; then
    while IFS= read -r svc; do
      [[ -z "${svc}" || "${svc}" == "${SERVICE}" ]] ||
        die "stack ${STACK} has the service ${svc}, which ${PHLOEM_STACK_FILE##*/} does not define: remove it (docker service rm ${svc}) and run this again"
    done < <(stack_services "${STACK}")
  fi
}

# undrift - "docker stack deploy" leaves the image of a service alone while the file names the
# image the stack gave it last (it compares with the label com.docker.stack.image, not with the
# image the service runs: 48-cortex.sh, undrift). A service given another image by hand gets this
# deploy's image directly first.
undrift() {
  local image label
  docker service inspect "${SERVICE}" >/dev/null 2>&1 || return 0
  image="$(service_image "${SERVICE}")"
  label="$(service_label "${SERVICE}" com.docker.stack.image)"
  [[ "${image}" != "${label}" && "${label}" == "${PHLOEM_IMAGE}" ]] || return 0
  warn "${SERVICE} runs ${image}, not ${label}, which the stack gave it (docker service update by hand?): it is given ${PHLOEM_IMAGE} directly first"
  docker service update --detach --quiet --no-resolve-image --image "${PHLOEM_IMAGE}" "${SERVICE}" >/dev/null
}

deploy() {
  step "docker stack deploy ${STACK}: ${PHLOEM_IMAGE}, revision ${PHLOEM_REV}"
  local told
  if docker service inspect "${SERVICE}" >/dev/null 2>&1; then
    log "it runs $(service_image "${SERVICE}"), revision $(service_label "${SERVICE}" "${REV_LABEL}")"
  fi
  undrift
  # Substituted into the stack file by "docker stack deploy".
  export STACK_NAME="${STACK}" APP_HOST="${PHLOEM_HOST}" PHLOEM_ORIGINS="${PHLOEM_ORIGIN_LIST}"
  export PHLOEM_BUDGET_BYTES="${PHLOEM_BUDGET}" PHLOEM_IMAGE PHLOEM_REV
  # --resolve-image never: no registry knows the image. --prune: a service that left the file
  # leaves the swarm as well.
  docker stack deploy --detach=true --prune --resolve-image never -c "${PHLOEM_STACK_FILE}" "${STACK}"

  # What swarm was told, not what this script meant: the substitution of "docker stack deploy"
  # has surprises (stacks/betula.yml), and a wrong image is retried forever.
  told="$(service_image "${SERVICE}")"
  [[ "${told}" == "${PHLOEM_IMAGE}" ]] || die "${SERVICE} was given the image '${told}' instead of ${PHLOEM_IMAGE}: look at the image line of ${PHLOEM_STACK_FILE}"
  told="$(service_label "${SERVICE}" "${REV_LABEL}")"
  [[ "${told}" == "${PHLOEM_REV}" ]] || die "${SERVICE} was given the revision '${told}' instead of ${PHLOEM_REV}: look at the label ${REV_LABEL} in ${PHLOEM_STACK_FILE}"
  told="$(service_label "${SERVICE}" "traefik.http.routers.${STACK}-sync.rule")"
  [[ "${told}" == "Host(\`${PHLOEM_HOST}\`) && PathPrefix(\`/sync/v1/\`)" ]] ||
    die "the router of ${SERVICE} says '${told}', wanted Host(\`${PHLOEM_HOST}\`) && PathPrefix(\`/sync/v1/\`): look at its labels in ${PHLOEM_STACK_FILE}"
  told="$(docker service inspect "${SERVICE}" --format '{{range .Spec.TaskTemplate.ContainerSpec.Env}}{{println .}}{{end}}' | sed -n 's/^PHLOEM_ORIGINS=//p')"
  [[ "${told}" == "${PHLOEM_ORIGIN_LIST}" ]] || die "${SERVICE} was given the origins '${told}' instead of ${PHLOEM_ORIGIN_LIST}"

  wait_for_stack "${STACK}"
  [[ "${#FAILED_STACKS[@]}" -eq 0 ]] ||
    die "${STACK} did not converge (above: docker service ps and the last log lines). A failed update was rolled back by swarm; fix the cause and run this again"
}

# announce_probes - tells the monitoring that probes run now, so that the drops they cause in
# Phloem's firewall are no alarm (the rules of group betula-phloem leave out the 15 minutes after
# it). A directory of root's: through sudo -n, as 91-verify-stacks.sh does it.
announce_probes() {
  if printf '# TYPE betula_verify_probes_timestamp_seconds gauge\nbetula_verify_probes_timestamp_seconds %s\n' "$(date +%s)" |
    sudo -n sh -c "install -d -m 0755 '${PROBES_FILE%/*}' && cat >'${PROBES_FILE}.tmp' && chmod 0644 '${PROBES_FILE}.tmp' && mv -f '${PROBES_FILE}.tmp' '${PROBES_FILE}'" 2>/dev/null; then
    log "the monitoring knows that probes run (${PROBES_FILE})"
  else
    warn "could not write ${PROBES_FILE}: the probes may set off \"Phloem: its firewall drops connections\""
  fi
}

# router_of PATH - one request of ours to https://<APP_HOST>PATH: ROUTER_NAME is the router
# Traefik's access log names for it (empty while the line is not there), ROUTER_STATUS and
# ROUTER_BODY what came back. Sets globals: not to be called in $(...).
ROUTER_NAME=""
ROUTER_STATUS=""
ROUTER_BODY=""
router_of() {
  local path=$1 marker="phloem-${RANDOM}${RANDOM}" out
  out="$(betula_tmpfile)"
  # Through this host's Traefik, whatever the name resolves to; -k: the certificate is not the
  # question here (91-verify-stacks.sh checks it). A pairing parcel that cannot exist: no change.
  ROUTER_STATUS="$(curl -sS -k --noproxy '*' --max-time 10 --resolve "${PHLOEM_HOST}:443:127.0.0.1" \
    -H 'Betula-Pair: AAAAAAAAAAAAAAAAAAAAAA' -o "${out}" -w '%{http_code}' \
    "https://${PHLOEM_HOST}${path}?${marker}" 2>/dev/null || true)"
  ROUTER_BODY="$(head -c 200 "${out}" | tr -d '\n')"
  sleep 1
  ROUTER_NAME="$(docker service logs --since 2m "${TRAEFIK}" 2>&1 | grep -F "${marker}" |
    sed -n 's/.*"RouterName":"\([^"]*\)".*/\1/p' | tail -n 1 || true)"
}

probes() {
  step "What ${STACK} must not be able to do, tried"
  local cid address line fields result target what detail open=0 out code prom age
  cid="$(service_container "${SERVICE}")"
  [[ -n "${cid}" ]] || die "${SERVICE} has no running container on this node"
  announce_probes

  # From inside: every probe from Phloem's own network namespace, as its own user. Traefik's
  # address on <stack>_in is the one other member of that network: Phloem may answer it, never
  # call it.
  address="$(docker network inspect "${STACK}_in" --format '{{range .Containers}}{{.Name}} {{.IPv4Address}}{{"\n"}}{{end}}' 2>/dev/null |
    awk -v t="${TRAEFIK}." 'index($1, t) == 1 { sub(/\/.*/, "", $2); print $2; exit }')"
  local -a targets=()
  if [[ -n "${address}" ]]; then
    targets=("${address}:443" "${address}:80" "${address}:8082")
  else
    bad "no address of ${TRAEFIK} on ${STACK}_in to probe"
  fi
  out="$(betula_tmpfile)"
  code=0
  docker exec "${cid}" /bin/phloem probe ${targets[@]+"${targets[@]}"} >"${out}" 2>&1 || code=$?
  while IFS= read -r line; do
    [[ -n "${line}" ]] || continue
    # One line per probe: {"check", "target", "what", "result", "detail"} (phloem/cmd/phloem/probe_linux.go).
    if ! fields="$(jq -r '[.result, .check, (.target // ""), ((.what // "") + (if .detail then ": " + .detail else "" end))] | @tsv' <<<"${line}" 2>/dev/null)"; then
      bad "phloem probe said: ${line}"
      continue
    fi
    IFS=$'\t' read -r result what target detail <<<"${fields}"
    case "${result}" in
      blocked | ok) log "  ${result}: ${what} ${target} (${detail})" ;;
      *)
        bad "from inside ${SERVICE}: ${what} ${target} is ${result} (${detail})"
        open=$((open + 1))
        ;;
    esac
  done <"${out}"
  if [[ "${code}" -eq 0 && "${open}" -eq 0 ]]; then
    ok "from inside: nothing gets out (DNS, the internet, the metadata service, IPv6, Traefik), and the service has no privilege"
  elif [[ "${open}" -eq 0 ]]; then
    bad "phloem probe ended with ${code} (docker exec ${cid:0:12} /bin/phloem probe)"
  fi

  # From outside: through Traefik, as a browser would.
  router_of /sync/v1/pair
  if [[ "${ROUTER_NAME}" == "${STACK}-sync@swarm" && "${ROUTER_STATUS}" == "404" && "${ROUTER_BODY}" == '{"error":"not_found"}' ]]; then
    ok "https://${PHLOEM_HOST}/sync/v1/pair reaches Phloem (router ${ROUTER_NAME}, 404 not_found: no such parcel)"
  else
    bad "https://${PHLOEM_HOST}/sync/v1/pair: router '${ROUTER_NAME:-none in the access log}', ${ROUTER_STATUS:-no answer} ${ROUTER_BODY}; wanted router ${STACK}-sync@swarm and Phloem's 404 {\"error\":\"not_found\"}"
  fi
  router_of /healthz
  if [[ "${ROUTER_NAME}" != "${STACK}-sync@swarm" ]]; then
    ok "https://${PHLOEM_HOST}/healthz does not reach Phloem (router ${ROUTER_NAME:-none}, ${ROUTER_STATUS})"
  else
    bad "https://${PHLOEM_HOST}/healthz reaches Phloem (router ${ROUTER_NAME}): only /sync/v1/ may"
  fi

  # Its numbers: a file on its tmpfs, never a network.
  prom="$(phloem_stats_dir "${STACK}")/phloem.prom"
  if [[ -r "${prom}" ]]; then
    age=$(($(date +%s) - $(stat -c %Y "${prom}")))
    if [[ "$(awk '$1 ~ /^phloem_up([{]|$)/ { print $2 }' "${prom}")" == "1" && "${age}" -le 60 ]]; then
      ok "its stats file says phloem_up 1, written ${age} s ago (${prom})"
    else
      bad "its stats file ${prom} does not say phloem_up 1, or is ${age} s old (Phloem writes it every 15 s)"
    fi
  else
    bad "no stats file ${prom}: Phloem writes it every 15 s, 15 s after its start at the latest"
  fi
  out="$(docker exec "${cid}" /bin/phloem status 2>/dev/null || true)"
  if jq -e . >/dev/null 2>&1 <<<"${out}"; then
    log "phloem status: $(jq -c . <<<"${out}")"
  else
    bad "phloem status does not answer (docker exec ${cid:0:12} /bin/phloem status)"
  fi
}

verify() {
  if [[ "${PHLOEM_VERIFY}" == "0" ]]; then
    warn "PHLOEM_VERIFY=0: 91-verify-stacks.sh phloem is skipped"
    return 0
  fi
  step "91-verify-stacks.sh phloem"
  if ! bash "${BETULA_VPS_DIR}/91-verify-stacks.sh" phloem; then
    bad "91-verify-stacks.sh phloem has FAILs (above)"
  fi
}

report() {
  step "Done"
  docker stack services "${STACK}" --format '{{.Name}}  {{.Replicas}}  {{.Image}}' | sed 's/^/  /'
  if [[ "${FAILS}" -gt 0 ]]; then
    die "${FAILS} FAIL(s) above: ${STACK} runs ${PHLOEM_IMAGE}, but should not stay so. Fix the cause and deploy again, or go back: bash ${BETULA_ROOT}/vps/49-phloem.sh ${STACK} <previous tag> (loaded: $(loaded_images))"
  fi
  log "${STACK} runs ${PHLOEM_IMAGE} at https://${PHLOEM_HOST}/sync/v1/ (revision ${PHLOEM_REV})"
}

# ---------------------------------------------------------------- main

[[ "$#" -ge 1 && "$#" -le 2 && "${1}" != -* && "${2:-}" != -* ]] || usage
load_phloem "$1"
STACK="${PHLOEM_STACK}"
SERVICE="${STACK}_sync"
require_ubuntu

TAG="${2:-}"
if [[ -z "${TAG}" ]]; then
  TAG="$(running_tag)"
  [[ -n "${TAG}" ]] || die "${STACK} is not deployed yet, so there is no release to keep: name a tag (loaded: $(loaded_images))"
fi
[[ "${TAG}" =~ ${TAG_PATTERN} && "${TAG}" != "latest" ]] ||
  die "'${TAG}' is not a release tag. Never \"latest\": swarm compares the service definition, not the image content, so a re-loaded \"latest\" restarts nothing"
PHLOEM_IMAGE="betula-phloem:${TAG}"

preflight
# What this deploy is made of besides the image: the stack file and the instance's file.
PHLOEM_REV="$(cat "${PHLOEM_STACK_FILE}" "${PHLOEM_ENV_DIR}/${STACK}.env" | sha256sum | cut -c1-12)"
deploy
probes
verify
report
