# Phloem: running it

Phloem is the service of Betula Sync: it keeps the sealed records of the profiles that sync and
hands them between the devices of a profile, live, and can open none of them. What it says on
the wire, and what may reach what, is the contract in [`protocol.md`](protocol.md); the design
and the owner's decisions are in `research/sync/entwurf.html`. This file is how to build, run,
deploy and watch it.

| | |
|---|---|
| code | `phloem/`, a Go module of its own (`cmd/phloem`, `internal/{server,store,metrics,oplog,dotenv,version}`), its firewall `phloem/deploy/firewall.nft` |
| version | `phloem/internal/version/version.go` (0.1.0); `phloem version` and `phloem_build_info` show it with the build (the start of the executable's SHA-256) |
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
  socket at ws://127.0.0.1:8200/sync/v1/ws.
- `PHLOEM_ORIGINS` lists the origins it takes for the socket and answers CORS for (preflights
  included: `Authorization`, `Betula-Space`, `Betula-Pair`, `Content-Type`; GET, POST, DELETE).
  Another origin gets 403 `origin`; a request without one (curl, a test) is taken.
- `phloem-data/phloem.db` is made on the first start (SQLite, migrated by Phloem itself).
- Without Traefik in front, the limits per address count the peer: everything from 127.0.0.1 is
  one address (20 new spaces an hour). They live in memory: a restart starts them anew.
- A `.env` in the working directory (git-ignored; another file: `PHLOEM_ENV_FILE`) is read for
  every command but `start`; the real environment wins over it.

## Commands

| | |
|---|---|
| `serve` | the service (the default command): the HTTP API and the sockets, until SIGTERM; then no new connections, the requests in flight end within 10 s, every device hears 1001, and the stats file says `phloem_up 0` |
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
| `PHLOEM_BUDGET_BYTES` | 50 000 000 000 | the stack | what all spaces may hold; at it no new space and no parcel (507) |
| `PHLOEM_STACK` | none | the stack | the label `stack=` of the stats file |
| `PHLOEM_LOG_FORMAT`, `_LEVEL`, `_FILE` | `json`, `info`, none | `json` | |
| `PHLOEM_FIREWALL`, `PHLOEM_NFT` | `/etc/phloem/firewall.nft`, `nft` | | for `start` only, from the image and the stack alone |

## Limits

Every limit is Phloem's own and holds whatever a client does (`internal/server/server.go`,
`DefaultLimits`; the numbers a device meets are in `protocol.md`): 16 devices and 16 sockets per
space, a push of at most 64 KiB, a snapshot of at most 6 MiB, `compact` above 128 KiB of entries
since the snapshot and `compact_first` above 256 KiB, 50 pushes a second per space (bursts of
200), 20 new spaces per address and hour and 500 in all, 16 parcels per space for 10 minutes,
120 parcel takes per address and hour, 4096 sockets per instance and 32 per address before their
hello, 395 days untouched and a space is deleted.

Memory is bounded the same way: at most 8 frames above 128 KiB are read at a time (others wait),
and a device's queue holds at most 512 frames or 16 MiB, beyond which Phloem lets it go and it
comes back with `since`. The stack gives the container 512 MiB, `GOMEMLIMIT` 400 MiB, one CPU and
128 processes.

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

Look at it on the server (as root):

```
sudo nsenter -t "$(docker inspect -f '{{.State.Pid}}' "$(docker ps -q -f label=com.docker.swarm.service.name=phloem_sync)")" -n nft list table inet phloem
```

## Numbers and logs

The stats file `phloem.prom` (Prometheus text, every 15 s, written by a rename) holds:
`phloem_up`, `phloem_build_info{version,build}`, `phloem_start_time_seconds`,
`phloem_stats_time_seconds`, `phloem_spaces`, `phloem_spaces_online`, `phloem_devices`,
`phloem_sockets`, `phloem_bytes`, `phloem_budget_bytes`, `phloem_entries_total`,
`phloem_entry_bytes_total`, `phloem_snapshots_total`, `phloem_snapshot_bytes_total`,
`phloem_parcels_total`, `phloem_pushes_again_total`, `phloem_spaces_created_total`,
`phloem_spaces_deleted_total{reason}`, `phloem_rejections_total{reason}`,
`phloem_socket_closes_total{reason}`, `phloem_log_problems_total`, each with `stack`. Alloy reads
it through `/rootfs` (`config/monitoring/alloy/config.alloy`, job `phloem`); the dashboard "Sync"
and the rules of the group `betula-phloem` use these names.

Logs are JSON lines (`level`, `event`, `msg`, `component`), one per event, never one per request
or frame: `start.firewall`, `start.dropped`, `start.failed`, `service.started`,
`service.stopped`, `service.fatal`, `service.root`, `http.listening`, `http.failed`,
`db.migrated`, `store.failed`, `sweep.done`, `stats.failed`, `stats.recovered`, `budget.half`,
`budget.reached`, `budget.recovered`. Nothing a device sends is logged, nor a space, a token or
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
- Step 2 (WebRTC between the devices, STUN and TURN): `<stack>_out`, published ports, the
  firewall's way out for TURN datagrams. Step 3: the parts for calendars and friends.
