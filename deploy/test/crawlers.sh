#!/usr/bin/env bash
# crawlers.sh - the filter in front of the sites in a real Traefik: config/traefik/dynamic/crawlers.yml,
# crawler-networks.yml and europe.yml, who gets 403 and who is served. Needs Docker and curl, no
# swarm; pulls the Traefik image of stacks/edge.yml. Never on the server (it publishes 127.0.0.1:18444).
#
#   bash deploy/test/crawlers.sh        # up, check, down; exit 1 when a check fails
#
# Traefik runs with the three files as they are, next to two stand-ins of the sites' routers:
# Folia's for the host betula.app (priority 37, the colour's) and Phloem's /sync/v1/ (1000). Both
# lead to Traefik's ping, which answers 200 to every path, so 200 is "served" and 403 is the filter.
# The user agents are the ones the crawlers send (their operators' documentation, 2026-11). Then the
# networks, in copies that Traefik reloads: the test's own address (Docker's bridge) put among the
# clouds' of crawler-networks.yml, and taken out of Europe's open networks of europe.yml. Then the
# same over IPv6, from a client container on two IPv6 networks of Docker that Traefik joins (the
# host needs no IPv6 of its own): one unique local (open as a private network, until the test takes
# fc00::/7 out), one inside the first IPv6 network of Europe's list.
set -Euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPLOY_DIR="$(cd "${TEST_DIR}/.." && pwd)"
IMAGE="$(sed -n 's/^[[:space:]]*image:[[:space:]]*\(traefik:v[0-9.]*\)[[:space:]]*$/\1/p' "${DEPLOY_DIR}/stacks/edge.yml" | head -n 1)"
NAME="betula-crawlers-test"
PORT=18444
# The two IPv6 networks: unique local, and European (made from europe.yml below).
NET_ULA="betula-crawlers-test6"
NET_EU="betula-crawlers-test6-eu"
SUBNET_ULA="fd0b:7a00:c4a1:1::/64"
WORK="$(mktemp -d)"
FAILED=0

for cmd in docker curl; do
  command -v "${cmd}" >/dev/null || { echo "crawlers.sh needs ${cmd}" >&2; exit 2; }
done
[[ -n "${IMAGE}" ]] || { echo "no traefik image in stacks/edge.yml" >&2; exit 2; }

cleanup() {
  docker rm -f "${NAME}" >/dev/null 2>&1 || true
  docker network rm "${NET_ULA}" "${NET_EU}" >/dev/null 2>&1 || true
  rm -rf -- "${WORK}"
}
trap cleanup EXIT

mkdir -p "${WORK}/dynamic"
for file in crawlers.yml crawler-networks.yml europe.yml; do
  cp "${DEPLOY_DIR}/config/traefik/dynamic/${file}" "${WORK}/dynamic/"
done
cat >"${WORK}/dynamic/sites.yml" <<'EOF'
http:
  routers:
    folia:
      entryPoints: [websecure]
      rule: "Host(`betula.app`)"
      priority: 37
      tls: {}
      service: ping@internal
    sync:
      entryPoints: [websecure]
      rule: "Host(`betula.app`) && PathPrefix(`/sync/v1/`)"
      priority: 1000
      tls: {}
      service: ping@internal
EOF
chmod -R a+rX "${WORK}"

docker rm -f "${NAME}" >/dev/null 2>&1 || true
docker run -d --name "${NAME}" -p "127.0.0.1:${PORT}:8443" -v "${WORK}/dynamic:/etc/traefik/dynamic:ro" "${IMAGE}" \
  --entryPoints.websecure.address=:8443 \
  --providers.file.directory=/etc/traefik/dynamic \
  --ping=true --ping.manualRouting=true \
  --accessLog=true --accessLog.format=json \
  --log.level=INFO >/dev/null

# status AGENT PATH -> the status code of https://betula.app PATH with that user agent ("-": none)
status() {
  local agent=(-A "$1")
  [[ "$1" == "-" ]] && agent=(-H "User-Agent:")
  curl -sk --noproxy '*' --max-time 5 --resolve "betula.app:${PORT}:127.0.0.1" "${agent[@]}" -o /dev/null -w '%{http_code}' "https://betula.app:${PORT}$2" 2>/dev/null || echo 000
}

