#!/usr/bin/env bash
# Helpers of Phloem, the service of Betula Sync (docs/phloem/phloem.md), shared by
# 47-phloem-host.sh (root: its file systems) and 49-phloem.sh (deploy: its stack). Sourced AFTER
# lib.sh and lib-stacks.sh, never executed.
# shellcheck shell=bash

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  echo "lib-phloem.sh is a library; source it after lib.sh and lib-stacks.sh" >&2
  exit 2
fi

# The instances: one file each, stacks/phloem/<stack>.env. Not stacks/<stack>.env: every file
# there is an instance of the application (lib-stacks.sh, instance_names).
PHLOEM_ENV_DIR="${STACKS_DIR}/phloem"
PHLOEM_STACK_FILE="${STACKS_DIR}/phloem.yml"
# The user Phloem runs as in its image (flake.nix: "phloem start" drops to it before it serves).
PHLOEM_UID=65532
# The tmpfs of the stats file: what one file can bring to the monitoring is bounded by it.
PHLOEM_STATS_SIZE="1m"

# Set by load_phloem, of one instance.
PHLOEM_STACK=""
PHLOEM_HOST=""
PHLOEM_ORIGIN_LIST=""
PHLOEM_BUDGET=""
PHLOEM_SIZE=""
PHLOEM_SIZE_BYTES=""
# TURN (step 2, docs/phloem/phloem.md, „TURN"): on where PHLOEM_TURN_PORT is set.
PHLOEM_TURN_PORT=""   # STUN and TURN, UDP and TCP, published in host mode at the same number
PHLOEM_TURN_RELAYS="" # first-last, the relays' UDP ports, published in host mode
PHLOEM_TURN_TLS=""    # the name of TURN over TLS on 443 (Traefik, by SNI); empty: none
PHLOEM_TURN_IP=""     # the host's public IPv4 as the file names it; empty: phloem_turn_ip finds it
# At most this many relay ports per instance: each is a docker-proxy on the host.
PHLOEM_MAX_RELAYS=128

# phloem_names -> the instances that have a file stacks/phloem/<stack>.env, one per line.
phloem_names() {
  local file
  for file in "${PHLOEM_ENV_DIR}"/*.env; do
    [[ -f "${file}" ]] || continue
    file="${file##*/}"
    printf '%s\n' "${file%.env}"
  done
}

# size_bytes SIZE -> the bytes of 512M, 56G or 1T (powers of 1024, as truncate and the file
# system count them); nothing when SIZE is not of that form.
size_bytes() {
  [[ "$1" =~ ^([1-9][0-9]{0,5})([MGT])$ ]] || return 0
  local n=${BASH_REMATCH[1]}
  case "${BASH_REMATCH[2]}" in
    M) printf '%s' $((n << 20)) ;;
    G) printf '%s' $((n << 30)) ;;
    T) printf '%s' $((n << 40)) ;;
  esac
}

