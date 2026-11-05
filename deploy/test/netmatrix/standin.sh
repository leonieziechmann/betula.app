#!/bin/sh
# A stand-in for a service of the stacks (deploy/test/netmatrix.sh): alpine's busybox, one tiny
# HTTP server per port in LISTEN, answering what the scripts and the scrapers ask:
#   /metrics                  200, Prometheus text format (with the Content-Type Prometheus 3 wants)
#   /api/status /api/db       401 (closed testing, as Folia with FOLIA_ACCESS_GATE=on)
#   everything else           200, the body names the service (NAME) and the path
# With STATS_DIR set (Phloem's stand-in) it writes phloem.prom there every 15 s, as Phloem does.
#   standin.sh               start the servers
#   standin.sh answer        answer one request on stdin/stdout (what nc -e runs)
set -eu

if [ "${1:-}" = "answer" ]; then
  read -r method path proto || exit 0
  # The rest of the request head, until the empty line (or two seconds of nothing).
  while read -r -t 2 line; do
    case "${line}" in "" | "$(printf '\r')") break ;; esac
  done
  path="${path%%\?*}"
  type="text/plain"
  case "${path}" in
    /metrics)
      status="200 OK"
      type="text/plain; version=0.0.4; charset=utf-8"
      body="$(printf '# HELP standin_up 1 while the stand-in answers.\n# TYPE standin_up gauge\nstandin_up{name="%s"} 1\n' "${NAME:-standin}")"
      ;;
    /api/status | /api/db)
      status="401 Unauthorized"
      body="closed"
      ;;
    *)
      status="200 OK"
      body="${NAME:-standin} ${path}"
      ;;
  esac
  printf 'HTTP/1.0 %s\r\nContent-Type: %s\r\nContent-Length: %s\r\nConnection: close\r\n\r\n%s\n' \
    "${status}" "${type}" "$((${#body} + 1))" "${body}"
  exit 0
fi

for port in ${LISTEN:-8080}; do
  nc -lk -p "${port}" -e /bin/sh "$0" answer &
done
if [ -n "${STATS_DIR:-}" ]; then
  while :; do
    printf '# HELP phloem_up 1 while Phloem serves.\n# TYPE phloem_up gauge\nphloem_up 1\n# TYPE phloem_spaces gauge\nphloem_spaces 0\n' >"${STATS_DIR}/.phloem.prom.tmp"
    mv -f "${STATS_DIR}/.phloem.prom.tmp" "${STATS_DIR}/phloem.prom"
    sleep 15
  done
fi
wait
