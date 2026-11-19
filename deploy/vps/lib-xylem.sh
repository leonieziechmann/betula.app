#!/usr/bin/env bash
# Helpers of Xylem, the service of Betula's notifications (docs/xylem/xylem.md), shared by
# 47-xylem-host.sh (root: its file systems), 49-xylem.sh (deploy: its stack) and
# 91-verify-stacks.sh. Sourced AFTER lib.sh and lib-stacks.sh, never executed. On its own beside
# lib-phloem.sh on purpose: the two services share a shape, not a script, so that a change for
# one never touches the other's deploy.
# shellcheck shell=bash

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  echo "lib-xylem.sh is a library; source it after lib.sh and lib-stacks.sh" >&2
  exit 2
fi

# The instances: one file each, stacks/xylem/<stack>.env. Not stacks/<stack>.env: every file
# there is an instance of the application (lib-stacks.sh, instance_names).
XYLEM_ENV_DIR="${STACKS_DIR}/xylem"
XYLEM_STACK_FILE="${STACKS_DIR}/xylem.yml"
# The user Xylem runs as in its image (flake.nix: "xylem start" drops to it before it serves).
XYLEM_UID=65532
# The tmpfs of the stats file: what one file can bring to the monitoring is bounded by it.
XYLEM_STATS_SIZE="1m"
# Where the site names what Betula announces by itself (folia/crates/server/src/announcements.rs).
XYLEM_FEED_PATH="/api/announcements"

# Set by load_xylem, of one instance.
XYLEM_STACK=""
XYLEM_HOST=""
XYLEM_LANGUAGE=""
XYLEM_CONTACT_ADDRESS="" # where a push service's operators reach Xylem's; empty: the site
XYLEM_FEED_SWITCH=""     # on or off
XYLEM_FEED_URL=""        # the feed's address while it is on, else empty
XYLEM_MAX=""
XYLEM_SIZE=""
XYLEM_SIZE_BYTES=""

# xylem_names -> the instances that have a file stacks/xylem/<stack>.env, one per line.
xylem_names() {
  local file
  for file in "${XYLEM_ENV_DIR}"/*.env; do
    [[ -f "${file}" ]] || continue
    file="${file##*/}"
    printf '%s\n' "${file%.env}"
  done
}

# xylem_size_bytes SIZE -> the bytes of 256M, 1G or 1T (powers of 1024, as truncate and the file
# system count them); nothing when SIZE is not of that form.
xylem_size_bytes() {
  [[ "$1" =~ ^([1-9][0-9]{0,5})([MGT])$ ]] || return 0
  local n=${BASH_REMATCH[1]}
  case "${BASH_REMATCH[2]}" in
    M) printf '%s' $((n << 20)) ;;
    G) printf '%s' $((n << 30)) ;;
    T) printf '%s' $((n << 40)) ;;
  esac
}

