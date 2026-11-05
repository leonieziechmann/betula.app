#!/usr/bin/env bash
# Helpers of Phloem, the service of Betula Sync (docs/phloem/phloem.md), shared by
# 47-phloem-host.sh (root: its file systems) and 49-phloem.sh (deploy: its stack). Sourced AFTER
# lib.sh and lib-stacks.sh, never executed.
# shellcheck shell=bash

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  echo "lib-phloem.sh is a library; source it after lib.sh and lib-stacks.sh" >&2
  exit 2
fi

# The instances: one file each, stacks/phloem/<stack>.env. Not stacks/<stack>.env: every file
# there is an instance of the application (lib-stacks.sh, instance_names).
PHLOEM_ENV_DIR="${STACKS_DIR}/phloem"
PHLOEM_STACK_FILE="${STACKS_DIR}/phloem.yml"
# The user Phloem runs as in its image (flake.nix: "phloem start" drops to it before it serves).
PHLOEM_UID=65532
# The tmpfs of the stats file: what one file can bring to the monitoring is bounded by it.
PHLOEM_STATS_SIZE="1m"

# Set by load_phloem, of one instance.
PHLOEM_STACK=""
PHLOEM_HOST=""
PHLOEM_ORIGIN_LIST=""
PHLOEM_BUDGET=""
PHLOEM_SIZE=""
PHLOEM_SIZE_BYTES=""

# phloem_names -> the instances that have a file stacks/phloem/<stack>.env, one per line.
phloem_names() {
  local file
  for file in "${PHLOEM_ENV_DIR}"/*.env; do
    [[ -f "${file}" ]] || continue
    file="${file##*/}"
    printf '%s\n' "${file%.env}"
  done
}

# size_bytes SIZE -> the bytes of 512M, 56G or 1T (powers of 1024, as truncate and the file
# system count them); nothing when SIZE is not of that form.
size_bytes() {
  [[ "$1" =~ ^([1-9][0-9]{0,5})([MGT])$ ]] || return 0
  local n=${BASH_REMATCH[1]}
  case "${BASH_REMATCH[2]}" in
    M) printf '%s' $((n << 20)) ;;
    G) printf '%s' $((n << 30)) ;;
    T) printf '%s' $((n << 40)) ;;
  esac
}

