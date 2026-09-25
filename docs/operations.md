# Operating Radix

Radix runs as one long-lived process. It keeps the raw archive fresh at a polite pace,
rebuilds the catalog, publishes a new snapshot when the content changed, and serves the
snapshots over HTTP. The web server is an HTTP client of it; they share no files.

## 1. Running it

```bash
radix run --addr 127.0.0.1:8090          # Windows, Linux, macOS: same binary, same flags
radix run --once                          # one cycle, then exit (exit code 1 if it failed)
```

One cycle, every `--interval` (30 min):

| Stage | When | What |
|---|---|---|
| `lists` | every cycle, if older than 12 h | module catalog list, FÜS list (one request each) |
| `modules` | off-peak only | module pages older than `--module-max-age` (7 d), oldest first, at most 400 per cycle |
| `qis-modules` | off-peak only | the QIS module table (in chunks, if older than 12 h) and the QIS module descriptions older than `--qis-module-max-age` (3 d), at most 600 per cycle. These are the source of the module fields and of the events of the current semester (`docs/data-sources.md` §10) |
| `tree` | off-peak only | QIS program tree; pages older than `--tree-max-age` (7 d), at most 300 requests per cycle; discovers new programs and PO versions |
| `events` | off-peak only | QIS pages of the events module pages link; older than `--event-max-age` (3 d), at most 600 per cycle |
| `retention` | every cycle | removes events `--event-retention` (30 d) after their last date, and events no module page links any more; remembers them, so they are not fetched again |
| `build` | every cycle | raw archive → canonical tables, one transaction, about 20 s. **Only the current dataset is built:** modules on the current lists, tree pages the current QIS root leads to |
| `archive` | every cycle | removes archived pages nothing leads to any more, `--archive-grace` (7 d) after their fetch |
| `validate` | when the content changed | invariants and count baselines; a failure blocks the export |
| `export` | when the content changed and validation passed | new `snapshot/catalog-<hash>.db`, `current.json` replaced atomically |

Off-peak is `--offpeak 1-6` (local hours, `Europe/Berlin` in the container; `any` disables the
window). A source that has no archived page yet is crawled immediately, so a fresh installation
fills itself without waiting for the night. Interrupting the process is always safe: every
page is archived on its own, and the build and the plan import are single transactions.

A cycle ends as `ok`, `degraded` (a crawl stage had failures; published data is intact and
only ages) or `failed` (build, validation or export failed; **the previous snapshot stays
current**). A failed cycle never stops the service; the next cycle tries again.

### Configuration

Every flag of `run` has an environment variable, so a container or unit file needs no command line.

| Flag | Environment | Default |
|---|---|---|
| `--db` | `RADIX_DB` | `radix.db` |
| `--snapshot-dir` | `RADIX_SNAPSHOT_DIR` | `snapshot` |
| `--addr` | `RADIX_ADDR` | `127.0.0.1:8090` |
| `--interval` | `RADIX_INTERVAL` | `30m` |
| `--offpeak` | `RADIX_OFFPEAK` | `1-6` |
| `--module-delay`, `--qis-delay` (ms) | `RADIX_MODULE_DELAY_MS`, `RADIX_QIS_DELAY_MS` | `500`, `500` (tree: twice the QIS delay) |
| `--module-max-age`, `--qis-module-max-age`, `--event-max-age`, `--tree-max-age` | `RADIX_MODULE_MAX_AGE`, `RADIX_QIS_MODULE_MAX_AGE`, `RADIX_EVENT_MAX_AGE`, `RADIX_TREE_MAX_AGE` | `168h`, `72h`, `72h`, `168h` |
| `--event-retention` | `RADIX_EVENT_RETENTION` | `720h` (0 keeps everything) |
| `--archive-grace` | `RADIX_ARCHIVE_GRACE` | `168h` (0 keeps unused pages) |
| `--stale-after` | `RADIX_STALE_AFTER` | `26h` |
| `--log-format`, `--log-level`, `--log-file` | `RADIX_LOG_FORMAT`, `RADIX_LOG_LEVEL`, `RADIX_LOG_FILE` | `json` for `run` (else `text`), `info`, none |

