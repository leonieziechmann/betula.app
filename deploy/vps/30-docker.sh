#!/usr/bin/env bash
# 30-docker.sh - Docker Engine, one-node swarm, overlay networks, published-port firewall, prune timer.
#
#   sudo bash /opt/betula/vps/30-docker.sh
#
# Idempotent. A re-run never upgrades or restarts Docker on its own (both restart every
# container, because swarm rules out live-restore), and the packages are put on hold so that a
# routine "apt full-upgrade" does not do it either. Ask for it explicitly:
#
# Environment:
#   DOCKER_UPGRADE=1          upgrade the Docker packages within the pinned major version
#   ALLOW_DOCKER_RESTART=1    restart dockerd when daemon.json changed although containers run
#                             (works on a later run too: the pending restart is remembered in /run)
#   DOCKER_APT_SUITE=noble    force the repository suite (default: this release, else the fallback)
#   SWARM_ADVERTISE_ADDR=...  override the detected public IPv4
#   SWARM_ADDR_POOL=...       address pool for overlay networks (default 10.200.0.0/16, /24 each);
#                             only used when the swarm is created
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
betula_init --tmp
require_root
require_ubuntu

# Verified on 2026-09-20 against https://download.docker.com/linux/ubuntu/dists/ :
# "resolute" (26.04) is served, newest engine 29.8.1. The major version is pinned through
# files/apt-preferences-docker; keep DOCKER_MAJOR in step with it (and with 90-verify-host.sh).
DOCKER_MAJOR=29
DOCKER_REPO="https://download.docker.com/linux/ubuntu"
# Fingerprint of "Docker Release (CE deb)", as published on docs.docker.com/engine/install/ubuntu.
DOCKER_KEY_FINGERPRINT="9DC858229FC7DD38854AE2D88D81803C0EBFCD88"
DOCKER_KEYRING="/etc/apt/keyrings/docker.asc"
# If Docker does not serve this Ubuntu release yet (the first weeks after a release), use the
# newest LTS it does serve. The packages are Go binaries with few distribution dependencies and
# this is the combination everybody runs in that window. The script says so loudly.
DOCKER_FALLBACK_SUITE="noble"
# No buildx/compose plugins: images arrive with "docker save | ssh docker load", and
# "docker stack deploy" needs neither.
DOCKER_PACKAGES=(docker-ce docker-ce-cli containerd.io)
CONFLICTING_PACKAGES=(docker.io docker-doc docker-compose docker-compose-v2 docker-buildx podman-docker containerd runc)

SWARM_ADDR_POOL="${SWARM_ADDR_POOL:-10.200.0.0/16}"
SWARM_ADDR_POOL_MASK=24
OVERLAY_NETWORKS=(edge monitoring cortex)
# Of those, the ones without a way out of the host. "cortex" joins Cortex (stacks/cortex.yml) and
# the services that fetch through it: Radix's networks are internal, so Cortex, which has a
# network with a way out of its own, is its way to the internet (docs/cortex/cortex.md).
INTERNAL_NETWORKS=(cortex)
# Must match "default-address-pools" in files/docker-daemon.json (local bridges, docker_gwbridge).
BRIDGE_POOL_REGEX='^172\.30\.'

# 0 = nothing to do, 1 = restart dockerd in install_engine, 2 = needed but refused (containers run).
DOCKER_RESTART_PENDING=0
# Exists while /etc/docker/daemon.json is newer than the running dockerd (see install_daemon_config).
DOCKER_RESTART_MARKER="${BETULA_RUN_DIR}/docker-restart-pending"

# ---------------------------------------------------------------- steps

install_prerequisites() {
  step "Prerequisites"
  # Normally all there after 10-base.sh; listed so that this script also stands on its own.
  #   ca-certificates, curl  fetch the repository key over https
  #   gpg                    check that key's fingerprint (apt itself verifies with sqv on 26.04)
  #   jq                     validate daemon.json before dockerd has to
  #   iptables               the published-port filter runs before docker-ce (which depends on it)
  apt_install ca-certificates curl gpg jq iptables
  require_cmd ip iptables iptables-restore
  if ipv6_enabled; then
    require_cmd ip6tables ip6tables-restore
  fi
}

