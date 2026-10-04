#!/usr/bin/env bash
# Shared helpers for the host scripts in this directory. Sourced, never executed.
# The caller sets "set -Eeuo pipefail"; everything here has to stay safe under it.
# One rule throughout: never pipe into "grep -q" (pipefail + SIGPIPE gives false negatives);
# capture the output first and match the variable.
# shellcheck shell=bash

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  echo "lib.sh is a library; source it from one of the numbered scripts" >&2
  exit 2
fi

BETULA_VPS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BETULA_FILES_DIR="${BETULA_VPS_DIR}/files"
BETULA_SCRIPT="$(basename "${0}")"
BETULA_RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)"
# Backups live outside /etc: *.d directories (sudoers.d, apt.conf.d, jail.d) would parse stray copies.
BETULA_BACKUP_ROOT="/var/backups/betula"
BETULA_ETC_DIR="/etc/betula"
# Volatile state (tmpfs): markers that a reboot makes meaningless, e.g. "dockerd restart pending".
BETULA_RUN_DIR="/run/betula"
BETULA_MIN_UBUNTU="24.04"
BETULA_TMPDIR=""
BETULA_STEP=0
BETULA_APT_UPDATED=0
BETULA_EXIT_HOOKS=()
INSTALL_CHANGED=0

# Contract values shared by all host scripts.
DEPLOY_USER="deploy"
SSH_PORT=22
# Owner decision: the host runs on German local time, so the 04:30 reboot of unattended-upgrades
# and the systemd timers mean "night for the audience" all year. Log lines of these scripts stay UTC.
HOST_TIMEZONE="Europe/Berlin"

# Units a single-purpose VPS does not need. 10-base.sh disables them only when present (disabled,
# not masked or purged: "systemctl enable --now" brings any of them back); 90-verify-host.sh
# reports the ones that still run. Sockets before their services, so nothing re-activates them.
BETULA_UNNEEDED_UNITS=(
  ModemManager.service                    # no modem
  udisks2.service                         # desktop disk automounter
  multipathd.socket multipathd.service    # SAN multipath; an idle daemon with locked memory
  iscsid.socket iscsid.service open-iscsi.service # no iSCSI storage
  snapd.socket snapd.service snapd.seeded.service snapd.snap-repair.timer # no snaps are used: a
                                          # resident daemon with its own refresh schedule, outside
                                          # the unattended-upgrades policy (skipped if snaps exist)
  apport.service apport-autoreport.path whoopsie.service # crash upload for desktops
  motd-news.timer                         # fetches advertising from motd.ubuntu.com
  rpcbind.socket rpcbind.service          # listens on 111; nothing here speaks NFS/RPC
  avahi-daemon.socket avahi-daemon.service # mDNS announcements into the provider's network
)

# ---------------------------------------------------------------- logging

_ts() { date -u +%Y-%m-%dT%H:%M:%SZ; }
log() { printf '%s [%s] %s\n' "$(_ts)" "${BETULA_SCRIPT}" "$*"; }
warn() { printf '%s [%s] WARNING: %s\n' "$(_ts)" "${BETULA_SCRIPT}" "$*" >&2; }
die() {
  printf '%s [%s] FATAL: %s\n' "$(_ts)" "${BETULA_SCRIPT}" "$*" >&2
  exit 1
}
step() {
  BETULA_STEP=$((BETULA_STEP + 1))
  printf '\n%s [%s] ==> step %d: %s\n' "$(_ts)" "${BETULA_SCRIPT}" "${BETULA_STEP}" "$*"
}

# ---------------------------------------------------------------- lifecycle

_betula_on_error() {
  local rc=$1 line=$2 cmd=$3
  printf '%s [%s] FATAL: line %s: "%s" exited with %s\n' "$(_ts)" "${BETULA_SCRIPT}" "${line}" "${cmd}" "${rc}" >&2
}

_betula_on_exit() {
  local rc=$?
  trap - ERR EXIT
  set +e
  local hook
  for hook in "${BETULA_EXIT_HOOKS[@]:-}"; do
    [[ -n "${hook}" ]] && "${hook}" "${rc}"
  done
  [[ -n "${BETULA_TMPDIR}" && -d "${BETULA_TMPDIR}" ]] && rm -rf -- "${BETULA_TMPDIR}"
  exit "${rc}"
}

# Hooks run on every exit (also after a failure) and get the exit code as $1.
add_exit_hook() { BETULA_EXIT_HOOKS+=("$1"); }

