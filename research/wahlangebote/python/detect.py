# The detection folia/design/wahlangebote/konzept.html proposes, run on a snapshot: per module and
# semester (per location course where the module has courses), which events are extras, which form
# a choice, and of what kind (program, group, topic). No module is named anywhere: SWS, forms,
# titles, QIS's audiences.
#   python3 detect.py catalog.db 2026W detect.json
import sqlite3, sys, re, json, statistics
from collections import defaultdict
db = sqlite3.connect(f"file:{sys.argv[1]}?mode=ro", uri=True)
SEM = sys.argv[2] if len(sys.argv) > 2 else "2026W"
CONTACT = ("lecture", "exercise", "practical", "seminar", "project", "tutorial", "consultation", "excursion", "other")
SHARE = 0.75
GROUP = re.compile(r"\s*[\(\[]?\s*(?:\bGruppe\s*\d+\b|\b\d+\.\s*Gruppe\b|\b[A-D]-Kurs\b)\s*[\)\]]?", re.I)
EXTRA = re.compile(r"Tutori|Aufbaukurs|Sprechstunde|Fragestunde|Repetitori|Vorkurs|Brückenkurs", re.I)
KINDWORD = re.compile(r"^\s*(?:Vorlesung|Übung|Seminar|Praktikum|Projekt|Tutorium|Laborpraktikum|Proseminar|Oberseminar)\s*[:\-–]?\s*", re.I)
WORD = re.compile(r"[a-zäöüß0-9]+", re.I)

title = dict(db.execute("select id, title from module"))
msws = defaultdict(lambda: defaultdict(float))
for mid, form, sws in db.execute("select module_id, form, sws from module_teaching_form where sws > 0"):
    if form in CONTACT: msws[mid][form] += sws
forms = defaultdict(set)
for eid, form in db.execute("select event_id, form from event_form"): forms[eid].add(form)
aud = defaultdict(set)
for eid, fk in db.execute("select distinct r.event_id, p.family_key from event_program_ref r join program p on p.id = r.program_id"): aud[eid].add(fk)
for eid, label in db.execute("select event_id, label from event_program where resolve_status = 'unresolved'"): aud[eid].add("label:" + label)
track = {(m, e): c for m, e, c in db.execute("select module_id, event_id, campus from module_track where semester_key = ?", (SEM,))}
mods = defaultdict(list)
for mid, eid, et, sws in db.execute("""select me.module_id, e.id, e.title, ifnull(e.sws, 0) from module_event me
        join event e on e.id = me.event_id and e.category = 'teaching' where e.semester_key = ?""", (SEM,)):
    mods[mid].append(dict(id=eid, title=et, sws=float(sws), kind=frozenset(forms[eid]) or frozenset({"other"})))

def stem(t):
    t = GROUP.sub(" ", t)
    t = KINDWORD.sub("", t)
    return re.sub(r"\s+", " ", t).strip(" -–:/").lower()

def parts_by_title(module_title, options):
    """The module's title names each option apart: „Werkstoff- und Physiklabor" over „… Werkstofflabor" and „… Physiklabor"."""
    mt = [w.lower() for w in WORD.findall(module_title)]
    toks = [set(w.lower() for w in WORD.findall(o["title"])) for o in options]
    common = set.intersection(*toks) if toks else set()
    def named(w): return any(w.startswith(m) or m.startswith(w) for m in mt if len(m) >= 4 and len(w) >= 4)
    own = [t - common for t in toks]
    return all(o and any(named(w) for w in o) for o in own)

MARKER = {"für", "for", "und", "and", "the", "der", "die", "das", "den", "online", "präsenz", "gruppe", "group", "kurs", "a", "b", "c", "d"}
PART = re.compile(r"^(?:t\d+|teil|part|i|ii|iii|iv|v)$")

def words(t):
    return {w.lower() for w in WORD.findall(KINDWORD.sub("", t))}

def parts_apart(options):
    """Titles that differ only by a part's number (T1, Teil 2, II): parts of the module, not a choice."""
    ws = [words(o["title"]) for o in options]
    for i in range(len(ws)):
        for j in range(i + 1, len(ws)):
            diff = ws[i] ^ ws[j]
            if diff and all(PART.match(w) for w in diff):
                return True
    return False

def alike(options):
    """The same content more than once: titles alike but for a group, a variant („(Online)") or an audience („für MT")."""
    ws = [words(o["title"]) - MARKER for o in options]
    for i in range(len(ws)):
        for j in range(i + 1, len(ws)):
            a, b = ws[i], ws[j]
            if not a or not b or len(a & b) / len(a | b) < 0.6:
                return False
    return True

def similar(sizes):
    sized = [x for x in sizes if x > 0]
    return bool(sized) and max(sized) <= 2 * min(sized)

def classify(options):
    """'program' or 'group': one content more than once, by whom QIS names at each or not; 'topic':
    different contents to choose from. Whom QIS names per option refines it per program."""
    if not alike(options):
        return "topic"
    sets = [aud[o["id"]] for o in options]
    if all(sets) and len({frozenset(s) for s in sets}) > 1:
        return "program"
    return "group"

def choice(options, need, pick, basis):
    return dict(kind=classify(options), basis=basis, pick=pick, need=need, options=options)

