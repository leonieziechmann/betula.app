# Phloem: running it

Phloem is the service of Betula Sync: it keeps the sealed records of the profiles that sync and
hands them between the devices of a profile, live, keeps the sealed parts of shared calendars
for anyone with their link, and can open none of them. What it says on the wire, and what may
reach what, is the contract in [`protocol.md`](protocol.md); the design and the owner's
decisions are in `research/sync/entwurf.html` and, for sharing (step 3),
`folia/design/teilen/teilen.html`. This file is how to build, run, deploy and watch it.

| | |
|---|---|
| code | `phloem/`, a Go module of its own (`cmd/phloem`, `internal/{server,store,metrics,oplog,dotenv,version}`), its firewall `phloem/deploy/firewall.nft` |
| version | `phloem/internal/version/version.go` (0.2.0); `phloem version` and `phloem_build_info` show it with the build (the start of the executable's SHA-256) |
| builds | `nix build .#phloem` (the binary, its tests run in the build), `nix build .#phloem-image` (`betula-phloem:latest`: the binary, the firewall and `nft`, no shell) |
| deploy | `deploy/stacks/phloem.yml`, one file per instance in `deploy/stacks/phloem/`, `deploy/vps/47-phloem-host.sh`, `deploy/vps/49-phloem.sh` (with `lib-phloem.sh`), `deploy/ship-phloem.sh` |

## Development

Go 1.27, as Radix and Cortex. In `phloem/`:

```
go test ./...
go build -o phloem ./cmd/phloem
```

For Folia's dev server (the app on http://127.0.0.1:8080), Phloem runs next to it and the
browser talks to it directly, cross-origin:

```
PHLOEM_ORIGINS=http://127.0.0.1:8080 PHLOEM_DATA=phloem-data PHLOEM_LOG_FORMAT=text ./phloem serve
```

- It listens on `127.0.0.1:8200` (`PHLOEM_ADDR`): the API at http://127.0.0.1:8200/sync/v1/, the
  socket at ws://127.0.0.1:8200/sync/v1/ws, and the same socket over plain requests at
  http://127.0.0.1:8200/sync/v1/poll.
- `PHLOEM_ORIGINS` lists the origins it takes for the socket and answers CORS for (preflights
  included: `Authorization`, `Betula-Space`, `Betula-Pair`, `Betula-Part`, `Betula-Poll`,
  `Content-Type`, `If-None-Match`; GET, POST, PUT, DELETE; `ETag` is exposed, a part's version).
  Another origin gets 403 `origin`; a request without one (curl, a test) is taken.
- `phloem-data/phloem.db` is made on the first start (SQLite, migrated by Phloem itself).
- Without Traefik in front, the limits per address count the peer: everything from 127.0.0.1 is
  one address (20 new spaces an hour). They live in memory: a restart starts them anew.
- A `.env` in the working directory (git-ignored; another file: `PHLOEM_ENV_FILE`) is read for
  every command but `start`; the real environment wins over it.

## Commands

| | |
|---|---|
| `serve` | the service (the default command): the HTTP API and the sockets, until SIGTERM; then no new connections, every device hears 1001 (as the shutdown begins, so that the GETs of the poll sessions that wait end with it), the requests in flight end within 10 s, and the stats file says `phloem_up 0` |
| `start` | the image's entrypoint, as root with NET_ADMIN, SETUID and SETGID only: loads the firewall, checks that it holds (a datagram to Docker's resolver must be dropped), sets no_new_privs, drops to uid/gid 65532 without any capability, checks `/proc` that nothing is left, and execs `serve`. Anything failing ends it before Phloem serves. It reads no `.env` |
| `healthcheck` | exit 0 when the local instance answers `/healthz` (the image's HEALTHCHECK, every 10 s); drops root first |
| `status` | the local instance's numbers as one JSON object (`GET /status`, loopback only); drops root first |
| `probe [ip:port…]` | tries from inside what the firewall forbids (Docker's resolver, a public resolver and web server, the metadata service, IPv6, a name lookup, and the addresses given) and checks that the service (pid 1) has no privilege; one JSON object per line, exit 1 when anything got out. Drops root first |
| `version`, `help` | |

## Settings

Every flag of `serve` has an environment variable; the image sets the ones in its column.

| variable | default | image | |
|---|---|---|---|
| `PHLOEM_ADDR` | `127.0.0.1:8200` | `0.0.0.0:8200` | listen address |
| `PHLOEM_DATA` | `phloem-data` | `/data` | the directory of `phloem.db` |
| `PHLOEM_STATS_DIR` | none | `/stats` | where `phloem.prom` is written |
| `PHLOEM_STATS_INTERVAL` | `15s` | | how often |
| `PHLOEM_ORIGINS` | the request's own host | the stack: the site's origin | comma-separated `scheme://host[:port]` |
| `PHLOEM_BUDGET_BYTES` | 50 000 000 000 | the stack | what all spaces may hold (see Storage); at it no new space, parcel or part (507) |
| `PHLOEM_STACK` | none | the stack | the label `stack=` of the stats file |
| `PHLOEM_LOG_FORMAT`, `_LEVEL`, `_FILE` | `json`, `info`, none | `json` | |
| `PHLOEM_FIREWALL`, `PHLOEM_FIREWALL_TURN`, `PHLOEM_NFT` | `/etc/phloem/firewall.nft`, `/etc/phloem/turn.nft`, `nft` | | for `start` only, from the image and the stack alone |
| `PHLOEM_TURN_HOST` | none | the stack (TURN's file) | the site's host, named by the URLs of STUN and TURN; empty: no STUN and TURN (step 2, „TURN") |
| `PHLOEM_TURN_PORT` | 3478 | the stack | where STUN and TURN listen, UDP and TCP, on every address |
| `PHLOEM_TURN_IP` | none | the stack | the host's public IPv4 address: the relays', never a peer |
| `PHLOEM_TURN_RELAY_PORTS` | none | the stack | the relays' UDP ports, `first-last` |
| `PHLOEM_TURN_TLS_HOST` | none | the stack | the name of `turns:<name>:443` (Traefik ends TLS); empty: none |

## Limits

Every limit is Phloem's own and holds whatever a client does (`internal/server/server.go`,
`DefaultLimits`; the numbers a device meets are in `protocol.md`): 16 devices and 16 sockets per
space, a push of at most 64 KiB, a snapshot of at most 6 MiB, `compact` above 128 KiB of entries
since the snapshot and `compact_first` above 256 KiB, 50 pushes a second per space (bursts of
200; a parcel, a part and a watch count as one each), 20 new spaces per address and hour and 500
in all, 32 parcels waiting per space for 60 s to 7 days (10 minutes unless the device names
another), what became of a parcel kept 30 days and for the newest 256 of a space (an older one
goes first), 120 parcel takes per address and hour, parts of at most 16 KiB, 32 per space and 32
new ones per space and day (a part deleted stays gone 395 days, beyond its space, so the new parts
of a day bound what a space can leave behind), 3600 part reads per address and hour, a watch of
at most 64 parts, 4096 sockets per instance and 32 per address before their hello, 395 days
untouched and a space is deleted. Step 2: a signal of at most 8 KiB, 20 signals and `turn` frames a
second per socket (bursts of 100); TURN as section „TURN" says. The socket over HTTPS (Phloem
0.4.0): a poll session is a socket among the 4096, a GET waits at most 25 s for a frame, a session
nobody asks anything for 60 s ends, a POST's body holds at most 9 MiB (`PollWait`, `PollIdle`).

Memory is bounded the same way: at most 8 frames above 128 KiB are read at a time (others wait),
a device's queue holds at most 512 frames or 16 MiB, beyond which Phloem lets it go and it comes
back with `since`, and a socket watches at most 64 parts. Of the poll sessions, at most 2 bodies
above 128 KiB are read at a time and at most 2 frames above 128 KiB wait for their GET (others
wait), each a pool of its own beside the 8, so that none waits for another. The stack gives the
container 512 MiB, `GOMEMLIMIT` 400 MiB, one CPU and 128 processes.

## Storage

`phloem.db` in the data directory, SQLite (WAL, synchronous FULL), migrated by Phloem itself
(`internal/store/migrations`, the schema's version in `PRAGMA user_version`; a binary refuses an
index a newer one migrated further). One writer, a pool of readers: an id that finds nothing (a
parcel, a part) costs a read, never the writer.

| what | kept |
|---|---|
| a space, its devices, its entries after the snapshot, its snapshot | until „Sync beenden", or 395 days untouched |
| a parcel | until it is taken or withdrawn, or expires |
| what became of a parcel (`taken`, `expired`, `withdrawn`, when) | 30 days, the newest 256 of a space; with the space |
| a part (its space, version, sealed data) | until „Nicht mehr teilen", or its space goes |
| the id of a part deleted, and when | 395 days, also after its space went: the id never becomes a part again, so that a link to it never shows another calendar; nothing else of the part or its space is kept |

The budget counts what the spaces hold (data, and 64 bytes for every row) and a row for every
part tombstone, which outlives its space; `phloem_bytes` is that number, kept exact in memory.

## Firewall

`phloem/deploy/firewall.nft`, the table `inet phloem` in the container's network namespace. In:
loopback, replies, new TCP connections to 8200. Out: replies and loopback. No new connection out,
no DNS (Docker's resolver at 127.0.0.11 neither), no IPv6, nothing forwarded. Every chain ends
with a counting drop, and every drop that means someone tried something is counted in the chain
it happened in: betula-phloem-guard (`deploy/vps/files`, on the host every minute) reads the
counters and the rights of each process of the container from outside, and the monitoring alerts
on any counted packet ("Phloem: its firewall drops connections"). Not counted, on purpose: the
datagram `start` sends as root to check the firewall (otherwise every start would set off the
alarm), and packets conntrack cannot place. Docker's NAT rewrites the resolver's port before the
table sees a packet; the file says how the rule copes.

With TURN (`PHLOEM_TURN_HOST` set), `start` loads `phloem/deploy/turn.nft` with it in one
transaction, after it defined `$turn_port`, `$relay_ports` and `$turn_ip` from the same settings
`serve` reads: the chains `turn_in` and `turn_out`, which the base chains jump to before their
counting drop, let in STUN and TURN (UDP and TCP on the port) and the peers' datagrams to the
relays, and let out a relay's UDP datagram to a public address alone; one to Phloem's own address
or to an address that is none on the internet is dropped and counted. `start` checks the rules
are there before it drops its rights; `phloem probe` tries, from a relay's port, a private
address, the metadata service, Docker's resolver and the host's own address.

Look at it on the server (as root):

```
sudo nsenter -t "$(docker inspect -f '{{.State.Pid}}' "$(docker ps -q -f label=com.docker.swarm.service.name=phloem_sync)")" -n nft list table inet phloem
```

## TURN

Step 2 of Betula Sync (owner, 2026-11-08: „time für phase 2 bei dem sync"): the devices of a
profile online connect directly with WebRTC (`docs/folia/sync.md`, „The live way"), their signals
sealed through the WebSocket (`protocol.md` section 4). Where no direct way exists, Phloem relays
(Pion, `internal/server/turn.go`; the rules in `protocol.md` section 6): a credential per socket,
held while it lives; a peer only at the address another socket of the same space came from or,
since Phloem 0.4.0, allocated from (a VPN, a device whose socket goes over IPv6 and its UDP over
IPv4; on canary on 2026-11-10, 60 permissions were refused against 12 granted), never a special
address nor Phloem's own; UDP relays alone, 128 KiB a second each way each, 4 MiB a second all of
them; 8 allocations per socket, 32 per space. Over UDP every nonce is bound to the address it goes
to (`internal/server/nonce.go`): an allocation's address answered Phloem, a forged source cannot
make an address a peer. Its numbers: `phloem_turn_allocations`, `phloem_turn_total{event}`
(`allocated`, `permitted`, `learned` (an allocation came from an address its socket did not),
`refused`, `unauthorized`, `quota`, `no_port`, `nonce` (a request over UDP with a nonce not
handed to its address, answered 438), `forbidden`), `phloem_relay_bytes_total{way}` and
`phloem_relay_dropped_bytes_total{way}` (`in`: from a peer, `out`: to one), `phloem_signals_total`.

Its ports are published in host mode (`deploy/vps/lib-phloem.sh`, `phloem_turn_file`): Phloem
must see a device's own address, as Traefik does, both for the permission and for the relay's
datagrams. `deploy/test/turn.sh` builds two homes behind NATs that give every destination a port of
its own, in network namespaces, and checks that the first attempt (STUN alone) finds no way, the
one with TURN opens, a change goes through the relay alone, and the firewall counts no drop.

Not done on purpose: a relay to another relay of the same instance (both devices behind such
NATs): it would go through the same machine as the WebSocket, which carries the changes then, and
it would make Phloem's own address a peer.

## The socket over HTTPS

Since Phloem 0.4.0 (owner, 2026-11-10: „wenn eine methode nicht funktioniert muss dann halt
ausgewichen werden auf eine andere die garantiert läuft"): where a network breaks the WebSocket's
upgrade and lets plain requests through, the browser polls (`protocol.md` section 4, „The socket
over HTTPS"). A poll session is a bridge (`internal/server/poll.go`): Phloem dials its own handler
of `/sync/v1/ws` in memory, the two ends of a `net.Pipe` served by an HTTP server on a listener of
its own, so nothing touches the network and the firewall stays as it is. The socket carries the
request's `X-Real-Ip` and `Origin`: the limits of an address and TURN see the device. A goroutine
reads the socket into the session's queue (and so answers Phloem's pings); a POST writes into it,
a GET takes what waits. Every rule of the socket holds for both, since there is one socket.

Traefik needs nothing new: `/sync/v1/poll` is under the router of `/sync/v1/`, a GET of 25 s is
within its timeouts, and its access log keeps no header (the session's id is one). Its
`sync-ratelimit` (20 requests a second per address, bursts of 60) counts every GET and POST,
where a WebSocket was one request.

## Numbers and logs

The stats file `phloem.prom` (Prometheus text, every 15 s, written by a rename) holds:
`phloem_up`, `phloem_build_info{version,build}`, `phloem_start_time_seconds`,
`phloem_stats_time_seconds`, `phloem_spaces`, `phloem_spaces_online`, `phloem_devices`,
`phloem_sockets`, `phloem_bytes`, `phloem_budget_bytes`, `phloem_parts`, `phloem_parts_watched`,
`phloem_entries_total`, `phloem_entry_bytes_total`, `phloem_snapshots_total`,
`phloem_snapshot_bytes_total`, `phloem_parcels_total{event}`, `phloem_parts_total{event}`,
`phloem_pushes_again_total`, `phloem_spaces_created_total`, `phloem_spaces_deleted_total{reason}`,
`phloem_rejections_total{reason}`, `phloem_socket_closes_total{reason}`,
`phloem_poll_sessions`, `phloem_poll_sessions_opened_total`,
`phloem_poll_sessions_ended_total{reason}`,
`phloem_log_problems_total`, and for step 2 the numbers of „TURN", each with `stack`. Alloy reads it through `/rootfs`
(`config/monitoring/alloy/config.alloy`, job `phloem`); the dashboard "Sync" and the rules of the
group `betula-phloem` use these names.

- `phloem_parcels_total{event}`: `left`, `taken`, `missed` (a take that found none), `expired`,
  `withdrawn`.
- `phloem_parts`, `phloem_parts_watched`: the parts kept (counted every 15 s) and the parts that
  sockets watch now. `phloem_parts_total{event}`: `put` (made or replaced), `deleted` („Nicht mehr
  teilen", or with their space), and the reads: `served` (200), `not_modified` (304), `missed`
  (404), `gone` (410).
- `phloem_rejections_total{reason}`: `unauthorized`, `too_big`, `compact_first`, `slow_down`,
  `too_many` (32 parts), `full`, `budget`, `exists`, `origin`, `busy`, `invalid`.
- `phloem_poll_sessions`: the poll sessions whose socket lives (among `phloem_sockets` too).
  `phloem_poll_sessions_opened_total`, and `phloem_poll_sessions_ended_total{reason}` by what
  ended their socket: `closed` (a close the device hears: left, gone, replaced, restart, 1008,
  1009, 1011), `lost` (ended without one), `slow` (too far behind), `idle` (nobody asked for a
  minute). The closes of the socket itself count in `phloem_socket_closes_total` as any socket's.

Logs are JSON lines (`level`, `event`, `msg`, `component`), one per event, never one per request
or frame: `start.firewall`, `start.dropped`, `start.failed`, `service.started`,
`service.stopped`, `service.fatal`, `service.root`, `http.listening`, `http.failed`,
`db.migrated`, `store.failed`, `sweep.done`, `stats.failed`, `stats.recovered`, `budget.half`,
`budget.reached`, `budget.recovered`, `turn.forbidden` (WARN: a relay was to send where no peer
may be), `turn.pion` (what Pion says, at debug). Nothing a device sends is logged, nor a space, a token or
an address: what happens to spaces and sockets is counted in the stats file instead.

## Deploy

Two instances, each a stack of its own with its own network, data and numbers: `phloem` for
https://betula.app and `phloem-canary` for https://canary.betula.app. What differs is in
`deploy/stacks/phloem/<stack>.env`. Phloem is not part of the canary pipeline: a push to master
does not restart it.

Once per server, before the first deploy of an instance:

1. Its network `<stack>_in` (overlay, internal, not attachable), Traefik on it, and the guard:
   `deploy/vps/30-docker.sh` and `stacks/edge.yml`.
2. Its file systems, as root: `sudo bash /opt/betula/vps/47-phloem-host.sh <stack>`. The data on
   an ext4 of its own of `PHLOEM_DATA_SIZE` (a sparse file on a loop device, so Phloem fills
   nothing else of the host), the stats on a tmpfs of 1 MiB; both mount units are enabled, so
   they are there at every boot before Docker. Beneath both lies a directory of root's with mode
   0000: a container that starts without them cannot write anything.

Each release, from the workstation (Nix, ssh to the server):

```
SSH_TARGET=betula deploy/ship-phloem.sh phloem-canary
SSH_TARGET=betula deploy/ship-phloem.sh phloem
```

It builds `.#phloem-image` from HEAD (the tag is `<date>-<hash>` of the last commit to what the
image is made of), loads it over ssh, runs `deploy/sync.sh`, then `vps/49-phloem.sh <stack>
<tag>` on the server. That checks the network, Traefik on it, the file systems and the image,
deploys the stack (stop-first: one writer on `phloem.db`; the devices come back within seconds),
waits for the health check, and then tries what is forbidden: `phloem probe` from inside (with
Traefik's addresses), https://<host>/sync/v1/ through Traefik (Phloem's router, by the access
log) and /healthz (not Phloem's), the stats file, `phloem status`; at last
`91-verify-stacks.sh phloem`. A FAIL ends with exit 1.

- Rollback: `bash /opt/betula/vps/49-phloem.sh <stack> <previous tag>` (still loaded:
  `docker image ls betula-phloem`).
- A change to the stack file or the instance's file: `deploy/sync.sh`, then
  `bash /opt/betula/vps/49-phloem.sh <stack>` (the running tag).
- More room: a larger `PHLOEM_DATA_SIZE`, `deploy/sync.sh`, then `sudo bash
  /opt/betula/vps/47-phloem-host.sh <stack>` (grows it while Phloem runs; never shrinks). A
  larger budget then with 49-phloem.sh; the budget has to stay below 95 % of the file system.
- Status: `docker exec "$(docker ps -q -f label=com.docker.swarm.service.name=<stack>_sync)" /bin/phloem status`.

## Not yet

- A backup of `phloem.db`: sealed by the devices, so it may leave the house; SQLite's online
  backup into the data file system and off the server, not written yet.
- A dashboard panel for TURN (the numbers are in the stats file already).
