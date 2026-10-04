#!/usr/bin/env bash
# cortex-seed.sh - give Cortex (stacks/cortex.yml) the raw pages a Radix has archived, so that it
# serves them as if it had fetched them itself: offline too, which is how the canary's colours
# read it (RADIX_CRAWL=cortex-offline). Run on the server as the deploy user, WITHOUT sudo (it asks
# sudo for the copy of the database alone, as canary-agent.sh does):
#
#   bash /opt/betula/vps/cortex-seed.sh                    # the public site's archive, with the canary's Radix
#   bash /opt/betula/vps/cortex-seed.sh betula <tag>       # another instance's, with a loaded release
#   CORTEX_SEED_ARGS="--dry-run" bash /opt/betula/vps/cortex-seed.sh   # count only
#
# What it does:
#   1. The instance's radix.db (default: of the instances of https://betula.app, the one whose
#      Radix crawls; when none crawls, the one that serves) is copied by the host's sqlite3,
#      read-only, as one transaction (VACUUM INTO), while its Radix writes on: the instance is
#      neither stopped nor slowed down (canary-agent.sh, copy_public_database).
#   2. "radix seed-cortex" of a loaded release (default: the one the canary serves with) reads the
#      copy as it is and gives Cortex every archived page whose page is the whole answer of its URL,
#      with the archive's times (docs/radix/operations.md, "One-off commands"; docs/cortex/cortex.md
#      §4.3). It runs in a container on the internal network "cortex", with nothing else: it
#      reaches Cortex and nothing on the internet.
#   3. The copy is removed.
# Run again whenever it is worth it: what Cortex has, or has newer, changes nothing, so a second
# run writes only what the archive got since. A release from before seed-cortex is refused.
#
# Environment (optional): CORTEX_SEED_ARGS  more flags of seed-cortex (--workers N, --sources ...,
# --dry-run).
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
# shellcheck source=lib-stacks.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-stacks.sh"
betula_init

SQLITE=/usr/bin/sqlite3
CORTEX_URLS="http://cortex_a:8100,http://cortex_b:8100"
WORK=""

usage() {
  die "usage: $(basename "$0") [<instance> [<tag>]]   (instances: $(instance_names | tr '\n' ' '))"
}

remove_work_dir() {
  if [[ -n "${WORK}" && -d "${WORK}" ]]; then
    rm -rf -- "${WORK}" 2>/dev/null || sudo -n rm -rf -- "${WORK}" || true
  fi
}
add_exit_hook remove_work_dir

# public_source -> of the instances of https://betula.app, the one whose Radix crawls (its archive
# is the newest), else the one that serves.
public_source() {
  local name
  local -a crawling=()
  while IFS= read -r name; do
    [[ -n "${name}" ]] || continue
    ! is_canary_colour "${name}" || continue
    load_instance "${name}"
    [[ "${INSTANCE_HOST}" == "${SITE_HOST}" ]] || continue
    if radix_crawls "${name}"; then
      crawling+=("${name}")
    fi
  done < <(instance_names)
  if [[ "${#crawling[@]}" -gt 1 ]]; then
    die "the Radix of ${crawling[*]} all crawl: which archive is the public site's? Name the instance"
  elif [[ "${#crawling[@]}" -eq 1 ]]; then
    printf '%s' "${crawling[0]}"
  else
    app_stack_for_host "${SITE_HOST}"
  fi
}

# radix_tag_of STACK -> the tag of the image its Radix runs (nothing when it does not run).
radix_tag_of() {
  local image
  image="$(docker service inspect "$1_radix" --format '{{.Spec.TaskTemplate.ContainerSpec.Image}}' 2>/dev/null || true)"
  image="${image%%@*}"
  [[ "${image}" == *:* ]] || return 0
  printf '%s' "${image##*:}"
}

# ---------------------------------------------------------------- main

[[ "$#" -le 2 ]] || usage
require_ubuntu
require_cmd docker sudo
require_swarm_manager
[[ -x "${SQLITE}" ]] || die "${SQLITE} is missing (sudo apt-get install sqlite3; vps/60-canary.sh installs it)"

step "What goes where"
SOURCE="${1:-}"
if [[ -z "${SOURCE}" ]]; then
  SOURCE="$(public_source)"
  [[ -n "${SOURCE}" ]] || die "no instance of https://${SITE_HOST} is deployed: name the instance whose archive Cortex gets"
fi
load_instance "${SOURCE}"
TAG="${2:-}"
if [[ -z "${TAG}" ]]; then
  load_instance "${CANARY_COLOURS[0]}"
  canary="$(app_stack_for_host "${INSTANCE_HOST}")"
  [[ -n "${canary}" ]] || die "the canary serves nothing, so there is no release to take: name the tag (docker image ls betula-radix)"
  TAG="$(radix_tag_of "${canary}")"
  load_instance "${SOURCE}"
