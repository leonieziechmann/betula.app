#!/usr/bin/env bash
# canary-agent.sh - keeps https://canary.betula.app on the newest build of master, with the data of
# the public site. Runs on the server as the deploy user; betula-canary.timer starts it every two
# minutes (vps/60-canary.sh installs both, README.md section 12):
#
#   canary-agent.sh poll                                   # the timer: a new build of master? deploy it
#   bash /opt/betula/vps/canary-agent.sh deploy [<tag>]    # by hand: a loaded release (default: the
#                                                          # one canary runs) again, with fresh data
#   bash /opt/betula/vps/canary-agent.sh status            # what serves, what came last
#
# A build is the artifact betula-images-<tag>.tar of .github/workflows/images.yml. The server
# fetches it itself: GitHub holds nothing that reaches this machine, and the token the agent gets
# from systemd can only read this repository's workflow runs and artifacts. Only a successful run
# of that workflow, started by a push to master of this repository, counts; the file has to match
# the digest GitHub keeps for it and the sha256 sums it carries. The agent never takes a script
# from GitHub: deploy/ still reaches /opt/betula only through deploy/sync.sh.
#
# A deploy, in this order. The colour that serves is only touched once its successor serves:
#   1. The images are loaded and given the release tag, as deploy/ship.sh does.
#   2. The colour of the canary that does not serve (CANARY_COLOURS, lib-stacks.sh) is removed,
#      with its volumes.
#   3. Its new database is a copy of the public site's: of the instances of betula.app, the one
#      whose Radix crawls. The host's own sqlite3 (Ubuntu's, not a binary of the release) reads it
#      read-only, as one transaction (VACUUM INTO), while that Radix writes on. Nothing of a new
#      release ever opens the public site's database, so no migration can reach it.
#   4. vps/45-seed.sh puts the copy into the colour's volume; vps/50-app.sh builds and exports the
#      catalog with the new release (its migrations run on the copy, in a container without a
#      network) and deploys the colour as the standby.
#   5. Once its web server answers /healthz, vps/55-switch.sh hands the host over, and the agent
#      waits until the new colour has settled and still answers.
#   6. The colour that served before is removed with its volumes: canary keeps no backup. Images
#      this agent loaded and nothing uses any more are removed, but the newest three.
# A failure before 6 leaves the canary that serves as it is (the half-made colour stays for a look;
# the next deploy removes it). A release is tried three times, ten minutes apart and longer; then
# the agent waits for the next build of master.
#
# poll only acts on a build it has not seen: a release deployed by hand (ship.sh, or "deploy" here)
# stays until master is built again. Working on canary by hand: sudo bash 60-canary.sh off first.
#
# Environment (all optional):
#   CANARY_SEED_FROM=<instance>   take the database of this instance instead (not a canary colour)
#   CANARY_HEALTH_TIMEOUT=600     seconds the new colour has to answer /healthz after its deploy
#   CANARY_GITHUB_API             the API's address (tests; default https://api.github.com)
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
# shellcheck source=lib-stacks.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-stacks.sh"
betula_init

# Where the builds come from (lib-stacks.sh): .github/workflows/images.yml, pushes to master.
REPO="${CANARY_REPO}"
WORKFLOW="${CANARY_WORKFLOW}"
BRANCH="${CANARY_BRANCH}"
GITHUB_API="${CANARY_GITHUB_API:-https://api.github.com}"
# The token, as systemd hands it over (LoadCredentialEncrypted= in betula-canary.service).
TOKEN_NAME="github-token"

STATE_DIR="${STATE_DIRECTORY:-/var/lib/betula-canary}"
MAX_ATTEMPTS=3
# Minutes after a failed attempt before the next one, times the number of failures so far.
RETRY_MINUTES=10
KEEP_IMAGES=3
HEALTH_TIMEOUT="${CANARY_HEALTH_TIMEOUT:-600}"
# Swarm stops Radix within its stop_grace_period (60 s) and Folia within 30 s.
REMOVE_TIMEOUT=180
# Both images together are about a hundred megabytes; anything near this is not a build of ours.
ARTIFACT_MAX_BYTES=$((2 * 1024 * 1024 * 1024))
TAG_PATTERN='^[A-Za-z0-9][A-Za-z0-9_.-]{0,100}$'
ARTIFACT_PATTERN='^betula-images-([A-Za-z0-9][A-Za-z0-9_.-]{0,100})\.tar$'
SQLITE=/usr/bin/sqlite3

# Set by check_setup: the host name both canary colours serve.
CANARY_HOST=""
# Set by latest_run and run_artifact.
BUILD_RUN=""
BUILD_TAG=""
BUILD_ARTIFACT=""
BUILD_DIGEST=""
BUILD_BYTES=""
# Set by seed_source and copy_public_database.
SEED_SOURCE=""
SEED_SCHEMA=""
WORK=""
AUTH_HEADERS=""