# load_phloem NAME - reads stacks/phloem/NAME.env into PHLOEM_STACK, PHLOEM_HOST,
# PHLOEM_ORIGIN_LIST, PHLOEM_BUDGET, PHLOEM_SIZE and PHLOEM_SIZE_BYTES. The file is read, never
# sourced: a value is data, whatever it looks like.
load_phloem() {
  local name=$1 file line key value origin
  file="${PHLOEM_ENV_DIR}/${name}.env"
  # The name is its stack, its network <stack>_in, its router <stack>-sync and its host paths:
  # "phloem" in front keeps all of them apart from every other stack's.
  [[ "${name}" =~ ^phloem(-[a-z0-9]+)*$ && "${#name}" -le 30 ]] ||
    die "'${name}' is not a name of an instance of Phloem (phloem, or phloem-<lower case letters and digits>)"
  [[ -f "${file}" ]] ||
    die "there is no instance '${name}' of Phloem: ${file} does not exist (known: $(phloem_names | tr '\n' ' '))"
  assert_lf "${file}"
  PHLOEM_STACK=""
  PHLOEM_HOST=""
  PHLOEM_ORIGIN_LIST=""
  PHLOEM_BUDGET=""
  PHLOEM_SIZE=""
  PHLOEM_TURN_PORT=""
  PHLOEM_TURN_RELAYS=""
  PHLOEM_TURN_TLS=""
  PHLOEM_TURN_IP=""
  while IFS= read -r line || [[ -n "${line}" ]]; do
    [[ -n "${line}" && "${line}" != \#* ]] || continue
    [[ "${line}" == *=* ]] || die "${file}: '${line}' is not NAME=value"
    key="${line%%=*}"
    value="${line#*=}"
    case "${key}" in
      STACK_NAME) PHLOEM_STACK="${value}" ;;
      APP_HOST) PHLOEM_HOST="${value}" ;;
      PHLOEM_ORIGINS) PHLOEM_ORIGIN_LIST="${value}" ;;
      PHLOEM_BUDGET_BYTES) PHLOEM_BUDGET="${value}" ;;
      PHLOEM_DATA_SIZE) PHLOEM_SIZE="${value}" ;;
      PHLOEM_TURN_PORT) PHLOEM_TURN_PORT="${value}" ;;
      PHLOEM_TURN_RELAY_PORTS) PHLOEM_TURN_RELAYS="${value}" ;;
      PHLOEM_TURN_TLS_HOST) PHLOEM_TURN_TLS="${value}" ;;
      PHLOEM_TURN_IP) PHLOEM_TURN_IP="${value}" ;;
      *) die "${file}: unknown setting ${key} (known: STACK_NAME, APP_HOST, PHLOEM_ORIGINS, PHLOEM_BUDGET_BYTES, PHLOEM_DATA_SIZE, PHLOEM_TURN_PORT, PHLOEM_TURN_RELAY_PORTS, PHLOEM_TURN_TLS_HOST, PHLOEM_TURN_IP)" ;;
    esac
  done <"${file}"
  [[ "${PHLOEM_STACK}" == "${name}" ]] ||
    die "${file}: STACK_NAME is '${PHLOEM_STACK}', the file says '${name}'; they have to agree"
  [[ "${PHLOEM_HOST}" =~ ^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,}$ ]] ||
    die "${file}: APP_HOST '${PHLOEM_HOST}' is not a host name"
  [[ -n "${PHLOEM_ORIGIN_LIST}" ]] || die "${file}: PHLOEM_ORIGINS is empty (the site's own origin: https://${PHLOEM_HOST})"
  for origin in ${PHLOEM_ORIGIN_LIST//,/ }; do
    [[ "${origin}" =~ ^https?://([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)*[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(:[0-9]{1,5})?$ ]] ||
      die "${file}: PHLOEM_ORIGINS names '${origin}', which is no origin (scheme://host[:port], lower case, comma-separated, no spaces)"
  done
  [[ ",${PHLOEM_ORIGIN_LIST}," == *",https://${PHLOEM_HOST},"* ]] ||
    warn "${file}: PHLOEM_ORIGINS (${PHLOEM_ORIGIN_LIST}) does not name the site's own origin https://${PHLOEM_HOST}: its pages could not use Phloem"
  [[ "${PHLOEM_BUDGET}" =~ ^[1-9][0-9]{6,14}$ ]] ||
    die "${file}: PHLOEM_BUDGET_BYTES '${PHLOEM_BUDGET}' is not a number of bytes (at least 1000000)"
  PHLOEM_SIZE_BYTES="$(size_bytes "${PHLOEM_SIZE}")"
  [[ -n "${PHLOEM_SIZE_BYTES}" ]] || die "${file}: PHLOEM_DATA_SIZE '${PHLOEM_SIZE}' is not a size like 3G or 512M"
  # The budget is Phloem's own limit, the file system the one it cannot pass: at the budget it
  # says 507 to new spaces and parcels; on a full file system every write would fail.
  [[ "${PHLOEM_BUDGET}" -le $((PHLOEM_SIZE_BYTES * 95 / 100)) ]] ||
    die "${file}: PHLOEM_BUDGET_BYTES (${PHLOEM_BUDGET}) leaves less than 5 % of PHLOEM_DATA_SIZE (${PHLOEM_SIZE}) for SQLite's journal and pages; make the file system larger or the budget smaller"
  if [[ -z "${PHLOEM_TURN_PORT}" ]]; then
    [[ -z "${PHLOEM_TURN_RELAYS}${PHLOEM_TURN_TLS}${PHLOEM_TURN_IP}" ]] ||
      die "${file}: TURN settings without PHLOEM_TURN_PORT (TURN is on where the port is set)"
    return 0
  fi
  [[ "${PHLOEM_TURN_PORT}" =~ ^[0-9]{4,5}$ && "${PHLOEM_TURN_PORT}" -ge 1024 && "${PHLOEM_TURN_PORT}" -le 65535 ]] ||
    die "${file}: PHLOEM_TURN_PORT '${PHLOEM_TURN_PORT}' is not a port of 1024-65535 (3478: the one of STUN and TURN)"
  [[ "${PHLOEM_TURN_RELAYS}" =~ ^([0-9]{4,5})-([0-9]{4,5})$ && "${BASH_REMATCH[1]}" -ge 1024 && "${BASH_REMATCH[2]}" -le 65535 &&
    "${BASH_REMATCH[1]}" -le "${BASH_REMATCH[2]}" ]] ||
    die "${file}: PHLOEM_TURN_RELAY_PORTS '${PHLOEM_TURN_RELAYS}' is not first-last within 1024-65535"
  [[ $((${PHLOEM_TURN_RELAYS#*-} - ${PHLOEM_TURN_RELAYS%-*} + 1)) -le "${PHLOEM_MAX_RELAYS}" ]] ||
    die "${file}: PHLOEM_TURN_RELAY_PORTS names more than ${PHLOEM_MAX_RELAYS} ports (each one is published, a docker-proxy of the host)"
  [[ "${PHLOEM_TURN_PORT}" -lt "${PHLOEM_TURN_RELAYS%-*}" || "${PHLOEM_TURN_PORT}" -gt "${PHLOEM_TURN_RELAYS#*-}" ]] ||
    die "${file}: PHLOEM_TURN_PORT ${PHLOEM_TURN_PORT} lies among the relays' ports ${PHLOEM_TURN_RELAYS}"
  [[ -z "${PHLOEM_TURN_TLS}" || "${PHLOEM_TURN_TLS}" =~ ^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,}$ ]] ||
    die "${file}: PHLOEM_TURN_TLS_HOST '${PHLOEM_TURN_TLS}' is not a host name"
  [[ -z "${PHLOEM_TURN_TLS}" || "${PHLOEM_TURN_TLS}" != "${PHLOEM_HOST}" ]] ||
    die "${file}: PHLOEM_TURN_TLS_HOST is the site's own host: Traefik would hand all of its TLS to TURN (a name of its own: turn.${PHLOEM_HOST})"
  [[ -z "${PHLOEM_TURN_IP}" ]] || public_ipv4 "${PHLOEM_TURN_IP}" ||
    die "${file}: PHLOEM_TURN_IP '${PHLOEM_TURN_IP}' is not a public IPv4 address"
}

# public_ipv4 ADDR - true for an IPv4 address that is one on the internet: none of the private,
# shared, loopback, link-local, documentation, benchmarking or multicast ranges (as Phloem's own
# rule, phloem/internal/server/turn.go).
public_ipv4() {
  local a b c d
  [[ "$1" =~ ^([0-9]{1,3})\.([0-9]{1,3})\.([0-9]{1,3})\.([0-9]{1,3})$ ]] || return 1
  a=${BASH_REMATCH[1]} b=${BASH_REMATCH[2]} c=${BASH_REMATCH[3]} d=${BASH_REMATCH[4]}
  [[ "${a}" -le 255 && "${b}" -le 255 && "${c}" -le 255 && "${d}" -le 255 ]] || return 1
  case "${a}" in
    0 | 10 | 127) return 1 ;;
    100) [[ "${b}" -lt 64 || "${b}" -gt 127 ]] || return 1 ;;
    169) [[ "${b}" != 254 ]] || return 1 ;;
    172) [[ "${b}" -lt 16 || "${b}" -gt 31 ]] || return 1 ;;
    192)
      [[ "${b}" != 168 ]] || return 1
      case "${b}.${c}" in 0.0 | 0.2 | 31.196 | 52.193 | 88.99 | 175.48) return 1 ;; esac
      ;;
    198) [[ "${b}" != 18 && "${b}" != 19 && "${b}.${c}" != 51.100 ]] || return 1 ;;
    203) [[ "${b}.${c}" != 0.113 ]] || return 1 ;;
    *) [[ "${a}" -lt 224 ]] || return 1 ;;
  esac
  return 0
}

