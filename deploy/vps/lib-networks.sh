#!/usr/bin/env bash
# The network plan of this host (files/networks.conf) and what makes and checks it. Sourced AFTER
# lib.sh, never executed: by 30-docker.sh and 90-verify-host.sh (root), by 40-stacks.sh,
# 48-cortex.sh, 50-app.sh, cortex-seed.sh and 91-verify-stacks.sh (deploy), and by
# deploy/test/netmatrix.sh. Needs docker, awk and, for the probes, nsenter (util-linux), curl and
# root (sudo -n for deploy).
# shellcheck shell=bash

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  echo "lib-networks.sh is a library; source it after lib.sh" >&2
  exit 2
fi

NETPLAN_FILE="${NETPLAN_FILE:-${BETULA_FILES_DIR}/networks.conf}"
# Filled by netplan_read, in the order of the file.
NETPLAN_NETS=()
NETPLAN_OLD=()
NETPLAN_NOOUT=()
NETPLAN_LISTENERS=()
# The IPv6 subnet of docker_gwbridge (line "gwbridge"); empty: IPv4 only.
NETPLAN_GWBRIDGE6=""
declare -A NETPLAN_BY=() NETPLAN_INTERNAL=() NETPLAN_ATTACHABLE=() NETPLAN_SUBNET=() NETPLAN_MEMBERS=() NETPLAN_PORTS=()
# Set by netplan_check: what is wrong with a network, in words.
NETPLAN_DETAIL=""
# Filled by netmap_build: the services swarm attaches to each network, and the networks of each
# service (both by name, space separated), from the service specs.
declare -A NETMAP_SERVICES=() NETMAP_NETS=()
NETMAP_ALL_SERVICES=()

# netplan_read [FILE] - reads the plan; dies on anything it does not understand.
netplan_read() {
  local file="${1:-${NETPLAN_FILE}}" line n=0 name word
  local -a w
  [[ -r "${file}" ]] || die "the network plan ${file} is missing (run deploy/sync.sh)"
  assert_lf "${file}"
  NETPLAN_NETS=() NETPLAN_OLD=() NETPLAN_NOOUT=() NETPLAN_LISTENERS=() NETPLAN_GWBRIDGE6=""
  NETPLAN_BY=() NETPLAN_INTERNAL=() NETPLAN_ATTACHABLE=() NETPLAN_SUBNET=() NETPLAN_MEMBERS=() NETPLAN_PORTS=()
  while IFS= read -r line || [[ -n "${line}" ]]; do
    n=$((n + 1))
    line="${line%%#*}"
    read -r -a w <<<"${line}" || true
    [[ "${#w[@]}" -gt 0 ]] || continue
    case "${w[0]}" in
      net)
        [[ "${#w[@]}" -ge 7 ]] || die "${file}:${n}: 'net NAME BY INTERNAL ATTACHABLE SUBNET MEMBER...'"
        name="${w[1]}"
        [[ "${name}" =~ ^[a-z][a-z0-9_-]{0,62}$ ]] || die "${file}:${n}: '${name}' is no network name"
        [[ -z "${NETPLAN_BY[${name}]:-}" ]] || die "${file}:${n}: network ${name} is named twice"
        [[ "${w[2]}" == "host" || "${w[2]}" == "stack" ]] || die "${file}:${n}: BY is '${w[2]}', not host or stack"
        [[ "${w[3]}" == "yes" || "${w[3]}" == "no" ]] || die "${file}:${n}: INTERNAL is '${w[3]}', not yes or no"
        [[ "${w[4]}" == "yes" || "${w[4]}" == "no" ]] || die "${file}:${n}: ATTACHABLE is '${w[4]}', not yes or no"
        [[ "${w[5]}" == "-" || "${w[5]}" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}/[0-9]{1,2}$ ]] || die "${file}:${n}: SUBNET is '${w[5]}', not - or a.b.c.d/n"
        [[ "${w[5]}" == "-" || "${w[2]}" == "host" ]] || die "${file}:${n}: a fixed subnet needs a network the host makes"
        for word in "${w[@]:6}"; do
          [[ "${word}" =~ ^[a-z][a-z0-9_-]*(_\*)?\??$ ]] || die "${file}:${n}: '${word}' is no service name (name, name?, stack_* or stack_*?)"
        done
        NETPLAN_NETS+=("${name}")
        NETPLAN_BY["${name}"]="${w[2]}"
        NETPLAN_INTERNAL["${name}"]="${w[3]}"
        NETPLAN_ATTACHABLE["${name}"]="${w[4]}"
        NETPLAN_SUBNET["${name}"]="${w[5]}"
        NETPLAN_MEMBERS["${name}"]="${w[*]:6}"
        ;;
      listen)
        [[ "${#w[@]}" -ge 3 ]] || die "${file}:${n}: 'listen SERVICE PORT...'"
        for word in "${w[@]:2}"; do
          [[ "${word}" =~ ^[0-9]{1,5}$ ]] || die "${file}:${n}: '${word}' is no port"
        done
        NETPLAN_LISTENERS+=("${w[1]}")
        NETPLAN_PORTS["${w[1]}"]="${w[*]:2}"
        ;;
      noout)
        NETPLAN_NOOUT+=("${w[@]:1}")
        ;;
      old)
        [[ "${#w[@]}" -eq 2 ]] || die "${file}:${n}: 'old NAME'"
        NETPLAN_OLD+=("${w[1]}")
        ;;
      gwbridge)
        [[ "${#w[@]}" -eq 2 && "${w[1]}" =~ ^f[cd][0-9a-f]{2}(:[0-9a-f]{1,4}){0,3}::/64$ ]] ||
          die "${file}:${n}: 'gwbridge SUBNET6', a unique local /64 (fdxx:xxxx:xxxx:n::/64)"
        [[ -z "${NETPLAN_GWBRIDGE6}" ]] || die "${file}:${n}: gwbridge is named twice"
        NETPLAN_GWBRIDGE6="${w[1]}"
        ;;
      *) die "${file}:${n}: '${w[0]}' is no keyword of the plan (net, listen, noout, old, gwbridge)" ;;
    esac
  done <"${file}"
  [[ "${#NETPLAN_NETS[@]}" -gt 0 ]] || die "${file} names no network"
}

