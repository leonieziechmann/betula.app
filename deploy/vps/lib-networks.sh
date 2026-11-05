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
  NETPLAN_NETS=() NETPLAN_OLD=() NETPLAN_NOOUT=() NETPLAN_LISTENERS=()
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
      *) die "${file}:${n}: '${w[0]}' is no keyword of the plan (net, listen, noout, old)" ;;
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