# load_xylem NAME - reads stacks/xylem/NAME.env into the variables above. The file is read, never
# sourced: a value is data, whatever it looks like.
load_xylem() {
  local name=$1 file line key value
  file="${XYLEM_ENV_DIR}/${name}.env"
  # The name is its stack, its networks <stack>_in and <stack>_out, its router <stack>-push and
  # its host paths: "xylem" in front keeps all of them apart from every other stack's.
  [[ "${name}" =~ ^xylem(-[a-z0-9]+)*$ && "${#name}" -le 30 ]] ||
    die "'${name}' is not a name of an instance of Xylem (xylem, or xylem-<lower case letters and digits>)"
  [[ -f "${file}" ]] ||
    die "there is no instance '${name}' of Xylem: ${file} does not exist (known: $(xylem_names | tr '\n' ' '))"
  assert_lf "${file}"
  XYLEM_STACK=""
  XYLEM_HOST=""
  XYLEM_LANGUAGE=""
  XYLEM_CONTACT_ADDRESS=""
  XYLEM_FEED_SWITCH=""
  XYLEM_FEED_URL=""
  XYLEM_MAX=""
  XYLEM_SIZE=""
  while IFS= read -r line || [[ -n "${line}" ]]; do
    [[ -n "${line}" && "${line}" != \#* ]] || continue
    [[ "${line}" == *=* ]] || die "${file}: '${line}' is not NAME=value"
    key="${line%%=*}"
    value="${line#*=}"
    case "${key}" in
      STACK_NAME) XYLEM_STACK="${value}" ;;
      APP_HOST) XYLEM_HOST="${value}" ;;
      XYLEM_LANG) XYLEM_LANGUAGE="${value}" ;;
      XYLEM_CONTACT) XYLEM_CONTACT_ADDRESS="${value}" ;;
      XYLEM_FEED) XYLEM_FEED_SWITCH="${value}" ;;
      XYLEM_MAX_SUBSCRIPTIONS) XYLEM_MAX="${value}" ;;
      XYLEM_DATA_SIZE) XYLEM_SIZE="${value}" ;;
      *) die "${file}: unknown setting ${key} (known: STACK_NAME, APP_HOST, XYLEM_LANG, XYLEM_CONTACT, XYLEM_FEED, XYLEM_MAX_SUBSCRIPTIONS, XYLEM_DATA_SIZE)" ;;
    esac
  done <"${file}"
  [[ "${XYLEM_STACK}" == "${name}" ]] ||
    die "${file}: STACK_NAME is '${XYLEM_STACK}', the file says '${name}'; they have to agree"
  [[ "${XYLEM_HOST}" =~ ^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,}$ ]] ||
    die "${file}: APP_HOST '${XYLEM_HOST}' is not a host name"
  [[ "${XYLEM_LANGUAGE}" =~ ^[a-z]{2}$ ]] ||
    die "${file}: XYLEM_LANG '${XYLEM_LANGUAGE}' is not a language (two lower case letters: de)"
  # A mailto: or https: address without anything a YAML string or a shell would read as its own.
  [[ -z "${XYLEM_CONTACT_ADDRESS}" || "${XYLEM_CONTACT_ADDRESS}" =~ ^(mailto:[A-Za-z0-9._+-]+@[a-z0-9.-]+|https://[a-z0-9.-]+(/[A-Za-z0-9._~/-]*)?)$ ]] ||
    die "${file}: XYLEM_CONTACT '${XYLEM_CONTACT_ADDRESS}' is not a mailto: or https: address"
  case "${XYLEM_FEED_SWITCH}" in
    on) XYLEM_FEED_URL="https://${XYLEM_HOST}${XYLEM_FEED_PATH}" ;;
    off) XYLEM_FEED_URL="" ;;
    *) die "${file}: XYLEM_FEED is '${XYLEM_FEED_SWITCH}', not on or off" ;;
  esac
  [[ "${XYLEM_MAX}" =~ ^[1-9][0-9]{0,8}$ ]] ||
    die "${file}: XYLEM_MAX_SUBSCRIPTIONS '${XYLEM_MAX}' is not a number of subscriptions"
  XYLEM_SIZE_BYTES="$(xylem_size_bytes "${XYLEM_SIZE}")"
  [[ -n "${XYLEM_SIZE_BYTES}" ]] || die "${file}: XYLEM_DATA_SIZE '${XYLEM_SIZE}' is not a size like 1G or 256M"
  [[ "${XYLEM_SIZE_BYTES}" -ge $((64 << 20)) ]] ||
    die "${file}: XYLEM_DATA_SIZE '${XYLEM_SIZE}' is less than 64M: no room for an ext4 with its journal"
  # A subscription takes about 500 bytes with its topics and indexes; 2 KiB each leaves room for
  # the announcements, the WAL and SQLite's free pages. A full file system would fail every write.
  [[ $((XYLEM_MAX * 2048)) -le $((XYLEM_SIZE_BYTES * 80 / 100)) ]] ||
    die "${file}: XYLEM_MAX_SUBSCRIPTIONS (${XYLEM_MAX}) at 2 KiB each needs more than 80 % of XYLEM_DATA_SIZE (${XYLEM_SIZE}); make the file system larger or the number smaller"
}