# netplan_host_networks -> the networks vps/30-docker.sh makes, one per line.
netplan_host_networks() {
  local net
  for net in "${NETPLAN_NETS[@]}"; do
    if [[ "${NETPLAN_BY[${net}]}" == "host" ]]; then printf '%s\n' "${net}"; fi
  done
}

# netplan_known NAME - true when NAME is a network of the plan.
netplan_known() { [[ -n "${NETPLAN_BY[$1]:-}" ]]; }

# netplan_is_old NAME - true for a network from before the plan (old NAME).
netplan_is_old() {
  local net
  for net in ${NETPLAN_OLD[@]+"${NETPLAN_OLD[@]}"}; do
    [[ "$1" == "${net}" ]] && return 0
  done
  return 1
}

# member_matches PATTERN SERVICE - true when SERVICE is meant by PATTERN (name, name?, stack_*,
# stack_*?).
member_matches() {
  local pattern="${1%\?}"
  if [[ "${pattern}" == *_\* ]]; then
    [[ "$2" == "${pattern%\*}"* ]]
  else
    [[ "$2" == "${pattern}" ]]
  fi
}

# netplan_planned NET SERVICE -> "required" or "optional" when the plan puts SERVICE on NET;
# nothing (and false) when it does not.
netplan_planned() {
  local word
  for word in ${NETPLAN_MEMBERS[$1]:-}; do
    if member_matches "${word}" "$2"; then
      if [[ "${word}" == *\? ]]; then printf 'optional'; else printf 'required'; fi
      return 0
    fi
  done
  return 1
}

# netplan_networks_of SERVICE -> the networks the plan puts SERVICE on, "<net> required|optional"
# per line.
netplan_networks_of() {
  local net how
  for net in "${NETPLAN_NETS[@]}"; do
    if how="$(netplan_planned "${net}" "$1")"; then printf '%s %s\n' "${net}" "${how}"; fi
  done
}

# netplan_ports SERVICE -> the ports the plan says SERVICE listens on (nothing when it names none).
netplan_ports() {
  local pattern
  for pattern in ${NETPLAN_LISTENERS[@]+"${NETPLAN_LISTENERS[@]}"}; do
    if member_matches "${pattern}" "$1"; then
      printf '%s' "${NETPLAN_PORTS[${pattern}]}"
      return 0
    fi
  done
  return 0
}

# netplan_noout SERVICE - true when the plan says a firewall of its own lets nothing out of it.
netplan_noout() {
  local pattern
  for pattern in ${NETPLAN_NOOUT[@]+"${NETPLAN_NOOUT[@]}"}; do
    member_matches "${pattern}" "$1" && return 0
  done
  return 1
}

# netplan_stack_of SERVICE -> the stack of a service named <stack>_<name>.
netplan_stack_of() { printf '%s' "${1%_*}"; }

