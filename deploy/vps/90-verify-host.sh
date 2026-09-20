#!/usr/bin/env bash
# 90-verify-host.sh - read-only audit of what 10-base, 20-ssh-lockdown and 30-docker set up.
#
#   sudo bash /opt/betula/vps/90-verify-host.sh              # everything
#   sudo bash /opt/betula/vps/90-verify-host.sh ssh ufw      # only some sections
#
# Sections: os user ssh ufw fail2ban updates journald sysctl swap time services docker listeners reboot
# Prints one PASS / WARN / FAIL line per check and exits non-zero when anything FAILed.
# Changes nothing: no files, no services, no firewall rules. Between the numbered scripts a
# FAIL can simply mean "not done yet" (ssh before 20-ssh-lockdown.sh, docker before 30-docker.sh).
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
betula_init
require_root
require_ubuntu

DOCKER_MAJOR=29
ALL_SECTIONS=(os user ssh ufw fail2ban updates journald sysctl swap time services docker listeners reboot)
PASSED=0
WARNED=0
FAILED=0
# Results of check_published_filter (one call per address family).
FILTER_OK=1
FILTER_PORTS=""

pass() {
  PASSED=$((PASSED + 1))
  printf 'PASS  %s\n' "$*"
}
warning() {
  WARNED=$((WARNED + 1))
  printf 'WARN  %s\n' "$*"
}
fail() {
  FAILED=$((FAILED + 1))
  printf 'FAIL  %s\n' "$*"
}
section() { printf '\n-- %s\n' "$*"; }

# expect LABEL ACTUAL WANTED
expect() {
  if [[ "$2" == "$3" ]]; then
    pass "$1 = $3"
  else
    fail "$1 = '${2:-<unset>}' (wanted '$3')"
  fi
}

# as_shipped LABEL INSTALLED_PATH PAYLOAD_NAME - the installed file is the one from this repo.
as_shipped() {
  if [[ ! -f "$2" ]]; then
    fail "$1: $2 is missing"
  elif cmp -s -- "$2" "${BETULA_FILES_DIR}/$3"; then
    pass "$1: $2 as shipped"
  else
    fail "$1: $2 differs from files/$3 (edited on the server, or deploy/ is newer: re-run the script)"
  fi
}

docker_usable() { have_cmd docker && unit_active docker.service; }

# One "port/proto" per line for everything files/public-ports.conf declares public.
public_port_list() {
  local p
  for p in ${PUBLIC_TCP_PORTS}; do printf '%s/tcp\n' "${p}"; done
  for p in ${PUBLIC_UDP_PORTS}; do printf '%s/udp\n' "${p}"; done
}

# is_public PORT/PROTO
is_public() {
  local list
  list="$(public_port_list)"
  grep -qxF -- "$1" <<<"${list}"
}

# ---------------------------------------------------------------- sections

check_os() {
  section "operating system"
  pass "Ubuntu ${OS_VERSION_ID} (${OS_CODENAME}), kernel $(uname -r)"
  if is_container; then
    warning "container guest: swap, sysctl and clock checks may not apply"
  fi
}

check_user() {
  section "admin user"
  local home keys groups state perm owner
  if ! id -u "${DEPLOY_USER}" >/dev/null 2>&1; then
    fail "user ${DEPLOY_USER} does not exist"
    return 0
  fi
  pass "user ${DEPLOY_USER} exists"
  groups=" $(id -nG "${DEPLOY_USER}") "
  if [[ "${groups}" == *" sudo "* ]]; then pass "${DEPLOY_USER} is in group sudo"; else fail "${DEPLOY_USER} is not in group sudo"; fi
  if getent group docker >/dev/null; then
    if [[ "${groups}" == *" docker "* ]]; then pass "${DEPLOY_USER} is in group docker"; else fail "${DEPLOY_USER} is not in group docker"; fi
  fi
  state="$(passwd --status "${DEPLOY_USER}" | awk '{ print $2 }')"
  expect "password state of ${DEPLOY_USER} (L = locked)" "${state}" "L"

  home="$(getent passwd "${DEPLOY_USER}" | cut -d: -f6)"
  keys="${home}/.ssh/authorized_keys"
  if [[ -s "${keys}" ]] && ssh-keygen -l -f "${keys}" >/dev/null 2>&1; then
    owner="$(stat -c %U "${keys}")"
    perm="$(stat -c %a "${keys}")"
    if [[ "${owner}" == "${DEPLOY_USER}" && ("${perm}" == "600" || "${perm}" == "400") ]]; then
      pass "authorized_keys: $(ssh-keygen -l -f "${keys}" | wc -l) key(s), ${owner} ${perm}"
    else
      fail "authorized_keys: owner ${owner}, mode ${perm} (wanted ${DEPLOY_USER}, 600)"
    fi
  else
    fail "${keys} is missing, empty or holds no valid key"
  fi

  as_shipped "sudoers" /etc/sudoers.d/90-betula-deploy sudoers-deploy
  if visudo -c >/dev/null 2>&1; then pass "visudo -c: sudoers configuration is valid"; else fail "visudo -c reports errors"; fi
  if runuser -u "${DEPLOY_USER}" -- sudo -n true 2>/dev/null; then
    pass "passwordless sudo works for ${DEPLOY_USER}"
  else
    fail "'sudo -n true' fails for ${DEPLOY_USER}"
  fi
  check_other_accounts
}

