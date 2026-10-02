# Concept: Cortex, the cache between Betula and the internet

> **Status: concept (2026-10-02), nothing is built.** Asked for by the owner: every scraping
> request goes through one internal service that keeps the raw pages and files and serves them
> again, with support for model weights, a hot spare that follows the data and takes over at
> once when the active instance crashes, a REST endpoint for the internal services, and a choice
> per request whether it may go online. Open points are marked **(Q1)** … and collected in §11,
> each with a recommendation. The name is a proposal: Betula is the birch, Radix its root, Folia
> its leaves, and the cortex is its bark, which everything that enters the tree passes through.

## 1. What it is for

Every request a Betula service sends to a server Betula does not run goes through Cortex: QIS
(`qis.b-tu.de`), the module pages on `www.b-tu.de`, the statute PDFs on `opus4.kobv.de`, and the
base models on Hugging Face. Cortex keeps the raw answer (body, status, the headers that matter),
so that every internal service that asks again gets it from Cortex instead of from the source.

| Today | With Cortex |
|---|---|
| At most one colour of a site may crawl (`RADIX_CRAWL=on`); its `radix.db` travels with the crawl in a five-step handover (`deploy/README.md` §4) | Two Radix that ask for the same page within the same hour cause one request upstream: the second gets the answer from Cortex |
| Canary is offline and gets a copy of the public site's `radix.db` at every deploy (§12 of the runbook) | Canary's Radix can run its whole cycle against Cortex in `offline` mode: it sees every page the public site fetched, minutes later, and cannot send a request to the university, even misconfigured, because Cortex refuses it (§6) |
| Politeness lives in each Radix process (pause, caps per cycle, off-peak window); two processes add up | A floor per host in Cortex holds for all clients together (§7): one request at a time, a minimum pause, a breaker |
| `raw_page` keeps the latest body of each page | Cortex keeps every version that differed, with its time: a parser can be tried on a page as it was on a given day (`at=`) **(Q5)** |
| The model store is a directory of the host (`/var/lib/betula/models`), filled by `ship-models.sh` | The same content-addressed store, with a replica and an HTTP API; base models from Hugging Face fetched once, checked against a pinned sha256 (§4.3) **(Q7)** |
| Radix's containers need a way to the internet | Only Cortex needs one; Radix's network can be `internal: true`, so "everything goes through Cortex" is enforced by the network, not by convention **(Q6, Q8)** |

Not through Cortex: Folia's snapshot downloads from Radix (internal), the browsers (Traefik and
Folia's page cache), the canary agent's downloads from GitHub (checked against GitHub's own
sha256 already). Gemini is an open question **(Q6)**: its requests are POSTs with a key, and their
results (the summaries) are kept by Radix already.

## 2. Overview

```
one host (betula.app, one-node swarm)

Radix (public colour)    ──┐  overlay "cortex"       ┌─ cortex_a: LEADER ─────────┐
Radix (canary, offline)  ──┤  (internal: true);      │  index (SQLite) + blobs    │──▶ qis.b-tu.de
radix scan-curriculum    ──┼─ either instance ──────▶│        │ journal, blobs   │    www.b-tu.de
ship-models.sh (via ssh) ──┘  answers, a write       │        ▼                   │    opus4.kobv.de
                              goes to the leader     │  cortex_b: FOLLOWER        │    huggingface.co
                                                     │  own copy, < 1 s behind    │    (allowlist only)
                                                     └────────────────────────────┘
                     /var/lib/betula/cortex/leader.lock: whoever holds the flock leads
```

Cortex is passive: it fetches only when a client asks. What is due and when stays Radix's
decision (`crawl.Due`, the off-peak window, the caps per cycle); how fast the university is asked
at most becomes Cortex's **(Q4)**.

## 3. Storage

Each instance has a volume of its own with two parts.

**Blobs.** Every body and every uploaded file is a file named by the sha256 of its content,
`blobs/sha256/<first two hex>/<hash>`: written once (temporary name, fsync, rename), never
changed, the same content stored once however many URLs or versions deliver it. The hash is
always that of the original bytes; text (HTML, JSON) is stored gzip-compressed as in Radix's
archive and handed to a client that accepts gzip as it is, files such as models and PDFs are
stored as they are, so a `Range` request reads them directly.

**Index**, SQLite in WAL mode (`modernc.org/sqlite`, as Radix: no CGO):