check_address_pools() {
  step "Address pools must not collide with the host's networks"
  local addrs pool_regex
  addrs="$(ip -4 -o addr show scope global | awk '$2 !~ /^(docker|br-|veth)/ { print $4 }')"
  # Compared by the first two octets (the pools are /16 by convention here):
  # 10.200.0.0/16 -> ^10\.200\.
  pool_regex="^$(cut -d. -f1-2 <<<"${SWARM_ADDR_POOL}" | sed 's/\./\\./g')\\."
  if contains_line "${addrs}" "${pool_regex}"; then
    die "a host address lies inside the swarm pool ${SWARM_ADDR_POOL}; choose another with SWARM_ADDR_POOL="
  fi
  if contains_line "${addrs}" "${BRIDGE_POOL_REGEX}"; then
    die "a host address lies inside 172.30.0.0/16; change default-address-pools in files/docker-daemon.json (and BRIDGE_POOL_REGEX here)"
  fi
  log "host addresses: ${addrs//$'\n'/ } - no overlap with 172.30.0.0/16 or ${SWARM_ADDR_POOL}"
}

remove_conflicting_packages() {
  step "Remove distribution packages that conflict with docker-ce (only if installed)"
  local pkg
  local -a present=()
  # Once docker-ce is installed, "containerd"/"runc" are provided by containerd.io; never purge then.
  if pkg_installed docker-ce; then
    log "docker-ce is installed; nothing to remove"
    return 0
  fi
  for pkg in "${CONFLICTING_PACKAGES[@]}"; do
    if pkg_installed "${pkg}"; then present+=("${pkg}"); fi
  done
  if [[ "${#present[@]}" -eq 0 ]]; then
    log "none installed"
    return 0
  fi
  log "removing: ${present[*]}"
  apt_run remove "${present[@]}"
}

# True when docker.service was (re)started after the marker was written. Compared as ages in
# seconds (monotonic clock for the unit, mtime for the marker): no parsing of localized
# timestamps. Anything unreadable answers "no", which leaves the restart pending: the safe side.
docker_restarted_since_marker() {
  local started marked
  started="$(systemctl show docker.service --property=ActiveEnterTimestampMonotonic --value 2>/dev/null || true)"
  marked="$(stat -c %Y "${DOCKER_RESTART_MARKER}" 2>/dev/null || true)"
  [[ "${started}" =~ ^[0-9]+$ && "${started}" -gt 0 && "${marked}" =~ ^[0-9]+$ ]] || return 1
  started=$(($(cut -d. -f1 /proc/uptime) - started / 1000000))
  marked=$(($(date +%s) - marked))
  [[ "${started}" -lt "${marked}" ]]
}