# network_facts NAME -> "<driver> <scope> <attachable> <internal> <subnet>" of an existing
# network (subnet "-" when it has several or none); nothing when it does not exist.
network_facts() {
  local facts subnets
  facts="$(docker network inspect "$1" --format '{{.Driver}} {{.Scope}} {{.Attachable}} {{.Internal}}' 2>/dev/null)" || return 0
  subnets="$(docker network inspect "$1" --format '{{range .IPAM.Config}}{{.Subnet}} {{end}}' 2>/dev/null || true)"
  read -r -a subnets <<<"${subnets}" || true
  if [[ "${#subnets[@]}" -eq 1 ]]; then
    printf '%s %s' "${facts}" "${subnets[0]}"
  else
    printf '%s -' "${facts}"
  fi
}

# netplan_wanted NAME -> the facts network_facts should say of a network of the plan; the subnet
# is "*" (any) where the plan leaves it to the pool.
netplan_wanted() {
  local attachable=false internal=false subnet="${NETPLAN_SUBNET[$1]}"
  [[ "${NETPLAN_ATTACHABLE[$1]}" == "yes" ]] && attachable=true
  [[ "${NETPLAN_INTERNAL[$1]}" == "yes" ]] && internal=true
  [[ "${subnet}" != "-" ]] || subnet="*"
  printf 'overlay swarm %s %s %s' "${attachable}" "${internal}" "${subnet}"
}

# netplan_check NAME -> 0: exists as planned, 1: missing, 2: differs (NETPLAN_DETAIL says how).
netplan_check() {
  local name=$1 facts want
  NETPLAN_DETAIL=""
  facts="$(network_facts "${name}")"
  want="$(netplan_wanted "${name}")"
  if [[ -z "${facts}" ]]; then
    NETPLAN_DETAIL="network ${name} does not exist"
    return 1
  fi
  # shellcheck disable=SC2053  # the subnet of the wanted facts may be the pattern "*"
  if [[ "${facts}" != ${want} ]]; then
    NETPLAN_DETAIL="network ${name} is '${facts}' (driver scope attachable internal subnet), the plan says '${want}'"
    return 2
  fi
  return 0
}

# ensure_host_networks - vps/30-docker.sh: makes every network of the plan marked "host" that is
# missing, and refuses one that differs. Docker sets "internal", "attachable" and the subnet when
# it makes a network and changes none of them later, and a network in use cannot be removed, so
# one that differs is never touched here.
ensure_host_networks() {
  local net rc subnet
  local -a flags
  while IFS= read -r net; do
    rc=0
    netplan_check "${net}" || rc=$?
    subnet=""
    [[ "${NETPLAN_SUBNET[${net}]}" == "-" ]] || subnet=", subnet ${NETPLAN_SUBNET[${net}]}"
    case "${rc}" in
      0) log "network ${net} exists (internal ${NETPLAN_INTERNAL[${net}]}, attachable ${NETPLAN_ATTACHABLE[${net}]}${subnet})" ;;
      2) die "${NETPLAN_DETAIL}: remove it by hand once nothing uses it (docker network inspect ${net}), then run this again" ;;
      *)
        flags=(--driver overlay)
        [[ "${NETPLAN_INTERNAL[${net}]}" == "yes" ]] && flags+=(--internal)
        [[ "${NETPLAN_ATTACHABLE[${net}]}" == "yes" ]] && flags+=(--attachable)
        [[ "${NETPLAN_SUBNET[${net}]}" == "-" ]] || flags+=(--subnet "${NETPLAN_SUBNET[${net}]}")
        docker network create "${flags[@]}" --label "app.betula.managed-by=vps/30-docker.sh" "${net}" >/dev/null
        log "network ${net} created (${flags[*]:2})"
        ;;
    esac
  done < <(netplan_host_networks)
}

# require_host_networks [NAME...] - the deploy scripts: every network of the plan marked "host"
# (or the ones named) exists as planned; dies otherwise. A name the plan does not know is an
# instance without networks of its own.
require_host_networks() {
  local net
  local -a nets=("$@")
  if [[ "${#nets[@]}" -eq 0 ]]; then
    mapfile -t nets < <(netplan_host_networks)
  fi
  for net in "${nets[@]}"; do
    netplan_known "${net}" ||
      die "the network plan (${NETPLAN_FILE}) has no network ${net}. A new instance needs its two, edge_<instance> and cortex_<instance>, there and in stacks/edge.yml, stacks/cortex.yml and stacks/monitoring.yml (README.md section 15)"
    netplan_check "${net}" || die "${NETPLAN_DETAIL} (sudo bash ${BETULA_ROOT:-/opt/betula}/vps/30-docker.sh makes the networks of the plan)"
  done
}

