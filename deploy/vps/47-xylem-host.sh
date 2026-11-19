#!/usr/bin/env bash
# 47-xylem-host.sh - the host's part of Xylem, the service of Betula's notifications
# (docs/xylem/xylem.md, stacks/xylem.yml): for each instance, the only two places its container
# may write to. The same as 47-phloem-host.sh does for Phloem, and kept beside it. Run as root,
# before the first deploy of an instance (49-xylem.sh refuses to deploy without them) and after a
# change to its XYLEM_DATA_SIZE (deploy/sync.sh brings stacks/xylem/<stack>.env):
#
#   sudo bash /opt/betula/vps/47-xylem-host.sh                  # every instance in stacks/xylem/
#   sudo bash /opt/betula/vps/47-xylem-host.sh xylem-canary     # this one
#
# For each instance <stack>; running it again changes nothing that is as it should be:
#   1. Its data, /var/lib/betula/xylem/<stack>/data: an ext4 of its own, of fixed size
#      (XYLEM_DATA_SIZE), in the sparse file data.img next to it, on a loop device; mounted
#      nosuid,nodev,noexec, its root Xylem's (uid 65532, mode 0700). A Xylem taken over fills
#      that and nothing else of the host. A larger size grows it, also while Xylem runs
#      (resize2fs online); a smaller one is refused: a file system is never shrunk here.
#   2. Its stats, /run/betula/xylem/<stack>/stats: a tmpfs of 1 MiB, Xylem's (0755). Xylem
#      writes xylem.prom there every 15 s; Alloy reads it through /rootfs (it shares no network
#      with Xylem).
# Both are systemd mount units, enabled: mounted at every boot, before Docker starts. The bare
# directory beneath the data is root's with mode 0000: should its file system not be mounted when
# the container starts, Xylem (no capability lets it pass that) cannot write there and fails,
# instead of writing to the host's own disk.
#
# Not here: its networks (<stack>_in, vps/30-docker.sh), its guard (betula-xylem-guard,
# 30-docker.sh), its stack (49-xylem.sh). Removing an instance is done by hand, on purpose: its
# stack (docker stack rm <stack>), then both units (systemctl disable --now), then the directory,
# which holds its data.
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
# shellcheck source=lib-stacks.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-stacks.sh"
# shellcheck source=lib-xylem.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-xylem.sh"
betula_init --tmp

UNIT_DIR="/etc/systemd/system"

usage() {
  die "usage: sudo bash $(basename "$0") [<instance>...]   (instances: $(xylem_names | tr '\n' ' '))"
}

# unit_of PATH -> the name of the mount unit of PATH (systemd derives it from the path).
unit_of() {
  systemd-escape --path --suffix=mount "$1"
}

# install_unit NAME - installs the unit in ${BETULA_TMPDIR}/NAME; RELOAD_NEEDED=1 when it changed.
RELOAD_NEEDED=0
install_unit() {
  install_file "${BETULA_TMPDIR}/$1" "${UNIT_DIR}/$1" 0644
  if [[ "${INSTALL_CHANGED}" -eq 1 ]]; then
    RELOAD_NEEDED=1
  fi
  return 0
}

# bare_dir PATH - the directory a file system is mounted on: root's, mode 0000, empty. Left alone
# while something is mounted there (the directory is then hidden beneath it).
bare_dir() {
  local path=$1 entries
  [[ -z "$(xylem_mount_facts "${path}")" ]] || return 0
  [[ ! -e "${path}" || -d "${path}" ]] || die "${path} exists and is no directory"
  if [[ -d "${path}" ]]; then
    entries="$(find "${path}" -mindepth 1 -maxdepth 1 -printf '%f ' 2>/dev/null)"
    [[ -z "${entries}" ]] ||
      die "nothing is mounted at ${path}, and it is not empty (${entries% }): files written while its file system was not there? Look at them, move them away, and run this again"
  fi
  mkdir -p -- "${path}"
  chown root:root "${path}"
  chmod 0000 "${path}"
}

# mount_now UNIT PATH - the unit enabled (every boot) and started (mounted now).
mount_now() {
  systemctl enable --quiet "$1" 2>/dev/null || systemctl enable "$1"
  if [[ -z "$(xylem_mount_facts "$2")" ]]; then
    systemctl start "$1" || die "cannot mount $2: journalctl -u $1 -n 20"
    log "mounted: $2 ($1)"
  fi
}