| Table | One row per | What |
|---|---|---|
| `entry` | request identity: the normalized URL | `url`, `host`, `source` (the client's name for it, §4.1), the current version |
| `version` | answer that differed from the one before | `fetched_at` (first fetch of this content), `checked_at` (last fetch that returned it), `status`, the kept response headers (`Content-Type`, `ETag`, `Last-Modified`), `blob`, `bytes` |
| `pin` | name → blob | files that are never collected: the models of `models.lock`, uploads |
| `journal` | change of the index | `seq`, `epoch`, the change; the follower's feed (§5) |

A fetch that returns the content of the current version only moves its `checked_at` (as
`catalogdb.PutPageChanged` does), so a page fetched daily that changes twice a year costs two
blobs, not 365. Cortex keeps 200 and 404/410 (a page that is gone is an answer, as in `raw_page`);
other statuses are passed on and never stored.

**The URL as key.** Normalized conservatively: scheme and host in lower case, the default port and
the fragment dropped, path and query exactly as sent. QIS's parameter order is part of its URLs;
sorting would merge nothing and could break something. Clients' request headers are not
forwarded: Cortex sends a fixed set per host (§7), so the answer depends on the URL alone, and no
client can put a cookie or a credential into a request whose answer others get.

**Retention.** Current versions stay. Older versions go after a period **(Q5)**; a blob that no
version and no pin names any more is removed after a grace period (7 days, as `--archive-grace`).

## 4. The API

`http://cortex_a:8100` and `http://cortex_b:8100` on the overlay `cortex`, no published port.
Either instance answers every request (§5); JSON for metadata, the raw body for content.

### 4.1 Fetching: `GET /v1/fetch`

| Parameter | Meaning | Default |
|---|---|---|
| `url` | absolute `http`/`https` URL whose host is on the allowlist (§7) | required |
| `mode` | `offline`, `cache` or `refresh` (below) | `cache` |
| `max_age` | for `cache`: a stored answer younger than this is served without asking upstream | per host (§7), e.g. `24h` |
| `stale` | `if-error`: when upstream fails, serve the stored answer, marked stale; `never`: fail instead | `if-error` |
| `expect` | `sha256:<hex>`: the content must have this hash. A different download is refused and not stored; a hash any URL ever delivered is served from the blobs without a request (a model that moved is not downloaded again) | none |
| `at` | RFC 3339: the version that was current at that time; implies `offline` | none |
| `source` | a name for metrics, logs and policies: `qis_tree`, `module_page`, `statute`, `model`, … | `unknown` |

| `mode` | stored, younger than `max_age` | stored, older | not stored |
|---|---|---|---|
| `offline` | served | served (`Age` says how old) | `504`, as `only-if-cached` in RFC 9111 |
| `cache` | served | fetched, stored, served | fetched, stored, served |
| `refresh` | fetched, stored, served | fetched, stored, served | fetched, stored, served |

`refresh` is `cache` with `max_age=0`. In both, a request for a URL that is being fetched at
that moment joins that fetch instead of starting another (coalescing): ten clients that ask for
the same page at once cause one request upstream.

**The answer.** The status is upstream's (`200`, `404`, …) whether it came from the store or the
network; Cortex's own are `504` (`offline`, not stored), `502` (upstream failed and nothing may be
served instead), `403` (host not on the allowlist, or the mode not allowed for this client, §6)
and `429` with `Retry-After` (the host's queue is full, §7). The body is the raw body. Headers:

| Header | Value |
|---|---|
| `Content-Type`, `Last-Modified` | upstream's |
| `ETag` | `"sha256:<hex>"` of the content: the same content has the same tag, whenever it was fetched. `If-None-Match` with it → `304` |
| `Cache-Status` | RFC 9211: `Cortex; hit`, `Cortex; fwd=miss; stored`, `Cortex; fwd=stale; fwd-status=200`, `Cortex; hit; detail=stale-if-error` |
| `Age`, `Cortex-Fetched-At`, `Cortex-Checked-At` | how old the answer is; when this content was first and last fetched |

Radix archives `Cortex-Checked-At` as the page's `fetched_at`, not its own clock: a page served
from the store is not taken for a fresh one, and its day in its period (`crawl.Due`) stays right.

### 4.2 What the store holds

- `GET /v1/entries?url=…`: the versions of one URL (`fetched_at`, `checked_at`, status, hash,
  bytes).
- `GET /v1/entries?host=…&source=…&changed_since=…`: NDJSON, paged by a cursor. What a fresh
  colour or a developer's machine needs to fill its own archive without a request upstream
  (phase 5).
- `DELETE /v1/entries?url=…`: forget one URL (a page stored by mistake).

### 4.3 Files and model weights

- `GET /v1/blobs/sha256:<hex>`, `HEAD`: the file, with `Range`, `ETag` = the hash and
  `Cache-Control: immutable`.
- `PUT /v1/blobs/sha256:<hex>`: upload (what `ship-models.sh` does today). Cortex hashes while it
  writes and refuses a body whose hash differs (`422`). A blob it has already answers `200`
  without reading the body (`Expect: 100-continue`). `201` only once both instances have the
  file (§5): an upload may be the only copy outside the workstation.
- `GET /v1/pins`, `PUT /v1/pins/<name>` (a list of hashes): what is never collected.
  `models.lock` is the pin `models`; a new lock replaces it, and the old models stay until no
  pin names them and the grace period has passed.
- From Hugging Face: `GET /v1/fetch?url=https://huggingface.co/<repo>/resolve/<commit>/<file>&expect=sha256:<hex>&source=model`.
  A file is downloaded once, checked, and served from then on; streamed to disk, so the size of
  a base model (about 470 MB for `multilingual-e5-small`) is no problem. A mirror that
  `huggingface_hub` can use as `HF_ENDPOINT` would be a project of its own **(Q7)**.

How Radix and Folia get their models **(Q7)**: (a) they keep the bind mount of the host store,
and Cortex is a second, replicated copy of it with an API; or (b) Cortex replaces the store:
`50-app.sh` names the hashes, the service downloads its model at start into its volume (35 MB
over the overlay: well under a second), checks the hash as today, and runs without the semantic
search when Cortex cannot give it, as it does today without the store.

### 4.4 Operation

`GET /healthz`, `GET /status` (role, epoch, journal position, the follower's lag, every host with
its queue and breaker), `GET /metrics` (§9), `POST /v1/admin/step-down` (the leader hands over,
for an update without a gap).

## 5. The hot spare

**Two instances.** `cortex_a` and `cortex_b`: one stack, two services of the same image and
configuration, each with its own volume, so neither shares a file it could break with the other.
One of them is the **leader**: it fetches upstream and writes. The other is the **follower**: it
copies every change of the leader as it happens and answers from its own copy.

**Who leads.** Whoever holds an exclusive `flock` on `/var/lib/betula/cortex/leader.lock`, a host
directory both bind-mount (the lock file only, no data). The follower waits in `flock`. The kernel
releases the lock the moment the leader's process ends (crash, OOM kill, `SIGKILL`, a stopped
container), so the follower leads within milliseconds, without a timeout to wait out; and while
the old process lives it keeps the lock, so there are never two leaders. A leader that hangs
without dying keeps the lock until swarm kills it for its failing health check (10 s × 3 tries:
half a minute). This works because both run on one host **(Q1)**.