install_daemon_config() {
  step "/etc/docker/daemon.json"
  # Written BEFORE the package is installed, so the very first dockerd start already uses our
  # address pools and log driver (docker0 would otherwise be born as 172.17.0.0/16).
  #
  #   log-driver local      rotation built in (10 MB x 5, compressed) and "docker logs" / the API
  #                         still work, which is how Grafana Alloy's loki.source.docker reads.
  #                         (json-file has no rotation by default; syslog/journald as the driver
  #                         would put every container line into the capped host journal as well.)
  #                         One service goes there on purpose: Traefik, whose access log with the
  #                         visitors' addresses must go after 7 days, and the local driver only
  #                         rotates by size (stacks/edge.yml, "logging"; files/journald-betula.conf).
  #   default-address-pools /24 networks out of 172.30.0.0/16 instead of Docker's /16 slices of
  #                         172.17-31 and 192.168: 256 networks, no surprise overlap with a VPN
  #                         or provider network. (Overlay networks: SWARM_ADDR_POOL, at swarm init.)
  #   firewall-backend      iptables, explicitly: see files/betula-docker-firewall.sh. A future
  #                         default switch to nftables must not silently drop DOCKER-USER.
  #   live-restore false    swarm mode is incompatible with live-restore; written out so nobody
  #                         "optimises" it in.
  #   no-new-privileges     containers cannot gain privileges through setuid binaries; none of
  #                         our images needs that.
  #   icc false             containers on the default bridge (ad-hoc "docker run") cannot talk to
  #                         each other; services use their own networks and are unaffected.
  # Not set: "ip" (default bind address) only affects the default bridge, not swarm services,
  # and "userland-proxy" stays on (docker-proxy is what serves IPv6 clients and hairpin traffic).
  local src="${BETULA_FILES_DIR}/docker-daemon.json" containers backup
  jq -e . "${src}" >/dev/null || die "${src} is not valid JSON"
  install_file "${src}" /etc/docker/daemon.json 0644
  if [[ "${INSTALL_CHANGED}" -eq 1 ]] && have_cmd dockerd; then
    if ! dockerd --validate --config-file /etc/docker/daemon.json >/dev/null 2>&1; then
      backup="$(backup_path_of /etc/docker/daemon.json)"
      if [[ -n "${backup}" ]]; then cp -a -- "${backup}" /etc/docker/daemon.json; fi
      die "dockerd --validate rejects the new daemon.json; previous version restored"
    fi
    if unit_active docker.service; then
      # Remembered outside this process: on the re-run that is supposed to apply it
      # (ALLOW_DOCKER_RESTART=1) the file is already identical and INSTALL_CHANGED says 0.
      # In /run, because a reboot restarts dockerd and thereby settles the matter.
      install -d -m 0755 "${BETULA_RUN_DIR}"
      : >"${DOCKER_RESTART_MARKER}"
    fi
  fi

  [[ -e "${DOCKER_RESTART_MARKER}" ]] || return 0
  if ! unit_active docker.service; then
    # Not running: the next start reads the file anyway.
    rm -f -- "${DOCKER_RESTART_MARKER}"
    return 0
  fi
  # Somebody restarted dockerd since the marker was written? Then the new file is in effect.
  if docker_restarted_since_marker; then
    log "dockerd was restarted after daemon.json changed; nothing pending"
    rm -f -- "${DOCKER_RESTART_MARKER}"
    return 0
  fi
  DOCKER_RESTART_PENDING=1
  containers="$(docker ps --quiet 2>/dev/null | wc -l)"
  if [[ "${containers}" -gt 0 && "${ALLOW_DOCKER_RESTART:-0}" != "1" ]]; then
    warn "daemon.json is newer than the running dockerd and ${containers} container(s) are running; NOT restarting."
    warn "re-run with ALLOW_DOCKER_RESTART=1 in a quiet moment (every container restarts)."
    DOCKER_RESTART_PENDING=2
  fi
}

install_firewall_integration() {
  step "Published-port filter (DOCKER-USER), hooked into docker.service"
  # Installed before the engine, for the same reason as daemon.json: the package's first start
  # of dockerd must already run through ExecStartPre. Details: files/betula-docker-firewall.sh.
  load_public_ports "${BETULA_FILES_DIR}/public-ports.conf"
  install_file "${BETULA_FILES_DIR}/public-ports.conf" "${BETULA_ETC_DIR}/public-ports.conf" 0644
  install_file "${BETULA_FILES_DIR}/betula-docker-firewall.sh" /usr/local/sbin/betula-docker-firewall 0755
  install_file "${BETULA_FILES_DIR}/docker-service-firewall.conf" /etc/systemd/system/docker.service.d/10-betula-firewall.conf 0644
  if [[ "${INSTALL_CHANGED}" -eq 1 ]]; then
    systemctl daemon-reload
  fi
  # Apply now as well: idempotent, atomic, and it covers a dockerd that is already running.
  /usr/local/sbin/betula-docker-firewall
}

verify_repo_key() {
  local keyfile=$1 gnupghome fingerprint
  gnupghome="$(mktemp -d "${BETULA_TMPDIR}/gnupg.XXXXXX")"
  chmod 0700 "${gnupghome}"
  # First "fpr" record = primary key (sub keys follow).
  fingerprint="$(GNUPGHOME="${gnupghome}" gpg --batch --quiet --show-keys --with-colons "${keyfile}" |
    awk -F: '$1 == "fpr" { print $10 }' | sed -n '1p')"
  [[ "${fingerprint}" == "${DOCKER_KEY_FINGERPRINT}" ]] ||
    die "Docker repository key has fingerprint '${fingerprint}', expected ${DOCKER_KEY_FINGERPRINT}; not trusting it"
  log "Docker repository key verified: ${fingerprint}"
}

suite_is_served() {
  local suite=$1 arch=$2
  curl -fsSI --retry 2 --max-time 20 --proto '=https' \
    "${DOCKER_REPO}/dists/${suite}/stable/binary-${arch}/Packages" >/dev/null 2>&1
}

