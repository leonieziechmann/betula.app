#!/usr/bin/env python3
"""crawler-networks.py - the networks of the clouds whose crawlers pass for browsers, as a router of
Traefik that turns them away: writes config/traefik/dynamic/crawler-networks.yml.

    python3 deploy/crawler-networks.py        # on Windows: python deploy/crawler-networks.py

Then look at the diff, commit, and copy the file to /opt/betula/config/traefik/dynamic/ (sync.sh or
scp; Traefik reloads it within seconds, README section 3). Run it again now and then (once a month
is plenty): the clouds announce new networks, and an old list lets their new machines in.

Owner, 2026-11-08: „Irgend ein bot hat sich komplett anonymisiert und taucht bei den normalen usern
mit auf. Kann man dagegen was tun?" On that day, of 751 browsers (address + user agent) that started
the app within 3 hours, almost all came from the clouds of Tencent and Huawei, each downloading the
catalog (/api/db, about 8 MB, 2 GB together); one from Alibaba's walked the filtered views of the
catalog without running the app; real visitors were a handful. They send the user agents of Chrome,
so the crawler filter by user agent (crawlers.yml) cannot tell them, but no student browses from a
data centre. What a cloud announces is read from RIPEstat (announced-prefixes: what the routing
tables of the internet carried for the network in the last two weeks), merged where one network
holds another, and written as one ClientIP per network.

Needs Python 3.8+ and nothing else (urllib, json, ipaddress).
"""

import datetime
import ipaddress
import json
import pathlib
import sys
import urllib.request

# The autonomous systems (the networks the internet routes as one) and whose they are. Only clouds:
# a provider of homes and phones (China Mobile, China Telecom, ...) carries people as well.
CLOUDS = {
    132203: "Tencent Cloud",
    45090: "Tencent",
    136907: "Huawei Cloud",
    55990: "Huawei Cloud, China",
    45102: "Alibaba Cloud",
    37963: "Alibaba Cloud, China",
}

SOURCE = "https://stat.ripe.net/data/announced-prefixes/data.json?resource=AS{asn}&sourceapp=betula-crawler-networks"
OUT = pathlib.Path(__file__).resolve().parent / "config" / "traefik" / "dynamic" / "crawler-networks.yml"


def announced(asn):
    with urllib.request.urlopen(SOURCE.format(asn=asn), timeout=60) as answer:
        data = json.load(answer)
    if data.get("status") != "ok":
        sys.exit(f"RIPEstat answered {data.get('status')!r} for AS{asn}: {data.get('messages')}")
    return [ipaddress.ip_network(p["prefix"], strict=False) for p in data["data"]["prefixes"]]


def main():
    found, counts = [], {}
    for asn, name in CLOUDS.items():
        networks = announced(asn)
        if not networks:
            sys.exit(f"AS{asn} ({name}) announces nothing: RIPEstat's answer is not to be trusted, the file stays as it is")
        counts[asn] = len(networks)
        found += networks
    v4 = list(ipaddress.collapse_addresses(n for n in found if n.version == 4))
    v6 = list(ipaddress.collapse_addresses(n for n in found if n.version == 6))
    networks = v4 + v6

    today = datetime.date.today().isoformat()
    clouds = "\n".join(f"#   AS{asn:<7} {name}: {counts[asn]} networks announced" for asn, name in CLOUDS.items())
    matchers = "\n        || ".join(f"ClientIP(`{n}`)" for n in networks)
    OUT.write_text(
        f"""# Written by deploy/crawler-networks.py on {today}: do not edit, run it again. Why, and how often:
# its docstring.
#
# Every request from a network of these clouds gets 403, whatever its user agent says, and never
# reaches a site: their crawlers pass for Chrome and start the app, so that the dashboard "Visitors"
# counted them as people. The same exceptions as crawlers.yml (robots.txt, the calendar feeds,
# /healthz, /livez) and the same answer (its middleware and service); the dashboard counts these
# as "crawlers turned away" too (RouterName edge-crawler-networks@file).
#
{clouds}
#   merged: {len(v4)} IPv4 and {len(v6)} IPv6 networks. Traefik sees no IPv6 client so far
#   (stacks/edge.yml, "ipv6"); the IPv6 networks are here for the day it does.
#
# ClientIP judges the address the connection comes from, never X-Forwarded-For: for IPv4 that is
# the client (stacks/edge.yml). Somebody with a VPN whose exit is in one of these clouds is turned
# away as well; that is the price.

http:
  routers:
    edge-crawler-networks:
      entryPoints:
        - websecure
      rule: >-
        ({matchers})
        && !Path(`/robots.txt`) && !Path(`/healthz`) && !Path(`/livez`)
        && !PathRegexp(`^/([a-z]{{2}}/)?calendar/`)
      priority: 10000
      tls: {{}}
      middlewares:
        - edge-crawlers-refuse
      service: edge-crawlers-nowhere
""",
        encoding="utf-8",
        newline="\n",
    )
    print(f"{OUT}: {len(v4)} IPv4 + {len(v6)} IPv6 networks of {sum(counts.values())} announced", file=sys.stderr)


if __name__ == "__main__":
    main()
