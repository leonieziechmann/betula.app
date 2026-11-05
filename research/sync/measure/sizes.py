"""Rough sizes of a Betula profile in its own store formats (folia/crates/plans/src/studyplan.rs,
stores/src/bookmarks.rs, collections.rs), raw, deflated, and as an encrypted envelope."""
import os, random, zlib, json
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305

random.seed(7)
SEMS = ["2023W", "2024S", "2024W", "2025S", "2025W", "2026S", "2026W", "2027S", "2027W", "2028S", "2028W", "2029S"]
PROG = "079-82-2008"
WORDS = ("Modul Vorlesung Übung Seminar Prüfung Klausur Projekt Labor Praktikum Mathematik Informatik "
         "Algorithmen Datenstrukturen Netze Rechner Systeme Theorie Software Technik Grundlagen Analysis "
         "lernen wiederholen Altklausur Gruppe Abgabe Termin Freitag Montag wichtig vielleicht nehmen mit "
         "Lena zusammen Raum Bibliothek Notiz Folien Skript Tutorium").split()

def mid(): return str(random.randint(11000, 15999))
def eid(): return str(random.randint(300000, 349999))
def ts(): return str(random.randint(1_760_000_000, 1_795_000_000))
def text(n): return " ".join(random.choice(WORDS) for _ in range(n))

def plan(sems, per_sem, placeholders, hides, passed, own, calendars, cal_mods):
    lines = []
    for s in SEMS[:sems]:
        for _ in range(per_sem):
            lines.append(f"m\t{s}\t{mid()}\t{ts()}\t\t" + random.choice(["", "plan"]))
    for p in range(placeholders):
        lines.append(f"p\t{p+1}\t{random.choice(SEMS[:sems])}\t{PROG}\t{random.randint(1,300)}\t2-3\t6\telective\t\t{text(4)}")
    for _ in range(hides):
        lines.append(random.choice([f"k\t{random.choice(SEMS[:sems])}\tue,tut",
                                    f"e\t{random.choice(SEMS[:sems])}\t{eid()}",
                                    f"r\t{random.choice(SEMS[:sems])}\t{eid()}-{random.randrange(16**5):05x}",
                                    f"c\t{random.choice(SEMS[:sems])}\t{eid()}-{random.randrange(16**5):05x}"]))
    for _ in range(passed):
        lines.append(f"d\t{random.choice(SEMS[:sems])}\t{mid()}")
    for n in range(own):
        lines.append(f"t\t{n+1}\t0\t{SEMS[sems-1]}\tw\t{random.randint(1,5)}\t10:00-11:30\t\t\t\t{text(3)}\t{text(1)}\t")
    for c in range(calendars):
        lines.append(f"l\t{c+1}\tstrip\t{PROG}\t{random.choice(['Lena', 'Max', 'Sam', 'Uni-Sport'])}")
        for _ in range(cal_mods):
            lines.append(f"n\t{c+1}\t{SEMS[sems-1]}\t{mid()}")
    return "\n".join(lines) + "\n"

def bookmarks(n): return "".join(f"{mid()}\t{ts()}\n" for _ in range(n))

def collections(cols, notes, note_words):
    out = [f"c\t{i+1}\t{text(2)}" for i in range(cols)]
    out += [f"i\t{mid()}\t{random.randint(1, max(cols,1))}" for _ in range(notes * 2)]
    out += [f"n\t{mid()}\t" + text(note_words).replace("\n", " ") for _ in range(notes)]
    return "\n".join(out) + "\n"

def saved(k, per):
    return "".join(f"plan\t{text(2)}\t\n" + plan(1, per, 1, 3, 0, 0, 0, 0) for _ in range(k))

MINE = f"program\t{PROG}\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2025W\n"

profiles = {
    "light (one semester planned, a few marks)": dict(plan=plan(1, 6, 0, 2, 0, 0, 0, 0), bm=bookmarks(15), col=collections(0, 0, 0), saved="", mine=MINE),
    "typical (3 semesters, passed, a friend's calendar)": dict(plan=plan(4, 6, 6, 15, 12, 4, 1, 5), bm=bookmarks(60), col=collections(3, 6, 25), saved=saved(2, 6), mine=MINE),
    "heavy (whole study, many notes)": dict(plan=plan(10, 8, 30, 120, 60, 50, 3, 20), bm=bookmarks(256), col=collections(20, 60, 80), saved=saved(10, 7), mine=MINE),
}

key = ChaCha20Poly1305.generate_key()
aead = ChaCha20Poly1305(key)
print(f"{'profile':55} {'raw':>9} {'deflate':>9} {'sealed':>9}  per store sealed (plan / marks / notes / saved / mine)")
for name, p in profiles.items():
    blob = json.dumps(p, ensure_ascii=False).encode()
    raw = sum(len(v.encode()) for v in p.values())
    z = zlib.compress(blob, 9)
    sealed = 24 + len(aead.encrypt(os.urandom(12), z, b"hdr")) + 32  # nonce + ct+tag + header
    per = []
    for k in ("plan", "bm", "col", "saved", "mine"):
        v = p[k].encode()
        per.append(24 + len(aead.encrypt(os.urandom(12), zlib.compress(v, 9), b"h")) + 32 if v else 0)
    print(f"{name:55} {raw:9,} {len(z):9,} {sealed:9,}  {' / '.join(f'{x:,}' for x in per)}")

caps = 2*1024*1024 + 512*1024 + 2*1024*1024 + 2000*18 + 4*1024
print(f"\nupper bound by the stores' own read caps (2 MiB plan + 512 KiB saved + 2 MiB collections + 2000 marks + 4 KiB mine): {caps:,} bytes = {caps/1048576:.2f} MiB raw")
# A pack code like the ones the calendar and share links carry today
print("a calendar/share part holds one pack code: a few hundred bytes (today's /calendar/<code>.ics codes)")
