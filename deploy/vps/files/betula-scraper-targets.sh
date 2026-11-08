#!/usr/bin/env bash
# Installed by /opt/betula/vps/30-docker.sh as /usr/local/sbin/betula-scraper-targets. Run every
# minute by betula-scraper-targets.timer, as root, and right away by the scripts that deploy or
# remove an instance (40-stacks.sh monitoring, 50-app.sh, canary-agent.sh: scraper_targets_refresh
# in lib-stacks.sh). By hand:
#   sudo systemctl start betula-scraper-targets.service && cat /var/lib/betula/scraper/radix.json
#
# The Radix of every instance that runs, as the list of targets of the scraper's job radix
# (config/monitoring/scraper.yml reads it as a file_sd file; stacks/monitoring.yml mounts the
# directory read-only):
#   [{"targets": ["tasks.betula_radix.:8090", "tasks.canary_radix.:8090"]}]
# An instance counts when swarm has a service <stack>_radix and attaches it to cortex_<stack>, the
# network the scraper reaches it over (vps/files/networks.conf). One on other networks only
# (betula-green before it moved to them, README.md section 15) is left out: the scraper could not
# reach it, and it would only be a target that is down.
#
# Why a list and not DNS, as until 2026-11-08: the scraper is on internal networks only, so Docker's
# DNS cannot ask upstream about a name it does not know, and answers SERVFAIL instead of NXDOMAIN.
# Prometheus keeps the targets of a name whose lookup fails (discovery/dns: it cannot know whether
# the name still has them), and a reload keeps them as well. A colour canary-agent.sh removed stayed
# a target that is down until the scraper restarted, and "Monitoring target is down" fired for it.
# This list follows what swarm runs: a removed instance leaves it within a minute, and the scraper
# ends its series (staleness markers) instead of reporting it down.
#
# It writes only when the list changes, as a new file renamed over the old one (the scraper sees
# the rename at once and never reads half a file), and nothing when Docker cannot be asked: then
# the last list stays.
set -Eeuo pipefail
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

OUT_DIR="${BETULA_SCRAPER_TARGETS_DIR:-/var/lib/betula/scraper}"
OUT="${OUT_DIR}/radix.json"
PORT=8090

say() { printf 'betula-scraper-targets: %s\n' "$*" >&2; }
die() {
  say "$*"
  exit 1
}

[[ -d "${OUT_DIR}" ]] || die "${OUT_DIR} does not exist (sudo bash /opt/betula/vps/30-docker.sh makes it)"
services="$(docker service ls --format '{{.Name}}' 2>&1)" ||
  die "cannot list the services, ${OUT} stays as it is: ${services}"

stacks=()
while IFS= read -r svc; do
  # The names of instances (load_instance in lib-stacks.sh): nothing else ends up in the file.
  [[ "${svc}" =~ ^([a-z][a-z0-9-]{0,30})_radix$ ]] || continue
  stack="${BASH_REMATCH[1]}"
  net="$(docker network inspect --format '{{.Id}}' "cortex_${stack}" 2>/dev/null)" || continue
  # What swarm was told (service_networks in lib-stacks.sh): the IDs of the networks. A service
  # that went between the two calls is no target either.
  attached="$(docker service inspect --format '{{range .Spec.TaskTemplate.Networks}}{{.Target}} {{end}}' "${svc}" 2>/dev/null)" || continue
  [[ " ${attached}" == *" ${net} "* || " ${attached}" == *" cortex_${stack} "* ]] || continue
  stacks+=("${stack}")
done <<<"${services}"

json='[{"targets": ['
sep=""
for stack in ${stacks[@]+"${stacks[@]}"}; do
  json+="${sep}\"tasks.${stack}_radix.:${PORT}\""
  sep=", "
done
json+=']}]'

was=""
if [[ -f "${OUT}" ]]; then
  [[ "$(cat -- "${OUT}")" != "${json}" ]] || exit 0
  was="$(grep -oE 'tasks\.[a-z0-9-]+_radix' "${OUT}" | sed -e 's/^tasks\.//' -e 's/_radix$//' | tr '\n' ' ' || true)"
  was="${was% }"
fi
tmp="${OUT_DIR}/.radix.json.new"
printf '%s\n' "${json}" >"${tmp}"
chmod 0644 "${tmp}"
mv -f -- "${tmp}" "${OUT}"
now="${stacks[*]:-}"
say "the Radix of: ${now:-none} (before: ${was:-none})"