usage() {
  die "usage: $(basename "$0") poll | deploy [<tag>] | status"
}

# ---------------------------------------------------------------- helpers

# image_tag SERVICE -> the tag of the image the service runs (nothing when it does not exist).
image_tag() {
  local image
  image="$(docker service inspect "$1" --format '{{.Spec.TaskTemplate.ContainerSpec.Image}}' 2>/dev/null || true)"
  image="${image%%@*}"
  [[ "${image}" == *:* ]] || return 0
  printf '%s' "${image##*:}"
}

images_loaded() {
  docker image inspect "betula-radix:$1" >/dev/null 2>&1 && docker image inspect "betula-folia:$1" >/dev/null 2>&1
}

make_work_dir() {
  [[ -z "${WORK}" ]] || return 0
  # /var/tmp: /tmp may be memory, and the database alone has 150 MB. Private to the service
  # anyway (PrivateTmp= in betula-canary.service).
  WORK="$(mktemp -d /var/tmp/betula-canary.XXXXXXXX)"
  chmod 0700 "${WORK}"
  [[ "${WORK}" =~ ^/var/tmp/betula-canary\.[A-Za-z0-9]+$ ]] || die "unexpected temp directory ${WORK}"
}
remove_work_dir() {
  if [[ -n "${WORK}" && -d "${WORK}" ]]; then
    # The copy of the database belongs to deploy by then; if chown did not happen, root's file stays.
    rm -rf -- "${WORK}" 2>/dev/null || sudo -n rm -rf -- "${WORK}" || true
  fi
}
add_exit_hook remove_work_dir

# state_get NAME -> the content of a state file (nothing when it does not exist).
state_get() {
  [[ -f "${STATE_DIR}/$1" ]] || return 0
  cat -- "${STATE_DIR}/$1"
}
# state_set NAME VALUE - replaces a state file atomically.
state_set() {
  printf '%s\n' "$2" >"${STATE_DIR}/.$1.new"
  mv -f -- "${STATE_DIR}/.$1.new" "${STATE_DIR}/$1"
}

now_utc() { date -u +%Y-%m-%dT%H:%M:%SZ; }

# ---------------------------------------------------------------- preconditions

check_setup() {
  local colour host=""
  if [[ "${EUID}" -eq 0 && -n "${SUDO_USER:-}" ]]; then
    die "run this as ${DEPLOY_USER} WITHOUT sudo (the scripts it runs read the instance's values from the environment, and sudo resets it)"
  fi
  [[ "${BETULA_VPS_DIR}" == "${BETULA_ROOT}/vps" ]] ||
    die "this copy lives in ${BETULA_VPS_DIR}; the scripts it runs are read from ${BETULA_ROOT}/vps, so run ${BETULA_ROOT}/vps/$(basename "$0")"
  [[ "${HEALTH_TIMEOUT}" =~ ^[0-9]{1,5}$ ]] || die "CANARY_HEALTH_TIMEOUT='${HEALTH_TIMEOUT}' is not a number of seconds"
  require_cmd docker curl jq tar gzip sha256sum flock
  require_swarm_manager
  [[ -d "${STATE_DIR}" && -w "${STATE_DIR}" ]] || die "${STATE_DIR} is missing or not writable for $(id -un) (sudo bash ${BETULA_ROOT}/vps/60-canary.sh creates it)"
  for colour in "${CANARY_COLOURS[@]}"; do
    load_instance "${colour}"
    # The copy of the public site's database would make a crawling canary a second crawler of
    # everything the public site's Radix fetches already.
    [[ "${INSTANCE_CRAWL}" == "off" ]] ||
      die "${colour}.env says RADIX_CRAWL=on: a canary seeded with the public site's data must never crawl (the public site's Radix does that). Set it to off"
    [[ "${INSTANCE_HOST}" != "${SITE_HOST}" ]] ||
      die "${colour}.env names ${SITE_HOST}, the public site: the canary colours need a host of their own"
    if [[ -z "${host}" ]]; then
      host="${INSTANCE_HOST}"
    elif [[ "${INSTANCE_HOST}" != "${host}" ]]; then
      die "the canary colours name different hosts (${host}, ${INSTANCE_HOST}): ${CANARY_COLOURS[*]} have to be two colours of one site"
    fi
  done
  CANARY_HOST="${host}"
}

# take_lock - one run at a time: the timer's, or one by hand. A second one leaves quietly.
take_lock() {
  exec 9>"${STATE_DIR}/lock"
  if ! flock -n 9; then
    log "another run of $(basename "$0") is busy; leaving it alone"
    exit 0
  fi
}

# ---------------------------------------------------------------- GitHub

