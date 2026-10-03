#!/usr/bin/env bash
# 91-verify-stacks.sh - read-only audit of what 40-stacks.sh deployed. Run on the server as the
# deploy user (group docker), no sudo needed:
#
#   bash /opt/betula/vps/91-verify-stacks.sh                 # everything
#   bash /opt/betula/vps/91-verify-stacks.sh tls headers     # only some sections
#
# Sections: services http tls headers ports accesslog app cortex canary loki prometheus grafana alerts
# Prints one PASS / WARN / FAIL line per check and exits non-zero when anything FAILed.
# Changes nothing. The only traffic it causes: a few requests to the site (which also put fresh
# lines into Traefik's access log for the Loki check) and queries inside the monitoring stack.
#
# Ports that are not published (Loki 3100, Prometheus 9090, Grafana 3000) are reached with
# "docker exec" into the Prometheus container: it is on the stack network next to them, on the
# "monitoring" overlay next to Traefik's metrics port and on "cortex" next to Radix and Cortex, and
# its image has busybox wget. What Cortex says of itself is asked with "docker exec" into its own
# containers ("cortex status"), and so is what a release of Radix can do ("radix run -h", which
# prints the flags and exits).
#
# What this cannot prove: that the site is reachable from OUTSIDE. The requests below start on
# the server itself and never pass the provider's network or ufw's INPUT rules. From the
# workstation:  curl -sI http://betula.app/   and   curl -sI https://betula.app/
#
# Environment:
#   GRAFANA_HOST   public name of Grafana, default grafana.betula.app (only used when its router is deployed)
set -Eeuo pipefail

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
# shellcheck source=lib-stacks.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-stacks.sh"
betula_init
require_ubuntu
require_cmd docker curl openssl jq
require_swarm_manager

ALL_SECTIONS=(services http tls headers ports accesslog app cortex canary loki prometheus grafana alerts)
GRAFANA_PUBLIC_HOST="${GRAFANA_HOST:-${DEFAULT_GRAFANA_HOST}}"
RULES_FILE="${CONFIG_DIR}/monitoring/grafana/provisioning/alerting/rules.yml"
# Jobs that must be "up": six scraped by Prometheus (config/monitoring/prometheus.yml), two
# pushed by Alloy through remote write (config/monitoring/alloy/config.alloy).
EXPECTED_JOBS=(prometheus traefik loki grafana alloy cortex integrations/unix integrations/cadvisor)
# Ports of the contract that must NOT listen on the host: Traefik ping and metrics, socket proxy,
# Grafana, Loki, Prometheus, Alloy, Radix, Folia, Cortex.
PRIVATE_PORTS=(8081 8082 2375 3000 3100 9090 12345 8090 8080 8100)
# Seconds the follower of Cortex may be behind before this script warns (the alert waits for 300).
CORTEX_LAG_WARN=60
PASSED=0
WARNED=0
FAILED=0
PROM_CID=""
# Set by check_cortex: yes while the stack cortex runs, as 50-app.sh decides it (cortex_look,
# lib-stacks.sh: on the replicas of both instances).
CORTEX_RUNS="no"

pass() {
  PASSED=$((PASSED + 1))
  printf 'PASS  %s\n' "$*"
}
warning() {
  WARNED=$((WARNED + 1))
  printf 'WARN  %s\n' "$*"
}
fail() {
  FAILED=$((FAILED + 1))
  printf 'FAIL  %s\n' "$*"
}
section() { printf '\n-- %s\n' "$*"; }

# ---------------------------------------------------------------- helpers

# Router labels tell which optional override is deployed right now.
www_router_deployed() { [[ -n "$(service_label edge_traefik 'traefik.http.routers.edge-www.rule')" ]]; }
grafana_router_deployed() { [[ -n "$(service_label monitoring_grafana 'traefik.http.routers.monitoring-grafana.rule')" ]]; }

# Sets PROM_CID to the running Prometheus container (empty when there is none).
find_prometheus() {
  local ids
  [[ -z "${PROM_CID}" ]] || return 0
  ids="$(docker ps -q --filter 'label=com.docker.swarm.service.name=monitoring_prometheus' --filter 'status=running' 2>/dev/null || true)"
  PROM_CID="$(sed -n '1p' <<<"${ids}")"
  [[ -n "${PROM_CID}" ]]
}

# mon_get URL -> response body, fetched from inside the monitoring stack's networks.
mon_get() {
  local url=$1 host
  # Docker copies the host's "search ." (systemd-resolved) into every container, and the busybox
  # wget of the Prometheus image then answers "bad address" for a bare service name although
  # nslookup, musl and Go resolve it (seen on this server, 2026-09-20). An absolute name
  # (trailing dot) never goes through the search list. IP literals stay as they are.
  local scheme rest
  if [[ "${url}" =~ ^(https?://)([^/:]+)(.*)$ ]]; then
    # Copied out first: the next "=~" overwrites BASH_REMATCH.
    scheme="${BASH_REMATCH[1]}"
    host="${BASH_REMATCH[2]}"
    rest="${BASH_REMATCH[3]}"
    if [[ "${host}" != *. && ! "${host}" =~ ^[0-9.]+$ ]]; then
      url="${scheme}${host}.${rest}"
    fi
  fi
  docker exec "${PROM_CID}" wget -q -T 10 -O - "${url}" 2>/dev/null
}

urlencode() { jq -rn --arg v "$1" '$v | @uri'; }

# prom_value PROMQL -> first sample value of an instant query (empty: no series).
prom_value() {
  local body
  body="$(mon_get "http://127.0.0.1:9090/api/v1/query?query=$(urlencode "$1")")" || return 1
  jq -r '.data.result[0].value[1] // empty' <<<"${body}"
}

# loki_value LOGQL -> first sample value of an instant metric query (empty: no series).
loki_value() {
  local body
  body="$(mon_get "http://monitoring_loki:3100/loki/api/v1/query?query=$(urlencode "$1")")" || return 1
  jq -r '.data.result[0].value[1] // empty' <<<"${body}"
}

# tls_certificate HOST -> PEM of the certificate served for HOST on 443 (empty: handshake failed).
tls_certificate() {
  local out
  out="$(openssl s_client -connect "$1:443" -servername "$1" </dev/null 2>/dev/null || true)"
  sed -n '/-----BEGIN CERTIFICATE-----/,/-----END CERTIFICATE-----/p' <<<"${out}"
}

# check_certificate HOST - Let's Encrypt production is a PASS, staging a WARN, everything else a FAIL.
check_certificate() {
  local host=$1 pem="" issuer san i
  # An ACME order takes 10 to 60 s after the router appeared, and 40-stacks.sh only waits for the
  # tasks: give a certificate that is still on its way a minute before calling it missing.
  for i in 1 2 3 4 5 6; do
    pem="$(tls_certificate "${host}")"
    [[ -z "${pem}" ]] || break
    if [[ "${i}" -lt 6 ]]; then sleep 10; fi
  done
  if [[ -z "${pem}" ]]; then
    # sniStrict: without a certificate for the name there is no handshake at all.
    fail "${host}: no certificate is served (TLS handshake fails). ACME order failed? docker service logs edge_traefik 2>&1 | grep -i acme"
    return 0
  fi
  issuer="$(openssl x509 -noout -issuer <<<"${pem}" 2>/dev/null | sed 's/^issuer= *//')"
  case "${issuer}" in
    *"(STAGING)"*)
      warning "${host}: certificate from Let's Encrypt STAGING (${issuer}); browsers reject it. Run 40-stacks.sh without ACME_STAGING for production"
      ;;
    *"Let's Encrypt"*)
      pass "${host}: certificate issued by ${issuer}"
      if curl -sS -o /dev/null --max-time 15 "https://${host}/" 2>/dev/null; then
        pass "${host}: the chain is trusted by the system CA store"
      else
        fail "${host}: curl does not trust the chain although the issuer is Let's Encrypt (curl -v https://${host}/)"
      fi
      ;;
    *"TRAEFIK DEFAULT CERT"*)
      fail "${host}: Traefik's self-signed default certificate is served (no ACME certificate, and sniStrict is not active)"
      ;;
    *)
      fail "${host}: unexpected issuer: ${issuer:-<unreadable>}"
      ;;
  esac
  san="$(openssl x509 -noout -ext subjectAltName <<<"${pem}" 2>/dev/null || true)"
  if [[ "${san}" == *"DNS:${host}"* ]]; then
    pass "${host}: the certificate names ${host}"
  else
    fail "${host}: the certificate does not name ${host} (${san//$'\n'/ })"
  fi
  # Traefik renews 30 days before expiry; less than 14 days left means renewal keeps failing.
  if openssl x509 -noout -checkend $((14 * 86400)) <<<"${pem}" >/dev/null 2>&1; then
    pass "${host}: valid for more than 14 days ($(openssl x509 -noout -enddate <<<"${pem}" | sed 's/^notAfter=//'))"
  else
    fail "${host}: expires within 14 days ($(openssl x509 -noout -enddate <<<"${pem}" | sed 's/^notAfter=//')); renewal is failing"
  fi
}