# netmap_build - reads which networks swarm attaches every service to, from the service specs:
# what swarm was told, not what a container happens to have (a network a service left stays
# behind; "docker stack deploy --prune" only removes services).
netmap_build() {
  local line id name svc targets target
  local -A names=()
  NETMAP_SERVICES=() NETMAP_NETS=() NETMAP_ALL_SERVICES=()
  while read -r id name; do
    [[ -n "${id}" ]] && names["${id}"]="${name}"
  done < <(docker network ls --no-trunc --format '{{.ID}} {{.Name}}' 2>/dev/null || true)
  while IFS= read -r line; do
    [[ -n "${line}" ]] || continue
    svc="${line%% *}"
    targets="${line#* }"
    [[ "${targets}" != "${line}" ]] || targets=""
    NETMAP_ALL_SERVICES+=("${svc}")
    NETMAP_NETS["${svc}"]=""
    for target in ${targets}; do
      name="${names[${target}]:-${target}}"
      NETMAP_NETS["${svc}"]+="${name} "
      NETMAP_SERVICES["${name}"]="${NETMAP_SERVICES[${name}]:-}${svc} "
    done
  done < <(docker service ls -q 2>/dev/null | xargs -r docker service inspect --format '{{.Spec.Name}} {{range .Spec.TaskTemplate.Networks}}{{.Target}} {{end}}' 2>/dev/null || true)
}

# service_on_network SERVICE NET - true when swarm attaches SERVICE's tasks to NET.
service_on_network() {
  local id name
  for id in $(docker service inspect "$1" --format '{{range .Spec.TaskTemplate.Networks}}{{.Target}} {{end}}' 2>/dev/null || true); do
    name="$(docker network inspect "${id}" --format '{{.Name}}' 2>/dev/null || true)"
    [[ "${name}" == "$2" ]] && return 0
  done
  return 1
}

# network_used_by_others NET SERVICE... - true when a service other than the ones named is
# attached to NET (the old networks stay with Traefik and Cortex while anything else still uses
# them: stacks/edge.legacy.yml, stacks/cortex.legacy.yml).
network_used_by_others() {
  local net=$1 svc mine
  shift
  docker network inspect "${net}" >/dev/null 2>&1 || return 1
  [[ "${#NETMAP_ALL_SERVICES[@]}" -gt 0 ]] || netmap_build
  for svc in ${NETMAP_SERVICES[${net}]:-}; do
    for mine in "$@"; do
      [[ "${svc}" == "${mine}" ]] && continue 2
    done
    return 0
  done
  return 1
}

# ---------------------------------------------------------------- docker_gwbridge

# Docker's local bridge for swarm tasks: host-mode ports arrive on it (stacks/edge.yml), and every
# container with a network that is not internal goes out over it. Docker makes it by itself, IPv4
# only, the first time a task needs it, unless it exists already; it changes nothing of it later.
# With the plan's IPv6 subnet ("gwbridge" in files/networks.conf) a published port is reached over
# IPv6 by DNAT like over IPv4, and Traefik sees the visitor's own address instead of the gateway's
# that docker-proxy relays from. 30-docker.sh makes it (ensure_gwbridge), 90-verify-host.sh and
# deploy/test/gwbridge.sh check it (gwbridge_verify).
GWBRIDGE="docker_gwbridge"
# Set by gwbridge_verify / gwbridge_preflight: what is wrong, in words.
GWBRIDGE_DETAIL=""
# While gwbridge_swap has services at 0: "service=replicas" to restore, and the IPv4 the bridge had.
GWBRIDGE_SCALED=()
GWBRIDGE_OLD4="" GWBRIDGE_OLDGW="" GWBRIDGE_HOOKED=0

# gwbridge_wanted -> the IPv6 subnet docker_gwbridge should have; "-" for none (no line in the
# plan, no global IPv6 address here: nobody could reach it over IPv6, or GWBRIDGE_IPV6=off: the
# way back). After netplan_read.
gwbridge_wanted() {
  if [[ -n "${NETPLAN_GWBRIDGE6}" && "${GWBRIDGE_IPV6:-on}" != "off" ]] && ipv6_enabled && has_global_ipv6; then
    printf '%s' "${NETPLAN_GWBRIDGE6}"
  else
    printf -- '-'
  fi
}

