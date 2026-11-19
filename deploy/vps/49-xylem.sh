#!/usr/bin/env bash
# 49-xylem.sh - deploy an instance of Xylem, the service of Betula's notifications
# (stacks/xylem.yml, docs/xylem/xylem.md), from an image that is loaded on this server already,
# and then try what it must not be able to do and the way out it needs. Run on the server as the
# deploy user, WITHOUT sudo; deploy/ship-xylem.sh does that after it has built and loaded the
# image:
#
#   bash /opt/betula/vps/49-xylem.sh xylem-canary 2026-11-19-ab12cd3   # this release
#   bash /opt/betula/vps/49-xylem.sh xylem                             # the release that runs
#                                                     # now (after a change to stacks/xylem.yml
#                                                     # or stacks/xylem/xylem.env)
#
# Each instance is a stack of its own (xylem: https://betula.app, xylem-canary:
# https://canary.betula.app), with its own networks, data, key and numbers, deployed on its own.
# In this order:
#   1. What it needs: its file stacks/xylem/<stack>.env; its network <stack>_in (overlay,
#      internal, not attachable: vps/30-docker.sh), on which Traefik is; its two file systems
#      (vps/47-xylem-host.sh); the image.
#   2. docker stack deploy <stack>. Swarm stops the old task before it starts the new one (one
#      writer on xylem.db, one sender): for those seconds a browser that subscribes gets 502 and
#      tries again with its next visit, and an announcement that is due is sent by the new task.
#      What swarm was told is checked (image, revision, the router), and waited for until it runs.
#   3. What is forbidden, tried, and the way out. From inside: "xylem probe" (a resolver other
#      than Docker's, a port other than 443, the metadata service, a private address, IPv6,
#      Traefik's address, the host's: all must be dropped; a name must resolve, a public address
#      must answer on 443; and the service has no privilege left). From outside:
#      https://<APP_HOST>/push/v1/key reaches Xylem (its router, by Traefik's access log),
#      /healthz and /status do not; its stats file is there and fresh; with the feed on, one
#      read of it. The monitoring is told first that probes run: they set off "Xylem: its
#      firewall drops connections" otherwise.
#   4. 91-verify-stacks.sh xylem: its networks, the probes from the host's side, what the
#      monitoring sees.
# A FAIL in 3 or 4 ends with exit 1: the release runs, but it should not stay. Roll back with the
# tag before it, which is still loaded (docker image ls betula-xylem):
#   bash /opt/betula/vps/49-xylem.sh <stack> <previous tag>
#
# Environment (all optional):
#   CONVERGE_TIMEOUT=600   seconds to wait for the task to run (its health check gives it 30 s)
#   XYLEM_VERIFY=1         0: step 4 is skipped (a test swarm without the monitoring)
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
# shellcheck source=lib-stacks.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-stacks.sh"
# shellcheck source=lib-xylem.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-xylem.sh"
betula_init --tmp

# The service label that keeps the revision of what a deploy is made of (stack file and .env).
REV_LABEL="app.betula.xylem-rev"
TAG_PATTERN='^[A-Za-z0-9][A-Za-z0-9_.-]{0,100}$'
TRAEFIK="edge_traefik"
XYLEM_VERIFY="${XYLEM_VERIFY:-1}"
# Where the monitoring learns that probes run (91-verify-stacks.sh writes the same file).
PROBES_FILE="${BETULA_RUN_DIR}/metrics/verify-probes.prom"
FAILS=0

usage() {
  die "usage: bash $(basename "$0") <instance> [<tag>]   (instances: $(xylem_names | tr '\n' ' '); loaded: $(loaded_images))"
}