# The host paths of an instance (stacks/xylem.yml).
# xylem_dir STACK -> the directory of the instance: the image of its file system and the place it
# is mounted.
xylem_dir() { printf '/var/lib/betula/xylem/%s' "$1"; }
# xylem_image_file STACK -> the file that holds its file system (a sparse file, on a loop device).
xylem_image_file() { printf '%s/data.img' "$(xylem_dir "$1")"; }
# xylem_data_dir STACK -> where that file system is mounted: /data in the container.
xylem_data_dir() { printf '%s/data' "$(xylem_dir "$1")"; }
# xylem_stats_dir STACK -> the tmpfs of its stats file: /stats in the container; Alloy reads it
# through /rootfs.
xylem_stats_dir() { printf '/run/betula/xylem/%s/stats' "$1"; }

# xylem_mount_facts PATH -> "<fstype> <options> <uid> <mode>" of the file system mounted at
# exactly PATH (nothing when none is: a directory of another file system does not count).
xylem_mount_facts() {
  local line
  line="$(findmnt -n -M "$1" -o FSTYPE,OPTIONS 2>/dev/null || true)"
  [[ -n "${line}" ]] || return 0
  printf '%s %s' "${line}" "$(stat -c '%u %a' "$1")"
}

# xylem_check_mounts STACK - true when both file systems of the instance are in place: its data
# on an ext4 of its own, the stats on a tmpfs of its own, both nosuid,nodev,noexec and Xylem's.
# Otherwise false, with the reason in XYLEM_MOUNT_DETAIL.
XYLEM_MOUNT_DETAIL=""
xylem_check_mounts() {
  local stack=$1 path facts fstype options uid mode want_fs want_mode opt
  for path in "$(xylem_data_dir "${stack}")" "$(xylem_stats_dir "${stack}")"; do
    # The data are Xylem's alone; the stats file is read by Alloy (as root, through /rootfs).
    if [[ "${path}" == /var/* ]]; then
      want_fs="ext4" want_mode="700"
    else
      want_fs="tmpfs" want_mode="755"
    fi
    facts="$(xylem_mount_facts "${path}")"
    if [[ -z "${facts}" ]]; then
      XYLEM_MOUNT_DETAIL="nothing is mounted at ${path} (sudo bash ${BETULA_ROOT}/vps/47-xylem-host.sh ${stack})"
      return 1
    fi
    read -r fstype options uid mode <<<"${facts}"
    if [[ "${fstype}" != "${want_fs}" ]]; then
      XYLEM_MOUNT_DETAIL="${path} is ${fstype}, wanted ${want_fs} (47-xylem-host.sh)"
      return 1
    fi
    for opt in nosuid nodev noexec; do
      if [[ ",${options}," != *",${opt},"* ]]; then
        XYLEM_MOUNT_DETAIL="${path} is mounted without ${opt} (${options})"
        return 1
      fi
    done
    if [[ "${uid}" != "${XYLEM_UID}" || "${mode}" != "${want_mode}" ]]; then
      XYLEM_MOUNT_DETAIL="${path} belongs to uid ${uid} with mode ${mode}, wanted ${XYLEM_UID} (Xylem's user) and ${want_mode} (47-xylem-host.sh)"
      return 1
    fi
    if [[ "${want_fs}" == "tmpfs" && ",${options}," != *",size=1024k,"* ]]; then
      XYLEM_MOUNT_DETAIL="${path} is a tmpfs of another size (${options}), wanted 1 MiB"
      return 1
    fi
  done
  XYLEM_MOUNT_DETAIL=""
}
