#!/usr/bin/env bash
# 48-cortex.sh - deploy Cortex (stacks/cortex.yml), the cache between the application and the
# internet, from an image that is loaded on this server already, or bring it to another release.
# Run on the server as the deploy user, WITHOUT sudo; deploy/ship-cortex.sh does that after it has
# built and loaded the image:
#
#   bash /opt/betula/vps/48-cortex.sh 2026-10-02-ab12cd3   # this release
#   bash /opt/betula/vps/48-cortex.sh                      # the release that runs now (after a
#                                                          # change to stacks/cortex.yml)
#
# One Cortex per host, for every instance and colour, deployed before the application (50-app.sh
# points an instance's Radix at it while the stack runs). Two instances, cortex_a and cortex_b;
# which of them leads is theirs to decide (stacks/cortex.yml says how).
#
# The first deploy starts both. Every later one goes one instance at a time, so that there is no
# moment without a leader:
#   1. the follower is deployed alone, and waited for until it runs and has caught up with the
#      leader: its journal, and every blob its index names (what "cortex status" says in each,
#      asked with docker exec; caught_up says what counts);
#   2. "cortex step-down" in the leader: the follower takes the lock over, and is waited for
#      until it leads; then STEP_DOWN_PAUSE, in which the clients move over to it;
#   3. the old leader is deployed alone; it comes back as the follower and is waited for until
#      it has caught up.
# Deployed alone means: from stacks/cortex.yml without the other instance's block, and without
# --prune, so that "docker stack deploy" leaves the other service as it is. A change to
# cortex.yml therefore goes the same way as a new release. An instance that runs the wanted
# image from the current revision of cortex.yml is not touched: the same tag twice changes
# nothing, and a run that stopped half way goes on where it stopped. When no instance leads (or
# both say they do: the lock is not shared), there is no leader to keep, and both are deployed at
# once. At the end, also when nothing had to change: exactly one leader, and the follower has
# caught up.
#
# The host policy, config/cortex/hosts.json, needs no deploy: both instances read it again within
# 30 s of a change (deploy/sync.sh brings it). This script only refuses one that is no JSON object.
# Rollback = the previous tag again; it is still loaded (docker image ls betula-cortex).
#
# Environment (all optional):
#   CONVERGE_TIMEOUT=600   seconds to wait for an instance to run (the healthcheck gives a new one
#                          30 s; the update monitors it another 30 s)
#   CATCHUP_TIMEOUT=600    seconds to wait for the follower to catch up with the leader (a fresh
#                          volume starts from a copy of the leader's index, and then fetches every
#                          blob it names: the hand-over waits for that as well)
#   STALL_TIMEOUT=60       seconds a follower may say "no_leader", or apply no journal entry while
#                          it is behind and its lag grows, before this script gives up early: it
#                          cannot reach the leader, and waiting CATCHUP_TIMEOUT would not change
#                          that (not counted while it only copies the leader's index)
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
# shellcheck source=lib-stacks.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-stacks.sh"
betula_init --tmp
require_ubuntu

STACK="cortex"
STACK_FILE="${STACKS_DIR}/cortex.yml"
HOSTS_FILE="${CONFIG_DIR}/cortex/hosts.json"
# The services of STACK_FILE; cortex_a and cortex_b in the swarm.
INSTANCES=(a b)
NETWORK="cortex"
# The service label that keeps the revision of STACK_FILE a service was deployed from.
REV_LABEL="app.betula.cortex-rev"
TAG_PATTERN='^[A-Za-z0-9][A-Za-z0-9_.-]{0,100}$'
CATCHUP_TIMEOUT="${CATCHUP_TIMEOUT:-600}"
STALL_TIMEOUT="${STALL_TIMEOUT:-60}"
# A follower counts as caught up while it is less than this many seconds behind the leader (with
# the rest of caught_up). Not "following, 0 entries behind": under steady writes a follower that
# keeps up is always a batch or two behind and flips between "following" and "catching_up"
# (verify2 E2E-6, D3), so that gate passed only by chance and mostly ran into CATCHUP_TIMEOUT.
CAUGHT_UP_LAG=1
# Seconds between the step-down and the deploy of the old leader. Clients still talk to it over
# kept-alive connections, and it forwards their writes to the new leader; stopped at once, it
# cuts a PUT in flight on such a connection, and the client cannot know whether it was applied
# (ErrOutcomeUnknown, never replayed). The pause lets them move to the new leader on fresh
# connections first (verify2 D5).
STEP_DOWN_PAUSE=3
# Seconds a follower gets to take the lock over after the leader stepped down (it polls the lock
# every 50 ms; the one that stepped down campaigns again after 15 s).
TAKEOVER_TIMEOUT=60
# Seconds the instances get to agree on a leader before this script decides that none leads.
LEADER_WAIT=30

