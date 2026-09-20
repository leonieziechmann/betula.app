#!/usr/bin/env bash
# Installed by /opt/betula/vps/20-ssh-lockdown.sh as /usr/local/sbin/betula-ssh-rollback.
# The dead-man's switch of the ssh lockdown. 20-ssh-lockdown.sh starts a transient timer
# (betula-ssh-rollback.timer) right before it reloads sshd; "20-ssh-lockdown.sh --confirm", which
# only a NEW login as deploy can run, stops that timer. If nobody confirms, this script puts the
# sshd drop-ins back to what they were before the lockdown and reloads sshd, so root and
# password logins work again.
#
# Why it exists: the test login that 20-ssh-lockdown.sh asks for happens under the OLD settings
# (MaxAuthTries 6). A client whose agent offers three other keys first passes that test and
# fails every login afterwards. The operator drives this server through short non-interactive
# ssh sessions, so "keep this session open" is not a safety net; without this the only way back
# would be the provider's VNC console.
#
# Self-contained on purpose (no lib.sh): it must still work when /opt/betula is re-synced or
# half-updated while the timer is pending. Can also be run by hand as root.
set -Eeuo pipefail
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

STATE_DIR="${BETULA_SSH_STATE_DIR:-/run/betula/ssh-lockdown}"
ACTIONS="${STATE_DIR}/actions"
DROPIN_DIR="/etc/ssh/sshd_config.d"

say() { printf 'betula-ssh-rollback: %s\n' "$*"; }

if [[ "${EUID}" -ne 0 ]]; then
  say "must run as root" >&2
  exit 1
fi
if [[ ! -s "${ACTIONS}" ]]; then
  say "no unconfirmed lockdown is recorded in ${ACTIONS}; nothing to do"
  exit 0
fi

failed=0
# One action per line, tab separated: "remove <drop-in>" or "restore <backup> <drop-in>".
while IFS=$'\t' read -r verb first second; do
  case "${verb}" in
    remove)
      source_file=""
      target="${first}"
      ;;
    restore)
      source_file="${first}"
      target="${second}"
      ;;
    *)
      continue
      ;;
  esac
  # Whatever the state file says: this tool only ever touches sshd drop-ins.
  if [[ "${target}" != "${DROPIN_DIR}/"*.conf || "${target}" == *..* ]]; then
    say "refusing to touch '${target}'" >&2
    failed=1
    continue
  fi
  if [[ -n "${source_file}" && -f "${source_file}" ]]; then
    if cp -a -- "${source_file}" "${target}"; then
      say "restored ${target} from ${source_file}"
    else
      failed=1
    fi
  else
    # No backup (any more): removing the drop-in is the state that lets the operator back in.
    if rm -f -- "${target}"; then
      say "removed ${target}"
    else
      failed=1
    fi
  fi
done <"${ACTIONS}"

# Same as the unit's RuntimeDirectory=; with socket activation it may not exist right now.
[[ -d /run/sshd ]] || install -d -m 0755 /run/sshd
if sshd -t; then
  reloaded=0
  for unit in ssh.service sshd.service; do
    if systemctl is-active --quiet "${unit}"; then
      # Reload, never restart: running sessions stay up.
      if systemctl reload "${unit}"; then
        say "reloaded ${unit}; root and password logins follow the previous configuration again"
        reloaded=1
      else
        failed=1
      fi
      break
    fi
  done
  if [[ "${reloaded}" -eq 0 && "${failed}" -eq 0 ]]; then
    say "sshd is not running (socket activation); the next connection reads the restored configuration"
  fi
else
  say "sshd -t fails after the rollback; the problem is in a file this tool does not manage" >&2
  failed=1
fi

# A client that ran into MaxAuthTries a few times is banned by now, and a ban drops every packet
# of that address. Access is the whole point of this script, so the bans go as well; real
# attackers are banned again after their next five failures.
if command -v fail2ban-client >/dev/null 2>&1 && fail2ban-client ping >/dev/null 2>&1; then
  if fail2ban-client unban --all >/dev/null 2>&1; then
    say "fail2ban: all bans lifted"
  fi
fi

if [[ "${failed}" -eq 0 ]]; then
  rm -f -- "${ACTIONS}" "${STATE_DIR}/reloaded-at"
  say "done. Fix the client (IdentitiesOnly yes + IdentityFile), then run 20-ssh-lockdown.sh again."
else
  say "finished with errors; the state in ${STATE_DIR} is kept" >&2
fi
exit "${failed}"