### HTTP endpoints

| Path | Purpose |
|---|---|
| `GET /snapshot/catalog.db` | current snapshot. `ETag` is a hash of the content; send `If-None-Match` and get `304` while nothing changed. `503` + `Retry-After` until the first export. |
| `GET /snapshot/current.json` | `{"file","etag","bytes","exported_at"}` |
| `GET /healthz` | `200 {"status":"ok"}` or `503 {"status":"unhealthy","problems":[…]}` |
| `GET /status` | everything: last cycle with every stage, last success, failed cycles in a row, next cycle, snapshot, error/warning counters and the 20 most recent warnings and errors |

Unhealthy means: no snapshot to serve, or the last two cycles failed, or no successful cycle
for `--stale-after`.

## 2. Logging

One line per event on stderr (and in `--log-file` if set). With `--log-format json`:

```json
{"time":"2026-09-19T18:28:16+02:00","level":"INFO","msg":"tree crawl finished","component":"crawl","source":"qis_tree","event":"crawl.finished","pages_walked":2652,"fetched":152,"changed":152,"from_archive":2500,"not_found":0,"failed":0,"unreachable":0,"duration_s":175,"aborted":false}
```

**Levels are a contract.** `ERROR`: something needs a human, or published data could not be
updated. `WARN`: the source data has a problem, or something failed and recovered by itself.
`INFO`: the story of a run. `DEBUG`: health checks, `304` answers, build steps.

