#!/usr/bin/env bash
# turn.sh - Betula Sync's live way across two networks behind NAT (step 2, docs/phloem/phloem.md,
# „TURN"), in network namespaces of this machine: two homes behind NATs that give every new
# destination a port of its own (no direct way between them), Phloem and Folia on the side of the
# "internet". Run as root, on a throwaway machine (or a session sandbox): never on the server.
#
#   sudo PHLOEM_BIN=… FOLIA_BIN=… FOLIA_DATA_DIR=… bash deploy/test/turn.sh
#
#   PHLOEM_BIN      a phloem built from phloem/ (go build -o … ./cmd/phloem)
#   FOLIA_BIN       Folia's server with the browser app (scripts/build-client.sh --dev, cargo build)
#   FOLIA_DATA_DIR  a directory with a catalog snapshot Folia keeps (current.json and its file)
#   BROWSER         Chromium's headless shell (default: Playwright's under /opt/pw-browsers)
#
#   wan     a bridge, the "internet": 11.0.0.0/24 (public addresses as far as Phloem can tell,
#           in namespaces that reach nothing else)
#   srv     Phloem at 11.0.0.1: HTTP 8200, STUN and TURN 3478, relays 49160-49167; started by
#           `phloem start` as in its image: its firewall with TURN's rules, then 65532
#   web     Folia at 11.0.0.2:8080 (FOLIA_SYNC_URL: 127.0.0.1:8200 of each home, forwarded)
#   nata    a home router at 11.0.0.11, NAT for 10.1.0.0/24 (masquerade fully-random: a port of
#           its own for every destination, as a mobile network's CGNAT); homea: a browser at 10.1.0.2
#   natb    the same at 11.0.0.12 for 10.2.0.0/24; homeb: a browser at 10.2.0.2
#
# Then folia/e2e/turn.mjs: A and B, one profile, find no direct way (STUN alone fails), connect
# through Phloem's TURN, and a change goes over it alone. Afterwards `phloem probe` in srv (what the
# firewall forbids, from a relay's port too) and the firewall's counters: no counted drop.
set -Eeuo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${TEST_DIR}/../.." && pwd)"
PHLOEM_BIN="${PHLOEM_BIN:?PHLOEM_BIN: a phloem built from phloem/}"
FOLIA_BIN="${FOLIA_BIN:?FOLIA_BIN: the server of Folia}"
FOLIA_DATA_DIR="${FOLIA_DATA_DIR:?FOLIA_DATA_DIR: a directory with a catalog snapshot}"
BROWSER="${BROWSER:-$(ls -d /opt/pw-browsers/chromium_headless_shell-*/chrome-headless-shell-linux64/chrome-headless-shell 2>/dev/null | tail -n 1)}"
[[ "$(id -u)" == "0" ]] || { echo "turn.sh: run as root" >&2; exit 2; }
for cmd in ip nft node; do command -v "${cmd}" >/dev/null || { echo "turn.sh: ${cmd} is missing" >&2; exit 2; }; done
[[ -x "${BROWSER}" ]] || { echo "turn.sh: no browser at '${BROWSER}' (BROWSER)" >&2; exit 2; }

NS=(bt-wan bt-srv bt-web bt-nata bt-homea bt-natb bt-homeb)
TMP="$(mktemp -d)"
PIDS=()
say() { printf 'turn.sh: %s\n' "$*"; }
down() {
  for pid in "${PIDS[@]}"; do kill "${pid}" 2>/dev/null || true; done
  wait 2>/dev/null || true
  for ns in "${NS[@]}"; do ip netns del "${ns}" 2>/dev/null || true; done
  rm -rf "${TMP}"
}
trap down EXIT
for ns in "${NS[@]}"; do ip netns del "${ns}" 2>/dev/null || true; done

# ---- the namespaces ----
for ns in "${NS[@]}"; do
  ip netns add "${ns}"
  ip -n "${ns}" link set lo up
  # IPv4 alone, as in Phloem's container: no IPv6 chatter for the firewall to count.
  ip netns exec "${ns}" sysctl -q -w net.ipv6.conf.all.disable_ipv6=1 net.ipv6.conf.default.disable_ipv6=1 2>/dev/null || true
