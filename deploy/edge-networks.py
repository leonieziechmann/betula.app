#!/usr/bin/env python3
"""edge-networks.py - the lists of networks of the filter in front of the sites, as routers of
Traefik that turn requests away. Writes two files of config/traefik/dynamic/:

  crawler-networks.yml  every request from the clouds whose crawlers pass for browsers
  europe.yml            every request from outside Europe

    python3 deploy/edge-networks.py        # on Windows: python deploy/edge-networks.py

Then look at the diff, commit, and copy both files to /opt/betula/config/traefik/dynamic/ (sync.sh
or scp; Traefik reloads them within seconds, README section 3). Run it again now and then (once a
month is plenty): networks change hands, and an old list lets new machines in or keeps people out.

The clouds. Owner, 2026-11-08: „Irgend ein bot hat sich komplett anonymisiert und taucht bei den
normalen usern mit auf. Kann man dagegen was tun?" On that day, of 751 browsers (address + user
agent) that started the app within 3 hours, almost all came from the clouds of Tencent and Huawei,
each downloading the catalog (/api/db, about 8 MB, 2 GB together); one from Alibaba's walked the
filtered views of the catalog without running the app. They send the user agents of Chrome, so the
crawler filter by user agent (crawlers.yml) cannot tell them, but no student browses from a data
centre. What a cloud announces is read from RIPEstat (announced-prefixes: what the routing tables
of the internet carried for the network in the last two weeks), merged, one ClientIP per network.

Europe. Owner, the same day: „du kannst ruhig den ganzen non eu traffic sperren. Betula ist halt
einfach nur für die BTU in deutschland. Vielleicht können wir das auf Europa ausweiten. Also ja muss
nicht ganz so streng sein. Aber Amerika, Südostasien etc muss nich sein." Where a network is, is
read from the five registries' delegation files (the country of whom each block was given to).
Europe has some 36,000 networks there, and Traefik v3 takes one network per ClientIP, so that many
would cost every request milliseconds: the list is made of blocks of 65,536 addresses (/16)
instead, and a block is open as soon as any part of it is European. Of European addresses none is
turned away; of the others under 4 % pass (counted 2026-11-08). Sorted large first, so that most
European requests are told within the first few matchers.

Needs Python 3.8+ and nothing else (urllib, json, ipaddress).
"""

import collections
import datetime
import ipaddress
import json
import pathlib
import re
import sys
import textwrap
import urllib.request

DYNAMIC = pathlib.Path(__file__).resolve().parent / "config" / "traefik" / "dynamic"

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
ANNOUNCED = "https://stat.ripe.net/data/announced-prefixes/data.json?resource=AS{asn}&sourceapp=betula-edge-networks"

# Europe, generously: the EU and the EEA, Switzerland, the United Kingdom and the islands of the
# Crown, the small states, the Balkans, Ukraine and Moldova, Turkey and the Caucasus, the Faroes,
# Åland, Svalbard and Gibraltar. Not Russia and Belarus. ISO 3166 codes, as the registries write them.
EUROPE = set(
    "AD AL AM AT AX AZ BA BE BG CH CY CZ DE DK EE ES FI FO FR GB GE GG GI GR HR HU IE IM IS IT JE LI"
    " LT LU LV MC MD ME MK MT NL NO PL PT RO RS SE SI SJ SK SM TR UA VA XK".split()
)
REGISTRIES = [
    "https://ftp.ripe.net/pub/stats/ripencc/delegated-ripencc-extended-latest",
    "https://ftp.arin.net/pub/stats/arin/delegated-arin-extended-latest",
    "https://ftp.apnic.net/stats/apnic/delegated-apnic-extended-latest",
    "https://ftp.lacnic.net/pub/stats/lacnic/delegated-lacnic-extended-latest",
    "https://ftp.afrinic.net/pub/stats/afrinic/delegated-afrinic-extended-latest",
]
BLOCK = 16

# Told before any list: the private networks (IPv6 visitors reach Traefik from the gateway of
# docker_gwbridge, 172.30.x.1, stacks/edge.yml "ipv6"; the checks of the server come from inside),
# and every IPv6 address, so that the day Traefik sees IPv6 clients itself nobody is shut out
# before the list has them.
ALWAYS_OPEN = ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16", "127.0.0.0/8", "::/0"]

# What every agent may fetch from anywhere, as crawlers.yml.
EXCEPTIONS = """        && !Path(`/robots.txt`) && !Path(`/healthz`) && !Path(`/livez`)
        && !PathRegexp(`^/([a-z]{2}/)?calendar/`)"""


def fetch(url):
    with urllib.request.urlopen(url, timeout=120) as answer:
        return answer.read()


def clouds():
    found, counts = [], {}
    for asn, name in CLOUDS.items():
        data = json.loads(fetch(ANNOUNCED.format(asn=asn)))
        if data.get("status") != "ok":
            sys.exit(f"RIPEstat answered {data.get('status')!r} for AS{asn}: {data.get('messages')}")
        networks = [ipaddress.ip_network(p["prefix"], strict=False) for p in data["data"]["prefixes"]]
        if not networks:
            sys.exit(f"AS{asn} ({name}) announces nothing: RIPEstat's answer is not to be trusted, the files stay as they are")
        counts[asn] = len(networks)
        found += networks
    v4 = list(ipaddress.collapse_addresses(n for n in found if n.version == 4))
    v6 = list(ipaddress.collapse_addresses(n for n in found if n.version == 6))
    return v4 + v6, counts, len(v4), len(v6)