# make_auth_headers - the token goes into a header file that only this user can read: never into
# argv, where every user of the machine could see it.
make_auth_headers() {
  local token
  [[ -n "${CREDENTIALS_DIRECTORY:-}" && -s "${CREDENTIALS_DIRECTORY}/${TOKEN_NAME}" ]] ||
    die "no GitHub token: the timer's service gets it from systemd (store it: README.md section 12). By hand: sudo systemctl start betula-canary.service"
  token="$(tr -d '[:space:]' <"${CREDENTIALS_DIRECTORY}/${TOKEN_NAME}")"
  [[ "${token}" =~ ^github_pat_[A-Za-z0-9_]{20,255}$ ]] ||
    die "the stored GitHub token is not a fine-grained personal access token (github_pat_...): store a new one (README.md section 12)"
  make_work_dir
  AUTH_HEADERS="${WORK}/headers"
  (
    umask 077
    printf 'Authorization: Bearer %s\n' "${token}"
    printf 'Accept: application/vnd.github+json\n'
    printf 'X-GitHub-Api-Version: 2022-11-28\n'
    printf 'User-Agent: betula-canary-agent\n'
  ) >"${AUTH_HEADERS}"
}

# github_get PATH FILE - the API's answer to GET PATH in FILE (its headers in FILE.headers); dies
# unless it is 200.
github_get() {
  local path=$1 file=$2 code
  code="$(curl --proto '=https' --tlsv1.2 -sS --max-time 30 --retry 2 -H @"${AUTH_HEADERS}" \
    -D "${file}.headers" -o "${file}" -w '%{http_code}' "${GITHUB_API}${path}" 2>"${file}.err")" || true
  case "${code}" in
    200) return 0 ;;
    401) die "GitHub answers 401 to ${path}: the token has expired or was revoked. Store a new one (README.md section 12)" ;;
    403 | 404) die "GitHub answers ${code} to ${path}: the token may not read the Actions of ${REPO} (a fine-grained token for this one repository with \"Actions: Read-only\"), or its rate limit is spent: $(head -c 300 "${file}" | tr '\n' ' ')" ;;
    *) die "GitHub answers ${code:-nothing} to ${path}: $(head -c 300 "${file}.err" | tr '\n' ' ')" ;;
  esac
}

# latest_run - sets BUILD_RUN to the newest successful run of the workflow for a push to master
# (empty when there is none yet). Remembers when the token expires (state file token-expires,
# which 91-verify-stacks.sh reads): GitHub says it in a header of every answer.
latest_run() {
  local expires
  BUILD_RUN=""
  github_get "/repos/${REPO}/actions/workflows/${WORKFLOW}/runs?branch=${BRANCH}&event=push&status=success&exclude_pull_requests=true&per_page=20" "${WORK}/runs.json"
  expires="$(sed -n 's/^github-authentication-token-expiration: *\([0-9][0-9 :+UTC-]*\).*$/\1/Ip' "${WORK}/runs.json.headers" | tr -d '\r' | tail -n 1)"
  if [[ -n "${expires}" && "${expires}" != "$(state_get token-expires)" ]]; then
    state_set token-expires "${expires}"
  fi
  # Everything the answer says is checked again: the run is of this file, in this repository,
  # for a push to master (not a pull request from a fork whose branch is called master).
  BUILD_RUN="$(jq -r --arg repo "${REPO}" --arg wf ".github/workflows/${WORKFLOW}" --arg branch "${BRANCH}" '
    [.workflow_runs[]
      | select(.event == "push" and .head_branch == $branch and .status == "completed" and .conclusion == "success"
               and .repository.full_name == $repo and .head_repository.full_name == $repo
               and (.path == $wf or (.path | startswith($wf + "@"))))]
    | max_by(.run_number) | if . == null then "" else .id end' "${WORK}/runs.json")"
  [[ -z "${BUILD_RUN}" || "${BUILD_RUN}" =~ ^[0-9]{1,20}$ ]] || die "GitHub named the run '${BUILD_RUN}'"
}