# Set by read_roles, per instance: what its "cortex status" says. ROLE is leader or follower, or
# empty when it does not answer. Of a follower: LAG is its lag in seconds, STATE its state
# (following, snapshot, catching_up, no_leader; "-" on the leader), BEHIND the number of journal
# entries it has not applied and MISSING the number of blobs its index names that it has not
# fetched yet ("?" where "cortex status" does not say). URL: the leader URL a follower uses, the
# URL the leader announces of itself; "-" when it names none.
declare -A ROLE=() EPOCH=() SEQ=() LAG=() STATE=() BEHIND=() MISSING=() URL=()
# Set by find_leader: the instance that leads (empty when none does).
LEADER=""

usage() {
  die "usage: $(basename "$0") [<tag>]   (loaded: $(loaded_images))"
}

loaded_images() {
  docker image ls --format '{{.Repository}}:{{.Tag}}' betula-cortex 2>/dev/null | tr '\n' ' ' || true
}

# other X -> the other instance.
other() {
  if [[ "$1" == "a" ]]; then printf 'b'; else printf 'a'; fi
}

# service_image SERVICE -> the image the service is given, without a digest (nothing when the
# service does not exist).
service_image() {
  local image
  image="$(docker service inspect "$1" --format '{{.Spec.TaskTemplate.ContainerSpec.Image}}' 2>/dev/null || true)"
  printf '%s' "${image%%@*}"
}

# running_tag X -> the tag of the image cortex_X runs now (nothing when it does not exist).
running_tag() {
  local image
  image="$(service_image "${STACK}_$1")"
  [[ "${image}" == *:* ]] || return 0
  printf '%s' "${image##*:}"
}

# needs_update X - true when cortex_X is missing, runs another image or another revision of the
# stack file than this deploy's, or was scaled by hand (to stop it).
needs_update() {
  local svc="${STACK}_$1"
  [[ "$(service_image "${svc}")" != "${CORTEX_IMAGE}" ||
    "$(service_label "${svc}" "${REV_LABEL}")" != "${CORTEX_REV}" ||
    "$(docker service inspect "${svc}" --format '{{if .Spec.Mode.Replicated}}{{.Spec.Mode.Replicated.Replicas}}{{end}}' 2>/dev/null || true)" != "1" ]]
}

# ---------------------------------------------------------------- the stack file

