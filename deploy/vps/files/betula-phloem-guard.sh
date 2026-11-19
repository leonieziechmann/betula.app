#!/usr/bin/env bash
# Installed by /opt/betula/vps/30-docker.sh as /usr/local/sbin/betula-phloem-guard. Run every
# minute by betula-phloem-guard.timer, as root; by hand: sudo betula-phloem-guard && cat the file.
# The same script looks at Xylem, the notifications' service, which is kept the same way
# (docs/xylem/xylem.md, „On the server"): betula-xylem-guard.timer runs it with
# BETULA_GUARD_KIND=xylem; by hand: sudo BETULA_GUARD_KIND=xylem betula-phloem-guard. Below,
# <kind> is phloem or xylem.
#
# Phloem, Betula Sync's service (docs/phloem/protocol.md), is kept as if it were an outside one:
# networks of its own, and in its container a firewall of its own (table inet phloem: no DNS, no
# new connection out, replies only), set by "phloem start" before it drops to uid 65532 without
# capabilities. (Xylem's, table inet xylem, lets out a name asked of Docker's resolver and port
# 443 of an address on the internet.) This looks at that from outside, from the kernel's side,
# and writes what it sees for the monitoring (Alloy reads /run/betula/metrics through /rootfs;
# config.alloy, job phloem-guard; dashboard "Sync"; rules of groups betula-phloem and
# betula-xylem):
#
#   betula_<kind>_guard_containers{stack}            running containers of the stack here
#   betula_<kind>_guard_firewall{stack}              1: every one of them has the table inet <kind>
#                                                    in its network namespace
#   betula_<kind>_guard_privileged{stack}            1: a process of one of them runs as root or
#                                                    holds a capability
#   betula_<kind>_guard_counted_rules{stack,chain}   the table's rules that drop or reject and count
#   betula_<kind>_guard_dropped_packets_total{stack,chain}  what they counted, summed over the
#                                                    containers (starts at 0 with every container)
#   betula_<kind>_guard_last_run_timestamp_seconds   when this ran
#
# Drops that no rule counts (a chain's policy) are not seen: the table counts what it drops.
# It only reads ("nft list" in the container's network namespace, /proc), never trusts what the
# container says about itself, and writes nothing but its own file: in a directory of root's that
# no container mounts, so that neither service can write the numbers that watch it.
set -Eeuo pipefail
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

# Which service it looks at: its table inet <kind>, its stacks, and the names of what it writes.
KIND="${BETULA_GUARD_KIND:-phloem}"
case "${KIND}" in
  phloem) STACKS=(phloem phloem-canary) ;;
  xylem) STACKS=(xylem xylem-canary) ;;
  *)
    printf 'betula-phloem-guard: BETULA_GUARD_KIND is "%s", not phloem or xylem\n' "${KIND}" >&2
    exit 2
    ;;
esac
OUT_DIR="${BETULA_GUARD_DIR:-/run/betula/metrics}"
OUT="${OUT_DIR}/${KIND}-guard.prom"
M="betula_${KIND}_guard"

say() { printf 'betula-%s-guard: %s\n' "${KIND}" "$*" >&2; }

