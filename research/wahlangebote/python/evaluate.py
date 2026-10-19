# The detection (detect.py's output) against the manual review of the same snapshot.
#   python3 evaluate.py detect.json ../data/manual-2026W.json [module …]
import json, sys
from collections import Counter

det = json.load(open(sys.argv[1]))
manual = {m: v["class"] for m, v in json.load(open(sys.argv[2]))["modules"].items()}

def kinds(m):
    r = det.get(m)
    if not r:
        return set()
    out = {c[0] for c in r["choices"]}
    if r["extras"]:
        out.add("extra")
    return out

for k in "BACD":
    ms = [m for m, c in manual.items() if c == k]
    cnt = Counter(frozenset(kinds(m)) for m in ms)
    print(f"  {k} ({len(ms)}):", {",".join(sorted(x)) or "nothing": n for x, n in cnt.most_common()})
print("outside the manual review:", sorted((m, tuple(sorted(kinds(m)))) for m in det if m not in manual))
for m in sys.argv[3:]:
    print(m, det.get(m, {}).get("choices"), det.get(m, {}).get("extras"))