# phloem_turn_ip -> the host's public IPv4 address for TURN's relays: the file's, else the source
# address of the route to the internet. Nothing when that is no public address (a host behind NAT:
# PHLOEM_TURN_IP in stacks/phloem/<stack>.env then).
phloem_turn_ip() {
  local addr="${PHLOEM_TURN_IP}"
  if [[ -z "${addr}" ]] && have_cmd ip; then
    addr="$(ip -4 -o route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p' | head -n 1)"
  fi
  if public_ipv4 "${addr}"; then printf '%s' "${addr}"; fi
}

# phloem_turn_ports -> what an instance with TURN publishes, one "port/proto" per line, in the
# form of docker service inspect: its port over UDP and TCP, and every relay over UDP.
phloem_turn_ports() {
  [[ -n "${PHLOEM_TURN_PORT}" ]] || return 0
  printf '%s/udp\n%s/tcp\n' "${PHLOEM_TURN_PORT}" "${PHLOEM_TURN_PORT}"
  seq "${PHLOEM_TURN_RELAYS%-*}" "${PHLOEM_TURN_RELAYS#*-}" | sed 's|$|/udp|'
}

# phloem_turn_file FILE IP - writes what the stack file gets beside it for TURN, for the instance
# loaded: STUN's and TURN's settings, the ports in host mode (Phloem sees a device's own address,
# as Traefik does: a relay must know where its datagrams come from), the network <stack>_out (its
# way out: the relays' datagrams to the devices, which its firewall lets out alone), and with a
# name for TLS, Traefik's TCP router for it. docker stack deploy takes it as a second -c.
phloem_turn_file() {
  local file=$1 ip=$2 port
  {
    printf '# Made by vps/49-phloem.sh from stacks/phloem/%s.env (TURN, step 2): not to be edited.\n' "${PHLOEM_STACK}"
    printf 'services:\n  sync:\n    environment:\n'
    printf '      PHLOEM_TURN_HOST: "%s"\n      PHLOEM_TURN_PORT: "%s"\n' "${PHLOEM_HOST}" "${PHLOEM_TURN_PORT}"
    printf '      PHLOEM_TURN_IP: "%s"\n      PHLOEM_TURN_RELAY_PORTS: "%s"\n' "${ip}" "${PHLOEM_TURN_RELAYS}"
    printf '      PHLOEM_TURN_TLS_HOST: "%s"\n' "${PHLOEM_TURN_TLS}"
    printf '    networks:\n      out: {}\n    ports:\n'
    printf '      - {target: %s, published: %s, protocol: udp, mode: host}\n' "${PHLOEM_TURN_PORT}" "${PHLOEM_TURN_PORT}"
    printf '      - {target: %s, published: %s, protocol: tcp, mode: host}\n' "${PHLOEM_TURN_PORT}" "${PHLOEM_TURN_PORT}"
    for port in $(seq "${PHLOEM_TURN_RELAYS%-*}" "${PHLOEM_TURN_RELAYS#*-}"); do
      printf '      - {target: %s, published: %s, protocol: udp, mode: host}\n' "${port}" "${port}"
    done
    if [[ -n "${PHLOEM_TURN_TLS}" ]]; then
      printf '    deploy:\n      labels:\n'
      printf '        - "traefik.tcp.routers.%s-turn.rule=HostSNI(`%s`)"\n' "${PHLOEM_STACK}" "${PHLOEM_TURN_TLS}"
      printf '        - "traefik.tcp.routers.%s-turn.entrypoints=websecure"\n' "${PHLOEM_STACK}"
      printf '        - "traefik.tcp.routers.%s-turn.tls.certresolver=le"\n' "${PHLOEM_STACK}"
      printf '        - "traefik.tcp.routers.%s-turn.service=%s-turn"\n' "${PHLOEM_STACK}" "${PHLOEM_STACK}"
      printf '        - "traefik.tcp.services.%s-turn.loadbalancer.server.port=%s"\n' "${PHLOEM_STACK}" "${PHLOEM_TURN_PORT}"
    fi
    printf 'networks:\n  out:\n    driver: overlay\n'
  } >"${file}"
}