done
ip -n bt-wan link add br0 type bridge
ip -n bt-wan link set br0 up
# wan NS ADDR - NS on the "internet" at ADDR/24, its interface e0
wan() {
  ip link add "w-$1" netns bt-wan type veth peer name e0 netns "$1"
  ip -n bt-wan link set "w-$1" master br0 up
  ip -n "$1" addr add "$2/24" dev e0
  ip -n "$1" link set e0 up
}
wan bt-srv 11.0.0.1
wan bt-web 11.0.0.2
wan bt-nata 11.0.0.11
wan bt-natb 11.0.0.12
# home NAT HOME NET - HOME behind NAT at NET.2, its router NET.1
home() {
  ip link add h0 netns "$2" type veth peer name h1 netns "$1"
  ip -n "$1" addr add "$3.1/24" dev h1
  ip -n "$1" link set h1 up
  ip -n "$2" addr add "$3.2/24" dev h0
  ip -n "$2" link set h0 up
  ip -n "$2" route add default via "$3.1"
  ip netns exec "$1" sysctl -q -w net.ipv4.ip_forward=1
  # A port of its own for every destination, and nothing in that the home did not ask for.
  ip netns exec "$1" nft -f - <<EOF
table ip home {
  chain post { type nat hook postrouting priority srcnat; oifname "e0" masquerade fully-random; }
  chain through { type filter hook forward priority filter; policy drop; ct state established,related accept; iifname "h1" accept; }
}
EOF
}
home bt-nata bt-homea 10.1.0
home bt-natb bt-homeb 10.2.0

# ---- Phloem, as its image starts it ----
mkdir -p "${TMP}/phloem"
chown 65532:65532 "${TMP}/phloem"
chmod 0755 "${TMP}"
PHLOEM_ENV=(PATH="/usr/sbin:/usr/bin:/sbin:/bin" PHLOEM_ADDR=0.0.0.0:8200 PHLOEM_DATA="${TMP}/phloem"
  PHLOEM_ORIGINS=http://127.0.0.1:8080 PHLOEM_LOG_FORMAT=text
  PHLOEM_FIREWALL="${ROOT}/phloem/deploy/firewall.nft" PHLOEM_FIREWALL_TURN="${ROOT}/phloem/deploy/turn.nft"
  PHLOEM_TURN_HOST=11.0.0.1 PHLOEM_TURN_PORT=3478 PHLOEM_TURN_IP=11.0.0.1 PHLOEM_TURN_RELAY_PORTS=49160-49167)
ip netns exec bt-srv env -i "${PHLOEM_ENV[@]}" "${PHLOEM_BIN}" start >"${TMP}/phloem.log" 2>&1 &
PIDS+=("$!")
sleep 2
# A kernel that refuses no_new_privs (some sandboxes do; the server's does not) stops start after
# the firewall: then the same rules again with nft as start loads them, and serve as 65532.
if grep -q 'PR_SET_NO_NEW_PRIVS' "${TMP}/phloem.log"; then
  say "this kernel refuses no_new_privs: the firewall by nft as start loads it, serve under setpriv"
  printf 'define turn_port = 3478\ndefine relay_ports = 49160-49167\ndefine turn_ip = 11.0.0.1\ninclude "%s"\ninclude "%s"\n' \
    "${ROOT}/phloem/deploy/firewall.nft" "${ROOT}/phloem/deploy/turn.nft" | ip netns exec bt-srv nft -f -
  ip netns exec bt-srv env -i "${PHLOEM_ENV[@]}" setpriv --reuid=65532 --regid=65532 --clear-groups --inh-caps=-all --bounding-set=-all \
    "${PHLOEM_BIN}" serve >>"${TMP}/phloem.log" 2>&1 &
  PIDS+=("$!")
fi

# ---- Folia ----
(cd "${ROOT}/folia" && exec ip netns exec bt-web env FOLIA_ADDR=0.0.0.0:8080 FOLIA_SYNC=on \
  FOLIA_SYNC_URL=http://127.0.0.1:8200/sync/v1/ FOLIA_DATA_DIR="${FOLIA_DATA_DIR}" \
  FOLIA_SNAPSHOT_URL=http://127.0.0.1:9/snapshot/catalog.db FOLIA_WARM_CACHE=off FOLIA_LOG_LEVEL=warn \
  "${FOLIA_BIN}" --live-assets assets >"${TMP}/folia.log" 2>&1) &
PIDS+=("$!")