# Up when both files are loaded: a browser is served and GPTBot is not.
for _ in $(seq 1 60); do
  if [[ "$(status 'Mozilla/5.0' /)" == 200 && "$(status 'GPTBot/1.4' /)" == 403 ]]; then break; fi
  sleep 1
done

TURNED_AWAY=0
expect() { # EXPECTED PATH AGENT
  local got
  got="$(status "$3" "$2")"
  [[ "${got}" == 403 ]] && TURNED_AWAY=$((TURNED_AWAY + 1))
  if [[ "${got}" == "$1" ]]; then
    printf 'PASS  %s %-22s %s\n' "${got}" "$2" "$3"
  else
    FAILED=$((FAILED + 1))
    printf 'FAIL  %s %-22s %s (expected %s)\n' "${got}" "$2" "$3" "$1"
  fi
}

# Search engines.
expect 200 / 'Mozilla/5.0 (Linux; Android 6.0.1; Nexus 5X Build/MMB29P) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.6778.204 Mobile Safari/537.36 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)'
expect 200 /catalog 'Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; Googlebot/2.1; +http://www.google.com/bot.html) Chrome/131.0.6778.204 Safari/537.36'
expect 200 /favicon.svg 'Googlebot-Image/1.0'
expect 200 / 'Mozilla/5.0 (compatible; Google-InspectionTool/1.0)'
expect 200 / 'Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; bingbot/2.0; +http://www.bing.com/bingbot.htm) Chrome/116.0.1938.76 Safari/537.36'
expect 200 / 'msnbot/2.0b (+http://search.msn.com/msnbot.htm)'
expect 200 / 'DuckDuckBot/1.1; (+http://duckduckgo.com/duckduckbot.html)'
expect 200 /favicon.ico 'Mozilla/5.0 (compatible; DuckDuckGo-Favicons-Bot/1.0; +http://duckduckgo.com)'
expect 200 / 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_5) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/13.1.1 Safari/605.1.15 (Applebot/0.1; +http://www.apple.com/go/applebot)'
expect 200 / 'Mozilla/5.0 (compatible; Qwantbot/1.0; +https://help.qwant.com/bot/)'
expect 200 / 'Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; Bravebot/1.0; +https://search.brave.com/help/brave-search-crawler) Chrome/131.0.0.0 Safari/537.36'
expect 200 / 'Mozilla/5.0 (compatible; MojeekBot/0.11; +https://www.mojeek.com/bot.html)'
# Link previews, iMessage's mixed agent among them.
expect 200 '/catalog?turnus=winter' 'Twitterbot/1.0'
expect 200 /studyplan 'LinkedInBot/1.0 (compatible; Mozilla/5.0; Apache-HttpClient +http://www.linkedin.com)'
expect 200 /cards/module/11101.png 'facebookexternalhit/1.1 (+http://www.facebook.com/externalhit_uatext.php)'
expect 200 / 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_11_1) AppleWebKit/601.2.4 (KHTML, like Gecko) Version/9.0.1 Safari/601.2.4 facebookexternalhit/1.1 Facebot Twitterbot/1.0'
expect 200 / 'Slackbot-LinkExpanding 1.0 (+https://api.slack.com/robots)'
expect 200 / 'TelegramBot (like TwitterBot)'
expect 200 / 'Mozilla/5.0 (compatible; Discordbot/2.0; +https://discordapp.com)'
expect 200 / 'WhatsApp/2.23.20.0'
# People and tools that do not say they crawl.
expect 200 / 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0'
expect 200 / 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1'
expect 200 / 'Mozilla/5.0 (Linux; Android 11; CUBOT X50) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36'
expect 200 / 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Claude/1.3.0 Chrome/136.0.0.0 Electron/36.4.0 Safari/537.36'
expect 200 / 'curl/8.5.0'
expect 200 / '-'
# Every other crawler, on pages, the app's data and Phloem's path alike.
expect 403 / 'Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; GPTBot/1.4; +https://openai.com/gptbot)'
expect 403 /api/db 'Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; GPTBot/1.4; +https://openai.com/gptbot)'
expect 403 /sync/v1/x 'Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; GPTBot/1.4; +https://openai.com/gptbot)'
expect 403 / 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36; compatible; OAI-SearchBot/1.3; +https://openai.com/searchbot'
expect 403 / 'Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko); compatible; ChatGPT-User/1.0; +https://openai.com/bot'
expect 403 / 'Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; ClaudeBot/1.0; +claudebot@anthropic.com)'
expect 403 / 'Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; Claude-User/1.0; +Claude-User@anthropic.com)'
expect 403 / 'CCBot/2.0 (https://commoncrawl.org/faq/)'
expect 403 / 'Mozilla/5.0 (Linux; Android 5.0) AppleWebKit/537.36 (KHTML, like Gecko) Mobile Safari/537.36 (compatible; Bytespider; spider-feedback@bytedance.com)'
expect 403 / 'Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; Amazonbot/0.1; +https://developer.amazon.com/support/amazonbot) Chrome/119.0.6045.214 Safari/537.36'
expect 403 / 'meta-externalagent/1.1 (+https://developers.facebook.com/docs/sharing/webmasters/crawler)'
expect 403 / 'Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; PerplexityBot/1.0; +https://perplexity.ai/perplexitybot)'
expect 403 / 'Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; Perplexity-User/1.0; +https://perplexity.ai/perplexity-user)'
expect 403 / 'Mozilla/5.0 (Linux; Android 6.0.1; Nexus 5X Build/MMB29P) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.6778.204 Mobile Safari/537.36 (compatible; GoogleOther)'
expect 403 / 'AdsBot-Google (+http://www.google.com/adsbot.html)'
expect 403 / 'Mozilla/5.0 (compatible; AhrefsBot/7.0; +http://ahrefs.com/robot/)'
expect 403 / 'Mozilla/5.0 (compatible; SemrushBot/7~bl; +http://www.semrush.com/bot.html)'
expect 403 / 'Mozilla/5.0 (compatible; MJ12bot/v1.4.8; http://mj12bot.com/)'
expect 403 / 'Mozilla/5.0 (compatible; DotBot/1.2; +https://opensiteexplorer.org/dotbot; help@moz.com)'
expect 403 / 'Mozilla/5.0 (compatible; Barkrowler/0.9; +https://babbar.tech/crawler)'
expect 403 / 'Mozilla/5.0 (Linux; Android 7.0;) AppleWebKit/537.36 (KHTML, like Gecko) Mobile Safari/537.36 (compatible; PetalBot;+https://webmaster.petalsearch.com/site/petalbot)'
expect 403 / 'Mozilla/5.0 (compatible; YandexBot/3.0; +http://yandex.com/bots)'
expect 403 / 'Mozilla/5.0 (compatible; Baiduspider/2.0; +http://www.baidu.com/search/spider.html)'
expect 403 / 'Scrapy/2.11.2 (+https://scrapy.org)'
expect 403 / 'Mozilla/5.0 (compatible; UptimeRobot/2.0; http://www.uptimerobot.com/)'
# What every agent gets.
expect 200 /robots.txt 'Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; GPTBot/1.4; +https://openai.com/gptbot)'
expect 200 /calendar/x.ics 'Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; GPTBot/1.4; +https://openai.com/gptbot)'
expect 200 /en/calendar/x.ics 'CCBot/2.0 (https://commoncrawl.org/faq/)'
expect 200 /healthz 'Mozilla/5.0 (compatible; UptimeRobot/2.0; http://www.uptimerobot.com/)'
expect 200 /livez 'Mozilla/5.0 (compatible; UptimeRobot/2.0; http://www.uptimerobot.com/)'