# file_services FILE -> the services FILE defines (the keys two spaces deep under the top-level
# "services:"), one per line.
file_services() {
  awk '
    /^[^[:space:]#]/ { on = ($0 ~ /^services:[[:space:]]*$/); next }
    on && /^  [^[:space:]#]/ { name = $1; sub(/:.*$/, "", name); print name }
  ' "$1"
}

# instance_file X -> STACK_FILE without the services other than X: a service's block goes from its
# key to the next line that is not indented more (its comments and blank lines go with it).
# Everything outside "services:" stays, the x- blocks the services refer to included.
instance_file() {
  awk -v keep="$1" '
    /^[^[:space:]#]/ { in_services = ($0 ~ /^services:[[:space:]]*$/); drop = 0; print; next }
    in_services && /^  [^[:space:]#]/ { name = $1; sub(/:.*$/, "", name); drop = (name != keep) }
    !drop { print }
  ' "${STACK_FILE}"
}

# stack_rev -> the revision of STACK_FILE: a checksum of what "docker stack config" makes of it,
# with the image and the revision itself left out. Comments and layout do not count; the same for
# both instances.
stack_rev() {
  local out
  out="$(CORTEX_IMAGE=betula-cortex:rev CORTEX_REV=rev docker stack config -c "${STACK_FILE}" 2>/dev/null)" ||
    die "docker stack config cannot read ${STACK_FILE} (run it by hand to see why)"
  sha256sum <<<"${out}" | cut -c1-12
}

# ---------------------------------------------------------------- the instances

# status_json X -> what "cortex status" prints in cortex_X (nothing when it does not answer).
status_json() {
  local cid
  cid="$(service_container "${STACK}_$1")"
  [[ -n "${cid}" ]] || return 0
  timeout 20 docker exec "${cid}" /bin/cortex status 2>/dev/null || true
}

# read_roles - ROLE, EPOCH, SEQ, LAG, STATE, BEHIND, MISSING and URL of both instances. An answer
# without a role and two numbers counts as none.
read_roles() {
  local x line role epoch seq lag state behind missing url
  for x in "${INSTANCES[@]}"; do
    # The URL last: the one field that may be empty ("-" then) or hold anything, and read puts
    # the rest of the line into the last name.
    line="$(jq -r 'select(type == "object") | [.role, .epoch, .seq, (.follower.lag_seconds // 0), ((.follower.state // "") | if . == "" then "-" else . end), .follower.lag_entries, .follower.blobs_missing, ((if .role == "leader" then .leader.url else .follower.leader_url end) // "" | if . == "" then "-" else . end)] | map(tostring) | join(" ")' \
      <<<"$(status_json "${x}")" 2>/dev/null || true)"
    role="" epoch="" seq="" lag="" state="" behind="" missing="" url=""
    read -r role epoch seq lag state behind missing url <<<"${line}" || true
    if [[ "${role}" =~ ^(leader|follower)$ && "${epoch}" =~ ^[0-9]+$ && "${seq}" =~ ^[0-9]+$ ]]; then
      [[ "${state}" =~ ^[a-z_-]+$ ]] || state="?"
      [[ "${behind}" =~ ^[0-9]+$ ]] || behind="?"
      [[ "${missing}" =~ ^[0-9]+$ ]] || missing="?"
      # A number as jq prints it (1.5e-05 included), else "?": lag_below and lag_above take both.
      [[ "${lag}" =~ ^[0-9][0-9.eE+-]*$ ]] || lag="?"
      [[ "${url}" =~ ^[^[:space:]]+$ ]] || url="-"
      ROLE[$x]="${role}" EPOCH[$x]="${epoch}" SEQ[$x]="${seq}" LAG[$x]="${lag}"
      STATE[$x]="${state}" BEHIND[$x]="${behind}" MISSING[$x]="${missing}" URL[$x]="${url}"
    else
      ROLE[$x]="" EPOCH[$x]="" SEQ[$x]="" LAG[$x]="" STATE[$x]="" BEHIND[$x]="" MISSING[$x]="" URL[$x]=""
    fi
  done
}

# roles_summary -> what read_roles found, on one line.
roles_summary() {
  local x out=""
  for x in "${INSTANCES[@]}"; do
    case "${ROLE[$x]}" in
      leader) out+="${STACK}_${x} leads (epoch ${EPOCH[$x]}, seq ${SEQ[$x]}); " ;;
      follower) out+="${STACK}_${x} follows (epoch ${EPOCH[$x]}, seq ${SEQ[$x]}, ${STATE[$x]}, ${LAG[$x]} s and ${BEHIND[$x]} entries behind, ${MISSING[$x]} blobs missing); " ;;
      *) out+="${STACK}_${x} does not answer; " ;;
    esac
  done
  printf '%s' "${out%; }"
}

# roles_key -> the roles, epochs and states of read_roles, on one line: what a change worth a
# log line is.
roles_key() {
  local x
  for x in "${INSTANCES[@]}"; do
    printf '%s/%s/%s ' "${ROLE[$x]}" "${EPOCH[$x]}" "${STATE[$x]}"
  done
}

# leaders -> the instances that say they lead, on one line.
leaders() {
  local x
  for x in "${INSTANCES[@]}"; do
    if [[ "${ROLE[$x]}" == "leader" ]]; then printf '%s ' "${x}"; fi
  done
  return 0
}

# explain_instances - on stderr, for a human: the tasks, the last log lines and the status of both.
explain_instances() {
  local x
  for x in "${INSTANCES[@]}"; do
    printf '\n---- docker service ps --no-trunc %s_%s\n' "${STACK}" "${x}" >&2
    docker service ps --no-trunc "${STACK}_${x}" >&2 || true
    printf -- '---- docker service logs --tail 20 %s_%s\n' "${STACK}" "${x}" >&2
    docker service logs --no-task-ids --tail 20 "${STACK}_${x}" >&2 2>&1 || true
    printf -- '---- cortex status in %s_%s\n%s\n' "${STACK}" "${x}" "$(status_json "${x}")" >&2
  done
}

# two_leaders - dies with what to do. One look can see the old leader and the new one both
# leading (the instances are asked one after the other), so it is only called when that lasts.
two_leaders() {
  explain_instances
  die "both instances say they lead ($(roles_summary)): they do not share the lock. Both have to mount the volume ${STACK}_lock at /lock (docker service inspect ${STACK}_a ${STACK}_b); once ${STACK_FILE##*/} says so, run this script again (while both lead, it deploys both at once)"
}

# find_leader - sets LEADER to the instance that leads, waiting up to LEADER_WAIT for the
# instances to agree. Empty when none leads by then, or when both keep saying they do: then
# there is no leader to keep either.
find_leader() {
  local deadline=$((SECONDS + LEADER_WAIT)) twice=0
  local -a leading=()
  LEADER=""
  while :; do
    read_roles
    read -r -a leading <<<"$(leaders)" || true
    case "${#leading[@]}" in
      1)
        LEADER="${leading[0]}"
        log "$(roles_summary)"
        return 0
        ;;
      0) twice=0 ;;
      *)
        twice=$((twice + 1))
        if [[ "${twice}" -ge 3 ]]; then
          warn "both instances say they lead ($(roles_summary)): they do not share the lock"
          return 0
        fi
        ;;
    esac
    if [[ "${SECONDS}" -ge "${deadline}" ]]; then
      log "$(roles_summary)"
      return 0
    fi
    sleep 2
  done
}