# check_colours NAME - the live one of two colours of one site (blue-green, lib-stacks.sh): every
# other stack routed for its host must have a lower priority, or Traefik picks either.
check_colours() {
  local name=$1 other own priority
  own="$(router_priority "${INSTANCE_STACK}")"
  while IFS= read -r other; do
    [[ -n "${other}" && "${other}" != "${INSTANCE_STACK}" ]] || continue
    priority="$(router_priority "${other}")"
    if [[ "${priority}" -lt "${own}" ]]; then
      pass "${name}: serves https://${INSTANCE_HOST} (router priority ${own}); ${other} is the standby (${priority}; the host goes to it with: bash ${BETULA_ROOT}/vps/55-switch.sh ${other})"
    else
      fail "${name} and ${other} are routed for ${INSTANCE_HOST} with the same priority (${own}): Traefik picks either. bash ${BETULA_ROOT}/vps/55-switch.sh <the one that should serve>"
    fi
  done < <(app_stacks_for_host "${INSTANCE_HOST}")
}

# check_standby NAME LIVE - an instance whose host another one serves (blue-green): its public URL
# reaches LIVE, so it is asked directly, past Traefik - what 55-switch.sh asks before a switch.
check_standby() {
  local name=$1 live=$2 address path code
  pass "${name}: standby for https://${INSTANCE_HOST}, which ${live} serves (router priority $(router_priority "${INSTANCE_STACK}") < $(router_priority "${live}"); switch: bash ${BETULA_ROOT}/vps/55-switch.sh ${name})"
  address="$(folia_address "${INSTANCE_STACK}")"
  if [[ -z "${address}" ]]; then
    fail "${name}: no running container of ${INSTANCE_STACK}_folia to ask (docker service ps --no-trunc ${INSTANCE_STACK}_folia)"
    return 0
  fi
  code="$(curl -sS -o /dev/null --max-time 15 -w '%{http_code}' "http://${address}:8080/livez" 2>/dev/null || true)"
  if [[ "${code}" == "200" ]]; then pass "${name}: /livez -> 200 (asked directly at ${address}:8080)"; else
    fail "${name}: /livez -> ${code:-no answer} (asked directly at ${address}:8080; docker service logs ${INSTANCE_STACK}_folia)"
  fi
  code="$(curl -sS -o /dev/null --max-time 15 -w '%{http_code}' "http://${address}:8080/healthz" 2>/dev/null || true)"
  case "${code}" in
    200) pass "${name}: /healthz -> 200 (asked directly: a snapshot is served and Radix was heard from)" ;;
    503) warning "${name}: /healthz -> 503 (asked directly): no snapshot yet, or no answer from its Radix. Not ready to take ${INSTANCE_HOST} over" ;;
    *) fail "${name}: /healthz -> ${code:-no answer} (asked directly at ${address}:8080)" ;;
  esac
  if snapshot_outdated "${INSTANCE_STACK}"; then
    fail "${name}: its catalog is older than the schema its build reads (\"snapshot.outdated\" in the log of ${INSTANCE_STACK}_folia): build and export a new snapshot in ${INSTANCE_STACK}_radix before a switch (docker exec <its radix container> /bin/radix build --db /data/radix.db, then /bin/radix export --db /data/radix.db --out /data/snapshot)"
  fi
  if [[ "${INSTANCE_GATE}" == "on" ]]; then
    for path in /api/db /api/status; do
      code="$(curl -sS -o /dev/null --max-time 15 -w '%{http_code}' "http://${address}:8080${path}" 2>/dev/null || true)"
      if [[ "${code}" == "401" ]]; then pass "${name}: ${path} -> 401 without the password (asked directly)"; else
        fail "${name}: ${path} -> ${code:-no answer} without the password (asked directly), expected 401: a switch would make the catalog public"
      fi
    done
  fi
}

# canary_follows_master - true while betula-canary.timer is on (vps/60-canary.sh, README.md section 12).
canary_follows_master() { systemctl is-active --quiet betula-canary.timer 2>/dev/null; }

# header_value HEADERS NAME -> value of the LAST response header NAME (case-insensitive), without CR.
header_value() {
  awk -v want="$(tr 'A-Z' 'a-z' <<<"$2")" '
    { sub(/\r$/, "") }
    { i = index($0, ":"); if (i == 0) next
      name = tolower(substr($0, 1, i - 1)); value = substr($0, i + 1); sub(/^[ \t]+/, "", value)
      if (name == want) last = value }
    END { print last }' <<<"$1"
}

# ---------------------------------------------------------------- sections

check_services() {
  section "services (replicas running and healthy, no update in flight)"
  local stack svc state found site_owner
  site_owner="$(app_stack_for_host "${SITE_HOST}")"
  # shellcheck disable=SC2046  # instance names are single words
  for stack in edge placeholder cortex $(instance_names) monitoring; do
    if ! stack_exists "${stack}"; then
      case "${stack}" in
        edge | monitoring) fail "stack ${stack} is not deployed (vps/40-stacks.sh)" ;;
        cortex) warning "stack cortex is not deployed (deploy/ship-cortex.sh): a Radix that crawls fetches from the university directly (section cortex)" ;;
        placeholder) [[ -n "${site_owner}" ]] || fail "neither stack placeholder nor an instance of the application serves https://${SITE_HOST}: nothing answers there" ;;
      esac
      continue
    fi
    found=0
    while IFS= read -r svc; do
      [[ -n "${svc}" ]] || continue
      found=1
      state="$(service_state "${svc}")"
      case "${state}" in
        ok\ *) pass "${svc}: ${state#* }" ;;
        *) fail "${svc}: ${state#* }  (docker service ps --no-trunc ${svc})" ;;
      esac
    done < <(stack_services "${stack}")
    [[ "${found}" -eq 1 ]] || fail "stack ${stack} has no services"
  done
  if [[ -n "${site_owner}" ]] && stack_exists placeholder; then
    warning "stack placeholder still runs although stack ${site_owner} serves https://${SITE_HOST}; once the application works: docker stack rm placeholder"
  fi
  if www_router_deployed; then pass "override edge.www.yml is deployed (https://${WWW_HOST} redirects)"; else
    warning "override edge.www.yml is not deployed: https://${WWW_HOST} is not served (40-stacks.sh adds it when the name resolves here)"
  fi
  if grafana_router_deployed; then pass "override monitoring.public.yml is deployed (https://${GRAFANA_PUBLIC_HOST})"; else
    warning "override monitoring.public.yml is not deployed: Grafana is reachable through an ssh tunnel only"
  fi
}

check_http() {
  section "http -> https"
  local out code target
  out="$(curl -sS -o /dev/null --max-time 15 -w '%{http_code} %{redirect_url}' "http://${SITE_HOST}/some/path?x=1" 2>&1 || true)"
  code="${out%% *}"
  target="${out#* }"
  if [[ "${code}" =~ ^30[18]$ && "${target}" == "https://${SITE_HOST}/some/path?x=1" ]]; then
    pass "http://${SITE_HOST}/some/path?x=1 -> ${code} ${target}"
  else
    fail "http://${SITE_HOST}/ does not redirect permanently to https with the same path (got: ${out})"
  fi
  if www_router_deployed; then
    out="$(curl -sS -k -o /dev/null --max-time 15 -w '%{http_code} %{redirect_url}' "https://${WWW_HOST}/some/path?x=1" 2>&1 || true)"
    code="${out%% *}"
    target="${out#* }"
    if [[ "${code}" =~ ^30[18]$ && "${target}" == "https://${SITE_HOST}/some/path?x=1" ]]; then
      pass "https://${WWW_HOST}/some/path?x=1 -> ${code} ${target}"
    else
      fail "https://${WWW_HOST}/ does not redirect to https://${SITE_HOST}/ with the same path (got: ${out})"
    fi
  fi
}

check_tls() {
  section "certificates (Let's Encrypt, not Traefik's default)"
  local out
  check_certificate "${SITE_HOST}"
  if www_router_deployed; then check_certificate "${WWW_HOST}"; fi
  if grafana_router_deployed; then check_certificate "${GRAFANA_PUBLIC_HOST}"; fi
  # TLS options "default": nothing older than TLS 1.2, and no certificate for an unknown name.
  # Security level 0, or this openssl would refuse TLS 1.1 by itself and the test would prove nothing.
  out="$(openssl s_client -connect "${SITE_HOST}:443" -servername "${SITE_HOST}" -tls1_1 -cipher 'ALL:@SECLEVEL=0' </dev/null 2>&1 || true)"
  if [[ "${out}" == *"BEGIN CERTIFICATE"* ]]; then
    fail "TLS 1.1 is accepted (config/traefik/dynamic/tls.yml not loaded?)"
  elif [[ "${out}" == *"alert protocol version"* || "${out}" == *"tlsv1 alert"* || "${out}" == *"wrong version number"* || "${out}" == *"unsupported protocol"* ]]; then
    pass "TLS 1.1 is refused by the server"
  else
    warning "could not test TLS 1.1 (this openssl build does not offer it): $(sed -n '1p' <<<"${out}")"
  fi
  out="$(openssl s_client -connect "${SITE_HOST}:443" -servername "unknown.invalid" </dev/null 2>/dev/null || true)"
  if [[ "${out}" == *"BEGIN CERTIFICATE"* ]]; then
    fail "an unknown SNI name gets a certificate (sniStrict is not active: config/traefik/dynamic/tls.yml not loaded?)"
  else
    pass "an unknown SNI name gets no certificate (sniStrict)"
  fi
}

