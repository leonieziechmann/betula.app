# Betula server runbook

Everything the server `betula.app` (77.237.233.41, one Contabo VPS, Ubuntu 26.04, one-node Docker
Swarm) needs lives in this directory. It is copied verbatim to `/opt/betula` on the server; nothing
is edited there. No password, token or key is ever part of these files.

```
deploy/
  sync.sh                 workstation -> /opt/betula (tar over ssh; refuses CR line endings)
  ship.sh                 workstation: build the images of a commit, load them on the server, deploy an instance
  vps/                    host scripts, run on the server, in the order of their numbers
    10-base.sh            upgrade, user deploy, ufw, fail2ban, unattended-upgrades, journald, sysctl, swap
    20-ssh-lockdown.sh    key-only ssh for deploy, root login off (with automatic rollback until --confirm)
    30-docker.sh          Docker Engine 29, swarm, overlay networks edge + monitoring, published-port filter
    40-stacks.sh          swarm secrets, stacks edge -> placeholder -> monitoring, waits for convergence
    45-seed.sh            once per instance, before its first deploy: a Radix database (tar on stdin) into its volume
    50-app.sh             one instance of the application (stacks/betula.yml + stacks/<instance>.env) at a release tag
    55-switch.sh          blue-green: hand a host name to the other of its two instances (the rollback is the same)
    60-canary.sh          canary follows master: sqlite3, the timer, the GitHub token (section 12)
    canary-agent.sh       what that timer runs: fetch a build of master, seed it with the public site's data, switch
    90-verify-host.sh     PASS/WARN/FAIL audit of 10-30      91-verify-stacks.sh  the same for 40 to 60
    files/                config payloads the host scripts install;  lib*.sh, sync-receive.sh  helpers
  stacks/                 edge(.www).yml, placeholder.yml, monitoring(.public|.smtp).yml, betula(.gemini|.offline).yml,
                          betula.env, canary(-green).env, monitoring.notify.example.yml, monitoring-secrets.sh
  config/                 bind-mounted read-only into the services: traefik/ placeholder/ monitoring/
```

