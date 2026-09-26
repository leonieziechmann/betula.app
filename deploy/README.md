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
    90-verify-host.sh     PASS/WARN/FAIL audit of 10-30      91-verify-stacks.sh  the same for 40 to 55
    files/                config payloads the host scripts install;  lib*.sh, sync-receive.sh  helpers
  stacks/                 edge(.www).yml, placeholder.yml, monitoring(.public|.smtp).yml, betula(.gemini|.offline).yml,
                          canary(-green).env, monitoring.notify.example.yml, monitoring-secrets.sh
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
https://canary.betula.app; the placeholder keeps https://betula.app until an instance gets
`APP_HOST=betula.app` (its router outranks the placeholder's priority 1, so there is no gap; then
`docker stack rm placeholder`). A new instance also has to be named in the two log rules of
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
is started as `radix serve-snapshot` instead of `radix run`. It sends nothing to the university's
servers - no crawl, no cycle, no build - and only hands the snapshot it has to Folia, so the
catalog stays as it was exported. `canary.env` says `off` for the time of the closed test (owner,
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

Only with `RADIX_CRAWL=off` in both files (two Radix that crawl would ask the university for
everything twice) and the same `FOLIA_ACCESS_GATE`; `50-app.sh` refuses anything else. The next
release goes to the colour that does not serve. Its volume keeps its database: for new data
remove its stack and volume and ship it with `--seed` again; after a release with a new schema,
build and export a new snapshot in its Radix (`docker exec <its radix container> /bin/radix build
--db /data/radix.db`, then `docker exec <its radix container> /bin/radix export --db
/data/radix.db --out /data/snapshot`; `55-switch.sh` refuses a catalog the build cannot read).
The build is not optional: the new binary migrates the database when it opens it, but only a
build fills what the migration adds (schema 9: the short names), and `export` validates first
and refuses a database that was not built again. Do not reach for `--skip-validate` here.

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
host journal; a subscribed calendar appears there on every poll (Google about daily). Alloy ships the
journal without the lines of containers (`loki.relabel "journal"`), or Traefik's would be kept 30
days as `{job="journal"}`. Not in Docker's local log files, which only rotate by size: Traefik's
stopped containers from before the journald driver still have such files, and `40-stacks.sh edge`
removes them (`91-verify-stacks.sh accesslog` checks; `90-verify-host.sh journald` checks the oldest
journal entry). The placeholder's nginx writes no access log. The privacy notice of the site names the
first two rows („Zugriffsprotokoll" in `app/src/pages/legal.rs`). Levers: drop `ClientHost` in
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
