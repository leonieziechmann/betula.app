#!/usr/bin/env bash
# 47-phloem-host.sh - the host's part of Phloem, the service of Betula Sync (docs/phloem/phloem.md,
# stacks/phloem.yml): for each instance, the only two places its container may write to. Run as
# root, before the first deploy of an instance (49-phloem.sh refuses to deploy without them) and
# after a change to its PHLOEM_DATA_SIZE (deploy/sync.sh brings stacks/phloem/<stack>.env):
#
#   sudo bash /opt/betula/vps/47-phloem-host.sh                  # every instance in stacks/phloem/
#   sudo bash /opt/betula/vps/47-phloem-host.sh phloem-canary    # this one
#
# For each instance <stack>; running it again changes nothing that is as it should be:
#   1. Its data, /var/lib/betula/phloem/<stack>/data: an ext4 of its own, of fixed size
#      (PHLOEM_DATA_SIZE), in the sparse file data.img next to it, on a loop device; mounted
#      nosuid,nodev,noexec, its root Phloem's (uid 65532, mode 0700). A Phloem taken over fills
#      that and nothing else of the host. A larger size grows it, also while Phloem runs
#      (resize2fs online); a smaller one is refused: a file system is never shrunk here.
#   2. Its stats, /run/betula/phloem/<stack>/stats: a tmpfs of 1 MiB, Phloem's (0755). Phloem
#      writes phloem.prom there every 15 s; Alloy reads it through /rootfs (it shares no network
#      with Phloem).
# Both are systemd mount units, enabled: mounted at every boot, before Docker starts. The bare
# directory beneath the data is root's with mode 0000: should its file system not be mounted when
# the container starts, Phloem (no capability lets it pass that) cannot write there and fails,
# instead of writing to the host's own disk.
#
# Not here: its networks (<stack>_in, vps/30-docker.sh), its guard (betula-phloem-guard,
# 30-docker.sh), its stack (49-phloem.sh). Removing an instance is done by hand, on purpose: its
# stack (docker stack rm <stack>), then both units (systemctl disable --now), then the directory,
# which holds its data.
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
# shellcheck source=lib-stacks.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-stacks.sh"
# shellcheck source=lib-phloem.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-phloem.sh"
betula_init --tmp

UNIT_DIR="/etc/systemd/system"

usage() {
  die "usage: sudo bash $(basename "$0") [<instance>...]   (instances: $(phloem_names | tr '\n' ' '))"
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
  [[ -z "$(phloem_mount_facts "${path}")" ]] || return 0
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
  if [[ -z "$(phloem_mount_facts "$2")" ]]; then
    systemctl start "$1" || die "cannot mount $2: journalctl -u $1 -n 20"
    log "mounted: $2 ($1)"
  fi
}

# data_fs STACK - step 1 of the head of this file.
data_fs() {
  local stack=$1 dir img data unit have source grow=0 fs_bytes allocated avail facts
  dir="$(phloem_dir "${stack}")"
  img="$(phloem_image_file "${stack}")"
  data="$(phloem_data_dir "${stack}")"
  unit="$(unit_of "${data}")"
  install -d -m 0755 -o root -g root -- "${dir}"
  chown root:root "${dir}"
  chmod 0755 "${dir}"

  if [[ ! -e "${img}" ]]; then
    log "${img}: a new file system of ${PHLOEM_SIZE} (ext4, a sparse file)"
    rm -f -- "${img}.new"
    truncate -s "${PHLOEM_SIZE_BYTES}" -- "${img}.new"
    chmod 0600 "${img}.new"
    # -m 0: no blocks kept for root, which Phloem never is. The label is at most 16 bytes.
    mkfs.ext4 -q -F -m 0 -L "${stack:0:16}" -- "${img}.new"
    mv -f -- "${img}.new" "${img}"
  else
    [[ -f "${img}" ]] || die "${img} exists and is no file"
    have="$(stat -c %s -- "${img}")"
    if [[ "${PHLOEM_SIZE_BYTES}" -lt "${have}" ]]; then
      die "PHLOEM_DATA_SIZE is ${PHLOEM_SIZE} (${PHLOEM_SIZE_BYTES} bytes), smaller than ${img} (${have} bytes): a file system is never shrunk here. Name its size or more"
    fi
    if [[ "${PHLOEM_SIZE_BYTES}" -gt "${have}" ]]; then
      grow=1
    fi
    chown root:root "${img}"
    chmod 0600 "${img}"
  fi

  facts="$(phloem_mount_facts "${data}")"
  if [[ -n "${facts}" ]]; then
    source="$(findmnt -n -M "${data}" -o SOURCE)"
    [[ "$(losetup -n -O BACK-FILE "${source}" 2>/dev/null | sed 's/[[:space:]]*$//')" == "${img}" ]] ||
      die "${data} is mounted from ${source}, which is not the loop device of ${img}: unmount it, and run this again"
  fi
  bare_dir "${data}"

  cat >"${BETULA_TMPDIR}/${unit}" <<EOF
# Made by ${BETULA_ROOT}/vps/47-phloem-host.sh: the data of Phloem (${stack}), Betula Sync's
# service (docs/phloem/phloem.md). Do not edit: the script writes it again.
[Unit]
Description=Phloem (${stack}): its data, a file system of fixed size of its own
Documentation=file://${BETULA_ROOT}/vps/47-phloem-host.sh
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
    log "${img}: growing it to ${PHLOEM_SIZE}"
    truncate -s "${PHLOEM_SIZE_BYTES}" -- "${img}"
  fi
  if [[ "$(blockdev --getsize64 "${source}")" -lt "$(stat -c %s -- "${img}")" ]]; then
    losetup -c "${source}"
  fi
  fs_bytes="$(tune2fs -l "${source}" | awk -F: '$1 == "Block count" { c = $2 } $1 == "Block size" { s = $2 } END { printf "%.0f", c * s }')"
  if [[ "${fs_bytes}" -lt $(($(blockdev --getsize64 "${source}") - (1 << 20))) ]]; then
    log "${data}: the file system has $((fs_bytes >> 20)) MiB, it grows into the $(($(blockdev --getsize64 "${source}") >> 20)) MiB of ${img}"
    resize2fs "${source}"
  fi

  # The root of the mounted file system, not the bare directory beneath: Phloem's alone.
  [[ -n "$(phloem_mount_facts "${data}")" ]] || die "${data} is not mounted after all"
  chown "${PHLOEM_UID}:${PHLOEM_UID}" "${data}"
  chmod 0700 "${data}"

  # Sparse: it takes room on the host as Phloem writes. Should the host's file system have less
  # room than what Phloem's may still grow by, Phloem would run into write errors before its own
  # budget stops it (507), and the host's disk would be full as well.
  allocated=$(($(stat -c '%b * %B' -- "${img}")))
  avail="$(df --output=avail -B1 -- "${dir}" | tail -n 1 | tr -d ' ')"
  if [[ "${avail}" =~ ^[0-9]+$ && "${avail}" -lt $((PHLOEM_SIZE_BYTES - allocated)) ]]; then
    warn "the host's file system under ${dir} has $((avail >> 30)) GiB free, less than the $(((PHLOEM_SIZE_BYTES - allocated) >> 30)) GiB ${img} may still take: make room, or make PHLOEM_DATA_SIZE smaller (for a new instance; one that exists never shrinks)"
  fi
}