check_headers() {
  section "security headers (middleware secure-headers@file) on https://${SITE_HOST}/"
  local headers status value
  # -k: whether the chain is trusted is the tls section's verdict; the headers do not depend on it.
  headers="$(curl -sS -k -o /dev/null -D - --max-time 15 "https://${SITE_HOST}/" 2>/dev/null || true)"
  if [[ -z "${headers}" ]]; then
    fail "no answer from https://${SITE_HOST}/"
    return 0
  fi
  status="$(sed -n '1p' <<<"${headers}" | tr -d '\r')"
  if [[ "${status}" == *" 200"* ]]; then pass "status: ${status}"; else warning "status: ${status} (expected 200 from the placeholder or the application)"; fi

  value="$(header_value "${headers}" strict-transport-security)"
  if [[ "${value}" == *"max-age=31536000"* && "${value,,}" == *"includesubdomains"* && "${value,,}" != *"preload"* ]]; then
    pass "strict-transport-security: ${value}"
  else
    fail "strict-transport-security is '${value:-<missing>}' (wanted max-age=31536000; includeSubDomains, no preload)"
  fi
  value="$(header_value "${headers}" x-content-type-options)"
  if [[ "${value,,}" == "nosniff" ]]; then pass "x-content-type-options: ${value}"; else fail "x-content-type-options is '${value:-<missing>}'"; fi
  value="$(header_value "${headers}" x-frame-options)"
  if [[ "${value^^}" == "SAMEORIGIN" || "${value^^}" == "DENY" ]]; then pass "x-frame-options: ${value}"; else fail "x-frame-options is '${value:-<missing>}'"; fi
  value="$(header_value "${headers}" referrer-policy)"
  if [[ -n "${value}" ]]; then pass "referrer-policy: ${value}"; else fail "referrer-policy is missing"; fi
  value="$(header_value "${headers}" permissions-policy)"
  if [[ -n "${value}" ]]; then pass "permissions-policy is set"; else fail "permissions-policy is missing"; fi
  value="$(header_value "${headers}" server)"
  if [[ -z "${value}" ]]; then pass "no Server header"; else warning "Server header present: ${value}"; fi
}

check_ports() {
  section "published ports (only Traefik, only the public ports)"
  local svc ports want have addr port p
  local -a addrs=(127.0.0.1)
  load_public_ports "${BETULA_FILES_DIR}/public-ports.conf"

  # From the service spec, not from the PORTS column of "docker service ls": that column only
  # lists ingress-mode ports, and a host-mode port is exactly what a copied Traefik example brings.
  while IFS= read -r svc; do
    [[ -n "${svc}" && "${svc}" != "edge_traefik" ]] || continue
    ports="$(docker service inspect "${svc}" --format '{{range .Endpoint.Spec.Ports}}{{.PublishedPort}}/{{.Protocol}}/{{.PublishMode}} {{end}}' 2>/dev/null || true)"
    if [[ -n "${ports// /}" ]]; then
      fail "${svc} publishes ${ports% }: only edge_traefik may publish ports"
    fi
  done < <(docker service ls --format '{{.Name}}')

  want="$(
    for p in ${PUBLIC_TCP_PORTS}; do printf '%s/tcp/host\n' "${p}"; done
    for p in ${PUBLIC_UDP_PORTS}; do printf '%s/udp/host\n' "${p}"; done
  )"
  want="$(sort <<<"${want}" | tr '\n' ' ')"
  have="$(docker service inspect edge_traefik --format '{{range .Endpoint.Spec.Ports}}{{.PublishedPort}}/{{.Protocol}}/{{.PublishMode}}{{"\n"}}{{end}}' 2>/dev/null | sed '/^$/d' | sort | tr '\n' ' ' || true)"
  if [[ "${have}" == "${want}" ]]; then
    pass "edge_traefik publishes exactly: ${have% } (host mode = real client addresses; same list as vps/files/public-ports.conf)"
  else
    fail "edge_traefik publishes '${have% }', the contract says '${want% }'"
  fi

  # "Not published" checked the hard way: nothing on the host accepts a connection on these ports.
  # Without ip (iproute2) only on 127.0.0.1: default_ipv4 needs it, and a host that lacks it
  # should get a warning here, not lose every section after this one (verify2 D6).
  if have_cmd ip; then
    addr="$(default_ipv4)"
    if [[ -n "${addr}" ]]; then addrs+=("${addr}"); fi
  else
    warning "ip (package iproute2) is missing: the private ports are only tried on 127.0.0.1, not on the default IPv4 address (sudo apt-get install iproute2)"
  fi
  have=""
  for addr in "${addrs[@]}"; do
    for port in "${PRIVATE_PORTS[@]}"; do
      if timeout 3 bash -c ": </dev/tcp/${addr}/${port}" 2>/dev/null; then
        have+="${addr}:${port} "
      fi
    done
  done
  if [[ -z "${have}" ]]; then
    pass "nothing listens on the host on ${PRIVATE_PORTS[*]} (Traefik ping/metrics, socket proxy, Grafana, Loki, Prometheus, Alloy, Radix, Folia, Cortex)"
  else
    fail "reachable on the host: ${have% } - a service publishes a port it should not (docker service ls; sudo ss -tlnp)"
  fi
}

check_accesslog() {
  section "access log (through the journal, which deletes it after 7 days; nowhere else on the host)"
  local driver id line state kept=0
  driver="$(docker service inspect edge_traefik --format '{{with .Spec.TaskTemplate.LogDriver}}{{.Name}}{{end}}' 2>/dev/null || true)"
  if [[ "${driver}" == "journald" ]]; then
    pass "edge_traefik logs through journald (7 days: vps/files/journald-betula.conf, as the privacy notice says)"
  else
    fail "edge_traefik logs through '${driver:-the daemon default}', not journald: its access log sits in Docker's local files, which rotate by size only (bash ${BETULA_ROOT}/vps/40-stacks.sh edge)"
  fi
  # A container's driver is fixed when it is created: an older task may still have local files.
  while IFS= read -r id; do
    [[ -n "${id}" ]] || continue
    line="$(docker inspect --format '{{.State.Status}} {{.HostConfig.LogConfig.Type}}' "${id}" 2>/dev/null || true)"
    state="${line%% *}"
    driver="${line#* }"
    [[ -n "${line}" && "${driver}" != "journald" ]] || continue
    kept=1
    if [[ "${state}" == "running" ]]; then
      fail "the running Traefik container ${id} logs to Docker's ${driver} files: bash ${BETULA_ROOT}/vps/40-stacks.sh edge"
    else
      fail "the ${state} Traefik container ${id} keeps its access log in Docker's ${driver} files: docker rm ${id} (40-stacks.sh edge does it)"
    fi
  done < <(docker ps -aq --filter "label=com.docker.swarm.service.name=edge_traefik")
  if [[ "${kept}" -eq 0 ]]; then pass "no Traefik container keeps log files of its own"; fi
}

# service_env SERVICE NAME -> the value the service is given for the variable NAME (nothing without).
service_env() {
  docker service inspect "$1" --format '{{range .Spec.TaskTemplate.ContainerSpec.Env}}{{println .}}{{end}}' 2>/dev/null |
    sed -n "s#^$2=##p" || true
}

# check_models INSTANCE - the semantic search: does the instance run the models of models.lock?
# MODELS_STORE_READY is models_ready's verdict, asked once for all instances.
check_models() {
  local name=$1 radix folia
  radix="$(service_env "${INSTANCE_STACK}_radix" RADIX_EMBED_MODEL)"
  folia="$(service_env "${INSTANCE_STACK}_folia" FOLIA_SEMANTIC_MODEL)"
  if [[ -z "${radix}${folia}" ]]; then
    if [[ "${MODELS_STORE_READY}" == "yes" ]]; then
      warning "${name}: runs without the semantic search, though the model store holds the models of models.lock (bash ${BETULA_ROOT}/vps/50-app.sh ${name})"
    else
      warning "${name}: runs without the semantic search: ${MODELS_DETAIL}"
    fi
  elif [[ "${radix}" == "/models/${MODEL_PASSAGE}" && "${folia}" == "/models/${MODEL_QUERY}" ]]; then
    pass "${name}: runs the models of models.lock (passage ${MODEL_PASSAGE:0:16}, query ${MODEL_QUERY:0:16})"
  else
    warning "${name}: runs other models than models.lock names (Radix ${radix##*/}, Folia ${folia##*/}); its next deploy brings the lock's (bash ${BETULA_ROOT}/vps/50-app.sh ${name})"
  fi
}