# run_artifact - the artifact of BUILD_RUN: sets BUILD_TAG, BUILD_ARTIFACT, BUILD_DIGEST and
# BUILD_BYTES. Returns 1 when it has none (any more: it is kept for a day).
run_artifact() {
  local line name
  BUILD_TAG=""
  github_get "/repos/${REPO}/actions/runs/${BUILD_RUN}/artifacts?per_page=100" "${WORK}/artifacts.json"
  # A run that was re-run may hold one per attempt: the newest counts, as for actions/download-artifact.
  line="$(jq -r --argjson run "${BUILD_RUN}" --arg pattern "${ARTIFACT_PATTERN}" '
    [.artifacts[] | select((.name | test($pattern)) and .expired == false and .workflow_run.id == $run)]
    | max_by(.id) | if . == null then "" else "\(.id) \(.name) \(.size_in_bytes) \(.digest // "")" end' "${WORK}/artifacts.json")"
  [[ -n "${line}" ]] || return 1
  read -r BUILD_ARTIFACT name BUILD_BYTES BUILD_DIGEST <<<"${line}"
  [[ "${name}" =~ ${ARTIFACT_PATTERN} ]] || die "the artifact is called '${name}'"
  BUILD_TAG="${BASH_REMATCH[1]}"
  [[ "${BUILD_TAG}" =~ ${TAG_PATTERN} && "${BUILD_TAG}" != "latest" ]] || die "'${BUILD_TAG}' is not a release tag"
  [[ "${BUILD_ARTIFACT}" =~ ^[0-9]{1,20}$ && "${BUILD_BYTES}" =~ ^[0-9]{1,15}$ ]] || die "GitHub describes the artifact of run ${BUILD_RUN} oddly: ${line}"
  [[ "${BUILD_DIGEST}" =~ ^sha256:[0-9a-f]{64}$ ]] || die "GitHub names no sha256 digest for the artifact of run ${BUILD_RUN} ('${BUILD_DIGEST}'): refusing a file that cannot be checked"
  return 0
}

