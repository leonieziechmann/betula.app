#!/usr/bin/env bash
# 10-base.sh - base hardening of a fresh Ubuntu server. Run as root in the initial root session:
#
#   bash /opt/betula/vps/10-base.sh
#
# Idempotent and non-interactive; safe to re-run after an interrupted session.
# It does NOT touch sshd's authentication settings and leaves root's password alone: the root
# login keeps working until 20-ssh-lockdown.sh runs, which is only after the operator has proven
# that "ssh deploy@host sudo -n true" works. The cloud image's unused "ubuntu" account loses its
# login and its passwordless sudo here; the time zone stays Europe/Berlin (lib.sh).
#
# Environment:
#   DEPLOY_PUBKEY_FILE   file with the public key(s) for the deploy user
#                        (default: /root/.ssh/authorized_keys)
#
# Prints REBOOT_REQUIRED as the last line when the upgrade wants a reboot.
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
betula_init --tmp
require_root
require_ubuntu

DEPLOY_PUBKEY_FILE="${DEPLOY_PUBKEY_FILE:-/root/.ssh/authorized_keys}"

# Every package has a reason to be here; nothing is installed "because servers have it".
BASE_PACKAGES=(
  ca-certificates    # TLS trust for apt over https (Docker repository) and for curl
  curl               # 30-docker.sh fetches Docker's repository key; health checks from the shell
  ufw                # host firewall front end; drives the kernel's nftables through iptables-nft
  fail2ban           # long bans for ssh brute force (the firewall rule for 22 is a plain allow)
  python3-systemd    # fail2ban's journal backend; only a Recommends, so it has to be named
  nftables           # the nft binary: fail2ban bans in its own nftables table
  unattended-upgrades # automatic security updates and the 04:30 reboot
  needrestart        # restarts services that still run an outdated library after an upgrade
  jq                 # validates daemon.json; 91-verify-stacks.sh reads the Loki/Prometheus answers;
                     # the scraper logs JSON lines (docs/operations.md)
  bind9-dnsutils     # dig: 40-stacks.sh checks that a host name resolves to this server before it
                     # lets Traefik request a certificate (Let's Encrypt rate limits)
  # No rsync: deploy/sync.sh streams a tar archive over ssh (Git Bash on Windows has no rsync),
  # and tar and gzip are part of every Ubuntu installation.
)

# Accounts that provider images create and nobody uses here. The cloud image's "ubuntu" user
# holds passwordless sudo (/etc/sudoers.d/90-cloud-init-users) and sits in the root-equivalent
# lxd group: a second root that only a locked password keeps shut. It ends up without login and
# without privileges; disabled rather than deleted, so uid 1000 is never handed out again and
# cloud-init (which would re-create a missing default user on a new instance id) finds it there.
UNUSED_LOGIN_USERS=(ubuntu)

# The units this server does not need are listed in lib.sh (BETULA_UNNEEDED_UNITS): the audit
# script reads the same list.

# ---------------------------------------------------------------- steps

# dpkg has no lock timeout of its own (DPkg::Lock::Timeout only helps apt-get), and on a fresh
# server apt-daily or unattended-upgrades can hold the lock at any moment. Wait for them instead
# of dying with "dpkg frontend lock was locked by another process".
dpkg_configure_pending() {
  local attempt out
  for attempt in $(seq 1 30); do
    # Captured, because the reason decides: a held lock is worth waiting for, anything else is not.
    # (Normally this prints nothing at all; messages are English, betula_init sets LC_ALL.)
    if out="$(DEBIAN_FRONTEND=noninteractive dpkg --force-confdef --force-confold --configure -a 2>&1)"; then
      [[ -z "${out}" ]] || printf '%s\n' "${out}"
      return 0
    fi
    printf '%s\n' "${out}" >&2
    [[ "${out}" == *"lock"* ]] || return 1
    log "the dpkg lock is held by another process; waiting 20 s (attempt ${attempt}/30)"
    sleep 20
  done
  return 1
}

upgrade_system() {
  step "System upgrade (non-interactive, existing conffiles are kept)"
  # Repairs a dpkg run that a dropped ssh session interrupted; a no-op otherwise.
  dpkg_configure_pending || die "dpkg --configure -a failed; look at the output above"
  apt_update --force
  apt_run full-upgrade
  apt_run autoremove --purge
}

install_base_packages() {
  step "Base packages"
  apt_install "${BASE_PACKAGES[@]}"
}

configure_needrestart() {
  step "needrestart: restart services automatically, but never the container runtime"
  install_file "${BETULA_FILES_DIR}/needrestart-betula.conf" /etc/needrestart/conf.d/50-betula.conf 0644
}

