# Cortex, the cache between Betula and the internet

> **Status: built 2026-10-02/03, shipped by `deploy/ship-cortex.sh`, not yet deployed.** One
> service that every outgoing data request of the project may go through: it fetches from public
> hosts under one floor per host for all clients together, keeps every answer that differed with
> its history, stores named files over REST, and replicates all of it to a second instance that
> takes over when the first one goes away. The name: Betula is the birch, Radix its root, Folia
> its leaves, and the cortex is its bark, which everything that enters the tree passes through.
> The concept this replaces asked nine questions; the owner's answers (2026-10-02) are in §1.

## 1. What it is for, and the owner's decisions

Cortex is a general caching service for requests (owner, 2026-10-02: „Bau cortex einfach als
generellen chaching service für requests."): nothing in it is specific to Betula. Any client may
ask it for any public URL; what a host gets (how many requests at once, how far apart, what is
kept of it) is a file of per-host settings (§5). Today its clients are Radix's crawl and the
statute download (§8); the model hosts of Hugging Face have their entries ready.

| # | The owner, 2026-10-02 | What Cortex does |
|---|---|---|
| 1 | „Also wir bauen das initial auf einem sever, langfristig ist ja die überlegung wert dann ein kubernetes kluster zu verwenden" | two instances on one host (one-node swarm), leader elected with `flock`; the election is an interface a Kubernetes Lease can implement later (§6.9), not built |
| 2 | „Ja die letzen 5 min sind egal" | replication is asynchronous for everything (pages, files, blobs): a crash may lose what the follower had not received yet; the alert fires when the follower is more than 5 minutes behind |
| 3 | „Offline ist wunsch des clients" | `mode=offline` is a parameter the client chooses; no tokens, no per-client policy, nothing enforced |
| 4 | „Ja es können ruhig alle requests durch cortex gehen" | Cortex is passive (it fetches only when asked) and holds every host to one floor for all clients together |
| 5 | „Vorschlag mit 180tagen und + aktuell klingt gut. Aber die historischen daten müssen unbedingt komprimiert sein und nur bei changes speichern, also wenn sich die hash ändert." | current versions stay for ever, a superseded one 180 days; a new version only when the sha256 (or the status) changes; every blob gzip-compressed at best compression unless that saves less than 1 % (§3) |
| 6 | „Cortex ist nur für daten da." | Gemini does not go through Cortex; neither do model weights by default (§12) |
| 7 | „Also datein sollen auch einfach per web request anfragbar sein. Also so dass man die per rest request ablegen kann und dann per rest auslesen kann." | named files: `PUT` and `GET /v1/files/{name}`, with versions (§4.4) |
| 8 | „Ja möglichst kein zugang mehr" | Radix's networks are internal; only Cortex has a way out, Gemini is the exception (`deploy/README.md` section 14) |
| 9 | „Ja go ist super und cortex auch" | Go, in this repository: `cmd/cortex`, `internal/cortex/…` |
| 10 | „Bau cortex einfach als generellen chaching service für requests." | any public host, per-host settings by configuration (§5) |

Not through Cortex: Gemini (`generativelanguage.googleapis.com`, POSTs with a key; decision 6),
Folia's snapshot downloads from Radix (internal), the browsers, and the canary agent's downloads
from GitHub (checked against GitHub's own sha256 already).

## 2. Overview

```
one host (betula.app, one-node swarm)

Radix, crawling colour ──┐  overlay "cortex"       ┌─ cortex_a: LEADER ─────────────┐
Radix, other colours   ──┤  (internal: true)       │  index.db (SQLite) + blobs     │──▶ qis.b-tu.de
radix download-statutes──┼─ either instance ──────▶│     │ journal (NDJSON), blobs  │    www.b-tu.de
cortex put / get, curl ──┤  answers; what needs    │     ▼                          │    opus4.kobv.de
Prometheus (scrapes)   ──┘  the leader goes there  │  cortex_b: FOLLOWER            │    any public host
                                                   │  own volume, own copy          │    (stack network
                                                   └────────────────────────────────┘     with a way out)
          volume cortex_lock: /lock/leader.lock (flock), leader.json, head.json
```

Cortex fetches only when a client asks. What is due and when stays the client's decision
(`crawl.Due`, the off-peak window, the caps per cycle); how fast a host is asked at most is
Cortex's.

## 3. Storage

Each instance has a data directory of its own (`--data`, `/data` in the image):

```
DIR/index.db                       the index: SQLite in WAL mode (modernc.org/sqlite, no CGO)
DIR/blobs/sha256/<hh>/<hex>        a blob stored as it came (raw)
DIR/blobs/sha256/<hh>/<hex>.gz     a blob stored gzip-compressed
DIR/tmp/                           writes in progress; emptied at start
```

**Blobs.** Every body and every uploaded file is a file named by the sha256 of its original
bytes, the same content once however many URLs, versions or files deliver it. A body streams to
`tmp/` while it is hashed and compressed (gzip, best compression; memory stays at about 2 MB
whatever the size), is synced and renamed into place; a blob that exists already is kept and its
time of last use renewed. **The gzip rule:** the compressed form is kept when it is at least 1 %
smaller than the original, else the original. Every 32 MB the compressor checks whether it still
saves 1 %, and stops when it does not, so an already compressed file (a PDF, model weights) is
not run through best compression to its end. A raw blob is served with `Range` directly from
the file; a compressed one is decompressed on the way (and handed as it is to a client that
accepts gzip, §4.5).

**Index.** One row per request identity (`entry`: the canonical key, URL, host, `source`), one
per answer that differed (`version`: status, sha256, size, the kept headers, `fetched_at` when
this content was first fetched, `checked_at` when it was last fetched, `superseded_at` once the
next version replaced it), the named files (`file`, `file_version`, a deletion is a tombstone
version), the journal (§6.4) and `meta` (epoch, the trim mark). Times are UTC with microseconds,
fixed width; the API writes them in RFC 3339.

**The key.** `store.Canonical`: only `http` and `https`; scheme and host in lower case; the
default port and the fragment dropped; an empty path becomes `/`; path and query exactly as sent
(no decoding, no sorting: QIS's parameter order is part of its URLs). A URL with user info, text
that is not UTF-8, and a host that is not ASCII (write it in punycode, `xn--…`) are refused. The
key is `GET <url>`, then `accept: …` and `accept-language: …` when the request had them: two
clients that ask with another `Accept` get two entries.

**Versions.** A fetch whose status and sha256 are those of the current version only moves its
`checked_at` (a `check`); anything else is a new version, and the old one gets `superseded_at`. A
page fetched daily that changes twice a year costs two versions and two blobs, not 365. Stored are
the statuses 200, 203, 204, 404 and 410 (a page that is gone is an answer), with the headers
`Content-Type`, `Content-Language`, `Content-Disposition`, `Last-Modified` and `ETag` (each value
valid UTF-8 and at most 8 KiB, a longer one is dropped). Upstream's `Cache-Control`, `Expires` and
`Vary` are ignored: Cortex is an archive for its clients, not a shared HTTP cache. Each version
also keeps the URL that answered after redirects, internally (§5).

**Retention (180 days).** Every `--prune-interval` (1 h, and once at start) the leader removes,
in one journaled step the follower repeats exactly: versions and file versions superseded more
than `--history` (`4320h`, 180 days) ago, files whose current version is a deletion older than
that, and entries left without a version. Current versions stay for ever. Both instances then
trim their journal (§6.4) and remove blob files no row references once they have been unused for
`--blob-grace` (7 days). An empty run writes nothing.

**Files.** A named file is a name (1 to 1024 bytes of UTF-8 in `/`-separated segments, none empty,
`.` or `..`, no control characters, no leading or trailing `/`) with versions: a `PUT` with the
content and content type of the current version adds none (`200`), anything else adds one
(`201`); a `DELETE` adds a tombstone. Earlier versions stay readable by id and by time until
retention.

**Journal.** Every write to the index is one transaction that also appends one journal entry
(`seq`, `epoch`, `at`, `op`, the rows it wrote with their ids, the blob it needs): `epoch`,
`version`, `check`, `entry_delete`, `file_version`, `prune`. A follower applies the same entries
to the same rows (§6.4), so both indexes stay identical row for row. A row that names a blob is
never durable before the blob is: a blob's directory is synced before the next commit of the
index.

## 4. The API

One listener per instance (`--addr`, port 8100), on the overlay `cortex` as `http://cortex_a:8100`
and `http://cortex_b:8100`, no published port. Either instance answers every request (§6.6): JSON
for metadata, the raw body for content. Every answer carries `Cortex-Instance: <instance>;
role=<leader|follower>`, the role the instance held when it wrote the answer (a forwarded answer
carries the leader's). The server closes keep-alive connections idle for 2 minutes and wants the
request headers within 10 s; there is no read or write timeout, because of large uploads,
downloads and the journal's long polls.

### 4.1 Fetching: `GET /v1/fetch`

`GET` and `HEAD`. Bad parameters answer `400 bad-request`.

| Parameter | Meaning | Default |
|---|---|---|
| `url` | absolute `http`/`https` URL (§3, „The key") | required |
| `mode` | `offline`, `cache` or `refresh` (below) | `cache` |
| `max_age` | Go duration, `0` allowed: in `cache`, a stored answer checked less than this long ago is served without asking upstream | the host's `max_age` (§5) |
| `stale` | `if-error`: when upstream fails, serve the stored version, marked stale; `never`: answer with the failure | `if-error` |
| `expect` | `sha256:<hex>`: a stored version with this hash is fresh at any age; a version of another hash counts as not stored; a download of another hash is refused (`502 hash-mismatch`) and not stored | none |
| `at` | RFC 3339: the version current at that time (implies `offline`) | none |
| `source` | `[a-z0-9_.-]{1,64}`, for metrics and logs: `qis_tree`, `module_page`, `statute`, … | `unknown` |

The client's `Accept` and `Accept-Language` are part of the key and go upstream; `User-Agent` goes
upstream when given (else the policy's) and is not part of the key. Every other request header is
ignored; `Cookie` and `Authorization` are never sent upstream.

| `mode` | stored, age ≤ `max_age` | stored, older | not stored |
|---|---|---|---|
| `offline` | served | served | `504 offline-miss` |
| `cache` | served | fetched, stored, served | fetched, stored, served |
| `refresh` | fetched, stored, served | fetched, stored, served | fetched, stored, served |

The age is the time since the current version's `checked_at`. With `at`, the newest version
first fetched at or before that time and not superseded by then is served, else `504
offline-miss`. Concurrent requests for the same key and `expect` wait for one upstream request
(coalescing), which runs detached from the client (bounded by the host's `queue_wait` plus twice
its `timeout`): a client that gives up does not end a download others wait for. Coalesced
requests share the first one's `User-Agent` and `source`.

**Upstream's answer.** A storable status (§3) is stored and served; a `304` to Cortex's
conditional request confirms the current version (a `check`) and serves it. Anything else (a
network error, a timeout, a 5xx or another status, too large, the wrong type, a hash mismatch, the
host paused or its queue full, the host or address not allowed) is served as the stored version
when `stale=if-error` and one is stored (`Cache-Status: Cortex; hit; detail=stale-if-error`,
`Cortex-Upstream-Error: <code>`), else answered with Cortex's error (§4.8). A failure of Cortex's
own store while it saves a download is `500 internal`, not upstream's.

### 4.2 The answer of a fetch

The status is the stored upstream status (`200`, `404`, …), whether it came from the store or the
network; the body is the raw body. `If-None-Match` with Cortex's `ETag` gives `304` for a 2xx; a
`200` answers `Range`, `If-Range`, `If-Modified-Since` and `HEAD`.

| Header | Value |
|---|---|
| `Content-Type`, `Content-Language`, `Content-Disposition` | upstream's, as kept |
| `ETag` | `"sha256:<hex>"` of the content: the same content has the same tag, whenever and wherever it was fetched |
| `Last-Modified` | upstream's if it sent one, else `fetched_at` |
| `Age` | seconds since `checked_at` |
| `Cache-Status` | RFC 9211: `Cortex; hit`, `Cortex; fwd=miss; fwd-status=200; stored`, `Cortex; fwd=stale; fwd-status=304; stored`, `Cortex; fwd=request; …` (refresh), `Cortex; hit; detail=stale-if-error` |
| `Cortex-Fetched-At`, `Cortex-Checked-At` | when this content was first and last fetched upstream (RFC 3339, microseconds) |
| `Cortex-Version`, `Cortex-Status` | the version's id; upstream's status |
| `Cortex-Upstream-ETag` | upstream's own `ETag`, if it sent one |
| `Cortex-Upstream-Error` | with `detail=stale-if-error`: the code of the failure the stored version stands in for |

**Ranges.** One or two ranges are honoured in any order. Three to 16 are honoured only when they
ascend, do not overlap, have no suffix range (`-N`) and only the last is open-ended; any other
`Range` is answered `200` with the whole content (RFC 9110 §14.2). The reason: a compressed blob is
decompressed from the start again for every backward seek. One read of a compressed blob may
decompress about twice its content; past that the answer is cut off. This holds for
`/v1/fetch`, `/v1/files` and `/v1/blobs`.

### 4.3 What the store holds: `/v1/entries`

| Request | Answer |
|---|---|
| `GET /v1/entries?url=…[&accept=…][&accept_language=…]` | `{"entry":{…},"versions":[…]}`, newest first (id, status, `sha256:<hex>`, size, headers, `fetched_at`, `checked_at`, `superseded_at`); `404 not-found` when none |
| `GET /v1/entries?host=&source=&changed_since=&cursor=&limit=` | `{"entries":[{…,"current":{…}}],"next_cursor":"…"}`: entries with a current version, in the order they were created; `changed_since` (RFC 3339) keeps those whose current version was first fetched at or after it; `limit` 100, at most 1000 |
| `DELETE /v1/entries?url=…` | the entry and its versions are forgotten: `204`; `404` when none |

Both roles answer the reads from their own copy (a follower may be a moment behind).

### 4.4 Named files: `/v1/files`

| Request | Answer |
|---|---|
| `PUT /v1/files/{name}[?expect=sha256:<hex>]` | the body becomes the file's content with its `Content-Type` (default `application/octet-stream`): `201` for a new version, `200` when content and type are the current ones; JSON `{name, version, sha256, size, content_type, created_at}`, `ETag`, `Cortex-Version`. `If-Match: "sha256:<hex>"` (`*`: a current file must exist) → `412 precondition-failed` when it does not hold; `422 hash-mismatch` when the content has another hash than `expect`; `400 bad-name` |
| `GET`, `HEAD /v1/files/{name}` | the current content, with `?at=<RFC 3339>` the version current then, with `?version=<id>` that one; `ETag: "sha256:<hex>"`, `Cortex-Version`, `Content-Type`, `Last-Modified` (when the version was stored), `Range`, `If-None-Match` → `304`; `404 not-found` when there is none or it is deleted |
| `GET /v1/files/{name}?versions` | `{"name":…,"versions":[…]}`, deletions included |
| `DELETE /v1/files/{name}` | a tombstone: `204`; `404` when there is none or it is deleted already |
| `GET /v1/files?prefix=&cursor=&limit=` | the current files (not deleted), by name: `{"files":[…],"next_cursor":"…"}`; `limit` 100, at most 1000 |

A `PUT` and a `DELETE` of one name are serialised with each other, so an `If-Match` holds from
the check to the write. There is no size limit of Cortex's own: the disk is the limit.

```bash
curl -T report.pdf -H 'Content-Type: application/pdf' http://cortex_a:8100/v1/files/reports/2026/report.pdf
curl -o report.pdf http://cortex_b:8100/v1/files/reports/2026/report.pdf      # either instance
```

### 4.5 Blobs by hash: `/v1/blobs/sha256:<hex>`

- `GET`, `HEAD`: the content, `Cache-Control: public, max-age=31536000, immutable`,
  `ETag: "sha256:<hex>"`, `Range`. A client that accepts gzip gets a compressed blob as it is stored
  (`Content-Encoding: gzip`) unless it asks for a `Range`. `404 not-found` when it is not there.
- `PUT`: an upload by hash, checked while it is written: `201`; `200` without reading the body when
  the blob is there; `422 hash-mismatch`. A blob no row references is removed after the grace
  period (§3): a `PUT` of a blob alone is no way to keep it.

### 4.6 Health, status and administration

| Path | Purpose |
|---|---|
| `GET /livez` | `200` while the process serves and the index answers a trivial query within 2 s, else `503`: the container's health check (`cortex healthcheck`) |
| `GET /healthz` | `200 {"status":"ok","role":…}`, or `503 {"status":"unhealthy","role":…,"problems":[…]}`: the index does not answer, no leader known for 60 s, the follower more than 300 s behind, blobs the index references are missing (on the follower, or on a leader promoted while it was still fetching them) |
| `GET /status` | one JSON object: `instance`, `role`, `epoch`, `seq`, `oldest_seq`, `head_at`, `leader` {`instance`, `url`, `epoch`, `since`} or null, `follower` {`state`, `leader_url`, `lag_seconds`, `lag_entries`, `blobs_missing`}, `healthy`, `problems`, `hosts` [{`host`, `queue`, `in_flight`, `paused_until`, `failures_in_row`}], `store` (counts and bytes, the last background count: at most about a minute old, null with `store_error` until the first count), `last_prune`, `version`, `build`, `started_at`, `log` (the 20 latest warnings and errors) |
| `GET /metrics` | §10 |
| `POST /v1/admin/step-down` | the leader hands over (§6.7) and stays out of the election for 15 s: `200`; `409 not-leader` on a follower, `409 cannot-step-down` on a single instance |
| `POST /v1/admin/prune` | the retention now, on the leader (a follower forwards it): its report as JSON |

### 4.7 Replication (the leader's side)

| Request | Answer |
|---|---|
| `GET /internal/v1/journal?after=<seq>&epoch=<e>&sum=<hex>&wait=<duration>` | up to 1000 entries (about 8 MiB) after `after` as NDJSON (`application/x-ndjson`), waiting up to `wait` (at most 30 s) for the first. `Cortex-Head-Seq`, `Cortex-Head-Epoch`, `Cortex-Head-At`: the leader's newest entry when the page was read; the page holds every entry up to it unless the limits cut it short, never one beyond. `409 diverged` when the leader's entry at `after` has another epoch or sum, or there is none; `410 trimmed` when the journal no longer reaches back that far; `409 not-leader` on a follower |
| `GET /internal/v1/snapshot` | the leader's index as an SQLite file (`application/vnd.sqlite3`), `Cortex-Seq`, `Cortex-Epoch`; one copy at a time (a second request waits), ended when the client goes away. `HEAD` answers the position without a copy |

### 4.8 Errors

An error is JSON `{"error":"<code>","message":"…"}` with the header `Cortex-Error: <code>` and
`Cache-Control: no-store`. An unknown path is `404 not-found`, a method a path does not take `405
method-not-allowed` with `Allow`.

| Code | Status | When |
|---|---|---|
| `bad-request` | 400 | a parameter is missing or invalid, a body could not be read |
| `bad-name` | 400 | not a file name (§3) |
| `not-found` | 404 | no such entry, file, blob or endpoint |
| `method-not-allowed` | 405 | with `Allow` |
| `precondition-failed` | 412 | `If-Match` of a `PUT` does not hold |
| `hash-mismatch` | 422 / 502 | an upload / a download with another sha256 than `expect` |
| `internal` | 500 | Cortex itself failed (its store, the disk) |
| `upstream-failed` | 502 | upstream failed (network, timeout, 5xx, a status Cortex does not store); the message names it |
| `wrong-type` | 502 | a 2xx that is not the host's `expect_type` (a bot-protection page instead of a PDF) |
| `too-large` | 502 | more than the host's `max_body` |
| `forward-failed` | 502 | a write forwarded to the leader, which failed after it got the request: it may have applied it, so it is not sent again elsewhere |
| `host-busy` | 429 | the host's queue: not started within `queue_wait`; `Retry-After` |
| `host-paused` | 503 | the host's breaker or upstream's `Retry-After`; `Retry-After` |
| `host-not-allowed` | 403 | the host (or a redirect's) is not in `allow`, or not ASCII |
| `address-not-allowed` | 403 | the name resolves to an address no request may reach (§5) |
| `not-leader` | 409 | only the leader does this (step-down, journal, snapshot) |
| `cannot-step-down` | 409 | a single instance has nobody to hand over to |
| `diverged` | 409 | the follower's journal is not the leader's: it takes a snapshot |
| `trimmed` | 410 | the journal no longer reaches back to the follower: it takes a snapshot |
| `offline-miss` | 504 | `offline` or `at`, and nothing is stored (as `only-if-cached`, RFC 9111) |
| `no-leader` | 503 | no leader can be reached; `Retry-After: 1`, the client tries the other instance |
| `blob-missing` | 503 | a version whose blob neither this instance nor the other one can serve yet; `Retry-After: 1` |

## 5. Upstream: politeness and safety

**The host policy**, a JSON file (`--hosts`, in a deploy `deploy/config/cortex/hosts.json`, mounted
read-only as `/etc/cortex/hosts.json`). Checked every 30 s and read again when its modification
time or size changed: `deploy/sync.sh` is all a change needs. A file that is broken at start stops
Cortex (exit 2); later, a broken one keeps the last good policy (WARN `policy.invalid`). Without a
file: every public host with the defaults, `qis.b-tu.de` and `www.b-tu.de` 500 ms apart,
`opus4.kobv.de` 2 s apart, a month fresh, PDFs only.

```json
{
  "allow": ["*"],
  "user_agent": "Cortex/1.0 (+https://betula.app; info@betula.app)",
  "default": {"concurrency": 1, "pause": "0s", "max_age": "24h", "queue_wait": "60s",
              "breaker_failures": 10, "breaker_pause": "15m", "timeout": "10m", "max_body": 8589934592},
  "hosts": {
    "qis.b-tu.de":   {"pause": "500ms"},
    "www.b-tu.de":   {"pause": "500ms"},
    "opus4.kobv.de": {"pause": "2s", "max_age": "720h", "expect_type": "application/pdf"},
    "huggingface.co": {"concurrency": 2},
    "cdn-lfs.huggingface.co": {"concurrency": 2}
  }
}
```

| Field | Default | Meaning, bounds |
|---|---|---|
| `allow` | `["*"]` | hosts Cortex may fetch from: exact names, `*.suffix` (every name below it, not the suffix itself), `*`; redirects are held to it too |
| `user_agent` | `Cortex/1.0 (+https://betula.app; info@betula.app)` | sent when the client names none; no control characters |
| `concurrency` | `1` | requests in flight to the host at most, 1 to 1000 |
| `pause` | `0s` | after each request (±30 % jitter) before its slot is used again, up to `1h` |
| `max_age` | `24h` | how old a stored answer may be in `cache` without a `max_age` of the request's own |
| `queue_wait` | `60s` | a request not started within this answers `429 host-busy`; more than 0, up to `1h` |
| `breaker_failures`, `breaker_pause` | `10`, `15m` | that many failures in a row (network error, timeout, 5xx) pause the host that long (more than 0, up to `24h`) |
| `timeout` | `10m` | the whole request, from sending it to the end of the body; up to `24h` |
| `max_body` | `8589934592` (8 GiB) | bytes of a body at most, both decoded and as they came over the wire; 1 to 1 PiB |
| `expect_type` | none | the media type a 2xx must have (`502 wrong-type` otherwise, not stored) |

An entry of `hosts` (an exact name or `*.suffix`) overrides the fields of `default` it names; a
host matches its exact entry, then the longest `*.suffix`, then `default`. The entries name the
`host` label of the metrics, every other host is `other`. The file's `hosts` replace the built-in
ones; unknown fields, a file over 1 MiB and data after the object are errors, so a misspelt
setting does not go unnoticed.

- **Only `GET`**, with the client's `Accept` and `Accept-Language`, the `User-Agent`, and
  `Accept-Encoding: gzip`, which Cortex decodes itself (so that `max_body` sees the bytes on the
  wire). No cookie jar, never `Authorization`; a URL with user info is refused. A body in another
  `Content-Encoding` is refused rather than stored as if it were the content.
- **Host names in ASCII only:** an internationalised name must be given in punycode, so that no
  Unicode spelling of a host escapes its entry. A request URL with one is `400`, a redirect to one
  `403 host-not-allowed`.
- **No private addresses, whatever a name resolves to:** the dialer checks every address after the
  lookup and before it connects (so a name rebound to one is refused too) against everything that
  is not globally reachable unicast: loopback, RFC 1918, carrier-grade NAT `100.64/10`,
  link-local (the cloud metadata services among it), unique local `fc00::/7`, multicast,
  documentation, benchmarking, reserved and unspecified ranges, and IPv4 inside IPv6 (mapped,
  NAT64, 6to4) by the IPv4 address it carries. An IP literal is refused before it takes a slot.
  With a proxy from the environment the target is resolved and checked before the request (a
  rebinding between that check and the proxy's own lookup is not caught). `--allow-private` lifts
  this, for development and tests only. Otherwise a client could make Cortex fetch Grafana, the
  Docker socket proxy or another instance's Radix.
- **Redirects:** up to 10, `http` and `https`, to allowed hosts only, each one a request of its own
  host under its floor. The answer at the end of a chain must have the `expect_type` of the host
  asked first as well as its own, and is held to the smaller `max_body` of the two.
- **Conditional requests, first hop only:** where the stored version has upstream's `ETag` or
  `Last-Modified`, Cortex asks with `If-None-Match` / `If-Modified-Since`, but only when the version
  was answered by the URL asked itself, not through a redirect; only a `304` to a request that
  carried them confirms anything (any other `304` is a status Cortex does not store). A refresh of a
  URL that redirects therefore always downloads the whole body.
- **A floor per host, for every client together:** at most `concurrency` in flight, `pause` after
  each; the others wait in a queue in order of arrival, and one not started within `queue_wait`
  gets `429 host-busy` with an estimated `Retry-After`. Clients keep their own pace on top; Cortex
  guarantees that two clients do not add up.
- **A breaker per host:** `breaker_failures` failures in a row pause the host for `breaker_pause`
  (WARN `host.paused`, then INFO `host.resumed`); a `429` or `503` with `Retry-After` pauses it that
  long, at most an hour; a success resets the count. A paused host answers `503 host-paused` at
  once, and `cache` serves what is stored (if `stale` allows).
- **Timeouts:** connect 10 s, TLS 10 s, response headers 60 s, the whole request `timeout`.
- **Host states** (`/status` `hosts`, the `cortex_host_*` gauges): every host the policy names
  exactly, and every other host while it has requests in flight, waiting or in their pause, a
  pause or failures in a row. An idle host's failures are forgotten after an hour; at most 1000
  idle states are kept.

## 6. The hot spare

### 6.1 Two instances

`cortex_a` and `cortex_b`: one stack (`deploy/stacks/cortex.yml`), two services of the same image,
each with a volume of its own, so neither shares a file it could break with the other. One is
the **leader**: it fetches upstream and writes. The other is the **follower**: it copies every
change of the leader and answers what it can from its own copy (§6.6).

### 6.2 Who leads: `flock`

Whoever holds an exclusive `flock` on `--lock` (`/lock/leader.lock`, in the volume `cortex_lock`,
the one thing both instances share) leads. Every instance campaigns by trying the lock every
50 ms. The kernel releases it the moment the leader's process ends (a crash, an OOM kill,
`SIGKILL`, a stopped container), so the follower leads within about 100 ms, with no timeout to
wait out; while the old process lives it keeps the lock, so there are never two leaders. The
holder checks every 500 ms that the lock file is still the file it locked: one removed or replaced
ends its term. A leader that hangs without dying keeps the lock until swarm kills it for its
failing health check (`/livez` every 10 s, 3 tries). This works only on one host (one kernel,
one volume); a network file system whose `flock` is not shared between clients would elect two.

Beside the lock the leader keeps two files (each written to a temporary file, synced, renamed):

- **`leader.json`** {`instance`, `url`, `epoch`, `since`}: the announcement a follower reads to find
  the leader. The leader checks it every second and announces itself again when it is missing or
  names another (WARN `leader.reannounced`).
- **`head.json`** {`instance`, `url`, `epoch`, `seq`, `sum`, `at`, `final`}: the leader's newest
  journal entry, written every second when it changed, and once more with `final: true` when it
  ends its term on purpose.

### 6.3 Taking over

The instance that wins the lock, in this order:

1. catches up from its predecessor (`leader.json`, and `head.json`'s writer if that is another):
   until it has the predecessor's head, or the predecessor stops answering (refused connection,
   `409 not-leader`), at most 30 s;
2. compares itself with `head.json`. Behind it (a lower seq, or another entry at the recorded seq)
   while the record is final, or when it would lose more than 5 minutes, it waits for the writer:
   WARN `leader.deferred`, resign, try again every second, for up to 60 s; then it leads anyway with
   ERROR `replica.stale_takeover` (`behind_entries`, `behind_seconds`, `recorded_by`). A crash record
   with a smaller lag leads at once (`leader.acquired` names `behind_entries`): the owner accepts
   the last minutes of a crash (decision 2);
3. begins its epoch: max(its index's, `leader.json`'s, `head.json`'s) + 1, the first entry of its
   term in the journal;
4. announces itself in `leader.json` and opens the write fence (INFO `leader.acquired`).

Swarm restarts a dead instance; it finds the lock taken and comes back as the follower. Entries of
its old epoch that the new leader never received are a divergence (§6.4): the follower takes a
snapshot and drops them. Nothing fails back: the roles stay swapped until the next crash or
step-down.

### 6.4 Following: the journal

The follower long-polls `GET /internal/v1/journal?after=<seq>&epoch=<e>&sum=<hex>&wait=25s`, naming
its newest entry by seq, epoch and `sum` (a sha256 over a versioned encoding of seq, epoch, op,
payload and blob, the same on both sides). The leader decides against its database: the same
entry at `after` → the next page; another one or none → `409 diverged`; below the journal's start
→ `410 trimmed`. At the boundary (`after` = oldest − 1) it compares against the trim mark, the
{seq, epoch, sum} of the last entry it trimmed (meta `journal_trimmed`), so a check right after a
trim is a real one.

- **Pipelined:** the next page is asked for while the blobs of the last one arrive and the one
  before is applied; pages apply in transactions of 250 entries (one commit each); a blob an entry
  names and the follower lacks is fetched first (`GET /v1/blobs/…` with gzip accepted, its hash
  checked), 4 to 16 at a time (more while every fetch succeeds, half after a failure). After an
  error it waits 250 ms, doubling to 5 s.
- **A snapshot** when the journal cannot continue the local index (`diverged`, `trimmed`, or an
  empty index while the leader has entries): `GET /internal/v1/snapshot` (the leader's
  `VACUUM INTO` copy) replaces the index, and the follower goes on from the snapshot's seq. The
  blobs the new index names and the store lacks are then fetched in the background, 8 at a
  time (`replica.backfilled`); a read that needs one of them meanwhile is forwarded to the leader.
- **Journal trimming:** every retention run removes the prefix up to the newest entry older than
  `--journal-keep` (7 days), never the newest entry. A follower away longer starts from a snapshot.
- **State** (`/status` `follower`): `following` while the lag is under 1 s (and again only under
  0.5 s once it reached 1 s), with the missing blobs counted; `catching_up`, `snapshot`,
  `no_leader`; empty on the leader. `lag_seconds` is the age of the oldest leader entry not
  applied (0 when current); `lag_entries` the leader's newest head minus the local seq (exact, and
  briefly above 0 under load while following); `blobs_missing` the blobs the index references and
  the store lacks, on the leader too. `replica.caught_up` is logged once after a lag of 1 s or more.

### 6.5 A leader with missing blobs

A follower promoted before its back-fill ended leads with blobs missing. It fetches them from the
other instance (passes with a 1 s pause, doubling to 1 min); a read that needs one meanwhile is
proxied there (`Cortex-Forwarded`), and answered `503 blob-missing` only when the other instance
cannot serve it either. A fetch in `cache` or `refresh` treats such a version as not stored and
fetches it upstream again. `/healthz` reports the missing blobs. If the old leader never comes
back, those blobs are gone (§12).

### 6.6 Answering on the follower

What the follower can answer from its copy it answers itself: an `offline` hit, a fresh `cache`
hit, an `at` hit, entries, blobs and files it has. Everything else (a miss, a stale version,
`refresh`, every write, `prune`, an entry or file whose blob it lacks, and an `offline` miss: the
leader may have fetched it a second ago) goes to the leader through a reverse proxy, with the
header `Cortex-Forwarded: <instance>`; a request that carries it is never forwarded again (`503
no-leader`). When no leader is known or it cannot be reached, the follower waits up to 2 s for a
leader (itself, or one that announces itself); when none comes, a fetch with `stale=if-error` is
answered with the stored version if there is one (`Cortex-Upstream-Error: no-leader`), else `503
no-leader` with `Retry-After: 1`. A write that may have reached the leader before it failed is
never answered `no-leader` (which a client may repeat elsewhere), but `502 forward-failed`.

### 6.7 Hand-over without loss: step-down and SIGTERM

`cortex step-down` (`POST /v1/admin/step-down`) and `SIGTERM` (a container stop, an update) end a
term on purpose:

1. the write fence closes; a write that arrives now is forwarded or answered `503 no-leader`, and
   the client sends it to the other instance;
2. every write in flight ends, however long it runs (WARN `leader.fence_wait` every 5 s): a late
   write must keep its own term's epoch;
3. `head.json` with `final: true`, then the lock is released;
4. the instance keeps serving its journal and blobs (role leader, writes refused) until the
   successor announces itself, at most 20 s, or until nobody has held the lock for 1 s (no
   successor is coming). The successor announces itself only once its catch-up has the
   predecessor's head, so the announcement confirms that it has every acknowledged write;
5. it follows (`leader.stepped_down` with `head_seq`, `successor_wait` `announced`, `none` or
   `timeout`, `duration_ms`); after a step-down it stays out of the election for 15 s.

On `SIGTERM`, within swarm's 30 s `stop_grace_period`: the hand-over gets up to 22 s while HTTP
serves; then an instance of a pair answers with `Connection: close` and listens 1 s more, so that
clients move to the other instance on fresh connections; the requests in flight get until 24 s
after the signal (20 s for a single instance), and what runs detached at most 5 s more. A
hand-over that does not end within 22 s (a write that does not end) is given up: ERROR
`service.fatal`, exit 1, as a crash would.

### 6.8 What a crash can lose, measured

A `kill -9`, an OOM kill or a dead host process loses what the follower had not received: the
writes of the last moment. Graceful stops lose nothing (§6.7). Measured 2026-10-03 with real
processes of the release on one machine (loopback), a fake upstream of 5,000 pages and a load of
concurrent fetches and file writes, two runs:

| Measure | Result |
|---|---|
| leader `kill -9` every 10 to 20 s, restarted after 2 s | 29 kills (13 and 16) |
| take-over, kill to the first acknowledged write on the survivor | 34 to 141 ms, median about 75 ms |
| acknowledged writes lost that were older than 5 s at the kill | 0 of 90,510 |
| acknowledged writes lost within 5 s of a kill (allowed) | 600 of 32,331; the oldest 707 ms before its kill |
| follower lag at the kills | 0 to 0.72 s, 0 blobs missing |
| `SIGTERM` of the leader, follower current | 0 of 32,938 lost, take-over 129 to 180 ms, exit after 1.2 to 5.5 s |
| `SIGTERM`, follower restarted 12 s earlier (2,500 entries behind) | 0 of 31,033 lost, take-over 1.4 to 1.8 s, exit after 6 to 8 s |
| `cortex step-down`, 3 rounds | 0 of 12,260 lost, 0 client errors, CLI about 210 ms |
| empty follower next to 105 MB of index and 90,402 blobs | snapshot in 1.9 s, following at 4.7 s; back-fill 443 blobs/s (about 17 MB/s) with 2 workers. The back-fill now runs 8 workers; its re-measurement is pending |
| `/status` under load | p50 2.8 ms, p99 24 ms |

A restarted crashed leader always takes a full snapshot (its unreplicated tail is a true
divergence), which grows with the index (2.6 s for 68 MB, 4.8 s for 133 MB in these runs).

### 6.9 Kubernetes, later

The election is an interface (`cluster.Elector`: `Campaign`, `Leader`, `Announce`, a `Term` with
`Resign` and `Lost`). A Kubernetes elector would hold a `coordination.k8s.io` Lease: `Campaign`
takes it once it expired, with the advertise URL as `holderIdentity`, and renews it; `Leader` reads
`holderIdentity` and the epoch from an annotation `Announce` writes; `Lost` fires when a renewal
fails before the Lease expires. The journal, the follower and the API work over the network
unchanged. It is not built (decision 1).

## 7. Clients

**The Go client**, `internal/cortex/client` (standard library only, so that Radix imports nothing
of the server):

- `client.New("http://cortex_a:8100,http://cortex_b:8100", client.Options{})`: base URLs are a scheme
  and a host (and port), with nothing or `/` after it; a path, credentials, a query or a fragment is
  refused (below a path every request would get Cortex's `404 not-found`, which a careless caller
  could take for the host's 404).
- `Fetch(ctx, url, FetchOptions{Mode, MaxAge, Stale, Expect, Source, At, Accept, AcceptLanguage,
  UserAgent})`; `Transport(o)`, a `RoundTripper` that sends `GET` and `HEAD` to `/v1/fetch` instead of
  the host (other methods are refused); `HTTPClient(o, timeout)`; `WithSource(ctx, source)` names the
  source per request; `CheckedAt(header)`; `ErrorCode(resp)` and `ResponseError(resp)` tell Cortex's
  own error from the host's status (a 404 with `Cortex-Error` says nothing about the page).
- `PutFile`, `GetFile` (the content is checked against its `ETag` while it is read), `DeleteFile`,
  `Status`, `StepDown`.

**Fail-over.** A request goes to the instance that answered last, and on to the next on a dial
error, a connection closed or reset before the answer, or `503 no-leader`: round after round, 250 ms
apart, for up to `FailoverWait` (5 s). What may go to the next instance:

- a request no instance could be connected to (nothing was sent), whatever its method, as long as
  its body can be read again (a file, `GetBody`; not stdin);
- after `503 no-leader`: `GET`, `HEAD`, `PUT` and `DELETE`: nobody carried it out;
- after a connection that broke once the request was sent: `GET` and `HEAD` only. A `PUT` or
  `DELETE` may have been carried out, and another instance's answer would describe the state it
  left (a `412` for an `If-Match` that won, a `404` for a delete that was done): the caller gets an
  error with `ErrOutcomeUnknown`, and `GetFile` tells what is stored. Writes carry no
  `Idempotency-Key`, so Go's transport does not repeat them either.

**Without Go:** any HTTP client, `curl` included (§4.4); `cortex put NAME [FILE]` and `cortex get
NAME` from the command line (§9).

## 8. Radix through Cortex

With `--cortex` / `RADIX_CORTEX_URL` (both instances, comma-separated, no path; an invalid value
exits with code 2), every request of the crawl (`run`, `crawl-modules`, `crawl-qis-modules`,
`crawl-events`, `crawl-tree`) and of `download-statutes` goes through `client.Transport`; without
it Radix fetches directly, as before. Gemini is asked directly either way.

- **`cache` with `max_age` 1 h** (`--cortex-max-age` / `RADIX_CORTEX_MAX_AGE`; negative: the host's
  `max_age`): a page that is due is fetched, unless another client fetched it within the hour. Two
  crawling colours ask the university once. `stale=never`: a failure stays a failure, so Radix's
  retries, `crawl.aborted` and the degraded cycle stay as they were. A client timeout of 3 minutes
  per request covers Cortex's queue per host.
- **`fetched_at` is `Cortex-Checked-At`,** when Cortex last had the content from the university, not
  Radix's clock: a page served from the store is not taken for a fresh one, and its day in its
  period (`crawl.Due`) stays right. The archive's `fetched_at` never goes backwards: an answer older
  than the archived page (from the store, after a direct fetch since) is not stored.
- **Cortex's own errors are failures:** an answer with `Cortex-Error` (a `502 upstream-failed`, a
  `429 host-busy`, a `404 not-found` of a misconfigured URL) is a failed attempt, never a page and
  never archived as a 404: retried, given up and counted towards the abort like any other.
- **Statutes:** source `statute`, `download-statutes --force` uses `refresh` (OPUS is asked again,
  not Cortex's store). A `502 wrong-type` (OPUS's entry takes only PDFs) is the bot-protection page
  it is (`statutes.blocked`); another Cortex error reads `unexpected status N (Cortex: <code>)`.
- Each request names its source (`qis_tree`, `module_page`, …), so Cortex's metrics show the same
  sources as Radix's. `crawl.cortex` (INFO) at start names the instances, the mode and `max_age`.

Offline for Radix through Cortex (`mode=offline`) is a later step; `RADIX_CRAWL=off` still means
`serve-snapshot`, which sends nothing at all. The deploy side (who gets `RADIX_CORTEX_URL`, who gets
a way out) is `deploy/README.md` section 14; the flags in Radix's table are in
`docs/operations.md` §1.

## 9. Configuration

Every flag of `cortex serve` has an environment variable; the help of each names it. A git-ignored
`.env` file is read from `CORTEX_ENV_FILE` (default `.env`), and the real environment wins over it.

| Flag | Environment | Default |
|---|---|---|
| `--addr` | `CORTEX_ADDR` | `127.0.0.1:8100` (the image: `0.0.0.0:8100`) |
| `--data` | `CORTEX_DATA` | `cortex-data` (the image: `/data`) |
| `--instance` | `CORTEX_INSTANCE` | the host name: unique in the pair |
| `--advertise-url` | `CORTEX_ADVERTISE_URL` | `http://<addr>`: where the other instance reaches this one; with `--lock` it must not be a wildcard address (exit 2) |
| `--lock` | `CORTEX_LOCK` | none: one instance, always the leader |
| `--hosts` | `CORTEX_HOSTS_FILE` | none: the built-in policy (§5) |
| `--history` | `CORTEX_HISTORY` | `4320h` (180 days) |
| `--journal-keep`, `--blob-grace` | `CORTEX_JOURNAL_KEEP`, `CORTEX_BLOB_GRACE` | `168h`, `168h` |
| `--prune-interval` | `CORTEX_PRUNE_INTERVAL` | `1h` (the four durations must be positive: exit 2) |
| `--allow-private` | `CORTEX_ALLOW_PRIVATE` | `false`: development and tests only |
| `--log-format`, `--log-level`, `--log-file` | `CORTEX_LOG_FORMAT`, `CORTEX_LOG_LEVEL`, `CORTEX_LOG_FILE` | `json` for `serve`, `info`, none |

| Command | What it does |
|---|---|
| `cortex [serve]` | the service (the default command) |
| `cortex healthcheck [--url]` | `GET` of `CORTEX_HEALTH_URL` (`http://127.0.0.1:8100/livez`) within 5 s: exit 0 or 1; the image's `HEALTHCHECK` |
| `cortex status [--url]` | prints `GET /status` as one JSON object (the deploy scripts read it with `jq`) |
| `cortex step-down [--url]` | asks the instance to hand over; exit 1 when it does not lead or cannot |
| `cortex put NAME [FILE] [--type]` | stores a file (stdin without `FILE`; `--type` / `CORTEX_CONTENT_TYPE`, else from the name's extension); prints the current version as JSON with `created` |
| `cortex get NAME` | the current content to stdout; exit 1 when there is none or it does not match its hash |
| `cortex version`, `cortex help` | |

`--url` / `CORTEX_URL` of the client commands: `http://127.0.0.1:8100`, a comma-separated list
allowed. Exit codes as Radix: `0`, `1` failure, `2` invalid flags or configuration, `130`
interrupted.

**The image** (`nix build .#cortex-image`, `betula-cortex:latest`): the static binary and CA
certificates; user `10002:10002`; `/data` and `/lock` owned by it; entrypoint `/bin/cortex`,
command `serve`; `CORTEX_ADDR=0.0.0.0:8100`, `CORTEX_DATA=/data`, `CORTEX_LOG_FORMAT=json`,
`TZ=Europe/Berlin`; port 8100; volume `/data`; health check `cortex healthcheck` every 10 s, 5 s
timeout, 30 s start period, 3 retries. `nix build .#cortex` builds the binary alone; a change to
Cortex alone does not change Radix's image (Radix's source keeps only `internal/cortex/client`).

## 10. Metrics

`GET /metrics` is what the Grafana dashboard „Cortex" shows
(`deploy/config/monitoring/grafana/dashboards/betula-cortex.json`; Prometheus job `cortex`, static
targets `cortex_a:8100` and `cortex_b:8100`, `instance` relabelled to `cortex_a` / `cortex_b`).
Cortex serves a registry of its own: no `radix_*` family appears there. No metric has a label
`instance`; the scrape sets it.

| Metric | What it says |
|---|---|
| `cortex_requests_total{source,mode,result}` | fetches: `hit`, `miss`, `stale`, `refresh` (fetched upstream for these three), `stale_if_error`, `offline_miss`, `error`, `forwarded` (to the leader), `not_modified`. The first 64 sources a process sees (`unknown` among them) keep their label, later ones count as `other`; the full source is in the log |
| `cortex_coalesced_total` | fetches that joined an upstream request already running |
| `cortex_upstream_requests_total{host,code}`, `cortex_upstream_request_duration_seconds{host}`, `cortex_upstream_bytes_total{host}` | what reached the hosts, redirects included (`code` the status, `error` without one), how long to the end of the body (histogram), bytes of content; `host` an entry of the policy or `other` |
| `cortex_host_queue{host}`, `cortex_host_in_flight{host}`, `cortex_host_paused{host}` | waiting requests, requests in flight, the breaker (1 or 0; for `*.suffix` and `other` the number of paused hosts) |
| `cortex_role`, `cortex_epoch`, `cortex_journal_seq`, `cortex_leader_changes_total` | 1 for the leader, 0 for the follower; the epoch and seq of the newest journal entry; role changes |
| `cortex_replication_lag_seconds`, `cortex_replication_lag_entries` | the follower's distance (0 on the leader) |
| `cortex_entries`, `cortex_versions`, `cortex_files`, `cortex_blobs`, `cortex_blob_bytes`, `cortex_blob_original_bytes` | the store: from a background count at most about a minute old (no sample until the first one); blob bytes as stored and before compression |
| `cortex_pruned_total{what}` | retention: `versions`, `file_versions`, `files`, `entries`, `blobs`, `journal_entries` |
| `cortex_http_requests_total{route,code}` | every request, by route (`fetch`, `entries`, `files`, `blobs`, `livez`, `healthz`, `status`, `metrics`, `step_down`, `prune`, `journal`, `snapshot`, `other`) |
| `cortex_log_problems_total{level,event}` | log records at WARN and ERROR by event (§11) |
| `cortex_build_info{build}`, `cortex_start_time_seconds` | the binary (the start of its sha256), when the process started |

Alerts (`deploy/README.md` section 11): no leader or two (critical), the follower more than 300 s
behind (warning), a host paused for 5 minutes (warning); all quiet for a Cortex that has not
answered a scrape within 7 days.

## 11. Logging

One JSON line per event on stderr (and in `--log-file`), as Radix's (`docs/operations.md` §2);
components `cortex`, `http`, `upstream`, `store`, `cluster`, `cli`. **Levels are a contract.**
`ERROR`: something needs a human, or data may have been lost. `WARN`: something failed and
recovered by itself, or will. `INFO`: the story of the service. `DEBUG`: health checks, scrapes,
status reads, the journal's long polls, `304` answers.

| Level | `event` | Meaning |
|---|---|---|
| ERROR | `service.fatal` | the process stops: the data directory, the pair or the listener failed, or a hand-over did not end within 22 s (exit 1) |
| ERROR | `http.failed`, `cli.failed` | the HTTP endpoint died / an invalid flag (exit 2) |
| ERROR | `replica.stale_takeover` | an instance led although behind the head the last leader recorded, after waiting 60 s: what it lacked is lost (`behind_entries`, `behind_seconds`, `recorded_by`, `recorded_seq`) |
| ERROR | `upstream.failed` with `code=internal` | Cortex could not store or record a download (a full disk) |
| ERROR | `leader.campaign_failed` after it won the lock | it could not begin its epoch or announce itself, and resigned |
| ERROR | `http.request` with `error=internal` or a 5xx of its own | |
| WARN | `upstream.failed` | a fetch failed upstream (`url`, `host`, `source`, `code`, `duration_ms`); INFO for Cortex's own refusals (`host-busy`, `host-paused`, `host-not-allowed`) and a fetch whose client went away |
| WARN | `host.paused` | the breaker or upstream's `Retry-After` paused a host (`until`) |
| WARN | `policy.invalid` | the host policy file cannot be read or is invalid; the last good one stays |
| WARN | `blob.rejected` | an upload whose content has another hash than it should (`422`) |
| WARN | `leader.lost`, `leader.campaign_failed` | the term ended without a resign (the lock file went away) / the election cannot be held (the lock file cannot be opened), logged once a minute |
| WARN | `leader.deferred`, `leader.reannounced`, `leader.fence_wait`, `leader.record_failed` | waits for the instance that recorded a newer head (§6.3) / `leader.json` was missing or named another / a hand-over waits for writes in flight (every 5 s) / `head.json` could not be written |
| WARN | `replica.diverged`, `replica.failed` | the follower's index is not the leader's: a snapshot follows / a replication round failed, or the leader lacks blobs the index references (throttled to once a minute) |
| WARN | `snapshot.failed`, `retention.failed`, `metrics.stats_failed`, `index.close_failed`, `index.sync_failed` | a snapshot was not sent (INFO when the follower went away) / a step of retention failed / the store could not be counted / closing the old index or syncing the directory after a snapshot failed |
| WARN | `http.request` with any other `error` | a Cortex error the client caused or that recovers; `not-found`, `offline-miss` and `precondition-failed` are INFO |
| INFO | `service.started`, `service.stopped`, `http.listening`, `db.migrated` | lifecycle (`service.stopped` with `duration_ms`) |
| INFO | `leader.acquired`, `leader.stepped_down` | a term began (`epoch`, `previous`, `caught_up_entries`, `behind_entries` when behind the record) / ended on purpose (`reason`, `head_seq`, `successor_wait`, `duration_ms`) |
| INFO | `replica.following`, `replica.caught_up`, `replica.snapshot`, `index.replaced`, `replica.backfilled`, `snapshot.sent` | following a leader / current again after a lag of 1 s or more / the index replaced by the leader's / the blobs it lacked fetched (`fetched`, `failed`, `remaining`) / a snapshot sent to the follower |
| INFO | `upstream.fetched` | a fetch upstream (`url`, `host`, `source`, `status`, `bytes`, `changed`, `version`, `duration_ms`) |
| INFO | `host.resumed`, `policy.loaded` | a pause is over / the host policy read (at start and after a change) |
| INFO | `prune.finished`, `journal.trimmed`, `gc.finished` | retention: versions and files removed, journal entries trimmed, unreferenced blobs removed |
| INFO | `http.request` | every request (`route`, `status`, `url`, `source`, `mode`, `result`, `cache`, `forwarded_to`); DEBUG for health, metrics, status, the journal and `304` |

## 12. Operation

How it is deployed, updated and watched: `deploy/README.md` section 14. What it does not cover:

- **The host.** Both instances run on it (decision 1): a host that dies takes both. With a second
  host the election needs a lease with a third party (§6.9); the journal works over the network
  as it is.
- **Backups.** None exist yet (`deploy/README.md` section 8). The follower is no backup: a deletion,
  a retention run or a bug that writes wrong data replicates to it. Worth backing up: the volumes
  `cortex_a-data` and `cortex_b-data` (one of them is enough; the leader's is the newer).
- **A double failure.** A follower promoted while it still fetched blobs lacks them until the other
  instance is back (`503 blob-missing`, §6.5); if it never comes back, they are gone. A crash
  during that window loses more than the usual moment (6 s in one measured case).
- **An announcement that cannot be written** (a full disk): each attempt begins an epoch and then
  resigns, so epochs climb, and the stray epoch entries force a snapshot afterwards. Nothing is
  lost, but `cortex_epoch` rising without `leader.acquired` means: look at the disk.
- **Gemini goes direct** (decision 6): a Radix that asks Gemini needs a way out of its own, and
  Docker has no allowlist per host for it, so that way out reaches every host.
- **Egress.** Cortex itself reaches every public host its policy allows (`"allow": ["*"]`): it is
  passive, and what no client asks for is never fetched. Narrowing `allow` narrows what any
  client can make it fetch.
- **Model weights.** The model store stays the host directory filled by `ship-models.sh`
  (`deploy/README.md` section 13); the Hugging Face entries of the policy are ready for a download
  by URL and `expect`.
