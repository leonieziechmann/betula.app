#!/usr/bin/env bash
# sync-receive.sh - the server side of deploy/sync.sh. Not meant to be called by hand:
#
#   sync-receive.sh <archive.tar.gz> [prune: 1|0]
#
# sync.sh uploads the archive into a private temp directory, takes THIS file out of the archive
# and runs it as root. So the receiving logic always matches the sender, the very first upload
# needs nothing on the server but bash and tar, and the archive never travels through sudo's stdin.
#
# What it does: mirror the archive into /opt/betula with the contract's ownership and modes
# (root:root, directories 0755, files 0644, *.sh 0755).
#   - In place: a directory that exists is never replaced, because running containers bind-mount
#     directories below config/ and a new directory would leave them looking at the old one.
#   - A changed file is replaced by renaming the new one into place: readers (Traefik's file
#     watcher, a script that is running right now) see the old or the new file, never half of one.
#     The price: a SINGLE-FILE bind mount keeps the old inode until its task restarts, which
#     40-stacks.sh takes care of (config-rev labels).
#   - An unchanged file is left alone (same inode, same mtime): nothing reloads for nothing.
#   - prune=1: what is not in the archive is removed, and every removal is logged. A stale file is
#     not harmless here: Traefik and Grafana load every file they find in their directories.
set -Eeuo pipefail
export LC_ALL=C.UTF-8 LANG=C.UTF-8
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
umask 022

DEST="/opt/betula"
ARCHIVE="${1:-}"
PRUNE="${2:-1}"
WORK=""

say() { printf 'sync-receive: %s\n' "$*"; }
die() {
  printf 'sync-receive: FATAL: %s\n' "$*" >&2
  exit 1
}
cleanup() {
  if [[ -n "${WORK}" && -d "${WORK}" ]]; then
    rm -rf -- "${WORK}"
  fi
}
trap cleanup EXIT
trap 'die "line ${LINENO}: \"${BASH_COMMAND}\" failed"' ERR

[[ "${EUID}" -eq 0 ]] || die "must run as root (sync.sh calls it through sudo)"
[[ -n "${ARCHIVE}" && -f "${ARCHIVE}" ]] || die "usage: $(basename "$0") <archive.tar.gz> [prune: 1|0]"
[[ "${PRUNE}" == "0" || "${PRUNE}" == "1" ]] || die "prune must be 0 or 1, not '${PRUNE}'"

# The contract's /etc/os-release gate, here as well: this is the first thing that ever runs on a new server.
os_id="$(sed -n 's/^ID=//p' /etc/os-release 2>/dev/null | tr -d '"' || true)"
os_version="$(sed -n 's/^VERSION_ID=//p' /etc/os-release 2>/dev/null | tr -d '"' || true)"
[[ "${os_id}" == "ubuntu" ]] || die "this is '${os_id:-unknown}', not Ubuntu; refusing to install anything here"
dpkg --compare-versions "${os_version:-0}" ge 24.04 || die "Ubuntu ${os_version} is older than 24.04"

# Staged next to the destination: the same file system, so the final step is a rename per file.
install -d -m 0755 -o root -g root "${DEST}"
WORK="$(mktemp -d "$(dirname "${DEST}")/.betula-sync.XXXXXXXX")"
STAGE="${WORK}/tree"
FILE_LIST="${WORK}/files"
DIR_LIST="${WORK}/dirs"
mkdir -- "${STAGE}"
# Never the archive's owners or modes: it was packed on Windows, where both are meaningless.
tar -xzf "${ARCHIVE}" -C "${STAGE}" --no-same-owner --no-same-permissions

# An archive that is not deploy/ (wrong directory, truncated upload) must not get to prune anything.
for must in vps/lib.sh vps/10-base.sh stacks/edge.yml config/traefik/dynamic; do
  [[ -e "${STAGE}/${must}" ]] || die "the archive does not contain ${must}; this is not deploy/ - nothing was changed"
