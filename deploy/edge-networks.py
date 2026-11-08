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
Europe has some 36,000 IPv4 networks there, and Traefik v3 takes one network per ClientIP, so that
many would cost every request milliseconds: the list is made of blocks of 65,536 addresses (/16)
instead, and a block is open as soon as any part of it is European. Of European addresses none is
turned away; of the others under 4 % pass (counted 2026-11-08).

IPv6 the same way, in blocks of /32 (what a provider gets: one /32 holds 65,536 customers' /48s):
some 20,900 European networks make 16,900 blocks. A block then grows into its neighbours as long
as that lets in no network the blocks keep out, i.e. over space nobody has been given yet: 8,300
networks (2026-11-08), and of the networks outside Europe no more pass than with the blocks alone
(15 % of them, mostly single /48s of the registries' shared /32s; by addresses under 0.0001 %).
Coarser blocks would shorten the list but let in whole regions: at /26 America's and South Asia's
providers start to pass, next to a few European networks moved into their registries' space.
Measured with Traefik 3.7: one ClientIP costs about 0.27 µs (2026-11-08, on a laptop; the server
is slower), so a request from outside Europe pays for every matcher of its family, about 2 ms for
the IPv6 list.

Each family has its own list behind a guard (ClientIP(`0.0.0.0/0`), ClientIP(`::/0`)), so that an
IPv4 request never walks the IPv6 list and the other way round. Within a list the networks that
hold German addresses come first (Betula is for the BTU: German visitors are told before the rest
of Europe is tried), then the large ones.

Needs Python 3.8+ and nothing else (urllib, json, ipaddress).
"""

import bisect
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
BLOCK6 = 32
# How far an IPv6 block may grow over space nobody has been given.
GROW6 = 16

# Told before each list: the private networks. The checks of the server come from inside; IPv6
# visitors reach Traefik from the gateway of docker_gwbridge (172.30.x.1) while it has no IPv6
# (stacks/edge.yml, "ipv6"), and with it a container that asks the public address over IPv6
# arrives through docker-proxy from the bridge's unique local gateway.
ALWAYS_OPEN4 = ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16", "127.0.0.0/8"]
ALWAYS_OPEN6 = ["::1/128", "fc00::/7"]

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
    return v4, v6, counts


class Spans:
    """Networks as sorted, disjoint address ranges: which one holds an address, and whether any
    meets a range."""

    def __init__(self, networks):
        nets = sorted(ipaddress.collapse_addresses(networks))
        self.starts = [int(n.network_address) for n in nets]
        self.ends = [int(n.broadcast_address) for n in nets]

    def meets(self, start, end):
        i = bisect.bisect_right(self.starts, end) - 1
        return i >= 0 and self.ends[i] >= start

    def holds(self, address):
        return self.meets(address, address)


def german_first(networks, german):
    """Networks with German addresses first, then the large ones, then by address."""
    return sorted(
        networks,
        key=lambda n: (not german.meets(int(n.network_address), int(n.broadcast_address)), n.prefixlen, n.network_address),
    )


def europe():
    """Europe's networks per family, German first, and how much of the rest passes with them:
    IPv4 in /16 blocks with any European address in them, merged; IPv6 in /32 blocks, grown over
    unallocated space."""
    size = 1 << (32 - BLOCK)
    european, other, german4 = collections.Counter(), collections.Counter(), set()
    european6, other6, german6 = [], [], []
    for url in REGISTRIES:
        lines = fetch(url).decode("utf-8", errors="replace").splitlines()
        if len(lines) < 1000:
            sys.exit(f"{url} has {len(lines)} lines: not to be trusted, the files stay as they are")
        for line in lines:
            f = line.split("|")
            if len(f) < 7 or f[1] == "*" or f[6] not in ("allocated", "assigned"):
                continue
            if f[2] == "ipv6":
                net = ipaddress.IPv6Network(f"{f[3]}/{f[4]}", strict=False)
                (european6 if f[1] in EUROPE else other6).append(net)
                if f[1] == "DE":
                    german6.append(net)
                continue
            if f[2] != "ipv4":
                continue
            pos = int(ipaddress.IPv4Address(f[3]))
            end = pos + int(f[4])
            counter = european if f[1] in EUROPE else other
            while pos < end:
                upto = min(end, (pos // size + 1) * size)
                counter[pos // size] += upto - pos
                if f[1] == "DE":
                    german4.add(pos // size)
                pos = upto
    if len(european6) < 1000:
        sys.exit(f"the registries name {len(european6)} European IPv6 networks: not to be trusted, the files stay as they are")
    open_blocks = set(european)
    v4 = list(ipaddress.collapse_addresses(ipaddress.IPv4Network((b * size, BLOCK)) for b in open_blocks))
    v4 = german_first(v4, Spans(ipaddress.IPv4Network((b * size, BLOCK)) for b in german4))
    let_in4 = sum(other[b] for b in open_blocks) / max(1, sum(other.values()))

    # IPv6: the /32 blocks, then each grown while it meets no network outside Europe that the blocks
    # keep out (only space given to nobody joins it).
    blocks = Spans(n if n.prefixlen <= BLOCK6 else n.supernet(new_prefix=BLOCK6) for n in european6)
    kept_out = Spans(n for n in other6 if not blocks.holds(int(n.network_address)))
    grown = []
    for n in ipaddress.collapse_addresses(n if n.prefixlen <= BLOCK6 else n.supernet(new_prefix=BLOCK6) for n in european6):
        while n.prefixlen > GROW6:
            up = n.supernet()
            if kept_out.meets(int(up.network_address), int(up.broadcast_address)):
                break
            n = up
        grown.append(n)
    v6 = german_first(list(ipaddress.collapse_addresses(grown)), Spans(german6))
    open6 = Spans(v6)
    passing = [n for n in other6 if open6.holds(int(n.network_address))]
    let_in6 = len(passing) / max(1, len(other6))
    return v4, let_in4, v6, let_in6, len(european6)


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


def by_family(v4, v6, outside=False):
    """A request of either family in its networks (outside=True: not in them), each family's list
    behind a guard that only its addresses pass, so that a request never walks the other list."""
    parts = [
        f"(ClientIP(`{guard}`) && {'!' if outside else ''}({matchers(networks)}))"
        for guard, networks in (("0.0.0.0/0", v4), ("::/0", v6))
        if networks
    ]
    return "(" + "\n        || ".join(parts) + ")"


def main():
    today = datetime.date.today().isoformat()

    cloud4, cloud6, counts = clouds()
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
#   merged: {len(cloud4)} IPv4 and {len(cloud6)} IPv6 networks, each family behind its guard (an
#   IPv4 request never walks the IPv6 list, and the other way round).
#
# ClientIP judges the address the connection comes from, never X-Forwarded-For: that is the client
# (for IPv6 once docker_gwbridge has IPv6, stacks/edge.yml "ipv6"). Somebody with a VPN whose exit
# is in one of these clouds is turned away as well; that is the price.

http:
  routers:
    edge-crawler-networks:
      entryPoints:
        - websecure
      rule: >-
        {by_family(cloud4, cloud6)}
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

    v4, let_in4, v6, let_in6, n_eu6 = europe()
    agents = served_agents() + ["google-site-verification", "mastodon", "bluesky", "skypeuripreview"]
    countries = "\n#   ".join(textwrap.wrap(" ".join(sorted(EUROPE)), 96))
    (DYNAMIC / "europe.yml").write_text(
        f"""# Written by deploy/edge-networks.py on {today}: do not edit, run it again. Why, how the list is
# made, and how often: its docstring.
#
# Every request from outside Europe gets 403 and never reaches a site (owner, 2026-11-08: Betula is
# for the BTU; America, South-East Asia and so on need not reach it). Europe:
#   {countries}
# IPv4: {len(v4)} networks, blocks of /{BLOCK} with any European address; under {max(1, round(100 * let_in4))} % of the
# addresses elsewhere pass with them. IPv6: {len(v6)} networks, blocks of /{BLOCK6} with any of the {n_eu6}
# European networks, grown over space given to nobody; {round(100 * let_in6)} % of the networks elsewhere pass
# with them, mostly single /48s in blocks the registries share out. Each family behind its guard,
# networks with German addresses first, then the large ones. Open from anywhere:
#   - the private networks: the checks of the server, and every IPv6 visitor while docker_gwbridge
#     has no IPv6 (they reach Traefik from its gateway then, stacks/edge.yml "ipv6");
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
        {by_family(ALWAYS_OPEN4 + [str(n) for n in v4], ALWAYS_OPEN6 + [str(n) for n in v6], outside=True)}
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
    print(f"crawler-networks.yml: {len(cloud4)} IPv4 + {len(cloud6)} IPv6 networks of {sum(counts.values())} announced", file=sys.stderr)
    print(f"europe.yml: {len(v4)} IPv4 networks, {100 * let_in4:.1f} % of the other addresses pass;"
          f" {len(v6)} IPv6 networks, {100 * let_in6:.1f} % of the other networks pass", file=sys.stderr)


if __name__ == "__main__":
    main()
