"""What sync would cost the server, by number of students who use it (BTU: about 7,500 in 2026)."""
KB, MB, GB = 1e3, 1e6, 1e9
TYPICAL, HEAVY = 2.27 * KB, 15.4 * KB       # sealed profile, measured with sizes.py
MIX = 0.7 * TYPICAL + 0.3 * HEAVY           # average sealed profile
PROFILES = 1.3                              # profiles per user that sync
VERSIONS = 4                                # current + 3 kept for "Stand wiederherstellen"
PAD = 1.25                                  # padding to size buckets
ACTIVE = 0.4                                # share of users active on a day
PUSH, PUSH_B = 40, 1.5 * KB                 # pushes per active day (debounced), bytes each (one store)
PULL304, PULL304_B = 40, 0.4 * KB           # pulls answered 304 (headers only)
PULL, PULL_B = 10, 3 * KB                   # pulls with content
OPEN = 0.08                                 # share of users with a visible tab at the peak
SVC_CONN, PROXY_CONN = 19.2 * KB, 63 * KB   # measured: axum SSE / Go reverse proxy per idle stream
RTC_SESS, RTC_TURN, RTC_RELAY_B = 1.0, 0.2, 0.25 * MB   # P2P sessions per active day, share via TURN, bytes per relayed session (30 min: consent checks, SCTP heartbeats, payload)

print(f"average sealed profile {MIX/KB:.1f} KB")
print(f"{'users':>6} | {'disk':>8} | {'traffic/day':>11} | {'traffic/month':>13} | {'req/s avg':>9} | {'open tabs':>9} | {'RAM live (svc+proxy)':>20} | {'TURN/month':>10}")
for users in (500, 2000, 7500):
    disk = users * PROFILES * MIX * VERSIONS * PAD
    active = users * ACTIVE
    day = active * (PUSH * PUSH_B + PULL304 * PULL304_B + PULL * PULL_B)
    reqs = active * (PUSH + PULL304 + PULL) / 86400
    tabs = users * OPEN
    ram = tabs * (SVC_CONN + PROXY_CONN)
    turn = active * RTC_SESS * RTC_TURN * RTC_RELAY_B * 30
    print(f"{users:6,} | {disk/MB:6.1f} MB | {day/MB:8.0f} MB | {day*30/GB:10.1f} GB | {reqs:9.2f} | {tabs:9,.0f} | {ram/MB:17.1f} MB | {turn/GB:7.2f} GB")
print("\nfor comparison: one first visit with JavaScript downloads about 10 MB (docs/folia/frontend.md, Load)")