# lag_below LAG MAX -> true when LAG, a number of seconds as "cortex status" prints it (1.5e-05
# included), is below MAX; false for "?".
lag_below() {
  [[ "$1" != "?" ]] && awk -v lag="$1" -v max="$2" 'BEGIN { exit !(lag + 0 < max + 0) }'
}

# lag_above LAG WAS -> true when LAG is above WAS (both as lag_below takes them); false when
# either is "?".
lag_above() {
  [[ "$1" != "?" && "$2" != "?" ]] && awk -v lag="$1" -v was="$2" 'BEGIN { exit !(lag + 0 > was + 0) }'
}

# caught_up F EPOCH SEQ - true when cortex_F follows and has caught up with a leader that was at
# EPOCH and SEQ one look (2 s) earlier: on that epoch, at least at that sequence number (so what
# the leader had written by then is on both), less than CAUGHT_UP_LAG seconds behind, and with no
# blob missing. "following" or "catching_up" alike: under steady writes a follower that keeps up
# flips between the two on every batch, and its lag_entries is counted against a head that may
# be older or newer than the batch (verify2 E2E-6), so neither says more than the lag and the
# sequence number do. Not "snapshot" or "no_leader": then it is not applying the leader's journal.
# The blobs, because a follower that started again from a copy of the leader's index (a fresh
# volume, a journal trimmed while it was away, a divergence) is at the leader's sequence number
# at once and fetches the blobs afterwards; handed the lead before it has them, it would answer
# 500 for every file whose blob it lacks.
caught_up() {
  local f=$1
  [[ "${ROLE[$f]}" == "follower" && "${STATE[$f]}" =~ ^(following|catching_up)$ &&
    "${EPOCH[$f]}" == "$2" && "${SEQ[$f]}" -ge "$3" && "${MISSING[$f]}" == "0" ]] &&
    lag_below "${LAG[$f]}" "${CAUGHT_UP_LAG}"
}