# The dashboard "Visitors" counts the turned away requests from the access log, by their router:
# each 403 above is a line there (Traefik leaves out the routers of a service @internal; the
# stand-ins of the sites lead to its ping and are not logged, which is all right).
sleep 1
logged="$(docker logs "${NAME}" 2>/dev/null | grep -c '"DownstreamStatus":403,.*"RouterName":"edge-crawlers@file"' || true)"
if [[ "${logged}" -ge "${TURNED_AWAY}" && "${TURNED_AWAY}" -gt 0 ]]; then
  printf 'PASS  the access log has the %s requests turned away (%s lines of edge-crawlers@file)\n' "${TURNED_AWAY}" "${logged}"
else
  FAILED=$((FAILED + 1))
  printf 'FAIL  the access log has %s lines of edge-crawlers@file for %s requests turned away\n' "${logged}" "${TURNED_AWAY}"
fi

# The clouds' networks: Docker's bridge, where the test's requests come from, among them (the IPv4
# list, behind its guard). Written beside the directory and moved in, so that Traefik never reads
# half a file.
sed 's/ClientIP(`0\.0\.0\.0\/0`) && (ClientIP(/ClientIP(`0.0.0.0\/0`) \&\& (ClientIP(`172.16.0.0\/12`) || ClientIP(/' "${WORK}/dynamic/crawler-networks.yml" >"${WORK}/crawler-networks.yml"
chmod a+r "${WORK}/crawler-networks.yml"
mv -f "${WORK}/crawler-networks.yml" "${WORK}/dynamic/crawler-networks.yml"
for _ in $(seq 1 30); do
  [[ "$(status 'Mozilla/5.0' /)" == 403 ]] && break
  sleep 1