# Nobody but deploy (and root on the provider's console) may be able to log in or become root.
# The cloud image's "ubuntu" user is the reason this exists: NOPASSWD sudo and group lxd.
check_other_accounts() {
  local name _pw uid _gid _gecos home shell pw group member file line who bad=0
  while IFS=: read -r name _pw uid _gid _gecos home shell; do
    [[ "${name}" == "root" || "${name}" == "${DEPLOY_USER}" ]] && continue
    case "${shell}" in
      */nologin | */false | */sync) continue ;;
    esac
    # A login shell (an empty field means /bin/sh). Dangerous only with a way to authenticate.
    pw="$(getent shadow "${name}" | cut -d: -f2 || true)"
    if [[ -z "${pw}" || ("${pw}" != "!"* && "${pw}" != "*"*) ]]; then
      fail "account ${name} (uid ${uid}) has a login shell and a usable password"
      bad=1
    elif [[ -s "${home}/.ssh/authorized_keys" ]]; then
      fail "account ${name} (uid ${uid}) has a login shell and authorized_keys"
      bad=1
    else
      warning "account ${name} (uid ${uid}) has a login shell (password locked, no keys); 10-base.sh disables unused accounts"
      bad=1
    fi
  done </etc/passwd
  [[ "${bad}" -eq 1 ]] || pass "no account besides root and ${DEPLOY_USER} has a login shell"

  bad=0
  for group in sudo admin docker lxd; do
    line="$(getent group "${group}" | cut -d: -f4 || true)"
    for member in ${line//,/ }; do
      if [[ "${member}" != "${DEPLOY_USER}" ]]; then
        fail "${member} is a member of group ${group} (root-equivalent)"
        bad=1
      fi
    done
  done
  [[ "${bad}" -eq 1 ]] || pass "groups sudo, admin, docker and lxd have no member besides ${DEPLOY_USER}"

  bad=0
  for file in /etc/sudoers /etc/sudoers.d/*; do
    [[ -f "${file}" ]] || continue
    # sudo skips names with a dot or a trailing tilde in sudoers.d.
    if [[ "${file}" == /etc/sudoers.d/* && ("${file##*/}" == *.* || "${file}" == *~) ]]; then continue; fi
    while IFS= read -r line; do
      who="$(awk '{ print $1 }' <<<"${line}")"
      case "${who}" in
        "${DEPLOY_USER}" | root | Defaults* | *_Alias | @include* | "#include"*) continue ;;
        %sudo | %admin)
          # The stock group rules ask for a password (which deploy does not have); NOPASSWD
          # for a whole group would hand root to every future member.
          [[ "${line}" == *NOPASSWD* ]] || continue
          ;;
      esac
      if [[ "${line}" == *NOPASSWD* ]]; then
        fail "${file} grants passwordless sudo to ${who}"
      else
        fail "${file} has a sudo rule for ${who}"
      fi
      bad=1
    done < <(sudoers_rules "${file}")
  done
  [[ "${bad}" -eq 1 ]] || pass "sudoers: no rule for anybody besides root, %sudo, %admin and ${DEPLOY_USER}"
}

