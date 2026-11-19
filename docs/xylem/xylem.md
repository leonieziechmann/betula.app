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

What is announced, Betula mostly knows by itself (owner, 2026-11-18: „Mach das mal auch so, dass
das automatisch funktioniert mit den notifications"): the site names the dates of a semester as
a feed, and Xylem reads it („The feed" below). A person announces what no data says.

What it says on the wire, and what Folia's part has to do, is the contract in
[`protocol.md`](protocol.md). This file is how to build, run and watch it.

| | |
|---|---|
| code | `xylem/`, a Go module of its own (`cmd/xylem`, `internal/{server,store,webpush,metrics,oplog,dotenv,version}`), the standard library and SQLite (`modernc.org/sqlite`) alone |
| version | `xylem/internal/version/version.go` (0.2.0); `xylem version`, `xylem status` and `xylem_build_info` show it with the build (the start of the executable's SHA-256) |
| builds | `nix build .#xylem` (the binary, its tests run in the build), `nix build .#xylem-image` (the image `betula-xylem`: the binary, its firewall, `nft` and the CA certificates; no shell) |
| on the server | one stack per site, `xylem` and `xylem-canary` (`deploy/stacks/xylem.yml`, `deploy/ship-xylem.sh`): „On the server" below, and section 17 of `deploy/README.md` |

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
- Folia offers notifications and finds it there with `FOLIA_PUSH=on
  FOLIA_PUSH_URL=http://127.0.0.1:8300/push/v1/` (docs/folia/frontend.md, „Notifications"). The
  row of the menu and the subscription work with `scripts/dev.sh` as well; the service worker of
  `--live-assets` shows no notification, so to see one arrive the server runs without it.
- `XYLEM_SITE` is the origin a notification opens (its `navigate`): always https, so on a
  development machine a notification opens the real site, or canary's.
- `XYLEM_FEED=http://127.0.0.1:8080/api/announcements` lets it read what the dev server's
  catalog announces („The feed"); without it only `xylem announce` announces.
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
| `start` | the image's entrypoint („On the server"): as root, the firewall; then, as Xylem's user without any privilege, `serve` with the arguments it was given |
| `serve` | the service (the default command): the HTTP API and the sender, until SIGTERM; then no new connections, the requests in flight end (5 s), the sender ends the batch it is in (a request to a push service ends within 15 s), and the stats file says `xylem_up 0` |
| `announce` | tell everybody who listens to a topic something, now or at a time (below) |
| `announcements` | the newest announcements with how far each has come (`--limit`, `--json`) |
| `cancel <id>` | end an announcement that is not finished: nobody else gets it; one of the feed's stays cancelled whatever the feed names |
| `feed` | read the feed now instead of at its next turn, and say what the read changed (`--json`) |
| `status` | the local instance's numbers as one JSON object (`GET /status`): subscriptions in all, by topic and by language, the announcements that are not finished, the hosts of endpoints it refused |
| `key` | the public half of the VAPID key |
| `healthcheck` | exit 0 when the local instance answers `/healthz` |
| `version`, `help` | |

`announce`, `announcements`, `cancel`, `feed` and `status` ask the instance of the same machine over
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
id    state      by       topic        at                       sent   gone failed  title
3     waiting    feed     exams        2027-02-08 07:00 CET        0      0      0  Die Prüfungsphase beginnt
2     waiting    feed     breaks       2026-12-18 15:00 CET        0      0      0  Weihnachtspause
1     done       command  semester     2026-11-23 08:00 CET      412      3      0  Neu in Betula: Benachrichtigungen
```

`waiting` (its time has not come), `sending`, `retrying` (through; some subscriptions wait for
another attempt), `done`, `cancelled` (by a person), `expired` (Xylem did not run while it was
due: nobody hears stale news, and the log says so at WARN), and the feed's two: `withdrawn` and
`skipped` („The feed"). `by` is who made it: a person's `command`, or the `feed`. `sent` are the
subscriptions a push service took it for, `gone` the ones a push service called gone (Xylem
forgot them), `failed` the ones given up on.

## The feed

`XYLEM_FEED` is an address of the site: Folia's `GET /api/announcements`
(`folia/crates/pages/src/announce.rs`, docs/folia/frontend.md), which names what the catalog says
of a semester as announcements, each with a key, a topic, its words in every language, and the
day and hour it is worth telling:

| key | topic | told | from |
|---|---|---|---|
| `lectures-2026w-begin` | `semester` | the morning of the first lecture day, 07:00 | the lecture period, which Folia reads from the weekly rows' dates |
| `breaks-2026w-1` | `breaks` | the Friday before a week without lectures, 15:00, until Monday („Weihnachtspause" where it holds Christmas) | the weeks of the period with next to no single dates |
| `exams-2026w-1`, `-2` | `exams` | the Monday morning an exam period begins | the weeks the semester's exam dates gather in |
| `dates-2027s` | `semester` | when Xylem first reads it, in the hours of the day | the catalog moved on to a semester that has not begun: its schedule is published |

Xylem reads the feed at its start and every ten minutes (`XYLEM_FEED_INTERVAL`), with the
feed's ETag, and keeps what it names as announcements of its own (`internal/server/feed.go`,
`store.SyncFeed`, one step per read):

- **New**: it waits for its time, like one a person made (`made`).
- **Named otherwise while it waits** (the catalog learned more: the exams begin a week later):
  it is changed (`revised`). Once it is being told or was told, nothing changes it.
- **No longer named while it waits**: `withdrawn`, and nobody is told. Named again, it waits
  again (`revived`).
- **Its time is over** when Xylem reads it: not taken (`late`).
- **Not readable** (no key, a key twice, a text too long, a link off the site): left out, the
  rest is read, and one that waited under its key stays as it was (`invalid`, and `feed.invalid`
  at WARN once).
- **A person's word stands above the feed's.** What `xylem cancel` cancelled stays cancelled, one
  the feed withdrew can be cancelled so that it stays away, and an announcement a person made
  under a key is theirs whatever the feed names under it.

Three rules keep a feed that went wrong from every phone:

- **The first read tells no old news.** Something without an hour of its own (`dates-…`) is so,
  and nobody knows since when: on the first read of a feed (a new database, another feed's
  address) it is kept as `skipped` and never told. Only what becomes so later is news.
- **By day.** What has no hour of its own is told at once between 08:00 and 20:00 in the
  machine's time zone, else at the next 08:00: the catalog is built at night.
- **Four within a day.** The feed makes at most four announcements whose times lie within a day
  of each other; a fifth is not taken (`capped`, `feed.flood` at ERROR once an hour) and is read
  again at the next turn. Everything else of an announcement holds as for a person's: a link is
  a path of the site, the texts have their lengths.

A feed that cannot be read (the site is down, has no snapshot yet, answers what is no feed)
changes nothing: what waits is told at its time, nothing is withdrawn, `feed.failed` at WARN
until it reads again, and `xylem status` says why. `{"announcements": []}` is a feed that names
nothing; a document without that member is no feed.

`xylem feed` reads it now:

```
The feed names 3 announcements.
  made       breaks-2026w-1  (2026-12-18 15:00 CET)
  made       exams-2026w-1  (2027-02-08 07:00 CET)
  made       exams-2026w-2  (2027-03-08 07:00 CET)
```

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
| `XYLEM_FEED` | none | the address of the site's feed of announcements, `http://` or `https://`; none: only a person announces |
| `XYLEM_FEED_INTERVAL` | `10m` | how often it is read; a minute at least |
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

| the feed | 256 KiB, 64 announcements, read within 20 s; four of its announcements within a day of each other; without an hour of its own told between 08:00 and 20:00 |

The limits per address live in memory: a restart starts them anew.

## Storage

`xylem.db` (SQLite, WAL, synchronous FULL; one writer, `internal/store`):

| table | |
|---|---|
| `meta` | `vapid`: the private half of the VAPID key, made on the first start; `feed`: the address of the feed last read (another one's first read is a first read) |
| `subscriptions` | `nr` (the order they came, never given out again), `id` (SHA-256 of the endpoint), `endpoint`, `p256dh`, `auth`, `lang`, `created` |
| `topics` | a subscription's topics |
| `announcements` | topic, time, texts, state, how far it has come (`cursor`), `sent`, `gone`, `failed`; who made it (`source`: `command` or `feed`) and whether its maker named its time (`timed`) |
| `retries` | the subscriptions an announcement could not be written to yet, with how often it tried and when it tries again |

Nothing else of a browser is kept: no address, no profile, no account, nothing of Betula Sync,
not when it was last seen. A subscription goes when its browser deletes it, puts it without a
topic, or its push service calls it gone; never by age (a phone that only reads its
notifications never comes back to say it is there).

**The key is as old as the first subscription.** Every subscription was made with the public
half of the VAPID key, and a push service takes a message for it only with a token of that key.
A database that is lost takes the key along: every browser has to subscribe again, which Folia's
part does by itself at its next start (protocol.md section 5), so nothing is asked of anybody,
but a phone that is not opened hears nothing until it is.

**Backup.** `xylem.db` lies on the server's disk and is part of the host's own daily backup
(owner, 2026-11-19: „ich habe auto backup bei meinem hosting anbieter enabled … 24h datenverlust
ist verkraftbar"); Xylem makes none of its own. The database is written with `synchronous FULL`,
so a copy of the disk at any moment opens like one after a power cut. Brought back from a day
ago, it holds the key and every subscription but the last day's: those browsers put theirs
again with their next visit (the same step of Folia's part), and the announcements made since
come back with the feed's next read, each under its key. An announcement that was told in that
day and is still due in the copy would be told a second time: after a restore, look at
`xylem announcements` before Xylem's next minute, and `xylem cancel` what was told already.

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
- `xylem_feed_reads_total{result}` (`read`, `unchanged`: 304, `failed`),
  `xylem_feed_changes_total{event}` (`made`, `revised`, `revived`, `withdrawn`, `skipped`,
  `late`, `invalid`, `capped`), `xylem_feed_read_time_seconds` (the last read that came through;
  0: never);
- `xylem_rejections_total{reason}`: what browsers were refused: `origin`, `invalid`, `too_big`,
  `endpoint`, `keys`, `lang`, `topics`, `slow_down`, `full`.

A topic is never a label: anybody may name one. `xylem status` counts by topic.

Logs are JSON lines (`level`, `event`, `msg`, `component`), one per event, never one per request
or push: `service.started`, `service.stopped`, `service.fatal`, `service.root`, `service.key`
(a database's first start: its VAPID key was made), `http.listening`, `http.failed`,
`db.migrated`, `store.failed`, `sweep.done`, `stats.failed`, `stats.recovered`,
`subscriptions.full`, `announcement.made`, `announcement.started`, `announcement.done` (with
`sent`, `gone`, `failed`), `announcement.expired` (WARN), `announcement.cancelled`,
`push.unauthorized` (ERROR), `feed.read` (a read that changed something, with what),
`feed.failed` (WARN, once until it reads again), `feed.recovered`, `feed.invalid` (WARN, once
per entry and reason), `feed.flood` (ERROR, once an hour). Nothing of a subscription is logged, ever: an endpoint is a
browser's address, and who knows it may delete the subscription.

## On the server

Xylem is run as Phloem is, as if it were an outside service: a stack of its own per site
(`xylem` for https://betula.app, `xylem-canary` for https://canary.betula.app; one file,
`deploy/stacks/xylem.yml`, and what differs in `deploy/stacks/xylem/<stack>.env`), nothing but
Traefik on the network its requests come in by, its data on a file system of its own, its
numbers as a file, a firewall inside its container, and a guard that looks at it from outside.
The one difference: Xylem has to reach the internet, so it has a way out, and the firewall is
what holds that way to what Xylem needs (owner, 2026-11-19, of a network of its own that lets
out DNS and port 443 to public addresses alone: „Ja passt so"). Section 17 of
`deploy/README.md` has the steps; this is what they set up.

| | |
|---|---|
| `<stack>_in` | overlay, internal, not attachable, made by `vps/30-docker.sh`: Traefik and Xylem. Traefik's router `<stack>-push` takes `https://<host>/push/v1/` above Folia's and hands it to port 8300 (`push-ratelimit`: 5 requests a second per address, bursts of 20) |
| `<stack>_out` | overlay, not internal, made by the stack, Xylem's alone: its way out |
| `/data` | `/var/lib/betula/xylem/<stack>/data`: an ext4 of fixed size (`XYLEM_DATA_SIZE`) in a sparse file on a loop device, Xylem's (65532, 0700); `vps/47-xylem-host.sh` |
| `/stats` | `/run/betula/xylem/<stack>/stats`: a tmpfs of 1 MiB; Alloy reads `xylem.prom` there through `/rootfs` |
| the feed | `XYLEM_FEED=on`: `https://<host>/api/announcements`, read at the site's public address like anybody (from a container the host's own address answers through Traefik's published port) |

**`xylem start`** is the image's entrypoint. As root with `NET_ADMIN`, `SETUID` and `SETGID`
and nothing else, it loads `/etc/xylem/firewall.nft` (`xylem/deploy/firewall.nft`) into the
container's network namespace, checks that the table is there with its way out behind the drop
of every address that is none on the internet and that a datagram to such an address is dropped,
sets `no_new_privs`, becomes 65532 without groups, reads from `/proc` that no capability is
left and root does not come back, and then becomes `xylem serve` (the same process, no shell).
Anything that fails ends it before Xylem serves: without its firewall Xylem does not run. Every
other command run as root (`docker exec`, the health check) becomes Xylem's user first.

**The firewall** (table `inet xylem`):

- in: loopback (the commands, the health check), replies, and new TCP connections to 8300;
- out: replies; loopback; a name asked of Docker's resolver (`127.0.0.11`, which asks the
  host's resolvers from the host's side); new TCP connections to port 443 of an address on the
  internet;
- nothing to an address that is none on the internet (RFC 6890: every other container, the
  host's own networks, the metadata service at `169.254.169.254`), no other port, no datagram,
  no IPv6, nothing routed through.

Docker has no allowlist of hosts for a container's way out, and the push services have many
addresses that change, so neither has this: a Xylem taken over could speak HTTPS to any address
on the internet. It could not reach anything inside the house, nor be reached but on 8300.
Xylem's own dialer holds the same rule in front of it (a push service's name that leads to a
private address reaches nothing, „The push services"), and sends over IPv4 alone, so that
nothing Xylem itself does is ever dropped. Every drop that means someone tried something is
counted; `betula-xylem-guard` (Phloem's guard script with `BETULA_GUARD_KIND=xylem`, every
minute as root on the host) reads the counters and the rights of the container's processes from
outside, and the rules of group `betula-xylem` alert: not running, no firewall or a process
with rights, a counted drop, a feed that does not read for an hour, notifications given up on.

**`xylem probe`** tries all of that from inside (`vps/49-xylem.sh` runs it after every deploy,
with Traefik's and the host's addresses as more targets): one JSON line per attempt with what
was found and what should be, exit 1 where they differ. Closed: a resolver other than Docker's,
a port other than 443, the metadata service, a private address, IPv6. Open: a name through
Docker's resolver, a public address on 443. And the service's process has no privilege.

The commands on the server go through `docker exec`, over loopback, where Xylem answers them and
nobody else:

```
x() { docker exec "$(docker ps -q -f label=com.docker.swarm.service.name=xylem_push | head -n 1)" /bin/xylem "$@"; }
x status
x announcements
x announce --key exam-registration-2027s --topic exams --at "2027-05-03 08:00" --title "Die Prüfungsanmeldung beginnt" --body "…"
```

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
- `internal/server/feed_test.go`: against a feed of the test's own: the first read's old news,
  what is told by day, an announcement that follows the feed while it waits and not after, what
  is withdrawn and named again, a person's word above the feed's, entries that are not readable,
  the four within a day, a feed that cannot be read, another feed.
- `cmd/xylem`: `serve` with every command against it, a feed among them; what `start` reads
  of `/proc` and of the table before it serves; the firewall's addresses that are none on the
  internet are the dialer's; `firewall.nft` loads in nft's dry run (as root, where nft is).

On the server's side, `deploy/test/netmatrix.sh` builds the networks of the plan in a local
swarm and holds Xylem to its place with `vps/91-verify-stacks.sh xylem`: its two networks and
who is on them, its table, what it reaches (Docker's resolver, the internet on 443) and what it
does not (every other service, Traefik over the network they share, the host), what Traefik
gets of it, and two mutations the checks must catch (no firewall; a firewall that lets it into
the house). With `NETMATRIX_XYLEM_IMAGE` it is the real service, with the firewall `xylem
start` loaded itself. Passed on 2026-11-19 both ways, the real one from an image put together
by hand with the layout of `xylem-image` (`nix build .#xylem-image` itself was not run in that
session: no way to nixpkgs there).

Folia's own checks cover what the feed names (docs/folia/frontend.md). Both together, on
2026-11-18: Folia's server on canary's catalog and Xylem with its feed made, by themselves,
`breaks-2026w-1` for Friday 18.12.2026 15:00 („Weihnachtspause"), `exams-2026w-1` for Monday
08.02.2027 and `exams-2026w-2` for 08.03.2027.

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
message, which opened to the notification that was announced. Again on 2026-11-19 from Xylem's
container, as its stack runs it and through its firewall: the subscription put through Traefik's
router, the announcement made with `docker exec`, the message delivered and opened; and none of
it tripped a counter of the firewall. Google's, Apple's and Microsoft's services take the same
request by the same RFCs, but only a browser subscribes at them: they are checked with Folia's
part, on a phone.

## Not yet

- **A real phone.** Folia's part is there (protocol.md section 5), checked in a browser against
  a Xylem that runs; Google's, Apple's and Microsoft's push services, the browsers' own
  permission dialogs and an iPhone's home-screen app are tried on canary once Xylem runs there.
- **A dashboard.** The numbers are in Prometheus (job `xylem`) and the alerts are there
  (group `betula-xylem`); a dashboard of its own beside „Sync" is not.
- **More that Betula could announce by itself.** The feed names what the catalog's dates say.
  What they do not say stays a person's to announce (`xylem announce`): when registration for
  the exams opens, re-registration, anything about Betula itself.
- **A person's own day** (the timetable as a notification): another kind of message, since no
  server knows a plan. Xylem's messages are declarative ones without `mutable`; a message a
  service worker fills in from what the browser keeps would be a second kind beside them.