configure_repository() {
  step "Docker's official apt repository"
  local arch suite keytmp rendered

  arch="$(dpkg --print-architecture)"
  case "${arch}" in
    amd64 | arm64) ;;
    *) die "architecture ${arch} is not covered by this script" ;;
  esac

  keytmp="$(betula_tmpfile)"
  curl -fsSL --retry 3 --max-time 30 --proto '=https' --tlsv1.2 "${DOCKER_REPO}/gpg" -o "${keytmp}"
  verify_repo_key "${keytmp}"
  install_file "${keytmp}" "${DOCKER_KEYRING}" 0644

  suite="${DOCKER_APT_SUITE:-${OS_CODENAME}}"
  if suite_is_served "${suite}" "${arch}"; then
    log "repository suite: ${suite}"
  elif [[ -n "${DOCKER_APT_SUITE:-}" ]]; then
    die "DOCKER_APT_SUITE=${suite} is not served by ${DOCKER_REPO} for ${arch}"
  elif suite_is_served "${DOCKER_FALLBACK_SUITE}" "${arch}"; then
    warn "Docker does not serve '${suite}' (yet); FALLING BACK to '${DOCKER_FALLBACK_SUITE}'."
    warn "re-run this script once ${DOCKER_REPO}/dists/${suite}/ exists to switch over."
    suite="${DOCKER_FALLBACK_SUITE}"
  else
    die "cannot reach ${DOCKER_REPO} (neither '${suite}' nor '${DOCKER_FALLBACK_SUITE}' answer)"
  fi

  # The one-line format of older instructions would make apt warn about a duplicate source.
  remove_file /etc/apt/sources.list.d/docker.list
  rendered="$(render_template "${BETULA_FILES_DIR}/docker.sources.tmpl" "SUITE=${suite}" "ARCH=${arch}")"
  install_file "${rendered}" /etc/apt/sources.list.d/docker.sources 0644
  local sources_changed=${INSTALL_CHANGED}
  install_file "${BETULA_FILES_DIR}/apt-preferences-docker" /etc/apt/preferences.d/betula-docker 0644
  if [[ "${sources_changed}" -eq 1 || "${INSTALL_CHANGED}" -eq 1 ]]; then
    apt_update --force
  fi
}

install_engine() {
  step "Docker Engine ${DOCKER_MAJOR}.x"
  local version
  if [[ "${DOCKER_UPGRADE:-0}" == "1" ]] && pkg_installed docker-ce; then
    warn "DOCKER_UPGRADE=1: upgrading the engine restarts every container"
    apt-mark unhold "${DOCKER_PACKAGES[@]}" >/dev/null
    apt_update
    apt_run install --only-upgrade --no-install-recommends "${DOCKER_PACKAGES[@]}"
  else
    apt_install "${DOCKER_PACKAGES[@]}"
  fi
  # On hold: "apt full-upgrade" (a re-run of 10-base.sh, or an operator's routine upgrade) must
  # not bounce every container as a side effect. Engine upgrades go through DOCKER_UPGRADE=1.
  apt-mark hold "${DOCKER_PACKAGES[@]}" >/dev/null
  log "on hold: ${DOCKER_PACKAGES[*]}"

  systemctl enable containerd.service docker.service >/dev/null 2>&1
  if [[ "${DOCKER_RESTART_PENDING}" -eq 1 ]] && unit_active docker.service; then
    # A package upgrade a few lines up has restarted dockerd already; once is enough.
    if docker_restarted_since_marker; then
      log "dockerd was just restarted by the package upgrade; the new daemon.json is in effect"
    else
      log "restarting dockerd to apply the new daemon.json"
      systemctl restart docker.service
    fi
    rm -f -- "${DOCKER_RESTART_MARKER}"
  fi
  unit_active docker.service || systemctl start docker.service
  unit_active docker.service || die "docker.service is not running; see: journalctl -u docker.service -n 50"

  version="$(docker version --format '{{.Server.Version}}')"
  log "Docker Engine ${version}"
  if [[ "${version%%.*}" != "${DOCKER_MAJOR}" ]]; then
    warn "expected major version ${DOCKER_MAJOR}; the firewall integration was only verified against it"
  fi
}

add_deploy_to_docker_group() {
  step "Group docker for ${DEPLOY_USER}"
  if ! id -u "${DEPLOY_USER}" >/dev/null 2>&1; then
    warn "user ${DEPLOY_USER} does not exist (10-base.sh creates it); skipped"
    return 0
  fi
  # Equivalent to root, which deploy already is through sudo; it saves the operator's
  # automation a "sudo" in front of every docker command (docker load, stack deploy).
  usermod --append --groups docker "${DEPLOY_USER}"
  log "${DEPLOY_USER} is in group docker (new sessions only)"
}