done
TURNED_AWAY=0
expect 403 / 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36'
expect 403 /api/db 'Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Mobile Safari/537.36'
expect 403 / 'Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)'
expect 403 /sync/v1/x 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36'
expect 200 /robots.txt 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36'
expect 200 /calendar/x.ics 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36'
expect 200 /healthz 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36'
sleep 1
logged="$(docker logs "${NAME}" 2>/dev/null | grep -c '"DownstreamStatus":403,.*"RouterName":"edge-crawler-networks@file"' || true)"
if [[ "${logged}" -ge "${TURNED_AWAY}" && "${TURNED_AWAY}" -gt 0 ]]; then
  printf 'PASS  the access log has the %s requests from a cloud turned away (%s lines of edge-crawler-networks@file)\n' "${TURNED_AWAY}" "${logged}"
else
  FAILED=$((FAILED + 1))
  printf 'FAIL  the access log has %s lines of edge-crawler-networks@file for %s requests turned away\n' "${logged}" "${TURNED_AWAY}"
fi

# Back to the clouds' list as it is, then Europe without the private networks: the test comes from
# outside Europe now.
cp "${DEPLOY_DIR}/config/traefik/dynamic/crawler-networks.yml" "${WORK}/crawler-networks.yml"
chmod a+r "${WORK}/crawler-networks.yml"
mv -f "${WORK}/crawler-networks.yml" "${WORK}/dynamic/crawler-networks.yml"
for _ in $(seq 1 30); do
  [[ "$(status 'Mozilla/5.0' /)" == 200 ]] && break
  sleep 1
done
grep -v '|| ClientIP(`172.16.0.0/12`)' "${WORK}/dynamic/europe.yml" >"${WORK}/europe.yml"
chmod a+r "${WORK}/europe.yml"
mv -f "${WORK}/europe.yml" "${WORK}/dynamic/europe.yml"
for _ in $(seq 1 30); do
  [[ "$(status 'Mozilla/5.0' /)" == 403 ]] && break
  sleep 1
done
TURNED_AWAY=0
expect 403 / 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0'
expect 403 /api/db 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36'
expect 200 / 'Mozilla/5.0 (Linux; Android 6.0.1; Nexus 5X Build/MMB29P) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.6778.204 Mobile Safari/537.36 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)'
expect 200 / 'Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; bingbot/2.0; +http://www.bing.com/bingbot.htm) Chrome/116.0.1938.76 Safari/537.36'
expect 200 / 'Mozilla/5.0 (compatible; Google-Site-Verification/1.0)'
expect 200 /catalog/module/11101 'Twitterbot/1.0'
expect 200 / 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36 Edg/136.0.0.0 SkypeUriPreview Preview/0.5'
expect 200 /robots.txt 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0'
expect 200 /calendar/x.ics 'Google-Calendar-Importer'
expect 200 /healthz 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0'
sleep 1
logged="$(docker logs "${NAME}" 2>/dev/null | grep -c '"DownstreamStatus":403,.*"RouterName":"edge-outside-europe@file"' || true)"
if [[ "${logged}" -ge "${TURNED_AWAY}" && "${TURNED_AWAY}" -gt 0 ]]; then
  printf 'PASS  the access log has the %s requests from outside Europe turned away (%s lines of edge-outside-europe@file)\n' "${TURNED_AWAY}" "${logged}"