configure_time() {
  step "Timezone ${HOST_TIMEZONE} and time synchronisation"
  local tz
  tz="$(timedatectl show --property=Timezone --value 2>/dev/null || true)"
  if [[ "${tz}" == "${HOST_TIMEZONE}" ]]; then
    log "timezone already ${HOST_TIMEZONE}"
  else
    # Local time of the audience, not UTC (owner decision, lib.sh): unattended-upgrades reboots at
    # 04:30 HOST time and the prune timer runs at 03:30 host time; both have to be night in
    # Germany in summer and in winter. A rebuilt server may come up with another zone.
    [[ -e "/usr/share/zoneinfo/${HOST_TIMEZONE}" ]] || die "time zone ${HOST_TIMEZONE} is unknown here (package tzdata?)"
    timedatectl set-timezone "${HOST_TIMEZONE}"
    log "timezone: ${tz:-unknown} -> ${HOST_TIMEZONE}"
  fi

  if is_container; then
    warn "container guest: the clock belongs to the host, skipping time synchronisation"
    return 0
  fi
  # Use what the release ships (26.04: chrony with NTS, 24.04: systemd-timesyncd) instead of
  # forcing one daemon; two of them fight over the clock.
  if pkg_installed chrony; then
    systemctl enable --now chrony.service
    log "time sync: chrony"
  elif unit_exists systemd-timesyncd.service; then
    timedatectl set-ntp true
    log "time sync: systemd-timesyncd"
  else
    apt_install chrony
    systemctl enable --now chrony.service
    log "time sync: chrony (installed)"
  fi
}