# fetch_images - download the artifact of the build, check it, load its images and tag them.
fetch_images() {
  local answer code url have image loaded
  step "Images of ${BUILD_TAG} (run ${BUILD_RUN} of ${WORKFLOW})"
  if images_loaded "${BUILD_TAG}"; then
    log "betula-radix:${BUILD_TAG} and betula-folia:${BUILD_TAG} are loaded already"
    return 0
  fi
  [[ "${BUILD_BYTES}" -le "${ARTIFACT_MAX_BYTES}" ]] || die "the artifact has ${BUILD_BYTES} bytes: that is not a build of the two images"
  # The API answers with a redirect to a short-lived address of the file. That request goes
  # without the token, and its address (a signature in the query) is handed to curl on stdin.
  answer="$(curl --proto '=https' --tlsv1.2 -sS --max-time 30 -H @"${AUTH_HEADERS}" -o /dev/null \
    -w '%{http_code} %{redirect_url}' "${GITHUB_API}/repos/${REPO}/actions/artifacts/${BUILD_ARTIFACT}/zip" 2>/dev/null)" || true
  code="${answer%% *}"
  url="${answer#* }"
  [[ "${code}" == "302" ]] || die "GitHub answers ${code:-nothing} instead of a redirect to the artifact ${BUILD_ARTIFACT}"
  [[ "${url}" =~ ^https://[^[:space:]\"\\]+$ ]] || die "GitHub redirects the artifact to something that is not an https address"
  printf 'url = "%s"\n' "${url}" | curl --proto '=https' --tlsv1.2 -sS --fail --max-time 1200 \
    --max-filesize "${ARTIFACT_MAX_BYTES}" -o "${WORK}/release.tar" -K - ||
    die "the download of the artifact ${BUILD_ARTIFACT} failed"
  have="$(sha256sum "${WORK}/release.tar" | cut -d ' ' -f 1)"
  [[ "sha256:${have}" == "${BUILD_DIGEST}" ]] ||
    die "the artifact has the sha256 ${have}, GitHub says ${BUILD_DIGEST#sha256:}: not loading it"
  log "downloaded betula-images-${BUILD_TAG}.tar ($(du -h "${WORK}/release.tar" | cut -f1)), sha256 as GitHub keeps it"

  mkdir -- "${WORK}/release"
  # Only the three names, never the archive's owners or modes.
  tar -xf "${WORK}/release.tar" -C "${WORK}/release" --no-same-owner --no-same-permissions -- \
    release.json radix-image.tar.gz folia-image.tar.gz ||
    die "the artifact does not hold release.json, radix-image.tar.gz and folia-image.tar.gz"
  rm -f -- "${WORK}/release.tar"
  for image in release.json radix-image.tar.gz folia-image.tar.gz; do
    [[ -f "${WORK}/release/${image}" && ! -L "${WORK}/release/${image}" ]] || die "${image} in the artifact is not a plain file"
  done
  jq -e --arg tag "${BUILD_TAG}" --arg repo "${REPO}" '.tag == $tag and .repository == $repo' \
    "${WORK}/release/release.json" >/dev/null || die "release.json names another release or repository: $(head -c 300 "${WORK}/release/release.json")"
  for image in radix folia; do
    have="$(sha256sum "${WORK}/release/${image}-image.tar.gz" | cut -d ' ' -f 1)"
    [[ "${have}" == "$(jq -r --arg i "${image}" '.images[$i].sha256' "${WORK}/release/release.json")" ]] ||
      die "${image}-image.tar.gz does not have the sha256 release.json names"
  done

  for image in radix folia; do
    loaded="$(docker load -i "${WORK}/release/${image}-image.tar.gz")"
    # The flake names every build "latest"; the tag is given here, as deploy/ship.sh does, and
    # "latest" itself is never deployed (50-app.sh refuses it).
    [[ "${loaded}" == *"Loaded image: betula-${image}:latest"* ]] ||
      die "${image}-image.tar.gz did not load as betula-${image}:latest: ${loaded}"
    docker tag "betula-${image}:latest" "betula-${image}:${BUILD_TAG}"
    log "loaded betula-${image}:${BUILD_TAG}"
    rm -f -- "${WORK}/release/${image}-image.tar.gz"
  done
  if ! grep -qxF -- "${BUILD_TAG}" <<<"$(state_get loaded)"; then
    printf '%s\n' "${BUILD_TAG}" >>"${STATE_DIR}/loaded"
  fi
}

# ---------------------------------------------------------------- the public site's database

# seed_source - sets SEED_SOURCE to the stack whose database is the public site's: of the deployed
# instances of https://betula.app, the one whose Radix crawls (its data is the newest); when none
# crawls, the one that serves. CANARY_SEED_FROM names another instance.
seed_source() {
  local name args
  local -a crawling=()
  SEED_SOURCE=""
  if [[ -n "${CANARY_SEED_FROM:-}" ]]; then
    load_instance "${CANARY_SEED_FROM}"
    ! is_canary_colour "${CANARY_SEED_FROM}" || die "CANARY_SEED_FROM=${CANARY_SEED_FROM} is a colour of the canary itself"
    docker volume inspect "${CANARY_SEED_FROM}_radix-data" >/dev/null 2>&1 || die "CANARY_SEED_FROM=${CANARY_SEED_FROM}: there is no volume ${CANARY_SEED_FROM}_radix-data"
    SEED_SOURCE="${CANARY_SEED_FROM}"
    return 0
  fi
  while IFS= read -r name; do
    [[ -n "${name}" ]] || continue
    ! is_canary_colour "${name}" || continue
    load_instance "${name}"
    [[ "${INSTANCE_HOST}" == "${SITE_HOST}" ]] || continue
    docker service inspect "${name}_radix" >/dev/null 2>&1 || continue
    args="$(docker service inspect "${name}_radix" --format '{{join .Spec.TaskTemplate.ContainerSpec.Args " "}}' 2>/dev/null || true)"
    # No arguments: the image's own "run", the Radix that crawls (offline it is "serve-snapshot").
    if [[ -z "${args}" ]]; then
      crawling+=("${name}")
    fi
  done < <(instance_names)
  if [[ "${#crawling[@]}" -gt 1 ]]; then
    die "the Radix of ${crawling[*]} all crawl: which database is the public site's? (50-app.sh never allows that; CANARY_SEED_FROM=<instance> decides for one run)"
  elif [[ "${#crawling[@]}" -eq 1 ]]; then
    SEED_SOURCE="${crawling[0]}"
  else
    SEED_SOURCE="$(app_stack_for_host "${SITE_HOST}")"
    [[ -n "${SEED_SOURCE}" ]] || die "no instance of https://${SITE_HOST} is deployed: there is no database to give canary"
  fi
}

# copy_public_database DIR - DIR/radix.db becomes a copy of SEED_SOURCE's database, consistent to one
# transaction, owned by this user. Sets SEED_SCHEMA to its schema version (PRAGMA user_version).
#
# Read by the host's sqlite3 as root (the volume is root's), with -readonly: the database file is
# opened read-only, and VACUUM INTO writes one transaction's view of it into a new file, frames that
# Radix has not yet moved out of its WAL included. Radix is neither stopped nor slowed down: in WAL
# mode a reader does not block the writer. Measured with a writer committing all the time: every
# copy was complete to one commit and passed the integrity check.
copy_public_database() {
  local dir=$1 volume mountpoint check started=${SECONDS}
  volume="${SEED_SOURCE}_radix-data"
  mountpoint="$(docker volume inspect --format '{{.Mountpoint}}' "${volume}" 2>/dev/null || true)"
  [[ "${mountpoint}" =~ ^/var/lib/docker/volumes/[A-Za-z0-9][A-Za-z0-9_.-]*/_data$ ]] ||
    die "volume ${volume} is not a local volume below /var/lib/docker (${mountpoint:-no such volume})"
  [[ -x "${SQLITE}" ]] || die "${SQLITE} is missing (sudo bash ${BETULA_ROOT}/vps/60-canary.sh installs it)"
  [[ "${dir}" =~ ^/var/tmp/betula-canary\.[A-Za-z0-9]+/[a-z]+$ ]] || die "unexpected directory for the copy: ${dir}"
  mkdir -- "${dir}"
  sudo -n /usr/bin/test -s "${mountpoint}/radix.db" || die "${volume} holds no radix.db (or sudo refused: this needs ${DEPLOY_USER}'s sudo)"
  sudo -n "${SQLITE}" -readonly -bail -batch -cmd '.timeout 120000' "${mountpoint}/radix.db" "VACUUM INTO '${dir}/radix.db'" ||
    die "sqlite3 could not copy ${volume}/radix.db (the public site is not affected)"
  sudo -n /usr/bin/chown "$(id -u):$(id -g)" "${dir}/radix.db"
  check="$("${SQLITE}" -readonly -bail -batch "${dir}/radix.db" 'PRAGMA quick_check;' 2>&1 || true)"
  [[ "${check}" == "ok" ]] || die "the copy of ${volume}/radix.db fails the integrity check: ${check}"
  SEED_SCHEMA="$("${SQLITE}" -readonly -bail -batch "${dir}/radix.db" 'PRAGMA user_version;')"
  log "copied ${volume}/radix.db in $((SECONDS - started)) s: $(du -h "${dir}/radix.db" | cut -f1), schema ${SEED_SCHEMA}, integrity ok"
}

# ---------------------------------------------------------------- colours

# remove_colour NAME - the stack of a canary colour and its volumes. Never anything else.
remove_colour() {
  local name=$1 volume left deadline
  is_canary_colour "${name}" || die "refusing to remove '${name}': not one of the canary colours (${CANARY_COLOURS[*]})"
  if stack_exists "${name}"; then
    docker stack rm "${name}" >/dev/null
    log "docker stack rm ${name}"
  fi
  # Swarm stops the tasks after "stack rm" has returned. A volume can only go once no container
  # uses it, and a deploy of the same name trips over a network that is still there.
  deadline=$((SECONDS + REMOVE_TIMEOUT))
  while :; do
    left="$(docker ps -aq --filter "label=com.docker.stack.namespace=${name}")"
    left+="$(docker network ls -q --filter "label=com.docker.stack.namespace=${name}")"
    [[ -n "${left}" ]] || break
    [[ "${SECONDS}" -lt "${deadline}" ]] ||
      die "${name} still has containers or networks after ${REMOVE_TIMEOUT} s (docker ps -a --filter label=com.docker.stack.namespace=${name})"
    sleep 2
  done
  for volume in radix-data folia-data; do
    if docker volume inspect "${name}_${volume}" >/dev/null 2>&1; then
      docker volume rm "${name}_${volume}" >/dev/null
      log "volume ${name}_${volume} removed"
    fi
  done
}

# wait_until_healthy NAME - until the colour's web server answers /healthz with 200, asked directly
# (Traefik still sends the host to the other colour): it has a catalog and hears from its Radix.
wait_until_healthy() {
  local name=$1 address code="" deadline
  deadline=$((SECONDS + HEALTH_TIMEOUT))
  while :; do
    address="$(folia_address "${name}")"
    if [[ -n "${address}" ]]; then
      code="$(curl -sS -o /dev/null --max-time 10 -w '%{http_code}' "http://${address}:8080/healthz" 2>/dev/null || true)"
      if [[ "${code}" == "200" ]]; then
        log "${name}_folia answers /healthz with 200 (asked directly at ${address}:8080)"
        return 0
      fi
    fi
    [[ "${SECONDS}" -lt "${deadline}" ]] ||
      die "${name}_folia does not answer /healthz with 200 after ${HEALTH_TIMEOUT} s (last: ${code:-no answer}): docker service logs ${name}_folia; ${name}_radix"
    sleep 5
  done
}

# remove_old_images - images this agent loaded (state file "loaded") that no service uses, but the
# newest KEEP_IMAGES of them: those stay for a deploy by hand. Images shipped with deploy/ship.sh
# are never touched.
remove_old_images() {
  local used tag image i failed
  local -a loaded=() keep=()
  mapfile -t loaded < <(state_get loaded | sed '/^$/d')
  [[ "${#loaded[@]}" -gt "${KEEP_IMAGES}" ]] || return 0
  used="$(docker service ls -q | xargs -r docker service inspect --format '{{.Spec.TaskTemplate.ContainerSpec.Image}}' | sed 's/@.*//' | sort -u)"
  for i in "${!loaded[@]}"; do
    tag="${loaded[${i}]}"
    if [[ "${i}" -ge $((${#loaded[@]} - KEEP_IMAGES)) ]] || grep -qxF -e "betula-radix:${tag}" -e "betula-folia:${tag}" <<<"${used}"; then
      keep+=("${tag}")
      continue
    fi
    failed=0
    for image in radix folia; do
      if docker image inspect "betula-${image}:${tag}" >/dev/null 2>&1; then
        docker image rm "betula-${image}:${tag}" >/dev/null 2>&1 || failed=1
      fi
    done
    if [[ "${failed}" -eq 0 ]]; then
      log "images of ${tag} removed (no service uses them)"
    else
      warn "the images of ${tag} could not all be removed (a stopped container may still use them); trying again after the next deploy"
      keep+=("${tag}")
    fi
  done
  state_set loaded "$(printf '%s\n' "${keep[@]}")"
}

# ---------------------------------------------------------------- deploy

# deploy_release TAG - bring canary to the loaded release TAG, with a fresh copy of the public
# site's database (the steps at the head of this file).
deploy_release() {
  local tag=$1 live target colour bytes serving="nobody"
  local -a others=()
  step "Where https://${CANARY_HOST} stands"
  images_loaded "${tag}" || die "the images of ${tag} are not loaded (docker image ls 'betula-*')"
  live="$(app_stack_for_host "${CANARY_HOST}")"
  if [[ -n "${live}" ]]; then
    is_canary_colour "${live}" || die "https://${CANARY_HOST} is served by ${live}, which is not one of the canary colours (${CANARY_COLOURS[*]})"
    serving="${live} ($(image_tag "${live}_folia"))"
  fi
  target=""
  for colour in "${CANARY_COLOURS[@]}"; do
    if [[ "${colour}" != "${live}" ]]; then
      target="${colour}"
      break
    fi
  done
  seed_source
  log "https://${CANARY_HOST} is served by ${serving}. Release ${tag} goes to ${target}, with the data of ${SEED_SOURCE}"

  step "Remove ${target}: it does not serve"
  remove_colour "${target}"

  step "A copy of the public site's database (${SEED_SOURCE}_radix-data)"
  make_work_dir
  copy_public_database "${WORK}/seed"

  step "Seed ${target} with it"
  bytes="$(stat -c %s "${WORK}/seed/radix.db")"
  # pipefail: a broken pipe anywhere fails the step.
  tar -C "${WORK}/seed" --owner=0 --group=0 --mode=0644 -cf - radix.db | gzip -1 |
    bash "${BETULA_VPS_DIR}/45-seed.sh" "${target}" "${tag}" "${bytes}"
  rm -f -- "${WORK}/seed/radix.db"

  step "Deploy ${target} (the release migrates, builds and exports the copy)"
  bash "${BETULA_VPS_DIR}/50-app.sh" "${target}" "${tag}"
  wait_until_healthy "${target}"

  if [[ -n "${live}" ]]; then
    step "Hand https://${CANARY_HOST} to ${target}"
    bash "${BETULA_VPS_DIR}/55-switch.sh" "${target}"
    # The priority label can restart the web server after all: the CLI hands swarm the whole
    # spec back, normalised (mounts sorted, an empty DNSConfig), and swarm takes that for a new
    # task template (seen with Docker 29.3). Until that update is through and the new task
    # serves, the old colour is what Traefik falls back to, so it stays until then.
    wait_for_stack "${target}"
    [[ "${#FAILED_STACKS[@]}" -eq 0 ]] || die "${target} did not settle after the switch (docker service ps --no-trunc ${target}_folia); ${live} stays, and Traefik sends the host to it while ${target} has no healthy task"
    wait_until_healthy "${target}"
  fi

  for colour in "${CANARY_COLOURS[@]}"; do
    if [[ "${colour}" != "${target}" ]] && { stack_exists "${colour}" || docker volume inspect "${colour}_radix-data" >/dev/null 2>&1; }; then
      others+=("${colour}")
    fi
  done
  for colour in ${others[@]+"${others[@]}"}; do
    step "Remove ${colour}: ${target} serves now"
    remove_colour "${colour}"
  done

  step "Images"
  remove_old_images
  state_set deployed "$(now_utc) ${tag} ${target} seeded from ${SEED_SOURCE} (schema ${SEED_SCHEMA})"
  step "Done"
  log "https://${CANARY_HOST} runs ${tag} (${target}), with the data of ${SEED_SOURCE} as of $(now_utc)"
}

# ---------------------------------------------------------------- commands

poll() {
  local seen live_tag failed failed_tag failures last_failure wait_until
  take_lock
  make_auth_headers
  latest_run
  if [[ -z "${BUILD_RUN}" ]]; then
    state_set last-check "$(now_utc) no successful run of ${WORKFLOW} for master yet"
    return 0
  fi
  seen="$(state_get seen-run)"
  # A build that was handled (deployed, found running, given up, or gone) is not looked at again:
  # one request to GitHub per look, and nothing in the journal.
  if [[ "${BUILD_RUN}" == "${seen}" ]]; then
    state_set last-check "$(now_utc) run ${BUILD_RUN}, handled"
    return 0
  fi
  if ! run_artifact; then
    log "run ${BUILD_RUN} of ${WORKFLOW} has no artifact any more (kept for a day): canary waits for the next build of master"
    state_set last-check "$(now_utc) run ${BUILD_RUN}, no artifact"
    state_set seen-run "${BUILD_RUN}"
    rm -f -- "${STATE_DIR}/failed"
    return 0
  fi
  state_set last-check "$(now_utc) run ${BUILD_RUN}: ${BUILD_TAG}"

  # The failures counted are those of the newest build; a newer one starts from nothing.
  failed="$(state_get failed)"
  failures=0
  last_failure=0
  if [[ -n "${failed}" ]]; then
    read -r failed_tag failures last_failure <<<"${failed}"
    if [[ "${failed_tag}" != "${BUILD_TAG}" ]]; then
      rm -f -- "${STATE_DIR}/failed"
      failures=0
      last_failure=0
    fi
  fi

  live_tag="$(image_tag "$(app_stack_for_host "${CANARY_HOST}")_folia")"
  if [[ "${live_tag}" == "${BUILD_TAG}" ]]; then
    log "run ${BUILD_RUN} built ${BUILD_TAG}, which https://${CANARY_HOST} runs already"
    state_set seen-run "${BUILD_RUN}"
    rm -f -- "${STATE_DIR}/failed"
    return 0
  fi
  if [[ "${failures}" -ge "${MAX_ATTEMPTS}" ]]; then
    log "release ${BUILD_TAG} failed ${failures} times: not trying it again. Canary waits for the next build of master (by hand: bash $0 deploy <tag>)"
    state_set seen-run "${BUILD_RUN}"
    return 0
  fi
  wait_until=$((last_failure + failures * RETRY_MINUTES * 60))
  [[ "$(date +%s)" -ge "${wait_until}" ]] || return 0

  # Counted before the attempt: one that dies (or a reboot in the middle) counts as well.
  state_set failed "${BUILD_TAG} $((failures + 1)) $(date +%s)"
  log "new build of master: run ${BUILD_RUN}, release ${BUILD_TAG} (canary runs ${live_tag:-nothing}); attempt $((failures + 1)) of ${MAX_ATTEMPTS}"
  require_ubuntu
  fetch_images
  deploy_release "${BUILD_TAG}"
  rm -f -- "${STATE_DIR}/failed"
  state_set seen-run "${BUILD_RUN}"
}

deploy_by_hand() {
  local tag=${1:-}
  take_lock
  require_ubuntu
  if [[ -z "${tag}" ]]; then
    tag="$(image_tag "$(app_stack_for_host "${CANARY_HOST}")_folia")"
    [[ -n "${tag}" ]] || die "https://${CANARY_HOST} is not served: name the release to deploy (docker image ls 'betula-*')"
    log "no release named: ${tag}, the one https://${CANARY_HOST} runs, again with fresh data"
  fi
  [[ "${tag}" =~ ${TAG_PATTERN} && "${tag}" != "latest" ]] || die "'${tag}' is not a release tag"
  deploy_release "${tag}"
}

status() {
  local colour live line timer
  live="$(app_stack_for_host "${CANARY_HOST}")"
  printf 'https://%s is served by %s\n' "${CANARY_HOST}" "${live:-nobody}"
  for colour in "${CANARY_COLOURS[@]}"; do
    if stack_exists "${colour}"; then
      printf '  %-14s deployed, release %s, router priority %s\n' "${colour}" "$(image_tag "${colour}_folia")" "$(router_priority "${colour}")"
    else
      printf '  %-14s not deployed\n' "${colour}"
    fi
  done
  # In a subshell: without a public site to copy from, status still says the rest.
  if (seed_source && printf 'data of the next deploy: %s_radix-data\n' "${SEED_SOURCE}"); then :; else
    printf 'data of the next deploy: none (see above)\n'
  fi
  if [[ -f /etc/systemd/system/betula-canary.timer ]]; then
    timer="$(systemctl show betula-canary.timer --property=ActiveState --value 2>/dev/null || true)"
    printf 'timer betula-canary.timer: %s (next look: %s); last run of the service: %s\n' "${timer:-?}" \
      "$(systemctl show betula-canary.timer --property=NextElapseUSecRealtime --value 2>/dev/null || true)" \
      "$(systemctl show betula-canary.service --property=Result --value 2>/dev/null || true)"
  else
    printf 'timer betula-canary.timer: not installed (sudo bash %s/vps/60-canary.sh)\n' "${BETULA_ROOT}"
  fi
  for line in last-check seen-run deployed failed token-expires; do
    printf '%-14s %s\n' "${line}:" "$(state_get "${line}" | tr '\n' ' ')"
  done
  printf 'loaded by this agent (newest last): %s\n' "$(state_get loaded | tr '\n' ' ')"
}

# ---------------------------------------------------------------- main

[[ "$#" -ge 1 ]] || usage
action=$1
shift
check_setup
case "${action}" in
  poll)
    [[ "$#" -eq 0 ]] || usage
    poll
    ;;
  deploy)
    [[ "$#" -le 1 ]] || usage
    deploy_by_hand "$@"
    ;;
  status)
    [[ "$#" -eq 0 ]] || usage
    status
    ;;
  *) usage ;;
esac
