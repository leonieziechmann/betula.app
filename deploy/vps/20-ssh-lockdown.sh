#!/usr/bin/env bash
# 20-ssh-lockdown.sh - key-only ssh for the deploy user, root login off.
#
#   sudo bash /opt/betula/vps/20-ssh-lockdown.sh             # apply (arms an automatic rollback)
#   sudo bash /opt/betula/vps/20-ssh-lockdown.sh --confirm   # from a NEW login: make it final
#
# Run ONLY after this works from the operator's workstation:
#
#   ssh deploy@<host> sudo -n true          (or the ssh alias that logs in as deploy)
#
# The script does not take that on trust. It checks the preconditions itself and refuses to
# continue when one of them fails; it validates the new configuration before sshd sees it,
# compares the EFFECTIVE values afterwards, and on any failure takes the drop-ins it wrote in
# this run away again (a changed file goes back to its previous version) and exits non-zero.
# sshd is reloaded, never restarted; existing sessions (including the one running this) stay up.
# Root's password is left alone: the provider's VNC console is the break-glass path.
#
# Dead-man's switch: the test login above happens under the OLD settings. What the new ones do
# to the operator's client (MaxAuthTries 3 against an agent full of keys) only shows afterwards,
# and the operator works through short ssh sessions, so there is no "still open" session to fall
# back on. Right before the reload a transient systemd timer is armed that undoes the change
# after LOCKDOWN_ROLLBACK_MINUTES (files/betula-ssh-rollback.sh). Only "--confirm" stops it, and
# --confirm insists on a public key login of deploy that happened AFTER the reload.
#
# Environment:
#   LOCKDOWN_SKIP_LOGIN_PROOF=1    do not search the journal for "Accepted publickey for deploy"
#                                  (only if the journal was rotated since the test login)
#   LOCKDOWN_ROLLBACK_MINUTES=10   delay of the automatic rollback; 0 switches it off
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
betula_init --tmp
require_root
require_ubuntu
require_cmd sshd ssh-keygen visudo runuser systemctl journalctl

DROPIN_DIR="/etc/ssh/sshd_config.d"
MAIN_DROPIN="${DROPIN_DIR}/00-betula-hardening.conf"
PENALTY_DROPIN="${DROPIN_DIR}/01-betula-penalties.conf"
# Files this run created or replaced and has not yet proven good; the exit hook rolls exactly
# these back. A drop-in that was already in place and did not change is never removed: on a
# re-run that would reopen password and root logins because of somebody else's mistake.
ROLLBACK_PENDING=0
CHANGED_DROPINS=()

ROLLBACK_TOOL="/usr/local/sbin/betula-ssh-rollback"
ROLLBACK_UNIT="betula-ssh-rollback"
ROLLBACK_MINUTES="${LOCKDOWN_ROLLBACK_MINUTES:-10}"
# Volatile on purpose: a reboot drops the transient timer too, and the state with it.
STATE_DIR="${BETULA_RUN_DIR}/ssh-lockdown"
ACTIONS_FILE="${STATE_DIR}/actions"
RELOADED_AT_FILE="${STATE_DIR}/reloaded-at"
ARMED_THIS_RUN=0
STATE_NEW_THIS_RUN=0

# What sshd has to report (sshd -T prints keywords in lower case) once the drop-in is active.
EXPECTED=(
  "permitrootlogin no"
  "pubkeyauthentication yes"
  "passwordauthentication no"
  "kbdinteractiveauthentication no"
  "permitemptypasswords no"
  "authenticationmethods publickey"
  "allowusers ${DEPLOY_USER}"
  "maxauthtries 3"
  "logingracetime 30"
  "x11forwarding no"
  "allowagentforwarding no"
  "allowtcpforwarding local"
  "allowstreamlocalforwarding no"
  "gatewayports no"
  "permittunnel no"
)

# ---------------------------------------------------------------- helpers

sshd_test() {
  # sshd -t wants its privilege separation directory; with socket activation it only exists
  # while ssh.service runs. Creating it is what the unit's RuntimeDirectory= does as well.
  [[ -d /run/sshd ]] || install -d -m 0755 /run/sshd
  sshd -t
}