# wait_for_roles - until exactly one instance leads and the other follows it and has caught up
# (caught_up, against the leader's epoch and sequence number of the look before). Dies after
# CATCHUP_TIMEOUT; and early, after STALL_TIMEOUT, when the follower cannot reach the leader:
# it says "no_leader" all the time, or it applies no entry while it is behind and its lag grows.
# Waiting longer would not help then (a network fault, a wrong leader URL; verify2 D4: a
# follower behind an i/o timeout kept this script waiting the whole 600 s, and the message then
# suggested a longer CATCHUP_TIMEOUT). Not while it only copies the leader's index ("snapshot")
# and never said "no_leader" meanwhile: a large index takes a while to copy.
wait_for_roles() {
  local deadline=$((SECONDS + CATCHUP_TIMEOUT)) summary key last="" next_log=0 lead follower
  local head="" head_epoch="" head_seq="" twice=0
  # Of the follower that is watched for a stall: who follows whom, its sequence number, since
  # when it has not moved, its lag then, whether it said "no_leader" since, and since when it has
  # said "no_leader" without a break (empty while it does not).
  local stall_key="" stall_seq="" stall_since=0 stall_lag=0 stall_nl=no nl_since="" stuck=""
  local -a leading=()
  while :; do
    read_roles
    summary="$(roles_summary)"
    # A line when a role, an epoch or a state changes, else one every 30 s while the follower
    # catches up.
    key="$(roles_key)"
    if [[ "${key}" != "${last}" || "${SECONDS}" -ge "${next_log}" ]]; then
      log "${summary}"
      last="${key}"
      next_log=$((SECONDS + 30))
    fi
    read -r -a leading <<<"$(leaders)" || true
    if [[ "${#leading[@]}" -eq 1 ]]; then
      twice=0
      lead="${leading[0]}"
      follower="$(other "${lead}")"
      if [[ "${head}" == "${lead}" ]] && caught_up "${follower}" "${head_epoch}" "${head_seq}"; then
        log "${STACK}_${lead} leads, ${STACK}_${follower} follows it and has caught up (journal and blobs)"
        return 0
      fi
      head="${lead}" head_epoch="${EPOCH[$lead]}" head_seq="${SEQ[$lead]}"
      stuck=""
      if [[ "${ROLE[$follower]}" == "follower" ]]; then
        if [[ "${follower}>${lead}" != "${stall_key}" || "${SEQ[$follower]}" != "${stall_seq}" ]]; then
          stall_key="${follower}>${lead}" stall_seq="${SEQ[$follower]}" stall_since="${SECONDS}"
          stall_lag="${LAG[$follower]}" stall_nl=no nl_since=""
        fi
        if [[ "${STATE[$follower]}" == "no_leader" ]]; then
          stall_nl=yes
          nl_since="${nl_since:-${SECONDS}}"
        else
          nl_since=""
        fi
        if [[ -n "${nl_since}" && $((SECONDS - nl_since)) -ge "${STALL_TIMEOUT}" ]]; then
          stuck="has said no_leader for $((SECONDS - nl_since)) s"
        elif [[ "${SEQ[$follower]}" -lt "${SEQ[$lead]}" && $((SECONDS - stall_since)) -ge "${STALL_TIMEOUT}" &&
          ("${stall_nl}" == "yes" || "${STATE[$follower]}" != "snapshot") ]] &&
          lag_above "${LAG[$follower]}" "${stall_lag}"; then
          stuck="has applied no journal entry for $((SECONDS - stall_since)) s (it stays at ${SEQ[$follower]}, the leader is at ${SEQ[$lead]}) while its lag grew from ${stall_lag} to ${LAG[$follower]} s"
          if [[ "${stall_nl}" == "yes" ]]; then stuck+=", and said no_leader meanwhile"; fi
        fi
      else
        stall_key=""
        nl_since=""
      fi
      if [[ -n "${stuck}" ]]; then
        explain_instances
        if [[ "${stall_nl}" == "yes" ]]; then
          die "${STACK}_${follower} cannot reach the leader ${STACK}_${lead} at ${URL[$lead]}: it ${stuck} (${summary}). Waiting longer will not help: docker service logs ${STACK}_${follower} 2>&1 | grep replica.failed says why (above: the tasks, the last log lines and what each says). Fix the way between the two (the network ${NETWORK}, the URL the leader announces), then run this script again: it goes on where it stopped"
        fi
        die "${STACK}_${follower} does not get on with the leader ${STACK}_${lead} at ${URL[$lead]}: it ${stuck}, state ${STATE[$follower]} (${summary}). It cannot reach the leader, or is stuck on one entry (a blob it cannot fetch): docker service logs ${STACK}_${follower} 2>&1 | grep replica.failed says why (above: the tasks, the last log lines and what each says). Only if it fetches one very large blob: STALL_TIMEOUT=<seconds> bash ${BETULA_ROOT}/vps/48-cortex.sh ${TAG} goes on where this stopped"
      fi
    else
      head=""
      stall_key=""
      nl_since=""
      if [[ "${#leading[@]}" -gt 1 ]]; then
        twice=$((twice + 1))
        [[ "${twice}" -lt 3 ]] || two_leaders
      else
        twice=0
      fi
    fi
    if [[ "${SECONDS}" -ge "${deadline}" ]]; then
      explain_instances
      die "after ${CATCHUP_TIMEOUT} s still not one leader with a follower that has caught up: following or catching_up, on the leader's epoch and at least at its sequence number of 2 s before, less than ${CAUGHT_UP_LAG} s behind, 0 blobs missing (${summary}; above: the tasks, the last log lines and what each says). A follower that fetches the blobs of a whole index (blobs missing above) may need longer: CATCHUP_TIMEOUT=<seconds> bash ${BETULA_ROOT}/vps/48-cortex.sh ${TAG} goes on where this stopped"
    fi
    sleep 2
  done
}

# hand_over X - cortex_X leads and steps down; the other instance, which follows it and has
# caught up, every blob included (wait_for_roles before), takes the lock over. Dies when it does
# not lead within TAKEOVER_TIMEOUT; returns STEP_DOWN_PAUSE after it leads, so that the old
# leader is not stopped under the clients that still write through it.
hand_over() {
  local from=$1 to cid deadline
  to="$(other "${from}")"
  cid="$(service_container "${STACK}_${from}")"
  [[ -n "${cid}" ]] || die "${STACK}_${from} leads, but no container of it runs on this node"
  log "${STACK}_${from} steps down, ${STACK}_${to} takes over"
  timeout 30 docker exec "${cid}" /bin/cortex step-down >/dev/null ||
    die "\"cortex step-down\" failed in ${STACK}_${from} (above: why); it was not updated"
  deadline=$((SECONDS + TAKEOVER_TIMEOUT))
  while :; do
    read_roles
    if [[ "${ROLE[$to]}" == "leader" ]]; then
      log "${STACK}_${to} leads (epoch ${EPOCH[$to]}); ${STEP_DOWN_PAUSE} s for the clients to move over to it"
      sleep "${STEP_DOWN_PAUSE}"
      return 0
    fi
    if [[ "${SECONDS}" -ge "${deadline}" ]]; then
      explain_instances
      die "${STACK}_${to} did not take over within ${TAKEOVER_TIMEOUT} s of the step-down ($(roles_summary)); ${STACK}_${from} was not updated and campaigns again 15 s after its step-down"
    fi
    sleep 1
  done
}