check_app() {
  section "application (every instance in stacks/*.env: router, release, crawling, models, certificate, alive, closed testing)"
  local name url rule radix_tag folia_tag radix_args code out body path live deployed=0
  MODELS_STORE_READY="no"
  read_model_lock
  if models_ready; then
    MODELS_STORE_READY="yes"
    pass "the model store holds the models of models.lock, intact (bash ${BETULA_ROOT}/vps/models.sh status)"
  else
    warning "${MODELS_DETAIL}"
  fi
  while IFS= read -r name; do
    [[ -n "${name}" ]] || continue
    load_instance "${name}"
    url="https://${INSTANCE_HOST}"
    if ! stack_exists "${INSTANCE_STACK}"; then
      # Following master, canary keeps one colour: the other one is where the next release goes.
      if is_canary_colour "${name}" && canary_follows_master; then
        pass "instance ${name} is not deployed: canary follows master and keeps one colour, the next release goes here (section canary)"
      else
        warning "instance ${name} (${url}) is not deployed (deploy/ship.sh ${name})"
      fi
      continue
    fi
    deployed=1

    rule="$(service_label "${INSTANCE_STACK}_folia" "traefik.http.routers.${INSTANCE_STACK}-folia.rule")"
    if [[ "${rule}" == "Host(\`${INSTANCE_HOST}\`)" ]]; then pass "${name}: routed for ${INSTANCE_HOST}"; else
      fail "${name}: the router rule is '${rule:-<none>}', ${name}.env says ${INSTANCE_HOST} (bash ${BETULA_ROOT}/vps/50-app.sh ${name})"
    fi
    radix_tag="$(docker service inspect "${INSTANCE_STACK}_radix" --format '{{.Spec.TaskTemplate.ContainerSpec.Image}}' 2>/dev/null || true)"
    folia_tag="$(docker service inspect "${INSTANCE_STACK}_folia" --format '{{.Spec.TaskTemplate.ContainerSpec.Image}}' 2>/dev/null || true)"
    radix_tag="${radix_tag%%@*}"
    folia_tag="${folia_tag%%@*}"
    if [[ -n "${folia_tag}" && "${radix_tag##*:}" == "${folia_tag##*:}" ]]; then pass "${name}: both services run release ${folia_tag##*:}"; else
      warning "${name}: the services run different releases (${radix_tag:-no radix} / ${folia_tag:-no folia})"
    fi

    # Does Radix do what the instance's file promises? Offline it is started as "serve-snapshot"
    # (no crawl, no cycle); online it runs the image's own "run".
    radix_args="$(docker service inspect "${INSTANCE_STACK}_radix" --format '{{join .Spec.TaskTemplate.ContainerSpec.Args " "}}' 2>/dev/null || true)"
    if [[ "${INSTANCE_CRAWL}" == "off" ]]; then
      if [[ "${radix_args}" == serve-snapshot* ]]; then
        pass "${name}: Radix is offline as ${name}.env says (it serves its snapshot and fetches nothing from the university)"
        warning "${name}: the catalog does not change while Radix is offline (RADIX_CRAWL=on in ${name}.env, sync, 50-app.sh ${name} brings it back)"
      else
        fail "${name}: ${name}.env says RADIX_CRAWL=off, but Radix runs the command \"${radix_args:-run}\" and CRAWLS (bash ${BETULA_ROOT}/vps/50-app.sh ${name})"
      fi
    elif [[ -z "${radix_args}" ]]; then
      pass "${name}: Radix is online (it keeps the catalog fresh)"
    else
      fail "${name}: ${name}.env says RADIX_CRAWL=on, but Radix runs the command \"${radix_args}\" (bash ${BETULA_ROOT}/vps/50-app.sh ${name})"
    fi

    check_models "${name}"

    # Blue-green: of two instances with one host, the public URL only reaches the live one.
    live="$(app_stack_for_host "${INSTANCE_HOST}")"
    if [[ "${live}" != "${INSTANCE_STACK}" ]]; then
      check_standby "${name}" "${live}"
      continue
    fi
    check_colours "${name}"

    check_certificate "${INSTANCE_HOST}"

    # -k: whether the chain is trusted is check_certificate's verdict.
    code="$(curl -sS -k -o /dev/null --max-time 15 -w '%{http_code}' "${url}/livez" 2>/dev/null || true)"
    if [[ "${code}" == "200" ]]; then pass "${url}/livez -> 200 (the web server answers through Traefik)"; else
      fail "${url}/livez -> ${code:-no answer} (docker service ps --no-trunc ${INSTANCE_STACK}_folia; docker service logs edge_traefik)"
    fi
    code="$(curl -sS -k -o /dev/null --max-time 15 -w '%{http_code}' "${url}/healthz" 2>/dev/null || true)"
    case "${code}" in
      200) pass "${url}/healthz -> 200 (a snapshot is served and Radix was heard from)" ;;
      503) warning "${url}/healthz -> 503: no snapshot yet, or no answer from Radix for 6 h. A fresh Radix needs hours for its first export (curl -s ${url}/healthz; docker service logs ${INSTANCE_STACK}_radix)" ;;
      *) fail "${url}/healthz -> ${code:-no answer}" ;;
    esac

    if [[ "${INSTANCE_GATE}" == "on" ]]; then
      # Closed testing: a page leads to the login page, nothing else answers, crawlers are sent away.
      out="$(curl -sS -k -o /dev/null --max-time 15 -H 'Accept: text/html' -w '%{http_code} %{redirect_url}' "${url}/catalog" 2>/dev/null || true)"
      if [[ "${out}" == "302 ${url}/access?next=%2Fcatalog" ]]; then pass "${name}: a page leads to the login page (${out})"; else
        fail "${name}: ${url}/catalog answers '${out}' instead of leading to /access: closed testing is NOT in force"
      fi
      for path in /api/db /api/status /sitemap.xml; do
        code="$(curl -sS -k -o /dev/null --max-time 15 -w '%{http_code}' "${url}${path}" 2>/dev/null || true)"
        if [[ "${code}" == "401" ]]; then pass "${name}: ${path} -> 401 without the password"; else
          fail "${name}: ${path} -> ${code:-no answer} without the password, expected 401: the catalog is public"
        fi
      done
      body="$(curl -sS -k --max-time 15 "${url}/robots.txt" 2>/dev/null || true)"
      if [[ "${body}" == *"Disallow: /"$'\n'* || "${body}" == *"Disallow: /" ]] && [[ "${body}" != *"Sitemap:"* ]]; then
        pass "${name}: robots.txt turns crawlers away"
      else
        fail "${name}: robots.txt does not say 'Disallow: /' (${body//$'\n'/ | })"
      fi
      body="$(curl -sS -k --max-time 15 "${url}/access" 2>/dev/null || true)"
      if [[ "${body}" == *'type="password"'* ]]; then pass "${name}: ${url}/access shows the login form"; else
        fail "${name}: ${url}/access shows no login form"
      fi
    else
      warning "${name}: closed testing is off (${name}.env): ${url} is open to everybody"
      code="$(curl -sS -k -o /dev/null --max-time 15 -H 'Accept: text/html' -w '%{http_code}' "${url}/" 2>/dev/null || true)"
      if [[ "${code}" == "200" || "${code}" == "503" ]]; then pass "${name}: ${url}/ -> ${code}"; else fail "${name}: ${url}/ -> ${code:-no answer}"; fi
    fi
  done < <(instance_names)
  if [[ "${deployed}" -eq 0 ]]; then
    warning "no instance of the application is deployed (from the workstation: deploy/ship.sh canary)"
  fi
}

# mount_source SERVICE TARGET -> the source of the service's mount at TARGET (a volume's full
# name, a host path; nothing without such a mount).
mount_source() {
  docker service inspect "$1" --format '{{range .Spec.TaskTemplate.ContainerSpec.Mounts}}{{.Target}} {{.Source}}{{println}}{{end}}' 2>/dev/null |
    awk -v target="$2" '$1 == target { print $2; exit }' || true
}

# Set by read_cortex_roles, per instance (a, b): what its "cortex status" says. CX_ROLE is leader
# or follower, empty when it does not answer. Of a follower: CX_LAG is its lag in whole seconds,
# CX_STATE its state ("-" on the leader), CX_BEHIND the journal entries it has not applied and
# CX_MISSING the blobs its index names that it has not fetched yet ("?" where it does not say).
declare -A CX_ROLE=() CX_EPOCH=() CX_SEQ=() CX_LAG=() CX_STATE=() CX_BEHIND=() CX_MISSING=()