# load_phloem NAME - reads stacks/phloem/NAME.env into PHLOEM_STACK, PHLOEM_HOST,
# PHLOEM_ORIGIN_LIST, PHLOEM_BUDGET, PHLOEM_SIZE and PHLOEM_SIZE_BYTES. The file is read, never
# sourced: a value is data, whatever it looks like.
load_phloem() {
  local name=$1 file line key value origin
  file="${PHLOEM_ENV_DIR}/${name}.env"
  # The name is its stack, its network <stack>_in, its router <stack>-sync and its host paths:
  # "phloem" in front keeps all of them apart from every other stack's.
  [[ "${name}" =~ ^phloem(-[a-z0-9]+)*$ && "${#name}" -le 30 ]] ||
    die "'${name}' is not a name of an instance of Phloem (phloem, or phloem-<lower case letters and digits>)"
  [[ -f "${file}" ]] ||
    die "there is no instance '${name}' of Phloem: ${file} does not exist (known: $(phloem_names | tr '\n' ' '))"
  assert_lf "${file}"
  PHLOEM_STACK=""
  PHLOEM_HOST=""
  PHLOEM_ORIGIN_LIST=""
  PHLOEM_BUDGET=""
  PHLOEM_SIZE=""
  while IFS= read -r line || [[ -n "${line}" ]]; do
    [[ -n "${line}" && "${line}" != \#* ]] || continue
    [[ "${line}" == *=* ]] || die "${file}: '${line}' is not NAME=value"
    key="${line%%=*}"
    value="${line#*=}"
    case "${key}" in
      STACK_NAME) PHLOEM_STACK="${value}" ;;
      APP_HOST) PHLOEM_HOST="${value}" ;;
      PHLOEM_ORIGINS) PHLOEM_ORIGIN_LIST="${value}" ;;
      PHLOEM_BUDGET_BYTES) PHLOEM_BUDGET="${value}" ;;
      PHLOEM_DATA_SIZE) PHLOEM_SIZE="${value}" ;;
      *) die "${file}: unknown setting ${key} (known: STACK_NAME, APP_HOST, PHLOEM_ORIGINS, PHLOEM_BUDGET_BYTES, PHLOEM_DATA_SIZE)" ;;
    esac
  done <"${file}"
  [[ "${PHLOEM_STACK}" == "${name}" ]] ||
    die "${file}: STACK_NAME is '${PHLOEM_STACK}', the file says '${name}'; they have to agree"
  [[ "${PHLOEM_HOST}" =~ ^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,}$ ]] ||
    die "${file}: APP_HOST '${PHLOEM_HOST}' is not a host name"
  [[ -n "${PHLOEM_ORIGIN_LIST}" ]] || die "${file}: PHLOEM_ORIGINS is empty (the site's own origin: https://${PHLOEM_HOST})"
  for origin in ${PHLOEM_ORIGIN_LIST//,/ }; do
    [[ "${origin}" =~ ^https?://([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)*[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(:[0-9]{1,5})?$ ]] ||
      die "${file}: PHLOEM_ORIGINS names '${origin}', which is no origin (scheme://host[:port], lower case, comma-separated, no spaces)"
  done
  [[ ",${PHLOEM_ORIGIN_LIST}," == *",https://${PHLOEM_HOST},"* ]] ||
    warn "${file}: PHLOEM_ORIGINS (${PHLOEM_ORIGIN_LIST}) does not name the site's own origin https://${PHLOEM_HOST}: its pages could not use Phloem"
  [[ "${PHLOEM_BUDGET}" =~ ^[1-9][0-9]{6,14}$ ]] ||
    die "${file}: PHLOEM_BUDGET_BYTES '${PHLOEM_BUDGET}' is not a number of bytes (at least 1000000)"
  PHLOEM_SIZE_BYTES="$(size_bytes "${PHLOEM_SIZE}")"
  [[ -n "${PHLOEM_SIZE_BYTES}" ]] || die "${file}: PHLOEM_DATA_SIZE '${PHLOEM_SIZE}' is not a size like 3G or 512M"
  # The budget is Phloem's own limit, the file system the one it cannot pass: at the budget it
  # says 507 to new spaces and parcels; on a full file system every write would fail.
  [[ "${PHLOEM_BUDGET}" -le $((PHLOEM_SIZE_BYTES * 95 / 100)) ]] ||
    die "${file}: PHLOEM_BUDGET_BYTES (${PHLOEM_BUDGET}) leaves less than 5 % of PHLOEM_DATA_SIZE (${PHLOEM_SIZE}) for SQLite's journal and pages; make the file system larger or the budget smaller"
}

# The host paths of an instance (stacks/phloem.yml, docs/phloem/protocol.md section 1).
# phloem_dir STACK -> the directory of the instance: the image of its file system and the place
# it is mounted.
phloem_dir() { printf '/var/lib/betula/phloem/%s' "$1"; }
# phloem_image_file STACK -> the file that holds its file system (a sparse file, on a loop device).
phloem_image_file() { printf '%s/data.img' "$(phloem_dir "$1")"; }
# phloem_data_dir STACK -> where that file system is mounted: /data in the container.
phloem_data_dir() { printf '%s/data' "$(phloem_dir "$1")"; }
# phloem_stats_dir STACK -> the tmpfs of its stats file: /stats in the container; Alloy reads it
# through /rootfs.
phloem_stats_dir() { printf '/run/betula/phloem/%s/stats' "$1"; }

# phloem_mount_facts PATH -> "<fstype> <options> <uid> <mode>" of the file system mounted at
# exactly PATH (nothing when none is: a directory of another file system does not count).
phloem_mount_facts() {
  local line
  line="$(findmnt -n -M "$1" -o FSTYPE,OPTIONS 2>/dev/null || true)"
  [[ -n "${line}" ]] || return 0
  printf '%s %s' "${line}" "$(stat -c '%u %a' "$1")"
}

# phloem_check_mounts STACK - true when both file systems of the instance are in place: its data
# on an ext4 of its own, the stats on a tmpfs of its own, both nosuid,nodev,noexec and Phloem's.
# Otherwise false, with the reason in PHLOEM_MOUNT_DETAIL.
PHLOEM_MOUNT_DETAIL=""
phloem_check_mounts() {
  local stack=$1 path facts fstype options uid mode want_fs want_mode opt
  for path in "$(phloem_data_dir "${stack}")" "$(phloem_stats_dir "${stack}")"; do
    # The data are Phloem's alone; the stats file is read by Alloy (as root, through /rootfs).
    if [[ "${path}" == /var/* ]]; then
      want_fs="ext4" want_mode="700"
    else
      want_fs="tmpfs" want_mode="755"
    fi
    facts="$(phloem_mount_facts "${path}")"
    if [[ -z "${facts}" ]]; then
      PHLOEM_MOUNT_DETAIL="nothing is mounted at ${path} (sudo bash ${BETULA_ROOT}/vps/47-phloem-host.sh ${stack})"
      return 1
    fi
    read -r fstype options uid mode <<<"${facts}"
    if [[ "${fstype}" != "${want_fs}" ]]; then
      PHLOEM_MOUNT_DETAIL="${path} is ${fstype}, wanted ${want_fs} (47-phloem-host.sh)"
      return 1
    fi
    for opt in nosuid nodev noexec; do
      if [[ ",${options}," != *",${opt},"* ]]; then
        PHLOEM_MOUNT_DETAIL="${path} is mounted without ${opt} (${options})"
        return 1
      fi
    done
    if [[ "${uid}" != "${PHLOEM_UID}" || "${mode}" != "${want_mode}" ]]; then
      PHLOEM_MOUNT_DETAIL="${path} belongs to uid ${uid} with mode ${mode}, wanted ${PHLOEM_UID} (Phloem's user) and ${want_mode} (47-phloem-host.sh)"
      return 1
    fi
    if [[ "${want_fs}" == "tmpfs" && ",${options}," != *",size=1024k,"* ]]; then
      PHLOEM_MOUNT_DETAIL="${path} is a tmpfs of another size (${options}), wanted 1 MiB"
      return 1
    fi
  done
  PHLOEM_MOUNT_DETAIL=""
}
