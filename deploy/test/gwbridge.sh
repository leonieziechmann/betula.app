#!/usr/bin/env bash
# gwbridge.sh - docker_gwbridge made anew with IPv6, by the code of vps/30-docker.sh
# (GWBRIDGE_REBUILD=1: gwbridge_preflight, gwbridge_rebuild of vps/lib-networks.sh), in a local
# one-node swarm, and what it is for: a client with an IPv6 address of its own reaches a stand-in of
# Traefik on a host-mode port and is seen as itself, not as the bridge's gateway. Run as root, on a
# throwaway machine with Docker and no swarm (it makes its own and leaves it): never on the server.
#
#   sudo bash deploy/test/gwbridge.sh        # up, check, down; exit 1 when a check fails
#
# up      IPv6 for lo and the new interfaces only (WSL has it off: disable_ipv6 1, addr_gen_mode 1;
#         where lo had no ::1, dockerd is restarted, here and at the end), a
#         swarm, three stand-ins of the real image: "traefik" (host-mode port 18480, on an outward
#         overlay network and an internal one; its one router answers the client's own addresses
#         and nothing else), "cortex" (outward network only), "folia" (internal network only); a
#         client in a network namespace of its own, 2001:db8:6::2 and 192.0.2.2, behind a veth.
# check   before: over IPv6 the client arrives as the gateway (404), over IPv4 as itself (200).
#         gwbridge_preflight refuses while addr_gen_mode is 1 (no link-local address on a new
#         bridge: the WSL trap). gwbridge_rebuild to the plan's subnet (vps/files/networks.conf):
#         "traefik" and "cortex" to 0 and back, "folia" keeps running, how long "traefik" did not
#         answer, the client seen as itself over both families; edge_ipv6_check of lib-stacks.sh
#         (what 40-stacks.sh edge and 91-verify-stacks.sh ipv6 ask) gets an answer over the host's
#         own IPv6 address, none while a stale neighbour entry for Traefik stands, an answer again
#         once it is gone. Then the way back (GWBRIDGE_IPV6=off): the gateway again.
# Uses the host's port 443 for the stand-in (HTTPS, as edge_ipv6_check asks).
# down    the swarm left, docker_gwbridge removed, the namespace, the kernel settings as they were.
set -Eeuo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPLOY_DIR="$(cd "${TEST_DIR}/.." && pwd)"
# shellcheck source=../vps/lib.sh
. "${DEPLOY_DIR}/vps/lib.sh"
# shellcheck source=../vps/lib-networks.sh
. "${DEPLOY_DIR}/vps/lib-networks.sh"
# shellcheck source=../vps/lib-stacks.sh
. "${DEPLOY_DIR}/vps/lib-stacks.sh"
betula_init --tmp
require_root
for cmd in docker curl ip ip6tables systemctl; do require_cmd "${cmd}"; done

IMAGE="$(sed -n 's/^[[:space:]]*image:[[:space:]]*\(traefik:v[0-9.]*\)[[:space:]]*$/\1/p' "${DEPLOY_DIR}/stacks/edge.yml" | head -n 1)"
PORT=18480
NS="betula-gw6c"
WORK="$(mktemp -d /var/tmp/betula-gwbridge.XXXXXX)"
CHECKS_FAILED=0
declare -A SAVED=()
PROBER=""
RESTARTED=0

ok() { printf 'PASS  %s\n' "$*"; }
bad() {
  CHECKS_FAILED=$((CHECKS_FAILED + 1))
  printf 'FAIL  %s\n' "$*"
}

# ---------------------------------------------------------------- down

down() {
  [[ -z "${PROBER}" ]] || kill "${PROBER}" 2>/dev/null || true
  gwbridge_restore
  docker swarm leave --force >/dev/null 2>&1 || true
  for _ in 1 2 3 4 5; do
    docker network rm "${GWBRIDGE}" >/dev/null 2>&1 && break
    docker network inspect "${GWBRIDGE}" >/dev/null 2>&1 || break
    sleep 1
  done
  ip netns del "${NS}" 2>/dev/null || true
  ip link del gw6h 2>/dev/null || true
  # "all" and "default" first: writing "all" sets every interface.
  local f
  for f in /proc/sys/net/ipv6/conf/all/disable_ipv6 /proc/sys/net/ipv6/conf/default/disable_ipv6 /proc/sys/net/ipv6/conf/default/addr_gen_mode; do
    [[ -z "${SAVED[${f}]:-}" ]] || printf '%s' "${SAVED[${f}]}" >"${f}"
  done
  for f in "${!SAVED[@]}"; do
    [[ -e "${f}" ]] && printf '%s' "${SAVED[${f}]}" >"${f}"
  done
  [[ "${RESTARTED}" -eq 0 ]] || systemctl restart docker.service || true
  rm -rf -- "${WORK}"
}

# ---------------------------------------------------------------- up