init_swarm() {
  step "Swarm (single node, manager)"
  local state manager addr
  state="$(docker info --format '{{.Swarm.LocalNodeState}}')"
  case "${state}" in
    active)
      manager="$(docker info --format '{{.Swarm.ControlAvailable}}')"
      [[ "${manager}" == "true" ]] || die "this node is a swarm worker, not a manager; refusing to touch it"
      log "swarm already active, advertise address $(docker info --format '{{.Swarm.NodeAddr}}')"
      ;;
    inactive)
      # eth0 carries a global IPv6 address as well. default_ipv4 only asks the IPv4 routing
      # table, and the pattern below refuses anything that is not an IPv4 address.
      addr="${SWARM_ADVERTISE_ADDR:-$(default_ipv4)}"
      [[ "${addr}" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "could not detect the public IPv4 (set SWARM_ADVERTISE_ADDR)"
      if is_private_ipv4 "${addr}"; then
        warn "${addr} is a private address (host behind NAT?); fine for a single node, using it"
      fi
      # The swarm ports (2377/tcp, 7946, 4789/udp) listen on all addresses and stay closed in
      # ufw: a single node only talks to itself, and that goes through the loopback interface.
      # Output discarded on purpose: it contains the join token, and tokens do not belong in logs.
      docker swarm init \
        --advertise-addr "${addr}" \
        --default-addr-pool "${SWARM_ADDR_POOL}" \
        --default-addr-pool-mask-length "${SWARM_ADDR_POOL_MASK}" >/dev/null
      log "swarm initialised, advertise address ${addr}, overlay pool ${SWARM_ADDR_POOL} (/${SWARM_ADDR_POOL_MASK} each)"
      ;;
    *)
      die "swarm is in state '${state}'; resolve that by hand (docker info)"
      ;;
  esac
  # Keep 2 old tasks per service instead of 5: enough for "docker service ps" forensics, fewer
  # dead containers and logs on a small disk.
  docker swarm update --task-history-limit 2 >/dev/null
}

# is_internal_network NAME - true for the networks of INTERNAL_NETWORKS.
is_internal_network() {
  local net
  for net in "${INTERNAL_NETWORKS[@]}"; do
    [[ "$1" == "${net}" ]] && return 0
  done
  return 1
}

create_overlay_networks() {
  step "Overlay networks: ${OVERLAY_NETWORKS[*]} (internal: ${INTERNAL_NETWORKS[*]})"
  local net facts internal
  local -a flags
  for net in "${OVERLAY_NETWORKS[@]}"; do
    internal=false
    if is_internal_network "${net}"; then
      internal=true
    fi
    # Checked, never changed: Docker sets "internal" when it creates a network and cannot change
    # it afterwards, and a network that is in use cannot be removed.
    if facts="$(docker network inspect "${net}" --format '{{.Driver}} {{.Scope}} {{.Attachable}} {{.Internal}}' 2>/dev/null)"; then
      [[ "${facts}" == "overlay swarm true ${internal}" ]] ||
        die "network ${net} exists but is '${facts}' (driver, scope, attachable, internal; wanted 'overlay swarm true ${internal}'); remove it by hand once nothing uses it"
      log "network ${net} exists"
    else
      # Attachable: the stacks reference them as external networks, and a one-off
      # "docker run --network edge ..." for debugging has to be possible.
      flags=(--driver overlay --attachable)
      if [[ "${internal}" == "true" ]]; then
        flags+=(--internal)
      fi
      docker network create "${flags[@]}" \
        --label "app.betula.managed-by=vps/30-docker.sh" "${net}" >/dev/null
      log "network ${net} created (internal: ${internal})"
    fi
  done
}