# betula_init [--tmp]
# --tmp creates the private temp directory right away. It cannot be lazy: betula_tmpfile is
# called inside $(...) subshells, which could not hand a new directory back to us for cleanup.
# The read-only audit script omits --tmp and therefore never writes anything.
betula_init() {
  # Tool output is parsed (ufw, sshd -T, ss); a localized server must not change it.
  export LC_ALL=C.UTF-8 LANG=C.UTF-8
  export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
  umask 022
  trap '_betula_on_error "$?" "${LINENO}" "${BASH_COMMAND}"' ERR
  trap _betula_on_exit EXIT
  if [[ "${1:-}" == "--tmp" ]]; then
    BETULA_TMPDIR="$(mktemp -d /tmp/betula.XXXXXXXX)"
    chmod 0700 "${BETULA_TMPDIR}"
  fi
}

betula_tmpfile() {
  [[ -n "${BETULA_TMPDIR}" ]] || die "betula_tmpfile needs 'betula_init --tmp'"
  mktemp "${BETULA_TMPDIR}/f.XXXXXXXX"
}

# ---------------------------------------------------------------- preconditions

require_root() {
  [[ "${EUID}" -eq 0 ]] || die "must run as root (try: sudo ${0})"
}

require_cmd() {
  local c
  for c in "$@"; do
    command -v "${c}" >/dev/null 2>&1 || die "required command not found: ${c}"
  done
}

have_cmd() { command -v "$1" >/dev/null 2>&1; }

# Reads one key from /etc/os-release without sourcing it (sourcing leaks NAME, VERSION, ... into us).
os_release_get() {
  local key=$1 line value
  while IFS= read -r line; do
    [[ "${line}" == "${key}="* ]] || continue
    value="${line#*=}"
    value="${value%\"}"
    value="${value#\"}"
    printf '%s' "${value}"
    return 0
  done </etc/os-release
  return 1
}

# Sets OS_VERSION_ID and OS_CODENAME; refuses anything that is not Ubuntu >= 24.04.
require_ubuntu() {
  [[ -r /etc/os-release ]] || die "/etc/os-release is missing; refusing to guess the distribution"
  local id
  id="$(os_release_get ID)" || die "/etc/os-release has no ID"
  [[ "${id}" == "ubuntu" ]] || die "this is '${id}', not Ubuntu; these scripts only support Ubuntu >= ${BETULA_MIN_UBUNTU}"
  OS_VERSION_ID="$(os_release_get VERSION_ID)" || die "/etc/os-release has no VERSION_ID"
  dpkg --compare-versions "${OS_VERSION_ID}" ge "${BETULA_MIN_UBUNTU}" ||
    die "Ubuntu ${OS_VERSION_ID} is older than ${BETULA_MIN_UBUNTU}"
  OS_CODENAME="$(os_release_get UBUNTU_CODENAME || os_release_get VERSION_CODENAME)" ||
    die "/etc/os-release has no codename"
  log "operating system: Ubuntu ${OS_VERSION_ID} (${OS_CODENAME})"
}

# LXC/OpenVZ guests cannot set sysctls, swap or the clock; KVM guests (the usual VPS) can.
is_container() {
  have_cmd systemd-detect-virt || return 1
  systemd-detect-virt --container --quiet
}

mem_total_mb() {
  local kb
  kb="$(awk '$1 == "MemTotal:" { print $2 }' /proc/meminfo)"
  printf '%d' "$((kb / 1024))"
}

# ---------------------------------------------------------------- text helpers

# contains_line TEXT REGEX - true when a line of TEXT matches the extended REGEX.
contains_line() { grep -Eq -- "$2" <<<"$1"; }

# sudoers_rules FILE - the lines of FILE that are rules ("#include..." is one, other "#" lines are not).
sudoers_rules() {
  awk '/^[[:space:]]*$/ { next } /^[[:space:]]*#include/ { print; next } /^[[:space:]]*#/ { next } { print }' "$1"
}

# Source address of the default route: the address the internet sees unless the host is behind NAT.
# Nothing without ip (iproute2), as when there is no default route: under pipefail the missing
# command would fail the caller's assignment, and with it a whole verify run (verify2 D6).
default_ipv4() {
  have_cmd ip || return 0
  ip -4 -o route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p'
}

is_private_ipv4() {
  [[ "$1" =~ ^(10\.|127\.|192\.168\.|169\.254\.|172\.(1[6-9]|2[0-9]|3[01])\.|100\.(6[4-9]|[7-9][0-9]|1[01][0-9]|12[0-7])\.) ]]
}

# True when the kernel has IPv6 at all (ip6tables and ufw's v6 half only work then).
ipv6_enabled() { [[ -e /proc/net/if_inet6 ]]; }