# read_cortex_roles - asks both instances. "cortex status" prints the JSON of GET /status; it is
# asked inside the instance's own container, since Cortex publishes no port (stacks/cortex.yml).
read_cortex_roles() {
  local x cid body line role epoch seq lag state behind missing
  for x in a b; do
    CX_ROLE[$x]="" CX_EPOCH[$x]="" CX_SEQ[$x]="" CX_LAG[$x]="" CX_STATE[$x]="" CX_BEHIND[$x]="" CX_MISSING[$x]=""
    cid="$(service_container "cortex_${x}")"
    [[ -n "${cid}" ]] || continue
    body="$(timeout 20 docker exec "${cid}" /bin/cortex status 2>/dev/null || true)"
    line="$(jq -r 'select(type == "object") | [.role, .epoch, .seq, ((.follower.lag_seconds // 0) | if type == "number" then floor else . end), ((.follower.state // "") | if . == "" then "-" else . end), .follower.lag_entries, .follower.blobs_missing] | map(tostring) | join(" ")' <<<"${body}" 2>/dev/null || true)"
    role="" epoch="" seq="" lag="" state="" behind="" missing=""
    read -r role epoch seq lag state behind missing <<<"${line}" || true
    if [[ "${role}" =~ ^(leader|follower)$ && "${epoch}" =~ ^[0-9]+$ && "${seq}" =~ ^[0-9]+$ ]]; then
      [[ "${state}" =~ ^[a-z_-]+$ ]] || state="?"
      [[ "${behind}" =~ ^[0-9]+$ ]] || behind="?"
      [[ "${missing}" =~ ^[0-9]+$ ]] || missing="?"
      CX_ROLE[$x]="${role}" CX_EPOCH[$x]="${epoch}" CX_SEQ[$x]="${seq}" CX_LAG[$x]="${lag}" CX_STATE[$x]="${state}"
      CX_BEHIND[$x]="${behind}" CX_MISSING[$x]="${missing}"
    fi
  done
}

# Set by look_cortex: the instances that lead at its last look (CX_LEADING), and the epoch and
# sequence number the one leader had at the look before, 2 s earlier (CX_PREV_EPOCH,
# CX_PREV_SEQ; empty when that look did not see the same single leader).
CX_LEADING=()
CX_PREV_EPOCH=""
CX_PREV_SEQ=""

# cortex_in_step X -> true when cortex_X follows and has caught up as 48-cortex.sh wants it
# before it hands the lead over (caught_up there): "following" or "catching_up", less than 1 s
# behind, on the leader's epoch and at least at the sequence number the leader had at the look
# before, and 0 blobs missing. Not "following, 0 entries behind": under steady writes a follower
# that keeps up flips between the two states on every batch and is always a few entries behind
# (verify2 E2E-6), so that said "not caught up" about half of the time. The blobs: a follower
# that started again from a copy of the leader's index fetches them afterwards, while it is at
# the leader's sequence number already; as leader it could not read the files whose blob it lacks.
cortex_in_step() {
  [[ "${CX_ROLE[$1]}" == "follower" && "${CX_STATE[$1]}" =~ ^(following|catching_up)$ &&
    "${CX_LAG[$1]}" == "0" && "${CX_MISSING[$1]}" == "0" &&
    -n "${CX_PREV_SEQ}" && "${CX_EPOCH[$1]}" == "${CX_PREV_EPOCH}" && "${CX_SEQ[$1]}" -ge "${CX_PREV_SEQ}" ]]
}

# look_cortex - asks both instances (read_cortex_roles) up to four times, 2 s apart, until one
# leads and the other is no follower or is in step (cortex_in_step, which needs the leader's
# sequence number of the look before). The two are asked one after the other, so a takeover in
# between can show both or neither, and a follower may be a batch behind at any one look: what
# is not one leader with a follower in step is asked again.
look_cortex() {
  local i lead y prev_lead=""
  CX_PREV_EPOCH="" CX_PREV_SEQ=""
  for i in 1 2 3 4; do
    read_cortex_roles
    read -r -a CX_LEADING <<<"$(cortex_leaders)" || true
    lead=""
    if [[ "${#CX_LEADING[@]}" -eq 1 ]]; then
      lead="${CX_LEADING[0]}"
      y="b"
      if [[ "${lead}" == "b" ]]; then y="a"; fi
      [[ "${prev_lead}" == "${lead}" ]] || CX_PREV_EPOCH="" CX_PREV_SEQ=""
      if [[ "${CX_ROLE[$y]}" != "follower" ]] || cortex_in_step "${y}"; then return 0; fi
    fi
    [[ "${i}" -lt 4 ]] || return 0
    prev_lead="${lead}"
    CX_PREV_EPOCH="${lead:+${CX_EPOCH[$lead]}}" CX_PREV_SEQ="${lead:+${CX_SEQ[$lead]}}"
    sleep 2
  done
}

# cortex_leaders -> the instances that say they lead, on one line.
cortex_leaders() {
  local x
  for x in a b; do
    if [[ "${CX_ROLE[$x]}" == "leader" ]]; then printf '%s ' "${x}"; fi
  done
  return 0
}

# check_radix_ways - every instance's Radix on internal networks only (owner, 2026-10-02: no way to
# the internet but Cortex, where possible), with <stack>_egress exactly where 50-app.sh gives it:
# RADIX_CRAWL=on and (Cortex does not run, or the release of Radix cannot fetch through it, or the
# secret gemini-api-key exists). CORTEX_RUNS is check_cortex's verdict. Read from the service
# specs (what swarm was told), and what the release can do from its own container: "radix run -h"
# prints the flags of run and exits, and one from before Cortex has no --cortex (it ignores
# RADIX_CORTEX_URL and fetches directly).
check_radix_ways() {
  local name net internal nets egress want cortex_url gemini=no problems support cid
  if secret_exists gemini-api-key; then gemini=yes; fi
  while IFS= read -r name; do
    [[ -n "${name}" ]] || continue
    load_instance "${name}"
    docker service inspect "${INSTANCE_STACK}_radix" >/dev/null 2>&1 || continue
    nets="" egress=no problems=0
    while read -r net internal; do
      [[ -n "${net}" ]] || continue
      nets+="${net} "
      case "${net}" in
        monitoring | "${INSTANCE_STACK}_default")
          fail "${name}: Radix is on ${net}, which is not internal: a way to the internet. Deployed from a betula.yml from before Cortex? bash ${BETULA_ROOT}/vps/50-app.sh ${name}"
          problems=1
          ;;
        "${INSTANCE_STACK}_egress")
          egress=yes
          ;;
        *)
          if [[ "${internal}" != "true" ]]; then
            fail "${name}: Radix is on ${net}, which is not internal (${internal}): a way to the internet no file of stacks/ means (docker service inspect ${INSTANCE_STACK}_radix)"
            problems=1
          fi
          ;;
      esac
    done < <(service_networks "${INSTANCE_STACK}_radix")
    if [[ " ${nets}" != *" cortex "* || " ${nets}" != *" ${INSTANCE_STACK}_snapshot "* ]]; then
      fail "${name}: Radix is on '${nets% }', not on cortex (Cortex, Prometheus) and ${INSTANCE_STACK}_snapshot (Folia): bash ${BETULA_ROOT}/vps/50-app.sh ${name}"
      problems=1
    fi
    if [[ " $(service_networks "${INSTANCE_STACK}_folia" | awk '{ print $1 }' | tr '\n' ' ')" == *" cortex "* ]]; then
      fail "${name}: Folia is on the network cortex, where every instance's Radix is \"radix\": it may download another instance's snapshot (stacks/betula.yml)"
      problems=1
    fi

    cortex_url="$(service_env "${INSTANCE_STACK}_radix" RADIX_CORTEX_URL)"
    support=unknown
    if [[ "${INSTANCE_CRAWL}" == "on" ]]; then
      cid="$(service_container "${INSTANCE_STACK}_radix")"
      if [[ -n "${cid}" ]]; then
        support="$( { timeout 20 docker exec "${cid}" /bin/radix run -h 2>&1 || true; } | radix_cortex_support)"
      fi
    fi
    want=no
    if [[ "${INSTANCE_CRAWL}" == "on" && ( "${CORTEX_RUNS}" == "no" || "${gemini}" == "yes" || "${support}" == "no" ) ]]; then want=yes; fi
    if [[ "${INSTANCE_CRAWL}" == "on" && -n "${cortex_url}" && "${support}" == "no" ]]; then
      if [[ "${egress}" == "yes" ]]; then
        warning "${name}: Radix was given RADIX_CORTEX_URL, but its release cannot fetch through Cortex (one from before it): it fetches from the university directly, through ${INSTANCE_STACK}_egress. bash ${BETULA_ROOT}/vps/50-app.sh ${name} deploys it as what it is; a newer release fetches through Cortex (deploy/ship.sh ${name})"
      else
        fail "${name}: Radix was given RADIX_CORTEX_URL, but its release cannot fetch through Cortex (one from before it): it fetches directly and has no way out, so every fetch fails. bash ${BETULA_ROOT}/vps/50-app.sh ${name} gives it one; a newer release fetches through Cortex (deploy/ship.sh ${name})"
      fi
      problems=1
    elif [[ "${INSTANCE_CRAWL}" == "on" && -n "${cortex_url}" && "${CORTEX_RUNS}" == "no" ]]; then
      fail "${name}: Radix fetches through Cortex (RADIX_CORTEX_URL=${cortex_url}), which does not run: every fetch fails. Bring Cortex back (bash ${BETULA_ROOT}/vps/48-cortex.sh), or deploy without it: bash ${BETULA_ROOT}/vps/50-app.sh ${name}"
      problems=1
    elif [[ "${INSTANCE_CRAWL}" == "on" && -z "${cortex_url}" && "${CORTEX_RUNS}" == "yes" && "${support}" != "no" ]]; then
      warning "${name}: Cortex runs, but Radix fetches from the university directly: deployed before Cortex ran (bash ${BETULA_ROOT}/vps/50-app.sh ${name})"
    fi
    if [[ "${egress}" == "yes" ]]; then
      if [[ "${INSTANCE_CRAWL}" == "off" ]]; then
        fail "${name}: Radix is offline, but on ${INSTANCE_STACK}_egress: a way to the internet it must not have (bash ${BETULA_ROOT}/vps/50-app.sh ${name})"
      elif [[ "${want}" == "no" ]]; then
        warning "${name}: Radix is on ${INSTANCE_STACK}_egress although Cortex runs and there is no secret gemini-api-key: deployed before Cortex ran (bash ${BETULA_ROOT}/vps/50-app.sh ${name})"
      elif [[ "${support}" == "no" ]]; then
        if [[ -z "${cortex_url}" ]]; then
          warning "${name}: Radix reaches the internet through ${INSTANCE_STACK}_egress and fetches from the university directly: its release cannot fetch through Cortex (one from before it); a newer release does (deploy/ship.sh ${name})"
        fi
      elif [[ -n "${cortex_url}" ]]; then
        warning "${name}: Radix fetches through Cortex, and reaches the internet through ${INSTANCE_STACK}_egress for Gemini (the secret gemini-api-key exists; Gemini does not go through Cortex)"
      else
        warning "${name}: Radix reaches the internet through ${INSTANCE_STACK}_egress and fetches from the university directly (no Cortex)"
      fi
    elif [[ "${want}" == "yes" ]]; then
      if [[ -z "${cortex_url}" ]]; then
        fail "${name}: Radix crawls, but has no way to fetch: neither Cortex (RADIX_CORTEX_URL) nor ${INSTANCE_STACK}_egress (bash ${BETULA_ROOT}/vps/50-app.sh ${name})"
      elif [[ "${support}" == "no" ]]; then
        : # its FAIL is above
      else
        warning "${name}: the secret gemini-api-key exists, but Radix has no way to Gemini (no ${INSTANCE_STACK}_egress): scan-curriculum and the summaries fail (bash ${BETULA_ROOT}/vps/50-app.sh ${name})"
      fi
    elif [[ "${problems}" -eq 0 ]]; then
      if [[ -n "${cortex_url}" && "${INSTANCE_CRAWL}" == "on" ]]; then
        pass "${name}: Radix has no way to the internet and fetches through Cortex (networks: ${nets% })"
      else
        pass "${name}: Radix has no way to the internet (networks: ${nets% })"
      fi
    fi
  done < <(instance_names)
}

