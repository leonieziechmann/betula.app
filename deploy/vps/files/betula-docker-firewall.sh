#!/usr/bin/env bash
# Installed by /opt/betula/vps/30-docker.sh as /usr/local/sbin/betula-docker-firewall.
# Runs as ExecStartPre of docker.service (every boot, every docker restart) and can be run by
# hand at any time; it is idempotent and replaces its chain atomically.
#
# Why this exists
#   Ports published by Docker never pass ufw. Docker DNATs them in nat/PREROUTING, the packet is
#   then FORWARDed to the container, and ufw's allow/deny rules sit in INPUT. "ufw deny 9090"
#   does not stop "docker run -p 9090:9090" from being world readable.
#   The only hook Docker reserves for the administrator is the filter chain DOCKER-USER: Docker
#   jumps to it first from FORWARD, creates it when it is missing and never flushes it.
#   (Docker 29's native nftables backend has no DOCKER-USER, is experimental and refuses to run
#   in swarm mode, so this host pins "firewall-backend": "iptables" in daemon.json. On Ubuntu the
#   iptables command is iptables-nft: same kernel backend as ufw, no legacy/nft mix.)
#
# What it does
#   Everything that enters FORWARD from an interface that is not a Docker bridge came from
#   outside. Of that, only connections whose ORIGINAL destination port (before DNAT, hence the
#   conntrack match) is in /etc/betula/public-ports.conf may continue to Docker's own chains.
#   The rest is dropped: accidentally published ports (host mode and swarm ingress mode alike;
#   ingress ports went through a DOCKER-INGRESS chain until 29.7 and use the ordinary bridge
#   path since 29.8, in both cases after DOCKER-USER) and packets routed straight at container
#   addresses by a neighbour in the provider's network.
#   Rules live in our own chain BETULA-PUBLISHED; DOCKER-USER only holds the jump, so
#   "iptables-restore --noflush" can swap the chain in one transaction without an open window.
#
# Why ExecStartPre and not a unit of its own
#   It must be in place BEFORE dockerd starts containers after a boot, and docker must not start
#   without it (a failing ExecStartPre keeps the daemon down: fail closed, loudly).
#   ufw is no alternative home: its after.rules would have to declare DOCKER-USER, which flushes
#   the chain on every "ufw reload".
set -Eeuo pipefail
# iptables and ip live in sbin; do not depend on the caller's PATH (systemd, sudo, cron differ).
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

CONF="${BETULA_PUBLIC_PORTS_CONF:-/etc/betula/public-ports.conf}"
CHAIN="BETULA-PUBLISHED"
# Docker's bridge interfaces: default bridge, swarm gateway bridge, user-defined bridges.
# A bridge created with a custom com.docker.network.bridge.name would be treated as "outside"
# (fails closed: its containers could only open connections to the public ports).
DOCKER_BRIDGES=(docker0 docker_gwbridge "br-+")

PUBLIC_TCP_PORTS="80 443"
PUBLIC_UDP_PORTS=""
LOG_BLOCKED="yes"
if [[ -r "${CONF}" ]]; then
  # shellcheck disable=SC1090
  . "${CONF}"
fi

say() { printf 'betula-docker-firewall: %s\n' "$*"; }

for port in ${PUBLIC_TCP_PORTS} ${PUBLIC_UDP_PORTS}; do
  if ! [[ "${port}" =~ ^[0-9]{1,5}$ ]] || [[ "${port}" -lt 1 || "${port}" -gt 65535 ]]; then
    say "invalid port '${port}' in ${CONF}" >&2
    exit 1
  fi
done

