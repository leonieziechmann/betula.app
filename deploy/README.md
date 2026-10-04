# Betula server runbook

Everything the server `betula.app` (77.237.233.41, one Contabo VPS, Ubuntu 26.04, one-node Docker
Swarm) needs lives in this directory. It is copied verbatim to `/opt/betula` on the server; nothing
is edited there. No password, token or key is ever part of these files.

```
deploy/
  sync.sh                 workstation -> /opt/betula (tar over ssh; refuses CR line endings)
  ship.sh                 workstation: build the images of a commit, load them on the server, deploy an instance
  ship-models.sh          workstation: the models of models.lock into the server's model store (section 13)
  ship-cortex.sh          workstation: build the Cortex image of a commit, load it on the server, 48-cortex.sh (section 14)
  models.lock             the two models of the semantic search, pinned by sha256 (section 13)
  vps/                    host scripts, run on the server, in the order of their numbers
    10-base.sh            upgrade, user deploy, ufw, fail2ban, unattended-upgrades, journald, sysctl, swap
    20-ssh-lockdown.sh    key-only ssh for deploy, root login off (with automatic rollback until --confirm)
    30-docker.sh          Docker Engine 29, swarm, overlay networks edge + monitoring + cortex (internal), published-port filter
    40-stacks.sh          swarm secrets, stacks edge -> placeholder -> monitoring, waits for convergence
    45-seed.sh            once per instance, before its first deploy: a Radix database (tar on stdin) into its volume
    48-cortex.sh          Cortex (stacks/cortex.yml) at a release tag, one instance at a time (section 14)
    50-app.sh             one instance of the application (stacks/betula.yml + stacks/<instance>.env) at a release tag
    55-switch.sh          blue-green: hand a host name to the other of its two instances (the rollback is the same)
    60-canary.sh          canary follows master: sqlite3, the timer, the GitHub token (section 12)
    canary-agent.sh       what that timer runs: fetch a build of master, seed it with the public site's data, switch
    models.sh             the model store: status, missing, receive (what ship-models.sh sends), prune
    cortex-seed.sh        give Cortex an instance's archive (radix seed-cortex on a copy of its radix.db; section 14)
    90-verify-host.sh     PASS/WARN/FAIL audit of 10-30      91-verify-stacks.sh  the same for 40 to 60
    files/                config payloads the host scripts install;  lib*.sh, sync-receive.sh  helpers
  stacks/                 edge(.www).yml, placeholder.yml, monitoring(.public|.smtp).yml, cortex.yml,
                          betula(.gemini|.offline|.models|.cortex|.cortex-offline|.egress).yml,
                          betula.env, canary(-green).env, monitoring.notify.example.yml, monitoring-secrets.sh
  config/                 bind-mounted read-only into the services: traefik/ placeholder/ monitoring/ cortex/ (hosts.json)
```