create_deploy_user() {
  step "Admin user '${DEPLOY_USER}' (sudo, key-only, locked password)"
  local home ssh_dir target line tmp fp state added=0 accepted=0
  local key_re='^(ssh-ed25519|ssh-rsa|ecdsa-sha2-nistp(256|384|521)|sk-ssh-ed25519@openssh\.com|sk-ecdsa-sha2-nistp256@openssh\.com)[[:space:]]'

  [[ -s "${DEPLOY_PUBKEY_FILE}" ]] || die "no public keys: ${DEPLOY_PUBKEY_FILE} is missing or empty (set DEPLOY_PUBKEY_FILE)"

  if id -u "${DEPLOY_USER}" >/dev/null 2>&1; then
    log "user ${DEPLOY_USER} exists"
  else
    useradd --create-home --shell /bin/bash --comment "Betula deploy and admin" "${DEPLOY_USER}"
    log "user ${DEPLOY_USER} created"
  fi
  usermod --append --groups sudo "${DEPLOY_USER}"
  # The docker group only exists after 30-docker.sh, which adds the user itself.
  if getent group docker >/dev/null; then
    usermod --append --groups docker "${DEPLOY_USER}"
  fi

  # Locked password: no password login, no su, no sudo password. Key login still works because
  # Ubuntu's sshd runs with UsePAM yes (20-ssh-lockdown.sh checks that before it locks root out).
  state="$(passwd --status "${DEPLOY_USER}" | awk '{ print $2 }')"
  if [[ "${state}" != "L" ]]; then
    passwd --lock "${DEPLOY_USER}" >/dev/null
    log "password of ${DEPLOY_USER} locked"
  fi

  home="$(getent passwd "${DEPLOY_USER}" | cut -d: -f6)"
  [[ -d "${home}" ]] || die "home directory of ${DEPLOY_USER} not found: ${home}"
  ssh_dir="${home}/.ssh"
  target="${ssh_dir}/authorized_keys"
  install -d -m 0700 -o "${DEPLOY_USER}" -g "${DEPLOY_USER}" "${ssh_dir}"
  if [[ ! -e "${target}" ]]; then
    # Born private (umask), then handed over; no "install /dev/null", which not every coreutils
    # implementation accepts as a source (26.04 ships the Rust rewrite).
    (
      umask 077
      : >"${target}"
    )
    chown "${DEPLOY_USER}:${DEPLOY_USER}" "${target}"
  fi
  # A last line without newline would be glued to the first key we append.
  if [[ -s "${target}" && -n "$(tail -c 1 "${target}")" ]]; then
    printf '\n' >>"${target}"
  fi

  # Merge, never replace: keys the operator added by hand survive a re-run.
  tmp="$(betula_tmpfile)"
  while IFS= read -r line || [[ -n "${line}" ]]; do
    line="${line%$'\r'}"
    [[ -z "${line}" || "${line}" == \#* ]] && continue
    if ! [[ "${line}" =~ ${key_re} ]]; then
      # Cloud images prefix root's keys with command="echo 'Please login as ...'"; copied to
      # deploy, such a line would turn every login into that echo.
      warn "skipping a line of ${DEPLOY_PUBKEY_FILE} that does not start with a key type (options prefix?)"
      continue
    fi
    printf '%s\n' "${line}" >"${tmp}"
    if ! fp="$(ssh-keygen -l -f "${tmp}" 2>/dev/null)"; then
      warn "skipping a line of ${DEPLOY_PUBKEY_FILE} that ssh-keygen cannot parse"
      continue
    fi
    accepted=$((accepted + 1))
    if grep -qxF -- "${line}" "${target}"; then
      log "key already present: ${fp}"
    else
      printf '%s\n' "${line}" >>"${target}"
      added=$((added + 1))
      log "key added: ${fp}"
    fi
  done <"${DEPLOY_PUBKEY_FILE}"
  [[ "${accepted}" -ge 1 ]] || die "${DEPLOY_PUBKEY_FILE} contains no usable public key; ${DEPLOY_USER} could never log in"
  chown "${DEPLOY_USER}:${DEPLOY_USER}" "${target}"
  chmod 0600 "${target}"
  log "authorized_keys of ${DEPLOY_USER}: ${accepted} key(s) from ${DEPLOY_PUBKEY_FILE}, ${added} new"

  # Syntax check of the payload first, then of the whole configuration with the file in place;
  # a broken file in sudoers.d disables sudo for everybody.
  # (Checked as a 0440 copy: visudo also complains about the mode, and the synced file is 0644.)
  assert_lf "${BETULA_FILES_DIR}/sudoers-deploy"
  install -m 0440 "${BETULA_FILES_DIR}/sudoers-deploy" "${tmp}"
  visudo -c -f "${tmp}" >/dev/null || die "sudoers payload does not validate"
  install_file "${BETULA_FILES_DIR}/sudoers-deploy" /etc/sudoers.d/90-betula-deploy 0440
  if ! visudo -c >/dev/null; then
    rm -f /etc/sudoers.d/90-betula-deploy
    die "sudoers configuration invalid with 90-betula-deploy in place; file removed again"
  fi
  # The real thing, as the user: proves NOPASSWD is effective (last match wins in sudoers).
  runuser -u "${DEPLOY_USER}" -- sudo -n true ||
    die "'sudo -n true' fails for ${DEPLOY_USER}; check /etc/sudoers.d for a later, stricter rule"
  log "passwordless sudo works for ${DEPLOY_USER}"
}

disable_unused_accounts() {
  step "Provider accounts nobody uses: no login, no groups, no sudo"
  # Runs after create_deploy_user on purpose: passwordless sudo for deploy is proven by now, and
  # root's login is untouched, so taking privileges away here cannot lock anybody out.
  local user shell shadow pw expiry extra file rules foreign backup
  local -a lock_args
  for user in "${UNUSED_LOGIN_USERS[@]}"; do
    if ! id -u "${user}" >/dev/null 2>&1; then
      log "no account '${user}' on this host"
      continue
    fi
    if [[ "${user}" == "root" || "${user}" == "${DEPLOY_USER}" ]]; then
      continue
    fi
    # In use after all (somebody logged in as this user and ran us through sudo)? Hands off;
    # 90-verify-host.sh keeps reporting the account until a human decides.
    if [[ "${user}" == "${SUDO_USER:-}" || -n "$(pgrep -u "${user}" 2>/dev/null || true)" ]]; then
      warn "account ${user} is in use right now (running processes); left alone"
      continue
    fi

    # 1. No login: locked password, expired account (PAM refuses ssh keys, su and cron as well),
    #    and no shell. Any authorized_keys file stays where it is; it is useless now.
    shell="$(getent passwd "${user}" | cut -d: -f7)"
    shadow="$(getent shadow "${user}" || true)"
    pw="$(cut -d: -f2 <<<"${shadow}")"
    expiry="$(cut -d: -f8 <<<"${shadow}")"
    lock_args=()
    if [[ "${pw}" != "!"* && "${pw}" != "*"* ]]; then lock_args+=(--lock); fi
    if [[ "${expiry}" != "1" ]]; then lock_args+=(--expiredate 1); fi
    if [[ "${shell}" != */nologin ]]; then lock_args+=(--shell /usr/sbin/nologin); fi
    if [[ "${#lock_args[@]}" -gt 0 ]]; then
      usermod "${lock_args[@]}" "${user}"
      log "account ${user}: ${lock_args[*]}"
    else
      log "account ${user} is already locked, expired and without a shell"
    fi

    # 2. No supplementary groups: sudo, lxd and docker are root, adm reads every log.
    extra="$(id -nG "${user}" | tr ' ' '\n' | grep -vxF -- "$(id -ng "${user}")" | tr '\n' ' ' || true)"
    if [[ -n "${extra// /}" ]]; then
      usermod --groups "" "${user}"
      log "account ${user} removed from: ${extra% }"
    fi

    # 3. No sudo rule. A file that holds nothing but this user's rules (cloud-init's
    #    90-cloud-init-users) goes away, with a backup; a mixed file is for a human.
    for file in /etc/sudoers.d/*; do
      [[ -f "${file}" ]] || continue
      rules="$(sudoers_rules "${file}")"
      contains_line "${rules}" "^[[:space:]]*${user}[[:space:]]" || continue
      foreign="$(grep -Ev -- "^[[:space:]]*${user}[[:space:]]" <<<"${rules}" || true)"
      if [[ -n "${foreign}" ]]; then
        warn "${file} has a sudo rule for ${user} next to other rules; edit it by hand (visudo -f ${file})"
        continue
      fi
      remove_file "${file}"
      if ! visudo -c >/dev/null; then
        backup="$(backup_path_of "${file}")"
        if [[ -n "${backup}" ]]; then cp -a -- "${backup}" "${file}"; fi
        die "sudoers configuration invalid without ${file}; file restored"
      fi
    done
  done
  # Whatever was removed above: the admin's own sudo must have survived it.
  runuser -u "${DEPLOY_USER}" -- sudo -n true ||
    die "'sudo -n true' fails for ${DEPLOY_USER} after the clean-up; restore from ${BETULA_BACKUP_ROOT}/${BETULA_RUN_ID}"
}

# ufw_default_is KEY VALUE - true when /etc/default/ufw already says DEFAULT_<KEY>_POLICY="VALUE".
ufw_default_is() {
  grep -Eqx "DEFAULT_$1_POLICY=\"?$2\"?[[:space:]]*" /etc/default/ufw
}

# ufw loads a sysctl file of its own at the end of every start and the packaged one sets
# rp_filter=1 (strict) for all/default. Left alone, it undoes the rp_filter=2 of
# files/sysctl-betula.conf on the first reboot: containers on several networks lose packets and
# 90-verify-host.sh fails. Only the rp_filter lines are rewritten; the rest of ufw's file agrees
# with ours. (/etc/ufw/sysctl.conf is a dpkg conffile: --force-confold keeps the edit.)
align_ufw_sysctl() {
  local file conflicts
  file="$(ufw_sysctl_file)"
  if [[ -z "${file}" || ! -f "${file}" ]]; then
    log "ufw applies no sysctl file of its own (IPT_SYSCTL='${file}')"
    return 0
  fi
  conflicts="$(ufw_rp_filter_conflicts "${file}")"
  if [[ -z "${conflicts}" ]]; then
    log "${file}: rp_filter already agrees with /etc/sysctl.d/99-zz-betula.conf"
    return 0
  fi
  backup_file "${file}"
  sed -E -i "s|(${UFW_RP_FILTER_KEY_RE})[[:space:]]*=.*\$|\\1=2|" "${file}"
  conflicts="$(ufw_rp_filter_conflicts "${file}")"
  [[ -z "${conflicts}" ]] || die "${file} still sets a strict rp_filter: ${conflicts//$'\n'/; }"
  log "${file}: rp_filter lines set to 2 (loose), so a ufw start no longer overrides sysctl.d"
}

configure_firewall() {
  step "Firewall (ufw): deny incoming, allow outgoing, allow ssh + public ports"
  local port added ipv6="yes" status was_active=0 reload_needed=0
  local -a ssh_ports=("${SSH_PORT}")

  if contains_line "$(ufw status 2>/dev/null || true)" '^Status: active'; then was_active=1; fi

  load_public_ports "${BETULA_FILES_DIR}/public-ports.conf"
  install_file "${BETULA_FILES_DIR}/public-ports.conf" "${BETULA_ETC_DIR}/public-ports.conf" 0644

  # Never cut the branch we sit on: besides the contract port, allow whatever port sshd really
  # uses and the port of the session that runs this script (they are all 22 on a stock server).
  while IFS= read -r port; do
    if [[ "${port}" =~ ^[0-9]+$ ]]; then
      ssh_ports+=("${port}")
    fi
  done < <(sshd -T 2>/dev/null | awk '$1 == "port" { print $2 }')
  if [[ -n "${SSH_CONNECTION:-}" ]]; then
    read -r _ _ _ port <<<"${SSH_CONNECTION}"
    if [[ "${port}" =~ ^[0-9]+$ ]]; then
      ssh_ports+=("${port}")
    fi
  fi
  mapfile -t ssh_ports < <(printf '%s\n' "${ssh_ports[@]}" | sort -un)

  # The host has a global IPv6 address (no AAAA record, but reachable all the same): every rule
  # below must exist for both families. ufw with IPV6=yes cannot start on a kernel without IPv6
  # (some provider images disable it), hence the probe.
  ipv6_enabled || ipv6="no"
  if [[ "${ipv6}" == "no" ]]; then
    warn "IPv6 is disabled in the kernel; ufw is configured for IPv4 only"
  fi
  if ! grep -Eqx "IPV6=\"?${ipv6}\"?[[:space:]]*" /etc/default/ufw; then
    backup_file /etc/default/ufw
    if grep -q '^IPV6=' /etc/default/ufw; then
      sed -i "s/^IPV6=.*/IPV6=${ipv6}/" /etc/default/ufw
    else
      printf 'IPV6=%s\n' "${ipv6}" >>/etc/default/ufw
    fi
    log "/etc/default/ufw: IPV6=${ipv6}"
    # A running ufw only builds (or drops) its ip6tables half on a restart.
    reload_needed=1
  fi

  align_ufw_sysctl

  # "ufw default ..." stops and starts the whole firewall when ufw is enabled: for a moment the
  # INPUT policy is ACCEPT and the swarm ports (2377, 7946, VXLAN 4789/udp) are open to the
  # internet. A re-run must therefore not call it when the value is already there. While ufw
  # is still inactive (first run) the command only edits /etc/default/ufw.
  if ufw_default_is INPUT DROP; then
    log "default incoming: deny (unchanged)"
  else
    ufw default deny incoming >/dev/null
    log "default incoming: deny"
  fi
  if ufw_default_is OUTPUT ACCEPT; then
    log "default outgoing: allow (unchanged)"
  else
    ufw default allow outgoing >/dev/null
    log "default outgoing: allow"
  fi
  # Routed traffic: DROP. Docker inserts its own FORWARD rules in front, and 30-docker.sh
  # filters those (published container ports never pass ufw's INPUT rules).
  if ufw_default_is FORWARD DROP; then
    log "default routed: deny (unchanged)"
  else
    ufw default deny routed >/dev/null
    log "default routed: deny"
  fi

  # Plain allow, deliberately not "ufw limit": the operator's automation opens many short
  # sessions and limit (6 connections / 30 s) would throttle exactly that. Brute force is handled
  # by key-only authentication, fail2ban and sshd's PerSourcePenalties.
  for port in "${ssh_ports[@]}"; do
    ufw allow "${port}/tcp" comment "ssh" >/dev/null
  done
  for port in ${PUBLIC_TCP_PORTS}; do
    ufw allow "${port}/tcp" comment "betula public (traefik)" >/dev/null
  done
  for port in ${PUBLIC_UDP_PORTS}; do
    ufw allow "${port}/udp" comment "betula public (traefik http/3)" >/dev/null
  done
  # Blocked-packet logging is background noise from the whole internet; it would fill the capped
  # journal and Loki with lines nobody acts on. Switch it on temporarily when debugging.
  if ! grep -Eqx 'LOGLEVEL="?off"?[[:space:]]*' /etc/ufw/ufw.conf 2>/dev/null; then
    ufw logging off >/dev/null
    log "ufw logging: off"
  fi

  # Last look before the firewall goes live: the ssh rules must be in the rule set.
  added="$(ufw show added)"
  for port in "${ssh_ports[@]}"; do
    contains_line "${added}" "^ufw allow ${port}/tcp( |\$)" ||
      die "ufw rule for ssh port ${port} is missing; refusing to enable the firewall"
  done

  # --force only skips the "may disrupt existing ssh connections" question. Established
  # connections survive: the ssh port is allowed without a state match and ufw accepts
  # ESTABLISHED traffic before anything else.
  # On a re-run this is a no-op: ufw-init returns early when its chains are already loaded.
  ufw --force enable >/dev/null
  unit_enabled ufw.service || systemctl enable ufw.service
  if [[ "${was_active}" -eq 1 && "${reload_needed}" -eq 1 ]]; then
    log "IPV6 setting changed while ufw was running: one reload"
    ufw reload >/dev/null
  fi
  status="$(ufw status verbose)"
  if [[ "${ipv6}" == "yes" ]]; then
    # The host is reachable over IPv6; an IPv4-only rule set would leave that side to chance.
    # Only a warning: nothing is exposed by a missing v6 allow rule, and 90-verify-host.sh FAILs on it.
    for port in "${ssh_ports[@]}"; do
      contains_line "${status}" "^${port}/tcp \\(v6\\) +ALLOW IN" ||
        warn "ufw shows no IPv6 rule for ssh port ${port}; check IPV6= in /etc/default/ufw"
    done
  fi
  log "ufw is active:"
  printf '%s\n' "${status}"
}

configure_fail2ban() {
  step "fail2ban: sshd jail (journal backend, nftables bans, incremental ban time)"
  local changed=0 i jail_up=0
  install_file "${BETULA_FILES_DIR}/fail2ban-jail-betula.local" /etc/fail2ban/jail.d/betula.local 0644
  changed=$((changed + INSTALL_CHANGED))
  install_file "${BETULA_FILES_DIR}/fail2ban-filter-sshd.local" /etc/fail2ban/filter.d/sshd.local 0644
  changed=$((changed + INSTALL_CHANGED))
  install_file "${BETULA_FILES_DIR}/fail2ban-fail2ban-betula.local" /etc/fail2ban/fail2ban.d/betula.local 0644
  changed=$((changed + INSTALL_CHANGED))

  fail2ban-client -t >/dev/null || die "fail2ban configuration test failed (fail2ban-client -t); the running instance was not touched"

  systemctl enable fail2ban.service >/dev/null 2>&1
  if [[ "${changed}" -gt 0 ]] || ! unit_active fail2ban.service; then
    systemctl restart fail2ban.service
  fi
  # Wait for the JAIL, not for "ping": the unit counts as started and the server answers ping
  # as soon as its socket is up, while the jails are still being created in the background.
  for i in $(seq 1 30); do
    if fail2ban-client status sshd >/dev/null 2>&1; then
      jail_up=1
      break
    fi
    sleep 1
  done
  [[ "${jail_up}" -eq 1 ]] || die "fail2ban runs, but the sshd jail did not start within 30 s (journalctl -u fail2ban)"
  log "fail2ban sshd jail is running (attempt ${i})"
}

configure_unattended_upgrades() {
  step "unattended-upgrades: security updates, reboot at 04:30, unused dependencies removed"
  install_file "${BETULA_FILES_DIR}/apt-20auto-upgrades" /etc/apt/apt.conf.d/20auto-upgrades 0644
  install_file "${BETULA_FILES_DIR}/apt-52-betula-unattended-upgrades" /etc/apt/apt.conf.d/52-betula-unattended-upgrades 0644
  # A syntax error in apt.conf.d breaks every apt call, including the automatic ones.
  apt-config dump >/dev/null || die "apt configuration does not parse after installing our files"
  systemctl enable --now apt-daily.timer apt-daily-upgrade.timer >/dev/null 2>&1
  if unit_exists unattended-upgrades.service; then
    systemctl enable unattended-upgrades.service >/dev/null 2>&1
  fi
}

configure_journald() {
  step "journald: persistent, capped at 500 MB"
  install_file "${BETULA_FILES_DIR}/journald-betula.conf" /etc/systemd/journald.conf.d/50-betula.conf 0644
  if [[ "${INSTALL_CHANGED}" -eq 1 ]]; then
    systemctl restart systemd-journald.service
  fi
}

configure_sysctl() {
  step "Kernel parameters (hardening that keeps Docker Swarm networking intact)"
  # Must run after configure_firewall: the file turns ip_forward on, and the FORWARD policy has
  # to be DROP (ufw) by then, or the host would route for its neighbours for a moment.
  # ufw re-applies its own sysctl file on every start; align_ufw_sysctl made that file agree
  # with this one, so the values below also hold after a reboot or a "ufw reload".
  install_file "${BETULA_FILES_DIR}/sysctl-betula.conf" /etc/sysctl.d/99-zz-betula.conf 0644
  if sysctl -p /etc/sysctl.d/99-zz-betula.conf >/dev/null; then
    log "sysctl values applied"
  elif is_container; then
    warn "some sysctl values could not be set (container guest, /proc/sys is read-only)"
  else
    die "sysctl -p /etc/sysctl.d/99-zz-betula.conf failed"
  fi
}

SWAPFILE="/swapfile"
SWAPFILE_UNFINISHED=0

# Exit hook: a swap file that this run created but could not activate must not stay behind.
cleanup_swapfile() {
  [[ "${SWAPFILE_UNFINISHED}" -eq 1 ]] || return 0
  rm -f -- "${SWAPFILE}"
  warn "removed the unfinished ${SWAPFILE}"
}
add_exit_hook cleanup_swapfile

# The fstab line is ensured on its own, not as the tail of the creation path: a run that died
# between swapon and this line would otherwise lose its swap at the next reboot, for good.
ensure_swapfile_in_fstab() {
  if grep -Eq "^[[:space:]]*${SWAPFILE}[[:space:]]+[^[:space:]]+[[:space:]]+swap([[:space:]]|\$)" /etc/fstab; then
    log "/etc/fstab already lists ${SWAPFILE}"
    return 0
  fi
  backup_file /etc/fstab
  # A last line without newline would be glued to ours.
  if [[ -s /etc/fstab && -n "$(tail -c 1 /etc/fstab)" ]]; then
    printf '\n' >>/etc/fstab
  fi
  printf '%s none swap sw 0 0\n' "${SWAPFILE}" >>/etc/fstab
  log "/etc/fstab: ${SWAPFILE} added"
}

configure_swap() {
  step "Swap (only when there is none)"
  local active ram_mb size_mb free_mb fstype fstab_swap
  if is_container; then
    log "container guest: swap belongs to the host, skipping"
    return 0
  fi
  active="$(swapon --noheadings --show=NAME 2>/dev/null || true)"
  if [[ -n "${active}" ]]; then
    log "swap already active: ${active//$'\n'/ }"
    if grep -qxF -- "${SWAPFILE}" <<<"${active}"; then
      ensure_swapfile_in_fstab
    fi
    return 0
  fi
  fstab_swap="$(awk -v ours="${SWAPFILE}" '$1 !~ /^#/ && $3 == "swap" && $1 != ours { print $1 }' /etc/fstab)"
  if [[ -n "${fstab_swap}" ]]; then
    warn "fstab lists swap (${fstab_swap//$'\n'/ }) but none is active; not adding another one (try: swapon -a)"
    return 0
  fi
  if [[ -e "${SWAPFILE}" || -L "${SWAPFILE}" ]]; then
    # Left over from an earlier run. A finished one (swap signature) is simply switched on, e.g.
    # after a reboot that came before the fstab line; anything else is an interrupted creation.
    if [[ -f "${SWAPFILE}" && ! -L "${SWAPFILE}" &&
      "$(blkid -p -o value -s TYPE -- "${SWAPFILE}" 2>/dev/null || true)" == "swap" ]]; then
      chown root:root "${SWAPFILE}"
      chmod 0600 "${SWAPFILE}"
      if swapon "${SWAPFILE}" 2>/dev/null; then
        log "existing ${SWAPFILE} switched on"
        ensure_swapfile_in_fstab
        return 0
      fi
      warn "${SWAPFILE} has a swap signature but swapon refuses it; creating a new one"
    else
      warn "${SWAPFILE} exists without a swap signature (interrupted run); creating a new one"
    fi
    rm -f -- "${SWAPFILE}"
  fi
  fstype="$(findmnt --noheadings --output FSTYPE /)"
  case "${fstype}" in
    ext4 | xfs) ;;
    *)
      # btrfs and zfs need special swap files; guessing wrong there corrupts nothing but fails oddly.
      warn "root file system is ${fstype}; create swap by hand"
      return 0
      ;;
  esac

  # RAM <= 2 GB -> 2 GB, up to 4 GB -> same as RAM, above -> 4 GB. Swap is a safety net against
  # the OOM killer (monitoring stack + scraper build; 5.9 GB RAM and no swap when this server
  # was measured, which makes it 4 GB here), not working memory.
  ram_mb="$(mem_total_mb)"
  size_mb="${ram_mb}"
  if [[ "${size_mb}" -lt 2048 ]]; then size_mb=2048; fi
  if [[ "${size_mb}" -gt 4096 ]]; then size_mb=4096; fi
  size_mb=$((((size_mb + 255) / 256) * 256))
  # Never take more than a quarter of the free disk. (df -Pk is plain POSIX output.)
  free_mb="$(df -Pk / | awk 'NR == 2 { print int($4 / 1024) }')"
  if [[ "${size_mb}" -gt $((free_mb / 4)) ]]; then
    size_mb=$((((free_mb / 4) / 256) * 256))
  fi
  if [[ "${size_mb}" -lt 512 ]]; then
    warn "only ${free_mb} MB free on /; no swap file created"
    return 0
  fi

  log "RAM ${ram_mb} MB, free disk ${free_mb} MB -> ${size_mb} MB swap file"
  # From here until swapon succeeds, a failure (full disk, mkswap, swapon) removes the file again
  # through the exit hook instead of leaving gigabytes of dead weight behind.
  SWAPFILE_UNFINISHED=1
  # Created with mode 0600 from the first byte on: swap holds memory pages.
  (
    umask 077
    : >"${SWAPFILE}"
  )
  if ! fallocate -l "${size_mb}M" "${SWAPFILE}" 2>/dev/null; then
    dd if=/dev/zero of="${SWAPFILE}" bs=1M count="${size_mb}" status=none
  fi
  mkswap "${SWAPFILE}" >/dev/null
  if ! swapon "${SWAPFILE}" 2>/dev/null; then
    # Some file systems refuse preallocated (unwritten) extents for swap; write real zeroes.
    log "swapon refused the preallocated file; rewriting it with dd"
    dd if=/dev/zero of="${SWAPFILE}" bs=1M count="${size_mb}" status=none
    chmod 0600 "${SWAPFILE}"
    mkswap "${SWAPFILE}" >/dev/null
    swapon "${SWAPFILE}"
  fi
  SWAPFILE_UNFINISHED=0
  ensure_swapfile_in_fstab
  log "swap file active (vm.swappiness = 10 comes from the sysctl file)"
}