**Following.** The follower keeps one request open to the leader,
`GET /internal/v1/journal?after=<seq>`, a stream of NDJSON lines, one per change of the index, in
order. It applies each in a transaction of its own with its position. A change that names a blob
it lacks waits until it has fetched the blob from the leader (`GET /v1/blobs/…`, hash checked).
The lag is one local HTTP transfer: well under a second for pages. A fresh follower, or one that
was away longer than the journal reaches back (7 days), starts from a copy of the leader's index
(`VACUUM INTO`) and then fetches the blobs it lacks.

**Answering on the follower.** What the follower can answer from its copy (a hit, `offline`, a
blob) it answers itself. What needs upstream or a write (a miss, `refresh`, an upload, a pin) it
forwards to the leader and returns the leader's answer. Before it answers `offline` with `504`, it
asks the leader, which may have fetched the page in the last second. So a client may talk to
either instance, and both stay warm.

**What can be lost.** Pages replicate after the fact: a page the leader fetched in the last moment
before it died may be missing on the follower, and is fetched again when it is next asked for.
Uploads and pins replicate before they are confirmed (`201`, §4.3). If the follower is away, an
upload is confirmed with `Cortex-Replicas: 1` and a WARN **(Q2)**.

**Taking over.** The follower gets the lock, has applied what it received, starts epoch + 1 (the
first entry of its journal), and from then on fetches and writes. A client whose request died with
the old leader gets a connection error and repeats it at the new one (§6). Swarm restarts the dead
instance; it finds the lock taken and comes back as the follower: it asks for the journal from its
last position, and entries of the old epoch the new leader never received are dropped from its
index (their blobs are content-addressed, never wrong, and collected later). Nothing fails back:
the roles stay swapped until the next crash or `step-down`.

**Updates.** The follower first, then `step-down`, then the other: no moment without a leader.
Each service `stop-first`, so never two processes on one volume.

**Not covered.** The host itself: both instances are on it **(Q1)**. With a second host, `flock`
becomes a lease that needs a third party (a small witness, or Consul as LiteFS does it); the
journal works over the network unchanged. And the follower is no backup: a deletion, or a bug that
writes wrong data, replicates (`deploy/README.md` §8: no backups exist yet).