All host scripts are idempotent and non-interactive; every one can simply be run again. Root scripts
are run with `sudo bash ...`; `40-stacks.sh`, `48-cortex.sh`, `50-app.sh` and `91-verify-stacks.sh`
run as `deploy` **without** sudo (sudo would drop the environment variables that select staging
certificates, the Grafana host or an instance's values).

## 0. Workstation

Two host aliases in `~/.ssh/config`, both with the dedicated key and `IdentitiesOnly yes` (after the
lockdown sshd allows three authentication attempts; an agent that offers other keys first never
reaches the right one):

```
Host betula-root            Host betula
  HostName betula.app         HostName betula.app
  User root                   User deploy
  IdentityFile ~/.ssh/<key>   IdentityFile ~/.ssh/<key>
  IdentitiesOnly yes          IdentitiesOnly yes
```

`sync.sh` takes the alias from `SSH_TARGET` (default `deploy@betula.app` with your default keys);
`SSH_OPTS` is passed to ssh unchanged. Nothing here runs `ssh-keyscan`: accept the host key once by
hand (`ssh betula-root true`) and compare it with the provider's console if in doubt.

## 1. First bootstrap

Run from the repository root in Git Bash. Steps 2 to 4 belong together: do not stop in between.

| # | Where | Command | Expect |
|---|---|---|---|
| 1 | workstation | `SSH_TARGET=betula-root ROOT_BOOTSTRAP=1 bash deploy/sync.sh` | `... new, 0 updated ...` |
| 2 | workstation | `ssh betula-root bash /opt/betula/vps/10-base.sh` | ends with `REBOOT_REQUIRED` (a kernel is pending; the reboot is step 6) |
| 3 | workstation | **manual proof:** `ssh betula sudo -n true` | no output, exit code 0 |
| 4a | workstation | `ssh betula sudo bash /opt/betula/vps/20-ssh-lockdown.sh` | ends with `LOCKDOWN_CONFIRM_REQUIRED` |
| 4b | workstation, **new** connection, within 10 minutes | `ssh -o ControlPath=none betula sudo bash /opt/betula/vps/20-ssh-lockdown.sh --confirm` | `lockdown confirmed` |
| 4c | workstation | `ssh betula-root true` | `Permission denied` |
| 5 | workstation | `ssh betula sudo bash /opt/betula/vps/30-docker.sh` | `swarm initialised`, `network edge created`, `network monitoring created`, `network cortex created` (internal), no `FATAL` |
| 6 | workstation | `ssh betula sudo systemctl reboot`, wait a minute, then `ssh betula true` | login works again |
| 7 | workstation | `ssh betula sudo bash /opt/betula/vps/90-verify-host.sh` | `fail=0` |
| 8 | workstation | `ssh betula bash /opt/betula/vps/40-stacks.sh` | `converged` three times |
| 9 | workstation | `ssh betula bash /opt/betula/vps/91-verify-stacks.sh` | `fail=0` |
| 10 | workstation | `curl -sI http://betula.app/` and `curl -sI https://betula.app/` | `301` to https, then `200` with `strict-transport-security` |

Cortex and the application come after this: Cortex first (section 14, „Setting it up, once"), then
each instance (section 4).

Why this order, and what can go wrong:

- **Step 3 is the point of the whole exercise.** `10-base.sh` copies root's `authorized_keys` to
  `deploy`; step 3 proves from the outside that the key, the alias and passwordless sudo really work
  *before* root and passwords are locked out. `20-ssh-lockdown.sh` does not take your word for it:
  it looks for `Accepted publickey for deploy` in the journal and refuses without it.
- **Step 4b is a dead-man's switch.** The lockdown arms a timer that undoes itself after 10 minutes
  (`LOCKDOWN_ROLLBACK_MINUTES`). Only a login that happened *after* the change can confirm it, so a
  client that the new settings lock out (wrong key offered first) gets its access back by waiting.
  Do not reboot between 4a and 4b: a reboot drops the timer and keeps the lockdown. If 4b fails,
  wait for the rollback, fix the client, repeat 4a.
- **The reboot comes after Docker on purpose**: step 7 then audits the state a boot really produces
  (ufw before dockerd, the published-port filter as `ExecStartPre`, swap from fstab, ufw's own
  sysctl file) and the stacks start on the new kernel. While the server is down, poll with
  `ssh betula true` every 10-15 s, not with port probes: connections that never authenticate
  collect sshd penalties (`PerSourcePenalties`), failed connects do not.
- **Before step 8, a dry run that changes nothing:** `docker stack config` parses, merges and
  substitutes the stack files exactly like `stack deploy` and rejects what swarm would reject
  (unknown keys, bad `${VAR}` syntax). Nothing is printed when all is well:
  `ssh betula 'cd /opt/betula/stacks && for f in edge placeholder monitoring; do docker stack config -c $f.yml >/dev/null || echo "BROKEN: $f"; done'`
- **Step 8 in two stages, if you like:** `ssh betula ACME_STAGING=1 bash /opt/betula/vps/40-stacks.sh`
  first. Staging certificates are worthless for browsers (`.app` is HSTS-preloaded, there is no
  click-through) but prove DNS, port 80 and ACME without touching the production rate limits
  (5 failed validations per host name per hour). `91-verify-stacks.sh` then reports WARN for the
  certificates. Run step 8 again without the variable for production; the stores are separate.
- `40-stacks.sh` checks DNS before it lets Traefik order anything: `edge.www.yml` and
  `monitoring.public.yml` are only deployed while `www.betula.app` / `grafana.betula.app` resolve to
  this machine (they do today). An empty host keeps Grafana off the internet, on every run:
  `ssh betula GRAFANA_HOST= bash /opt/betula/vps/40-stacks.sh`.
  **Do not create AAAA records** for any of the names: Docker has no IPv6 network here, IPv6
  connections reach Traefik through docker-proxy, and every IPv6 visitor would share one client
  address (access log, rate limits). Ports 80/443 are open on IPv6 all the same; only scanners use them.
- A certificate order that failed is not retried by Traefik on its own. Fix the cause, then
  `docker service update --force edge_traefik` (one attempt per missing name; stay below 5 per hour).

## 2. Grafana

- Public (default): <https://grafana.betula.app>, user `betula`. The initial password was generated on
  the server and is in **`/root/betula-initial-credentials.txt`** (root only, mode 0600; `40-stacks.sh`
  prints only this path). Read it once with `ssh betula sudo cat /root/betula-initial-credentials.txt`,
  store it in your password manager, set your own password, then delete the file:
  `<password manager CLI> | ssh betula /opt/betula/stacks/monitoring-secrets.sh reset-admin-password`
  and `ssh betula sudo rm /root/betula-initial-credentials.txt`.
  (The swarm secret is only the *initial* password; Grafana reads it when it creates its database.)
- Basic auth is off: automation against Grafana's API needs a service account token.
- Without the public router, or when Traefik is the broken part, use a tunnel (sshd allows `-L`):
  ```
  ssh betula 'cid=$(docker ps -q --no-trunc -f name=monitoring_grafana | head -n 1); docker network inspect docker_gwbridge -f "{{(index .Containers \"$cid\").IPv4Address}}"'
  ssh -N -L 3000:<that address without /24>:3000 betula        # then open http://localhost:3000
  ```
  The same recipe reaches Prometheus (9090). Radix and Cortex have no address on `docker_gwbridge`
  (their networks are internal); their `/status` is read from inside:
  `docker exec "$(docker ps -q -f name=monitoring_prometheus | head -n 1)" wget -qO- http://<stack>_radix.:8090/status`
  and `docker exec "$(docker ps -q -f name=cortex_a | head -n 1)" /bin/cortex status`.
- Dashboards and alert rules are files (`config/monitoring/grafana/`); the UI refuses to save them.
  Edit, export JSON, commit, sync, `40-stacks.sh monitoring`.
- The dashboard "Radix" shows what the collector does, from its `GET /metrics` (Prometheus job `radix`):
  what it asked for by endpoint and status (to Cortex, or to the university without it: tile
  "Fetches through"), and, from Cortex's metrics, how much of that Cortex answered from its store
  and what reached b-tu.de, QIS and OPUS by host, status and answer time, and which of these hosts
  Cortex paused; then pages that changed, the archive per endpoint (newest and oldest fetch),
  cycles and stages, builds and snapshots, warnings and errors by event. Cortex serves every
  instance, so its panels do not follow the Instance filter.
  Prometheus reaches every instance's Radix over the `cortex` overlay (`stacks/betula.yml`; Radix
  is no longer on `monitoring`); a new instance file needs its `tasks.<stack>_radix` line in
  `config/monitoring/prometheus.yml`. An image from before `/metrics` answers 404 there, so the
  rule "Monitoring target is down" fires until a current one runs.
- The dashboard "Cortex" (`betula-cortex.json`, Prometheus job `cortex`: `cortex_a:8100` and
  `cortex_b:8100` over the `cortex` overlay) shows which instance leads, the follower's lag and
  missing blobs (`cortex_blobs_missing`), every host's queue, requests in flight and breaker, the clients' requests by
  source and result, what went upstream by host and status, the store, the answers it was given
  by `radix seed-cortex` (`cortex_imports_total`), what clients that read its store alone (the
  canary, `RADIX_CRAWL=cortex-offline`) got from it and what it had not stored, and warnings and
  errors by event (`docs/cortex/cortex.md` §10). The dashboard "Radix" names such an instance's
  mode "from Cortex, offline" and its way "Cortex's store (offline)"; a page Cortex had not stored
  is `offline_miss` there, skipped and not failed.
- The dashboard "Visitors" reads stored numbers only: Loki's ruler counts them from Traefik's access
  log every 5 minutes (the 7-day numbers and the calendar subscriptions once an hour) with the rules
  in `config/monitoring/loki-rules`, and writes them to Prometheus, which keeps them like every
  metric (15 days, `config/monitoring/prometheus.yml`). Its charts begin with the first count; there
  is no history from before.

## 3. Changing something later

```
edit below deploy/  ->  SSH_TARGET=betula bash deploy/sync.sh  ->  the script that owns the file  ->  verify
```

| Changed | Then run on the server |
|---|---|
| `stacks/*`, `config/*` | `bash /opt/betula/vps/40-stacks.sh [edge\|placeholder\|monitoring]`, then `91-verify-stacks.sh` |
| `stacks/betula*.yml`, `stacks/<instance>.env` | `bash /opt/betula/vps/50-app.sh <instance>` (section 4) |
| `stacks/cortex.yml` | `bash /opt/betula/vps/48-cortex.sh` (the release that runs, one instance at a time; section 14), then `91-verify-stacks.sh cortex` |
| `config/cortex/hosts.json` | nothing: both instances read it again within 30 s (log event `policy.loaded`; a broken file keeps the last good policy, `policy.invalid`) |
| `config/traefik/dynamic/*` | nothing: Traefik watches the directory (check `docker service logs edge_traefik`) |
| `vps/files/*`, `vps/10-base.sh` ... `30-docker.sh` | the numbered script again with `sudo bash`, then `90-verify-host.sh` |
| `vps/files/public-ports.conf` | `10-base.sh` **and** `30-docker.sh` |

`sync.sh` replaces only files whose content changed, removes what no longer exists in `deploy/`
(each removal is printed; `SYNC_PRUNE=0` disables it) and restarts nothing. `40-stacks.sh` exports a
checksum of `config/monitoring` (and of the placeholder's `nginx.conf`), so services restart exactly
when their bind-mounted configuration changed.

## 4. Shipping the application (no registry)

The application (Radix + Folia) is one stack file, `stacks/betula.yml`, for every **instance** of it.
An instance is a file `stacks/<instance>.env`: the name of its stack, its public host name, and
whether the site asks for the password of closed testing. `canary.env` is the closed test at
https://canary.betula.app; `betula.env` and `betula-green.env` are the two colours of the public
site at https://betula.app (since 2026-09-27; one of them crawls, see „A colour that crawls"
below). The placeholder kept https://betula.app until then; the routers of both outrank its
priority 1, so there was no gap, and it answers only while neither has a healthy web server
(`docker stack rm placeholder` removes it). A new instance
also has to be named in the two log rules of
`config/monitoring/grafana/provisioning/alerting/rules.yml` (`stack=~"(betula|canary)(-green)?|cortex"`),
or its errors stay silent.

Once per server: the DNS record of the instance's name, and the password of closed testing as a
swarm secret - the value travels on stdin, never in argv (`docs/folia/frontend.md`, "Closed testing"):

```bash
<password manager CLI> | ssh betula docker secret create folia-access-password -
```

Optional: `gemini-api-key`, created the same way. `radix scan-curriculum` and the summaries of the
semantic search use it; `50-app.sh` adds `stacks/betula.gemini.yml` while the secret exists. Gemini
does not go through Cortex, so while Cortex runs the secret is also what gives a crawling Radix a
way to the internet (`stacks/betula.egress.yml`, section 14).

**Through Cortex.** While the stack `cortex` runs (section 14), `50-app.sh` adds
`stacks/betula.cortex.yml`: Radix fetches through Cortex (`RADIX_CORTEX_URL`, `docs/radix/operations.md`
§1), and its networks are all internal. A crawling colour gets a way out of its own
(`stacks/betula.egress.yml`) only while Cortex does not run, while the secret `gemini-api-key`
exists, or while its release is from before Cortex (`50-app.sh` asks the image, `radix run -h`,
and says so with a WARN). Afterwards it checks what swarm was told: Radix on `cortex` and
`<stack>_snapshot`, the egress network exactly when it was added, every other network internal.

Every release, from the repository root on the workstation (Git Bash on Windows; Nix runs in the WSL
distribution `NixOS`, or on the PATH under Linux):

```bash
SSH_TARGET=betula bash deploy/ship.sh canary
```

The **first** deploy of an instance has to say where its data comes from:

```bash
SSH_TARGET=betula bash deploy/ship.sh canary --seed
```

`--seed` uploads the workstation's `radix.db` (`SEED_DB` names another one; no Radix may be
writing to it) into the instance's volume before anything starts (`vps/45-seed.sh`), so the server
does not crawl again what was crawled here: Radix builds and exports a snapshot from it within
minutes and then only keeps it fresh. `--no-seed` starts empty: Radix crawls everything itself,
politely, which takes hours and thousands of requests to the university's servers; until the first
snapshot the site says that the catalog is not available yet (`/healthz` 503, `/livez` 200). The
database has to fit the release (a database that a newer Radix migrated is not for an older
image), one more reason to ship the commit that wrote it. `45-seed.sh` refuses an instance that
is deployed or has a database already; starting over on purpose is `docker stack rm <instance>`,
`docker volume rm <instance>_radix-data`, then `--seed` again.