else
  FAILED=$((FAILED + 1))
  printf 'FAIL  the access log has %s lines of edge-outside-europe@file for %s requests turned away\n' "${logged}" "${TURNED_AWAY}"
fi

# ---- IPv6. Each family has its own list behind its guard: what was taken out of the IPv4 list
# above does not matter here. The European network: a /64 inside the first IPv6 network of
# Europe's list (after ::1/128 and fc00::/7), whose first two groups fix it (every network of the
# list is a /32 or larger).
log_count() { docker logs "${NAME}" 2>/dev/null | grep -c "\"DownstreamStatus\":403,.*\"RouterName\":\"$1\"" || true; }
eu6="$(sed -n 's/^[[:space:]]*|| ClientIP(`\([0-9a-f]*:[0-9a-f:]*\/[0-9]*\)`).*$/\1/p' "${DEPLOY_DIR}/config/traefik/dynamic/europe.yml" |
  grep -v -e '^::1/128$' -e '^fc00::/7$' | head -n 1)"
g1="${eu6%%:*}"
g2="${eu6#*:}"
g2="${g2%%:*}"
SUBNET_EU="${g1}:${g2:-0}:ffff:ffff::/64"
if [[ -z "${eu6}" ]]; then
  FAILED=$((FAILED + 1))
  echo "FAIL  europe.yml has no IPv6 network"
elif ! docker network create --ipv6 --subnet "${SUBNET_ULA}" "${NET_ULA}" >/dev/null 2>"${WORK}/net.err" ||
  ! docker network create --ipv6 --subnet "${SUBNET_EU}" "${NET_EU}" >/dev/null 2>>"${WORK}/net.err"; then
  FAILED=$((FAILED + 1))
  printf 'FAIL  Docker could not make the IPv6 networks %s and %s: %s\n' "${SUBNET_ULA}" "${SUBNET_EU}" "$(tr '\n' ' ' <"${WORK}/net.err")"