def units_of(events):
    """Events of one title that are of different forms belong together (a lecture and its seminar of
    one title); a title held more than once in one form is the module's own course, each event apart."""
    by = defaultdict(list)
    for e in events: by[stem(e["title"])].append(e)
    units = []
    for st, es in by.items():
        kinds = [e["kind"] for e in es]
        if len(es) > 1 and len(set(kinds)) == len(kinds):
            units.append(dict(id=",".join(e["id"] for e in es), title=es[0]["title"], sws=sum(e["sws"] for e in es),
                              kind=frozenset().union(*kinds), events=es))
        else:
            units.extend(dict(e, events=[e]) for e in es)
    return units

def guarded(mid, options):
    return parts_by_title(title.get(mid, ""), options) or parts_apart(options)

def overlap(options, stated):
    ks = [o["kind"] for o in options]
    if all(not (k & stated) for k in ks) and len(set(ks)) == 1:
        return True                       # all of one form the module does not state (QIS's „Projekt", the module's „Seminar")
    return all(a & b & stated for i, a in enumerate(ks) for b in ks[i + 1:])

def surplus(sizes, need):
    unit = statistics.median(sizes)
    return sum(sizes) >= 1.25 * need and sum(sizes) >= need + SHARE * unit

def scope_choices(mid, events):
    S = msws[mid]
    stated = frozenset(f for f, v in S.items() if v > 0)
    total = sum(S.values())
    out = dict(extras=[], choices=[])
    if total <= 0: return out
    rest = []
    # 1. extras: a tutorial the module states no SWS for, or a title naming a support format, where the rest still covers the module's SWS of that form
    for e in events:
        named = EXTRA.search(e["title"]) is not None
        tutorial = "tutorial" in e["kind"] and S.get("tutorial", 0) <= 0
        if named or tutorial:
            others = sum(o["sws"] for o in events if o is not e and o["kind"] & (e["kind"] - {"tutorial"}) and not EXTRA.search(o["title"]))
            need = sum(S.get(f, 0) for f in e["kind"] - {"tutorial"})
            if need <= 0 or others >= SHARE * need:
                out["extras"].append(e); continue
        rest.append(e)
    used = set()
    # 2. explicit groups: titles alike but for „Gruppe N", „A-Kurs"
    by_stem = defaultdict(list)
    for e in rest:
        if GROUP.search(e["title"]): by_stem[stem(e["title"])].append(e)
    for st, es in by_stem.items():
        if len(es) >= 2:
            out["choices"].append(dict(kind="group", basis="title", pick=1, need=None, options=es)); used |= {e["id"] for e in es}
    units = units_of([e for e in rest if e["id"] not in used])
    # 3. a catalogue of whole modules: units that each hold the module's SWS
    sized = [u for u in units if u["sws"] > 0]
    if len(sized) >= 2:
        whole = [u for u in sized if u["sws"] >= SHARE * total]
        if len(whole) == len(sized) and similar([u["sws"] for u in whole]) and surplus([u["sws"] for u in whole], total) and overlap(whole, stated) and not guarded(mid, whole):
            pick = max(1, round(total / statistics.median(u["sws"] for u in whole)))
            kinds = {u["kind"] for u in whole}
            options = whole + [u for u in units if u["sws"] <= 0 and u["kind"] in kinds]
            if len(options) > pick:
                out["choices"].append(choice(options, total, pick, "module"))
                return out
    # 4. per form: units of one form holding more than its SWS, each a share of them
    by_kind = defaultdict(list)
    for u in units: by_kind[u["kind"]].append(u)
    mismatch = not any(u["kind"] & stated for u in units)
    for kind, us in by_kind.items():
        need = sum(S.get(f, 0) for f in kind) or (total if mismatch else 0)
        sz = [u for u in us if u["sws"] > 0]
        if need <= 0 or len(sz) < 2: continue
        sizes = [u["sws"] for u in sz]
        if not surplus(sizes, need) or not similar(sizes): continue
        pick = max(1, round(need / statistics.median(sizes)))
        if len(us) <= pick or any(x < 0.5 * need / pick for x in sizes) or any(x > 1.25 * need for x in sizes): continue
        if guarded(mid, sz): continue
        out["choices"].append(choice(us, need, pick, "form"))
    return out

result = {}
for mid, evs in mods.items():
    courses = sorted({track.get((mid, e["id"])) for e in evs if track.get((mid, e["id"]))})
    scopes = [[e for e in evs if track.get((mid, e["id"])) in (c, None)] for c in courses] or [evs]
    found = [scope_choices(mid, s) for s in scopes]
    extras = {e["id"] for f in found for e in f["extras"]}
    choices = [(c["kind"], c["basis"], c["pick"], len(c["options"]), c["need"]) for f in found for c in f["choices"]]
    if extras or choices:
        result[mid] = dict(title=title.get(mid), extras=sorted(extras), choices=choices,
                           detail=[[dict(kind=c["kind"], pick=c["pick"], options=[(o["id"], o["title"][:40], o["sws"], sorted(aud[o["id"]])[:4]) for o in c["options"]]) for c in f["choices"]] for f in found])
json.dump(result, open(sys.argv[3], "w"), ensure_ascii=False, indent=1)
from collections import Counter
print(len(mods), "modules;", len(result), "with extras or choices")
print("choices by kind:", Counter(k for r in result.values() for (k, *_ ) in r["choices"]))
print("modules with extras:", sum(1 for r in result.values() if r["extras"]))
