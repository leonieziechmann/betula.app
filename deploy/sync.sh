#!/usr/bin/env bash
# sync.sh - copy this directory (deploy/) to /opt/betula on the server. Runs on the workstation:
# Git Bash on Windows, Linux or macOS. Needs only bash, tar, gzip, grep and ssh (no rsync: Git
# for Windows does not ship it).
#
#   SSH_TARGET=betula-root ROOT_BOOTSTRAP=1 deploy/sync.sh    # very first upload, as root
#   SSH_TARGET=betula deploy/sync.sh                          # every later one, as deploy (sudo)
#   deploy/sync.sh                                            # = deploy@betula.app with your default keys
#
# Environment:
#   SSH_TARGET        what ssh connects to: a Host alias from ~/.ssh/config (recommended: it carries
#                     the user, the dedicated IdentityFile and "IdentitiesOnly yes") or user@host.
#                     Default: $DEPLOY_USER@$DEPLOY_HOST.
#   DEPLOY_HOST       default betula.app
#   DEPLOY_USER       default deploy (root with ROOT_BOOTSTRAP=1)
#   SSH_OPTS          extra ssh options, split on spaces, e.g.
#                     SSH_OPTS="-i $HOME/.ssh/betula_ed25519 -o IdentitiesOnly=yes"
#                     (after the lockdown sshd allows 3 authentication attempts: an agent that
#                     offers other keys first never gets to the right one without IdentitiesOnly)
#   ROOT_BOOTSTRAP=1  log in as root and install without sudo: for the first run, before
#                     vps/10-base.sh has created the deploy user. Stops working after
#                     vps/20-ssh-lockdown.sh, as intended.
#   SYNC_PRUNE=0      keep files on the server that no longer exist here (default 1: /opt/betula
#                     mirrors deploy/, and every removed file is named in the output)
#   SYNC_CHECK_ONLY=1 run the local checks and stop before anything is uploaded
#
# One ssh connection, no scp, no ssh-keyscan: the host key has to be in known_hosts already
# (the first manual "ssh betula-root" asks for it). The archive goes to a private temp directory
# of the login user; vps/sync-receive.sh is taken out of it and run as root (through "sudo -n",
# or directly with ROOT_BOOTSTRAP=1). That script sets root:root, 0755 / 0644 / *.sh 0755 and
# replaces only files whose content changed. Nothing here restarts a service: that is
# vps/40-stacks.sh (stacks, config) or a re-run of the numbered host scripts (vps/files).
set -Eeuo pipefail

SCRIPT="$(basename "$0")"
DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RECEIVER="vps/sync-receive.sh"

log() { printf '[%s] %s\n' "${SCRIPT}" "$*"; }
die() {
  printf '[%s] FATAL: %s\n' "${SCRIPT}" "$*" >&2
  exit 1
}
trap 'die "line ${LINENO}: \"${BASH_COMMAND}\" failed"' ERR

ROOT_BOOTSTRAP="${ROOT_BOOTSTRAP:-0}"
SYNC_PRUNE="${SYNC_PRUNE:-1}"
DEPLOY_HOST="${DEPLOY_HOST:-betula.app}"
if [[ "${ROOT_BOOTSTRAP}" == "1" ]]; then
  DEPLOY_USER="${DEPLOY_USER:-root}"
else
  DEPLOY_USER="${DEPLOY_USER:-deploy}"
fi
SSH_TARGET="${SSH_TARGET:-${DEPLOY_USER}@${DEPLOY_HOST}}"
[[ "${SYNC_PRUNE}" == "0" || "${SYNC_PRUNE}" == "1" ]] || die "SYNC_PRUNE must be 0 or 1"
[[ "${SSH_TARGET}" != -* ]] || die "SSH_TARGET must not start with '-'"

# Word splitting is the documented interface of SSH_OPTS; paths with spaces belong into ~/.ssh/config.
ssh_opts=()
if [[ -n "${SSH_OPTS:-}" ]]; then
  read -r -a ssh_opts <<<"${SSH_OPTS}"
fi

for tool in tar gzip grep ssh find; do
  command -v "${tool}" >/dev/null 2>&1 || die "required command not found: ${tool}"
done
[[ -f "${DEPLOY_DIR}/${RECEIVER}" ]] || die "${RECEIVER} is missing below ${DEPLOY_DIR}"