check_ssh() {
  section "sshd (effective values from sshd -T)"
  local cfg user want key value first
  local -a expected=(
    "permitrootlogin no" "pubkeyauthentication yes" "passwordauthentication no"
    "kbdinteractiveauthentication no" "permitemptypasswords no"
    "authenticationmethods publickey" "allowusers ${DEPLOY_USER}"
    "maxauthtries 3" "logingracetime 30" "x11forwarding no" "allowagentforwarding no"
    "allowtcpforwarding local" "allowstreamlocalforwarding no" "gatewayports no"
    "permittunnel no" "port ${SSH_PORT}"
  )
  if ! have_cmd sshd; then
    fail "sshd is not installed"
    return 0
  fi
  if [[ ! -d /run/sshd ]]; then
    warning "/run/sshd does not exist (sshd not started since boot); sshd -T may refuse to run"
  fi
  for user in "${DEPLOY_USER}" root; do
    if ! cfg="$(sshd -T -C "user=${user},host=client.invalid,addr=203.0.113.10" 2>&1)"; then
      fail "sshd -T failed for user ${user}: $(sed -n '1p' <<<"${cfg}")"
      continue
    fi
    for want in "${expected[@]}"; do
      key="${want%% *}"
      value="$(awk -v k="${key}" '$1 == k { $1 = ""; sub(/^ /, ""); print }' <<<"${cfg}" | sort -u | tr '\n' ' ')"
      expect "sshd (${user}) ${key}" "${value% }" "${want#* }"
    done
  done
  cfg="$(sshd -T 2>/dev/null || true)"
  if contains_line "${cfg}" '^persourcepenalties '; then
    as_shipped "sshd penalties" /etc/ssh/sshd_config.d/01-betula-penalties.conf sshd-01-betula-penalties.conf
    value="$(awk '$1 == "persourcepenalties" { $1 = ""; sub(/^ /, ""); print }' <<<"${cfg}")"
    if [[ "${value}" == *"authfail:10"* ]]; then pass "persourcepenalties effective: ${value}"; else fail "persourcepenalties not ours: ${value}"; fi
  elif [[ -n "${cfg}" ]]; then
    warning "this sshd has no PerSourcePenalties (OpenSSH < 9.8); fail2ban is the only rate limiter"
  fi
  as_shipped "sshd hardening" /etc/ssh/sshd_config.d/00-betula-hardening.conf sshd-00-betula-hardening.conf
  first="$(find /etc/ssh/sshd_config.d -maxdepth 1 -name '*.conf' -printf '%f\n' 2>/dev/null | sort | sed -n '1p' || true)"
  if [[ "${first}" == "00-betula-hardening.conf" ]]; then
    pass "00-betula-hardening.conf is the first drop-in sshd reads"
  elif [[ -n "${first}" ]]; then
    warning "'${first}' sorts before 00-betula-hardening.conf (first value wins in sshd_config)"
  fi
  # The lockdown is only final once a fresh login has confirmed it.
  if [[ -f /usr/local/sbin/betula-ssh-rollback ]]; then
    as_shipped "rollback tool" /usr/local/sbin/betula-ssh-rollback betula-ssh-rollback.sh
  else
    warning "/usr/local/sbin/betula-ssh-rollback is missing (20-ssh-lockdown.sh not run yet, or run with LOCKDOWN_ROLLBACK_MINUTES=0)"
  fi
  if unit_active betula-ssh-rollback.timer; then
    warning "the ssh lockdown is NOT confirmed: betula-ssh-rollback.timer will undo it (run 20-ssh-lockdown.sh --confirm from a new login)"
  else
    pass "no automatic ssh rollback is pending"
  fi
}

check_ufw() {
  section "ufw"
  local status port line extra ipv6 ipt
  if ! have_cmd ufw; then
    fail "ufw is not installed"
    return 0
  fi
  load_public_ports "${BETULA_FILES_DIR}/public-ports.conf"
  status="$(ufw status verbose 2>&1 || true)"
  if contains_line "${status}" '^Status: active'; then pass "ufw is active"; else
    fail "ufw is not active"
    return 0
  fi
  if unit_enabled ufw.service; then pass "ufw.service is enabled (rules load at boot)"; else fail "ufw.service is not enabled"; fi
  if contains_line "${status}" '^Default: deny \(incoming\), allow \(outgoing\)'; then
    pass "defaults: deny incoming, allow outgoing"
  else
    fail "defaults are not 'deny (incoming), allow (outgoing)': $(grep '^Default:' <<<"${status}" || true)"
  fi
  if contains_line "${status}" '^Default: .*(deny|disabled|reject) \(routed\)'; then
    pass "routed traffic is not allowed by ufw"
  else
    fail "ufw allows routed traffic by default"
  fi

  if contains_line "${status}" "^${SSH_PORT}/tcp +LIMIT"; then
    fail "${SSH_PORT}/tcp is LIMIT; the contract wants a plain allow (automation opens many sessions)"
  fi
  # The host has a global IPv6 address: every rule has to exist for both families, a v4 match
  # alone proves nothing about the v6 side.
  ipv6="$(grep -E '^IPV6=' /etc/default/ufw | cut -d= -f2 | tr -d '"' || true)"
  for port in "${SSH_PORT}/tcp" $(public_port_list); do
    if contains_line "${status}" "^${port} +ALLOW IN +Anywhere"; then
      pass "allow ${port} from anywhere (IPv4)"
    else
      fail "no IPv4 'allow ${port}' rule"
    fi
    if ipv6_enabled; then
      if contains_line "${status}" "^${port} \\(v6\\) +ALLOW IN +Anywhere \\(v6\\)"; then
        pass "allow ${port} from anywhere (IPv6)"
      else
        fail "no IPv6 'allow ${port}' rule (IPV6=${ipv6:-?} in /etc/default/ufw)"
      fi
    fi
  done
  # What the kernel really has, per family: ufw's word for it is not enough on the v6 side.
  for ipt in iptables ip6tables; do
    if [[ "${ipt}" == "ip6tables" ]] && ! ipv6_enabled; then continue; fi
    if ! have_cmd "${ipt}"; then
      fail "${ipt} is not installed; cannot check the INPUT policy"
    elif contains_line "$("${ipt}" -w 10 -S INPUT 2>/dev/null || true)" '^-P INPUT DROP$'; then
      pass "${ipt}: INPUT policy is DROP"
    else
      fail "${ipt}: INPUT policy is not DROP (unfiltered address family)"
    fi
  done

  # Anything else that lets traffic in is not part of the contract.
  extra=""
  while IFS= read -r line; do
    [[ "${line}" =~ [[:space:]](ALLOW|LIMIT)([[:space:]]IN)?[[:space:]] ]] || continue
    port="${line%% *}"
    if [[ "${port}" != "${SSH_PORT}/tcp" ]] && ! is_public "${port}"; then
      extra+="${line}"$'\n'
    fi
  done <<<"${status}"
  if [[ -n "${extra}" ]]; then
    while IFS= read -r line; do
      [[ -n "${line}" ]] && warning "additional ufw rule: ${line}"
    done <<<"${extra}"
  else
    pass "no allow rules beyond ssh and the public ports"
  fi

  if ipv6_enabled; then
    expect "ufw IPV6" "${ipv6}" "yes"
  else
    warning "IPv6 is disabled in the kernel (ufw IPV6=${ipv6})"
  fi
}