check_cortex() {
  section "cortex (two instances, one leader, the follower in step, Radix only on internal networks)"
  local x y svc state facts lock_a lock_b data_a data_b lead down=0
  local -a leading=()
  CORTEX_RUNS=no
  facts="$(docker network inspect cortex --format '{{.Driver}} {{.Scope}} {{.Attachable}} {{.Internal}}' 2>/dev/null || true)"
  if [[ "${facts}" == "overlay swarm true true" ]]; then
    pass "network cortex: overlay, swarm scope, attachable, internal (no way out of the host)"
  else
    fail "network cortex is '${facts:-missing}' (driver scope attachable internal), wanted 'overlay swarm true true' (sudo bash ${BETULA_ROOT}/vps/30-docker.sh)"
  fi

  if ! stack_exists cortex; then
    warning "stack cortex is not deployed (deploy/ship-cortex.sh): a Radix that crawls fetches from the university directly"
  else
    # Whether it runs is decided on the replicas, as 50-app.sh decides it (cortex_look,
    # lib-stacks.sh): an instance whose last update swarm rolled back runs the definition before
    # it, and swarm says "rollback_completed" until its next update, which 48-cortex.sh makes only
    # for another release or a change to stacks/cortex.yml. Section services, which asks whether
    # the last deploy landed, FAILs it.
    for x in a b; do
      svc="cortex_${x}"
      state="$(service_state "${svc}" now)"
      case "${state}" in
        ok\ *rollback_completed\)) warning "${svc}: ${state#* }: it runs, but swarm rolled its last update back, so it runs the definition before that one (docker service ps --no-trunc ${svc}); swarm says so until its next update (bash ${BETULA_ROOT}/vps/48-cortex.sh <tag>, with another release or a changed stacks/cortex.yml)" ;;
        ok\ *) pass "${svc}: ${state#* }" ;;
        *)
          fail "${svc}: ${state#* }  (docker service ps --no-trunc ${svc})"
          down=1
          ;;
      esac
    done
    if cortex_look; then
      CORTEX_RUNS=yes
    elif [[ "${down}" -eq 0 ]]; then
      fail "the stack cortex does not run (${CORTEX_DETAIL})"
    fi
    # One lock for the two (a stack volume both mount: flock across the containers), and an index
    # of its own for each: two processes on one index would corrupt it.
    lock_a="$(mount_source cortex_a /lock)" lock_b="$(mount_source cortex_b /lock)"
    data_a="$(mount_source cortex_a /data)" data_b="$(mount_source cortex_b /data)"
    if [[ -n "${lock_a}" && "${lock_a}" == "${lock_b}" ]]; then
      pass "both instances mount ${lock_a} at /lock: one lock for the two"
    else
      fail "cortex_a mounts '${lock_a:-nothing}' and cortex_b '${lock_b:-nothing}' at /lock: without one lock both may lead (stacks/cortex.yml)"
    fi
    if [[ -n "${data_a}" && -n "${data_b}" && "${data_a}" != "${data_b}" ]]; then
      pass "each instance has a data volume of its own (${data_a}, ${data_b})"
    else
      fail "cortex_a mounts '${data_a:-nothing}' and cortex_b '${data_b:-nothing}' at /data: each needs a volume of its own (stacks/cortex.yml)"
    fi
    # Cortex's own way out: the stack's "default", the one network of it that is not internal.
    for x in a b; do
      if service_networks "cortex_${x}" | awk '$2 == "false" { found = 1 } END { exit !found }'; then
        pass "cortex_${x} has a way to the internet ($(service_networks "cortex_${x}" | awk '$2 == "false" { print $1 }' | tr '\n' ' ' | sed 's/ $//'))"
      else
        fail "cortex_${x} is on internal networks only: it cannot fetch anything (stacks/cortex.yml, networks)"
      fi
    done

    # Exactly one leader, and the follower in step.
    look_cortex
    leading=("${CX_LEADING[@]}")
    for x in a b; do
      [[ -n "${CX_ROLE[$x]}" ]] ||
        fail "cortex_${x} does not answer \"cortex status\" (docker exec <its container> /bin/cortex status; docker service logs cortex_${x})"
    done
    case "${#leading[@]}" in
      1)
        lead="${leading[0]}"
        y="b"
        if [[ "${lead}" == "b" ]]; then y="a"; fi
        pass "cortex_${lead} leads (epoch ${CX_EPOCH[$lead]}, journal at ${CX_SEQ[$lead]})"
        if [[ "${CX_ROLE[$y]}" == "follower" ]]; then
          if awk -v lag="${CX_LAG[$y]}" -v max="${CORTEX_LAG_WARN}" 'BEGIN { exit !(lag + 0 > max) }'; then
            warning "cortex_${y} follows ${CX_LAG[$y]} s behind (more than ${CORTEX_LAG_WARN} s; state ${CX_STATE[$y]}, ${CX_BEHIND[$y]} entries behind, ${CX_MISSING[$y]} blobs missing, journal at ${CX_SEQ[$y]}): docker service logs cortex_${y} (replica.*). The alert fires beyond 300 s"
          elif ! cortex_in_step "${y}"; then
            warning "cortex_${y} follows, but has not caught up (state ${CX_STATE[$y]}, ${CX_LAG[$y]} s and ${CX_BEHIND[$y]} entries behind, ${CX_MISSING[$y]} blobs missing, epoch ${CX_EPOCH[$y]}, journal at ${CX_SEQ[$y]}; wanted: following or catching_up, under 1 s behind, on the leader's epoch ${CX_PREV_EPOCH:-?} and at least at its journal of 2 s before, ${CX_PREV_SEQ:-?}, 0 blobs missing): it still fetches from the leader (after a copy of its index: every blob), and 48-cortex.sh hands it no lead before. docker service logs cortex_${y} (replica.*)"
          else
            pass "cortex_${y} follows and has caught up, under 1 s behind (state ${CX_STATE[$y]}, ${CX_BEHIND[$y]} entries behind, 0 blobs missing, journal at ${CX_SEQ[$y]}, the leader's 2 s before at ${CX_PREV_SEQ})"
          fi
        fi
        ;;
      0) fail "no instance of Cortex leads: what needs a fetch is answered 503 no-leader (docker service logs cortex_a / cortex_b)" ;;
      *) fail "both instances of Cortex say they lead: they do not share the lock (/lock above); stop one until it is fixed: docker service scale cortex_b=0" ;;
    esac
  fi

  check_radix_ways
}