loaded_images() {
  docker image ls --format '{{.Repository}}:{{.Tag}}' betula-xylem 2>/dev/null | tr '\n' ' ' || true
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

# service_env SERVICE NAME -> the value swarm gives the service's tasks for NAME.
service_env() {
  docker service inspect "$1" --format '{{range .Spec.TaskTemplate.ContainerSpec.Env}}{{println .}}{{end}}' | sed -n "s/^$2=//p"
}

# ---------------------------------------------------------------- steps

preflight() {
  step "Preconditions of ${STACK}"
  local facts services svc
  if [[ "${EUID}" -eq 0 && -n "${SUDO_USER:-}" ]]; then
    die "run this as ${DEPLOY_USER} WITHOUT sudo: docker stack deploy reads XYLEM_IMAGE and the rest from the environment, and sudo resets it"
  fi
  [[ "${BETULA_VPS_DIR}" == "${BETULA_ROOT}/vps" ]] ||
    die "this copy lives in ${BETULA_VPS_DIR}; the stack file is read from ${STACKS_DIR}, so run ${BETULA_ROOT}/vps/$(basename "$0")"
  [[ "${CONVERGE_TIMEOUT}" =~ ^[0-9]{1,5}$ ]] || die "CONVERGE_TIMEOUT='${CONVERGE_TIMEOUT}' is not a number of seconds"
  [[ "${XYLEM_VERIFY}" == "0" || "${XYLEM_VERIFY}" == "1" ]] || die "XYLEM_VERIFY='${XYLEM_VERIFY}' is not 0 or 1"
  require_cmd docker jq curl sha256sum findmnt awk
  require_swarm_manager
  log "instance ${STACK}: https://${XYLEM_HOST}/push/v1/, language ${XYLEM_LANGUAGE}, at most ${XYLEM_MAX} subscriptions, feed ${XYLEM_FEED_URL:-off}"

  # <stack>_in: Traefik and it, nothing else can join (attachable false), and no way out
  # (internal true). Made by 30-docker.sh, never by a stack. <stack>_out is the stack's own.
  facts="$(docker network inspect "${STACK}_in" --format '{{.Driver}} {{.Scope}} {{.Attachable}} {{.Internal}}' 2>/dev/null || true)"
  [[ "${facts}" == "overlay swarm false true" ]] ||
    die "network ${STACK}_in is missing or not what it has to be ('${facts:-missing}', wanted 'overlay swarm false true': internal, not attachable). sudo bash ${BETULA_ROOT}/vps/30-docker.sh makes it"
  [[ "$(service_networks "${TRAEFIK}")" == *"${STACK}_in true"* ]] ||
    die "${TRAEFIK} is not on ${STACK}_in, so it could not reach Xylem: bash ${BETULA_ROOT}/vps/40-stacks.sh edge (stacks/edge.yml names the network)"
  xylem_check_mounts "${STACK}" || die "${XYLEM_MOUNT_DETAIL}"
  ok "its file systems: $(xylem_data_dir "${STACK}") (ext4 of its own) and $(xylem_stats_dir "${STACK}") (tmpfs of 1 MiB)"

  docker image inspect "${XYLEM_IMAGE}" >/dev/null 2>&1 ||
    die "image ${XYLEM_IMAGE} is not loaded on this server (deploy/ship-xylem.sh builds and loads it; loaded: $(loaded_images))"
  [[ -f "${XYLEM_STACK_FILE}" ]] || die "stack file missing: ${XYLEM_STACK_FILE} (run deploy/sync.sh)"
  assert_lf "${XYLEM_STACK_FILE}"
  services="$(file_services "${XYLEM_STACK_FILE}" | tr '\n' ' ')"
  [[ "${services}" == "push " ]] ||
    die "${XYLEM_STACK_FILE} has to define exactly the service push, not: ${services:-none}"
  if stack_exists "${STACK}"; then
    while IFS= read -r svc; do
      [[ -z "${svc}" || "${svc}" == "${SERVICE}" ]] ||
        die "stack ${STACK} has the service ${svc}, which ${XYLEM_STACK_FILE##*/} does not define: remove it (docker service rm ${svc}) and run this again"
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
  [[ "${image}" != "${label}" && "${label}" == "${XYLEM_IMAGE}" ]] || return 0
  warn "${SERVICE} runs ${image}, not ${label}, which the stack gave it (docker service update by hand?): it is given ${XYLEM_IMAGE} directly first"
  docker service update --detach --quiet --no-resolve-image --image "${XYLEM_IMAGE}" "${SERVICE}" >/dev/null
}

deploy() {
  step "docker stack deploy ${STACK}: ${XYLEM_IMAGE}, revision ${XYLEM_REV}"
  local told
  if docker service inspect "${SERVICE}" >/dev/null 2>&1; then
    log "it runs $(service_image "${SERVICE}"), revision $(service_label "${SERVICE}" "${REV_LABEL}")"
  fi
  undrift
  # Substituted into the stack file by "docker stack deploy".
  export STACK_NAME="${STACK}" APP_HOST="${XYLEM_HOST}" XYLEM_LANG="${XYLEM_LANGUAGE}" XYLEM_CONTACT="${XYLEM_CONTACT_ADDRESS}"
  export XYLEM_FEED="${XYLEM_FEED_URL}" XYLEM_MAX_SUBSCRIPTIONS="${XYLEM_MAX}" XYLEM_IMAGE XYLEM_REV
  # --resolve-image never: no registry knows the image. --prune: a service that left the file
  # leaves the swarm as well.
  docker stack deploy --detach=true --prune --resolve-image never -c "${XYLEM_STACK_FILE}" "${STACK}"

  # What swarm was told, not what this script meant: the substitution of "docker stack deploy"
  # has surprises (stacks/betula.yml), and a wrong image is retried forever.
  told="$(service_image "${SERVICE}")"
  [[ "${told}" == "${XYLEM_IMAGE}" ]] || die "${SERVICE} was given the image '${told}' instead of ${XYLEM_IMAGE}: look at the image line of ${XYLEM_STACK_FILE}"
  told="$(service_label "${SERVICE}" "${REV_LABEL}")"
  [[ "${told}" == "${XYLEM_REV}" ]] || die "${SERVICE} was given the revision '${told}' instead of ${XYLEM_REV}: look at the label ${REV_LABEL} in ${XYLEM_STACK_FILE}"
  told="$(service_label "${SERVICE}" "traefik.http.routers.${STACK}-push.rule")"
  [[ "${told}" == "Host(\`${XYLEM_HOST}\`) && PathPrefix(\`/push/v1/\`)" ]] ||
    die "the router of ${SERVICE} says '${told}', wanted Host(\`${XYLEM_HOST}\`) && PathPrefix(\`/push/v1/\`): look at its labels in ${XYLEM_STACK_FILE}"
  told="$(service_env "${SERVICE}" XYLEM_SITE)"
  [[ "${told}" == "https://${XYLEM_HOST}" ]] || die "${SERVICE} was given the site '${told}' instead of https://${XYLEM_HOST}"
  told="$(service_env "${SERVICE}" XYLEM_FEED)"
  [[ "${told}" == "${XYLEM_FEED_URL}" ]] || die "${SERVICE} was given the feed '${told}' instead of '${XYLEM_FEED_URL}'"
  told="$(docker service inspect "${SERVICE}" --format '{{range .Endpoint.Spec.Ports}}{{.PublishedPort}}/{{.Protocol}} {{end}}')"
  [[ -z "${told// /}" ]] || die "${SERVICE} publishes '${told% }': Xylem publishes nothing (Traefik reaches it over ${STACK}_in)"

  wait_for_stack "${STACK}"
  [[ "${#FAILED_STACKS[@]}" -eq 0 ]] ||
    die "${STACK} did not converge (above: docker service ps and the last log lines). A failed update was rolled back by swarm; fix the cause and run this again"
}

# announce_probes - tells the monitoring that probes run now, so that the drops they cause in
# Xylem's firewall are no alarm (the rules of group betula-xylem leave out the 15 minutes after
# it). A directory of root's: through sudo -n, as 91-verify-stacks.sh does it.
announce_probes() {
  if printf '# TYPE betula_verify_probes_timestamp_seconds gauge\nbetula_verify_probes_timestamp_seconds %s\n' "$(date +%s)" |
    sudo -n sh -c "install -d -m 0755 '${PROBES_FILE%/*}' && cat >'${PROBES_FILE}.tmp' && chmod 0644 '${PROBES_FILE}.tmp' && mv -f '${PROBES_FILE}.tmp' '${PROBES_FILE}'" 2>/dev/null; then
    log "the monitoring knows that probes run (${PROBES_FILE})"
  else
    warn "could not write ${PROBES_FILE}: the probes may set off \"Xylem: its firewall drops connections\""
  fi
}

# router_of PATH - one request of ours to https://<APP_HOST>PATH: ROUTER_NAME is the router
# Traefik's access log names for it (empty while the line is not there), ROUTER_STATUS and
# ROUTER_BODY what came back. Sets globals: not to be called in $(...).
ROUTER_NAME=""
ROUTER_STATUS=""
ROUTER_BODY=""
router_of() {
  local path=$1 marker="xylem-${RANDOM}${RANDOM}" out
  out="$(betula_tmpfile)"
  # Through this host's Traefik, whatever the name resolves to; -k: the certificate is not the
  # question here (91-verify-stacks.sh checks it). GET only: nothing is changed.
  # --path-as-is: "/push/v1/../status" is asked as written, not as curl would tidy it.
  ROUTER_STATUS="$(curl -sS -k --noproxy '*' --path-as-is --max-time 10 --resolve "${XYLEM_HOST}:443:127.0.0.1" \
    -o "${out}" -w '%{http_code}' "https://${XYLEM_HOST}${path}?${marker}" 2>/dev/null || true)"
  ROUTER_BODY="$(head -c 200 "${out}" | tr -d '\n')"
  sleep 1
  ROUTER_NAME="$(docker service logs --since 2m "${TRAEFIK}" 2>&1 | grep -F "${marker}" |
    sed -n 's/.*"RouterName":"\([^"]*\)".*/\1/p' | tail -n 1 || true)"
}

