#!/usr/bin/env bash
# 50-app.sh - deploy one instance of the application (stacks/betula.yml) from images that are
# loaded on this server already. Run on the server as the deploy user, WITHOUT sudo; deploy/ship.sh
# does that after it has built and loaded the images:
#
#   bash /opt/betula/vps/50-app.sh canary 2026-09-21-ab12cd3   # this release
#   bash /opt/betula/vps/50-app.sh canary                      # the release that runs now (after a
#                                                              # change to canary.env or betula.yml)
#
# An instance is a file stacks/<instance>.env: the name of its stack, its public host name,
# whether the site asks for the password of closed testing (FOLIA_ACCESS_GATE) and whether Radix
# fetches anything from the university (RADIX_CRAWL; off = it only serves the snapshot it has,
# stacks/betula.offline.yml). Rollback = the previous tag again; it is still loaded
# (docker image ls 'betula-*').
#
# Idempotent: swarm only touches a service whose definition changed. Nothing is deployed unless
# the images exist, the host name resolves to this machine (a router for a name that does not
# makes Traefik order a certificate that cannot be validated; Let's Encrypt counts those, 5 per
# host name per hour) and the secrets exist. The value of a secret never passes through here.
#
# Blue-green (lib-stacks.sh): an instance whose host another instance serves already is deployed
# as its standby - running, but without the host's traffic - and only when both files name the
# same host, say RADIX_CRAWL=off and agree on FOLIA_ACCESS_GATE. This script never moves the
# traffic of a host; vps/55-switch.sh does.
#
# Environment (all optional):
#   PUBLIC_ADDRESSES="..."  this machine's public addresses, if they are not on an interface (NAT)
#   CONVERGE_TIMEOUT=600    seconds to wait for the stack (Radix counts as started after its
#                           healthcheck's first success, up to 2 minutes)
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
# shellcheck source=lib-stacks.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-stacks.sh"
betula_init
require_ubuntu

APP_FILE="${STACKS_DIR}/betula.yml"
GEMINI_FILE="${STACKS_DIR}/betula.gemini.yml"
OFFLINE_FILE="${STACKS_DIR}/betula.offline.yml"
GEMINI_SECRET="gemini-api-key"
GATE_SECRET="folia-access-password"
TAG_PATTERN='^[A-Za-z0-9][A-Za-z0-9_.-]{0,100}$'
# Set by preflight: the other stacks routed for this instance's host (blue-green), and the
# priority this instance's router is deployed with.
SIBLINGS=()
ROUTER_PRIORITY=""

usage() {
  die "usage: $(basename "$0") <instance> [<tag>]   (instances: $(instance_names | tr '\n' ' '))"
}

# running_tag SERVICE -> the tag of the image the service runs now (nothing when it does not exist).
running_tag() {
  local image
  image="$(docker service inspect "$1" --format '{{.Spec.TaskTemplate.ContainerSpec.Image}}' 2>/dev/null || true)"
  image="${image%%@*}"
  [[ "${image}" == *:* ]] || return 0
  printf '%s' "${image##*:}"
}

# in_radix_volume PATH - true when PATH exists in the instance's Radix volume. Asked through a
# container that is never started: the image has no shell to look with.
in_radix_volume() {
  local helper="betula-look-${INSTANCE_STACK}" found=1
  docker rm -f "${helper}" >/dev/null 2>&1 || true
  docker create --name "${helper}" -v "${INSTANCE_STACK}_radix-data:/data" "${RADIX_IMAGE}" >/dev/null
  if docker cp "${helper}:/data/$1" - >/dev/null 2>&1; then
    found=0
  fi
  docker rm -f "${helper}" >/dev/null 2>&1 || true
  return "${found}"
}

# Offline, Radix only serves the snapshot it has, so there has to be one. A volume that was
# seeded and never ran has a database and no snapshot: the snapshot is made from it here, in a
# container without a network (validate + export read nothing but the database).
ensure_snapshot() {
  step "Radix offline: a snapshot to serve"
  if ! docker volume inspect "${INSTANCE_STACK}_radix-data" >/dev/null 2>&1; then
    die "RADIX_CRAWL=off, but instance ${INSTANCE_STACK} has no data: there is no volume ${INSTANCE_STACK}_radix-data. Seed it first (deploy/ship.sh ${INSTANCE_STACK} --seed), or go online once (RADIX_CRAWL=on)"
  fi
  if in_radix_volume snapshot/current.json; then
    log "there is one in ${INSTANCE_STACK}_radix-data"
    return 0
  fi
  if docker service inspect "${INSTANCE_STACK}_radix" >/dev/null 2>&1; then
    die "RADIX_CRAWL=off, but the Radix of ${INSTANCE_STACK} has not exported a snapshot yet. Let it finish its first cycle (docker service logs ${INSTANCE_STACK}_radix), then run this again"
  fi
  in_radix_volume radix.db ||
    die "RADIX_CRAWL=off, but ${INSTANCE_STACK}_radix-data holds neither a snapshot nor a database. Seed it first (deploy/ship.sh ${INSTANCE_STACK} --seed)"
  log "none yet: exporting one from the seeded database (no network, about a minute)"
  docker run --rm --network none --cap-drop ALL -v "${INSTANCE_STACK}_radix-data:/data" "${RADIX_IMAGE}" \
    export --db /data/radix.db --out /data/snapshot ||
    die "the export failed: the database does not pass validation, or it does not fit this release. Nothing was deployed"
}