check_canary() {
  section "canary follows master (vps/canary-agent.sh, started by betula-canary.timer)"
  local unit state_dir="/var/lib/betula-canary" result started line expires colour
  local -a colours=()
  if [[ ! -f /etc/systemd/system/betula-canary.timer ]]; then
    warning "not installed: canary changes only with deploy/ship.sh (sudo bash ${BETULA_ROOT}/vps/60-canary.sh, README.md section 12)"
    return 0
  fi
  for unit in betula-canary.service betula-canary.timer; do
    if cmp -s -- "/etc/systemd/system/${unit}" "${BETULA_VPS_DIR}/files/${unit}"; then pass "${unit} as shipped"; else
      fail "/etc/systemd/system/${unit} differs from vps/files/${unit} (edited on the server, or deploy/ is newer: sudo bash ${BETULA_ROOT}/vps/60-canary.sh)"
    fi
  done
  if canary_follows_master; then
    pass "betula-canary.timer is on (next look: $(systemctl show betula-canary.timer --property=NextElapseUSecRealtime --value 2>/dev/null))"
  else
    warning "betula-canary.timer is off: canary does not follow master (sudo bash ${BETULA_ROOT}/vps/60-canary.sh on)"
  fi
  result="$(systemctl show betula-canary.service --property=Result --value 2>/dev/null || true)"
  started="$(systemctl show betula-canary.service --property=ExecMainStartTimestamp --value 2>/dev/null || true)"
  if [[ -z "${started}" ]]; then
    warning "betula-canary.service has not run since the last boot"
  elif [[ "${result}" == "success" ]]; then
    pass "the last run of betula-canary.service (${started}) succeeded"
  else
    fail "the last run of betula-canary.service (${started}) ended with '${result}': journalctl -u betula-canary.service -n 80"
  fi
  if [[ -r "${state_dir}/deployed" ]]; then
    pass "last deploy: $(head -n 1 "${state_dir}/deployed")"
  else
    warning "the agent has not deployed a release yet"
  fi
  # GitHub names the token's expiry in every answer; the agent keeps the last one it saw.
  if [[ -r "${state_dir}/token-expires" ]]; then
    line="$(head -n 1 "${state_dir}/token-expires")"
    expires="$(date -d "${line}" +%s 2>/dev/null || true)"
    if [[ ! "${expires}" =~ ^[0-9]+$ ]]; then
      warning "the GitHub token expires '${line}' (not a date this script can read)"
    elif [[ "${expires}" -le "$(date +%s)" ]]; then
      fail "the GitHub token of the agent expired ${line}: canary no longer follows master. A new one: README.md section 12"
    elif [[ "${expires}" -le $(($(date +%s) + 14 * 86400)) ]]; then
      warning "the GitHub token of the agent expires ${line}, in less than 14 days: store a new one (README.md section 12)"
    else
      pass "the GitHub token of the agent is valid until ${line}"
    fi
  fi
  if [[ -r "${state_dir}/failed" ]]; then
    line="$(head -n 1 "${state_dir}/failed")"
    warning "release ${line%% *} failed $(awk '{ print $2 }' <<<"${line}") time(s) so far (journalctl -u betula-canary.service); it is tried at most 3 times"
  fi
  for colour in "${CANARY_COLOURS[@]}"; do
    if stack_exists "${colour}"; then
      colours+=("${colour}")
    fi
  done
  case "${#colours[@]}" in
    1) pass "one colour of the canary is deployed: ${colours[0]}" ;;
    0) fail "no colour of the canary is deployed: nothing serves it (sudo systemctl start betula-canary.service, or deploy/ship.sh ${CANARY_COLOURS[0]})" ;;
    *) warning "both colours of the canary are deployed (${colours[*]}): a deploy is running, or one ended before it removed the old colour; the next deploy removes the one that does not serve" ;;
  esac
}

check_loki() {
  section "Loki (ready, fed by Alloy with the contract's labels, ruler storing the visitor numbers)"
  local body value i want have failing
  if ! find_prometheus; then
    fail "no running monitoring_prometheus container to query from (docker service ps monitoring_prometheus)"
    return 0
  fi
  body="$(mon_get "http://monitoring_loki:3100/ready" || true)"
  if [[ "${body}" == ready* ]]; then pass "Loki answers /ready"; else
    fail "Loki is not ready: '${body:-no answer}' (docker service logs monitoring_loki)"
    return 0
  fi
  # The requests of the other sections are at most seconds old; Alloy ships in batches.
  value=""
  for i in 1 2 3 4 5 6; do
    value="$(loki_value 'sum(count_over_time({stack="edge", service="edge_traefik"}[15m]))' || true)"
    if [[ "${value}" =~ ^[0-9]+$ && "${value}" -gt 0 ]]; then break; fi
    # A request of our own, so that "recent lines" does not depend on visitors.
    curl -sS -k -o /dev/null --max-time 10 "https://${SITE_HOST}/" 2>/dev/null || true
    sleep 5
  done
  if [[ "${value}" =~ ^[0-9]+$ && "${value}" -gt 0 ]]; then
    pass "{stack=\"edge\", service=\"edge_traefik\"}: ${value} lines in the last 15 minutes"
  else
    fail "no lines for {stack=\"edge\", service=\"edge_traefik\"} in the last 15 minutes (docker service logs monitoring_alloy)"
  fi
  value="$(loki_value 'sum(count_over_time({job="docker", container=~".+"}[15m]))' || true)"
  if [[ "${value}" =~ ^[0-9]+$ && "${value}" -gt 0 ]]; then pass "{job=\"docker\"} with a container label: ${value} lines"; else fail "no {job=\"docker\"} lines carry the container label"; fi
  value="$(loki_value 'sum(count_over_time({job="journal", unit=~".+"}[15m]))' || true)"
  if [[ "${value}" =~ ^[0-9]+$ && "${value}" -gt 0 ]]; then
    pass "{job=\"journal\"} with a unit label: ${value} lines (the ssh and OOM alert rules read these)"
  else
    fail "no {job=\"journal\"} lines in the last 15 minutes: the host journal is not shipped (is /var/log/journal persistent? docker service logs monitoring_alloy)"
  fi
  # Traefik's lines are in the journal too (journald driver); Alloy ships them as container logs
  # only. A copy under job="journal" would keep the addresses 30 days instead of 7.
  value="$(loki_value 'sum(count_over_time({job="journal"} |= "DownstreamStatus" [15m]))' || true)"
  if [[ -z "${value}" || "${value}" == "0" ]]; then
    pass "no access log lines under {job=\"journal\"} (Alloy drops the journal's copy of container lines)"
  else
    fail "${value} access log lines under {job=\"journal\"} in the last 15 minutes, kept 30 days there: loki.relabel \"journal\" in config/monitoring/alloy/config.alloy (40-stacks.sh monitoring)"
  fi
  # The ruler counts the visitors (config/monitoring/loki-rules) and writes the numbers to
  # Prometheus; the dashboard "Visitors" reads nothing else.
  want="$(find "${CONFIG_DIR}/monitoring/loki-rules" -name '*.yml' -exec grep -hcE '^[[:space:]]+- record: ' {} + 2>/dev/null | awk '{ s += $1 } END { print s + 0 }')"
  body="$(mon_get "http://monitoring_loki:3100/prometheus/api/v1/rules" || true)"
  have="$(jq -r '[.data.groups[].rules[]] | length' <<<"${body}" 2>/dev/null || true)"
  failing="$(jq -r '[.data.groups[].rules[] | select(.health == "err") | "\(.name): \(.lastError)"] | join("; ")' <<<"${body}" 2>/dev/null || true)"
  if [[ ! "${have}" =~ ^[0-9]+$ ]]; then
    fail "Loki's ruler does not answer /prometheus/api/v1/rules (docker service logs monitoring_loki)"
  elif [[ "${have}" -lt "${want}" ]]; then
    fail "Loki's ruler knows ${have} recording rules, config/monitoring/loki-rules defines ${want} (mounted at /etc/loki/rules? docker service logs monitoring_loki 2>&1 | grep -i rule)"
  elif [[ -n "${failing}" ]]; then
    fail "recording rules fail: ${failing}"
  else
    pass "Loki's ruler has ${have} recording rules, none failing"
  fi
  value="$(prom_value 'count({__name__=~"betula:.+"})' || true)"
  if [[ "${value}" =~ ^[0-9]+$ && "${value}" -gt 0 ]]; then
    pass "the visitor numbers reach Prometheus (${value} series betula:*)"
  else
    warning "no betula:* series in Prometheus: the ruler counts every 5 minutes after Loki started; if this stays, docker service logs monitoring_loki 2>&1 | grep -i -e rule -e remote"
  fi
}

