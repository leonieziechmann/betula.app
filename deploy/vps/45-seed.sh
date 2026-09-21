#!/usr/bin/env bash
# 45-seed.sh - give an instance of the application a Radix database BEFORE its first deploy, so
# that the server does not crawl the university's servers for everything the workstation has
# already (a fresh Radix needs hours, and thousands of requests, for that). Run on the server as
# the deploy user, WITHOUT sudo, with the database as a gzipped tar on stdin; "deploy/ship.sh
# <instance> --seed" does exactly that:
#
#   tar -cf - radix.db | gzip | ssh betula bash /opt/betula/vps/45-seed.sh canary <tag> <bytes of radix.db>
#
# The tar holds radix.db (and radix.db-wal, if SQLite left one). <tag> names a loaded Radix image:
# a container of it that is never started lends its /data to "docker cp", because the image has
# no shell and no tar. Radix writes a snapshot from the database in its first cycle by itself.
#
# Refuses to touch an instance that is deployed or whose volume holds a database already: seeding
# is for the first deploy. Starting over on purpose is three commands:
#   docker stack rm <instance> ; docker volume rm <instance>_radix-data ; then seed again.
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
# shellcheck source=lib-stacks.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-stacks.sh"
betula_init
require_ubuntu

[[ "$#" -eq 3 ]] || die "usage: $(basename "$0") <instance> <tag> <bytes of radix.db>   (the database comes on stdin: tar, gzip)"
load_instance "$1"
TAG="$2"
EXPECTED_BYTES="$3"
[[ "${TAG}" =~ ^[A-Za-z0-9][A-Za-z0-9_.-]{0,100}$ ]] || die "'${TAG}' is not a release tag"
[[ "${EXPECTED_BYTES}" =~ ^[0-9]{1,15}$ ]] || die "'${EXPECTED_BYTES}' is not a number of bytes"
[[ ! -t 0 ]] || die "the database has to come on stdin (tar, gzip); see the head of this script"

IMAGE="betula-radix:${TAG}"
VOLUME="${INSTANCE_STACK}_radix-data"
HELPER="betula-seed-${INSTANCE_STACK}"

require_cmd docker gzip tar
require_swarm_manager
docker image inspect "${IMAGE}" >/dev/null 2>&1 || die "image ${IMAGE} is not loaded on this server (deploy/ship.sh loads it before it seeds)"
if docker service inspect "${INSTANCE_STACK}_radix" >/dev/null 2>&1; then
  die "instance ${INSTANCE_STACK} is deployed: its Radix owns the database. Seeding is for the first deploy (starting over on purpose: see the head of this script)"
fi

remove_helper() { docker rm -f "${HELPER}" >/dev/null 2>&1 || true; }
add_exit_hook remove_helper
remove_helper

step "Volume ${VOLUME}"
if docker volume inspect "${VOLUME}" >/dev/null 2>&1; then
  log "exists already"
else
  # The label "docker stack deploy" would give it, so the volume reads as part of the stack.
  docker volume create --label "com.docker.stack.namespace=${INSTANCE_STACK}" "${VOLUME}" >/dev/null
  log "created"
fi
docker create --name "${HELPER}" -v "${VOLUME}:/data" "${IMAGE}" >/dev/null
if docker cp "${HELPER}:/data/radix.db" - >/dev/null 2>&1; then
  die "${VOLUME} holds a radix.db already; it is not replaced (starting over on purpose: see the head of this script)"
fi

step "Database"
# Unpacked here first, and packed again as root's files with mode 0644: docker cp keeps the owner
# a tar names, and a tar from a workstation names the workstation's user (from Git Bash: uid
# 197609). Radix runs as root WITHOUT capabilities (cap_drop ALL), so a file of somebody else is
# read-only to it: "attempt to write a readonly database" (seen on the first seeded deploy,
# 2026-09-21). /var/tmp: /tmp may be memory, and the database has 150 MB.
# pipefail: a broken upload fails here and not at the size check.
STAGE="$(mktemp -d /var/tmp/betula-seed.XXXXXXXX)"
remove_stage() { rm -rf -- "${STAGE}"; }
add_exit_hook remove_stage
gzip -dc | tar -xf - -C "${STAGE}" --no-same-owner --no-same-permissions
[[ -f "${STAGE}/radix.db" ]] || die "the upload holds no radix.db (it holds: $(ls -A "${STAGE}" | tr '\n' ' '))"
members=(radix.db)
if [[ -f "${STAGE}/radix.db-wal" ]]; then
  members+=(radix.db-wal)
fi
tar -cf - -C "${STAGE}" --owner=0 --group=0 --mode=0644 "${members[@]}" | docker cp - "${HELPER}:/data/"
listing="$(docker cp "${HELPER}:/data/radix.db" - | tar --numeric-owner -tvf -)"
have="$(awk '$NF == "radix.db" { print $3 }' <<<"${listing}")"
owner="$(awk '$NF == "radix.db" { print $2 }' <<<"${listing}")"
if [[ "${have}" != "${EXPECTED_BYTES}" || "${owner}" != "0/0" ]]; then
  remove_helper
  docker volume rm "${VOLUME}" >/dev/null 2>&1 || true
  die "radix.db arrived with ${have:-no} bytes (expected ${EXPECTED_BYTES}) and owner ${owner:-?} (expected 0/0); the volume was removed again (remove it by hand if that failed: docker volume rm ${VOLUME})"
fi
log "radix.db is in ${VOLUME} (${have} bytes, owner ${owner}). Next: bash ${BETULA_ROOT}/vps/50-app.sh ${INSTANCE_STACK} ${TAG}"
