# Xylem: running it

Xylem is the service of Betula's notifications (owner, 2026-11-18: „lass mal als erstes damit
anfangen einen notification server zu erstellen. Nicht für diese Modul Updates, sondern für so
allgemeine Updates, wie prüfungsphase beginnt, Termine für das nächste Semester verfügbar,
weihnachtspause etc."). A browser that asked for notifications leaves its push subscription and
the topics it listens to; an announcement for a topic is written to every one of them, in its
language, sealed for each browser, through the browser's own push service (Google's, Mozilla's,
Apple's, Microsoft's). It says the same to everybody who listens: it knows no profile and no
plan, and nothing in it says whose browser a subscription is. In the tree, the xylem carries one
way, from the root up to the leaves; Phloem carries both ways.

What it says on the wire, and what Folia's part has to do, is the contract in
[`protocol.md`](protocol.md). This file is how to build, run and watch it.

| | |
|---|---|
| code | `xylem/`, a Go module of its own (`cmd/xylem`, `internal/{server,store,webpush,metrics,oplog,dotenv,version}`), the standard library and SQLite (`modernc.org/sqlite`) alone |
| version | `xylem/internal/version/version.go` (0.1.0); `xylem version`, `xylem status` and `xylem_build_info` show it with the build (the start of the executable's SHA-256) |
| builds | `nix build .#xylem` (the binary, its tests run in the build) |
| not yet | an image, a stack and its place on the server („Not yet" below); Folia's part (the permission, the settings, the service worker's `push`) |

## Development

Go 1.27, as Radix, Cortex and Phloem. In `xylem/`:

```
go test ./...
go build -o xylem ./cmd/xylem
```

For Folia's dev server (the app on http://127.0.0.1:8080), Xylem runs next to it and the browser
talks to it directly, cross-origin:

```
XYLEM_SITE=https://betula.app XYLEM_ORIGINS=http://127.0.0.1:8080 XYLEM_DATA=xylem-data XYLEM_LOG_FORMAT=text ./xylem serve
```

- It listens on `127.0.0.1:8300` (`XYLEM_ADDR`): the API at http://127.0.0.1:8300/push/v1/.
- `XYLEM_SITE` is the origin a notification opens (its `navigate`): always https, so on a
  development machine a notification opens the real site, or canary's.
- `XYLEM_ORIGINS` lists the origins it answers CORS for (preflights included: `Content-Type`;
  GET, PUT, DELETE). Another origin gets 403 `origin`; a request without one (curl, a test) is
  taken.
- `xylem-data/xylem.db` is made on the first start (SQLite, migrated by Xylem itself), and with
  it the VAPID key (`service.key` in the log names its public half).
- A real browser subscribes at a real push service, and Xylem writes to it from the development
  machine as from the server: there is no test mode. What it sends to are public addresses on
  port 443 alone, through no proxy.
- A `.env` in the working directory (git-ignored; another file: `XYLEM_ENV_FILE`) is read for
  every command; the real environment wins over it.

## Commands

| | |
|---|---|
| `serve` | the service (the default command): the HTTP API and the sender, until SIGTERM; then no new connections, the requests in flight end (5 s), the sender ends the batch it is in (a request to a push service ends within 15 s), and the stats file says `xylem_up 0` |
| `announce` | tell everybody who listens to a topic something, now or at a time (below) |
| `announcements` | the newest announcements with how far each has come (`--limit`, `--json`) |
| `cancel <id>` | end an announcement that is not finished: nobody else gets it |
| `status` | the local instance's numbers as one JSON object (`GET /status`): subscriptions in all, by topic and by language, the announcements that are not finished, the hosts of endpoints it refused |
| `key` | the public half of the VAPID key |
| `healthcheck` | exit 0 when the local instance answers `/healthz` |
| `version`, `help` | |

`announce`, `announcements`, `cancel` and `status` ask the instance of the same machine over
loopback (`--instance`, else from `XYLEM_ADDR`), where it answers them and nobody else: a request
that came through a proxy, from another address or from a page (it names an `Origin`) gets 404.
In a container that is `docker exec <container> /bin/xylem announce …`. There is no way to make
an announcement from the network, on purpose: whoever could would write to every phone.

### An announcement

```
xylem announce --topic exams --key exams-2027-02 \
  --title "Die Prüfungsphase beginnt" --body "Ab heute laufen die Prüfungen. Viel Erfolg!" \
  --title en="Exam period begins"     --body en="Exams start today. Good luck!" \
  --link /studyplan --link en=/en/studyplan \
  --at "2027-02-01 07:00"
```

| flag | | |
|---|---|---|
| `--topic` | needed | who hears it: the subscriptions that listen to this topic |
| `--title`, `--body` | a title is needed | the text in the default language (`XYLEM_LANG`, `de`); once more per other language as `lang=text`. A browser of a language without a text reads the nearest: `en` for `en-gb`, else the default. 120 and 600 characters at most |
| `--link` | `/` | the path on the site the notification opens, always the site's own; `lang=/path` for a language whose page has another address |
| `--at` | now | when it is told: `2027-02-01 07:00` in the machine's time zone (the image's is Europe/Berlin), or RFC 3339 |
| `--for` | `24h` | how long it is told: a device that was off hears it when it comes back within this, and nobody after. One minute to 28 days |
| `--urgency` | `normal` | `very-low`, `low`, `normal`, `high`: how far a push service goes to wake a device (`high` wakes a sleeping phone at once) |
| `--key` | none | a name of its own: made again under it, it is not told twice (and it is the notification's tag) |
| `--dry-run` | | check it, say how many listen now by language, tell nobody |

What is wrong with one comes back in words („the title in "en" has 130 characters, 120 at
most"). An announcement whose time is over when it is made is not taken; one whose time began
and is not over is told now. `xylem announcements` shows them:

```
id    state      topic          at                       sent   gone failed  title
2     waiting    breaks         2026-12-21 07:00 CET        0      0      0  Weihnachtspause
1     done       exams          2026-11-23 08:00 CET      412      3      0  Die Prüfungsphase beginnt
```

`waiting` (its time has not come), `sending`, `retrying` (through; some subscriptions wait for
another attempt), `done`, `cancelled`, `expired` (Xylem did not run while it was due: nobody
hears stale news, and the log says so at WARN). `sent` are the subscriptions a push service took
it for, `gone` the ones a push service called gone (Xylem forgot them), `failed` the ones given
up on.

## Settings

Every flag of `serve` has an environment variable.

| variable | default | |
|---|---|---|
| `XYLEM_ADDR` | `127.0.0.1:8300` | listen address; also how the commands find the instance |
| `XYLEM_DATA` | `xylem-data` | the directory of `xylem.db` |
| `XYLEM_SITE` | none, needed | the site's origin, `https://host`: what a notification's link opens at |
| `XYLEM_CONTACT` | the site | a `mailto:` or `https:` address, sent to a push service with every message (the token's `sub`): where its operators reach Xylem's |
| `XYLEM_LANG` | `de` | the language every announcement is written in |
| `XYLEM_ORIGINS` | the request's own host | comma-separated `scheme://host[:port]` |
| `XYLEM_PUSH_HOSTS` | none | hosts taken as push services beside the four below, comma-separated; `.name`: every host under it |
| `XYLEM_MAX_SUBSCRIPTIONS` | 200 000 | at it no new subscription (507) |
| `XYLEM_STATS_DIR` | none | where `xylem.prom` is written |
| `XYLEM_STATS_INTERVAL` | `15s` | how often |
| `XYLEM_STACK` | none | the label `stack=` of the stats file |
| `XYLEM_LOG_FORMAT`, `_LEVEL`, `_FILE` | `json`, `info`, none | |

## The push services

A subscription's endpoint is an address a stranger hands Xylem, and Xylem sends a request
there. So an endpoint is taken only where it is a push service's (`internal/server/endpoint.go`):
https, no user, the standard port, and a host of

| | hosts | whose browsers |
|---|---|---|
| `google` | `fcm.googleapis.com`, `jmt17.google.com` | Chrome and what is built from it (Edge off Windows, Samsung Internet, Brave, Opera, Vivaldi) on Android and the desktops |
| `mozilla` | `*.push.services.mozilla.com` | Firefox |
| `apple` | `*.push.apple.com` | Safari on a Mac, and every web app on an iPhone's or iPad's home screen, whatever browser added it |
| `microsoft` | `*.notify.windows.com` | Edge on Windows |

Anything else is refused (400 `endpoint`), and its host, nothing more of it, is counted in
`xylem status` under `refused`: a browser with a push service Xylem does not know cannot
subscribe, and that is how one learns of it. `XYLEM_PUSH_HOSTS` takes more (label `other`).

Behind that check stands the dialer's (`publicOnly`): whatever a name resolved to, Xylem's own
client connects to a public IPv4 address on port 443 or not at all, goes through no proxy, takes
TLS 1.2 or newer and follows no redirect.

What a push service learns: that Betula's key sent this subscription a message, when, and how
large. Not what: the message is sealed with keys only the browser holds (RFC 8291).

## The sender

One goroutine (`internal/server/send.go`) looks for what is due every 5 s, and at once when an
announcement is made:

- An announcement whose time has come walks the subscriptions of its topic in the order they
  came, 100 at a step, 8 requests at a time. Who subscribed after its time does not hear what was
  said before.
- A step is written and recorded as one: after a crash it is written again whole or not at all,
  so at most 100 subscriptions hear an announcement twice; where the browser knows tags (all but
  Safari), the second takes the first one's place without a sound. A stop (SIGTERM) lets the
  step end first.
- What a push service answers:

  | answer | | counted as |
  |---|---|---|
  | 2xx | taken | `sent` |
  | 404, 410 | the subscription is no more (data cleared, permission taken back, app removed): Xylem forgets it | `gone` |
  | no answer, 429, 5xx | another attempt after 1 min, 5 min, 15 min, 1 h, 3 h, 6 h, 12 h (a `Retry-After` up to an hour is followed where it is longer), while the announcement is told | `retry`, at last `gave_up` |
  | 401, 403 | the service refuses Xylem's token. The subscription stays (a wrong clock or key of Xylem's must not empty the database); `push.unauthorized` at ERROR, once an hour per service | `unauthorized` |
  | 400, 413, other | given up for this announcement | `invalid`, `too_big`, `refused` |

- Every request carries `TTL` (what is left of the time the announcement is told: the push
  service keeps it that long for a device that is off), `Urgency`, `Content-Encoding: aes128gcm`
  and `Authorization: vapid t=…, k=…`: a token signed with the VAPID key for the push service's
  origin, for twelve hours, used for every request to it and replaced after ten.
- An announcement is finished when it has been through everybody and nothing waits, or when its
  time is over (what still waited counts as failed). Finished ones are kept 395 days.

## Limits

Every limit is Xylem's own and holds whatever a client does (`internal/server/server.go`,
`DefaultLimits`).

| | |
|---|---|
| a request's body | 32 KiB |
| an endpoint | 2048 bytes |
| topics of a subscription | 16, each lower-case letters, digits and hyphens, 32 at most |
| subscriptions put or deleted by one address | 1200 an hour (an IPv6 address counts by its /64; behind Traefik the address is its `X-Real-Ip`) |
| new subscriptions of one address | 120 an hour: a lecture hall behind one address; one Xylem holds already is brought up to date beyond it |
| new subscriptions in all | 5000 an hour |
| subscriptions in all | 200 000 (`XYLEM_MAX_SUBSCRIPTIONS`); at it 507 `full`, and `subscriptions.full` at WARN once |
| an announcement | a title of 120 and a body of 600 characters, a link of 512 bytes, 8 languages, told for a minute to 28 days; its notification at most 3993 bytes, what one push message carries |

The limits per address live in memory: a restart starts them anew.

## Storage

`xylem.db` (SQLite, WAL, synchronous FULL; one writer, `internal/store`):

| table | |
|---|---|
| `meta` | `vapid`: the private half of the VAPID key, made on the first start |
| `subscriptions` | `nr` (the order they came, never given out again), `id` (SHA-256 of the endpoint), `endpoint`, `p256dh`, `auth`, `lang`, `created` |
| `topics` | a subscription's topics |
| `announcements` | topic, time, texts, state, how far it has come (`cursor`), `sent`, `gone`, `failed` |
| `retries` | the subscriptions an announcement could not be written to yet, with how often it tried and when it tries again |

Nothing else of a browser is kept: no address, no profile, no account, nothing of Betula Sync,
not when it was last seen. A subscription goes when its browser deletes it, puts it without a
topic, or its push service calls it gone; never by age (a phone that only reads its
notifications never comes back to say it is there).

**The key is as old as the first subscription.** Every subscription was made with the public
half of the VAPID key, and a push service takes a message for it only with a token of that key.
A database that is lost takes the key along: every browser has to subscribe again, which Folia's
part does by itself at its next start (protocol.md section 5), so nothing is asked of anybody,
but a phone that is not opened hears nothing until it is. Whether `xylem.db` gets a backup is
part of the deploy („Not yet").

## Numbers and logs

The stats file `xylem.prom` (Prometheus text, every 15 s, written by a rename) holds, each with
`stack`:

- `xylem_up`, `xylem_build_info{version,build}`, `xylem_start_time_seconds`,
  `xylem_stats_time_seconds`, `xylem_log_problems_total{level,event}`;
- `xylem_subscriptions`, `xylem_subscriptions_max`,
  `xylem_subscriptions_changed_total{event,service}`: `added`, `updated` (put again), `removed`
  (by its browser), `gone` (its push service said so), by push service (`google`, `mozilla`,
  `apple`, `microsoft`, `other`);
- `xylem_pushes_total{service,result}`: `sent`, `gone`, `retry`, `gave_up`, `unauthorized`,
  `invalid`, `too_big`, `refused`; `xylem_push_seconds{service}`, how long a request took;
- `xylem_announcements_total{event}`: `made`, `again` (made under a key that was taken),
  `started`, `done`, `expired`, `cancelled`; `xylem_announcements_pending{state}` (`waiting`,
  `sending`, `retrying`), `xylem_retries_waiting`;
- `xylem_rejections_total{reason}`: what browsers were refused: `origin`, `invalid`, `too_big`,
  `endpoint`, `keys`, `lang`, `topics`, `slow_down`, `full`.

A topic is never a label: anybody may name one. `xylem status` counts by topic.

Logs are JSON lines (`level`, `event`, `msg`, `component`), one per event, never one per request
or push: `service.started`, `service.stopped`, `service.fatal`, `service.root`, `service.key`
(a database's first start: its VAPID key was made), `http.listening`, `http.failed`,
`db.migrated`, `store.failed`, `sweep.done`, `stats.failed`, `stats.recovered`,
`subscriptions.full`, `announcement.made`, `announcement.started`, `announcement.done` (with
`sent`, `gone`, `failed`), `announcement.expired` (WARN), `announcement.cancelled`,
`push.unauthorized` (ERROR). Nothing of a subscription is logged, ever: an endpoint is a
browser's address, and who knows it may delete the subscription.

## Checks

`go test ./...` in `xylem/`, without a network:

- `internal/webpush`: the example of RFC 8291, octet for octet; what is sealed opens only with
  its subscription's keys; a VAPID token as RFC 8292 wants it, and one that was changed or
  signed by another key is not taken.
- `internal/store`: subscriptions kept, replaced and counted; an announcement's walk in batches,
  its attempts, what ends it; a database of a newer Xylem is refused.
- `internal/server`: against a push service of the test's own (`helpers_test.go`), which takes
  a request only as a real one would (the headers, the token's signature, audience and time, the
  size) and opens every message with the browser's keys: who hears what in which language, gone
  subscriptions, the waits, giving up, cancelling, announcements late and expired; what a
  subscription and an announcement are checked for; the origins, the commands' side being
  local, the limits per address, the stats file, the addresses the dialer refuses.
- `cmd/xylem`: `serve` with every command against it.

The live check goes through a real push service, Mozilla's, and runs only when asked (it needs
the internet, and Playwright's Chromium as the browser's side):

```
cd xylem
dir=$(mktemp -d)
node live/autopush.mjs "$dir" &
XYLEM_LIVE_DIR="$dir" go test ./internal/server -run TestLiveThroughMozilla -count=1 -v
```

`live/autopush.mjs` subscribes at Mozilla's push service as a Firefox would, with the key the
test wrote; the test is Xylem as `serve` runs it, makes an announcement, and opens what the
service delivered. Passed on 2026-11-18: the service took the request (201) and delivered the
message, which opened to the notification that was announced. Google's, Apple's and Microsoft's
services take the same request by the same RFCs, but only a browser subscribes at them: they are
checked with Folia's part, on a phone.

## Not yet

- **Its place on the server.** An image, a stack (`deploy/stacks/xylem.yml`) with one instance
  per site as Phloem has, Traefik's router for `https://<host>/push/v1/` above Folia's, the
  stats file for Alloy, a dashboard and alerts (`xylem_up`, `push.unauthorized`, announcements
  expired, the pushes that fail), `deploy/ship-xylem.sh`. Unlike Phloem, Xylem needs a way out:
  HTTPS to the push services, and DNS for their names. That is a network of its own that leads
  out (as Radix's `egress`), and to decide with it: a firewall inside the container as Phloem's
  (out: DNS and TCP 443 to public addresses, nothing else; Xylem's dialer holds the same rule
  already), and whether `xylem.db` gets a backup.
- **Folia's part**: asking for the permission, the topics as settings, the service worker's
  `push` and `notificationclick`, the privacy notice (protocol.md section 5). Until then nothing
  subscribes.
- **Announcements by themselves.** Today a person makes them (`xylem announce`). Some of what
  they say Betula could know by itself: Radix sees when a semester's dates arrive, and Folia
  works out a lecture period and its pauses from the dates of the events
  (`folia_timetable::facts`); the exam period is in no data yet. Whoever makes them then gives
  each a key of its own (`semester-2027s-dates`), which is what the key is for: made twice, told
  once.
- **A person's own day** (the timetable as a notification): another kind of message, since no
  server knows a plan. Xylem's messages are declarative ones without `mutable`; a message a
  service worker fills in from what the browser keeps would be a second kind beside them.
