# Operating the scraper service

The scraper runs as one long-lived process. It keeps the raw archive fresh at a polite pace,
rebuilds the catalog, publishes a new snapshot when the content changed, and serves the
snapshots over HTTP. The web server is an HTTP client of it; they share no files.

## 1. Running it

```bash
scraper run --addr 127.0.0.1:8090          # Windows, Linux, macOS: same binary, same flags
scraper run --once                          # one cycle, then exit (exit code 1 if it failed)
```

One cycle, every `--interval` (30 min):

| Stage | When | What |
|---|---|---|
| `lists` | every cycle, if older than 12 h | module catalog list, FÜS list (one request each) |
| `modules` | off-peak only | module pages older than `--module-max-age` (7 d), oldest first, at most 400 per cycle |
| `tree` | off-peak only | QIS program tree; pages older than `--tree-max-age` (7 d), at most 300 requests per cycle; discovers new programs and PO versions |
| `events` | off-peak only | QIS pages of the events module pages link; older than `--event-max-age` (3 d), at most 600 per cycle |
| `retention` | every cycle | removes events `--event-retention` (30 d) after their last date and remembers them, so they are not fetched again while a module page still links them |
| `build` | every cycle | raw archive → canonical tables, one transaction, about 20 s |
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
| `--db` | `BTU_DB` | `btu_scraper.db` |
| `--snapshot-dir` | `BTU_SNAPSHOT_DIR` | `snapshot` |
| `--addr` | `BTU_ADDR` | `127.0.0.1:8090` |
| `--interval` | `BTU_INTERVAL` | `30m` |
| `--offpeak` | `BTU_OFFPEAK` | `1-6` |
| `--module-delay`, `--qis-delay` (ms) | `BTU_MODULE_DELAY_MS`, `BTU_QIS_DELAY_MS` | `500`, `500` (tree: twice the QIS delay) |
| `--module-max-age`, `--event-max-age`, `--tree-max-age` | `BTU_MODULE_MAX_AGE`, `BTU_EVENT_MAX_AGE`, `BTU_TREE_MAX_AGE` | `168h`, `72h`, `168h` |
| `--event-retention` | `BTU_EVENT_RETENTION` | `720h` (0 keeps everything) |
| `--stale-after` | `BTU_STALE_AFTER` | `26h` |
| `--log-format`, `--log-level`, `--log-file` | `BTU_LOG_FORMAT`, `BTU_LOG_LEVEL`, `BTU_LOG_FILE` | `json` for `run` (else `text`), `info`, none |

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
| WARN | `build.unresolved_refs`, `build.tree_leaves_without_module`, `build.tree_pages_missing`, `build.modules_without_page`, `build.plans_without_program`, `build.plan_entries_unknown_module`, `build.unpaired_departments` | source data the build could not use, with counts and examples |
| WARN | `http.request` with `status` 4xx/503 | |
| ERROR | `scan.failed`, `scan.extraction_failed`, `scan.save_failed`, `statutes.download_failed` | study plan scan: cannot run / a document could not be read / a plan could not be stored (the previous plan is unchanged) / a PDF could not be downloaded |
| WARN | `scan.rejected`, `scan.gemini_disabled`, `statutes.blocked` | a plan failed validation and was not stored / no API key, deterministic reader only / a PDF is behind bot protection |
| INFO | `service.started`, `service.stopped`, `http.listening`, `db.migrated` | lifecycle |
| INFO | `cycle.started`, `cycle.finished`, `crawl.started`, `crawl.progress`, `crawl.finished`, `crawl.up_to_date`, `build.started`, `build.finished`, `validate.finished`, `export.finished`, `retention.pruned` | progress, with counts and durations |

CLI commands use the same log and these exit codes: `0` success, `1` failure (pages failed,
validation failed, …), `2` invalid flags, `130` interrupted.

### Notifications

Any of these works; they can be combined.

- **Health probe.** Point an uptime monitor (Uptime Kuma, healthchecks.io, …) at `/healthz`.
  It turns `503` when there is nothing to serve, when cycles keep failing, or when the
  service has been stuck for a day. This catches what logs cannot: a process that hangs.