else
  docker network connect "${NET_ULA}" "${NAME}"
  docker network connect "${NET_EU}" "${NAME}"
  ADDR_ULA="$(docker inspect -f "{{(index .NetworkSettings.Networks \"${NET_ULA}\").GlobalIPv6Address}}" "${NAME}")"
  ADDR_EU="$(docker inspect -f "{{(index .NetworkSettings.Networks \"${NET_EU}\").GlobalIPv6Address}}" "${NAME}")"

  # status6 ula|eu AGENT PATH -> the status code over IPv6, from a client on that network (the
  # Traefik image's busybox wget: no other image to pull).
  status6() {
    local net="${NET_ULA}" addr="${ADDR_ULA}" out code
    [[ "$1" == "eu" ]] && net="${NET_EU}" addr="${ADDR_EU}"
    out="$(docker run --rm --network "${net}" --add-host "betula.app=${addr}" --entrypoint wget "${IMAGE}" \
      -S -O /dev/null -T 5 --no-check-certificate -U "$2" "https://betula.app:8443$3" 2>&1 || true)"
    code="$(grep -o 'HTTP/[0-9.]* [0-9]*' <<<"${out}" | head -n 1)"
    code="${code##* }"
    printf '%s' "${code:-000}"
  }
  expect6() { # EXPECTED ula|eu PATH AGENT
    local got
    got="$(status6 "$2" "$4" "$3")"
    [[ "${got}" == 403 ]] && TURNED_AWAY=$((TURNED_AWAY + 1))
    if [[ "${got}" == "$1" ]]; then
      printf 'PASS  %s %-22s %s (IPv6, %s)\n' "${got}" "$3" "$4" "$2"
    else
      FAILED=$((FAILED + 1))
      printf 'FAIL  %s %-22s %s (IPv6, %s; expected %s)\n' "${got}" "$3" "$4" "$2" "$1"
    fi
  }
  wait6() { # EXPECTED: until a browser on the unique local network gets it
    for _ in $(seq 1 30); do
      [[ "$(status6 ula 'Mozilla/5.0' /)" == "$1" ]] && return 0
      sleep 1
    done
  }
  echo "IPv6: unique local ${ADDR_ULA}, European ${ADDR_EU} (inside ${eu6})"

  # As the files are: unique local addresses are private and open, the European one is in the list.
  expect6 200 ula / 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0'
  expect6 200 eu / 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0'
  expect6 403 eu / 'Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; GPTBot/1.4; +https://openai.com/gptbot)'

  # The clouds' IPv6 list with the unique local network in it.
  before="$(log_count edge-crawler-networks@file)"
  sed "s/ClientIP(\`::\/0\`) && (ClientIP(/ClientIP(\`::\/0\`) \&\& (ClientIP(\`${SUBNET_ULA//\//\\/}\`) || ClientIP(/" \
    "${DEPLOY_DIR}/config/traefik/dynamic/crawler-networks.yml" >"${WORK}/crawler-networks.yml"
  chmod a+r "${WORK}/crawler-networks.yml"
  mv -f "${WORK}/crawler-networks.yml" "${WORK}/dynamic/crawler-networks.yml"
  wait6 403
  TURNED_AWAY=0
  expect6 403 ula / 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36'
  expect6 403 ula /api/db 'Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Mobile Safari/537.36'
  expect6 200 ula /robots.txt 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36'
  expect6 200 eu / 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36'
  sleep 1
  logged=$(($(log_count edge-crawler-networks@file) - before))
  if [[ "${logged}" -ge "${TURNED_AWAY}" && "${TURNED_AWAY}" -gt 0 ]]; then
    printf 'PASS  the access log has the %s IPv6 requests from a cloud turned away\n' "${TURNED_AWAY}"
  else
    FAILED=$((FAILED + 1))
    printf 'FAIL  the access log has %s new lines of edge-crawler-networks@file for %s IPv6 requests turned away\n' "${logged}" "${TURNED_AWAY}"
  fi

  # The clouds' list as it is, then Europe without fc00::/7: the unique local client comes from
  # outside Europe now, and no ::/0 lets it in.
  cp "${DEPLOY_DIR}/config/traefik/dynamic/crawler-networks.yml" "${WORK}/crawler-networks.yml"
  chmod a+r "${WORK}/crawler-networks.yml"
  mv -f "${WORK}/crawler-networks.yml" "${WORK}/dynamic/crawler-networks.yml"
  wait6 200
  before="$(log_count edge-outside-europe@file)"
  grep -v '|| ClientIP(`fc00::/7`)' "${WORK}/dynamic/europe.yml" >"${WORK}/europe.yml"
  chmod a+r "${WORK}/europe.yml"
  mv -f "${WORK}/europe.yml" "${WORK}/dynamic/europe.yml"
  wait6 403
  TURNED_AWAY=0
  expect6 403 ula / 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0'
  expect6 403 ula /api/db 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36'
  expect6 200 ula / 'Mozilla/5.0 (Linux; Android 6.0.1; Nexus 5X Build/MMB29P) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.6778.204 Mobile Safari/537.36 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)'
  expect6 200 ula /robots.txt 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0'
  expect6 200 ula /calendar/x.ics 'Google-Calendar-Importer'
  expect6 200 eu / 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0'
  expect6 200 eu /api/db 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36'
  sleep 1
  logged=$(($(log_count edge-outside-europe@file) - before))
  if [[ "${logged}" -ge "${TURNED_AWAY}" && "${TURNED_AWAY}" -gt 0 ]]; then
    printf 'PASS  the access log has the %s IPv6 requests from outside Europe turned away\n' "${TURNED_AWAY}"
  else
    FAILED=$((FAILED + 1))
    printf 'FAIL  the access log has %s new lines of edge-outside-europe@file for %s IPv6 requests turned away\n' "${logged}" "${TURNED_AWAY}"
  fi
fi

if [[ "${FAILED}" -gt 0 ]]; then
  echo "${FAILED} check(s) failed; Traefik's log:"
  docker logs "${NAME}" 2>&1 | tail -n 20
  exit 1
fi
echo "all checks passed (${IMAGE})"