[[ -n "${IMAGE}" ]] || die "no traefik image in stacks/edge.yml"
[[ "$(docker info --format '{{.Swarm.LocalNodeState}}')" == "inactive" ]] ||
  die "this Docker is in a swarm; gwbridge.sh makes its own and leaves it, so it runs only where there is none"
if docker network inspect "${GWBRIDGE}" >/dev/null 2>&1; then
  [[ -z "$(gwbridge_members)" ]] || die "${GWBRIDGE} has members: $(gwbridge_members)"
  docker network rm "${GWBRIDGE}" >/dev/null
fi
netplan_read
[[ -n "${NETPLAN_GWBRIDGE6}" ]] || die "vps/files/networks.conf names no gwbridge subnet"
for f in /proc/sys/net/ipv6/conf/*/disable_ipv6 /proc/sys/net/ipv6/conf/default/addr_gen_mode; do
  SAVED["${f}"]="$(cat "${f}")"
done
add_exit_hook down

# IPv6 for the interfaces made from now on, and for lo: Docker publishes a port on [::] only where
# it can listen on ::1, and it looks once, when dockerd starts (restarted here when lo had none,
# and again at the end). The other interfaces there were keep their setting.
echo 0 >/proc/sys/net/ipv6/conf/all/disable_ipv6
echo 0 >/proc/sys/net/ipv6/conf/default/disable_ipv6
for f in "${!SAVED[@]}"; do
  case "${f}" in */all/* | */default/* | */lo/*) ;; *) printf '%s' "${SAVED[${f}]}" >"${f}" ;; esac
done
if [[ "${SAVED[/proc/sys/net/ipv6/conf/lo/disable_ipv6]}" == "1" ]]; then
  log "lo had no ::1 when dockerd started: restarting dockerd (and once more at the end)"
  systemctl restart docker.service
  RESTARTED=1
fi
# A link-local address on every new interface, as on a normal host; the refusal is checked below.
echo 0 >/proc/sys/net/ipv6/conf/default/addr_gen_mode

docker pull -q "${IMAGE}" >/dev/null
mkdir -p "${WORK}/dynamic"
cat >"${WORK}/dynamic/seen.yml" <<'EOF'
http:
  routers:
    seen:
      entryPoints: [web]
      rule: "ClientIP(`2001:db8:6::2/128`) || ClientIP(`192.0.2.2/32`)"
      service: ping@internal
EOF
chmod -R a+rX "${WORK}"

ip netns add "${NS}"
ip link add gw6h type veth peer name gw6v netns "${NS}"
ip netns exec "${NS}" sh -c 'echo 0 >/proc/sys/net/ipv6/conf/all/disable_ipv6; echo 0 >/proc/sys/net/ipv6/conf/default/disable_ipv6'
ip addr add 2001:db8:6::1/64 dev gw6h nodad
ip addr add 192.0.2.1/24 dev gw6h
ip link set gw6h up
ip -n "${NS}" addr add 2001:db8:6::2/64 dev gw6v nodad
ip -n "${NS}" addr add 192.0.2.2/24 dev gw6v
ip -n "${NS}" link set gw6v up
ip -n "${NS}" -6 route add default via 2001:db8:6::1
ip -n "${NS}" route add default via 192.0.2.1

docker swarm init --advertise-addr "$(default_ipv4)" >/dev/null
docker network create --driver overlay gw6_out >/dev/null
docker network create --driver overlay --internal gw6_in >/dev/null
docker service create --detach --name gw6_traefik --network gw6_out --network gw6_in --stop-grace-period 20s \
  --publish "mode=host,target=80,published=${PORT}" --publish "mode=host,target=443,published=443" \
  --mount "type=bind,src=${WORK}/dynamic,dst=/dynamic,ro" \
  "${IMAGE}" --entryPoints.web.address=:80 --entryPoints.websecure.address=:443 --entryPoints.websecure.http.tls=true \
  --providers.file.directory=/dynamic --ping=true --ping.manualRouting=true >/dev/null
docker service create --detach --name gw6_cortex --network gw6_out "${IMAGE}" --ping=true >/dev/null
docker service create --detach --name gw6_folia --network gw6_in "${IMAGE}" --ping=true >/dev/null
gwbridge_wait_running gw6_traefik gw6_cortex gw6_folia

# status 6|4 -> what the stand-in answers the client (200: seen as itself; 404: as somebody else)
status() {
  local url="http://192.0.2.1:${PORT}/"
  [[ "$1" == 6 ]] && url="http://[2001:db8:6::1]:${PORT}/"
  ip netns exec "${NS}" curl -s -g -o /dev/null --max-time 3 -w '%{http_code}' "${url}" 2>/dev/null || true
}
expect() { # EXPECTED 6|4 WHAT
  local got
  for _ in $(seq 1 20); do
    got="$(status "$2")"
    [[ "${got}" == "$1" ]] && break
    sleep 1
  done
  if [[ "${got}" == "$1" ]]; then ok "$3: IPv$2 answers ${got}"; else bad "$3: IPv$2 answers ${got:-000}, expected $1"; fi
}

# ---------------------------------------------------------------- check

# edge_ipv6_check asks SITE_HOST over its AAAA addresses that are this machine's: here a name of
# the test's own, whose one AAAA address is the host's end of the client's veth.
SITE_HOST="betula.test"
PUBLIC_ADDRESSES="2001:db8:6::1"
host_addresses() {
  [[ "$1" == "${SITE_HOST}" ]] && printf '2001:db8:6::1\n'
  return 0
}
expect_ipv6_check() { # EXPECTED-RC WHAT
  local rc=0
  edge_ipv6_check || rc=$?
  if [[ "${rc}" == "$1" ]]; then ok "$2: edge_ipv6_check ${rc} (${EDGE_IPV6_DETAIL})"; else bad "$2: edge_ipv6_check ${rc}, expected $1 (${EDGE_IPV6_DETAIL})"; fi
}

expect 404 6 "before, the client over IPv6 is the bridge's gateway"
expect_ipv6_check 2 "before, nothing to ask"
expect 200 4 "before, the client over IPv4 is itself"
wanted="$(gwbridge_wanted)"
if [[ "${wanted}" == "${NETPLAN_GWBRIDGE6}" ]]; then ok "wanted: ${wanted}"; else bad "gwbridge_wanted says ${wanted}, the plan ${NETPLAN_GWBRIDGE6}"; fi
services="$(gwbridge_services | tr '\n' ' ')"
if [[ "${services}" == "gw6_cortex gw6_traefik " ]]; then ok "on the bridge: ${services}(not gw6_folia)"; else bad "gwbridge_services: '${services}', expected gw6_cortex gw6_traefik"; fi

echo 1 >/proc/sys/net/ipv6/conf/default/addr_gen_mode
if gwbridge_preflight "${wanted}"; then bad "gwbridge_preflight let addr_gen_mode 1 pass"; else ok "gwbridge_preflight refuses: ${GWBRIDGE_DETAIL}"; fi
echo 0 >/proc/sys/net/ipv6/conf/default/addr_gen_mode
if gwbridge_preflight "${wanted}"; then ok "gwbridge_preflight passes with addr_gen_mode 0"; else bad "gwbridge_preflight: ${GWBRIDGE_DETAIL}"; fi

folia="$(docker ps -q --filter name=gw6_folia)"
# Every quarter second: does the stand-in answer over IPv4?
(
  while :; do
    printf '%s %s\n' "$(date +%s.%N)" "$(status 4)"
    sleep 0.25
  done
) >"${WORK}/answers" &
PROBER=$!
gwbridge_rebuild "${wanted}"
sleep 2
kill "${PROBER}" 2>/dev/null || true
PROBER=""
away="$(awk '$2 != "200" { if (!first) first = $1; last = $1 } END { if (first) printf "%.1f", last - first + 0.25; else print 0 }' "${WORK}/answers")"
ok "the stand-in of Traefik did not answer for about ${away} s"
if [[ "$(docker ps -q --filter name=gw6_folia)" == "${folia}" ]]; then ok "gw6_folia kept running (internal networks only)"; else bad "gw6_folia was restarted"; fi
if gwbridge_verify "${wanted}"; then ok "gwbridge_verify: $(gwbridge_facts)"; else bad "gwbridge_verify: ${GWBRIDGE_DETAIL}"; fi
expect 200 6 "with IPv6 on the bridge, the client over IPv6 is itself"
expect 200 4 "with IPv6 on the bridge, the client over IPv4 is itself"
expect_ipv6_check 0 "with IPv6 on the bridge, Traefik answers over the host's IPv6"
# The suspected failure of 2026-11-08: the host's neighbour entry for Traefik's address points at a
# MAC address nobody has (a task replaced, its address kept).
traefik6="$(ip6tables -w 10 -t nat -S DOCKER | grep -- '--dport 443 ' | grep -o 'to-destination \[[^]]*' | sed 's/.*\[//' | head -n 1)"
ip -6 neigh replace "${traefik6}" lladdr 02:00:5e:00:53:01 dev "${GWBRIDGE}" nud permanent
expect_ipv6_check 1 "a stale neighbour entry for Traefik (${traefik6})"
ip -6 neigh del "${traefik6}" dev "${GWBRIDGE}"
expect_ipv6_check 0 "the stale entry gone"

GWBRIDGE_IPV6=off
wanted="$(gwbridge_wanted)"
gwbridge_rebuild "${wanted}"
if gwbridge_verify -; then ok "the way back: $(gwbridge_facts)"; else bad "the way back: ${GWBRIDGE_DETAIL}"; fi
expect 404 6 "IPv4 only again, the client over IPv6 is the gateway"
expect 200 4 "IPv4 only again, the client over IPv4 is itself"

if [[ "${CHECKS_FAILED}" -gt 0 ]]; then
  echo "${CHECKS_FAILED} check(s) failed"
  exit 1
fi
echo "all checks passed (${IMAGE}, Docker $(docker version --format '{{.Server.Version}}'))"