# effective_config USER -> the configuration sshd would apply to a connection of USER
# (resolves Match blocks; the address is from the documentation range).
effective_config() {
  sshd -T -C "user=$1,host=client.invalid,addr=203.0.113.10"
}

reload_sshd() {
  # Ubuntu starts sshd through ssh.socket. While ssh.service runs (it does on this server), a
  # reload (SIGHUP) makes it re-read the configuration and keeps the listening socket and all
  # sessions. When it is not running there is nothing to reload: the next connection starts sshd
  # with the new files. The socket unit itself only knows the port, which we do not change, so
  # it is never touched.
  local unit
  for unit in ssh.service sshd.service; do
    if unit_active "${unit}"; then
      # Explicit "|| return": callers use this function in a condition, where set -e is off.
      systemctl reload "${unit}" || return 1
      log "reloaded ${unit} (no restart; running sessions are untouched)"
      return 0
    fi
  done
  log "sshd is not running right now (socket activation); the next connection reads the new configuration"
}

timer_armed() { unit_active "${ROLLBACK_UNIT}.timer"; }

disarm_timer() {
  systemctl stop "${ROLLBACK_UNIT}.timer" 2>/dev/null || true
  systemctl reset-failed "${ROLLBACK_UNIT}.timer" "${ROLLBACK_UNIT}.service" 2>/dev/null || true
}

# find_login_proof SINCE - newest "Accepted publickey for deploy" journal line since SINCE
# (anything journalctl --since understands, e.g. "-30days" or "@<epoch>"); empty if there is none.
find_login_proof() {
  local since=$1 proof
  proof="$(journalctl --quiet --no-pager --output=cat --since="${since}" --lines=1 \
    --grep="^Accepted publickey for ${DEPLOY_USER} from " 2>/dev/null || true)"
  if [[ -z "${proof}" ]]; then
    # journalctl without pattern support, or a filtered view: look at sshd's own lines.
    proof="$(journalctl --quiet --no-pager --output=cat --since="${since}" \
      _COMM=sshd _COMM=sshd-session _COMM=sshd-auth 2>/dev/null |
      grep -F "Accepted publickey for ${DEPLOY_USER} from " | tail -n 1 || true)"
  fi
  printf '%s' "${proof}"
}

restore_or_remove() {
  local path=$1 backup
  backup="$(backup_path_of "${path}")"
  if [[ -n "${backup}" ]]; then
    cp -a -- "${backup}" "${path}"
    warn "restored the previous ${path}"
  else
    rm -f -- "${path}"
    warn "removed ${path}"
  fi
}

rollback() {
  local rc=${1:-1} path
  [[ "${ROLLBACK_PENDING}" -eq 1 ]] || return 0
  ROLLBACK_PENDING=0
  if [[ "${#CHANGED_DROPINS[@]}" -eq 0 ]]; then
    warn "validation failed, but this run changed no ssh drop-in; nothing to roll back"
    return 0
  fi
  warn "rolling back the ssh drop-ins of this run (exit code ${rc})"
  for path in "${CHANGED_DROPINS[@]}"; do
    restore_or_remove "${path}"
  done
  if sshd -t 2>/dev/null; then
    reload_sshd || true
  else
    warn "sshd -t still fails after the rollback; the problem is in a file this script did not write"
  fi
  # State that THIS run created has nothing left to undo. The timer of an earlier, still
  # unconfirmed run keeps counting: its lockdown is the active configuration again now.
  if [[ "${STATE_NEW_THIS_RUN}" -eq 1 ]]; then
    disarm_timer
    rm -f -- "${ACTIONS_FILE}" "${RELOADED_AT_FILE}"
  fi
}
add_exit_hook rollback

# ---------------------------------------------------------------- steps