# ---------------------------------------------------------------- deploying

# check_told X - what swarm was told for cortex_X, not what this script meant (stacks/betula.yml:
# the substitution of "docker stack deploy" has surprises, and a wrong image is retried forever).
check_told() {
  local svc="${STACK}_$1" image rev
  image="$(service_image "${svc}")"
  [[ "${image}" == "${CORTEX_IMAGE}" ]] ||
    die "service ${svc} was given the image '${image}' instead of ${CORTEX_IMAGE}: look at the image line of ${STACK_FILE}"
  rev="$(service_label "${svc}" "${REV_LABEL}")"
  [[ "${rev}" == "${CORTEX_REV}" ]] ||
    die "service ${svc} was given the revision '${rev}' instead of ${CORTEX_REV}: look at the label ${REV_LABEL} in ${STACK_FILE}"
}

# converge WHAT - until every service of the stack runs (wait_for_stack, lib-stacks.sh); dies
# when swarm gives up or the time is up.
converge() {
  wait_for_stack "${STACK}"
  [[ "${#FAILED_STACKS[@]}" -eq 0 ]] ||
    die "$1 did not converge (above: docker service ps and the last log lines). A failed update was rolled back by swarm; fix the cause and run this script again"
}

# converge_instance X - wait_for_stack for cortex_X alone: until it runs what it was given (every
# task running and healthy, no update in flight; service_state, lib-stacks.sh), twice in a row
# 5 s apart. Not the whole stack: the other instance, which was not deployed, may still show how
# an earlier update of it ended ("rollback_completed" until its next update). Dies when swarm
# gives up (the update was rolled back) or after CONVERGE_TIMEOUT.
converge_instance() {
  local svc="${STACK}_$1" state last="" streak=0 deadline=$((SECONDS + CONVERGE_TIMEOUT))
  # Give the orchestrator a moment to turn the new definition into an update.
  sleep 3
  while :; do
    state="$(service_state "${svc}")"
    if [[ "${state}" != "${last}" ]]; then
      log "${svc}: ${state}"
      last="${state}"
    fi
    case "${state}" in
      ok\ *)
        streak=$((streak + 1))
        if [[ "${streak}" -ge 2 ]]; then
          log "${svc}: converged"
          return 0
        fi
        ;;
      failed\ *) break ;;
      *) streak=0 ;;
    esac
    if [[ "${SECONDS}" -ge "${deadline}" ]]; then
      warn "${svc}: not converged after ${CONVERGE_TIMEOUT} s"
      break
    fi
    sleep 5
  done
  printf '\n---- docker service ps --no-trunc %s\n' "${svc}" >&2
  docker service ps --no-trunc "${svc}" >&2 || true
  printf -- '---- docker service logs --tail 15 %s\n' "${svc}" >&2
  docker service logs --no-task-ids --tail 15 "${svc}" >&2 2>&1 || true
  die "${svc} did not converge (above: docker service ps and the last log lines). A failed update was rolled back by swarm, and the other instance was not deployed; fix the cause and run this script again"
}

# undrift X - "docker stack deploy" leaves the image of a service alone while the file names the
# image the stack gave it last: the CLI compares the file with the service's label
# com.docker.stack.image, not with the image the service runs. A service that was given another
# image by hand (docker service update --image) would keep it, so it gets this deploy's image
# directly first. (Found 2026-10-02 on a test swarm: check_told refused the deploy.)
undrift() {
  local svc="${STACK}_$1" image label
  docker service inspect "${svc}" >/dev/null 2>&1 || return 0
  image="$(service_image "${svc}")"
  label="$(service_label "${svc}" com.docker.stack.image)"
  [[ "${image}" != "${label}" && "${label}" == "${CORTEX_IMAGE}" ]] || return 0
  warn "${svc} runs ${image}, not ${label}, which the stack gave it (docker service update by hand?): it is given ${CORTEX_IMAGE} directly first"
  docker service update --detach --quiet --no-resolve-image --image "${CORTEX_IMAGE}" "${svc}" >/dev/null
}