# stun_answers ADDR PORT - true when a STUN binding request to ADDR:PORT over UDP gets a binding
# response within 3 s (bash's /dev/udp: no tool of its own).
stun_answers() {
  local reply
  reply="$(timeout 3 bash -c 'exec 3<>"/dev/udp/$0/$1" && printf "\x00\x01\x00\x00\x21\x12\xa4\x42betulaverify" >&3 && head -c 4 <&3 | od -An -tx1' "$1" "$2" 2>/dev/null | tr -d ' \n' || true)"
  [[ "${reply}" == 0101* ]]
}

# The host paths of an instance (stacks/phloem.yml, docs/phloem/protocol.md section 1).
# phloem_dir STACK -> the directory of the instance: the image of its file system and the place
# it is mounted.
phloem_dir() { printf '/var/lib/betula/phloem/%s' "$1"; }
# phloem_image_file STACK -> the file that holds its file system (a sparse file, on a loop device).
phloem_image_file() { printf '%s/data.img' "$(phloem_dir "$1")"; }
# phloem_data_dir STACK -> where that file system is mounted: /data in the container.
phloem_data_dir() { printf '%s/data' "$(phloem_dir "$1")"; }
# phloem_stats_dir STACK -> the tmpfs of its stats file: /stats in the container; Alloy reads it
# through /rootfs.
phloem_stats_dir() { printf '/run/betula/phloem/%s/stats' "$1"; }