probes() {
  step "What ${STACK} must not be able to do, and what it must, tried"
  local cid address gateway line fields result want target what detail wrong=0 out code prom age path
  cid="$(service_container "${SERVICE}")"
  [[ -n "${cid}" ]] || die "${SERVICE} has no running container on this node"
  announce_probes

  # From inside: every probe from Xylem's own network namespace, as its own user. Traefik's
  # address on <stack>_in is the one other member of that network: Xylem may answer it, never
  # call it. The gateway of docker_gwbridge is the host itself, as Xylem's way out sees it.
  address="$(docker network inspect "${STACK}_in" --format '{{range .Containers}}{{.Name}} {{.IPv4Address}}{{"\n"}}{{end}}' 2>/dev/null |
    awk -v t="${TRAEFIK}." 'index($1, t) == 1 { sub(/\/.*/, "", $2); print $2; exit }')"
  gateway="$(docker network inspect docker_gwbridge --format '{{range .IPAM.Config}}{{.Gateway}}{{"\n"}}{{end}}' 2>/dev/null | grep -m 1 -E '^[0-9.]+$' || true)"
  local -a targets=()
  if [[ -n "${address}" ]]; then
    targets=("${address}:443" "${address}:80" "${address}:8082")
  else
    bad "no address of ${TRAEFIK} on ${STACK}_in to probe"
  fi
  if [[ -n "${gateway}" ]]; then
    targets+=("${gateway}:22" "${gateway}:443")
  fi
  out="$(betula_tmpfile)"
  code=0
  docker exec "${cid}" /bin/xylem probe ${targets[@]+"${targets[@]}"} >"${out}" 2>&1 || code=$?
  while IFS= read -r line; do
    [[ -n "${line}" ]] || continue
    # One line per probe: {"check", "target", "what", "result", "want", "detail"} (xylem/cmd/xylem/probe_linux.go).
    if ! fields="$(jq -r '[.result, .want, .check, (.target // ""), ((.what // "") + (if .detail then ": " + .detail else "" end))] | @tsv' <<<"${line}" 2>/dev/null)"; then
      bad "xylem probe said: ${line}"
      wrong=$((wrong + 1))
      continue
    fi
    IFS=$'\t' read -r result want what target detail <<<"${fields}"
    if [[ "${result}" == "${want}" ]]; then
      log "  ${result}: ${what} ${target} (${detail})"
    else
      bad "from inside ${SERVICE}: ${what} ${target} is ${result}, wanted ${want} (${detail})"
      wrong=$((wrong + 1))
    fi
  done <"${out}"
  if [[ "${code}" -eq 0 && "${wrong}" -eq 0 ]]; then
    ok "from inside: names resolve and port 443 of the internet answers; nothing else gets out (another resolver, another port, the metadata service, private addresses, IPv6, Traefik, the host), and the service has no privilege"
  elif [[ "${wrong}" -eq 0 ]]; then
    bad "xylem probe ended with ${code} (docker exec ${cid:0:12} /bin/xylem probe)"
  fi

  # From outside: through Traefik, as a browser would.
  router_of /push/v1/key
  if [[ "${ROUTER_NAME}" == "${STACK}-push@swarm" && "${ROUTER_STATUS}" == "200" && "${ROUTER_BODY}" == '{"key":"'* ]]; then
    ok "https://${XYLEM_HOST}/push/v1/key reaches Xylem (router ${ROUTER_NAME}, 200 with its key)"
  else
    bad "https://${XYLEM_HOST}/push/v1/key: router '${ROUTER_NAME:-none in the access log}', ${ROUTER_STATUS:-no answer} ${ROUTER_BODY}; wanted router ${STACK}-push@swarm and Xylem's 200 {\"key\":…}"
  fi
  for path in /healthz /status /announcements; do
    router_of "${path}"
    if [[ "${ROUTER_NAME}" != "${STACK}-push@swarm" ]]; then
      ok "https://${XYLEM_HOST}${path} does not reach Xylem (router ${ROUTER_NAME:-none}, ${ROUTER_STATUS})"
    else
      bad "https://${XYLEM_HOST}${path} reaches Xylem (router ${ROUTER_NAME}): only /push/v1/ may"
    fi
  done
  # What Xylem answers loopback alone, asked through its router's own prefix.
  router_of /push/v1/../status
  if [[ "${ROUTER_BODY}" == *'"subscriptions"'* ]]; then
    bad "https://${XYLEM_HOST}/push/v1/../status answers with Xylem's status (${ROUTER_STATUS}, router ${ROUTER_NAME:-none}): it is for loopback alone"
  else
    ok "https://${XYLEM_HOST}/push/v1/../status gives no status (${ROUTER_STATUS}, router ${ROUTER_NAME:-none})"
  fi

  # Its numbers: a file on its tmpfs, never a network.
  prom="$(xylem_stats_dir "${STACK}")/xylem.prom"
  for _ in 1 2 3 4; do
    [[ -r "${prom}" ]] && break
    sleep 5
  done
  if [[ -r "${prom}" ]]; then
    age=$(($(date +%s) - $(stat -c %Y "${prom}")))
    if [[ "$(awk '$1 ~ /^xylem_up([{]|$)/ { print $2 }' "${prom}")" == "1" && "${age}" -le 60 ]]; then
      ok "its stats file says xylem_up 1, written ${age} s ago (${prom})"
    else
      bad "its stats file ${prom} does not say xylem_up 1, or is ${age} s old (Xylem writes it every 15 s)"
    fi
  else
    bad "no stats file ${prom}: Xylem writes it every 15 s, 15 s after its start at the latest"
  fi
  out="$(docker exec "${cid}" /bin/xylem status 2>/dev/null || true)"
  if jq -e . >/dev/null 2>&1 <<<"${out}"; then
    log "xylem status: $(jq -c 'del(.key)' <<<"${out}")"
  else
    bad "xylem status does not answer (docker exec ${cid:0:12} /bin/xylem status)"
  fi

  # The feed: one read now. A site that does not serve it (a Folia before 1.108.0, or one that is
  # down) is no fault of this release: Xylem goes on with what a person announces, reads again
  # every ten minutes, and the monitoring says so while it lasts ("Xylem: the feed does not read").
  if [[ -n "${XYLEM_FEED_URL}" ]]; then
    code=0
    out="$(docker exec "${cid}" /bin/xylem feed --json 2>&1)" || code=$?
    if [[ "${code}" -eq 0 ]] && jq -e . >/dev/null 2>&1 <<<"${out}"; then
      ok "the feed reads (${XYLEM_FEED_URL}): $(jq -c '{entries, unchanged, baseline, changes: (.changes // [] | map(.what + " " + .key))}' <<<"${out}")"
    else
      warn "the feed does not read (${XYLEM_FEED_URL}): ${out:0:300}. Xylem tells what a person announces and tries the feed again every ten minutes; is the site up, and its Folia 1.108.0 or newer? (docker exec ${cid:0:12} /bin/xylem status says what failed)"
    fi
  else
    log "no feed (XYLEM_FEED=off in stacks/xylem/${STACK}.env): only a person announces"
  fi
}