| Level | `event` | Meaning |
|---|---|---|
| ERROR | `crawl.job_failed` | a page was given up after 3 attempts (`key`, `url`, `error`) |
| ERROR | `crawl.aborted` | 10 pages failed in a row; the stage stopped instead of hammering a failing server |
| ERROR | `crawl.archive_error` | the archive database could not be read |
| ERROR | `stage.failed` | a stage of a cycle failed (`stage`, `error`) |
| ERROR | `build.failed` | the build was rolled back; the canonical data is unchanged |
| ERROR | `validate.check_failed` | one failed invariant or baseline (`check`, `value`, `samples`). A baseline below its minimum usually means a page layout changed and a parser no longer recognises a label. |
| ERROR | `validate.finished` with `failed > 0` | no snapshot is published from this data |
| ERROR | `export.failed` | the previous snapshot stays current |
| ERROR | `cycle.finished` with `result=failed`, `cycle.panic` | the cycle failed / crashed (with stack) |
| ERROR | `http.failed`, `service.fatal` | the HTTP endpoint died; the process exits with 1 so the supervisor restarts it |
| ERROR | `http.request` with `status >= 500` | |
| WARN | `crawl.retry`, `crawl.slow` | a request failed and is retried / took longer than 15 s |
| WARN | `crawl.not_found` | a listed page answers 404 |
| WARN | `cycle.finished` with `result=degraded` | crawl problems; published data intact |
| WARN | `validate.check_warned` | e.g. kind conflicts between sources, programs without tree modules |
| WARN | `build.unresolved_refs`, `build.tree_leaves_without_module`, `build.tree_pages_missing`, `build.unlisted_module_pages`, `build.unreachable_tree_pages`, `build.modules_without_page`, `build.plans_without_program`, `build.plan_entries_unknown_module`, `build.unpaired_departments` | source data the build could not use, with counts and examples |
| WARN | `build.rooms_unknown_building`, `build.room_short_collisions`, `build.abbrev_overrides_unused` | short names (`docs/schema-v2.md`, „Short names"): an event room names a building the table in `internal/normalize/rooms.go` lacks (it keeps the building's full name) / two rooms would share a short form (both keep their long form) / a line of `internal/abbrev/overrides.tsv` applies to no module (a module number the catalog lacks, a program without that module, a pattern that matches nothing an earlier line does not take). Each wants a line in the table or the file. `build.finished` counts `abbrev_fell_back`, `abbrev_twins` and `abbrev_changed` (pairs whose abbreviation moved since the last build: a new title anywhere can move forms in other programs). |
| WARN | `http.request` with `status` 4xx/503 | |
| ERROR | `scan.failed`, `scan.extraction_failed`, `scan.save_failed`, `statutes.download_failed` | study plan scan: cannot run / a document could not be read / a plan could not be stored (the previous plan is unchanged) / a PDF could not be downloaded |
| WARN | `scan.rejected`, `scan.gemini_disabled`, `statutes.blocked` | a plan failed validation and was not stored / no API key, deterministic reader only / a PDF is behind bot protection |
| INFO | `crawl.list_chunks_dropped` | the QIS module table got shorter; chunks behind its end were removed |
| INFO | `service.started`, `service.stopped`, `http.listening`, `db.migrated` | lifecycle |
| INFO | `cycle.started`, `cycle.finished`, `crawl.started`, `crawl.progress`, `crawl.finished`, `crawl.up_to_date`, `build.started`, `build.finished`, `validate.finished`, `export.finished`, `retention.pruned`, `retention.archive_pruned` | progress, with counts and durations |

CLI commands use the same log and these exit codes: `0` success, `1` failure (pages failed,
validation failed, …), `2` invalid flags, `130` interrupted.

### Notifications

Any of these works; they can be combined.

- **Health probe.** Point an uptime monitor (Uptime Kuma, healthchecks.io, …) at `/healthz`.
  It turns `503` when there is nothing to serve, when cycles keep failing, or when the
  service has been stuck for a day. This catches what logs cannot: a process that hangs.
- **Container health.** The Nix image has `HEALTHCHECK radix healthcheck`; `docker ps` shows
  `unhealthy`, and tools like Autoheal or a Docker event listener can notify or restart.
- **Log level.** Alert on `level=ERROR`:
  `journalctl -u betula-radix -o cat -f | jq -c 'select(.level=="ERROR")'`, or a Loki/Promtail
  rule `{unit="betula-radix"} | json | level="ERROR"`, or Docker's logging driver of choice.
- **Specific events.** `validate.check_failed` (a parser probably broke) and `crawl.aborted`
  (the university's server is down or blocks us) deserve their own alert text.
- **`/status`** shows the most recent warnings and errors without access to the log stream.

## 3. Secrets

The only credential is the Gemini API key (`gemini-api-key`), and only `scan-curriculum` needs
it. It is never read from a command line flag or a configuration file, and it is redacted from
errors and audit files. `internal/secrets` looks for it in this order; the first source that is
configured wins, and a configured but unreadable source is an error (no silent fallback):

| # | Source | For |
|---|---|---|
| 1 | file named by `GEMINI_API_KEY_FILE` | Kubernetes secrets, sops-nix, agenix, a Docker secret under another name |
| 2 | `/run/secrets/gemini-api-key` (or `gemini_api_key`) | **Docker Swarm / Compose secrets**, found without any configuration |
| 3 | `$CREDENTIALS_DIRECTORY/gemini-api-key` | systemd `LoadCredential=` / `LoadCredentialEncrypted=` |
| 4 | `GEMINI_API_KEY` | CI, a one-off shell, and **development: a git-ignored `.env` file** |
| 5 | operating system credential store | developer machine: Windows Credential Manager, macOS Keychain, Secret Service |

```bash
radix secret set gemini-api-key       # hidden prompt, or: some-vault read … | radix secret set gemini-api-key
radix secret status                   # where each secret is found; never prints it
radix secret delete gemini-api-key
radix secret migrate-config config.yaml   # one-time: move the key out of a v1 config file
```

### Development: `.env`

Copy `.env.example` to `.env` and put the key there. The file is git-ignored and is read from the
working directory at startup (`RADIX_ENV_FILE` names another file). It can hold any `RADIX_*` setting
as well. A variable that is already set in the real environment always wins, so a stray `.env`
cannot change a deployment; `radix secret status` says when a key comes from the file. Do not
ship a `.env` with an image; production uses one of the sources above.

### Docker Swarm

Swarm keeps a secret encrypted in its Raft log and mounts it as a tmpfs file only into the
containers of the services that list it. Nothing else has to be configured:

```bash
docker secret create gemini-api-key -        # paste the key, Ctrl-D; or: < key-file
docker stack deploy -c docker-stack.yml betula
```

```yaml
# docker-stack.yml
services:
  radix:
    image: registry.example.org/betula-radix:latest   # nix build .#radix-image, docker load, tag, push
    secrets: [gemini-api-key]
    volumes: [radix-data:/data]
    networks: [internal]          # Folia reaches http://radix:8090/snapshot/catalog.db
    deploy:
      replicas: 1                 # one writer per database
      restart_policy: { condition: any, delay: 30s }
      update_config: { order: stop-first }   # never two writers on the volume
      resources: { limits: { memory: 1G } }
secrets:
  gemini-api-key: { external: true }
volumes:
  radix-data: {}
networks:
  internal: {}
```

The image's `HEALTHCHECK` makes Swarm restart a container that turns unhealthy. To rotate the
key: create `gemini-api-key-v2`, change the service to
`secrets: [{ source: gemini-api-key-v2, target: gemini-api-key }]`, deploy, remove the old secret.
The service itself does not need the key; it is used when you run
`docker exec <container> /bin/radix scan-curriculum …` (the statutes directory is
`RADIX_STATUTES_DIR`, default `statutes` below the working directory `/data`).

## 4. Deployment

### Container (built with Nix)

```bash
nix build .#radix-image            # result → docker image tarball
docker load < result
docker run -d --name betula-radix -p 8090:8090 -v radix-data:/data betula-radix:latest
```

The image contains the static `radix` binary and CA certificates, nothing else. `/data` holds
`radix.db` and `snapshot/`. Defaults inside the image: `RADIX_ADDR=0.0.0.0:8090`,
`RADIX_LOG_FORMAT=json`, `TZ=Europe/Berlin`. `nix build .#radix` builds only the binary;
`nix develop` gives a shell with Go.

After changing `go.mod`/`go.sum`, set `vendorHash = pkgs.lib.fakeHash;` in `flake.nix`, build
once, and copy the hash from the error message.

### systemd

```ini
[Unit]
Description=Radix, the collector of Betula
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=/usr/local/bin/radix run
Environment=RADIX_DB=/var/lib/betula-radix/radix.db
Environment=RADIX_SNAPSHOT_DIR=/var/lib/betula-radix/snapshot
Environment=RADIX_ADDR=127.0.0.1:8090
Environment=TZ=Europe/Berlin
StateDirectory=betula-radix
DynamicUser=yes
Restart=on-failure
RestartSec=30

[Install]
WantedBy=multi-user.target
```

The process stops cleanly on SIGTERM. JSON lines go to the journal.

### One-off commands

`crawl-modules`, `crawl-tree`, `crawl-events`, `prune`, `build`, `validate`, `export`,
`serve-snapshot`, `download-statutes`, `scan-curriculum`, `relink-plans` run one by one against
the same database. They can run next to a service: readers never block, and a writer waits up to
60 s for the other writer (a build holds the write lock for about 20 s).

`relink-plans` matches the stored plans against the catalog again and rewrites only the link from
a plan row to a module, from the name and code the scan stored. That is what a change to the
matching needs: re-reading the PDFs would extract everything again, spend a Gemini call per
program and change rows the change never touched. `--dry-run` reports the counts first; `new`,
`moved` and `cleared` say what a run would do, and a `moved` or `cleared` row is logged with its
program, so a rule that loses a link is visible before it is written. Run `build` afterwards:
`in_plan`, the kind of a membership and the plan semester are derived from the links.

**After a release with a migration**, an instance that does not crawl (`RADIX_CRAWL=off`) has to
be built by hand: `radix build`, then `radix validate` and `radix export`. The new binary migrates
`radix.db` when it opens it, but a migration does not rewrite the data it adds columns for; schema 9,
for instance, leaves `room_short` NULL and the abbreviation tables empty until the next build, and
`validate` — which `export` runs first — refuses such a database. Do not export it with
`--skip-validate`: Folia would get a catalog without short names. `deploy/vps/50-app.sh` does the
build itself when it makes the first snapshot of a seeded volume; for a running instance it is
`docker exec <radix container> /bin/radix build --db /data/radix.db`, then `… export --db
/data/radix.db --out /data/snapshot` (`deploy/README.md`, blue-green). An older binary refuses a
database newer than itself, so a rollback needs the copy of `radix.db` from before the release.