# True when the host can be reached over IPv6 from outside (this server can: 2a02:c206:.../64
# on eth0, although DNS has no AAAA record). Then every IPv6 firewall check is mandatory.
has_global_ipv6() {
  local out
  out="$(ip -6 -o addr show scope global 2>/dev/null || true)"
  [[ -n "${out}" ]]
}

# ---------------------------------------------------------------- ufw and sysctl

# ufw applies a sysctl file of its own at the end of EVERY start (boot, "ufw reload", "ufw
# default ..." while enabled): IPT_SYSCTL in /etc/default/ufw, by default /etc/ufw/sysctl.conf.
# Its header says it: "these settings override /etc/sysctl.conf and /etc/sysctl.d/*.conf".
# Prints the path; prints nothing when ufw is told to apply none.
ufw_sysctl_file() {
  local line value=""
  [[ -r /etc/default/ufw ]] || return 0
  while IFS= read -r line; do
    [[ "${line}" =~ ^[[:space:]]*IPT_SYSCTL=(.*)$ ]] || continue
    value="${BASH_REMATCH[1]}"
    value="${value%%[[:space:]]*}"
    value="${value%\"}"
    value="${value#\"}"
  done </etc/default/ufw
  printf '%s' "${value}"
}

# Matches "net/ipv4/conf/<if>/rp_filter=" in ufw's slash notation and in dotted notation.
UFW_RP_FILTER_KEY_RE='^[[:space:]]*net[./]ipv4[./]conf[./][^./=[:space:]]+[./]rp_filter'

# ufw_rp_filter_conflicts FILE - prints the lines of FILE that set rp_filter to anything but 2
# (the packaged file sets all/default to 1 = strict, which breaks multi-network containers and
# silently undoes /etc/sysctl.d/99-zz-betula.conf after the next reboot).
ufw_rp_filter_conflicts() {
  [[ -r "$1" ]] || return 0
  grep -E -- "${UFW_RP_FILTER_KEY_RE}[[:space:]]*=" "$1" | grep -Ev -- '=[[:space:]]*2[[:space:]]*$' || true
}

# ---------------------------------------------------------------- systemd

unit_exists() {
  local out
  out="$(systemctl list-unit-files --no-legend --no-pager "$1" 2>/dev/null || true)"
  [[ -n "${out}" ]]
}

unit_active() { systemctl is-active --quiet "$1" 2>/dev/null; }
unit_enabled() { systemctl is-enabled --quiet "$1" 2>/dev/null; }

# ---------------------------------------------------------------- apt

pkg_installed() {
  local status
  status="$(dpkg-query -W -f='${db:Status-Abbrev}' "$1" 2>/dev/null || true)"
  # The first letter is what is wanted, the second what is: "ii" installed, "hi" installed and on
  # hold, as 30-docker.sh holds the engine. Taken for missing, a held engine was "installed"
  # again on every later run of 30-docker.sh, which apt refused (held packages would change):
  # the script died before the overlay networks (2026-10-04).
  [[ "${status:1:1}" == "i" ]]
}

# Non-interactive in every respect: no debconf questions, no conffile prompts (keep the local
# version, take the maintainer's only where we never touched the file), no needrestart dialog,
# and wait for the lock instead of failing while apt-daily or cloud-init still hold it.
apt_run() {
  # UCF_FORCE_CONFFOLD: some packages keep their configuration under ucf instead of dpkg
  # (sshd_config, 50unattended-upgrades, ufw's before.rules); ucf does not read --force-confold.
  DEBIAN_FRONTEND=noninteractive \
    APT_LISTCHANGES_FRONTEND=none \
    NEEDRESTART_MODE=a \
    UCF_FORCE_CONFFOLD=1 \
    apt-get -y \
    -o DPkg::Lock::Timeout=600 \
    -o Dpkg::Options::=--force-confdef \
    -o Dpkg::Options::=--force-confold \
    "$@"
}

apt_update() {
  if [[ "${1:-}" != "--force" && "${BETULA_APT_UPDATED}" -eq 1 ]]; then
    return 0
  fi
  log "apt-get update"
  apt_run update
  BETULA_APT_UPDATED=1
}

# Installs only what is missing, so a re-run never upgrades (and restarts) something by accident.
apt_install() {
  local pkg
  local -a missing=()
  for pkg in "$@"; do
    pkg_installed "${pkg}" || missing+=("${pkg}")
  done
  if [[ "${#missing[@]}" -eq 0 ]]; then
    log "packages already installed: $*"
    return 0
  fi
  apt_update
  log "installing: ${missing[*]}"
  apt_run install --no-install-recommends "${missing[@]}"
}