## 6. Clients

**The Go client**, `internal/cortex/client`: an `http.RoundTripper` that turns
`GET https://qis.b-tu.de/…` into `GET /v1/fetch?url=…&mode=…&source=…`, tries the instances in
the order of `CORTEX_URL=http://cortex_a:8100,http://cortex_b:8100`, keeps the one that answered,
and switches on a connection error or a `503`. Radix gives it to `crawl.Options.Client` and to the
statutes download; the crawl code stays as it is. Without `RADIX_CORTEX_URL`, Radix fetches
directly as today.

| Radix | Mode |
|---|---|
| `RADIX_CRAWL=on` | `cache` with `max_age=1h`: a page that is due is fetched, unless another client fetched it in the last hour. Two crawling colours ask the university once. `stale=never`: a failure stays a failure, and Radix's retries, `crawl.aborted` and degraded cycle stay as they are |
| `RADIX_CRAWL=cache` (new) | the whole cycle with `offline`: canary, a standby colour. A page Cortex lacks (`504`) is skipped: not retried, not counted towards `crawl.aborted`, asked for again in the next cycle |
| `RADIX_CRAWL=off` | as today: `serve-snapshot`, no requests at all |

**Enforced, not asked for.** Each client sends a token (`Authorization: Bearer`, a swarm secret
per client: `cortex-token-<instance>`); Cortex maps it to a policy: the modes it may use, its
`source` names, its share of a host's queue. Canary's token allows `offline` only, so canary
cannot reach the university even with a wrong `RADIX_CRAWL`. Without a token: `offline` only
**(Q3)**.

**The network.** Cortex is on `cortex` (internal) and on a network with a way out. Radix needs a
way out today only for the university and for Gemini; with Cortex, and Gemini through Cortex or
dropped **(Q6)**, Radix's networks become internal, and nothing in it reaches the internet but
through Cortex **(Q8)**.

## 7. Upstream: politeness and safety

**An allowlist, a file in `deploy/config/cortex/`:**

```yaml
hosts:
  qis.b-tu.de:     { pause: 500ms, concurrency: 1, max_age: 24h }
  www.b-tu.de:     { pause: 500ms, concurrency: 1, max_age: 24h }
  opus4.kobv.de:   { pause: 2s,    concurrency: 1, max_age: 720h, expect_type: application/pdf }
  huggingface.co:  { pause: 0,     concurrency: 2, sources: [model] }
  cdn-lfs.huggingface.co: { pause: 0, concurrency: 2, sources: [model] }
user_agent: "Betula-Radix/1.0 (+https://betula.app; info@betula.app)"
```

- **Only these hosts**, only `GET` and `HEAD`, redirects only to hosts on the list, and never to an
  address in a private, loopback or link-local range, whatever a name resolves to. Otherwise a
  client could make Cortex fetch Grafana, the socket proxy or another instance's Radix.
- **A floor per host, for everybody together:** at most `concurrency` requests at a time, `pause`
  (±30 % jitter, as Radix) after each. Requests wait in a queue per host; a queue longer than a
  minute's worth answers `429` with `Retry-After`. Radix keeps its own pace on top; Cortex
  guarantees that two clients do not add up.
- **A breaker per host:** 10 failures in a row pause the host for 15 minutes (`crawl.aborted`'s
  rule, now for all clients); meanwhile `cache` serves what is stored (if `stale` allows) and
  `refresh` fails at once. `429` and `503` from upstream with `Retry-After` pause the host for
  that long.
- **Only what it claims to be is stored:** a host with `expect_type` keeps only answers of that
  type. A bot-protection page instead of a statute PDF (what `internal/statutes` detects today) is
  passed on as `502` with the reason and is not stored as the PDF.
- **Conditional requests:** where a stored version has upstream's `ETag` or `Last-Modified`,
  Cortex asks with `If-None-Match` / `If-Modified-Since`; a `304` only moves `checked_at`. Costs
  the university a body less where its servers support it.

## 8. Building and running it

- **Go, in this repository:** `cmd/cortex`, `internal/cortex/…`; with `internal/oplog`,
  `internal/metrics`, `modernc.org/sqlite`, the retry and jitter of `internal/crawl`. One static
  binary, `nix build .#cortex-image`, shipped like Radix (tag per commit, `docker load`), deployed
  by a host script of its own before the application (`vps/48-cortex.sh`), as infrastructure for
  every instance and colour: one Cortex per host, not per instance.
- **Resources:** I/O and network, little else: 0.5 CPU and 256 MB per instance. The disk: the
  pages of Radix's archive once, plus every version that changed, plus the models.