- **Container health.** The Nix image has `HEALTHCHECK scraper healthcheck`; `docker ps` shows
  `unhealthy`, and tools like Autoheal or a Docker event listener can notify or restart.
- **Log level.** Alert on `level=ERROR`:
  `journalctl -u btu-scraper -o cat -f | jq -c 'select(.level=="ERROR")'`, or a Loki/Promtail
  rule `{unit="btu-scraper"} | json | level="ERROR"`, or Docker's logging driver of choice.
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
| 4 | `GEMINI_API_KEY` | CI, a one-off shell |
| 5 | operating system credential store | developer machine: Windows Credential Manager, macOS Keychain, Secret Service |

```bash
scraper secret set gemini-api-key       # hidden prompt, or: some-vault read … | scraper secret set gemini-api-key
scraper secret status                   # where each secret is found; never prints it
scraper secret delete gemini-api-key
scraper secret migrate-config config.yaml   # one-time: move the key out of a v1 config file
```

### Docker Swarm

Swarm keeps a secret encrypted in its Raft log and mounts it as a tmpfs file only into the
containers of the services that list it. Nothing else has to be configured:

```bash
docker secret create gemini-api-key -        # paste the key, Ctrl-D; or: < key-file
docker stack deploy -c docker-stack.yml btu
```

```yaml
# docker-stack.yml
services:
  scraper:
    image: registry.example.org/btu-scraper:latest   # nix build .#container, docker load, tag, push
    secrets: [gemini-api-key]
    volumes: [btu-data:/data]
    networks: [internal]          # the web server reaches http://scraper:8090/snapshot/catalog.db
    deploy:
      replicas: 1                 # one writer per database
      restart_policy: { condition: any, delay: 30s }
      update_config: { order: stop-first }   # never two writers on the volume
      resources: { limits: { memory: 1G } }
secrets:
  gemini-api-key: { external: true }
volumes:
  btu-data: {}
networks:
  internal: {}
```

The image's `HEALTHCHECK` makes Swarm restart a container that turns unhealthy. To rotate the
key: create `gemini-api-key-v2`, change the service to
`secrets: [{ source: gemini-api-key-v2, target: gemini-api-key }]`, deploy, remove the old secret.
The service itself does not need the key; it is used when you run
`docker exec <container> /bin/scraper scan-curriculum …` (the statutes directory is
`BTU_STATUTES_DIR`, default `statutes` below the working directory `/data`).

## 4. Deployment

### Container (built with Nix)

```bash
nix build .#container            # result → docker image tarball
docker load < result
docker run -d --name btu-scraper -p 8090:8090 -v btu-data:/data btu-scraper:latest
```

The image contains the static `scraper` binary and CA certificates, nothing else. `/data` holds
`btu_scraper.db` and `snapshot/`. Defaults inside the image: `BTU_ADDR=0.0.0.0:8090`,
`BTU_LOG_FORMAT=json`, `TZ=Europe/Berlin`. `nix build .#scraper` builds only the binary;
`nix develop` gives a shell with Go.

After changing `go.mod`/`go.sum`, set `vendorHash = pkgs.lib.fakeHash;` in `flake.nix`, build
once, and copy the hash from the error message.

### systemd

```ini
[Unit]
Description=BTU catalog scraper
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=/usr/local/bin/scraper run
Environment=BTU_DB=/var/lib/btu-scraper/btu_scraper.db
Environment=BTU_SNAPSHOT_DIR=/var/lib/btu-scraper/snapshot
Environment=BTU_ADDR=127.0.0.1:8090
Environment=TZ=Europe/Berlin
StateDirectory=btu-scraper
DynamicUser=yes
Restart=on-failure
RestartSec=30

[Install]
WantedBy=multi-user.target
```

The process stops cleanly on SIGTERM. JSON lines go to the journal.

### One-off commands

`crawl-modules`, `crawl-tree`, `crawl-events`, `prune-events`, `build`, `validate`, `export`,
`serve-snapshot`, `download-statutes`, `scan-curriculum` run one by one against the same database.
They can run next to a service: readers never block, and a writer waits up to 60 s for the
other writer (a build holds the write lock for about 20 s).