# ruleset WITH_LOG -> iptables-restore input that (re)defines only our chain.
# In --noflush mode a declared user chain is emptied and refilled inside the same commit.
ruleset() {
  local with_log=$1 bridge port
  printf '*filter\n'
  printf ':%s - [0:0]\n' "${CHAIN}"
  # Replies to connections that containers opened, and later packets of accepted connections.
  printf -- '-A %s -m conntrack --ctstate RELATED,ESTABLISHED -j RETURN\n' "${CHAIN}"
  # Traffic that starts inside a container (to the internet or to another bridge) is not ours.
  for bridge in "${DOCKER_BRIDGES[@]}"; do
    printf -- '-A %s -i %s -j RETURN\n' "${CHAIN}" "${bridge}"
  done
  for port in ${PUBLIC_TCP_PORTS}; do
    printf -- '-A %s -p tcp -m conntrack --ctstate NEW --ctorigdstport %s -j RETURN\n' "${CHAIN}" "${port}"
  done
  for port in ${PUBLIC_UDP_PORTS}; do
    printf -- '-A %s -p udp -m conntrack --ctstate NEW --ctorigdstport %s -j RETURN\n' "${CHAIN}" "${port}"
  done
  if [[ "${with_log}" == "yes" ]]; then
    printf -- '-A %s -m limit --limit 6/min --limit-burst 10 -j LOG --log-prefix "[betula-docker-block] "\n' "${CHAIN}"
  fi
  printf -- '-A %s -j DROP\n' "${CHAIN}"
  printf 'COMMIT\n'
}

# apply iptables|ip6tables
apply() {
  local ipt=$1 first
  if [[ "${LOG_BLOCKED}" == "yes" ]]; then
    if ! ruleset yes | "${ipt}-restore" -w 10 --noflush; then
      # The LOG target needs a kernel module that minimal or containerised kernels lack.
      say "${ipt}: LOG target unavailable, installing the filter without logging" >&2
      ruleset no | "${ipt}-restore" -w 10 --noflush
    fi
  else
    ruleset no | "${ipt}-restore" -w 10 --noflush
  fi

  # Before the first dockerd start the chain does not exist yet; Docker adopts an existing one.
  "${ipt}" -w 10 -N DOCKER-USER 2>/dev/null || true

  # The jump has to be the FIRST rule: anything in front of it could accept before we filter.
  first="$("${ipt}" -w 10 -S DOCKER-USER | sed -n '2p')"
  if [[ "${first}" != "-A DOCKER-USER -j ${CHAIN}" ]]; then
    while "${ipt}" -w 10 -D DOCKER-USER -j "${CHAIN}" 2>/dev/null; do :; done
    "${ipt}" -w 10 -I DOCKER-USER 1 -j "${CHAIN}"
  fi

  # Docker only sets this policy when it had to enable ip_forward itself; sysctl already did
  # that here, and ufw sets DROP only while it is enabled. State it, so it never depends on either.
  "${ipt}" -w 10 -P FORWARD DROP
  say "${ipt}: ${CHAIN} active (tcp: ${PUBLIC_TCP_PORTS:-none}; udp: ${PUBLIC_UDP_PORTS:-none})"
}

# IPv4 is the path Docker really publishes on: any failure here must keep dockerd down.
apply iptables

# IPv6: this host has a global IPv6 address, so it is reachable over v6 whatever DNS says.
# Today no Docker network has IPv6: published ports reach v6 clients only through docker-proxy,
# which is INPUT traffic and therefore under ufw's v6 rules. The forward path gets the same
# filter for the day that changes. With a global address a failure is as fatal as on IPv4;
# only a host that cannot be reached over IPv6 anyway may start Docker without it.
if [[ -e /proc/net/if_inet6 ]]; then
  # Not "if ! apply ...": bash ignores "set -e" inside a condition, so a failing iptables call in
  # the middle of apply would go unnoticed. A plain subshell keeps -e alive.
  set +e
  (
    set -e
    apply ip6tables
  )
  v6_status=$?
  set -e
  if [[ "${v6_status}" -ne 0 ]]; then
    v6_global="$(ip -6 -o addr show scope global 2>/dev/null || true)"
    if [[ -n "${v6_global}" ]]; then
      say "ip6tables: could not install the filter and this host has a global IPv6 address; refusing to continue" >&2
      exit 1
    fi
    say "ip6tables: could not install the filter (no global IPv6 address, continuing; the IPv4 filter is active)" >&2
  fi
else
  say "IPv6 is disabled on this host; skipping ip6tables"
fi