It ships the **commit** `HEAD` (uncommitted changes to what the images are built from stop it;
`SHIP_WORKTREE=1` ships the working tree as it is, tagged `...-wip-<time>`): `nix build
.#radix-image .#folia-image` from a `git archive`, each image through ssh into `docker load`, the tag
`<date of the commit>-<short hash>` given on the server, `sync.sh`, then on the server
`vps/50-app.sh canary <tag>` (checks images, DNS and secrets, deploys, waits for convergence) and
`vps/91-verify-stacks.sh services app`. Shipping a commit a second time changes nothing.
`bash deploy/ship.sh canary --build-only` builds the images and sends nothing anywhere.

**Radix offline through Cortex** (`RADIX_CRAWL=cortex-offline` in the instance's file,
`stacks/betula.cortex.yml` plus `stacks/betula.cortex-offline.yml`; owner, 2026-10-04, for the
canary): Radix runs `radix run` as a colour that crawls does, its cycles at their times, build,
export and the semantic search included, but with `RADIX_CORTEX_MODE=offline`: every page comes
from Cortex's store, whatever its age, and a page Cortex has not stored waits for the next cycle.
Nothing reaches the university, Gemini is not asked, and Radix has no way to the internet. What
Cortex has stored is what a colour that crawls fetched through it and what `radix seed-cortex`
gave it (section 14). It needs a Cortex that runs and a release of Radix that knows
`--cortex-mode` (from 2026-10-04 on); without either, `50-app.sh` deploys the instance offline as
below and says so (a WARN, also in `91-verify-stacks.sh app`), so that a build of master without
it, or a Cortex that is down, still reaches canary. A seeded volume needs no snapshot made first:
the new release builds and exports at start (`service.rebuild`), within minutes.

**Radix offline** (`RADIX_CRAWL=off` in the instance's file, `stacks/betula.offline.yml`): Radix
is started as `radix serve-snapshot --db /data/radix.db` instead of `radix run`. It sends nothing
to the university's servers - no crawl, no cycle - and hands the snapshot it has to Folia, so the
data stays as it was fetched. A new release builds the catalog once more from the archived pages
at start (no network) and exports it if its parsers and rules make something else of them; the
old snapshot is served meanwhile and stays if that build or its validation fails (`level=ERROR`,
`docs/radix/operations.md` §1). `canary.env` says `off` for the time of the closed test (owner,
2026-09-21): the crawler's User-Agent names betula.app, and while that site shows only a login
page, requests in its name invite a block. `50-app.sh` makes sure there is a snapshot to serve (a
seeded volume that never ran gets one from its database: `radix build`, then `radix export`, in
containers without a network, so that a database older than the release is migrated and built) and
checks that swarm really starts `serve-snapshot`; `91-verify-stacks.sh app` checks the same and
reminds with a WARN that the data does not change. Back online: `RADIX_CRAWL=on`, sync,
`50-app.sh <instance>`; Radix then fetches what has aged in the meantime at its usual pace (one
request at a time with a pause after each, bulk only between 1 and 6 o'clock, a cap per source
and cycle: `docs/radix/operations.md` §1), through Cortex while it runs. `radix scan-curriculum` with
`docker exec` works in both modes; its Gemini enrichment only where Radix has a way out (a
crawling colour with the secret, section 14), else with the deterministic reader alone.

By hand, on the server: `bash /opt/betula/vps/50-app.sh canary <tag>` deploys a release that is
loaded already - that is also the **rollback** (`docker image ls 'betula-*'` lists what is there) -
and `bash /opt/betula/vps/50-app.sh canary` applies a change of `canary.env` or `betula.yml` to the
release that runs. Opening the site: `FOLIA_ACCESS_GATE=off` in `canary.env`, sync, `50-app.sh canary`.

Since `60-canary.sh` (section 12) the canary no longer needs any of this: every build of master
reaches it by itself, through the same scripts and the blue-green switch below, with a fresh copy of
the public site's data. What is described here stays the way to ship to canary by hand (the timer
off first: `sudo bash /opt/betula/vps/60-canary.sh off`) and the way to ship the public site.