# ---------------------------------------------------------------- files

# A payload with CRLF (Windows checkout without .gitattributes) breaks sudoers, sshd and
# systemd units in ways that are hard to see; refuse it before it reaches /etc.
assert_lf() {
  local cr=$'\r'
  if grep -q -- "${cr}" "$1"; then
    die "$1 contains CRLF line endings; re-sync deploy/ with LF endings (see deploy/.gitattributes)"
  fi
}

backup_file() {
  local path=$1 dest
  [[ -e "${path}" ]] || return 0
  dest="${BETULA_BACKUP_ROOT}/${BETULA_RUN_ID}$(dirname "${path}")"
  install -d -m 0700 "${BETULA_BACKUP_ROOT}"
  mkdir -p "${dest}"
  cp -a -- "${path}" "${dest}/"
  log "backup: ${path} -> ${dest}/"
}

# Path of the backup that backup_file made during THIS run (empty output if there is none).
backup_path_of() {
  local candidate
  candidate="${BETULA_BACKUP_ROOT}/${BETULA_RUN_ID}$1"
  [[ -e "${candidate}" ]] && printf '%s' "${candidate}"
  return 0
}

# install_file SRC DST [MODE] [OWNER] [GROUP]
# Idempotent: identical content only gets mode/owner fixed. Anything else is backed up first and
# replaced atomically (rename), so a reader never sees half a file. Sets INSTALL_CHANGED to 0 or 1.
install_file() {
  local src=$1 dst=$2 mode=${3:-0644} owner=${4:-root} group=${5:-root}
  local dir tmp
  INSTALL_CHANGED=0
  [[ -f "${src}" ]] || die "payload missing: ${src}"
  assert_lf "${src}"
  dir="$(dirname "${dst}")"
  [[ -d "${dir}" ]] || install -d -m 0755 "${dir}"
  if [[ -f "${dst}" && ! -L "${dst}" ]] && cmp -s -- "${src}" "${dst}"; then
    chown "${owner}:${group}" "${dst}"
    chmod "${mode}" "${dst}"
    log "unchanged: ${dst}"
    return 0
  fi
  backup_file "${dst}"
  # Dot-prefixed temp name: run-parts style directories ignore it if we die in between.
  tmp="${dir}/.$(basename "${dst}").betula-new"
  install -m "${mode}" -o "${owner}" -g "${group}" "${src}" "${tmp}"
  mv -f -- "${tmp}" "${dst}"
  INSTALL_CHANGED=1
  log "installed: ${dst} (mode ${mode}, ${owner}:${group})"
}

# render_template SRC KEY=VALUE... -> prints the path of a temp file with every @KEY@ replaced.
# Values must not contain '|' or newlines (they are code names and architectures).
render_template() {
  local src=$1 out pair
  shift
  [[ -f "${src}" ]] || die "template missing: ${src}"
  assert_lf "${src}"
  out="$(betula_tmpfile)"
  cp -- "${src}" "${out}"
  for pair in "$@"; do
    sed -i "s|@${pair%%=*}@|${pair#*=}|g" "${out}"
  done
  if grep -Eq '@[A-Z_]+@' "${out}"; then
    die "template ${src} still has unreplaced placeholders"
  fi
  printf '%s' "${out}"
}

remove_file() {
  local path=$1
  [[ -e "${path}" || -L "${path}" ]] || return 0
  backup_file "${path}"
  rm -f -- "${path}"
  log "removed: ${path}"
}

# ---------------------------------------------------------------- public ports

# The list of public ports has one source (files/public-ports.conf): ufw, the Docker forward
# filter and the audit all read it. Sets PUBLIC_TCP_PORTS and PUBLIC_UDP_PORTS.
load_public_ports() {
  local file=$1 p
  [[ -r "${file}" ]] || die "missing ${file}"
  assert_lf "${file}"
  PUBLIC_TCP_PORTS=""
  PUBLIC_UDP_PORTS=""
  # shellcheck disable=SC1090
  . "${file}"
  for p in ${PUBLIC_TCP_PORTS} ${PUBLIC_UDP_PORTS}; do
    [[ "${p}" =~ ^[0-9]{1,5}$ && "${p}" -ge 1 && "${p}" -le 65535 ]] || die "${file}: invalid port '${p}'"
  done
  [[ -n "${PUBLIC_TCP_PORTS}" ]] || die "${file}: PUBLIC_TCP_PORTS is empty"
}