# data_fs STACK - step 1 of the head of this file.
data_fs() {
  local stack=$1 dir img data unit have source grow=0 fs_bytes allocated avail facts
  dir="$(xylem_dir "${stack}")"
  img="$(xylem_image_file "${stack}")"
  data="$(xylem_data_dir "${stack}")"
  unit="$(unit_of "${data}")"
  install -d -m 0755 -o root -g root -- "${dir}"
  chown root:root "${dir}"
  chmod 0755 "${dir}"

  if [[ ! -e "${img}" ]]; then
    log "${img}: a new file system of ${XYLEM_SIZE} (ext4, a sparse file)"
    rm -f -- "${img}.new"
    truncate -s "${XYLEM_SIZE_BYTES}" -- "${img}.new"
    chmod 0600 "${img}.new"
    # -m 0: no blocks kept for root, which Xylem never is. The label is at most 16 bytes.
    mkfs.ext4 -q -F -m 0 -L "${stack:0:16}" -- "${img}.new"
    mv -f -- "${img}.new" "${img}"
  else
    [[ -f "${img}" ]] || die "${img} exists and is no file"
    have="$(stat -c %s -- "${img}")"
    if [[ "${XYLEM_SIZE_BYTES}" -lt "${have}" ]]; then
      die "XYLEM_DATA_SIZE is ${XYLEM_SIZE} (${XYLEM_SIZE_BYTES} bytes), smaller than ${img} (${have} bytes): a file system is never shrunk here. Name its size or more"
    fi
    if [[ "${XYLEM_SIZE_BYTES}" -gt "${have}" ]]; then
      grow=1
    fi
    chown root:root "${img}"
    chmod 0600 "${img}"
  fi

  facts="$(xylem_mount_facts "${data}")"
  if [[ -n "${facts}" ]]; then
    source="$(findmnt -n -M "${data}" -o SOURCE)"
    [[ "$(losetup -n -O BACK-FILE "${source}" 2>/dev/null | sed 's/[[:space:]]*$//')" == "${img}" ]] ||
      die "${data} is mounted from ${source}, which is not the loop device of ${img}: unmount it, and run this again"
  fi
  bare_dir "${data}"

  cat >"${BETULA_TMPDIR}/${unit}" <<EOF
# Made by ${BETULA_ROOT}/vps/47-xylem-host.sh: the data of Xylem (${stack}), the service of
# Betula's notifications (docs/xylem/xylem.md). Do not edit: the script writes it again.
[Unit]
Description=Xylem (${stack}): its data, a file system of fixed size of its own
Documentation=file://${BETULA_ROOT}/vps/47-xylem-host.sh
# Before Docker: its container never starts on the bare directory beneath (which it could not
# write anyway: root's, mode 0000).
Before=docker.service
RequiresMountsFor=${dir}

[Mount]
What=${img}
Where=${data}
Type=ext4
Options=loop,nosuid,nodev,noexec,noatime

[Install]
WantedBy=local-fs.target
EOF
  install_unit "${unit}"
  if [[ "${RELOAD_NEEDED}" -eq 1 ]]; then
    systemctl daemon-reload
    RELOAD_NEEDED=0
  fi
  mount_now "${unit}" "${data}"

  # Growing: the file, then the loop device, then the mounted file system (resize2fs online),
  # each when it is behind the one before, so that a run that stopped half way goes on there.
  source="$(findmnt -n -M "${data}" -o SOURCE)"
  if [[ "${grow}" -eq 1 ]]; then
    log "${img}: growing it to ${XYLEM_SIZE}"
    truncate -s "${XYLEM_SIZE_BYTES}" -- "${img}"
  fi
  if [[ "$(blockdev --getsize64 "${source}")" -lt "$(stat -c %s -- "${img}")" ]]; then
    losetup -c "${source}"
  fi
  fs_bytes="$(tune2fs -l "${source}" | awk -F: '$1 == "Block count" { c = $2 } $1 == "Block size" { s = $2 } END { printf "%.0f", c * s }')"
  if [[ "${fs_bytes}" -lt $(($(blockdev --getsize64 "${source}") - (1 << 20))) ]]; then
    log "${data}: the file system has $((fs_bytes >> 20)) MiB, it grows into the $(($(blockdev --getsize64 "${source}") >> 20)) MiB of ${img}"
    resize2fs "${source}"
  fi

  # The root of the mounted file system, not the bare directory beneath: Xylem's alone.
  [[ -n "$(xylem_mount_facts "${data}")" ]] || die "${data} is not mounted after all"
  chown "${XYLEM_UID}:${XYLEM_UID}" "${data}"
  chmod 0700 "${data}"

  # Sparse: it takes room on the host as Xylem writes. Should the host's file system have less
  # room than what Xylem's may still grow by, Xylem would run into write errors before its own
  # budget stops it (507), and the host's disk would be full as well.
  allocated=$(($(stat -c '%b * %B' -- "${img}")))
  avail="$(df --output=avail -B1 -- "${dir}" | tail -n 1 | tr -d ' ')"
  if [[ "${avail}" =~ ^[0-9]+$ && "${avail}" -lt $((XYLEM_SIZE_BYTES - allocated)) ]]; then
    warn "the host's file system under ${dir} has $((avail >> 30)) GiB free, less than the $(((XYLEM_SIZE_BYTES - allocated) >> 30)) GiB ${img} may still take: make room, or make XYLEM_DATA_SIZE smaller (for a new instance; one that exists never shrinks)"
  fi
}