**Blue-green** (owner, 2026-09-23): two instance files with the same `APP_HOST` are two colours of
one site, `canary.env` (the stack that ran first) and `canary-green.env`. Each is a stack of its
own with volumes of its own, and each has a router for the host. Traefik sends the host to the
router with the higher priority (a label of the web server's service, `vps/lib-stacks.sh`), and
only to a service whose task is healthy:

1. `SSH_TARGET=betula bash deploy/ship.sh canary-green --seed` (a later release without `--seed`)
   deploys the colour that does not serve as the **standby**: it runs, but gets no traffic.
   `50-app.sh` never moves traffic: a stack that runs keeps its priority, a new one next to a
   sibling starts below it. `91-verify-stacks.sh app` asks the standby directly, past Traefik.
2. `ssh betula bash /opt/betula/vps/55-switch.sh canary-green` checks the standby once more
   (converged, `/livez` and `/healthz` answered directly, closed testing in force, a catalog the
   build can read), raises its router above the other one (nothing restarts) and waits until
   Traefik's access log shows the host's requests arriving there.
3. The other colour keeps running. It is the **rollback** (`55-switch.sh canary`, seconds), and
   Traefik sends it the host's requests while the live one has no healthy task, so even an
   in-place update of the live colour leaves no gap. Once the new one has proven itself:
   `docker stack rm canary`; its volumes stay until you remove them. Grafana's rule „Service has
   no running container" leaves a colour removed this way alone, no silence needed.

Only with the same `FOLIA_ACCESS_GATE` in both files and `RADIX_CRAWL=on` in at most one of them
(two Radix that crawl would ask the university for everything twice); `50-app.sh` refuses
anything else, and before it deploys a colour that crawls it checks that the other one's Radix
really runs `serve-snapshot`, or reads Cortex's store alone (`cortex-offline`). Neither canary
colour crawls (both `cortex-offline` since 2026-10-04). The next release goes to the colour that
does not serve. Its volume keeps its database: for new data
remove its stack and volume and ship it with `--seed` again. A new release builds and exports a
new snapshot from that database by itself when its Radix starts (a new schema included: the
migration adds columns, the build fills them); wait for `export.finished` in `docker service logs
<instance>_radix` before `55-switch.sh`, which refuses a catalog the build cannot read. By hand
it is `docker exec <its radix container> /bin/radix build --db /data/radix.db`, then `... export
--db /data/radix.db --out /data/snapshot`. Do not reach for `--skip-validate` here.

**A colour that crawls** (the public site, owner 2026-09-27): the database goes with the crawl,
so that nothing Radix fetched is lost and nothing is fetched twice. `<old>` crawls and serves,
`<new>` is the colour that takes over (`betula`, `betula-green`):

1. `RADIX_CRAWL=off` in `<old>.env`, sync, `bash /opt/betula/vps/50-app.sh <old>`. Only Radix
   restarts, as `serve-snapshot` with the snapshot it exported last; Folia serves on. Radix stores
   every page as it arrives, so a stop costs at most the request in flight - none in the pause
   between two cycles (`docker service logs <old>_radix`: `cycle.finished`, then 30 minutes).
2. Its database, which nothing has open any more, to the workstation (a container that is never
   started, as in `50-app.sh`: the image has no shell):
   `ssh betula 'docker create --name radix-copy -v <old>_radix-data:/data:ro betula-radix:<its tag> >/dev/null && docker cp radix-copy:/data/radix.db -; docker rm -f radix-copy >/dev/null' > radix.db.tar`,
   then `tar -xf radix.db.tar` (a `radix.db-wal` next to it would have to come along; a clean
   stop leaves none).
3. `RADIX_CRAWL=off` in `<new>.env` for now, and `SEED_DB=<the copy> SSH_TARGET=betula bash
   deploy/ship.sh <new> --seed`: `50-app.sh` builds and exports a snapshot from it with the new
   release, without a network, and deploys the standby. Wait for Folia's `cache.warmed`. A
   colour that ran before still has its old database, and `45-seed.sh` refuses it: first
   `docker stack rm <new>` and `docker volume rm <new>_radix-data` (it is the standby, and the
   copy is the newer data).
4. `55-switch.sh <new>`.
5. `RADIX_CRAWL=on` in `<new>.env`, sync, `bash /opt/betula/vps/50-app.sh <new>`: its Radix
   crawls on from where the other one stopped (`50-app.sh` refuses while `<old>_radix` crawls).

Between 1 and 5 nobody crawls: what is due is fetched afterwards, nothing is lost. `<old>` stays
the rollback with the data of the handover; before it crawls again, the database goes back the
same way.

Swarm compares service definitions, not image contents: re-loading an existing tag restarts nothing,
hence a tag per commit and never `latest`. Old versions stay until you remove them
(`docker image rm ...`); the weekly prune timer only removes untagged images.

Study plans travel with a seeded database. On the server they come from `radix download-statutes`
and `radix scan-curriculum` (`docs/radix/operations.md`), run with `docker exec` in the Radix container
(the Gemini key is the optional secret above). `download-statutes` goes through Cortex wherever
the container has `RADIX_CORTEX_URL`, in every colour.

## 5. Secrets

Swarm secrets are immutable; rotation means a new name. Cortex needs none: its clients are not
authenticated (owner, 2026-10-02: „Offline ist wunsch des clients"), and only the internal
`cortex` network reaches it.

| Secret | Create / rotate |
|---|---|
| `grafana-admin-password` | created by `40-stacks.sh`. Later changes: `monitoring-secrets.sh reset-admin-password` (the secret itself stays) |
| `grafana-smtp-*`, `grafana-ntfy-url`, tokens | `... \| ssh betula /opt/betula/stacks/monitoring-secrets.sh set <name>`. Rotate: `set <name>-v2`, point `source:` in the override at it, sync, `40-stacks.sh monitoring`, `docker secret rm <name>` |
| GitHub token of the canary agent | not a swarm secret: a fine-grained token (section 12), stored encrypted with `60-canary.sh token`. Rotate: a new token, the same command (the old file is replaced), then revoke the old one on GitHub. It expires: 91-verify-stacks.sh warns 14 days ahead |
| `gemini-api-key` | `... \| ssh betula docker secret create gemini-api-key-v2 -`, in `betula.gemini.yml`: `- source: gemini-api-key-v2` / `target: gemini-api-key` (and the new name under `secrets:`), sync, `50-app.sh <instance>`, remove the old one |
| `folia-access-password` | the password testers get while `FOLIA_ACCESS_GATE` is on. Rotate like `gemini-api-key`, in `betula.yml` (`folia-access-password-v2`, `source:`/`target:`, sync, `50-app.sh <instance>`); a new password ends every tester's visit at once |
| ssh key of `deploy` | append the new public key to `/home/deploy/.ssh/authorized_keys`, test it in a new session, then remove the old line |
| root password | not touched by any script; it is the break-glass login on the provider's console. Keep it in the password manager |
| ACME account + certificates | volume `edge_acme` (`acme.json`). Losing it means ordering everything again (5 identical certificates per week) |

## 6. Updating pinned versions

Nothing uses `:latest`. Tags live in exactly one place each:

| Image | File |
|---|---|
| `traefik`, `wollomatic/socket-proxy` | `stacks/edge.yml` |
| `nginxinc/nginx-unprivileged` | `stacks/placeholder.yml` |
| `grafana/grafana`, `grafana/loki`, `prom/prometheus`, `grafana/alloy` | `stacks/monitoring.yml` |
| Docker Engine 29.x | `vps/files/apt-preferences-docker`, `DOCKER_MAJOR` in `30-docker.sh` and `90-verify-host.sh` |

Read the release notes (Traefik and Grafana: the migration notes), change the tag, sync,
`40-stacks.sh <stack>`, `91-verify-stacks.sh`. Rollback is the old tag. One image at a time.
Docker Hub allows only a few anonymous pulls per hour; `toomanyrequests` means wait, not retry.
The engine is on apt hold and **never upgrades by itself** (an upgrade restarts every container):
`ssh betula sudo env DOCKER_UPGRADE=1 bash /opt/betula/vps/30-docker.sh` in a quiet moment, every
month or two and after Docker security advisories. Ubuntu security updates install themselves.

## 7. Logs from the command line

```bash
docker stack services edge; docker service ps --no-trunc betula_radix       # state, and why a task failed
docker service logs -f --tail 100 betula_radix                              # one JSON line per event
docker service logs --since 1h betula_radix 2>&1 | grep '"level":"ERROR"'   # ERROR = a human is needed
docker service logs --since 1h edge_traefik 2>&1 | grep -i acme            # certificate orders
docker service logs --since 1h edge_traefik 2>&1 | grep '"DownstreamStatus":5'   # failed requests
sudo journalctl -u ssh -u docker -u fail2ban --since -2h                   # host units (docker: Traefik's lines too)
sudo journalctl -t dockerd --since -2h                                     # dockerd alone, without them
sudo journalctl -k --grep betula-docker-block                              # packets the port filter dropped
sudo fail2ban-client status sshd                                           # current bans
docker service logs --since 1h cortex_a 2>&1 | grep '"level":"\(WARN\|ERROR\)"'  # Cortex: the same for cortex_b
docker service logs --since 1h cortex_b 2>&1 | grep -E 'leader\.|replica\.'   # who led when, the follower
```

Loki keeps the long-term copy (Grafana > dashboard "Logs"; labels `stack`, `service`, `container`
for containers, `job="journal"` and `unit` for the host). Traefik logs through the journald driver,
so its lines, the access log with visitors' addresses, are in the host journal as well (7 days,
section 9); `docker service logs edge_traefik` reads them from there.

## 8. What happens without you

| What | When |
|---|---|
| Ubuntu security updates | daily (unattended-upgrades); services using an updated library are restarted, except docker/containerd |
| **Reboot** | 04:30 Europe/Berlin, only when an update asks for it (kernel, libc). All containers restart; the site is away for about a minute. Interrupting Radix is safe (docs/radix/operations.md) |
| Reboot after a kernel panic | after 60 s |
| Containers | swarm restarts a task that exits or turns unhealthy; after a boot everything comes back by itself |
| Cortex's leader dies | the follower takes the lock over within about 100 ms and leads; swarm restarts the dead one, which comes back as the follower (after a crash usually from a copy of the leader's index, which drops what it had not passed on). Nothing fails back (section 14) |
| Cortex's retention | every hour: versions superseded more than 180 days ago, journal entries older than 7 days, blobs nothing references after a 7-day grace |
| Canary follows master | every two minutes a look at GitHub for a new build of master; one that is there goes to https://canary.betula.app with the public site's data (section 12). Off: `sudo bash /opt/betula/vps/60-canary.sh off` |
| Certificates | Traefik renews 30 days before expiry; an alert fires below 14 days |
| Image cleanup | Sunday 03:30: dangling images and old build cache only, never networks or volumes |
| ssh bans | fail2ban: 5 failures in 10 min = 1 h, doubling up to a week; sshd penalises per source on top |
| **Not** automatic | Docker Engine upgrades, image tag updates, backups (none exist yet: volumes `edge_acme`, `betula_radix-data`, `monitoring_grafana-data`, `cortex_a-data`, `cortex_b-data`; Cortex's follower is no backup, a deletion replicates to it) |

Nothing on this server can report that the server itself is down: point an external uptime check
at `https://betula.app/`.

## 9. Where client addresses are stored

| Where | What | How long |
|---|---|---|
| Traefik access log -> the host journal (the service logs through the journald driver, `stacks/edge.yml`) | client IP and port, URL with query string, user agent, time | **7 days** (`MaxRetentionSec` + `MaxFileSec` in `vps/files/journald-betula.conf`), earlier when the journal reaches 500 MB |
| the same lines in Loki, stream `{service="edge_traefik"}` | same | **7 days** (`retention_stream` in `config/monitoring/loki.yml`) |
| sshd, fail2ban and the port filter in the journal; rsyslog copies to `/var/log/auth.log` | addresses of ssh clients and of dropped packets | journal 500 MB / 7 days; Loki 30 days; `auth.log` per logrotate |
| Grafana's own log in Loki | address of whoever logs in to Grafana | 30 days |
| fail2ban database | banned addresses | 8 days |

Folia's own log keeps paths (no addresses) 30 days in Loki, except `/calendar/…`, which it writes as
`/calendar/….ics`. Traefik logs full paths with the client address, 7 days in Loki and 7 days in the
host journal; a subscribed calendar appears there on every poll (Google about daily). What Loki's
ruler counts from these lines for the dashboard "Visitors" (`config/monitoring/loki-rules`) reaches
Prometheus as numbers only, without addresses, user agents or calendar codes. Alloy ships the
journal without the lines of containers (`loki.relabel "journal"`), or Traefik's would be kept 30
days as `{job="journal"}`. Not in Docker's local log files, which only rotate by size: Traefik's
stopped containers from before the journald driver still have such files, and `40-stacks.sh edge`
removes them (`91-verify-stacks.sh accesslog` checks; `90-verify-host.sh journald` checks the oldest
journal entry). The placeholder's nginx writes no access log. The privacy notice of the site names the
first two rows („Zugriffsprotokoll" in `folia/crates/home/src/i18n/legal.rs`, in every language). Levers: drop `ClientHost` in
`stacks/edge.yml` (loses abuse analysis) or shorten the period, in `loki.yml` and
`vps/files/journald-betula.conf` together.

Cortex keeps no visitor's address: its clients are the services, and its request log
(`http.request`) names their internal container address, the URL they asked for and their
User-Agent.

Applying this to a server that ran Traefik on the `local` driver, in this order, so that no line
reaches Loki under `{job="journal"}` and the journal keeps nothing older than 7 days:
`bash 40-stacks.sh monitoring` (Alloy drops the journal's copy), `sudo bash 10-base.sh` (journald:
7 days), `bash 40-stacks.sh edge` (Traefik through journald, old containers removed), then both
verify scripts.

## 10. Break-glass

When ssh does not let you in: Contabo customer panel -> VNC console -> log in as `root` with the root
password (passwords only work there, never over ssh). VNC consoles often assume a US keyboard
layout: type the special characters of the password at the `login:` prompt first to see what arrives.

```bash
/usr/local/sbin/betula-ssh-rollback            # only while a lockdown is unconfirmed: undo it
rm -f /etc/ssh/sshd_config.d/00-betula-hardening.conf /etc/ssh/sshd_config.d/01-betula-penalties.conf
sshd -t && systemctl reload ssh                # root + password logins work again: fix, then re-run 20-ssh-lockdown.sh
fail2ban-client unban --all                    # you may simply be banned
ufw status verbose; ufw allow 22/tcp           # the firewall rule for ssh
journalctl -u docker -n 50                     # dockerd refuses to start when the port filter cannot be installed
/usr/local/sbin/betula-docker-firewall         # ... run it by hand to see why
```

Replaced files are backed up under `/var/backups/betula/<run id>/`.

## 11. Alert notifications

Until a channel is wired, alerts are only visible in Grafana (panel "Alerts that need attention" on the
home dashboard); `40-stacks.sh` and `91-verify-stacks.sh` both say so. The contact point is called
`betula-owner`; switching the channel only replaces its receiver.

- **E-mail:** choose an SMTP provider, create the four secrets `grafana-smtp-host` (`host:587`),
  `grafana-smtp-user`, `grafana-smtp-password`, `grafana-smtp-from` with `monitoring-secrets.sh set`,
  run `40-stacks.sh monitoring` (it adds `monitoring.smtp.yml` by itself once all four exist), then
  Grafana > Alerting > Contact points > betula-owner > Test.
- **ntfy (push to the phone, no account), webhook or Telegram:** follow
  `stacks/monitoring.notify.example.yml`: copy it to `monitoring.notify.yml`, swap the receiver in
  `config/monitoring/grafana/provisioning/alerting/contact-points.yml` (ready blocks in its header),
  create the secret, sync, `40-stacks.sh monitoring`.

Cortex's alerts (group `betula-cortex`, dashboard "Cortex"): "Cortex has no leader, or two"
(critical, after a minute), "Cortex: the follower is more than 5 minutes behind" (warning, after 5
minutes) and "Cortex has paused a host" (warning, after 5 minutes), plus "Monitoring target is
down" for job `cortex`. They apply only to a Cortex that answered a scrape within the last 7 days:
quiet before the first deploy, and 7 days after Cortex was last up (after removing Cortex on
purpose, silence them for that long).

Critical alerts repeat every 4 hours, warnings daily (`policies.yml`). Expected once during the first
bring-up: "SSH login that is not the deploy key", if the last root login was less than 15 minutes
before the monitoring stack started.

## 12. Canary follows master

Every push to master that changes what the images are built from reaches https://canary.betula.app
by itself: built on GitHub, fetched by the server, seeded with the data of the public site, migrated
by the new release, switched blue-green, and the colour that served before is removed. The public
site is still shipped by hand (section 4); nothing here touches it but one read-only copy of its
database per deploy.

```
push to master ─▶ GitHub Actions  .github/workflows/images.yml
                    nix build .#radix-image .#folia-image, exactly as ship.sh (same sources, same tag)
                    ─▶ artifact betula-images-<tag>.tar (release.json + both images), kept a week
server  betula-canary.timer, 2 minutes after the last run ─▶ vps/canary-agent.sh poll (as deploy)
   1  GitHub API, read-only token: a new successful run for a push to master? its artifact, sha256 checked
   2  docker load, tag <tag> (as ship.sh)
   3  the colour that does not serve (canary / canary-green): stack and volumes removed
   4  the public site's radix.db (the colour whose Radix crawls) ─ host sqlite3, read-only, VACUUM INTO ─▶ copy
   5  45-seed.sh <colour>, 50-app.sh <colour> <tag>: the new release migrates, builds and exports the
      copy in containers without a network, then runs as the standby
   6  /healthz 200 ─▶ 55-switch.sh <colour>; until the new colour has settled the old one stays
   7  the colour that served before: stack and volumes removed; images of old builds go (the newest three stay)
```

A deploy takes a few minutes on the server; the build on GitHub takes longer (see "How fast, and what it costs").
When a step fails, canary keeps the release it had and the alert "canary: a build of master did not
reach canary" fires. A release is tried three times, 10 and 20 minutes apart; then the agent waits
for the next build of master. A release deployed by hand stays until master is built again: the
agent only acts on a build it has not seen.

### Branches: develop gathers, master goes to canary

Since 2026-10-01 a finished branch is merged into `develop`, not into master (owner: canary should
not deploy ten times an hour). A push to `develop` starts nothing: `images.yml` builds on pushes to
master only, and the agent takes nothing else. When the features gathered there are to reach
canary, `develop` goes into master in one merge: one build, one deploy.

```bash
# a finished branch, "Merge branch '<branch>' into develop: <what it brings>"; nothing is built
git switch develop && git merge --no-ff <branch> && git push origin develop
# a release to canary, "Merge branch 'develop' into master: <the features it brings>"
git switch master && git merge --no-ff develop && git push origin master
```

Master takes nothing but `develop`, so `develop` always holds all of master and nothing has to be
merged back. `develop` is GitHub's default branch (since 2026-10-01): new branches, pull requests
and sessions of Claude Code start from it. `CLAUDE.md` says the same for Claude.

### Setting it up, once

1. Merge. The workflow runs on the merge itself (it adds `.github/workflows/images.yml`) and on every
   later push to master that touches the sources; its job summary names the release and the artifact.
2. The token, on GitHub: Settings > Developer settings > Personal access tokens > Fine-grained tokens >
   Generate. Resource owner `leonieziechmann`, repository access "Only select repositories":
   `betula.app`, repository permission **Actions: Read-only** and nothing else (GitHub adds
   "Metadata: Read-only" by itself), an expiry date (a year) and an entry in the calendar before it.
   Straight into the password manager.
3. `SSH_TARGET=betula bash deploy/sync.sh`, then `ssh betula sudo bash /opt/betula/vps/60-canary.sh`
   (sqlite3, the units, `/var/lib/betula-canary`).
4. `<password manager CLI> | ssh betula sudo bash /opt/betula/vps/60-canary.sh token`: tries the
   token (GitHub has to accept it, it has to be allowed to download an artifact, and one that can see
   the repository's administration is refused), stores it encrypted with the host's key
   (`systemd-creds`, `/etc/credstore.encrypted/betula-canary-github-token`), switches the timer on.
5. `ssh betula journalctl -fu betula-canary.service`. **The first run replaces both canary colours:**
   the data canary had (the workstation's crawl) is gone, the public site's takes its place.
6. `ssh betula bash /opt/betula/vps/40-stacks.sh monitoring` (the new alert rule), then
   `ssh betula bash /opt/betula/vps/91-verify-stacks.sh app canary alerts`.

### Day to day

| | |
|---|---|
| what serves, what failed, what the next deploy copies | `bash /opt/betula/vps/canary-agent.sh status` |
| watch a deploy | `journalctl -fu betula-canary.service` |
| look now, not within two minutes | `sudo systemctl start betula-canary.service` (returns when it is done) |
| fresh data for the release canary runs | `bash /opt/betula/vps/canary-agent.sh deploy` |
| back to an older release (the images of the newest three stay) | `bash /opt/betula/vps/canary-agent.sh deploy <tag>`; it stays until master is built again |
| ship to canary by hand, or work on it | `sudo bash /opt/betula/vps/60-canary.sh off` first (a run in progress is finished), `on` afterwards |
| a release that failed | `journalctl -u betula-canary.service -n 100`: the FATAL line names the step. The colour it left half made stays for a look (`docker service logs`, the `docker run ... build` of 50-app.sh); the next deploy removes it |
| a new token | step 2 and 4 above; revoke the old one on GitHub |

`canary.env` and `canary-green.env` have to say `RADIX_CRAWL=off` or `cortex-offline` (the agent
refuses `on`: with the public site's data a canary that crawls would ask the university for
everything a second time) and the same `FOLIA_ACCESS_GATE`. `CANARY_SEED_FROM=<instance>` takes another instance's
database for one `deploy` by hand.

### Why it is safe enough

- **Nothing reaches into the server.** No ssh key, no webhook and no runner at GitHub: the server
  asks (HTTPS to api.github.com and GitHub's artifact storage), ufw and sshd stay as they are.
- **The token can do one thing.** Read this repository's workflow runs and artifacts - public
  anyway, since the repository is; GitHub only wants a token for the download. `60-canary.sh`
  refuses a classic token and one that can see the repository's administration. Encrypted at rest,
  in clear only in the service's own credentials directory, never in argv (header files, curl's
  config on stdin); the short-lived address of the download gets no token.
- **Only master counts.** A run of `images.yml` in this repository, started by a push to master,
  finished with success; a pull request, another branch or a fork produces nothing the agent takes.
  The file has to match the sha256 GitHub keeps for it and the sums in its `release.json`.
- **The agent never updates itself.** `deploy/` reaches `/opt/betula` only through `sync.sh` from
  the workstation. What a push to master changes is what runs *inside* canary's containers
  (`cap_drop: ALL`, Folia read-only and not root, closed testing, Radix offline) - what shipping
  master by hand would run as well.
- **The public site's database is only read.** By Ubuntu's `sqlite3` with `-readonly`, as one read
  transaction (WAL: the reader does not block the writer, Radix crawls on); no binary of a new
  release ever opens it, so no migration can reach it. Tested with a writer committing all the
  time: every copy was complete to one commit and passed the integrity check.
- **The workflow** asks for `contents: read` only, uses no secret, runs on pushes to master only,
  and its actions are pinned to commits. Its build cache (GitHub's cache of this repository) is
  written by its own runs on master; a run reads only its own branch's entries and those of the
  default branch, `develop`, where nothing writes; never a pull request's or a fork's. Nix takes
  the cached paths without signatures, so a step of the job that went bad could leave something in
  it for later builds: the job uses four actions, three of them GitHub's own, all pinned. Keep it
  the only writer: a workflow added later that saves a cache on `develop` (on its pushes, on a
  schedule, which runs on the default branch, or for pull requests into it with
  `pull_request_target`) would write into what every build of master reads.
- **The repository is public** (since 2026-09-30). Everybody can read the code, the workflow's logs
  and, logged in to GitHub, its artifacts; none of them holds a secret (the workflow has none, and
  the history held no key or token when it went public). Everybody can fork it and open pull
  requests, but neither starts `images.yml`, and the agent takes nothing but runs of a push to
  master in this repository: from the outside, reading is all there is.
- **Worth adding on GitHub:** a branch protection rule for master (pull requests, no force
  pushes): whoever can push to master decides what canary runs. And under Settings > Actions >
  General "Allow ... select non-... actions": actions by GitHub, and `cachix/install-nix-action@*`
  - nothing else is used.

### How fast, and what it costs

- **Time**, measured 2026-09-30 on the standard runner of a public repository (4 processors,
  16 GB). GitHub hands out faster and slower ones: the same work took 1.6 times as long on a slow
  one, hence the ranges.

  | a push that changes | the job | of it the build |
  |---|---|---|
  | nothing Radix or Folia are built from (the workflow, say) | 47 s | 17 s |
  | the app, not its dependencies (the common case) | 7 to 11 min | 6½ to 10½ min |
  | `Cargo.lock` or `flake.lock`; and the first build on master | 13 to 21 min | 12½ to 20 min |

  After a change to the app everything but the workspace's own crates comes from the cache, in
  under half a minute: what is left is compiling folia-app and folia-server, 6 to 10 minutes, with
  the browser app (3 to 5) beside it. The web server's thin LTO is not what takes the time:
  without it that step took 8 minutes instead of 9 (on the workstation), so it stays. A change to
  Radix adds its build with the tests, 2 to 3 minutes. The deploy on the server adds a few
  minutes.
- **What makes it fast:** `flake.nix` builds the Rust dependencies apart from the workspace
  (crane), so a change to the app leaves them as they are; the workflow keeps what builds of
  master made and cache.nixos.org does not have in GitHub's cache ("Restore the Nix cache" in
  `images.yml`): some 500 paths, 1.2 GB, 560 MB compressed. A build from the cache gives the same
  images to the byte as one without it (the same sha256, measured). A new entry is only written
  when the dependencies, the flake or the Go sources change, and only after a build that
  succeeded; GitHub keeps 10 GB per repository and drops what has not been used for a week. The
  first build of master after the merge starts without it: master does not see a branch's
  entries. What would still help, a little: the web server and the browser app on two runners.
  They share the processors only while both compile folia-app, so that is about a minute, at the
  price of passing the browser app from one job to the other.
- **GitHub Actions minutes:** none - a public repository's standard runners cost nothing.
- **Artifact storage:** 57 MB per build (Radix 7, Folia 50), gone after a week; a public
  repository pays nothing for it.
- **This server:** per deploy one copy of the database, one build and export of the catalog (the
  CPU of a minute or two) and the new Folia warming its cache (up to 3 processors for a moment),
  next to the public site. The disk holds the images of the newest three builds.

## 13. The models of the semantic search

The semantic search needs two model files that are too large for git and the images: Radix's
passage model (`e5-de-en-server.bin`, 35 MB, the modules' vectors) and the browser's query model
(`e5-de-en.bin`, 15 MB, what a visitor types; `folia/crates/semantic/README.md` says how both are made). They
reach the services through the server's **model store**, not through a registry, a CDN or the images:

```
models.lock (git)          passage <sha256> <bytes> e5-de-en-server.bin
                           query   <sha256> <bytes> e5-de-en.bin
workstation models/  ──ship-models.sh: only what the store lacks, checked on both sides──▶
/var/lib/betula/models/<sha256>       one file per model, written once, never changed
  ├─ read-only into every instance's Radix   RADIX_EMBED_MODEL=/models/<passage sha256>
  └─ read-only into every instance's Folia   FOLIA_SEMANTIC_MODEL=/models/<query sha256>
        └─▶ browsers: /models/e5-de-en-<hash>.bin, immutable, brotli, kept by the service worker
```

- **Stable.** A file's name is its content's sha256: it is checked when it arrives (size and hash,
  before one atomic rename gives it its name), at every deploy (`50-app.sh` hashes both again), and
  by Radix and Folia when they load it (a damaged file is refused). Nothing in the store is ever
  overwritten, so a rollback to an older release, or a colour that has not been deployed again,
  still finds its models. Without its models, or with a broken store, an instance runs without the
  semantic search and says so (WARN in `50-app.sh` and `91-verify-stacks.sh`); it never fails to
  start for them: the store is mounted by an override, `stacks/betula.models.yml`, which `50-app.sh`
  adds only when both models are there, intact.
- **Scales.** One copy per host for every instance, colour and release (canary's colours are
  replaced with their volumes on every build of master; the store stays). The images stay as they
  were (canary's artifact on GitHub does not grow); a deploy uploads nothing when the store has
  the models. Each browser downloads the query model once per model, not once per deploy: its
  address names its content, `immutable`, and the service worker keeps it in a cache no build drops.
- **The pair.** A query is only comparable with passages of the model it was made for, so the lock
  names both, and they change together. The browser checks it once more against the snapshot
  (`docs/radix/schema-v2.md`, „Semantic search"): while Radix computes the vectors of a new passage
  model (about a day), the semantic search simply is not offered.
- **Offline too.** A Radix with `RADIX_CRAWL=off` computes the vectors its database lacks as well,
  with the encoder alone (no Gemini: nothing goes out), and publishes them part by part. Canary,
  seeded with the public site's database at every deploy, so has the semantic search a few hours
  after a deploy (its one CPU), and right away once the public site's Radix has the vectors.

Once per server (a fresh one gets it from `vps/10-base.sh`):

```bash
ssh betula sudo install -d -m 0755 -o deploy -g deploy /var/lib/betula/models
```

New models: build them, put them into `models/` in the repository's root (git ignores it), write
their sha256 and size into `deploy/models.lock` (`sha256sum models/*`, `stat -c %s models/*`) and
commit. Then

```bash
SSH_TARGET=betula bash deploy/ship-models.sh        # syncs deploy/, uploads what the store lacks
ssh betula bash /opt/betula/vps/50-app.sh <instance> # each instance, or with its next release
```

`deploy/ship.sh` runs `ship-models.sh` before every deploy (a failure there is a WARNING, the
deploy goes on without the semantic search); canary's agent deploys with whatever the store holds
of the lock, so after a change of the lock, `ship-models.sh` first. On the server:

| | |
|---|---|
| the lock, the store, which service runs which model | `bash /opt/betula/vps/models.sh status` |
| remove models neither the lock nor a service names | `bash /opt/betula/vps/models.sh prune` |
| what an instance runs | `bash /opt/betula/vps/91-verify-stacks.sh app` |


## 14. Cortex

Cortex is the cache between the application and the internet (`docs/cortex/cortex.md`): every request of
Radix's crawl and of the statute download goes to it, and it fetches from the university only what
it does not have fresh, at one floor per host for every instance and colour together, and keeps
every version. One Cortex per host, not per instance: two instances of one image in the stack
`cortex` (`stacks/cortex.yml`), each with a volume of its own, of which one leads and the other
follows and takes over. Built 2026-10-02/03; not deployed yet. It is not part of the canary
pipeline (`images.yml` builds Radix and Folia): a push to master does not restart the cache the
live site uses.

```
workstation   deploy/ship-cortex.sh: nix build .#cortex-image ─ssh─▶ docker load, tag <date>-<hash>
              ─▶ sync.sh ─▶ vps/48-cortex.sh <tag>

server        Radix of every instance and colour, Prometheus
                 │ overlay "cortex" (internal: true): http://cortex_a:8100, http://cortex_b:8100
                 ▼
              ┌─ cortex_a ── volume cortex_a-data ─┐  the one that holds the flock on
              │                                    │  /lock/leader.lock (volume cortex_lock) leads;
              └─ cortex_b ── volume cortex_b-data ─┘  the other follows its journal
                 │ cortex_default (the stack's own network, not internal)
                 ▼
              qis.b-tu.de, www.b-tu.de, opus4.kobv.de: any public host the policy allows

              Radix ─ <stack>_egress (not internal) ─▶ Gemini, only where 50-app.sh adds it
```

### Setting it up, once

In this order; steps 1 and 2 belong together.

1. `SSH_TARGET=betula bash deploy/sync.sh`. From here on **every `50-app.sh` stops in its
   preflight until the network `cortex` exists**, the canary agent's unattended one included (a
   build of master would fail to reach canary, and be tried again). Run step 2 right away, or
   pause the agent first (`ssh betula sudo bash /opt/betula/vps/60-canary.sh off`, `on` after
   step 5).
2. `ssh betula sudo bash /opt/betula/vps/30-docker.sh`: `network cortex created` (overlay,
   attachable, internal). It changes nothing else on a host that has the rest; then
   `ssh betula sudo bash /opt/betula/vps/90-verify-host.sh`, `fail=0`.
3. `SSH_TARGET=betula bash deploy/ship-cortex.sh`: builds the image of the commit, loads it, syncs,
   and runs `48-cortex.sh <tag>`, whose first deploy starts both instances. It ends with exactly one
   leader and the follower caught up, and suggests `91-verify-stacks.sh cortex`.
4. `ssh betula bash /opt/betula/vps/40-stacks.sh monitoring`: Prometheus joins `cortex`, the scrape
   job `cortex`, the alert group `betula-cortex` and the dashboard "Cortex" take effect (the four
   monitoring services restart once). Before step 5: once Radix leaves `monitoring`, Prometheus
   reaches it over `cortex` only.
5. `ssh betula bash /opt/betula/vps/50-app.sh <instance>` for each instance, **the standby colour
   first**, then the colour that serves and crawls. Each run restarts Radix and Folia once (their
   networks change). Afterwards Radix fetches through Cortex and has no way out of its own, but
   for Gemini (below). The canary colours follow with the agent's next deploy, or at once with
   `50-app.sh <the canary colour that serves>`.
6. `ssh betula bash /opt/betula/vps/91-verify-stacks.sh cortex`, then the whole of it: `fail=0`.

Without step 3 the others work all the same: a crawling Radix then gets `stacks/betula.egress.yml`
and fetches directly, as before; running step 5 again once Cortex runs moves it over.

**The public site may wait** (owner, 2026-10-04: „erstmal das system zu testen, ohne main zu
stören"): step 5 for the canary alone. The colours of https://betula.app keep running as they
were deployed, Radix on `monitoring` and `<stack>_default` and the crawling one fetching directly;
`91-verify-stacks.sh cortex` says so with a FAIL per colour (a way to the internet), until their
next deploy moves them over. Cortex then has what `cortex-seed.sh` gave it (below) and nothing
newer, and the canary reads that.

### Seeding Cortex with an archive

Cortex starts empty. `vps/cortex-seed.sh` gives it the raw pages a Radix archived, so that it
serves them as if it had fetched them itself, offline too (`radix seed-cortex`,
`docs/cortex/cortex.md` §4.3 and §8):

```bash
ssh betula bash /opt/betula/vps/cortex-seed.sh                  # the public site's archive (the colour that crawls)
ssh betula bash /opt/betula/vps/cortex-seed.sh betula <tag>     # another instance's, with a loaded release
```

It copies the instance's `radix.db` with the host's `sqlite3`, read-only, as one transaction
(`VACUUM INTO`, as the canary agent does: its Radix is neither stopped nor slowed down), runs
`radix seed-cortex` of a loaded release (default: the one the canary serves with; one from
before 2026-10-04 has none) in a container on the network `cortex` alone, and removes the copy.
Each archived page whose page is the whole answer of its URL becomes Cortex's answer to that URL,
first fetched when it last changed and last fetched when the archive last had it; the follower
copies it from the journal. A second run writes only what the archive has newer (`unchanged`,
`older` for the rest), so it may run whenever the canary should see newer pages while the public
site still fetches directly. `CORTEX_SEED_ARGS="--dry-run"` counts and sends nothing. The dashboard
"Cortex" shows what came (Imported answers by result) and, under it, what the canary's Radix found
in the store and what not.

The canary's colours (`RADIX_CRAWL=cortex-offline`, section 4) read Cortex's store alone: their
cycles take a page from Cortex whatever its age and skip one it lacks, and nothing of theirs
reaches the university.

### Day to day

| | |
|---|---|
| who leads, how far the follower is behind | `bash /opt/betula/vps/91-verify-stacks.sh cortex`; one instance: `docker exec "$(docker ps -q -f name=cortex_a \| head -n 1)" /bin/cortex status` (the same for `cortex_b`) |
| hand the lead to the other instance | `docker exec <the leader's container> /bin/cortex step-down` (exit 1 on the follower) |
| logs | `docker service logs --since 1h cortex_a` / `cortex_b` (section 7): `leader.*`, `replica.*`, `upstream.failed`, `host.paused` |
| a new release | `SSH_TARGET=betula bash deploy/ship-cortex.sh` (the tag changes only with what the image is built from; the same tag twice changes nothing) |
| give Cortex an instance's archive | `bash /opt/betula/vps/cortex-seed.sh [<instance> [<tag>]]` (above, „Seeding Cortex with an archive") |
| a release that is loaded already, the rollback | `bash /opt/betula/vps/48-cortex.sh <tag>` (`docker image ls betula-cortex`) |
| a change to `stacks/cortex.yml` | sync, `bash /opt/betula/vps/48-cortex.sh` (the release that runs) |
| the host policy | `config/cortex/hosts.json`, sync: read again within 30 s, no restart (`docs/cortex/cortex.md` §5) |
| a follower that fetches the blobs of a whole index (a fresh volume, away more than 7 days) | `CATCHUP_TIMEOUT=3600 bash /opt/betula/vps/48-cortex.sh <tag>` goes on where a stopped run ended (default 600 s) |
| a follower that cannot reach the leader | `48-cortex.sh` stops after `STALL_TIMEOUT` (60 s) of `no_leader`, or of no progress while its lag grows, and names the leader's URL: `docker service logs cortex_X 2>&1 \| grep replica.failed`, not a longer `CATCHUP_TIMEOUT`. One huge blob (a model of gigabytes) may need `STALL_TIMEOUT=<seconds>` |
| an update that failed | swarm rolled the instance back (the other one is untouched, the script exits 1); it shows `rollback_completed` until its next update, still counts as running for `50-app.sh` and is a WARN in `91-verify-stacks.sh cortex` (a FAIL in section services). The previous tag again, or a fixed one |
| store or read a file by hand | `docker exec -i <container> /bin/cortex put <name> < file`, `docker exec <container> /bin/cortex get <name> > file` |

**Never `docker stack deploy -c stacks/cortex.yml cortex` or `docker service update --image` on
Cortex by hand.** The first updates both instances at once: a gap without a leader. After the
second, a later stack deploy of the stack's last tag keeps the image set by hand (`48-cortex.sh`
repairs that). `48-cortex.sh` goes one instance at a time: the follower first, from
`stacks/cortex.yml` without the other's block (a change to the file goes out the same way); it
waits until the follower runs and has caught up (state `following` or `catching_up`, under 1 s
behind, on the leader's epoch, at or above the leader's sequence number of 2 s before, 0 blobs
missing); `cortex step-down` in the leader; it waits until the follower leads, then 3 s, in which
the clients move over on fresh connections; then the old leader the same way. An instance that runs
the wanted image from the current revision of the file (label `app.betula.cortex-rev`) is not
touched. When no instance leads, or both say they do (the lock is not shared), both are deployed at
once. `CONVERGE_TIMEOUT` (600 s) bounds the wait for an instance to run.

### The networks

| Network | Who is on it | Why |
|---|---|---|
| `cortex` (host, `30-docker.sh`: overlay, attachable, internal) | `cortex_a`, `cortex_b`, every instance's Radix, Prometheus | Radix and Prometheus reach `http://cortex_a:8100` and `http://cortex_b:8100`; Prometheus scrapes Radix here too. Nothing leaves the host this way |
| `cortex_default` (the stack's own, not internal) | `cortex_a`, `cortex_b` | Cortex's way to the internet |
| `<stack>_snapshot` (per instance, internal) | Radix, Folia | Folia's snapshot downloads from Radix |
| `<stack>_egress` (per instance, not internal; `stacks/betula.egress.yml`) | Radix, only where `50-app.sh` adds it | Gemini, which does not go through Cortex (owner, 2026-10-02: „Cortex ist nur für daten da.") |

Radix is on neither `<stack>_default` nor `monitoring` any more: Docker gives a container a way
out as soon as one of its networks is not internal. Folia keeps `default` and `edge`; `monitoring`
holds Traefik and Prometheus. `50-app.sh` adds the egress override when the instance's file says
`RADIX_CRAWL=on` and Cortex does not run, or the secret `gemini-api-key` exists, or the release is
from before Cortex; an offline colour never gets it. Docker has no allowlist per host for a
container's way out: while it is there, Radix can reach any host. So `radix scan-curriculum` with
Gemini works only in the crawling colour with the key; elsewhere it reads the PDFs with the
deterministic reader alone. A removed egress override leaves the empty network object
`<stack>_egress` behind. `50-app.sh` and `91-verify-stacks.sh` (sections `app` and `cortex`) check
what swarm was told: Radix on `cortex` and `<stack>_snapshot`, egress exactly where expected, every
other network internal, Folia not on `cortex`.

### Alerts and the dashboard

The dashboard "Cortex" and the alert group `betula-cortex` (section 11): no leader or two, the
follower more than 5 minutes behind, a host paused; "Monitoring target is down" for job `cortex`.
All are quiet for a Cortex that has not answered a scrape within 7 days, so a host without Cortex
pages nobody; `48-cortex.sh` and `91-verify-stacks.sh cortex` report a first deploy that never
answers. Cortex's `ERROR` lines reach the log alert like every stack's (`stack=~"…|cortex"`).

### Why it is safe enough

- **One leader, by the kernel.** Whoever holds the `flock` on `/lock/leader.lock` (the volume
  `cortex_lock`, the one thing both instances share) leads; the kernel releases it the moment the
  process ends, and the follower leads within about 100 ms (measured: median about 75 ms from
  `kill -9` to the first acknowledged write). There is no timeout during which both could write.
- **A planned stop loses nothing.** A step-down or `SIGTERM` waits for every write in flight and
  keeps serving its journal until the successor has all of it (`docs/cortex/cortex.md` §6.7): measured 0
  writes lost over step-downs and `SIGTERM`s under load, also with a follower 2,500 entries behind.
- **A crash loses a moment.** Replication is asynchronous (owner, 2026-10-02: „Ja die letzen 5 min
  sind egal"): over 29 `kill -9` of the leader under load, no acknowledged write older than 5 s was
  lost, the oldest lost one was 0.7 s old. The alert fires at 5 minutes behind.
- **Nothing reaches in.** No published port; the network `cortex` is internal and holds only the
  host's own services; no secret, no token, no credential is stored or forwarded (`Cookie` and
  `Authorization` never go upstream).
- **Nothing private goes out.** Cortex refuses every address that is not globally reachable,
  whatever a name resolves to (loopback, private, link-local and the metadata address, …), and host
  names that are not ASCII: no client can make it fetch Grafana, the socket proxy or another
  instance.
- **Polite for everybody together.** One floor per host (`concurrency`, `pause`, a breaker) for
  every client, so two colours that crawl do not add up; what was fetched within the hour is not
  fetched again.
- **Not covered.** The host itself: both instances are on it. And backups: none exist yet
  (section 8); the volumes `cortex_a-data` and `cortex_b-data` are worth one (either is enough).
  The follower is no backup: a deletion, or a bug that writes wrong data, replicates to it.