# True when a snap other than snapd's own runtime (snapd, core*, bare) is installed.
snaps_in_use() {
  local list
  have_cmd snap || return 1
  # No answer within 30 s (wedged daemon) counts as "no snaps": nothing of ours depends on it.
  list="$(timeout 30 snap list 2>/dev/null || true)"
  awk 'NR > 1 && $1 !~ /^(snapd|bare|core[0-9]*)$/ { found = 1 } END { exit !found }' <<<"${list}"
}

disable_unneeded_units() {
  step "Disable what this server does not need (only if present)"
  local unit state found=0 snaps=""
  for unit in "${BETULA_UNNEEDED_UNITS[@]}"; do
    unit_exists "${unit}" || continue
    # "static" and "indirect" units have nothing to disable; they only matter while they run.
    state="$(systemctl is-enabled "${unit}" 2>/dev/null || true)"
    if [[ "${state}" != enabled* ]] && ! unit_active "${unit}"; then
      continue
    fi
    case "${unit}" in
      snapd.*)
        # Asked once and before the first snapd unit goes down: "snap list" needs the socket.
        if [[ -z "${snaps}" ]]; then
          if snaps_in_use; then snaps="yes"; else snaps="no"; fi
        fi
        if [[ "${snaps}" == "yes" ]]; then
          warn "snaps are installed (snap list); leaving ${unit} alone"
          continue
        fi
        ;;
      multipathd.*)
        if [[ -n "$(lsblk --noheadings --output TYPE 2>/dev/null | grep -x mpath || true)" ]]; then
          warn "multipath devices in use; leaving ${unit} alone"
          continue
        fi
        ;;
      iscsid.* | open-iscsi.*)
        if [[ -d /sys/class/iscsi_session && -n "$(ls -A /sys/class/iscsi_session 2>/dev/null)" ]]; then
          warn "active iSCSI sessions; leaving ${unit} alone"
          continue
        fi
        ;;
    esac
    systemctl disable --now "${unit}" >/dev/null 2>&1 || warn "could not disable ${unit}"
    log "disabled: ${unit}"
    found=$((found + 1))
  done
  [[ "${found}" -gt 0 ]] || log "nothing to disable"
}

