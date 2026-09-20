#!/usr/bin/env bash
# 50-app.sh - deploy one instance of the application (stacks/betula.yml) from images that are
# loaded on this server already. Run on the server as the deploy user, WITHOUT sudo; deploy/ship.sh
# does that after it has built and loaded the images:
#
#   bash /opt/betula/vps/50-app.sh canary 2026-09-21-ab12cd3   # this release
#   bash /opt/betula/vps/50-app.sh canary                      # the release that runs now (after a
#                                                              # change to canary.env or betula.yml)
#
# An instance is a file stacks/<instance>.env: the name of its stack, its public host name and
# whether the site asks for the password of closed testing. Rollback = the previous tag again; it
# is still loaded (docker image ls 'betula-*').
#
# Idempotent: swarm only touches a service whose definition changed. Nothing is deployed unless
# the images exist, the host name resolves to this machine (a router for a name that does not
# makes Traefik order a certificate that cannot be validated; Let's Encrypt counts those, 5 per
# host name per hour) and the secrets exist. The value of a secret never passes through here.
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
GEMINI_SECRET="gemini-api-key"
GATE_SECRET="folia-access-password"
TAG_PATTERN='^[A-Za-z0-9][A-Za-z0-9_.-]{0,100}$'

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

  log "instance ${INSTANCE_STACK}: https://${INSTANCE_HOST}, closed testing ${INSTANCE_GATE}, release ${TAG}"
  for image in "${RADIX_IMAGE}" "${FOLIA_IMAGE}"; do
    docker image inspect "${image}" >/dev/null 2>&1 ||
      die "image ${image} is not loaded on this server (deploy/ship.sh builds and loads it; loaded: $(docker image ls --format '{{.Repository}}:{{.Tag}}' "${image%%:*}" | tr '\n' ' '))"
  done

  resolves_here "${INSTANCE_HOST}" ||
    die "${RESOLVE_DETAIL}: create the DNS record first. A router for this name would make Traefik order a certificate that cannot be validated"
  log "${RESOLVE_DETAIL}"

  owner="$(app_stack_for_host "${INSTANCE_HOST}")"
  [[ -z "${owner}" || "${owner}" == "${INSTANCE_STACK}" ]] ||
    die "https://${INSTANCE_HOST} is served by stack ${owner} already; two routers for one name would take turns"

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
  # Substituted into the stack files by "docker stack deploy".
  export STACK_NAME="${INSTANCE_STACK}" APP_HOST="${INSTANCE_HOST}" FOLIA_ACCESS_GATE="${INSTANCE_GATE}" RADIX_IMAGE FOLIA_IMAGE
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
  log "a fresh Radix needs hours for its first snapshot; until then the site says that the catalog is not available yet"
  if [[ "${INSTANCE_HOST}" == "${SITE_HOST}" ]] && stack_exists placeholder; then
    log "the application owns https://${SITE_HOST} now; once it works: docker stack rm placeholder"
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
deploy_app
report