done
odd="$(find "${STAGE}" ! -type f ! -type d -print | sed -n '1,5p')"
[[ -z "${odd}" ]] || die "the archive contains something that is neither file nor directory: ${odd//$'\n'/, }"
# The lists below are one path per line. (Patterns with control characters live in variables:
# some bash builds lose a $'...' that is written inside "$( ... )".)
NL=$'\n'
CR=$'\r'
odd="$(find "${STAGE}" -name "*${NL}*" -print | sed -n '1,5p')"
[[ -z "${odd}" ]] || die "a file name in the archive contains a line break"
# sync.sh checks this before it uploads; checked again because everything below trusts it.
crlf="$(grep -rl -- "${CR}" "${STAGE}" | sed -n '1,5p' || true)"
[[ -z "${crlf}" ]] || die "files with CR characters (CRLF line endings): ${crlf//$'\n'/, }"

chown -R root:root "${STAGE}"
# Paths relative to the root, without "./" (plain find + sed: no GNU-only -printf).
(cd "${STAGE}" && find . -mindepth 1 -type d | sed 's|^\./||' | sort) >"${DIR_LIST}"
(cd "${STAGE}" && find . -type f | sed 's|^\./||' | sort) >"${FILE_LIST}"
[[ -s "${FILE_LIST}" ]] || die "the archive contains no files"

created=0
updated=0
unchanged=0
removed=0

# Directories first; sorted, so a parent comes before its children.
while IFS= read -r rel; do
  target="${DEST}/${rel}"
  if [[ -L "${target}" || (-e "${target}" && ! -d "${target}") ]]; then
    rm -f -- "${target}"
    say "removed: ${rel} (was a file, is a directory in deploy/)"
    removed=$((removed + 1))
  fi
  [[ -d "${target}" ]] || mkdir -- "${target}"
  chown root:root "${target}"
  chmod 0755 "${target}"
done <"${DIR_LIST}"

while IFS= read -r rel; do
  source_file="${STAGE}/${rel}"
  target="${DEST}/${rel}"
  mode=0644
  if [[ "${rel}" == *.sh ]]; then mode=0755; fi
  chmod "${mode}" "${source_file}"
  if [[ -d "${target}" && ! -L "${target}" ]]; then
    die "${target} is a directory on the server but a file in deploy/; look at it and remove it by hand"
  fi
  if [[ -f "${target}" && ! -L "${target}" ]] && cmp -s -- "${source_file}" "${target}"; then
    chown root:root "${target}"
    chmod "${mode}" "${target}"
    unchanged=$((unchanged + 1))
    continue
  fi
  if [[ -e "${target}" || -L "${target}" ]]; then
    say "updated: ${rel}"
    updated=$((updated + 1))
  else
    say "new:     ${rel}"
    created=$((created + 1))
  fi
  mv -f -- "${source_file}" "${target}"
done <"${FILE_LIST}"

if [[ "${PRUNE}" == "1" ]]; then
  # Files and links first, then the directories they leave empty (deepest first).
  while IFS= read -r rel; do
    if ! grep -qxF -- "${rel}" "${FILE_LIST}"; then
      rm -f -- "${DEST:?}/${rel}"
      say "removed: ${rel} (not in deploy/)"
      removed=$((removed + 1))
    fi
  done < <(cd "${DEST}" && find . -mindepth 1 ! -type d | sed 's|^\./||')
  while IFS= read -r rel; do
    if ! grep -qxF -- "${rel}" "${DIR_LIST}"; then
      if rmdir -- "${DEST:?}/${rel}" 2>/dev/null; then
        say "removed: ${rel}/ (not in deploy/)"
        removed=$((removed + 1))
      else
        say "kept:    ${rel}/ (not in deploy/, but not empty)"
      fi
    fi
  done < <(cd "${DEST}" && find . -mindepth 1 -depth -type d | sed 's|^\./||')
fi

say "${DEST}: ${created} new, ${updated} updated, ${unchanged} unchanged, ${removed} removed"
if [[ $((created + updated + removed)) -gt 0 ]]; then
  say "changes below config/ and stacks/ reach the running services with: bash ${DEST}/vps/40-stacks.sh (as deploy, no sudo)"
fi