# check_siblings - another stack routed for this instance's host is only allowed as the other colour
# of blue-green: an instance whose own file names the same host, both offline, both closed or both
# open. Two Radix that crawl would ask the university for everything twice, and a switch between
# the colours must not open or close the site. Sets SIBLINGS.
check_siblings() {
  local other
  local -a self=("${INSTANCE_STACK}" "${INSTANCE_HOST}" "${INSTANCE_GATE}" "${INSTANCE_CRAWL}")
  SIBLINGS=()
  while IFS= read -r other; do
    [[ -n "${other}" && "${other}" != "${self[0]}" ]] || continue
    [[ -f "${STACKS_DIR}/${other}.env" ]] ||
      die "https://${self[1]} is served by stack ${other} already, which is no instance (no stacks/${other}.env): two routers for one name would take turns"
    # Overwrites the INSTANCE_* values of this instance; they are restored below.
    load_instance "${other}"
    [[ "${INSTANCE_HOST}" == "${self[1]}" ]] ||
      die "stack ${other} is routed for https://${self[1]}, but ${other}.env names ${INSTANCE_HOST}: bash ${BETULA_ROOT}/vps/50-app.sh ${other} first"
    [[ "${self[3]}" == "off" && "${INSTANCE_CRAWL}" == "off" ]] ||
      die "stack ${other} serves https://${self[1]} already. Two colours of one site need RADIX_CRAWL=off in both files (${self[0]}.env: ${self[3]}, ${other}.env: ${INSTANCE_CRAWL}): two Radix that crawl would ask the university for everything twice"
    [[ "${INSTANCE_GATE}" == "${self[2]}" ]] ||
      die "FOLIA_ACCESS_GATE is ${self[2]} in ${self[0]}.env and ${INSTANCE_GATE} in ${other}.env: switching between the two would open or close the site"
    SIBLINGS+=("${other}")
  done < <(app_stacks_for_host "${self[1]}")
  INSTANCE_STACK="${self[0]}" INSTANCE_HOST="${self[1]}" INSTANCE_GATE="${self[2]}" INSTANCE_CRAWL="${self[3]}"
}

# priority_to_deploy -> the priority of this instance's router. A deployed web server keeps its
# own (label unset = 0 = Traefik's default); a new one next to a sibling starts as its standby.
priority_to_deploy() {
  local value
  if docker service inspect "${INSTANCE_STACK}_folia" >/dev/null 2>&1; then
    value="$(service_label "${INSTANCE_STACK}_folia" "traefik.http.routers.${INSTANCE_STACK}-folia.priority")"
    printf '%s' "${value:-0}"
  elif [[ "${#SIBLINGS[@]}" -gt 0 ]]; then
    printf '%s' "${STANDBY_PRIORITY}"
  else
    printf '0'
  fi
}

# ---------------------------------------------------------------- steps