# stats_fs STACK - step 2 of the head of this file.
stats_fs() {
  local stack=$1 stats unit options facts
  stats="$(xylem_stats_dir "${stack}")"
  unit="$(unit_of "${stats}")"
  options="size=${XYLEM_STATS_SIZE},mode=0755,uid=${XYLEM_UID},gid=${XYLEM_UID},nosuid,nodev,noexec"
  facts="$(xylem_mount_facts "${stats}")"
  if [[ -n "${facts}" && "${facts%% *}" != "tmpfs" ]]; then
    die "${stats} has a file system mounted that is no tmpfs (${facts}): unmount it, and run this again"
  fi
  install -d -m 0755 -o root -g root -- "$(dirname "${stats}")"
  bare_dir "${stats}"

  cat >"${BETULA_TMPDIR}/${unit}" <<EOF
# Made by ${BETULA_ROOT}/vps/47-xylem-host.sh: where Xylem (${stack}), the service of Betula's
# notifications, writes its numbers (docs/xylem/xylem.md). Do not edit: the script writes it again.
[Unit]
Description=Xylem (${stack}): the tmpfs of its stats file
Documentation=file://${BETULA_ROOT}/vps/47-xylem-host.sh
Before=docker.service

[Mount]
What=tmpfs
Where=${stats}
Type=tmpfs
Options=${options}

[Install]
WantedBy=local-fs.target
EOF
  install_unit "${unit}"
  if [[ "${RELOAD_NEEDED}" -eq 1 ]]; then
    systemctl daemon-reload
    RELOAD_NEEDED=0
    # A changed unit of a mounted tmpfs: remounted with its new options, in place. A running
    # container keeps seeing it (umount and mount would leave it the old one).
    if [[ -n "$(xylem_mount_facts "${stats}")" ]]; then
      systemctl reload "${unit}"
      log "remounted: ${stats} (${options})"
    fi
  fi
  mount_now "${unit}" "${stats}"
}

# ---------------------------------------------------------------- main

for arg in "$@"; do
  [[ "${arg}" != -* ]] || usage
done

require_root
require_ubuntu
require_cmd systemctl systemd-escape findmnt losetup blockdev mkfs.ext4 tune2fs resize2fs truncate stat df find
[[ "${BETULA_VPS_DIR}" == "${BETULA_ROOT}/vps" ]] ||
  die "this copy lives in ${BETULA_VPS_DIR}; the instances are read from ${XYLEM_ENV_DIR}, so run ${BETULA_ROOT}/vps/$(basename "$0")"
[[ -d "${XYLEM_ENV_DIR}" ]] || die "${XYLEM_ENV_DIR} does not exist (run deploy/sync.sh)"

names=("$@")
if [[ "${#names[@]}" -eq 0 ]]; then
  mapfile -t names < <(xylem_names)
  [[ "${#names[@]}" -gt 0 ]] || die "${XYLEM_ENV_DIR} names no instance (stacks/xylem/<stack>.env)"
fi
for name in "${names[@]}"; do
  load_xylem "${name}"
  step "${name}: its data, ${XYLEM_SIZE} at $(xylem_data_dir "${name}")"
  data_fs "${name}"
  step "${name}: its stats, a tmpfs of 1 MiB at $(xylem_stats_dir "${name}")"
  stats_fs "${name}"
  xylem_check_mounts "${name}" || die "${name}: ${XYLEM_MOUNT_DETAIL}"
done

step "Done"
for name in "${names[@]}"; do
  df -h --output=target,size,used,avail "$(xylem_data_dir "${name}")" "$(xylem_stats_dir "${name}")" | sed 's/^/  /'
done
log "next, as ${DEPLOY_USER} without sudo: bash ${BETULA_ROOT}/vps/49-xylem.sh <instance> <tag> (deploy/ship-xylem.sh <instance> builds and loads the image, and calls it)"