check_fail2ban() {
  section "fail2ban"
  local status actions bantime
  if ! have_cmd fail2ban-client; then
    fail "fail2ban is not installed"
    return 0
  fi
  if unit_active fail2ban.service; then pass "fail2ban.service is active"; else
    fail "fail2ban.service is not active"
    return 0
  fi
  if unit_enabled fail2ban.service; then pass "fail2ban.service is enabled"; else fail "fail2ban.service is not enabled"; fi
  as_shipped "jail" /etc/fail2ban/jail.d/betula.local fail2ban-jail-betula.local
  as_shipped "filter override" /etc/fail2ban/filter.d/sshd.local fail2ban-filter-sshd.local
  as_shipped "ban history" /etc/fail2ban/fail2ban.d/betula.local fail2ban-fail2ban-betula.local
  if ! status="$(fail2ban-client status sshd 2>&1)"; then
    fail "sshd jail is not running: $(sed -n '1p' <<<"${status}")"
    return 0
  fi
  pass "sshd jail is running ($(grep -E 'Currently banned|Total banned' <<<"${status}" | sed 's/^[^A-Za-z]*//' | tr '\t\n' '  '))"
  if contains_line "${status}" 'Journal matches'; then
    pass "sshd jail reads the systemd journal"
  else
    fail "sshd jail does not use the systemd backend (no journal matches in its status)"
  fi
  actions="$(fail2ban-client get sshd actions 2>/dev/null || true)"
  if [[ "${actions}" == *nftables* ]]; then
    pass "ban action: nftables (own table, independent of ufw and Docker rules)"
  else
    fail "ban action is not nftables: ${actions//$'\n'/ }"
  fi
  bantime="$(fail2ban-client get sshd bantime 2>/dev/null || true)"
  expect "base ban time (seconds)" "${bantime}" "3600"
  if have_cmd nft; then pass "nft binary present"; else fail "nft binary missing: bans cannot be applied"; fi
}

check_updates() {
  section "unattended-upgrades"
  local conf unit
  if pkg_installed unattended-upgrades; then pass "unattended-upgrades is installed"; else
    fail "unattended-upgrades is not installed"
    return 0
  fi
  as_shipped "periodic switch" /etc/apt/apt.conf.d/20auto-upgrades apt-20auto-upgrades
  as_shipped "overrides" /etc/apt/apt.conf.d/52-betula-unattended-upgrades apt-52-betula-unattended-upgrades
  if ! conf="$(apt-config dump 2>&1)"; then
    fail "apt-config dump fails: apt configuration is broken"
    return 0
  fi
  local want
  for want in \
    'APT::Periodic::Update-Package-Lists "1";' \
    'APT::Periodic::Unattended-Upgrade "1";' \
    'Unattended-Upgrade::Automatic-Reboot "true";' \
    'Unattended-Upgrade::Automatic-Reboot-Time "04:30";' \
    'Unattended-Upgrade::Remove-Unused-Dependencies "true";'; do
    if grep -qxF -- "${want}" <<<"${conf}"; then pass "effective: ${want}"; else fail "not effective: ${want}"; fi
  done
  if contains_line "${conf}" '^Unattended-Upgrade::Allowed-Origins:: ".*-security"'; then
    pass "the security pocket is an allowed origin"
  else
    fail "no '-security' entry in Unattended-Upgrade::Allowed-Origins"
  fi
  if contains_line "${conf}" '^Unattended-Upgrade::Allowed-Origins:: ".*-updates"'; then
    warning "the -updates pocket is allowed too (the contract says security updates only)"
  fi
  for unit in apt-daily.timer apt-daily-upgrade.timer; do
    if unit_active "${unit}"; then pass "${unit} is active"; else fail "${unit} is not active"; fi
  done
  as_shipped "needrestart" /etc/needrestart/conf.d/50-betula.conf needrestart-betula.conf
}