check_prometheus() {
  section "Prometheus (ready, all targets up)"
  local body job value
  if ! find_prometheus; then
    fail "no running monitoring_prometheus container (docker service ps monitoring_prometheus)"
    return 0
  fi
  if mon_get "http://127.0.0.1:9090/-/ready" >/dev/null; then pass "Prometheus answers /-/ready"; else
    fail "Prometheus is not ready (docker service logs monitoring_prometheus)"
    return 0
  fi
  body="$(mon_get "http://127.0.0.1:9090/api/v1/query?query=up" || true)"
  for job in "${EXPECTED_JOBS[@]}"; do
    value="$(jq -r --arg job "${job}" '[.data.result[] | select(.metric.job == $job) | .value[1]] | if length == 0 then "absent" else (map(tonumber) | min | tostring) end' <<<"${body}" 2>/dev/null || true)"
    case "${job}:${value}" in
      *:1) pass "up{job=\"${job}\"} = 1" ;;
      traefik:*) fail "up{job=\"traefik\"} is ${value:-unknown}: Prometheus cannot scrape edge_traefik:8082 over the monitoring overlay" ;;
      cortex:*)
        if stack_exists cortex; then
          fail "up{job=\"cortex\"} is ${value:-unknown}: Prometheus cannot scrape cortex_a:8100 and cortex_b:8100 over the network cortex (does each run? section cortex. Is monitoring_prometheus on the network? bash ${BETULA_ROOT}/vps/40-stacks.sh monitoring)"
        else
          warning "up{job=\"cortex\"} is ${value:-unknown}: the stack cortex is not deployed (deploy/ship-cortex.sh). The rules for it (\"Monitoring target is down\" for job cortex, group betula-cortex) fire only for a Cortex that answered within the last 7 days: after removing it on purpose, silence them until then"
        fi
        ;;
      integrations/*:absent) fail "up{job=\"${job}\"} is absent: Alloy does not push host/container metrics (docker service logs monitoring_alloy)" ;;
      *) fail "up{job=\"${job}\"} is ${value:-unknown}" ;;
    esac
  done
  # Radix of every instance that runs (job radix, found by DNS: tasks.<stack>_radix; dashboard
  # "Radix"). An instance without a line in prometheus.yml is not scraped at all.
  local stack
  for stack in $(instance_names); do
    [[ "$(service_state "${stack}_radix")" == ok* ]] || continue
    value="$(jq -r --arg stack "${stack}" '[.data.result[] | select(.metric.job == "radix" and .metric.stack == $stack) | .value[1]] | if length == 0 then "absent" else (map(tonumber) | min | tostring) end' <<<"${body}" 2>/dev/null || true)"
    case "${value}" in
      1) pass "up{job=\"radix\", stack=\"${stack}\"} = 1" ;;
      absent) fail "${stack}_radix runs but Prometheus does not scrape it: is tasks.${stack}_radix in config/monitoring/prometheus.yml, and are the service and monitoring_prometheus both on the network cortex (stacks/betula.yml, stacks/monitoring.yml)?" ;;
      *) fail "up{job=\"radix\", stack=\"${stack}\"} is ${value:-unknown}: an image from before GET /metrics answers 404 (ship a current one)" ;;
    esac
  done
  # The alert rules and dashboards select on these labels; cAdvisor has to see Docker for them.
  value="$(prom_value 'count(container_last_seen{service="edge_traefik", stack="edge"})' || true)"
  if [[ "${value}" =~ ^[0-9]+$ && "${value}" -gt 0 ]]; then
    pass "container metrics carry stack/service labels (container_last_seen{service=\"edge_traefik\"})"
  else
    fail "container_last_seen has no series for service=\"edge_traefik\": cAdvisor inside Alloy does not resolve Docker containers, so the service-down alerts are blind"
  fi
  value="$(prom_value 'count(node_filesystem_avail_bytes{mountpoint="/"})' || true)"
  if [[ "${value}" =~ ^[0-9]+$ && "${value}" -gt 0 ]]; then pass "host metrics: node_filesystem_avail_bytes{mountpoint=\"/\"} exists"; else fail "node_filesystem_avail_bytes{mountpoint=\"/\"} is missing: the disk alert would report NoData"; fi
}

check_grafana() {
  section "Grafana"
  local body value code
  if ! find_prometheus; then
    fail "no running monitoring_prometheus container to query from"
    return 0
  fi
  body="$(mon_get "http://monitoring_grafana:3000/api/health" || true)"
  value="$(jq -r '.database // empty' <<<"${body}" 2>/dev/null || true)"
  if [[ "${value}" == "ok" ]]; then
    pass "Grafana /api/health: database ok, version $(jq -r '.version // "?"' <<<"${body}")"
  else
    fail "Grafana /api/health does not report a healthy database: '${body:-no answer}' (docker service logs monitoring_grafana)"
    return 0
  fi
  if grafana_router_deployed; then
    code="$(curl -sS -k -o /dev/null --max-time 15 -w '%{http_code}' "https://${GRAFANA_PUBLIC_HOST}/api/health" 2>/dev/null || true)"
    if [[ "${code}" == "200" ]]; then pass "https://${GRAFANA_PUBLIC_HOST}/api/health -> 200"; else fail "https://${GRAFANA_PUBLIC_HOST}/api/health -> ${code:-no answer}"; fi
    # /metrics answers without a login; the public router excludes it.
    code="$(curl -sS -k -o /dev/null --max-time 15 -w '%{http_code}' "https://${GRAFANA_PUBLIC_HOST}/metrics" 2>/dev/null || true)"
    if [[ "${code}" == "200" ]]; then fail "https://${GRAFANA_PUBLIC_HOST}/metrics is public (the router rule must exclude it)"; else pass "https://${GRAFANA_PUBLIC_HOST}/metrics is not public (${code})"; fi
  fi
}

check_alerts() {
  section "alert rules (provisioned from config/monitoring/grafana/provisioning/alerting)"
  local want have firing
  if ! find_prometheus; then
    fail "no running monitoring_prometheus container to query from"
    return 0
  fi
  [[ -r "${RULES_FILE}" ]] || {
    fail "${RULES_FILE} is missing"
    return 0
  }
  want="$(grep -cE '^[[:space:]]+- uid: ' "${RULES_FILE}" || true)"
  # Grafana's API needs a login (basic auth is off on purpose); its own metrics, which Prometheus
  # scrapes inside the stack, tell the same: the number of rules the scheduler knows.
  have="$(prom_value 'max(grafana_alerting_schedule_alert_rules)' || true)"
  if [[ ! "${have}" =~ ^[0-9]+$ ]]; then
    have="$(prom_value 'sum(grafana_alerting_rule_group_rules)' || true)"
  fi
  if [[ ! "${have}" =~ ^[0-9]+$ ]]; then
    warning "Grafana's rule metrics are not in Prometheus (yet): look at Grafana > Alerting > Alert rules; ${want} rules are expected in folder 'Betula alerts'"
  elif [[ "${have}" -ge "${want}" ]]; then
    pass "${have} alert rules are scheduled (rules.yml defines ${want})"
  else
    fail "only ${have} alert rules are scheduled, rules.yml defines ${want} (docker service logs monitoring_grafana 2>&1 | grep -i provision)"
  fi
  firing="$(prom_value 'sum(grafana_alerting_alerts{state="alerting"})' || true)"
  if [[ "${firing}" =~ ^[0-9]+$ && "${firing}" -gt 0 ]]; then
    warning "${firing} alert instance(s) are firing right now: see 'Alerts that need attention' on the overview dashboard"
  elif [[ "${firing}" =~ ^[0-9]+$ ]]; then
    pass "no alert is firing"
  fi
  if secret_exists grafana-smtp-host && [[ -n "$(docker service inspect monitoring_grafana --format '{{range .Spec.TaskTemplate.ContainerSpec.Env}}{{println .}}{{end}}' 2>/dev/null | grep -x 'GF_SMTP_ENABLED=true' || true)" ]]; then
    pass "SMTP override is deployed: alert e-mails can be sent (test: Grafana > Alerting > Contact points > betula-owner > Test)"
  else
    warning "the SMTP override is not deployed: unless another channel was wired in contact-points.yml, alerts are only visible in Grafana (README: notification channel)"
  fi
}

# ---------------------------------------------------------------- main

sections=("$@")
if [[ "${#sections[@]}" -eq 0 ]]; then
  sections=("${ALL_SECTIONS[@]}")
fi
for name in "${sections[@]}"; do
  if [[ " ${ALL_SECTIONS[*]} " != *" ${name} "* ]]; then
    die "unknown section '${name}' (known: ${ALL_SECTIONS[*]})"
  fi
done
for name in "${sections[@]}"; do
  "check_${name}"
done

printf '\nSUMMARY  pass=%d warn=%d fail=%d\n' "${PASSED}" "${WARNED}" "${FAILED}"
if [[ "${FAILED}" -gt 0 ]]; then
  exit 1
fi