check_preconditions() {
  step "Preconditions (refuse to lock anybody out)"
  local home ssh_dir keys perm owner path groups shadow_state usepam proof cfg

  [[ "${ROLLBACK_MINUTES}" =~ ^[0-9]{1,4}$ ]] || die "LOCKDOWN_ROLLBACK_MINUTES='${ROLLBACK_MINUTES}' is not a number of minutes"
  if [[ "${ROLLBACK_MINUTES}" -gt 0 ]]; then
    require_cmd systemd-run
    [[ -f "${BETULA_FILES_DIR}/betula-ssh-rollback.sh" ]] || die "payload missing: ${BETULA_FILES_DIR}/betula-ssh-rollback.sh"
  fi

  sshd_test || die "sshd -t fails BEFORE any change; fix the existing configuration first"
  [[ -d "${DROPIN_DIR}" ]] || die "${DROPIN_DIR} does not exist; this sshd_config has no drop-in directory"

  id -u "${DEPLOY_USER}" >/dev/null 2>&1 || die "user ${DEPLOY_USER} does not exist (run 10-base.sh)"
  home="$(getent passwd "${DEPLOY_USER}" | cut -d: -f6)"
  ssh_dir="${home}/.ssh"
  keys="${ssh_dir}/authorized_keys"

  case "$(getent passwd "${DEPLOY_USER}" | cut -d: -f7)" in
    */nologin | */false | "") die "${DEPLOY_USER} has no login shell" ;;
  esac

  [[ -f "${keys}" && ! -L "${keys}" ]] || die "${keys} is missing or not a regular file"
  [[ -s "${keys}" ]] || die "${keys} is empty"
  ssh-keygen -l -f "${keys}" >/dev/null 2>&1 || die "${keys} contains no valid public key"

  # sshd's StrictModes ignores authorized_keys when the file, ~/.ssh or the home directory
  # belong to somebody else or are writable by group/others.
  for path in "${home}" "${ssh_dir}" "${keys}"; do
    owner="$(stat -c %U "${path}")"
    perm="$(stat -c %a "${path}")"
    [[ "${owner}" == "${DEPLOY_USER}" ]] || die "${path} is owned by ${owner}, not ${DEPLOY_USER}"
    if (( (8#${perm} & 8#022) != 0 )); then
      die "${path} has mode ${perm}: writable by group or others, sshd would ignore the keys"
    fi
  done
  perm="$(stat -c %a "${keys}")"
  [[ "${perm}" == "600" || "${perm}" == "400" ]] || die "${keys} has mode ${perm}; expected 600"
  perm="$(stat -c %a "${ssh_dir}")"
  [[ "${perm}" == "700" ]] || die "${ssh_dir} has mode ${perm}; expected 700"
  log "authorized_keys: $(ssh-keygen -l -f "${keys}" | wc -l) key(s), owner and modes are correct"

  groups=" $(id -nG "${DEPLOY_USER}") "
  [[ "${groups}" == *" sudo "* ]] || die "${DEPLOY_USER} is not in group sudo"
  visudo -c >/dev/null || die "visudo -c reports an invalid sudoers configuration"
  runuser -u "${DEPLOY_USER}" -- sudo -n true || die "'sudo -n true' fails for ${DEPLOY_USER}: no passwordless sudo"
  log "sudo: group membership, sudoers syntax and passwordless sudo are fine"

  # A locked password blocks key logins too when sshd runs without PAM.
  shadow_state="$(passwd --status "${DEPLOY_USER}" | awk '{ print $2 }')"
  cfg="$(sshd -T 2>/dev/null || true)"
  usepam="$(awk '$1 == "usepam" { print $2 }' <<<"${cfg}")"
  if [[ "${shadow_state}" == "L" && "${usepam}" != "yes" ]]; then
    die "${DEPLOY_USER} has a locked password and sshd runs with UsePAM ${usepam:-?}: sshd would refuse the account"
  fi

  # The operator's test login leaves a trace. No trace, no lockdown.
  if [[ "${LOCKDOWN_SKIP_LOGIN_PROOF:-0}" == "1" ]]; then
    warn "LOCKDOWN_SKIP_LOGIN_PROOF=1: not looking for a successful key login of ${DEPLOY_USER}"
  else
    proof="$(find_login_proof "-30days")"
    [[ -n "${proof}" ]] || die "the journal shows no successful public key login of ${DEPLOY_USER}. Run 'ssh ${DEPLOY_USER}@<host> sudo -n true' from your workstation first (LOCKDOWN_SKIP_LOGIN_PROOF=1 overrides)"
    log "proof of a working key login: ${proof}"
  fi

  # Said BEFORE the change, not after it: that login was accepted with MaxAuthTries 6.
  log "client check: MaxAuthTries drops to 3. If your ssh agent holds other keys, the host alias"
  log "  needs 'IdentitiesOnly yes' next to its IdentityFile, or the right key is never offered."
}

install_dropins() {
  step "Install the sshd drop-ins"
  local first cfg penalty_changed

  if [[ "${ROLLBACK_MINUTES}" -gt 0 ]]; then
    install_file "${BETULA_FILES_DIR}/betula-ssh-rollback.sh" "${ROLLBACK_TOOL}" 0755
  fi

  ROLLBACK_PENDING=1
  install_file "${BETULA_FILES_DIR}/sshd-00-betula-hardening.conf" "${MAIN_DROPIN}" 0644
  if [[ "${INSTALL_CHANGED}" -eq 1 ]]; then CHANGED_DROPINS+=("${MAIN_DROPIN}"); fi
  sshd_test || die "sshd -t rejects the configuration with ${MAIN_DROPIN} in place"

  # PerSourcePenalties exists since OpenSSH 9.8. Ask this sshd instead of parsing version strings.
  # (Captured first: "sshd -T | awk ... exit" would die of SIGPIPE under pipefail.)
  cfg="$(sshd -T 2>/dev/null || true)"
  if contains_line "${cfg}" '^persourcepenalties '; then
    install_file "${BETULA_FILES_DIR}/sshd-01-betula-penalties.conf" "${PENALTY_DROPIN}" 0644
    penalty_changed="${INSTALL_CHANGED}"
    if sshd_test; then
      if [[ "${penalty_changed}" -eq 1 ]]; then CHANGED_DROPINS+=("${PENALTY_DROPIN}"); fi
    else
      # Optional extra; it must never stand between us and a working lockdown. A version that an
      # earlier run installed (and sshd accepted) comes back, otherwise the file goes.
      warn "sshd -t rejects the new ${PENALTY_DROPIN}; continuing without it"
      if [[ "${penalty_changed}" -eq 1 ]]; then
        restore_or_remove "${PENALTY_DROPIN}"
      else
        remove_file "${PENALTY_DROPIN}"
      fi
      if ! sshd_test; then
        rm -f -- "${PENALTY_DROPIN}"
        sshd_test || die "sshd -t fails even without ${PENALTY_DROPIN}"
      fi
    fi
  else
    log "this sshd has no PerSourcePenalties (OpenSSH < 9.8); skipping ${PENALTY_DROPIN}"
    remove_file "${PENALTY_DROPIN}"
  fi

  # First match wins, so our file has to be the first one sshd reads.
  first="$(find "${DROPIN_DIR}" -maxdepth 1 -name '*.conf' -printf '%f\n' | sort | sed -n '1p' || true)"
  if [[ "${first}" != "$(basename "${MAIN_DROPIN}")" ]]; then
    warn "${first} sorts before $(basename "${MAIN_DROPIN}"); the effective values below decide whether that matters"
  fi
}

check_effective() {
  step "Effective configuration (sshd -T)"
  local user cfg want key bad=0 value group ok=0
  for user in "${DEPLOY_USER}" root; do
    cfg="$(effective_config "${user}")" || die "sshd -T failed for user ${user}"
    for want in "${EXPECTED[@]}"; do
      key="${want%% *}"
      # AllowUsers is a list: every reported line has to be ours, not merely one of them.
      value="$(awk -v k="${key}" '$1 == k { $1 = ""; sub(/^ /, ""); print }' <<<"${cfg}" | sort -u | tr '\n' ' ')"
      value="${value% }"
      if [[ "${value}" == "${want#* }" ]]; then
        log "ok   (${user}) ${want}"
      else
        warn "FAIL (${user}) ${key}: effective '${value:-<unset>}', wanted '${want#* }'"
        bad=$((bad + 1))
      fi
    done
    # Directives of other files that would still keep deploy out although ours are effective.
    if contains_line "${cfg}" "^denyusers .*\\b${DEPLOY_USER}\\b"; then
      warn "FAIL (${user}) a DenyUsers directive names ${DEPLOY_USER}"
      bad=$((bad + 1))
    fi
  done

  cfg="$(effective_config "${DEPLOY_USER}")"
  if contains_line "${cfg}" '^allowgroups '; then
    for group in $(id -nG "${DEPLOY_USER}"); do
      if contains_line "${cfg}" "^allowgroups (.* )?${group}( |\$)"; then ok=1; fi
    done
    if [[ "${ok}" -eq 0 ]]; then
      warn "FAIL an AllowGroups directive exists and ${DEPLOY_USER} is in none of its groups"
      bad=$((bad + 1))
    fi
  fi
  for group in $(id -nG "${DEPLOY_USER}"); do
    if contains_line "${cfg}" "^denygroups (.* )?${group}( |\$)"; then
      warn "FAIL a DenyGroups directive names ${group}, a group of ${DEPLOY_USER}"
      bad=$((bad + 1))
    fi
  done

  if [[ "${bad}" -gt 0 ]]; then
    warn "a file that sshd reads earlier sets conflicting values; candidates:"
    grep -EnHi '^[[:space:]]*(permitrootlogin|passwordauthentication|kbdinteractiveauthentication|challengeresponseauthentication|authenticationmethods|allowusers|allowgroups|denyusers|denygroups|allowtcpforwarding|match)\b' \
      /etc/ssh/sshd_config "${DROPIN_DIR}"/*.conf 2>/dev/null | grep -v "^${MAIN_DROPIN}:" >&2 || true
    die "${bad} effective value(s) differ from the hardening drop-in"
  fi
  if [[ -e "${PENALTY_DROPIN}" ]]; then
    log "ok   persourcepenalties $(awk '$1 == "persourcepenalties" { $1 = ""; print }' <<<"${cfg}")"
  fi
}

arm_rollback() {
  step "Arm the automatic rollback (${ROLLBACK_MINUTES} min)"
  local path backup
  if [[ "${#CHANGED_DROPINS[@]}" -eq 0 ]]; then
    if timer_armed; then
      log "no drop-in changed in this run; the rollback that an earlier run armed keeps counting"
    else
      log "no drop-in changed in this run; nothing to undo, nothing to arm"
    fi
    return 0
  fi
  if [[ "${ROLLBACK_MINUTES}" -eq 0 ]]; then
    warn "LOCKDOWN_ROLLBACK_MINUTES=0: no automatic rollback. The VNC console is the only way back."
    return 0
  fi

  install -d -m 0755 "${BETULA_RUN_DIR}"
  install -d -m 0700 "${STATE_DIR}"
  if [[ ! -s "${ACTIONS_FILE}" ]]; then
    STATE_NEW_THIS_RUN=1
    : >"${ACTIONS_FILE}"
  fi
  for path in "${CHANGED_DROPINS[@]}"; do
    # An entry of an earlier, still unconfirmed run wins: it describes the state BEFORE the
    # lockdown, and that is where a rollback has to end up.
    if grep -qF -- $'\t'"${path}" "${ACTIONS_FILE}"; then
      continue
    fi
    backup="$(backup_path_of "${path}")"
    if [[ -n "${backup}" ]]; then
      printf 'restore\t%s\t%s\n' "${backup}" "${path}" >>"${ACTIONS_FILE}"
    else
      printf 'remove\t%s\n' "${path}" >>"${ACTIONS_FILE}"
    fi
  done

  # A timer left over from an earlier run is replaced: the clock starts again with this reload.
  disarm_timer
  systemd-run --quiet --unit="${ROLLBACK_UNIT}" \
    --description="Betula: undo the ssh lockdown unless it was confirmed" \
    --on-active="${ROLLBACK_MINUTES}min" --timer-property=AccuracySec=1s \
    "${ROLLBACK_TOOL}" ||
    die "systemd-run could not arm ${ROLLBACK_UNIT}.timer; no lockdown without a safety net"
  ARMED_THIS_RUN=1
  timer_armed || die "${ROLLBACK_UNIT}.timer is not active after systemd-run; no lockdown without a safety net"
  log "${ROLLBACK_UNIT}.timer armed: fires in ${ROLLBACK_MINUTES} min unless '--confirm' stops it"
}

activate() {
  step "Reload sshd"
  local listeners
  reload_sshd || die "reloading sshd failed"
  # From here on the new configuration is validated and active; the exit hook has nothing left
  # to do. Whether the operator's client copes with it is the dead-man's switch's business.
  ROLLBACK_PENDING=0
  if [[ "${ARMED_THIS_RUN}" -eq 1 ]]; then
    # --confirm only accepts logins that are newer than this moment.
    date +%s >"${RELOADED_AT_FILE}"
  fi
  listeners="$(ss -H -tln "sport = :${SSH_PORT}" 2>/dev/null || true)"
  [[ -n "${listeners}" ]] || warn "nothing listens on port ${SSH_PORT} right now; check 'systemctl status ssh.socket ssh.service'"
}

report() {
  step "Done"
  cat <<EOF

ssh is now: user ${DEPLOY_USER} only, public key only, no root login, no passwords.

  Test from a NEW connection (not a multiplexed one: ssh -o ControlPath=none ...); if this is
  an interactive session, keep it open until the test has worked:
      ssh ${DEPLOY_USER}@<host> sudo -n true        # must work (or your ssh alias for ${DEPLOY_USER})
      ssh root@<host>                        # must be refused

  Manual rollback (as root on the provider's VNC console; root's password was not touched):
      rm -f ${MAIN_DROPIN} ${PENALTY_DROPIN}
      sshd -t && systemctl reload ssh

  Client hint: MaxAuthTries is 3. If your agent holds several keys, use
      ssh -o IdentitiesOnly=yes -i <keyfile> ${DEPLOY_USER}@<host>
EOF
  if timer_armed; then
    cat <<EOF

  NOT FINAL YET: ${ROLLBACK_UNIT}.timer undoes this change automatically unless a new
  login as ${DEPLOY_USER} confirms it in time:
      ssh ${DEPLOY_USER}@<host> sudo bash /opt/betula/vps/20-ssh-lockdown.sh --confirm
  (do not reboot before that: a reboot drops the timer and keeps the lockdown)
EOF
    # Machine-readable marker for the operator's automation; keep it the last line.
    printf 'LOCKDOWN_CONFIRM_REQUIRED\n'
  fi
}

confirm() {
  step "Confirm the lockdown (stops the automatic rollback)"
  local since="" proof
  if ! timer_armed; then
    # Nothing is counting down: confirmed before, or the timer has fired and undone the change.
    if [[ -s "${ACTIONS_FILE}" ]]; then
      warn "the automatic rollback ran and reported errors: journalctl -u ${ROLLBACK_UNIT}.service"
    fi
    [[ -f "${MAIN_DROPIN}" ]] ||
      die "${MAIN_DROPIN} is not in place (the automatic rollback fired, or the lockdown never ran); run this script without --confirm"
    log "no rollback is pending and ${MAIN_DROPIN} is in place; nothing to confirm"
    return 0
  fi

  if [[ "${LOCKDOWN_SKIP_LOGIN_PROOF:-0}" == "1" ]]; then
    warn "LOCKDOWN_SKIP_LOGIN_PROOF=1: confirming without proof of a login under the new settings"
  else
    if [[ -r "${RELOADED_AT_FILE}" ]]; then since="$(<"${RELOADED_AT_FILE}")"; fi
    [[ "${since}" =~ ^[0-9]+$ ]] ||
      die "${RELOADED_AT_FILE} is missing; cannot tell new logins from old ones (LOCKDOWN_SKIP_LOGIN_PROOF=1 overrides)"
    # A session that was already open when sshd reloaded proves nothing about the new settings.
    proof="$(find_login_proof "@${since}")"
    [[ -n "${proof}" ]] ||
      die "no public key login of ${DEPLOY_USER} since the reload. Open a NEW connection (ssh -o ControlPath=none ...) and run --confirm from it; the automatic rollback stays armed"
    log "login under the new settings: ${proof}"
  fi

  disarm_timer
  if timer_armed; then
    die "could not stop ${ROLLBACK_UNIT}.timer; stop it by hand: systemctl stop ${ROLLBACK_UNIT}.timer"
  fi
  rm -f -- "${ACTIONS_FILE}" "${RELOADED_AT_FILE}"
  log "lockdown confirmed; the automatic rollback is cancelled"
}

# ---------------------------------------------------------------- main

case "${1:-}" in
  "")
    check_preconditions
    install_dropins
    check_effective
    arm_rollback
    activate
    report
    ;;
  --confirm)
    confirm
    ;;
  *)
    die "unknown argument '$1' (usage: $(basename "$0") [--confirm])"
    ;;
esac