fi
[[ "${TAG}" =~ ^[A-Za-z0-9][A-Za-z0-9_.-]{0,100}$ && "${TAG}" != "latest" ]] || die "'${TAG}' is not a release tag"
IMAGE="betula-radix:${TAG}"
docker image inspect "${IMAGE}" >/dev/null 2>&1 || die "image ${IMAGE} is not loaded on this server (docker image ls betula-radix)"
help="$(docker run --rm --network none --cap-drop ALL "${IMAGE}" seed-cortex -h 2>&1 || true)"
grep -qxF 'Usage of seed-cortex:' <<<"${help}" ||
  die "release ${TAG} of Radix has no seed-cortex (one from before 2026-10-04): name a newer tag (docker image ls betula-radix)"
cortex_look || die "Cortex does not run (${CORTEX_DETAIL}): deploy/ship-cortex.sh, or bash ${BETULA_ROOT}/vps/48-cortex.sh"
facts="$(docker network inspect cortex --format '{{.Internal}} {{.Attachable}}' 2>/dev/null || true)"
[[ "${facts}" == "true true" ]] || die "the network cortex is missing, or not internal and attachable ('${facts}'): sudo bash ${BETULA_ROOT}/vps/30-docker.sh"
log "the archive of ${INSTANCE_STACK} goes to Cortex (${CORTEX_DETAIL}), read by ${IMAGE}"

step "A copy of ${INSTANCE_STACK}_radix-data/radix.db"
volume="${INSTANCE_STACK}_radix-data"
mountpoint="$(docker volume inspect --format '{{.Mountpoint}}' "${volume}" 2>/dev/null || true)"
[[ "${mountpoint}" =~ ^/var/lib/docker/volumes/[A-Za-z0-9][A-Za-z0-9_.-]*/_data$ ]] ||
  die "volume ${volume} is not a local volume below /var/lib/docker (${mountpoint:-no such volume})"
sudo -n /usr/bin/test -s "${mountpoint}/radix.db" || die "${volume} holds no radix.db (or sudo refused: this needs ${DEPLOY_USER}'s sudo)"
# /var/tmp: the database alone has 170 MB.
WORK="$(mktemp -d /var/tmp/betula-cortex-seed.XXXXXXXX)"
chmod 0755 "${WORK}"
started=${SECONDS}
sudo -n "${SQLITE}" -readonly -bail -batch -cmd '.timeout 120000' "${mountpoint}/radix.db" "VACUUM INTO '${WORK}/radix.db'" ||
  die "sqlite3 could not copy ${volume}/radix.db (${INSTANCE_STACK} is not affected)"
sudo -n /usr/bin/chown "$(id -u):$(id -g)" "${WORK}/radix.db"
chmod 0644 "${WORK}/radix.db"
check="$("${SQLITE}" -readonly -bail -batch "${WORK}/radix.db" 'PRAGMA quick_check;' 2>&1 || true)"
[[ "${check}" == "ok" ]] || die "the copy of ${volume}/radix.db fails the integrity check: ${check}"
log "copied in $((SECONDS - started)) s: $(du -h "${WORK}/radix.db" | cut -f1), schema $("${SQLITE}" -readonly -bail -batch "${WORK}/radix.db" 'PRAGMA user_version;'), integrity ok"
log "archived pages: $("${SQLITE}" -readonly -bail -batch "${WORK}/radix.db" "SELECT group_concat(source || ' ' || n, ', ') FROM (SELECT source, COUNT(*) AS n FROM raw_page GROUP BY source ORDER BY source);")"

step "radix seed-cortex"
# Word splitting is the interface of CORTEX_SEED_ARGS.
read -r -a extra <<<"${CORTEX_SEED_ARGS:-}"
docker run --rm --network cortex --cap-drop ALL --read-only --tmpfs /tmp -v "${WORK}:/seed:ro" "${IMAGE}" \
  seed-cortex --db /seed/radix.db --cortex "${CORTEX_URLS}" --log-format text ${extra[@]+"${extra[@]}"} ||
  die "seed-cortex did not end well (above: what Cortex did not take). Running it again is safe"

step "Done"
log "Cortex has the archive of ${INSTANCE_STACK} as of now; the dashboard \"Cortex\" counts it (Imported answers by result)"
log "who leads, what the store holds: docker exec \"\$(docker ps -q -f name=cortex_a | head -n 1)\" /bin/cortex status"