verify() {
  if [[ "${XYLEM_VERIFY}" == "0" ]]; then
    warn "XYLEM_VERIFY=0: 91-verify-stacks.sh xylem is skipped"
    return 0
  fi
  step "91-verify-stacks.sh xylem"
  if ! bash "${BETULA_VPS_DIR}/91-verify-stacks.sh" xylem; then
    bad "91-verify-stacks.sh xylem has FAILs (above)"
  fi
}

report() {
  step "Done"
  docker stack services "${STACK}" --format '{{.Name}}  {{.Replicas}}  {{.Image}}' | sed 's/^/  /'
  if [[ "${FAILS}" -gt 0 ]]; then
    die "${FAILS} FAIL(s) above: ${STACK} runs ${XYLEM_IMAGE}, but should not stay so. Fix the cause and deploy again, or go back: bash ${BETULA_ROOT}/vps/49-xylem.sh ${STACK} <previous tag> (loaded: $(loaded_images))"
  fi
  log "${STACK} runs ${XYLEM_IMAGE} at https://${XYLEM_HOST}/push/v1/ (revision ${XYLEM_REV})"
  log "the pages offer notifications once the site's instance says FOLIA_PUSH=on and is deployed: bash ${BETULA_ROOT}/vps/50-app.sh <instance> (README.md section 17)"
}

# ---------------------------------------------------------------- main

[[ "$#" -ge 1 && "$#" -le 2 && "${1}" != -* && "${2:-}" != -* ]] || usage
load_xylem "$1"
STACK="${XYLEM_STACK}"
SERVICE="${STACK}_push"
require_ubuntu

TAG="${2:-}"
if [[ -z "${TAG}" ]]; then
  TAG="$(running_tag)"
  [[ -n "${TAG}" ]] || die "${STACK} is not deployed yet, so there is no release to keep: name a tag (loaded: $(loaded_images))"
fi
[[ "${TAG}" =~ ${TAG_PATTERN} && "${TAG}" != "latest" ]] ||
  die "'${TAG}' is not a release tag. Never \"latest\": swarm compares the service definition, not the image content, so a re-loaded \"latest\" restarts nothing"
XYLEM_IMAGE="betula-xylem:${TAG}"

preflight
# What this deploy is made of besides the image: the stack file and the instance's file.
XYLEM_REV="$(cat "${XYLEM_STACK_FILE}" "${XYLEM_ENV_DIR}/${STACK}.env" | sha256sum | cut -c1-12)"
deploy
probes
verify
report
