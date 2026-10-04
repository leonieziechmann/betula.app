#!/usr/bin/env bash
# 40-stacks.sh - swarm secrets, then the stacks edge, placeholder and monitoring. Run on the server
# as the deploy user, WITHOUT sudo, in a session opened after 30-docker.sh (group docker):
#
#   bash /opt/betula/vps/40-stacks.sh                     # everything
#   bash /opt/betula/vps/40-stacks.sh monitoring          # only some stacks: edge placeholder monitoring
#
# Idempotent: "docker stack deploy" only touches a service whose definition changed, an existing
# secret is never replaced, and the same decisions are made from the same DNS and secrets on
# every run. Swarm replaces a whole service spec on each deploy, so an override file that is left
# out takes its routers away again - which is why the choice of files lives here and not in
# somebody's shell history.
#
# What decides which files are deployed:
#   edge.www.yml           only while www.betula.app resolves to this machine
#   placeholder            skipped as soon as an instance of the application serves betula.app (then
#                          remove it: docker stack rm placeholder). An instance at another name,
#                          like canary.betula.app, leaves it alone. The application itself is
#                          deployed by vps/50-app.sh, Cortex by vps/48-cortex.sh, never here.
#   monitoring.public.yml  only while GRAFANA_HOST is set and resolves to this machine
#   monitoring.smtp.yml    only while all of its swarm secrets exist (stacks/monitoring-secrets.sh)
#   monitoring.notify.yml  the same, if the file exists (your copy of monitoring.notify.example.yml)
# A router for a name that does not resolve here would make Traefik order a certificate that
# cannot be validated; Let's Encrypt counts those (5 failures per host name per hour).
#
# Environment (all optional; read by this script or substituted into the stack files):
#   GRAFANA_HOST=...        public name of Grafana, default grafana.betula.app.
#                           GRAFANA_HOST= (empty) keeps Grafana off the internet (ssh tunnel only).
#   GRAFANA_ADMIN_USER=...  admin login, default betula; only read when Grafana's database is created
#   ACME_STAGING=1          use Let's Encrypt's staging CA (untrusted certificates, generous limits);
#                           Traefik keeps them in a separate store. A run without it is production again.
#   ACME_CASERVER=...       any other ACME directory URL (https://); ACME_STAGING=1 sets it for you
#   SOCKET_PROXY_USER=...   see stacks/edge.yml
#   PUBLIC_ADDRESSES="..."  this machine's public addresses, if they are not on an interface (NAT)
#   CONVERGE_TIMEOUT=600    seconds to wait per stack (the first run pulls about 1 GB of images)
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
# shellcheck source=lib-stacks.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-stacks.sh"
betula_init --tmp
require_ubuntu

ALL_STACKS=(edge placeholder monitoring)
ACME_STAGING_URL="https://acme-staging-v02.api.letsencrypt.org/directory"
CREDENTIALS_FILE="/root/betula-initial-credentials.txt"
ADMIN_SECRET="grafana-admin-password"
# "-" and not ":-": an explicitly empty GRAFANA_HOST means "do not publish Grafana".
GRAFANA_PUBLIC_HOST="${GRAFANA_HOST-${DEFAULT_GRAFANA_HOST}}"
CREDENTIALS_WRITTEN=0

# ---------------------------------------------------------------- helpers

as_root() {
  if [[ "${EUID}" -eq 0 ]]; then
    "$@"
  else
    sudo -n "$@"
  fi
}

# checksum_of PATH... -> 12 hex digits over names and contents (order independent of the locale).
checksum_of() {
  find "$@" -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum | sha256sum | cut -c1-12
}

# deploy_stack and wait_for_stack live in lib-stacks.sh (50-app.sh uses them as well).

# ---------------------------------------------------------------- steps