# gwbridge_facts -> "<IPv4 subnet> <IPv4 gateway> <IPv6 subnet>" ("-" for what it lacks); nothing
# when the bridge does not exist.
gwbridge_facts() {
  local config subnet gateway v4=- gw=- v6=-
  config="$(docker network inspect "${GWBRIDGE}" --format '{{range .IPAM.Config}}{{.Subnet}} {{.Gateway}}{{println}}{{end}}' 2>/dev/null)" || return 0
  while read -r subnet gateway; do
    [[ -n "${subnet}" ]] || continue
    if [[ "${subnet}" == *:* ]]; then
      v6="${subnet}"
    else
      v4="${subnet}" gw="${gateway:--}"
    fi
  done <<<"${config}"
  if [[ "$(docker network inspect "${GWBRIDGE}" --format '{{.EnableIPv6}}' 2>/dev/null)" != "true" ]]; then v6=-; fi
  printf '%s %s %s' "${v4}" "${gw}" "${v6}"
}

# gwbridge_create IPV4 GATEWAY IPV6 - makes docker_gwbridge with the options Docker gives it itself
# (no traffic between its members, NAT on the way out); "-" for the IPv4 subnet takes one from the
# daemon's pool, "-" for IPv6 makes it IPv4 only.
gwbridge_create() {
  local -a flags=(--driver bridge)
  if [[ "$1" != "-" ]]; then
    flags+=(--subnet "$1")
    [[ "$2" == "-" ]] || flags+=(--gateway "$2")
  fi
  [[ "$3" == "-" ]] || flags+=(--ipv6 --subnet "$3")
  docker network create "${flags[@]}" \
    --opt com.docker.network.bridge.name="${GWBRIDGE}" \
    --opt com.docker.network.bridge.enable_icc=false \
    --opt com.docker.network.bridge.enable_ip_masquerade=true \
    --label "app.betula.managed-by=vps/30-docker.sh" "${GWBRIDGE}" >/dev/null
}

# gwbridge_members -> the endpoints on the bridge by name (gateway_<container>,
# gateway_ingress-sbox), space separated.
gwbridge_members() {
  docker network inspect "${GWBRIDGE}" --format '{{range $id, $c := .Containers}}{{$c.Name}} {{end}}' 2>/dev/null || true
}