preflight() {
  step "Preconditions"
  local facts image owner
  if [[ "${EUID}" -eq 0 && -n "${SUDO_USER:-}" ]]; then
    die "run this as ${DEPLOY_USER} WITHOUT sudo: docker stack deploy reads the instance's values from the environment, and sudo resets it"
  fi
  [[ "${BETULA_VPS_DIR}" == "${BETULA_ROOT}/vps" ]] ||
    die "this copy lives in ${BETULA_VPS_DIR}; the stack files are read from ${STACKS_DIR}, so run ${BETULA_ROOT}/vps/$(basename "$0")"
  [[ "${CONVERGE_TIMEOUT}" =~ ^[0-9]{1,5}$ ]] || die "CONVERGE_TIMEOUT='${CONVERGE_TIMEOUT}' is not a number of seconds"
  require_cmd docker ip
  require_swarm_manager
  facts="$(docker network inspect edge --format '{{.Driver}} {{.Scope}} {{.Attachable}}' 2>/dev/null || true)"
  [[ "${facts}" == "overlay swarm true" ]] || die "overlay network edge is missing or not attachable (run vps/30-docker.sh)"
  stack_exists edge || die "stack edge is not deployed: nothing would route to the application (run vps/40-stacks.sh)"

  log "instance ${INSTANCE_STACK}: https://${INSTANCE_HOST}, closed testing ${INSTANCE_GATE}, crawling ${INSTANCE_CRAWL}, release ${TAG}"
  for image in "${RADIX_IMAGE}" "${FOLIA_IMAGE}"; do
    docker image inspect "${image}" >/dev/null 2>&1 ||
      die "image ${image} is not loaded on this server (deploy/ship.sh builds and loads it; loaded: $(docker image ls --format '{{.Repository}}:{{.Tag}}' "${image%%:*}" | tr '\n' ' '))"
  done

  resolves_here "${INSTANCE_HOST}" ||
    die "${RESOLVE_DETAIL}: create the DNS record first. A router for this name would make Traefik order a certificate that cannot be validated"
  log "${RESOLVE_DETAIL}"

  check_siblings
  ROUTER_PRIORITY="$(priority_to_deploy)"
  [[ "${ROUTER_PRIORITY}" =~ ^[0-9]{1,9}$ ]] ||
    die "the router of ${INSTANCE_STACK}_folia has the priority '${ROUTER_PRIORITY}', which is not a number (docker service inspect ${INSTANCE_STACK}_folia)"
  if [[ "${#SIBLINGS[@]}" -gt 0 ]]; then
    owner="$(app_stack_for_host "${INSTANCE_HOST}")"
    log "blue-green: https://${INSTANCE_HOST} is routed to ${SIBLINGS[*]} as well; it is served by ${owner}, and this deploy does not change that (router priority ${ROUTER_PRIORITY})"
  fi

  # Needed while the gate is off as well: the stack file always names it, so that closing the
  # site again is one value in the instance's file and nothing else.
  secret_exists "${GATE_SECRET}" ||
    die "the swarm secret ${GATE_SECRET} (the password of closed testing) does not exist. Create it with the password on stdin, never in argv: <password manager CLI> | ssh betula docker secret create ${GATE_SECRET} -"
  if [[ "${INSTANCE_GATE}" == "off" ]]; then
    warn "FOLIA_ACCESS_GATE=off: https://${INSTANCE_HOST} will be open to everybody"
  fi
}

deploy_app() {
  step "Stack ${INSTANCE_STACK} (Radix, Folia)"
  local -a files=("${APP_FILE}")
  if secret_exists "${GEMINI_SECRET}"; then
    log "secret ${GEMINI_SECRET} exists: adding ${GEMINI_FILE##*/}"
    files+=("${GEMINI_FILE}")
  else
    log "${GEMINI_FILE##*/} is left out (no secret ${GEMINI_SECRET}): everything runs but \"radix scan-curriculum\""
  fi
  if [[ "${INSTANCE_CRAWL}" == "off" ]]; then
    # Last, so that its command and its health URL win over the files before it.
    log "RADIX_CRAWL=off: adding ${OFFLINE_FILE##*/} (Radix sends nothing to the university and serves the snapshot it has)"
    files+=("${OFFLINE_FILE}")
  fi
  # Substituted into the stack files by "docker stack deploy".
  export STACK_NAME="${INSTANCE_STACK}" APP_HOST="${INSTANCE_HOST}" FOLIA_ACCESS_GATE="${INSTANCE_GATE}" RADIX_IMAGE FOLIA_IMAGE ROUTER_PRIORITY
  # never: the default asks a registry for the digest of the tag, and no registry knows these images.
  STACK_DEPLOY_ARGS=(--resolve-image never)
  deploy_stack "${INSTANCE_STACK}" "${files[@]}"
  # What swarm was told, not what this script meant: the substitution of "docker stack deploy"
  # has surprises (see the head of stacks/betula.yml), and a wrong image name is retried forever.
  local svc image
  for svc in radix folia; do
    image="$(docker service inspect "${INSTANCE_STACK}_${svc}" --format '{{.Spec.TaskTemplate.ContainerSpec.Image}}' 2>/dev/null || true)"
    [[ "${image%%@*}" == "betula-${svc}:${TAG}" ]] ||
      die "service ${INSTANCE_STACK}_${svc} was given the image '${image}' instead of betula-${svc}:${TAG}: look at the image line of ${APP_FILE}"
  done
  # Which colour gets the host's traffic depends on it (blue-green).
  local priority
  priority="$(service_label "${INSTANCE_STACK}_folia" "traefik.http.routers.${INSTANCE_STACK}-folia.priority")"
  [[ "${priority}" == "${ROUTER_PRIORITY}" ]] ||
    die "the router of ${INSTANCE_STACK}_folia was given the priority '${priority}' instead of ${ROUTER_PRIORITY}: look at the priority label of ${APP_FILE}"
  # The same for the promise that matters to somebody else: offline means that swarm starts
  # "serve-snapshot", online that it starts the image's own "run".
  local args
  args="$(docker service inspect "${INSTANCE_STACK}_radix" --format '{{join .Spec.TaskTemplate.ContainerSpec.Args " "}}' 2>/dev/null || true)"
  if [[ "${INSTANCE_CRAWL}" == "off" ]]; then
    [[ "${args}" == serve-snapshot* ]] || die "RADIX_CRAWL=off, but ${INSTANCE_STACK}_radix was given the command \"${args:-run}\": it would crawl. Look at ${OFFLINE_FILE}"
  else
    [[ -z "${args}" ]] || die "RADIX_CRAWL=on, but ${INSTANCE_STACK}_radix was given the command \"${args}\" instead of the run of the image"
  fi
  wait_for_stack "${INSTANCE_STACK}"
}

