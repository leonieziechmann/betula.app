#!/usr/bin/env bash
# 91-verify-stacks.sh - read-only audit of what 40-stacks.sh deployed. Run on the server as the
# deploy user (group docker), no sudo needed:
#
#   bash /opt/betula/vps/91-verify-stacks.sh                 # everything
#   bash /opt/betula/vps/91-verify-stacks.sh tls headers     # only some sections
#
# Sections: services http tls headers ports loki prometheus grafana alerts
# Prints one PASS / WARN / FAIL line per check and exits non-zero when anything FAILed.
# Changes nothing. The only traffic it causes: a few requests to the site (which also put fresh
# lines into Traefik's access log for the Loki check) and queries inside the monitoring stack.
#
# Ports that are not published (Loki 3100, Prometheus 9090, Grafana 3000) are reached with
# "docker exec" into the Prometheus container: it is on the stack network next to them and on the
# "monitoring" overlay next to Traefik's metrics port, and its image has busybox wget.
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

ALL_SECTIONS=(services http tls headers ports loki prometheus grafana alerts)
GRAFANA_PUBLIC_HOST="${GRAFANA_HOST:-${DEFAULT_GRAFANA_HOST}}"
RULES_FILE="${CONFIG_DIR}/monitoring/grafana/provisioning/alerting/rules.yml"
# Jobs that must be "up": five scraped by Prometheus (config/monitoring/prometheus.yml), two
# pushed by Alloy through remote write (config/monitoring/alloy/config.alloy).
EXPECTED_JOBS=(prometheus traefik loki grafana alloy integrations/unix integrations/cadvisor)
# Ports of the contract that must NOT listen on the host: Traefik ping and metrics, socket proxy,
# Grafana, Loki, Prometheus, Alloy, Radix.
PRIVATE_PORTS=(8081 8082 2375 3000 3100 9090 12345 8090)
PASSED=0
WARNED=0
FAILED=0
PROM_CID=""

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
  local stack svc state found
  for stack in edge placeholder betula monitoring; do
    if ! stack_exists "${stack}"; then
      case "${stack}" in
        edge | monitoring) fail "stack ${stack} is not deployed (vps/40-stacks.sh)" ;;
        placeholder) stack_exists betula || fail "neither stack placeholder nor stack betula is deployed: nothing answers https://${SITE_HOST}" ;;
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
  if stack_exists betula && stack_exists placeholder; then
    warning "stack placeholder still runs next to betula; once the application works: docker stack rm placeholder"
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
  addr="$(default_ipv4)"
  if [[ -n "${addr}" ]]; then addrs+=("${addr}"); fi
  have=""
  for addr in "${addrs[@]}"; do
    for port in "${PRIVATE_PORTS[@]}"; do
      if timeout 3 bash -c ": </dev/tcp/${addr}/${port}" 2>/dev/null; then
        have+="${addr}:${port} "
      fi
    done
  done
  if [[ -z "${have}" ]]; then
    pass "nothing listens on the host on ${PRIVATE_PORTS[*]} (Traefik ping/metrics, socket proxy, Grafana, Loki, Prometheus, Alloy, Radix)"
  else
    fail "reachable on the host: ${have% } - a service publishes a port it should not (docker service ls; sudo ss -tlnp)"
  fi
}

check_loki() {
  section "Loki (ready, and fed by Alloy with the contract's labels)"
  local body value i
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
      integrations/*:absent) fail "up{job=\"${job}\"} is absent: Alloy does not push host/container metrics (docker service logs monitoring_alloy)" ;;
      *) fail "up{job=\"${job}\"} is ${value:-unknown}" ;;
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