- **Configuration:** `CORTEX_*` variables as Radix's flags, the allowlist as a file,
  `cortex-token-*` as swarm secrets.

## 9. Metrics, logs, alerts

`GET /metrics` (Prometheus job `cortex`, a dashboard „Cortex"):

| Metric | What it says |
|---|---|
| `cortex_requests_total{client,source,mode,result}` | `hit`, `miss`, `stale`, `coalesced`, `offline_miss`, `refused` |
| `cortex_upstream_requests_total{host,code}`, `cortex_upstream_request_duration_seconds{host}` | what reached the university, and how fast it answered |
| `cortex_host_queue{host}`, `cortex_host_paused{host}` | waiting requests; the breaker |
| `cortex_role`, `cortex_epoch`, `cortex_journal_seq`, `cortex_replication_lag_seconds` | 1 for the leader; the follower's distance |
| `cortex_blobs`, `cortex_blob_bytes`, `cortex_versions` | the store |

Logs as Radix's (JSON lines, the levels are a contract, `docs/operations.md` §2): `upstream.fetched`,
`upstream.failed`, `host.paused` (WARN), `blob.rejected` (hash mismatch, ERROR), `leader.acquired`,
`leader.stepped_down`, `replica.caught_up`, `replica.diverged` (entries dropped after a takeover,
WARN), `gc.finished`.

Alerts: no instance leads, or both say they do; the follower missing for 5 minutes or more than a
minute behind; a host paused; the volume 80 % full.

## 10. Phases

1. **Cortex alone**: fetch API, store, allowlist, politeness, metrics; one instance. Radix through
   it behind `RADIX_CORTEX_URL`; the crawling colour with `cache`.
2. **The hot spare**: journal, follower, `flock`, the client's switch-over, alerts. Accepted when
   a `kill -9` of the leader during a crawl costs Radix at most one repeated request and no failed
   page.
3. **Offline by policy**: tokens, `RADIX_CRAWL=cache` for canary, Radix's networks internal.
4. **Models**: blobs, pins from `models.lock`, `ship-models.sh` uploads to Cortex; Radix and Folia
   load from it or keep the mount (Q7); base models from Hugging Face with `expect`.
5. **Optional**: a fresh colour fills its archive from `/v1/entries` instead of a copy of
   `radix.db`; replays of old versions with `at=`.

**Not chosen, and why:** nginx, Varnish or Squid as a caching proxy: an `https` URL through a proxy
is a tunnel the cache cannot see into without a CA of its own; none of them has a hot spare that
follows its store, nor an offline mode a client can be held to, nor a content-addressed store for
models. One SQLite file both instances open on a shared volume: no second copy of anything (a
broken file breaks both), and no way to a second host. LiteFS: the same replication, with Consul
for the lease: one more service for what `flock` does on one host.

## 11. Open questions

| # | Question | Recommendation |
|---|---|---|
| Q1 | Both instances on this host (covers a crash, an OOM kill, a bug; not the host), or a second server? | This host now; the journal is built so that a second host needs only another lease |
| Q2 | Pages replicated after the fact (the last second before a crash may be fetched again), uploads before they are confirmed. An upload while the follower is away: accept with a WARN, or refuse? | Pages after the fact; uploads confirmed by both, accepted with a WARN while one is away |
| Q3 | Offline enforced by Cortex per client (tokens), or only a parameter the client chooses? Canary on `RADIX_CRAWL=cache`? | Enforced, and canary with `cache`: it then sees the public site's pages continuously instead of a copy per deploy |
| Q4 | Cortex only limits how fast (floor per host), Radix keeps what and when? Or should Cortex own the off-peak window, or refresh pages on its own? | Floor only; Cortex stays passive |
| Q5 | Every version that differed: how long? | Current versions forever, older ones 180 days (an unchanged fetch costs nothing) |
| Q6 | Gemini: not through Cortex, passed through without storing, or stored by a hash of the request? | Passed through without storing, so that Radix needs no way out; no key in the store, and the summaries are in `radix.db` |
| Q7 | Model weights: who fetches them? Radix and Folia on the server (Cortex replaces or mirrors the host store), builds (the workstation, GitHub Actions: Cortex would have to be reachable from outside), or both? An `HF_ENDPOINT` mirror? | Cortex replaces the store for the services (b in §4.3); base models by URL and hash; no mirror for now |
| Q8 | Take Radix's way to the internet away, so that only Cortex has one? | Yes, once Q6 is settled |
| Q9 | Go in this repository, and the name Cortex? | Yes: it reuses Radix's crawl, log, metrics and SQLite code |