All host scripts are idempotent and non-interactive; every one can simply be run again. Root scripts
are run with `sudo bash ...`; `40-stacks.sh`, `50-app.sh` and `91-verify-stacks.sh` run as `deploy`
**without** sudo (sudo would drop the environment variables that select staging certificates, the
Grafana host or an instance's values).

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
| 5 | workstation | `ssh betula sudo bash /opt/betula/vps/30-docker.sh` | `swarm initialised`, `network edge created`, `network monitoring created`, no `FATAL` |
| 6 | workstation | `ssh betula sudo systemctl reboot`, wait a minute, then `ssh betula true` | login works again |
| 7 | workstation | `ssh betula sudo bash /opt/betula/vps/90-verify-host.sh` | `fail=0` |
| 8 | workstation | `ssh betula bash /opt/betula/vps/40-stacks.sh` | `converged` three times |
| 9 | workstation | `ssh betula bash /opt/betula/vps/91-verify-stacks.sh` | `fail=0` |
| 10 | workstation | `curl -sI http://betula.app/` and `curl -sI https://betula.app/` | `301` to https, then `200` with `strict-transport-security` |

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
  The same recipe reaches Prometheus (9090) or Radix's `/status` (8090).
- Dashboards and alert rules are files (`config/monitoring/grafana/`); the UI refuses to save them.
  Edit, export JSON, commit, sync, `40-stacks.sh monitoring`.
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
`config/monitoring/grafana/provisioning/alerting/rules.yml` (`stack=~"(betula|canary)(-green)?"`),
or its errors stay silent.

Once per server: the DNS record of the instance's name, and the password of closed testing as a
swarm secret - the value travels on stdin, never in argv (`docs/frontend.md`, "Closed testing"):

```bash
<password manager CLI> | ssh betula docker secret create folia-access-password -
```

Optional: `gemini-api-key`, created the same way. Only `radix scan-curriculum` needs it; `50-app.sh`
adds `stacks/betula.gemini.yml` while the secret exists.

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

**Radix offline** (`RADIX_CRAWL=off` in the instance's file, `stacks/betula.offline.yml`): Radix
is started as `radix serve-snapshot --db /data/radix.db` instead of `radix run`. It sends nothing
to the university's servers - no crawl, no cycle - and hands the snapshot it has to Folia, so the
data stays as it was fetched. A new release builds the catalog once more from the archived pages
at start (no network) and exports it if its parsers and rules make something else of them; the
old snapshot is served meanwhile and stays if that build or its validation fails (`level=ERROR`,
`docs/operations.md` §1). `canary.env` says `off` for the time of the closed test (owner,
2026-09-21): the crawler's User-Agent names betula.app, and while that site shows only a login
page, requests in its name invite a block. `50-app.sh` makes sure there is a snapshot to serve (a
seeded volume that never ran gets one from its database: `radix build`, then `radix export`, in
containers without a network, so that a database older than the release is migrated and built) and
checks that swarm really starts `serve-snapshot`; `91-verify-stacks.sh app` checks the same and
reminds with a WARN that the data does not change. Back online: `RADIX_CRAWL=on`, sync,
`50-app.sh <instance>`; Radix then fetches what has aged in the meantime at its usual pace (one
request at a time with a pause after each, bulk only between 1 and 6 o'clock, a cap per source
and cycle: `docs/operations.md` §1). `radix scan-curriculum` with `docker exec` works in both
modes.

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
really runs `serve-snapshot`. Both canary colours are offline. The next
release goes to the colour that does not serve. Its volume keeps its database: for new data
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
and `radix scan-curriculum` (`docs/operations.md`), run with `docker exec` in the Radix container
(the Gemini key is the optional secret above).

## 5. Secrets

Swarm secrets are immutable; rotation means a new name.

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
```

Loki keeps the long-term copy (Grafana > dashboard "Logs"; labels `stack`, `service`, `container`
for containers, `job="journal"` and `unit` for the host). Traefik logs through the journald driver,
so its lines, the access log with visitors' addresses, are in the host journal as well (7 days,
section 9); `docker service logs edge_traefik` reads them from there.

## 8. What happens without you

| What | When |
|---|---|
| Ubuntu security updates | daily (unattended-upgrades); services using an updated library are restarted, except docker/containerd |
| **Reboot** | 04:30 Europe/Berlin, only when an update asks for it (kernel, libc). All containers restart; the site is away for about a minute. Interrupting Radix is safe (docs/operations.md) |
| Reboot after a kernel panic | after 60 s |
| Containers | swarm restarts a task that exits or turns unhealthy; after a boot everything comes back by itself |
| Canary follows master | every two minutes a look at GitHub for a new build of master; one that is there goes to https://canary.betula.app with the public site's data (section 12). Off: `sudo bash /opt/betula/vps/60-canary.sh off` |
| Certificates | Traefik renews 30 days before expiry; an alert fires below 14 days |
| Image cleanup | Sunday 03:30: dangling images and old build cache only, never networks or volumes |
| ssh bans | fail2ban: 5 failures in 10 min = 1 h, doubling up to a week; sshd penalises per source on top |
| **Not** automatic | Docker Engine upgrades, image tag updates, backups (none exist yet: volumes `edge_acme`, `betula_radix-data`, `monitoring_grafana-data`) |

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
first two rows („Zugriffsprotokoll" in `app/src/i18n/legal.rs`, in every language). Levers: drop `ClientHost` in
`stacks/edge.yml` (loses abuse analysis) or shorten the period, in `loki.yml` and
`vps/files/journald-betula.conf` together.

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

`canary.env` and `canary-green.env` have to say `RADIX_CRAWL=off` (the agent refuses otherwise:
with the public site's data a canary that crawls would ask the university for everything a second
time) and the same `FOLIA_ACCESS_GATE`. `CANARY_SEED_FROM=<instance>` takes another instance's
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
  written by its own runs on master; a run reads only its branch's entries and master's, never a
  pull request's or a fork's. Nix takes the cached paths without signatures, so a step of the job
  that went bad could leave something in it for later builds: the job uses four actions, three of
  them GitHub's own, all pinned. Keep it the only writer: a workflow added later that runs for pull
  requests with `pull_request_target` would write into master's part of the cache.
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