# deploy_all - both instances at once, from the whole file: the first deploy, or no leader to keep.
deploy_all() {
  local x
  for x in "${INSTANCES[@]}"; do
    undrift "${x}"
  done
  # never: the default asks a registry for the digest of the tag, and no registry knows the image.
  STACK_DEPLOY_ARGS=(--resolve-image never)
  deploy_stack "${STACK}" "${STACK_FILE}"
  for x in "${INSTANCES[@]}"; do
    check_told "${x}"
  done
  converge "stack ${STACK}"
}

# deploy_instance X - cortex_X alone, as the current STACK_FILE says it; the other service is
# not touched, and that is checked as well.
deploy_instance() {
  local x=$1 y file before
  y="$(other "${x}")"
  file="$(betula_tmpfile)"
  instance_file "${x}" >"${file}"
  [[ "$(file_services "${file}")" == "${x}" ]] ||
    die "${STACK_FILE} without the other service still defines '$(file_services "${file}" | tr '\n' ' ')' instead of ${x} alone: look at its layout (its head)"
  require_bind_sources "${file}"
  before="$(service_image "${STACK}_${y}") $(service_label "${STACK}_${y}" "${REV_LABEL}")"
  undrift "${x}"
  log "docker stack deploy ${STACK}: ${STACK_FILE##*/}, service ${x} alone"
  # Without --prune: the other service is not in this file, and stays as it is.
  docker stack deploy --detach=true --resolve-image never -c "${file}" "${STACK}"
  check_told "${x}"
  [[ "$(service_image "${STACK}_${y}") $(service_label "${STACK}_${y}" "${REV_LABEL}")" == "${before}" ]] ||
    die "deploying ${STACK}_${x} alone changed ${STACK}_${y} as well: look at the layout of ${STACK_FILE} (its head)"
}

# update - one instance at a time, the leader last (see the head of this file).
update() {
  local x
  local -a todo=() order=()
  for x in "${INSTANCES[@]}"; do
    if needs_update "${x}"; then
      todo+=("${x}")
    fi
  done
  if [[ "${#todo[@]}" -eq 0 ]]; then
    log "${STACK}_a and ${STACK}_b run ${CORTEX_IMAGE}, from revision ${CORTEX_REV} of ${STACK_FILE##*/}: nothing to change"
    return 0
  fi
  find_leader
  if [[ -z "${LEADER}" ]]; then
    warn "there is no single leader to keep, so both are deployed at once"
    deploy_all
    return 0
  fi
  # The leader last: it leads until the follower runs the new release and has caught up, and then
  # hands over to it.
  for x in "${todo[@]}"; do
    if [[ "${x}" != "${LEADER}" ]]; then order+=("${x}"); fi
  done
  for x in "${todo[@]}"; do
    if [[ "${x}" == "${LEADER}" ]]; then order+=("${x}"); fi
  done
  for x in "${order[@]}"; do
    step "${STACK}_${x}: ${CORTEX_IMAGE}, revision ${CORTEX_REV}"
    if docker service inspect "${STACK}_${x}" >/dev/null 2>&1; then
      log "it runs $(service_image "${STACK}_${x}"), revision $(service_label "${STACK}_${x}" "${REV_LABEL}")"
    else
      log "it does not exist (any more)"
    fi
    read_roles
    if [[ "${ROLE[$x]}" == "leader" ]]; then
      wait_for_roles
      # The look in wait_for_roles is the newer one: is it still the leader?
      if [[ "${ROLE[$x]}" == "leader" ]]; then
        hand_over "${x}"
      fi
    fi
    deploy_instance "${x}"
    converge_instance "${x}"
    wait_for_roles
  done
}

# ---------------------------------------------------------------- steps