# drops PID -> "<chain> <rules> <packets>" per chain of table inet ${KIND} that has a counting rule
# with a drop or reject verdict (anonymous counters and named ones alike); fails when the table
# is not there.
drops() {
  local json
  json="$(nsenter -t "$1" -n -- nft -j list table inet "${KIND}" 2>/dev/null)" || return 1
  jq -r '
    (reduce (.nftables[] | select(type == "object" and has("counter")) | .counter) as $c ({}; .[$c.name] = $c.packets)) as $named
    | [ .nftables[] | select(type == "object" and has("rule")) | .rule
        | select(any(.expr[]?; type == "object" and (has("drop") or has("reject"))))
        | . as $rule
        | [ .expr[]? | select(type == "object" and has("counter")) | .counter
            | if type == "object" then (.packets // 0) elif type == "string" then ($named[.] // 0) else 0 end ]
        | select(length > 0)
        | {chain: $rule.chain, packets: add} ]
    | group_by(.chain)[]
    | "\(.[0].chain) \(length) \(map(.packets) | add)"' <<<"${json}"
}

# privileged PID - true when a process of the container that PID belongs to runs as root (real or
# effective uid 0) or holds a capability (effective or permitted). Its processes are those of its
# cgroup; where that cannot be read, PID alone.
privileged() {
  local pid=$1 cgroup procs p status uid caps
  cgroup="$(sed -n 's/^0:://p' "/proc/${pid}/cgroup" 2>/dev/null || true)"
  procs="${pid}"
  if [[ -n "${cgroup}" && -r "/sys/fs/cgroup${cgroup}/cgroup.procs" ]]; then
    procs="$(cat "/sys/fs/cgroup${cgroup}/cgroup.procs")"
  fi
  for p in ${procs}; do
    status="$(cat "/proc/${p}/status" 2>/dev/null)" || continue
    uid="$(awk '$1 == "Uid:" { print $2 " " $3 }' <<<"${status}")"
    caps="$(awk '$1 == "CapEff:" || $1 == "CapPrm:" { print $2 }' <<<"${status}" | tr -d '0\n')"
    if [[ " ${uid} " == *" 0 "* || -n "${caps}" ]]; then
      return 0
    fi
  done
  return 1
}

main() {
  local stack ids id pid firewall privileged_any containers line chain rules packets now tmp
  local -A rules_of=() packets_of=()
  local -a lines=()
  now="$(date +%s)"
  lines+=("# HELP ${M}_containers Running containers of the stack on this host.")
  lines+=("# TYPE ${M}_containers gauge")
  local -a firewall_lines=() privileged_lines=() counted_lines=() dropped_lines=()
  for stack in "${STACKS[@]}"; do
    ids="$(docker ps -q --no-trunc --filter "label=com.docker.stack.namespace=${stack}" --filter status=running 2>/dev/null)" || {
      say "docker does not answer"
      ids=""
    }
    containers=0
    firewall=1
    privileged_any=0
    rules_of=() packets_of=()
    for id in ${ids}; do
      pid="$(docker inspect --format '{{.State.Pid}}' "${id}" 2>/dev/null || printf '0')"
      [[ "${pid}" =~ ^[1-9][0-9]*$ ]] || continue
      containers=$((containers + 1))
      if privileged "${pid}"; then privileged_any=1; fi
      if ! line="$(drops "${pid}")"; then
        firewall=0
        continue
      fi
      while read -r chain rules packets; do
        [[ -n "${chain}" && "${chain}" =~ ^[A-Za-z0-9_-]+$ && "${rules}" =~ ^[0-9]+$ && "${packets}" =~ ^[0-9]+$ ]] || continue
        rules_of["${chain}"]="${rules}"
        packets_of["${chain}"]=$((${packets_of[${chain}]:-0} + packets))
      done <<<"${line}"
    done
    [[ "${containers}" -gt 0 ]] || firewall=0
    lines+=("${M}_containers{stack=\"${stack}\"} ${containers}")
    if [[ "${containers}" -gt 0 ]]; then
      firewall_lines+=("${M}_firewall{stack=\"${stack}\"} ${firewall}")
      privileged_lines+=("${M}_privileged{stack=\"${stack}\"} ${privileged_any}")
      for chain in "${!rules_of[@]}"; do
        counted_lines+=("${M}_counted_rules{stack=\"${stack}\",chain=\"${chain}\"} ${rules_of[${chain}]}")
        dropped_lines+=("${M}_dropped_packets_total{stack=\"${stack}\",chain=\"${chain}\"} ${packets_of[${chain}]}")
      done
    fi
  done
  lines+=("# HELP ${M}_firewall 1 when every running container of the stack has the table inet ${KIND}.")
  lines+=("# TYPE ${M}_firewall gauge")
  lines+=(${firewall_lines[@]+"${firewall_lines[@]}"})
  lines+=("# HELP ${M}_privileged 1 when a process of a container of the stack runs as root or holds a capability.")
  lines+=("# TYPE ${M}_privileged gauge")
  lines+=(${privileged_lines[@]+"${privileged_lines[@]}"})
  lines+=("# HELP ${M}_counted_rules Rules of table inet ${KIND} that drop or reject and count it.")
  lines+=("# TYPE ${M}_counted_rules gauge")
  lines+=(${counted_lines[@]+"${counted_lines[@]}"})
  lines+=("# HELP ${M}_dropped_packets_total Packets those rules dropped, per chain.")
  lines+=("# TYPE ${M}_dropped_packets_total counter")
  lines+=(${dropped_lines[@]+"${dropped_lines[@]}"})
  lines+=("# HELP ${M}_last_run_timestamp_seconds When the guard last looked.")
  lines+=("# TYPE ${M}_last_run_timestamp_seconds gauge")
  lines+=("${M}_last_run_timestamp_seconds ${now}")

  install -d -m 0755 "${OUT_DIR}"
  tmp="$(mktemp "${OUT_DIR}/.${KIND}-guard.XXXXXX")"
  printf '%s\n' "${lines[@]}" >"${tmp}"
  chmod 0644 "${tmp}"
  # A rename: Alloy never reads half a file.
  mv -f "${tmp}" "${OUT}"
}

main "$@"