# phloem_mount_facts PATH -> "<fstype> <options> <uid> <mode>" of the file system mounted at
# exactly PATH (nothing when none is: a directory of another file system does not count).
phloem_mount_facts() {
  local line
  line="$(findmnt -n -M "$1" -o FSTYPE,OPTIONS 2>/dev/null || true)"
  [[ -n "${line}" ]] || return 0
  printf '%s %s' "${line}" "$(stat -c '%u %a' "$1")"
}

# phloem_check_mounts STACK - true when both file systems of the instance are in place: its data
# on an ext4 of its own, the stats on a tmpfs of its own, both nosuid,nodev,noexec and Phloem's.
# Otherwise false, with the reason in PHLOEM_MOUNT_DETAIL.
PHLOEM_MOUNT_DETAIL=""
phloem_check_mounts() {
  local stack=$1 path facts fstype options uid mode want_fs want_mode opt
  for path in "$(phloem_data_dir "${stack}")" "$(phloem_stats_dir "${stack}")"; do
    # The data are Phloem's alone; the stats file is read by Alloy (as root, through /rootfs).
    if [[ "${path}" == /var/* ]]; then
      want_fs="ext4" want_mode="700"
    else
      want_fs="tmpfs" want_mode="755"
    fi
    facts="$(phloem_mount_facts "${path}")"
    if [[ -z "${facts}" ]]; then
      PHLOEM_MOUNT_DETAIL="nothing is mounted at ${path} (sudo bash ${BETULA_ROOT}/vps/47-phloem-host.sh ${stack})"
      return 1
    fi
    read -r fstype options uid mode <<<"${facts}"
    if [[ "${fstype}" != "${want_fs}" ]]; then
      PHLOEM_MOUNT_DETAIL="${path} is ${fstype}, wanted ${want_fs} (47-phloem-host.sh)"
      return 1
    fi
    for opt in nosuid nodev noexec; do
      if [[ ",${options}," != *",${opt},"* ]]; then
        PHLOEM_MOUNT_DETAIL="${path} is mounted without ${opt} (${options})"
        return 1
      fi
    done
    if [[ "${uid}" != "${PHLOEM_UID}" || "${mode}" != "${want_mode}" ]]; then
      PHLOEM_MOUNT_DETAIL="${path} belongs to uid ${uid} with mode ${mode}, wanted ${PHLOEM_UID} (Phloem's user) and ${want_mode} (47-phloem-host.sh)"
      return 1
    fi
    if [[ "${want_fs}" == "tmpfs" && ",${options}," != *",size=1024k,"* ]]; then
      PHLOEM_MOUNT_DETAIL="${path} is a tmpfs of another size (${options}), wanted 1 MiB"
      return 1
    fi
  done
  PHLOEM_MOUNT_DETAIL=""
}