preflight() {
  step "Preconditions"
  local facts svc services
  if [[ "${EUID}" -eq 0 && -n "${SUDO_USER:-}" ]]; then
    die "run this as ${DEPLOY_USER} WITHOUT sudo: docker stack deploy reads CORTEX_IMAGE from the environment, and sudo resets it"
  fi
  [[ "${BETULA_VPS_DIR}" == "${BETULA_ROOT}/vps" ]] ||
    die "this copy lives in ${BETULA_VPS_DIR}; the stack file is read from ${STACKS_DIR}, so run ${BETULA_ROOT}/vps/$(basename "$0")"
  [[ "${CONVERGE_TIMEOUT}" =~ ^[0-9]{1,5}$ ]] || die "CONVERGE_TIMEOUT='${CONVERGE_TIMEOUT}' is not a number of seconds"
  [[ "${CATCHUP_TIMEOUT}" =~ ^[0-9]{1,5}$ ]] || die "CATCHUP_TIMEOUT='${CATCHUP_TIMEOUT}' is not a number of seconds"
  [[ "${STALL_TIMEOUT}" =~ ^[0-9]{1,5}$ ]] || die "STALL_TIMEOUT='${STALL_TIMEOUT}' is not a number of seconds"
  require_cmd docker jq timeout sha256sum awk
  require_swarm_manager
  # Internal: the application's way to Cortex must not be a way to the internet (vps/30-docker.sh).
  facts="$(docker network inspect "${NETWORK}" --format '{{.Driver}} {{.Scope}} {{.Attachable}} {{.Internal}}' 2>/dev/null || true)"
  [[ "${facts}" == "overlay swarm true true" ]] ||
    die "overlay network ${NETWORK} is missing or not internal and attachable ('${facts:-missing}', wanted 'overlay swarm true true'; run sudo bash ${BETULA_ROOT}/vps/30-docker.sh)"

  log "release ${TAG}"
  docker image inspect "${CORTEX_IMAGE}" >/dev/null 2>&1 ||
    die "image ${CORTEX_IMAGE} is not loaded on this server (deploy/ship-cortex.sh builds and loads it; loaded: $(loaded_images))"

  [[ -f "${STACK_FILE}" ]] || die "stack file missing: ${STACK_FILE} (run deploy/sync.sh)"
  assert_lf "${STACK_FILE}"
  services="$(file_services "${STACK_FILE}" | tr '\n' ' ')"
  services="${services% }"
  [[ "${services}" == "a b" ]] ||
    die "${STACK_FILE} has to define exactly the services a and b, two spaces deep under \"services:\" (its head says why), not: ${services:-none}"
  [[ -f "${HOSTS_FILE}" ]] || die "${HOSTS_FILE} does not exist (run deploy/sync.sh)"
  assert_lf "${HOSTS_FILE}"
  jq -e 'type == "object"' "${HOSTS_FILE}" >/dev/null 2>&1 ||
    die "${HOSTS_FILE} is not a JSON object, and Cortex would refuse it (jq . ${HOSTS_FILE} says where it breaks)"

  if stack_exists "${STACK}"; then
    # Deployed alone, an instance leaves every other service of the stack as it is: one that the
    # file does not name would stay, and could campaign for the lock as well.
    while IFS= read -r svc; do
      [[ -z "${svc}" || "${svc}" == "${STACK}_a" || "${svc}" == "${STACK}_b" ]] ||
        die "stack ${STACK} has the service ${svc}, which ${STACK_FILE} does not define: remove it (docker service rm ${svc}) and run this again"
    done < <(stack_services "${STACK}")
  fi
}

deploy() {
  if stack_exists "${STACK}"; then
    step "Stack ${STACK}: one instance at a time"
    update
  else
    step "Stack ${STACK}: the first deploy (both instances)"
    deploy_all
  fi
  step "One leader, and the follower has caught up"
  wait_for_roles
}

report() {
  step "Done"
  docker stack services "${STACK}" --format '{{.Name}}  {{.Replicas}}  {{.Image}}' | sed 's/^/  /'
  log "$(roles_summary)"
  log "the application reaches it at http://${STACK}_a:8100 and http://${STACK}_b:8100 over the network ${NETWORK}; 50-app.sh <instance> points an instance's Radix at it while this stack runs"
  log "next: bash ${BETULA_ROOT}/vps/91-verify-stacks.sh cortex"
}

# ---------------------------------------------------------------- main

[[ "$#" -le 1 && "${1:-}" != -* ]] || usage

TAG="${1:-}"
if [[ -z "${TAG}" ]]; then
  TAG="$(running_tag a)"
  tag_b="$(running_tag b)"
  [[ -n "${TAG}" || -n "${tag_b}" ]] ||
    die "Cortex is not deployed yet, so there is no release to keep: name a tag (loaded: $(loaded_images))"
  [[ "${tag_b}" == "${TAG}" ]] ||
    die "${STACK}_a and ${STACK}_b run different releases (${TAG:-none} / ${tag_b:-none}), the rest of an update that stopped half way: name the tag to deploy"
fi
[[ "${TAG}" =~ ${TAG_PATTERN} && "${TAG}" != "latest" ]] ||
  die "'${TAG}' is not a release tag. Never \"latest\": swarm compares the service definition, not the image content, so a re-loaded \"latest\" restarts nothing"
CORTEX_IMAGE="betula-cortex:${TAG}"

preflight
# Substituted into the stack file by "docker stack deploy".
CORTEX_REV="$(stack_rev)"
export CORTEX_IMAGE CORTEX_REV
log "revision ${CORTEX_REV} of ${STACK_FILE##*/}"
deploy
report