prepare_directories() {
  step "Directories"
  # /opt/betula receives deploy/ verbatim (root:root); /etc/betula holds host-level settings.
  install -d -m 0755 -o root -g root /opt/betula "${BETULA_ETC_DIR}"
  log "/opt/betula and ${BETULA_ETC_DIR} exist"
}

report() {
  step "Done"
  log "next: from your workstation run  ssh ${DEPLOY_USER}@<host> sudo -n true  (or your ssh alias for ${DEPLOY_USER})"
  log "      only when that works: sudo bash /opt/betula/vps/20-ssh-lockdown.sh"
  log "      and from a NEW connection: sudo bash /opt/betula/vps/20-ssh-lockdown.sh --confirm"
  log "      then: sudo bash /opt/betula/vps/30-docker.sh and sudo bash /opt/betula/vps/90-verify-host.sh"
  log "      last, as ${DEPLOY_USER} without sudo: bash /opt/betula/vps/40-stacks.sh and bash /opt/betula/vps/91-verify-stacks.sh"
  log "      (the whole order, with the reboot, is in /opt/betula/README.md)"
  log "backups of replaced files (if any): ${BETULA_BACKUP_ROOT}/${BETULA_RUN_ID}"
  if [[ -e /var/run/reboot-required ]]; then
    if [[ -r /var/run/reboot-required.pkgs ]]; then
      log "reboot requested by: $(sort -u /var/run/reboot-required.pkgs | tr '\n' ' ')"
    fi
    # Machine-readable marker for the operator's automation; keep it the last line.
    printf 'REBOOT_REQUIRED\n'
  fi
}

# ---------------------------------------------------------------- main

upgrade_system
install_base_packages
configure_needrestart
configure_time
prepare_directories
create_deploy_user
disable_unused_accounts
configure_firewall
configure_fail2ban
configure_unattended_upgrades
configure_journald
configure_sysctl
configure_swap
disable_unneeded_units
report