preflight() {
  step "Preconditions"
  local net facts
  if [[ "${EUID}" -eq 0 && -n "${SUDO_USER:-}" ]]; then
    die "run this as ${DEPLOY_USER} WITHOUT sudo: sudo resets the environment, and GRAFANA_HOST / ACME_STAGING given on the command line would silently not apply"
  fi
  [[ "${BETULA_VPS_DIR}" == "${BETULA_ROOT}/vps" ]] ||
    die "this copy lives in ${BETULA_VPS_DIR}; the stack files bind-mount ${CONFIG_DIR}, so run ${BETULA_ROOT}/vps/$(basename "$0")"
  [[ "${CONVERGE_TIMEOUT}" =~ ^[0-9]{1,5}$ ]] || die "CONVERGE_TIMEOUT='${CONVERGE_TIMEOUT}' is not a number of seconds"
  require_cmd docker ip sha256sum find xargs
  require_swarm_manager
  have_cmd dig || warn "dig is missing (package bind9-dnsutils, vps/10-base.sh); falling back to getent, which also reads /etc/hosts"
  # cortex: Prometheus scrapes Cortex and every instance's Radix over it (stacks/monitoring.yml).
  for net in edge monitoring cortex; do
    facts="$(docker network inspect "${net}" --format '{{.Driver}} {{.Scope}} {{.Attachable}}' 2>/dev/null || true)"
    [[ "${facts}" == "overlay swarm true" ]] || die "overlay network ${net} is missing or not attachable (run vps/30-docker.sh)"
  done

  if [[ "${ACME_STAGING:-0}" == "1" ]]; then
    if [[ -n "${ACME_CASERVER:-}" && "${ACME_CASERVER}" != "${ACME_STAGING_URL}" ]]; then
      die "ACME_STAGING=1 and a different ACME_CASERVER are both set; choose one"
    fi
    ACME_CASERVER="${ACME_STAGING_URL}"
  fi
  if [[ -n "${ACME_CASERVER:-}" ]]; then
    [[ "${ACME_CASERVER}" == https://* ]] || die "ACME_CASERVER must be an https:// URL"
    export ACME_CASERVER
    warn "certificates come from ${ACME_CASERVER} (not trusted by browsers); run again without ACME_STAGING / ACME_CASERVER for production"
  else
    log "certificates: Let's Encrypt production"
  fi
  log "this machine's public addresses: $(local_public_addresses | tr '\n' ' ')"
}

create_secrets() {
  step "Swarm secrets"
  local value="" user="${GRAFANA_ADMIN_USER:-betula}"
  if secret_exists "${ADMIN_SECRET}"; then
    log "exists: ${ADMIN_SECRET} (never replaced; change the password with stacks/monitoring-secrets.sh reset-admin-password)"
    return 0
  fi
  # Needed for the one file below; checked before anything is created.
  as_root true 2>/dev/null || die "passwordless sudo is needed once, to write ${CREDENTIALS_FILE}"

  # head reads a fixed number of bytes and tr reads to the end: no SIGPIPE under pipefail.
  # 32 characters out of 62 are about 190 bits.
  while [[ "${#value}" -lt 32 ]]; do
    value="$(head -c 512 /dev/urandom | LC_ALL=C tr -dc 'A-Za-z0-9')"
  done
  value="${value:0:32}"

  # printf is a shell builtin: the value is never an argument of a process and never on stdout.
  printf '%s' "${value}" | docker secret create --label app.betula.stack=monitoring "${ADMIN_SECRET}" - >/dev/null
  if ! printf '%s  grafana  user: %s  initial password: %s\n' "$(_ts)" "${user}" "${value}" |
    as_root sh -c 'umask 077; cat >>"$1" && chown root:root "$1" && chmod 0600 "$1"' sh "${CREDENTIALS_FILE}"; then
    # A password nobody can look up is worse than no secret: take it back, the next run starts over.
    docker secret rm "${ADMIN_SECRET}" >/dev/null 2>&1 || true
    value=""
    die "could not write ${CREDENTIALS_FILE}; the secret was removed again"
  fi
  value=""
  CREDENTIALS_WRITTEN=1
  log "created: ${ADMIN_SECRET} (random). The initial login is in ${CREDENTIALS_FILE} (root only, mode 0600): sudo cat ${CREDENTIALS_FILE}"
  if docker volume inspect monitoring_grafana-data >/dev/null 2>&1; then
    warn "volume monitoring_grafana-data already exists: Grafana only reads the secret when it creates its database,"
    warn "so the OLD password still applies. Set a new one: stacks/monitoring-secrets.sh reset-admin-password"
  fi
}

deploy_edge() {
  step "Stack edge (Traefik, socket proxy)"
  local -a files=("${STACKS_DIR}/edge.yml")
  if resolves_here "${WWW_HOST}"; then
    log "${RESOLVE_DETAIL}: adding edge.www.yml (https://${WWW_HOST} -> https://${SITE_HOST})"
    files+=("${STACKS_DIR}/edge.www.yml")
  else
    warn "${RESOLVE_DETAIL}: deploying WITHOUT edge.www.yml (no certificate is ordered for a name that cannot be validated)"
  fi
  if ! resolves_here "${SITE_HOST}"; then
    warn "${RESOLVE_DETAIL}: the certificate order for ${SITE_HOST} will fail until DNS is fixed (see 'certresolver' in stacks/edge.yml for the retry)"
  fi
  deploy_stack edge "${files[@]}"
  wait_for_stack edge
  remove_old_traefik_logs
}

# remove_old_traefik_logs - a stopped task container keeps its Docker log files, and Traefik's from
# before the journald driver (stacks/edge.yml, "logging") hold its access log with visitors'
# addresses in files that only rotate by size. They are history nobody needs: remove them. The
# running task is left alone, as is every container whose log is in the journal already.
remove_old_traefik_logs() {
  local id driver
  while IFS= read -r id; do
    [[ -n "${id}" ]] || continue
    driver="$(docker inspect --format '{{.HostConfig.LogConfig.Type}}' "${id}" 2>/dev/null || true)"
    [[ -n "${driver}" && "${driver}" != "journald" ]] || continue
    if docker rm "${id}" >/dev/null 2>&1; then
      log "removed the stopped Traefik container ${id}: its ${driver} log files held the access log"
    else
      warn "could not remove the stopped Traefik container ${id} (docker rm ${id}): its log files hold the access log"
    fi
  done < <(docker ps -aq --filter "label=com.docker.swarm.service.name=edge_traefik" --filter status=exited --filter status=created --filter status=dead)
}

deploy_placeholder() {
  step "Stack placeholder (static page at https://${SITE_HOST})"
  local owner
  # An instance at another name (canary.betula.app) leaves the placeholder where it is.
  owner="$(app_stack_for_host "${SITE_HOST}")"
  if [[ -n "${owner}" ]]; then
    log "stack ${owner} serves https://${SITE_HOST}: the application owns it, placeholder is not deployed"
    if stack_exists placeholder; then
      warn "stack placeholder is still there; once the application works: docker stack rm placeholder"
    fi
    return 0
  fi
  PLACEHOLDER_CONFIG_REV="$(checksum_of "${CONFIG_DIR}/placeholder/nginx.conf")"
  export PLACEHOLDER_CONFIG_REV
  deploy_stack placeholder "${STACKS_DIR}/placeholder.yml"
  wait_for_stack placeholder
}

deploy_monitoring() {
  step "Stack monitoring (Grafana, Loki, Prometheus, Alloy)"
  local -a files=("${STACKS_DIR}/monitoring.yml")
  local name override missing="" channels=0

  if [[ -z "${GRAFANA_PUBLIC_HOST}" ]]; then
    log "GRAFANA_HOST is empty: Grafana stays off the internet (ssh tunnel, see stacks/monitoring.yml)"
    unset GRAFANA_HOST
  elif resolves_here "${GRAFANA_PUBLIC_HOST}"; then
    log "${RESOLVE_DETAIL}: adding monitoring.public.yml (https://${GRAFANA_PUBLIC_HOST})"
    export GRAFANA_HOST="${GRAFANA_PUBLIC_HOST}"
    files+=("${STACKS_DIR}/monitoring.public.yml")
  else
    warn "${RESOLVE_DETAIL}: deploying WITHOUT monitoring.public.yml; Grafana is reachable through an ssh tunnel only"
    export GRAFANA_HOST="${GRAFANA_PUBLIC_HOST}"
  fi

  # Notification channels: an override joins the deploy when it exists and ALL of its secrets
  # exist. monitoring.smtp.yml ships with the repo (e-mail); monitoring.notify.yml is the
  # operator's copy of monitoring.notify.example.yml (ntfy, webhook or Telegram token).
  for override in monitoring.smtp.yml monitoring.notify.yml; do
    [[ -f "${STACKS_DIR}/${override}" ]] || continue
    missing=""
    while IFS= read -r name; do
      secret_exists "${name}" || missing+="${name} "
    done < <(yaml_external_secrets "${STACKS_DIR}/${override}")
    if [[ -z "${missing}" ]]; then
      log "all secrets of ${override} exist: adding it"
      files+=("${STACKS_DIR}/${override}")
      channels=$((channels + 1))
    else
      log "${override} is left out (missing secrets: ${missing% })"
    fi
  done
  if [[ "${channels}" -eq 0 ]]; then
    warn "no notification channel is wired: alerts only show up inside Grafana (README.md, 'Alert notifications')"
  fi

  # Bind-mounted config is invisible to swarm; a new checksum changes a container label and
  # restarts the four services (and nothing restarts while the files stay the same).
  MONITORING_CONFIG_REV="$(checksum_of "${CONFIG_DIR}/monitoring")"
  export MONITORING_CONFIG_REV
  log "MONITORING_CONFIG_REV=${MONITORING_CONFIG_REV}"
  deploy_stack monitoring "${files[@]}"
  wait_for_stack monitoring
}

report() {
  step "Done"
  local stack
  # cortex is deployed by vps/48-cortex.sh, never here, and listed all the same.
  # shellcheck disable=SC2046  # instance names are single words
  for stack in edge placeholder cortex $(instance_names) monitoring; do
    if stack_exists "${stack}"; then
      docker stack services "${stack}" --format '{{.Name}}  {{.Replicas}}  {{.Image}}' | sed 's/^/  /'
    fi
  done
  if [[ "${CREDENTIALS_WRITTEN}" -eq 1 ]]; then
    log "initial Grafana login: ${CREDENTIALS_FILE} on this server (sudo cat; move it into your password manager, then delete the file)"
  fi
  if [[ "${#FAILED_STACKS[@]}" -gt 0 ]]; then
    die "not converged: ${FAILED_STACKS[*]} (docker service ps output above). Fix the cause and run this script again"
  fi
  log "next: bash ${BETULA_ROOT}/vps/91-verify-stacks.sh"
}

# ---------------------------------------------------------------- main

selected=("$@")
if [[ "${#selected[@]}" -eq 0 ]]; then
  selected=("${ALL_STACKS[@]}")
fi
for name in "${selected[@]}"; do
  if [[ " ${ALL_STACKS[*]} " != *" ${name} "* ]]; then
    die "unknown stack '${name}' (known: ${ALL_STACKS[*]})"
  fi
done

preflight
# In the fixed order, whatever the order of the arguments: Traefik first (it holds the
# certificates), monitoring last (it then finds its required services running).
for name in "${ALL_STACKS[@]}"; do
  if [[ " ${selected[*]} " != *" ${name} "* ]]; then continue; fi
  case "${name}" in
    edge) deploy_edge ;;
    placeholder) deploy_placeholder ;;
    monitoring)
      create_secrets
      deploy_monitoring
      ;;
  esac
done
report