# ---------------------------------------------------------------- local checks

# Editor and OS droppings never travel. Everything else below deploy/ does, verbatim.
EXCLUDES=(--exclude='*.swp' --exclude='*~' --exclude='.DS_Store' --exclude='Thumbs.db' --exclude='desktop.ini')

cd "${DEPLOY_DIR}"

# CR characters: a CRLF checkout breaks sudoers, sshd drop-ins, systemd units and shebang lines in
# ways that are hard to see, so nothing is uploaded while one exists. deploy/.gitattributes keeps
# git from creating them; an editor still can.
#   -U  binary mode: a Windows build of grep would otherwise strip the CR before matching
#   LC_ALL=C  bytes, not characters
# The patterns live in variables: Git Bash loses a $'\r' that is written inside "$( ... )"; the
# pattern is then empty and matches every file.
CR=$'\r'
BOM=$'\xef\xbb\xbf'
crlf="$(LC_ALL=C grep -rlU "${EXCLUDES[@]}" -- "${CR}" . || true)"
if [[ -n "${crlf}" ]]; then
  printf '%s\n' "${crlf}" >&2
  die "the files above contain CR characters; convert them to LF (in deploy/: git add --renormalize . ; or dos2unix)"
fi
bom="$(LC_ALL=C grep -rlU "${EXCLUDES[@]}" -- "^${BOM}" . || true)"
if [[ -n "${bom}" ]]; then
  printf '%s\n' "${bom}" >&2
  die "the files above start a line with a UTF-8 byte order mark; save them as UTF-8 without BOM"
fi

# The server side only accepts regular files and directories.
odd="$(find . ! -type f ! -type d -print)"
if [[ -n "${odd}" ]]; then
  printf '%s\n' "${odd}" >&2
  die "only regular files and directories can be synced (no links or devices)"
fi

# A script that does not even parse must not reach a server where it runs as root.
while IFS= read -r -d '' script; do
  bash -n "${script}" || die "${script} has a syntax error"
done < <(find . -type f -name '*.sh' -print0)

files="$(find . -type f | wc -l)"
log "${files} files below ${DEPLOY_DIR}: no CR, no BOM, all *.sh parse"
if [[ "${SYNC_CHECK_ONLY:-0}" == "1" ]]; then
  log "SYNC_CHECK_ONLY=1: nothing uploaded"
  exit 0
fi

# ---------------------------------------------------------------- upload

if [[ "${ROOT_BOOTSTRAP}" == "1" ]]; then
  run_as_root=""
  log "ROOT_BOOTSTRAP=1: uploading as root, without sudo"
else
  # -n: fail instead of asking for a password that the deploy user does not have.
  run_as_root="sudo -n "
fi

# Runs in the login shell of the remote user (bash for root and for deploy; plain POSIX sh would do).
# Single quotes on purpose: every "$..." below is for the remote shell.
# shellcheck disable=SC2016
remote_command='set -eu; umask 077; t="$(mktemp -d)"; trap "rm -rf \"$t\"" EXIT; '
remote_command+='cat >"$t/deploy.tar.gz"; mkdir "$t/x"; '
remote_command+="tar -xzf \"\$t/deploy.tar.gz\" -C \"\$t/x\" ./${RECEIVER}; "
remote_command+="${run_as_root}bash \"\$t/x/${RECEIVER}\" \"\$t/deploy.tar.gz\" ${SYNC_PRUNE}"

# Git Bash rewrites arguments that look like POSIX paths when it starts a native Windows
# program (a Windows ssh.exe first in PATH): "/opt/betula" would become "C:/Program Files/Git/opt/betula".
export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*'
# macOS tar would add "._*" resource-fork files.
export COPYFILE_DISABLE=1

log "uploading to ${SSH_TARGET} -> /opt/betula (prune: ${SYNC_PRUNE})"
# pipefail: a failing tar fails the whole line even when ssh was happy with what it got.
# (the "+" form: an empty array is an "unbound variable" for the bash 3.2 that macOS ships)
tar "${EXCLUDES[@]}" -czf - . | ssh ${ssh_opts[@]+"${ssh_opts[@]}"} "${SSH_TARGET}" "${remote_command}"
log "done"