for _ in $(seq 1 120); do
  if ip netns exec bt-homea curl -s -o /dev/null --max-time 2 http://11.0.0.2:8080/livez &&
    ip netns exec bt-homeb curl -s -o /dev/null --max-time 2 http://11.0.0.1:8200/sync/v1/pair; then
    break
  fi
  sleep 1
done
say "Phloem: $(grep -o 'firewall in place.*' "${TMP}/phloem.log" | head -n 1)"

# ---- the browsers, each in its home ----
# WebCrypto wants a secure context: the pages are at http://127.0.0.1:8080 of each home, and a
# forwarder there takes 8080 to Folia and 8200 to Phloem, through the home's NAT (Phloem sees the
# router's address, as it sees a real home's). STUN and TURN go to 11.0.0.1 directly.
cat >"${TMP}/forward.py" <<'PYEOF2'
import asyncio, sys
async def pipe(r, w):
    try:
        while data := await r.read(65536):
            w.write(data)
            await w.drain()
    except Exception:
        pass
    finally:
        w.close()
async def handle(r, w, host, port):
    try:
        ur, uw = await asyncio.open_connection(host, port)
    except Exception:
        w.close()
        return
    await asyncio.gather(pipe(r, uw), pipe(ur, w))
async def main():
    servers = []
    for spec in sys.argv[1:]:
        lport, host, port = spec.split(":")
        servers.append(await asyncio.start_server(lambda r, w, h=host, p=int(port): handle(r, w, h, p), "127.0.0.1", int(lport)))
    await asyncio.gather(*(s.serve_forever() for s in servers))
asyncio.run(main())
PYEOF2
for h in a b; do
  ip netns exec "bt-home${h}" python3 "${TMP}/forward.py" 8080:11.0.0.2:8080 8200:11.0.0.1:8200 &
  PIDS+=("$!")
done
sleep 1
for h in a b; do
  printf '#!/bin/sh\nexec ip netns exec bt-home%s %s "$@"\n' "${h}" "${BROWSER}" >"${TMP}/browser-${h}"
  chmod +x "${TMP}/browser-${h}"
done
code=0
(cd "${ROOT}/folia/e2e" && BT_BROWSER_A="${TMP}/browser-a" BT_BROWSER_B="${TMP}/browser-b" \
  SMOKE_BASE_URL=http://127.0.0.1:8080 node turn.mjs) || code=1

# ---- nothing tried what the firewall forbids, and it stays forbidden ----
counted="$(ip netns exec bt-srv nft list table inet phloem | grep -o 'counter packets [0-9]*' | awk '{ s += $3 } END { print s + 0 }')"
if [[ "${counted}" == "0" ]]; then
  say "the firewall dropped nothing that it counts: TURN went through the rules it opens alone"
else
  say "FAIL: the firewall counted ${counted} dropped packets during the run:"
  ip netns exec bt-srv nft list table inet phloem | grep -E 'counter packets [1-9]' || true
  code=1
fi
pid="$(pgrep -f "${PHLOEM_BIN} serve" | head -n 1 || true)"
nonewprivs_refused=0
if grep -q 'PR_SET_NO_NEW_PRIVS' "${TMP}/phloem.log"; then nonewprivs_refused=1; fi
if [[ -n "${pid}" ]] && nsenter -t "${pid}" -n -m -- env PHLOEM_TURN_HOST=11.0.0.1 PHLOEM_TURN_IP=11.0.0.1 \
  PHLOEM_TURN_RELAY_PORTS=49160-49167 "${PHLOEM_BIN}" probe --pid "${pid}" >"${TMP}/probe.json" 2>&1 ||
  { [[ "${nonewprivs_refused}" == "1" ]] && ! grep -v -F -e '"result":"blocked"' -e '"result":"ok"' -e 'NoNewPrivs 0' "${TMP}/probe.json" | grep -q .; }; then
  blocked="$(grep -c -F '"result":"blocked"' "${TMP}/probe.json" || true)"
  say "probe: all ${blocked} targets blocked, the service unprivileged"
else
  say "probe FAILED:"
  grep -v -F -e '"result":"blocked"' -e '"result":"ok"' "${TMP}/probe.json" | head -n 8 || true
  code=1
fi
if [[ "${code}" -ne 0 ]]; then
  say "Phloem's log:"
  tail -n 30 "${TMP}/phloem.log"
fi
exit "${code}"