# stats_fs STACK - step 2 of the head of this file.
stats_fs() {
  local stack=$1 stats unit options facts
  stats="$(phloem_stats_dir "${stack}")"
  unit="$(unit_of "${stats}")"
  options="size=${PHLOEM_STATS_SIZE},mode=0755,uid=${PHLOEM_UID},gid=${PHLOEM_UID},nosuid,nodev,noexec"
  facts="$(phloem_mount_facts "${stats}")"
  if [[ -n "${facts}" && "${facts%% *}" != "tmpfs" ]]; then
    die "${stats} has a file system mounted that is no tmpfs (${facts}): unmount it, and run this again"
  fi
  install -d -m 0755 -o root -g root -- "$(dirname "${stats}")"
  bare_dir "${stats}"

  cat >"${BETULA_TMPDIR}/${unit}" <<EOF
# Made by ${BETULA_ROOT}/vps/47-phloem-host.sh: where Phloem (${stack}), Betula Sync's service,
# writes its numbers (docs/phloem/phloem.md). Do not edit: the script writes it again.
[Unit]
Description=Phloem (${stack}): the tmpfs of its stats file
Documentation=file://${BETULA_ROOT}/vps/47-phloem-host.sh
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
    if [[ -n "$(phloem_mount_facts "${stats}")" ]]; then
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
  die "this copy lives in ${BETULA_VPS_DIR}; the instances are read from ${PHLOEM_ENV_DIR}, so run ${BETULA_ROOT}/vps/$(basename "$0")"
[[ -d "${PHLOEM_ENV_DIR}" ]] || die "${PHLOEM_ENV_DIR} does not exist (run deploy/sync.sh)"

names=("$@")
if [[ "${#names[@]}" -eq 0 ]]; then
  mapfile -t names < <(phloem_names)
  [[ "${#names[@]}" -gt 0 ]] || die "${PHLOEM_ENV_DIR} names no instance (stacks/phloem/<stack>.env)"
fi
for name in "${names[@]}"; do
  load_phloem "${name}"
  step "${name}: its data, ${PHLOEM_SIZE} at $(phloem_data_dir "${name}")"
  data_fs "${name}"
  step "${name}: its stats, a tmpfs of 1 MiB at $(phloem_stats_dir "${name}")"
  stats_fs "${name}"
  phloem_check_mounts "${name}" || die "${name}: ${PHLOEM_MOUNT_DETAIL}"
done

step "Done"
for name in "${names[@]}"; do
  df -h --output=target,size,used,avail "$(phloem_data_dir "${name}")" "$(phloem_stats_dir "${name}")" | sed 's/^/  /'
done
log "next, as ${DEPLOY_USER} without sudo: bash ${BETULA_ROOT}/vps/49-phloem.sh <instance> <tag> (deploy/ship-phloem.sh <instance> builds and loads the image, and calls it)"
