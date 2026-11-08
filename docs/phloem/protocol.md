# Phloem: the protocol, the names and the boundaries

Phloem is the service of Betula Sync (`research/sync/entwurf.html`, owner 2026-11-05: „Passt, bau
Schritt 1 jetzt so"). It keeps sealed records of the profiles that sync and hands them between the
devices of a profile; it can open none of them. This file is the contract between its three parts:
the service (`phloem/`, Go), the browser (`folia/crates/sync` and the app), and the deployment
(`deploy/`). Step 1 is everything below except what is marked step 2 or 3; step 3 is the parts of
shared calendars and the parcels that wait up to 7 days (folia/design/teilen/teilen.html, owner
2026-11-08), since Phloem 0.2.0; step 2 the live way between the devices (WebRTC: the signals, STUN
and TURN; owner, 2026-11-08: „time für phase 2 bei dem sync"), since Phloem 0.3.0.

## 1. Names

| | |
|---|---|
| instances | one per site, each a stack of its own: `phloem` for https://betula.app, `phloem-canary` for https://canary.betula.app |
| networks | `<stack>_in` (`phloem_in`, `phloem-canary_in`): overlay, `--internal`, not attachable, created by `deploy/vps/30-docker.sh`; its members are Traefik and that instance, nothing else. `<stack>_out`: the stack's own overlay (not internal), its only member that instance; its way out, used by TURN only (step 2) |
| port | 8200/tcp inside the container, on `<stack>_in` only. With TURN (step 2) also `PHLOEM_TURN_PORT` over UDP and TCP and the relays' UDP ports `PHLOEM_TURN_RELAY_PORTS`, published in host mode at the same numbers (`phloem`: 3478 and 49160–49223, `phloem-canary`: 3479 and 49224–49239) |
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
| devices → Phloem's STUN and TURN (step 2) | yes | its port over UDP and TCP and its relays' over UDP, in host mode; TURN over TLS on 443 through Traefik by name (`PHLOEM_TURN_TLS_HOST`, a TCP router to the TURN port over `<stack>_in`) |
| Phloem's relays → the devices (step 2) | the space's alone | UDP from a relay's port, over `<stack>_out`, to the address another socket of the allocation's space came from (section 6); the firewall's `turn.nft` lets out a relay's datagram to a public address and nothing else |
| Folia → Phloem | only through Traefik | as an outside client, `https://<host>/sync/v1/` (step 3) |
| monitoring → Phloem | reads the file | no network |

## 3. HTTP

Every request names its space and proves it in headers, never in the path (Traefik's access log
keeps paths): `Betula-Space: <space>`, `Authorization: Bearer <token>`. All values are base64url
without padding. Bodies are JSON. Errors are `{"error": "<code>"}` with a fitting status.

| | |
|---|---|
| `POST /sync/v1/space` | makes a space. Body `{"space": "<16 bytes>", "token": "<32 bytes>", "device": "<16 bytes>"}`. Phloem keeps SHA-256 of the token. 201 `{"slot": 1}`; 409 `exists`; 429 `slow_down` (20 spaces per address and hour, 500 per hour in all; an IPv6 address counts by its /64; a 409 costs nothing); 507 `budget` (the budget is reached: no new spaces, the old ones go on) |
| `DELETE /sync/v1/space` | „Sync beenden": the space and everything of it, at once; connected devices hear `gone`. 204; 401 also when the space is gone already |
| `POST /sync/v1/pair` | leaves a sealed parcel for a device to come, or for a one-time link (authorised by the space's headers). Body `{"id": "<16 bytes>", "box": "<at most 4 KiB>", "ttl": <seconds, optional>}`. Kept `ttl` seconds, 60 to 604800 (7 days), else 600. 201 `{}`; 409 `exists` (also for an id whose outcome Phloem still remembers); 413 `too_big`; 429 `slow_down` (32 parcels waiting per space; a parcel counts as a push of the space); 507 `budget` |
| `GET /sync/v1/pair` | `Betula-Pair: <id>`: the parcel once, then it is gone. 200 `{"box": "…"}`; 404 with the code of what became of it, for 30 days after: `taken` (opened already), `expired`, `withdrawn`; else `not_found`. 429 `slow_down` (120 per address and hour, found or not) |
| `DELETE /sync/v1/pair` | `Betula-Pair: <id>` with the space's headers: „Zurückziehen", the space's parcel that waits is `withdrawn` from now on. 204, also for an id that is not the space's or waits no longer (nothing changes then) |
| `GET /sync/v1/pairs` | the space's parcels, waiting and for 30 days after: 200 `{"parcels": [{"id", "state": "waiting" \| "taken" \| "expired" \| "withdrawn", "at": <ms>}…]}`, `at` when a waiting one expires, else when it happened |
| `PUT /sync/v1/part` | `Betula-Part: <id>` (16 bytes the device chose) with the space's headers: a part of a shared calendar (step 3), made or replaced. Body `{"data": "<sealed, at most 16 KiB>"}`. 200 `{"rev": <n>}`, the part's version, 1 for a new one and one more with every replacement; its watchers hear `part`. 409 `exists` (the id is another space's part, or a part deleted: a part never comes back); 413 `too_big`; 429 `slow_down` (a part counts as a push of the space, and a space makes at most 32 new parts a day: a deleted part is remembered 395 days, and this bounds what a space leaves behind), `too_many` (32 parts per space); 507 `budget` (a new part) |
| `DELETE /sync/v1/part` | `Betula-Part: <id>` with the space's headers: „Nicht mehr teilen", the space's part is deleted and `gone` from now on; its watchers hear it. 204, also for an id that is not the space's (nothing changes then) |
| `GET /sync/v1/part` | `Betula-Part: <id>`, optionally `If-None-Match: "<rev>"`: the part, for anyone with its id (it is sealed with a key Phloem never sees). 200 `{"data": "…", "rev": <n>}` with `ETag: "<n>"`; 304 when the version is the one named; 410 `gone` (deleted, or its space was: kept 395 days); 404 `not_found`; 429 `slow_down` (3600 per address and hour, found or not) |
| `GET /sync/v1/ws` | the WebSocket of a device (section 4); 503 `busy` when the instance has 4096 sockets, or the address 32 that have not said hello yet |
| `GET /healthz` | 200 `ok`; not routed by Traefik |
| `GET /status` | the instance's numbers as JSON, for `phloem status`; answers loopback only, not routed by Traefik |

A parcel's body may name how long it waits: `"ttl": <seconds>`, 60 to 604800 (7 days), else 600;
at most 32 parcels wait per space. What became of a parcel (`taken`, `expired`, `withdrawn`) is
remembered 30 days, of a space's newest 256 at most. A space that is deleted takes its parcels and
what became of them with it, and its parts are `gone` from then on.

The other codes: 400 `invalid` (a header or a body that is not what this table says), 401
`unauthorized` (no or a wrong `Betula-Space` and token, or no such space: the same answer), 403
`origin`, 405 `method`, 500 `internal`.

`PHLOEM_ORIGINS` (comma separated, default the site's own origin: the one of the request's host)
are the origins Phloem accepts for the WebSocket and answers CORS for (development runs the app on
another port). A request with another `Origin` gets 403 `origin`; one without (not a browser) is
taken.

## 4. The WebSocket

Text frames, one JSON object each, `"t"` its type.

From a device:

| | |
|---|---|
| `{"t": "hello", "space", "token", "device", "since": <nr>}` | the first frame, within 10 s; nothing else before it. `since` is the number of the last entry the device holds (0 for none) |
| `{"t": "push", "id": "<the device's own id of it>", "data": "<sealed batch>"}` | one change: the sealed writes of it, at most 64 KiB (decoded). `id`: 1 to 64 characters |
| `{"t": "snapshot", "upto": <nr>, "data": "<sealed>"}` | every record of the profile as of entry `upto`, at most 6 MiB (decoded); Phloem keeps the newest one and forgets the entries up to `upto`. The first device sends one with `upto: 0` right after `POST /sync/v1/space`, before there is any entry |
| `{"t": "leave"}` | „Nur dieses Gerät trennen": its slot is free again; the socket closes (1000) |
| `{"t": "delete"}` | the same as `DELETE /sync/v1/space` |
| `{"t": "watch", "parts": ["<id>"…]}` | the parts of shared calendars this device follows (step 3), at most 64: the list from now on, in place of the one before. Counts as a push of the space |
| `{"t": "signal", "to": <slot>, "data": "<sealed, at most 8 KiB>"}` | a sealed signal of WebRTC (an offer, an answer, a candidate: section 5) for the device of slot `to` in the same space (step 2). A device not online gets nothing, and the sender hears nothing of it: presence says who is. 20 a second per socket, bursts of 100 (signals and `turn` together), beyond them `error slow_down` |
| `{"t": "turn"}` | the ICE servers and this socket's TURN credential, please (step 2) |

From Phloem:

| | |
|---|---|
| `{"t": "welcome", "slot", "head", "now", "devices": [<slots online>], "live": true, "snapshot": null \| {"upto", "data"}, "entries": [{"nr", "slot", "data"}…]}` | after a valid hello. `live` (since Phloem 0.3.0): this Phloem takes `signal` and `turn`; a device sends them only where the welcome says so, since an older Phloem closes a socket on a frame it does not know. `slot` is the device's place in the space (1–16, kept by its device id); `now` the server's clock in ms, for the device's clock. The snapshot comes whenever there is one and `since` is 0 or lies before its `upto`, and then the entries after it; otherwise the entries after `since`. A `since` beyond `head` (Phloem was restored from an older backup) is answered as 0: the snapshot and every entry |
| `{"t": "stored", "id", "nr"}` | a push is kept as entry `nr`. A push whose `id` the same slot pushed already among the entries since the snapshot is the same push sent again (its `stored` was lost: the device sends its outbox again after a reconnect): it gets `stored` with the old `nr`, and nothing is added or sent to the others |
| `{"t": "entry", "nr", "slot", "data"}` | another device's push, to every other device online |
| `{"t": "snapshotted", "upto"}` | the snapshot is kept. A snapshot older than the one kept is not: `upto` then names the kept one's |
| `{"t": "presence", "devices": [<slots online>]}` | a device came or went (not when a device's newer socket takes over from its older one) |
| `{"t": "compact"}` | the entries since the snapshot exceed 128 KiB: the device should send a snapshot. It goes to the device whose push crossed the line, or that came while above it, once until the next snapshot |
| `{"t": "error", "code", "id"?}` | `unauthorized` (and the socket closes), `too_big`, `compact_first` (above 256 KiB a push waits for a snapshot), `slow_down` (more than 50 pushes a second in a space, bursts of 200; a snapshot counts one push per 64 KiB begun), `full` (16 devices; the socket closes). `id` is the push's, for a push |
| `{"t": "gone"}` | the space was deleted; the socket closes |
| `{"t": "watching", "parts": [{"id", "rev"} \| {"id", "gone": true}…]}` | the answer to `watch`: each part's version now, `rev` 0 for a part Phloem does not have |
| `{"t": "part", "id", "rev"}`, `{"t": "part", "id", "gone": true}` | a part watched was replaced, or deleted, to every socket that watches it |
| `{"t": "pair", "id", "state": "taken"}` | a parcel of the space was opened, to every device of the space online |
| `{"t": "signal", "from": <slot>, "data"}` | another device's signal, named by the slot it came from (step 2) |
| `{"t": "turn", "urls": [...], "user", "password"}` | the answer to `turn` (step 2): `stun:<host>:<port>`, `turn:<host>:<port>?transport=udp`, `…?transport=tcp` and, where the instance has a name for it, `turns:<name>:443?transport=tcp`; a credential (two random strings) that holds while this socket lives (section 6). `{"t": "turn", "urls": []}` where the instance has no TURN |

At most 16 sockets per space; a second hello of one device closes its older socket. A space not
touched (hello, push, snapshot, pair, part) for 395 days is deleted. Every limit is Phloem's own
and holds whatever a client does.

Phloem pings every socket every 30 s; one that does not answer within 15 s is closed, and the
others hear `presence`. A frame of more than 8.5 MiB (a 6 MiB snapshot in base64url and its JSON
fit) closes the socket at once (1009); a frame against this section (another type, a second
hello, a value that is not base64url without padding, a snapshot beyond `head`, binary) closes it
with 1008, and so does a `watch` of more than 64 parts or with an id that is not 16 bytes, and a
`signal` without a slot of 1–16 or with data that is missing or larger than 8 KiB. The close
codes:

| | |
|---|---|
| 1000 | `left` (after `leave`), `gone` (after `gone`) |
| 1001 | `restart`: Phloem stops; come back with `since` |
| 1008 | `unauthorized`, `full`, or a frame against the protocol |
| 1009 | a frame too big |
| 1011 | `internal` |
| 4000 | `replaced`: a newer socket of the same device took over; do not come back on this one |
| none | the device stopped answering, or fell so far behind that Phloem let it go: come back with `since` |

## 5. In the browser (for the service: opaque)

- The profile key `K`: 32 random bytes, on the devices only. HKDF-SHA256 (salt `betula-sync-v1`)
  gives `space` (16 bytes, info `space`), `token` (32 bytes, info `token`), the AES-256-GCM key
  of the data (info `data`) and the one of the signals (info `live`, step 2). A device id is 16
  random bytes of its own.
- Sealed: a 12-byte random nonce, then AES-256-GCM's ciphertext and tag. The associated data names
  the space and what it is: `space ‖ "batch"`, `space ‖ "snapshot" ‖ upto` (8 bytes, big endian),
  `space ‖ "pair"`.
- A batch holds the writes of one change, a snapshot every record with its clocks
  (`folia_sync::codec`), uncompressed (a typical snapshot is 5.5 KB).
- The pairing link `https://<host>/sync#<id ‖ key>` (16 + 32 bytes): `id` finds the parcel,
  `key` opens it (AES-256-GCM, associated data `"pair"`); the parcel holds the profile keys chosen.
  The page is Folia's (Traefik sends Phloem `/sync/v1/` alone); until 2026-11-08 it was
  `/verbinden#…`, which Folia leads to `/sync` for good, the part after `#` carried over by the
  browser.
- A code that asks for a profile, `https://<host>/pair#<id ‖ public key>` (16 + 65 bytes): the device that shows it made an ECDH key pair on P-256; the device that scans it makes
  one too and leaves at `POST /sync/v1/pair`, under the code's `id`, its own public key (65 bytes)
  followed by the profile keys sealed with HKDF-SHA256 of the shared secret (salt `id`, info
  `betula-request`; associated data `"request"`). The asking device asks `GET /sync/v1/pair` for the
  `id` until it is there. Phloem sees a parcel like any other.
- The recovery code is `K` in base32 (Crockford), in groups of four.
- A shared calendar that stays current (step 3) is a part: `folia_calendar::live::LivePart` as a
  pack code, sealed with a key of its own, `kp` (32 random bytes), associated data
  `"part" ‖ id`. The share's record in the profile holds `id` and `kp`, so that every device of
  the profile renews the part when the calendar changes; nothing leads from the part to the
  profile.
- The link of a part, `https://<host>/studyplan#live=<id ‖ kp>`, and its calendar subscription,
  `https://<host>/calendar/s/<id ‖ kp>.ics` (`/en/calendar/s/…` in English): a calendar service
  sends no key of its own, so the key is in the address; Folia fetches the part through Traefik,
  opens it in memory for the answer and keeps nothing.
- A one-time link, `https://<host>/studyplan#once=<id ‖ key>`, is a parcel that waits up to 7
  days, associated data `"share"`: JSON `{"v": 1, "part": "<id>", "key": "<kp>"}` for a calendar
  that stays current, `{"v": 1, "copy": "<LivePart code>"}` for a copy of now.
- A signal (step 2, `folia/crates/stores/src/sync/live.rs`) is sealed with the live key,
  associated data `space ‖ "signal" ‖ from ‖ to` (the two slots, 2 bytes each, big endian), so
  that Phloem can neither read it nor hand it to another device than the one meant: JSON
  `{"k": "offer", "n", "sdp", "relay"}`, `{"k": "answer", "n", "sdp"}`, `{"k": "ice", "n", "c",
  "mid", "line"}`, `{"k": "bye", "n"}`; `n` names the attempt (a random number of the device
  that offered it). Of two devices the one of the smaller slot offers. The first attempt takes
  STUN alone; a later one (`"relay": true`) takes TURN as well.
- The data channel (negotiated, id 0, unordered, reliable) carries `{"t": "push", "data":
  "<sealed batch>"}`: each change of the device, as the batch it pushes to Phloem (associated data
  `space ‖ "batch"`), at most 60 000 characters (a larger one goes through Phloem alone). What
  comes over it is taken as an entry is, without its number; Phloem's copy of it works no second
  time.

## 6. TURN (step 2)

Pion's TURN (`github.com/pion/turn/v5`) inside Phloem, its rules Phloem's own
(`phloem/internal/server/turn.go`):

- **Who**: a credential is made on a socket's first `turn` frame, two random strings, and holds
  while that socket lives; a socket that ends takes it with it (its allocations then end with
  their lifetime, 10 minutes at the latest, and get no new permission).
- **To whom**: a permission (and a channel binding) only for the address another socket of the
  allocation's space came from (Traefik's `X-Real-Ip`), never for an address that is none on the
  internet (RFC 6890: private, shared, loopback, link local, documentation, benchmarking,
  multicast, reserved) nor for the instance's own (`PHLOEM_TURN_IP`): no relay to Phloem itself,
  so a relay never meets another. A relay that was to send elsewhere drops the datagram and counts
  it (`phloem_turn_total{event="forbidden"}`, an alert); the firewall's `turn.nft` holds the same
  line once more.
- **How much**: relays over UDP alone (no TCP relays, RFC 6062: WebRTC needs none), each at most
  128 KiB a second each way, all of them at most 4 MiB a second each way; at most 8 allocations per
  socket, 32 per space, and as many as the instance has relay ports. Beyond them Pion answers 486.
- **Where**: STUN and TURN over UDP and TCP on `PHLOEM_TURN_PORT` of every address of the
  container, published in host mode; TURN over TLS through Traefik, which ends TLS for
  `PHLOEM_TURN_TLS_HOST` and hands the stream to the same TCP port.