def europe():
    """The /16 blocks with any European address in them, merged; and what that lets through."""
    size = 1 << (32 - BLOCK)
    european, other = collections.Counter(), collections.Counter()
    for url in REGISTRIES:
        lines = fetch(url).decode("utf-8", errors="replace").splitlines()
        if len(lines) < 1000:
            sys.exit(f"{url} has {len(lines)} lines: not to be trusted, the files stay as they are")
        for line in lines:
            f = line.split("|")
            if len(f) < 7 or f[1] == "*" or f[2] != "ipv4" or f[6] not in ("allocated", "assigned"):
                continue
            pos = int(ipaddress.IPv4Address(f[3]))
            end = pos + int(f[4])
            counter = european if f[1] in EUROPE else other
            while pos < end:
                upto = min(end, (pos // size + 1) * size)
                counter[pos // size] += upto - pos
                pos = upto
    open_blocks = set(european)
    networks = list(ipaddress.collapse_addresses(ipaddress.IPv4Network((b * size, BLOCK)) for b in open_blocks))
    networks.sort(key=lambda n: (n.prefixlen, n.network_address))
    let_in = sum(other[b] for b in open_blocks) / max(1, sum(other.values()))
    return networks, let_in


def served_agents():
    """The search engines and link previews of crawlers.yml, which crawl from everywhere."""
    text = (DYNAMIC / "crawlers.yml").read_text(encoding="utf-8")
    found = re.search(r"&& !HeaderRegexp\(`User-Agent`, `\(\?i\)\(([^)]*)\)`\)", text)
    if not found:
        sys.exit("crawlers.yml names no agents it serves")
    # "cubot" is a phone there, not an exception from the place.
    return [agent for agent in found.group(1).split("|") if agent != "cubot"]


def matchers(networks):
    return "\n        || ".join(f"ClientIP(`{n}`)" for n in networks)


def main():
    today = datetime.date.today().isoformat()

    cloud_networks, counts, n4, n6 = clouds()
    listed = "\n".join(f"#   AS{asn:<7} {name}: {counts[asn]} networks announced" for asn, name in CLOUDS.items())
    (DYNAMIC / "crawler-networks.yml").write_text(
        f"""# Written by deploy/edge-networks.py on {today}: do not edit, run it again. Why, and how often:
# its docstring.
#
# Every request from a network of these clouds gets 403, whatever its user agent says, and never
# reaches a site: their crawlers pass for Chrome and start the app, so that the dashboard "Visitors"
# counted them as people. The same exceptions as crawlers.yml (robots.txt, the calendar feeds,
# /healthz, /livez) and the same answer (its middleware and service); the dashboard counts these
# as "crawlers turned away" too (RouterName edge-crawler-networks@file).
#
{listed}
#   merged: {n4} IPv4 and {n6} IPv6 networks. Traefik sees no IPv6 client so far
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
        ({matchers(cloud_networks)})
{EXCEPTIONS}
      priority: 10000
      tls: {{}}
      middlewares:
        - edge-crawlers-refuse
      service: edge-crawlers-nowhere
""",
        encoding="utf-8",
        newline="\n",
    )

    european, let_in = europe()
    agents = served_agents() + ["google-site-verification", "mastodon", "bluesky", "skypeuripreview"]
    countries = "\n#   ".join(textwrap.wrap(" ".join(sorted(EUROPE)), 96))
    (DYNAMIC / "europe.yml").write_text(
        f"""# Written by deploy/edge-networks.py on {today}: do not edit, run it again. Why, how the list is
# made, and how often: its docstring.
#
# Every request from outside Europe gets 403 and never reaches a site (owner, 2026-11-08: Betula is
# for the BTU; America, South-East Asia and so on need not reach it). Europe:
#   {countries}
# {len(european)} networks, blocks of /{BLOCK} with any European address; under {100 * let_in:.0f} % of the
# addresses elsewhere pass with them. Open from anywhere:
#   - the private networks and every IPv6 address: IPv6 visitors reach Traefik from the gateway of
#     docker_gwbridge (stacks/edge.yml, "ipv6"), so their place cannot be told;
#   - the search engines and link previews that crawlers.yml serves (they crawl from America),
#     Google's verification of the Search Console, the link previews of Mastodon, Bluesky, Teams and
#     Skype; told by their user agents, which can be faked;
#   - robots.txt, the calendar feeds (Google's and Apple's calendars fetch them from America),
#     /healthz and /livez, as crawlers.yml.
# Below the routers of crawlers.yml and crawler-networks.yml (priority 9000), with their middleware
# and service; the dashboard "Visitors" counts these as "outside Europe" (RouterName
# edge-outside-europe@file). A student abroad outside Europe is turned away as well.

http:
  routers:
    edge-outside-europe:
      entryPoints:
        - websecure
      rule: >-
        !({matchers(ALWAYS_OPEN + [str(n) for n in european])})
        && !HeaderRegexp(`User-Agent`, `(?i)({"|".join(agents)})`)
{EXCEPTIONS}
      priority: 9000
      tls: {{}}
      middlewares:
        - edge-crawlers-refuse
      service: edge-crawlers-nowhere
""",
        encoding="utf-8",
        newline="\n",
    )
    print(f"crawler-networks.yml: {n4} IPv4 + {n6} IPv6 networks of {sum(counts.values())} announced", file=sys.stderr)
    print(f"europe.yml: {len(european)} networks, {100 * let_in:.1f} % of the other addresses pass", file=sys.stderr)


if __name__ == "__main__":
    main()
