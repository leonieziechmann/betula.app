# Phloem: the protocol, the names and the boundaries

Phloem is the service of Betula Sync (`research/sync/entwurf.html`, owner 2026-11-05: „Passt, bau
Schritt 1 jetzt so"). It keeps sealed records of the profiles that sync and hands them between the
devices of a profile; it can open none of them. This file is the contract between its three parts:
the service (`phloem/`, Go), the browser (`folia/crates/sync` and the app), and the deployment
(`deploy/`). Step 1 is everything below except what is marked step 2 (WebRTC, STUN, TURN) or
step 3 (calendar and friends' parts).

## 1. Names

| | |
|---|---|
| instances | one per site, each a stack of its own: `phloem` for https://betula.app, `phloem-canary` for https://canary.betula.app |
| networks | `<stack>_in` (`phloem_in`, `phloem-canary_in`): overlay, `--internal`, not attachable, created by `deploy/vps/30-docker.sh`; its members are Traefik and that instance, nothing else. `<stack>_out`: the stack's own overlay (not internal), its only member that instance; its way out, used by TURN only (step 2) |
| port | 8200/tcp inside the container, on `<stack>_in` only; nothing published in step 1 (step 2 publishes 3478/udp+tcp and a relay range in host mode) |
| path | `/sync/v1/` on the site's own host, routed by Traefik to the site's instance |
| data | `/data` in the container: `/var/lib/betula/phloem/<stack>/data` on the host, a filesystem of fixed size of its own (a loop device), so that Phloem fills nothing but itself |
| numbers | `/stats` in the container: `/run/betula/phloem/<stack>/stats` on the host, a tmpfs of 1 MiB; Phloem writes `phloem.prom` there every 15 s, Alloy reads it (it sees the host at `/rootfs`, read-only) |
| image | `betula-phloem:<date of the commit>-<short hash>` (as Cortex) |
| version | `phloem/internal/version/version.go`, SemVer, starting at 0.1.0 |

## 2. What may reach what

Phloem shares no network with any internal service (owner, 2026-11-05: „als wäre es ein externer
dienst … Das einzigige, was intern laufen darf ist die anfragen von grafana an die stats api, aber
nicht anders herum"). Its numbers reach the monitoring as a file, since a shared network would let
either side reach the other.

| from → to | | how |
|---|---|---|
| internet → Traefik :443 `/sync/v1/` | yes | the router of the instance, above Folia's (explicit priority 1000) |
| Traefik → Phloem :8200 | yes | over `<stack>_in` |
| Phloem → anything | no | a firewall inside the container (`phloem/deploy/firewall.nft`): no new connection out, no DNS (also not Docker's 127.0.0.11), IPv6 off; replies only. Set by `phloem start` as root with NET_ADMIN, which then drops to uid 65532 without capabilities and execs `phloem serve` |
| Folia → Phloem | only through Traefik | as an outside client, `https://<host>/sync/v1/` (step 3) |
| monitoring → Phloem | reads the file | no network |

## 3. HTTP

Every request names its space and proves it in headers, never in the path (Traefik's access log
keeps paths): `Betula-Space: <space>`, `Authorization: Bearer <token>`. All values are base64url
without padding. Bodies are JSON. Errors are `{"error": "<code>"}` with a fitting status.

| | |
|---|---|
| `POST /sync/v1/space` | makes a space. Body `{"space": "<16 bytes>", "token": "<32 bytes>", "device": "<16 bytes>"}`. Phloem keeps SHA-256 of the token. 201 `{"slot": 1}`; 409 `exists`; 429 `slow_down` (20 spaces per address and hour, 500 in all); 507 `budget` (the budget is reached: no new spaces, the old ones go on) |
| `DELETE /sync/v1/space` | „Sync beenden": the space and everything of it, at once; connected devices hear `gone`. 204 |
| `POST /sync/v1/pair` | leaves a sealed parcel for a device to come (authorised by the space's headers). Body `{"id": "<16 bytes>", "box": "<at most 4 KiB>"}`. Kept 10 minutes. 201 |
| `GET /sync/v1/pair` | `Betula-Pair: <id>`: the parcel once, then it is gone. 200 `{"box": "…"}`; 404 |
| `GET /sync/v1/ws` | the WebSocket of a device (section 4) |
| `GET /healthz` | 200 `ok`; not routed by Traefik |

`PHLOEM_ORIGINS` (comma separated, default the site's own origin) are the origins Phloem accepts
for the WebSocket and answers CORS for (development runs the app on another port).

## 4. The WebSocket

Text frames, one JSON object each, `"t"` its type.

From a device:

| | |
|---|---|
| `{"t": "hello", "space", "token", "device", "since": <nr>}` | the first frame; nothing else before it. `since` is the number of the last entry the device holds (0 for none) |
| `{"t": "push", "id": "<the device's own id of it>", "data": "<sealed batch>"}` | one change: the sealed writes of it, at most 64 KiB |
| `{"t": "snapshot", "upto": <nr>, "data": "<sealed>"}` | every record of the profile as of entry `upto`, at most 6 MiB; Phloem keeps the newest one and forgets the entries up to `upto` |
| `{"t": "leave"}` | „Nur dieses Gerät trennen": its slot is free again |
| `{"t": "delete"}` | the same as `DELETE /sync/v1/space` |

From Phloem:

| | |
|---|---|
| `{"t": "welcome", "slot", "head", "now", "devices": [<slots online>], "snapshot": null \| {"upto", "data"}, "entries": [{"nr", "slot", "data"}…]}` | after a valid hello. `slot` is the device's place in the space (1–16, kept by its device id); `now` the server's clock in ms, for the device's clock. The snapshot comes when `since` lies before it, and then the entries after it; otherwise the entries after `since` |
| `{"t": "stored", "id", "nr"}` | a push is kept as entry `nr` |
| `{"t": "entry", "nr", "slot", "data"}` | another device's push, to every other device online |
| `{"t": "snapshotted", "upto"}` | the snapshot is kept |
| `{"t": "presence", "devices": [<slots online>]}` | a device came or went |
| `{"t": "compact"}` | the entries since the snapshot exceed 128 KiB: the device should send a snapshot |
| `{"t": "error", "code", "id"?}` | `unauthorized` (and the socket closes), `too_big`, `compact_first` (above 256 KiB a push waits for a snapshot), `slow_down` (more than 50 pushes a second in a space, bursts of 200), `full` (16 devices) |
| `{"t": "gone"}` | the space was deleted; the socket closes |

At most 16 sockets per space; a second hello of one device closes its older socket. A space not
touched (hello, push, snapshot, pair) for 395 days is deleted. Every limit is Phloem's own and
holds whatever a client does.

## 5. In the browser (for the service: opaque)

- The profile key `K`: 32 random bytes, on the devices only. HKDF-SHA256 (salt `betula-sync-v1`)
  gives `space` (16 bytes, info `space`), `token` (32 bytes, info `token`) and the AES-256-GCM key
  of the data (info `data`). A device id is 16 random bytes of its own.
- Sealed: a 12-byte random nonce, then AES-256-GCM's ciphertext and tag. The associated data names
  the space and what it is: `space ‖ "batch"`, `space ‖ "snapshot" ‖ upto` (8 bytes, big endian),
  `space ‖ "pair"`.
- A batch holds the writes of one change, a snapshot every record with its clocks
  (`folia_sync::codec`), uncompressed (a typical snapshot is 5.5 KB).
- The pairing link `https://<host>/verbinden#<id ‖ key>` (16 + 32 bytes): `id` finds the parcel,
  `key` opens it (AES-256-GCM, associated data `"pair"`); the parcel holds the profile keys chosen.
- The recovery code is `K` in base32 (Crockford), in groups of four.