# gwbridge_services -> every swarm service on the bridge, one per line: the ones with a task on it
# now, and the ones that would be as soon as a task starts (a network that is not internal). Dies
# on a container on it that belongs to no service: it would keep the bridge from going.
gwbridge_services() {
  local id name internal svc line net
  local -A found=() outward=()
  # Overlay networks only: a swarm task reaches docker_gwbridge through one that is not internal.
  # (The ingress network is never among a service's networks; its sandbox is Docker's own.)
  while read -r id name internal; do
    if [[ "${internal}" == "false" && "${name}" != "ingress" ]]; then
      outward["${id}"]=1
      outward["${name}"]=1
    fi
  done < <(docker network ls --no-trunc --filter driver=overlay --format '{{.ID}} {{.Name}} {{.Internal}}' 2>/dev/null || true)
  for id in $(docker network inspect "${GWBRIDGE}" --format '{{range $id, $c := .Containers}}{{$id}} {{end}}' 2>/dev/null || true); do
    [[ "${id}" == "ingress-sbox" ]] && continue
    svc="$(docker inspect --format '{{index .Config.Labels "com.docker.swarm.service.name"}}' "${id}" 2>/dev/null || true)"
    [[ -n "${svc}" ]] || die "container ${id:0:12} is on ${GWBRIDGE} and belongs to no swarm service: stop it first (docker ps)"
    found["${svc}"]=1
  done
  while read -r line; do
    [[ -n "${line}" ]] || continue
    svc="${line%% *}"
    for net in ${line#* }; do
      if [[ -n "${outward[${net}]:-}" ]]; then found["${svc}"]=1; fi
    done
  done < <(docker service ls -q 2>/dev/null | xargs -r docker service inspect --format '{{.Spec.Name}} {{range .Spec.TaskTemplate.Networks}}{{.Target}} {{end}}' 2>/dev/null || true)
  for svc in "${!found[@]}"; do printf '%s\n' "${svc}"; done | sort
}

# gwbridge_preflight WANTED - what must hold before the bridge gets IPv6; false with
# GWBRIDGE_DETAIL when something does not. Nothing is changed.
gwbridge_preflight() {
  local wanted=$1 mode dev ra
  GWBRIDGE_DETAIL=""
  [[ "${wanted}" != "-" ]] || return 0
  # Without a link-local address on the bridge the host cannot ask for a container's MAC address on
  # behalf of a forwarded packet (neighbour solicitations come from it): every IPv6 connection to a
  # published port would hang. Docker's bridges take the kernel's default; 1 means none (WSL has it).
  mode="$(sysctl -n net.ipv6.conf.default.addr_gen_mode 2>/dev/null || printf '?')"
  if [[ "${mode}" == "1" || "${mode}" == "?" ]]; then
    GWBRIDGE_DETAIL="net.ipv6.conf.default.addr_gen_mode is ${mode}: a new bridge would get no link-local address, and IPv6 to the containers would fail; set it to 0 first"
    return 1
  fi
  # Docker turns IPv6 forwarding on with the first IPv6 network, and a host that forwards ignores
  # router advertisements unless accept_ra is 2: a default route learned from them would expire.
  while read -r dev; do
    [[ -n "${dev}" ]] || continue
    ra="$(sysctl -n "net.ipv6.conf.${dev}.accept_ra" 2>/dev/null || printf '?')"
    if [[ "${ra}" != "2" ]]; then
      GWBRIDGE_DETAIL="the IPv6 default route on ${dev} comes from router advertisements, and with forwarding on (Docker turns it on) ${dev} would ignore them (accept_ra ${ra}, not 2): the host would lose its IPv6 route"
      return 1
    fi
  done < <(ip -6 route show default 2>/dev/null | awk '/ proto ra / { for (i = 1; i < NF; i++) if ($i == "dev") print $(i + 1) }' | sort -u)
  # Docker publishes a port on [::] only where it can listen on ::1, and it looks when dockerd
  # starts: ::1 given to lo later takes a restart of dockerd.
  if [[ -z "$(ip -6 -o addr show dev lo 2>/dev/null | grep -F '::1/128' || true)" ]]; then
    GWBRIDGE_DETAIL="lo has no ::1: Docker publishes no port on [::], and the bridge's IPv6 would serve nobody"
    return 1
  fi
  if ! ip6tables -w 10 -t nat -S >/dev/null 2>&1; then
    GWBRIDGE_DETAIL="ip6tables has no nat table here: Docker could not forward published ports over IPv6"
    return 1
  fi
  return 0
}

# gwbridge_verify WANTED - docker_gwbridge as wanted, and what IPv6 to the published ports needs;
# false with GWBRIDGE_DETAIL when something is not so. Read only.
gwbridge_verify() {
  local wanted=$1 facts v4 gw v6 port ports i
  GWBRIDGE_DETAIL=""
  facts="$(gwbridge_facts)"
  if [[ -z "${facts}" ]]; then
    GWBRIDGE_DETAIL="${GWBRIDGE} does not exist"
    return 1
  fi
  read -r v4 gw v6 <<<"${facts}"
  if [[ "${v6}" != "${wanted}" ]]; then
    GWBRIDGE_DETAIL="${GWBRIDGE} has IPv6 subnet ${v6}, the plan says ${wanted} (IPv4 ${v4})"
    return 1
  fi
  [[ "${wanted}" != "-" ]] || return 0
  # The kernel adds the link-local address when the first member's link comes up.
  if [[ -n "$(gwbridge_members)" ]]; then
    for i in $(seq 1 10); do
      [[ -z "$(ip -6 -o addr show dev "${GWBRIDGE}" scope link 2>/dev/null)" ]] || break
      sleep 1
    done
    if [[ -z "$(ip -6 -o addr show dev "${GWBRIDGE}" scope link 2>/dev/null)" ]]; then
      GWBRIDGE_DETAIL="${GWBRIDGE} has no link-local address: the host cannot reach its members over IPv6 on behalf of a visitor (net.ipv6.conf.${GWBRIDGE}.addr_gen_mode)"
      return 1
    fi
  fi
  if [[ "$(sysctl -n net.ipv6.conf.all.forwarding 2>/dev/null)" != "1" ]]; then
    GWBRIDGE_DETAIL="net.ipv6.conf.all.forwarding is not 1 (Docker sets it when it makes an IPv6 network; was it switched off since?)"
    return 1
  fi
  # Every port a task publishes on [::] needs its IPv6 DNAT rule: without it docker-proxy would take
  # the connection and Traefik would see the gateway again.
  ports="$(docker ps --format '{{.Ports}}' 2>/dev/null | tr ',' '\n' | sed -n 's/^ *\[::\]:\([0-9]*\)->.*/\1/p' | sort -u || true)"
  for port in ${ports}; do
    if ! ip6tables -w 10 -t nat -S DOCKER 2>/dev/null | grep -Eq -- "--dport ${port} -j DNAT --to-destination \[${v6%%::*}:"; then
      GWBRIDGE_DETAIL="port ${port} is published on [::], and ip6tables has no DNAT rule for it into ${v6}"
      return 1
    fi
  done
  return 0
}

# gwbridge_restore [RC] - after a swap that did not finish: the bridge back if it is missing (IPv4
# only, as it was), and every service scaled to 0 back to its replicas. An exit hook as well, so
# that no failure leaves the sites down; does nothing once the swap is done.
gwbridge_restore() {
  [[ "${#GWBRIDGE_SCALED[@]}" -gt 0 ]] || return 0
  if [[ -z "$(gwbridge_facts)" ]]; then
    warn "${GWBRIDGE} is missing: making it again, IPv4 only, as it was (${GWBRIDGE_OLD4})"
    gwbridge_create "${GWBRIDGE_OLD4:--}" "${GWBRIDGE_OLDGW:--}" - || warn "could not make ${GWBRIDGE}; Docker makes one itself (IPv4 only) for the first task"
  fi
  warn "scaling back: ${GWBRIDGE_SCALED[*]}"
  docker service scale --detach "${GWBRIDGE_SCALED[@]}" >/dev/null || warn "docker service scale ${GWBRIDGE_SCALED[*]} failed: do it by hand"
  GWBRIDGE_SCALED=()
}

# gwbridge_swap IPV6 - makes docker_gwbridge anew with the IPv6 subnet IPV6 ("-": IPv4 only) and
# the IPv4 subnet it has. Docker removes no network that has members, so every service on it is
# scaled to 0 first (gwbridge_services) and back afterwards. Services on internal networks only
# (Folia, Radix without egress, Prometheus, Loki, ...) keep running; the sites are away while
# Traefik is (a few seconds after its stop, see the log), Cortex while its two are.
gwbridge_swap() {
  local v6=$1 facts svc replicas left i
  local -a services=() targets=()
  facts="$(gwbridge_facts)"
  [[ -n "${facts}" ]] || die "${GWBRIDGE} does not exist; nothing to swap"
  read -r GWBRIDGE_OLD4 GWBRIDGE_OLDGW _ <<<"${facts}"
  mapfile -t services < <(gwbridge_services)
  for svc in ${services[@]+"${services[@]}"}; do
    replicas="$(docker service inspect --format '{{if .Spec.Mode.Replicated}}{{.Spec.Mode.Replicated.Replicas}}{{else}}-{{end}}' "${svc}")"
    [[ "${replicas}" =~ ^[0-9]+$ ]] || die "service ${svc} is on ${GWBRIDGE} and not replicated: it cannot be scaled to 0 for the swap"
    targets+=("${svc}=${replicas}")
  done
  if [[ "${GWBRIDGE_HOOKED}" -eq 0 ]]; then
    add_exit_hook gwbridge_restore
    GWBRIDGE_HOOKED=1
  fi
  GWBRIDGE_SCALED=(${targets[@]+"${targets[@]}"})
  if [[ "${#services[@]}" -gt 0 ]]; then
    log "to 0 for the swap: ${targets[*]}"
    docker service scale --detach "${services[@]/%/=0}" >/dev/null
  fi
  for i in $(seq 1 180); do
    left="$(gwbridge_members)"
    left="${left//gateway_ingress-sbox/}"
    [[ -n "${left// /}" ]] || break
    sleep 1
  done
  [[ -z "${left// /}" ]] || die "still on ${GWBRIDGE} after 3 minutes: ${left} (scaling back)"
  log "${GWBRIDGE} has no task left"
  # The ingress network's sandbox: Docker's own, reconnected to the new bridge by dockerd's next
  # start. Nothing here publishes through the routing mesh (host mode only), so it is not missed.
  if [[ " $(gwbridge_members) " == *" gateway_ingress-sbox "* ]]; then
    docker network disconnect --force "${GWBRIDGE}" gateway_ingress-sbox
  fi
  docker network rm "${GWBRIDGE}" >/dev/null
  if gwbridge_create "${GWBRIDGE_OLD4}" "${GWBRIDGE_OLDGW}" "${v6}"; then
    log "${GWBRIDGE} made anew: IPv4 ${GWBRIDGE_OLD4}, IPv6 ${v6}"
  else
    warn "docker network create refused ${GWBRIDGE} with IPv6 ${v6}; making it IPv4 only again"
    gwbridge_create "${GWBRIDGE_OLD4}" "${GWBRIDGE_OLDGW}" - || die "could not make ${GWBRIDGE} again"
  fi
  if [[ "${#services[@]}" -gt 0 ]]; then
    docker service scale --detach "${targets[@]}" >/dev/null
    log "scaled back: ${targets[*]}"
  fi
  GWBRIDGE_SCALED=()
  gwbridge_wait_running ${services[@]+"${services[@]}"}
}

# gwbridge_wait_running SERVICE... - waits up to 3 minutes until each service runs all its replicas;
# a warning for those that do not (the swap itself is done by then).
gwbridge_wait_running() {
  local svc line i pending
  for i in $(seq 1 180); do
    pending=""
    for svc in "$@"; do
      line="$(docker service ls --filter "name=${svc}" --format '{{.Name}} {{.Replicas}}' 2>/dev/null | awk -v s="${svc}" '$1 == s { print $2 }')"
      [[ "${line}" =~ ^([0-9]+)/([0-9]+)$ && "${BASH_REMATCH[1]}" == "${BASH_REMATCH[2]}" ]] || pending+="${svc} "
    done
    [[ -n "${pending}" ]] || return 0
    sleep 1
  done
  warn "not all replicas run after 3 minutes: ${pending% } (docker service ps ${pending%% *})"
}

# gwbridge_rebuild WANTED - the swap to WANTED, then gwbridge_verify; when that fails, the swap back
# to IPv4 only (the state from before, IPv6 through docker-proxy) and die. Run gwbridge_preflight
# first.
gwbridge_rebuild() {
  local wanted=$1 detail
  gwbridge_swap "${wanted}"
  if gwbridge_verify "${wanted}"; then
    log "${GWBRIDGE} as planned: $(gwbridge_facts)"
    return 0
  fi
  detail="${GWBRIDGE_DETAIL}"
  [[ "${wanted}" != "-" ]] || die "${detail}"
  warn "${detail}: back to IPv4 only"
  gwbridge_swap -
  die "${GWBRIDGE} is IPv4 only again, as before; the reason: ${detail}"
}

# ---------------------------------------------------------------- probes

# netprobe_as_root COMMAND... - runs it as root: directly as root, else through "sudo -n" (deploy
# has it).
netprobe_as_root() {
  if [[ "${EUID}" -eq 0 ]]; then
    "$@"
  else
    sudo -n "$@"
  fi
}

# netprobe_possible - true when this user can enter a container's network namespace.
netprobe_possible() {
  have_cmd nsenter && have_cmd curl && netprobe_as_root true 2>/dev/null
}

# container_pid CONTAINER -> the PID of its first process on this host (0 when it does not run).
container_pid() { docker inspect --format '{{.State.Pid}}' "$1" 2>/dev/null || printf '0'; }

# container_addresses CONTAINER -> "<network> <address>" per network of the running container.
container_addresses() {
  docker inspect --format '{{range $name, $net := .NetworkSettings.Networks}}{{$name}} {{$net.IPAddress}}{{println}}{{end}}' "$1" 2>/dev/null |
    awk 'NF == 2'
}

# netprobe_run PID - runs the probes read from stdin inside the network namespace of PID, with the
# host's bash and curl (nsenter -n changes nothing but the network): one line per probe,
#   <id> tcp <address> <port>                  -> "<id> open" or "<id> closed"
#   <id> http <address> <port> <path> [<from>] -> "<id> <status>" (000: no answer); from = the
#                                                 source address to use
# Every probe gives up after 2 seconds (curl: 3). Nothing is sent but a TCP handshake, or one GET.
netprobe_run() {
  local pid=$1
  # shellcheck disable=SC2016  # expanded by the bash inside the namespace
  netprobe_as_root nsenter -t "${pid}" -n -- bash -c '
    while read -r id kind address port path from; do
      [[ -n "${id}" ]] || continue
      case "${kind}" in
        tcp)
          if timeout 2 bash -c ": </dev/tcp/${address}/${port}" 2>/dev/null; then
            printf "%s open\n" "${id}"
          else
            printf "%s closed\n" "${id}"
          fi
          ;;
        http)
          code="$(curl -s --noproxy "*" -o /dev/null --max-time 3 ${from:+--interface "${from}"} -w "%{http_code}" "http://${address}:${port}${path}" 2>/dev/null || true)"
          printf "%s %s\n" "${id}" "${code:-000}"
          ;;
      esac
    done'
}