check_journald() {
  section "journald"
  as_shipped "size cap" /etc/systemd/journald.conf.d/50-betula.conf journald-betula.conf
  if [[ -d /var/log/journal ]]; then pass "journal is persistent (/var/log/journal)"; else fail "/var/log/journal is missing"; fi
  pass "$(journalctl --disk-usage 2>/dev/null | sed -n '1p')"
}

check_sysctl() {
  section "sysctl"
  local file="${BETULA_FILES_DIR}/sysctl-betula.conf" line key want have optional ufw_file conflicts
  as_shipped "kernel parameters" /etc/sysctl.d/99-zz-betula.conf sysctl-betula.conf
  while IFS= read -r line; do
    [[ "${line}" =~ ^[[:space:]]*(#|$) ]] && continue
    optional=0
    if [[ "${line}" == -* ]]; then
      optional=1
      line="${line#-}"
    fi
    key="$(sed 's/[[:space:]]*=.*//' <<<"${line}")"
    want="$(sed 's/^[^=]*=[[:space:]]*//' <<<"${line}")"
    if ! have="$(sysctl -n "${key}" 2>/dev/null)"; then
      if [[ "${optional}" -eq 1 ]]; then
        warning "${key} does not exist on this kernel (optional)"
      else
        fail "${key} cannot be read"
      fi
      continue
    fi
    expect "${key}" "${have}" "${want}"
  done <"${file}"

  # ufw loads its own sysctl file at the end of every start and announces that it overrides
  # sysctl.d. If that file still says rp_filter=1, the values above hold only until the next
  # reboot or "ufw reload" - a FAIL here explains an rp_filter FAIL above (or predicts one).
  ufw_file="$(ufw_sysctl_file)"
  if [[ -z "${ufw_file}" || ! -f "${ufw_file}" ]]; then
    pass "ufw applies no sysctl file of its own"
  else
    conflicts="$(ufw_rp_filter_conflicts "${ufw_file}")"
    if [[ -z "${conflicts}" ]]; then
      pass "${ufw_file}: no strict rp_filter that a ufw start would re-apply"
    else
      fail "${ufw_file} sets '${conflicts//$'\n'/; }': every ufw start (boot, reload) overrides rp_filter=2; re-run 10-base.sh"
    fi
  fi

  if docker_usable; then
    have="$(sysctl -n net.bridge.bridge-nf-call-iptables 2>/dev/null || true)"
    expect "net.bridge.bridge-nf-call-iptables (set by Docker)" "${have}" "1"
  fi
}

check_swap() {
  section "swap"
  local active
  active="$(swapon --noheadings --show=NAME,SIZE 2>/dev/null || true)"
  if [[ -n "${active}" ]]; then
    pass "swap active: ${active//$'\n'/; }"
    # Active now is not enough: without the fstab line the swap file is gone after the next reboot.
    if contains_line "${active}" '^/swapfile '; then
      if grep -Eq '^[[:space:]]*/swapfile[[:space:]]+[^[:space:]]+[[:space:]]+swap([[:space:]]|$)' /etc/fstab; then
        pass "/swapfile is in /etc/fstab"
      else
        fail "/swapfile is active but not in /etc/fstab (re-run 10-base.sh)"
      fi
    fi
  elif is_container; then
    warning "no swap (container guest: managed by the host)"
  else
    fail "no swap is active"
  fi
}

check_time() {
  section "time"
  local tz synced
  tz="$(timedatectl show --property=Timezone --value 2>/dev/null || true)"
  # Host-local time is the audience's time (lib.sh): the 04:30 reboot and the timers rely on it.
  expect "timezone" "${tz}" "${HOST_TIMEZONE}"
  if unit_active chrony.service; then
    pass "time sync daemon: chrony"
  elif unit_active systemd-timesyncd.service; then
    pass "time sync daemon: systemd-timesyncd"
  elif is_container; then
    warning "no time sync daemon (container guest: the host keeps the clock)"
  else
    fail "neither chrony nor systemd-timesyncd is running"
  fi
  synced="$(timedatectl show --property=NTPSynchronized --value 2>/dev/null || true)"
  if [[ "${synced}" == "yes" ]]; then
    pass "clock is synchronised"
  else
    warning "clock is not (yet) synchronised; normal for a few minutes after boot"
  fi
}

check_services() {
  section "services this server does not need"
  local unit running=0
  for unit in "${BETULA_UNNEEDED_UNITS[@]}"; do
    unit_exists "${unit}" || continue
    if unit_active "${unit}"; then
      # 10-base.sh leaves a unit alone when it is in use (snaps installed, multipath devices).
      warning "${unit} is running (10-base.sh disables it unless it is in use)"
      running=1
    fi
  done
  [[ "${running}" -eq 1 ]] || pass "none of the unneeded units is running (ModemManager, udisks2, multipathd, snapd, ...)"
}

# check_published_filter iptables|ip6tables - sets FILTER_OK=0 when the filter is not in place.
check_published_filter() {
  local ipt=$1 forward user chain first
  forward="$("${ipt}" -w 10 -S FORWARD 2>/dev/null || true)"
  user="$("${ipt}" -w 10 -S DOCKER-USER 2>/dev/null || true)"
  chain="$("${ipt}" -w 10 -S BETULA-PUBLISHED 2>/dev/null || true)"
  if contains_line "${forward}" '^-P FORWARD DROP$'; then pass "${ipt}: FORWARD policy is DROP"; else
    fail "${ipt}: FORWARD policy is not DROP"
    FILTER_OK=0
  fi
  first="$(grep -E '^-A FORWARD ' <<<"${forward}" | sed -n '1p' || true)"
  if [[ "${first}" == "-A FORWARD -j DOCKER-USER" ]]; then
    pass "${ipt}: FORWARD jumps to DOCKER-USER first"
  elif contains_line "${forward}" '^-A FORWARD -j DOCKER-USER$'; then
    # ufw's forward chains only accept ESTABLISHED traffic and explicit "ufw route" rules, so
    # new connections still reach the filter; worth a look anyway.
    warning "${ipt}: FORWARD jumps to DOCKER-USER, but not as its first rule: ${first}"
  elif [[ "${ipt}" == "ip6tables" ]]; then
    # Docker hooks into the IPv6 FORWARD chain only when it manages IPv6 forwarding. Without the
    # jump and with policy DROP nothing is forwarded at all.
    pass "${ipt}: Docker forwards no IPv6 (no jump to DOCKER-USER; policy decides)"
  else
    fail "${ipt}: FORWARD has no jump to DOCKER-USER (dockerd not on the iptables backend?)"
    FILTER_OK=0
  fi
  if [[ "$(sed -n '2p' <<<"${user}")" == "-A DOCKER-USER -j BETULA-PUBLISHED" ]]; then
    pass "${ipt}: DOCKER-USER jumps to BETULA-PUBLISHED first"
  else
    fail "${ipt}: the first DOCKER-USER rule is not the jump to BETULA-PUBLISHED"
    FILTER_OK=0
  fi
  if [[ "$(sed -n '$p' <<<"${chain}")" == "-A BETULA-PUBLISHED -j DROP" ]]; then
    pass "${ipt}: BETULA-PUBLISHED ends with DROP"
  else
    fail "${ipt}: BETULA-PUBLISHED does not end with DROP"
    FILTER_OK=0
  fi
  FILTER_PORTS="$(grep -oE -- '-p (tcp|udp) .*--ctorigdstport [0-9]+' <<<"${chain}" | awk '{ print $NF "/" $2 }' | sort | tr '\n' ' ' || true)"
}

check_docker() {
  section "docker and swarm"
  local version value net facts ports port conf_ok=1
  if ! have_cmd docker; then
    fail "docker is not installed"
    return 0
  fi
  if unit_active docker.service; then pass "docker.service is active"; else
    fail "docker.service is not active"
    return 0
  fi
  if unit_enabled docker.service; then pass "docker.service is enabled"; else fail "docker.service is not enabled"; fi

  version="$(docker version --format '{{.Server.Version}}' 2>/dev/null || true)"
  if [[ "${version%%.*}" == "${DOCKER_MAJOR}" ]]; then
    pass "Docker Engine ${version}"
  else
    warning "Docker Engine ${version}: the firewall integration was verified against ${DOCKER_MAJOR}.x"
  fi
  as_shipped "apt pin" /etc/apt/preferences.d/betula-docker apt-preferences-docker
  value="$(apt-mark showhold 2>/dev/null | tr '\n' ' ' || true)"
  if [[ " ${value}" == *" docker-ce "* && " ${value}" == *" containerd.io "* ]]; then
    pass "docker-ce and containerd.io are on hold (no container restarts from a routine apt upgrade)"
  else
    warning "docker packages are not on hold; 'apt full-upgrade' would restart every container"
  fi

  # daemon.json: the file, and what the running daemon reports.
  as_shipped "daemon.json" /etc/docker/daemon.json docker-daemon.json
  if [[ -e "${BETULA_RUN_DIR}/docker-restart-pending" ]]; then
    warning "daemon.json is newer than the running dockerd: re-run 30-docker.sh with ALLOW_DOCKER_RESTART=1 (or wait for the next reboot)"
  fi
  expect "log driver" "$(docker info --format '{{.LoggingDriver}}')" "local"
  expect "live-restore" "$(docker info --format '{{.LiveRestoreEnabled}}')" "false"
  value="$(docker info --format '{{range .DefaultAddressPools}}{{.Base}}/{{.Size}} {{end}}')"
  expect "default address pools" "${value% }" "172.30.0.0/16/24"
  value="$(docker info --format '{{json .SecurityOptions}}')"
  if [[ "${value}" == *no-new-privileges* ]]; then pass "no-new-privileges is the default for containers"; else fail "no-new-privileges is not active"; fi
  value="$(docker info --format '{{if .FirewallBackend}}{{.FirewallBackend.Driver}}{{end}}' 2>/dev/null || true)"
  if [[ "${value}" == iptables* ]]; then
    pass "firewall backend: ${value}"
  elif [[ -z "${value}" ]]; then
    warning "this engine does not report its firewall backend"
  else
    fail "firewall backend is '${value}': DOCKER-USER (and our filter) only exist with iptables"
  fi
  value="$(docker network inspect bridge --format '{{index .Options "com.docker.network.bridge.enable_icc"}}' 2>/dev/null || true)"
  expect "icc on the default bridge" "${value}" "false"

  # swarm
  expect "swarm state" "$(docker info --format '{{.Swarm.LocalNodeState}}')" "active"
  expect "swarm manager" "$(docker info --format '{{.Swarm.ControlAvailable}}')" "true"
  expect "swarm nodes" "$(docker info --format '{{.Swarm.Nodes}}')" "1"
  value="$(docker info --format '{{.Swarm.NodeAddr}}')"
  if [[ "${value}" == "$(default_ipv4)" ]]; then
    pass "swarm advertise address ${value} is the host's default IPv4"
  else
    warning "swarm advertise address ${value} differs from the default IPv4 $(default_ipv4)"
  fi
  for net in edge monitoring; do
    facts="$(docker network inspect "${net}" --format '{{.Driver}} {{.Scope}} {{.Attachable}}' 2>/dev/null || true)"
    expect "network ${net} (driver scope attachable)" "${facts}" "overlay swarm true"
  done

  # published-port filter
  load_public_ports "${BETULA_FILES_DIR}/public-ports.conf"
  as_shipped "public ports" "${BETULA_ETC_DIR}/public-ports.conf" public-ports.conf
  as_shipped "filter script" /usr/local/sbin/betula-docker-firewall betula-docker-firewall.sh
  as_shipped "docker.service drop-in" /etc/systemd/system/docker.service.d/10-betula-firewall.conf docker-service-firewall.conf
  value="$(systemctl show docker.service --property=ExecStartPre --value 2>/dev/null || true)"
  if [[ "${value}" == *betula-docker-firewall* ]]; then pass "docker.service runs the filter as ExecStartPre"; else fail "docker.service has no ExecStartPre for the filter (daemon-reload missing?)"; fi

  value="$(public_port_list | sort | tr '\n' ' ')"
  FILTER_OK=1
  check_published_filter iptables
  conf_ok="${FILTER_OK}"
  expect "iptables: ports the filter lets through" "${FILTER_PORTS% }" "${value% }"
  # The host has a global IPv6 address, so the same filter is mandatory for IPv6. (conf_ok stays
  # an IPv4 verdict: published ports are DNATed on IPv4 only; IPv6 clients reach them through
  # docker-proxy, which is INPUT traffic and judged in the ufw section.)
  if ipv6_enabled; then
    if have_cmd ip6tables; then
      check_published_filter ip6tables
      expect "ip6tables: ports the filter lets through" "${FILTER_PORTS% }" "${value% }"
    else
      fail "ip6tables is not installed although IPv6 is enabled"
    fi
  fi

  # What is published right now? Host-mode ports show up in "docker ps", ingress ports in "service ls".
  ports="$( (docker ps --format '{{.Ports}}'; docker service ls --format '{{.Ports}}') 2>/dev/null |
    tr ',' '\n' | grep -oE '(:|\*:)[0-9]+(-[0-9]+)?->[0-9-]+/(tcp|udp)' | sed -E 's/^\*?://; s/->[0-9-]+//' | sort -u || true)"
  if [[ -z "${ports}" ]]; then
    pass "no container publishes a port right now"
  fi
  for port in ${ports}; do
    if is_public "${port}"; then
      pass "published and public by contract: ${port}"
    elif [[ "${conf_ok}" -eq 1 ]]; then
      warning "port ${port} is published by a container; the filter drops it from outside, but it should not be published"
    else
      fail "port ${port} is published by a container and the filter is NOT in place: reachable from the internet"
    fi
  done

  if unit_active betula-docker-prune.timer; then pass "betula-docker-prune.timer is active"; else fail "betula-docker-prune.timer is not active"; fi
  as_shipped "prune service" /etc/systemd/system/betula-docker-prune.service betula-docker-prune.service
  as_shipped "prune timer" /etc/systemd/system/betula-docker-prune.timer betula-docker-prune.timer
}

check_listeners() {
  section "listening sockets (everything not bound to loopback)"
  local line proto local_addr addr port proc ufw_on=0 key
  local -A seen=()
  load_public_ports "${BETULA_FILES_DIR}/public-ports.conf"
  if have_cmd ufw && contains_line "$(ufw status 2>/dev/null || true)" '^Status: active'; then ufw_on=1; fi

  while IFS= read -r line; do
    [[ -n "${line}" ]] || continue
    proto="$(awk '{ print $1 }' <<<"${line}")"
    local_addr="$(awk '{ print $5 }' <<<"${line}")"
    port="${local_addr##*:}"
    addr="${local_addr%:*}"
    proc="$(sed -n 's/.*users:(("\([^"]*\)".*/\1/p' <<<"${line}")"
    proc="${proc:-?}"
    case "${addr}" in
      127.* | "[::1]" | "[::ffff:127."*) continue ;;
    esac
    # The same port shows up once per address family; report it once. (Key without quotes or
    # brackets: they are trouble inside an associative array subscript.)
    key="${port}_${proto}_${proc//[^A-Za-z0-9_.-]/_}"
    if [[ -n "${seen[${key}]:-}" ]]; then continue; fi
    seen[${key}]=1

    if [[ "${proto}" == "tcp" && "${port}" == "${SSH_PORT}" ]]; then
      pass "${port}/${proto} ${proc}: ssh"
    elif is_public "${port}/${proto}"; then
      pass "${port}/${proto} ${proc}: public by contract"
    elif [[ "${proc}" == "docker-proxy" ]]; then
      # Judged in the docker section (published ports bypass ufw; our filter decides).
      warning "${port}/${proto} ${proc}: a container publishes a port outside the contract (see section docker)"
    elif [[ "${ufw_on}" -eq 0 ]]; then
      fail "${port}/${proto} ${proc}: listening and ufw is not active"
    else
      case "${port}/${proto}" in
        2377/tcp | 7946/tcp | 7946/udp | 4789/udp)
          pass "${port}/${proto} ${proc}: swarm port, closed by ufw (single node talks to itself over loopback)"
          ;;
        68/udp | 546/udp)
          pass "${port}/${proto} ${proc}: DHCP client, closed by ufw"
          ;;
        *)
          warning "${port}/${proto} ${proc}: unexpected listener (closed by ufw, but why is it there?)"
          ;;
      esac
    fi
  done < <(ss -H -tulnp 2>/dev/null || true)
}

check_reboot() {
  section "pending reboot"
  if [[ -e /var/run/reboot-required ]]; then
    warning "reboot required ($(sort -u /var/run/reboot-required.pkgs 2>/dev/null | tr '\n' ' ')); unattended-upgrades reboots at 04:30 host-local time (${HOST_TIMEZONE})"
  else
    pass "no reboot pending"
  fi
}

# ---------------------------------------------------------------- main

sections=("$@")
if [[ "${#sections[@]}" -eq 0 ]]; then
  sections=("${ALL_SECTIONS[@]}")
fi
for name in "${sections[@]}"; do
  if [[ " ${ALL_SECTIONS[*]} " != *" ${name} "* ]]; then
    die "unknown section '${name}' (known: ${ALL_SECTIONS[*]})"
  fi
done
for name in "${sections[@]}"; do
  "check_${name}"
done

printf '\nSUMMARY  pass=%d warn=%d fail=%d\n' "${PASSED}" "${WARNED}" "${FAILED}"
if [[ "${FAILED}" -gt 0 ]]; then
  exit 1
fi