report() {
  step "Done"
  docker stack services "${INSTANCE_STACK}" --format '{{.Name}}  {{.Replicas}}  {{.Image}}' | sed 's/^/  /'
  if [[ "${#FAILED_STACKS[@]}" -gt 0 ]]; then
    die "not converged: ${FAILED_STACKS[*]} (docker service ps output above). A failed update was rolled back by swarm; fix the cause and run this script again"
  fi
  if [[ "${INSTANCE_GATE}" == "on" ]]; then
    log "https://${INSTANCE_HOST} asks for the access password (closed testing)"
  fi
  if [[ "${INSTANCE_CRAWL}" == "off" ]]; then
    log "Radix is offline: nothing is fetched from the university, the catalog stays as it was exported (RADIX_CRAWL=on in ${INSTANCE_STACK}.env brings it back)"
  else
    log "a Radix without a snapshot needs a cycle for its first one (minutes with a seeded database, hours without); until then the site says that the catalog is not available yet"
  fi
  if [[ "${INSTANCE_HOST}" == "${SITE_HOST}" ]] && stack_exists placeholder; then
    log "the application owns https://${SITE_HOST} now; once it works: docker stack rm placeholder"
  fi
  if [[ "${#SIBLINGS[@]}" -gt 0 ]]; then
    local owner
    owner="$(app_stack_for_host "${INSTANCE_HOST}")"
    if [[ "${owner}" == "${INSTANCE_STACK}" ]]; then
      log "blue-green: ${INSTANCE_STACK} serves https://${INSTANCE_HOST}, ${SIBLINGS[*]} is the standby (the host goes to it with: bash ${BETULA_ROOT}/vps/55-switch.sh ${SIBLINGS[0]})"
    else
      log "blue-green: ${INSTANCE_STACK} is the standby, https://${INSTANCE_HOST} is still served by ${owner}. Check it (91-verify-stacks.sh asks it directly), then hand the host over: bash ${BETULA_ROOT}/vps/55-switch.sh ${INSTANCE_STACK}"
    fi
  fi
  log "next: bash ${BETULA_ROOT}/vps/91-verify-stacks.sh services app"
}

# ---------------------------------------------------------------- main

[[ "$#" -ge 1 && "$#" -le 2 ]] || usage
load_instance "$1"

TAG="${2:-}"
if [[ -z "${TAG}" ]]; then
  TAG="$(running_tag "${INSTANCE_STACK}_folia")"
  [[ -n "${TAG}" ]] || die "instance ${INSTANCE_STACK} is not deployed yet, so there is no release to keep: name a tag (docker image ls 'betula-*')"
  [[ "$(running_tag "${INSTANCE_STACK}_radix")" == "${TAG}" ]] ||
    die "the services of ${INSTANCE_STACK} run different releases ($(running_tag "${INSTANCE_STACK}_radix") / ${TAG}): name the tag to deploy"
fi
[[ "${TAG}" =~ ${TAG_PATTERN} && "${TAG}" != "latest" ]] ||
  die "'${TAG}' is not a release tag. Never \"latest\": swarm compares the service definition, not the image content, so a re-loaded \"latest\" restarts nothing"
RADIX_IMAGE="betula-radix:${TAG}"
FOLIA_IMAGE="betula-folia:${TAG}"

preflight
if [[ "${INSTANCE_CRAWL}" == "off" ]]; then
  ensure_snapshot
fi
deploy_app
report