install_prune_timer() {
  step "Weekly prune timer (dangling images and old build cache only)"
  local changed=0
  install_file "${BETULA_FILES_DIR}/betula-docker-prune.service" /etc/systemd/system/betula-docker-prune.service 0644
  changed=$((changed + INSTALL_CHANGED))
  install_file "${BETULA_FILES_DIR}/betula-docker-prune.timer" /etc/systemd/system/betula-docker-prune.timer 0644
  changed=$((changed + INSTALL_CHANGED))
  if [[ "${changed}" -gt 0 ]]; then
    systemctl daemon-reload
    # A timer that is already waiting keeps the schedule it was started with.
    if unit_active betula-docker-prune.timer; then
      systemctl restart betula-docker-prune.timer
    fi
  fi
  systemctl enable --now betula-docker-prune.timer >/dev/null 2>&1
  log "next run: $(systemctl show betula-docker-prune.timer --property=NextElapseUSecRealtime --value)"
}

# check_filter_family iptables|ip6tables
check_filter_family() {
  local ipt=$1 forward user chain
  forward="$("${ipt}" -w 10 -S FORWARD)"
  user="$("${ipt}" -w 10 -S DOCKER-USER)"
  chain="$("${ipt}" -w 10 -S BETULA-PUBLISHED)"
  [[ "$(sed -n '2p' <<<"${user}")" == "-A DOCKER-USER -j BETULA-PUBLISHED" ]] ||
    die "${ipt}: the first rule of DOCKER-USER is not the jump to BETULA-PUBLISHED"
  [[ "$(sed -n '$p' <<<"${chain}")" == "-A BETULA-PUBLISHED -j DROP" ]] ||
    die "${ipt}: BETULA-PUBLISHED does not end with DROP"
  contains_line "${forward}" '^-P FORWARD DROP$' || die "${ipt}: FORWARD policy is not DROP"
  if contains_line "${forward}" '^-A FORWARD -j DOCKER-USER$'; then
    log "${ipt}: FORWARD -> DOCKER-USER -> BETULA-PUBLISHED is in place; policy DROP"
  elif [[ "${ipt}" == "iptables" ]]; then
    die "iptables: FORWARD does not jump to DOCKER-USER; is dockerd using the iptables backend?"
  else
    # Docker only hooks its chains into the IPv6 FORWARD chain when it manages IPv6 forwarding.
    # Without the jump nothing is forwarded at all (policy DROP), so this is the safe state; the
    # filter already sits in DOCKER-USER for the day an IPv6 network appears.
    log "${ipt}: Docker forwards no IPv6 (no jump to DOCKER-USER); policy DROP, filter prepared"
  fi
}

check_firewall() {
  step "Check the published-port filter after Docker has set up its own chains"
  check_filter_family iptables
  # The host has a global IPv6 address: the IPv6 half is not optional. (Published ports reach
  # IPv6 clients through docker-proxy, i.e. INPUT, which ufw's v6 rules close; this is the
  # forward path.)
  if ! ipv6_enabled; then
    log "IPv6 is disabled in the kernel; no ip6tables filter to check"
  elif has_global_ipv6 || ip6tables -w 10 -S BETULA-PUBLISHED >/dev/null 2>&1; then
    check_filter_family ip6tables
  else
    # Mirrors betula-docker-firewall: best effort only where no global IPv6 address exists.
    warn "the ip6tables filter is not installed (this host has no global IPv6 address, so nothing is exposed)"
  fi
}

report() {
  step "Done"
  log "networks: $(docker network ls --filter driver=overlay --format '{{.Name}}' | tr '\n' ' ')"
  log "public ports for containers: tcp ${PUBLIC_TCP_PORTS}${PUBLIC_UDP_PORTS:+, udp ${PUBLIC_UDP_PORTS}} (everything else Docker publishes is dropped)"
  if [[ "${DOCKER_RESTART_PENDING}" -eq 2 ]]; then
    warn "PENDING: daemon.json changed but dockerd was not restarted (ALLOW_DOCKER_RESTART=1)"
  fi
  log "next: sudo bash /opt/betula/vps/90-verify-host.sh"
  log "then, in a NEW session as ${DEPLOY_USER} (group docker only applies to new logins) and without sudo:"
  log "      bash /opt/betula/vps/40-stacks.sh && bash /opt/betula/vps/91-verify-stacks.sh"
  if [[ -e /var/run/reboot-required ]]; then
    printf 'REBOOT_REQUIRED\n'
  fi
}

# ---------------------------------------------------------------- main

install_prerequisites
check_address_pools
remove_conflicting_packages
install_daemon_config
install_firewall_integration
configure_repository
install_engine
add_deploy_to_docker_group
init_swarm
create_overlay_networks
install_prune_timer
check_firewall
report
