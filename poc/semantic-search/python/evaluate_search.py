"""How well the semantic search finds modules, measured on the deployed pipeline: the browser's
packed model, searched by the crate itself (target/release/embed --search, the bits the browser
computes), against the index of a snapshot's vectors (v_module_vector, schema 10) or an index file.

    python evaluate_search.py MODEL.bin catalog.db              # the snapshot's v_module_vector
    python evaluate_search.py MODEL.bin catalog.db --index index.bin

Two sets of queries (../data, written by a language model, Claude):
- known-item (eval-known): for 600 sampled modules, queries students would type for exactly what
  the module offers — short keywords, German and English paraphrases without the title's words, a
  goal, and an indirect situation („mein quadrocopter wackelt …“). Measured: the rank of the module
  (or a module of the same title) — recall@1, @10, MRR. finetune.py leaves these modules out.
- open (eval-open): 344 realistic queries written without seeing the catalog (sets a, b) and by
  personas (c: international students in English, d: first-semester and undecided, e: advanced and
  part-time students), with graded relevance of the modules the systems found (eval-judgments, 0 not
  relevant, 1 partly, 2 a good answer; a module nobody judged counts as 0 and is reported as a hole).
  Measured: the share of relevant modules in the first 10 (P@10), the good ones (good@10), nDCG@10.
Modules with the same text count once in a result list.
"""

from __future__ import annotations

import argparse
import struct
import subprocess
import tempfile
from collections import defaultdict
from pathlib import Path

import numpy as np

from finetune import DATA, collapse, read_jsonl, text_hash

EMBED = Path(__file__).resolve().parents[3] / "target" / "release" / "embed"


def index_from_snapshot(db: Path) -> bytes:
    """E5I2 (semantic/src/index.rs) from v_module_vector."""
    import sqlite3
    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    rows = con.execute("SELECT module_id, scale, vector FROM v_module_vector ORDER BY module_id").fetchall()
    if not rows:
        raise SystemExit(f"{db}: no vectors (v_module_vector is empty)")
    dims = len(rows[0][2])
    out = b"E5I2" + struct.pack("<II", len(rows), dims)
    out += b"".join(struct.pack("<H", len(i.encode())) + i.encode() for i, _, _ in rows)
    out += b"".join(struct.pack("<f", s) for _, s, _ in rows)
    out += b"".join(v for _, _, v in rows)
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("model", help="the browser's packed model")
    ap.add_argument("snapshot", help="a catalog snapshot: the modules' texts, and the vectors unless --index")
    ap.add_argument("--index", help="an index file (E5I2) instead of the snapshot's vectors")
    args = ap.parse_args()

    import sqlite3
    con = sqlite3.connect(f"file:{args.snapshot}?mode=ro", uri=True)
    hash_of, group_of = {}, {}
    for mid, de, en, c, o in con.execute("SELECT id, COALESCE(title_de, title), COALESCE(title_en, ''), COALESCE(contents, ''), "
                                         "COALESCE(learning_outcomes, '') FROM v_module"):
        hash_of[mid] = text_hash(de, en, c, o)
        group_of[hash_of[mid]] = collapse(de).lower()

    with tempfile.NamedTemporaryFile(suffix=".bin") as f:
        if args.index:
            f.write(Path(args.index).read_bytes())
        else:
            f.write(index_from_snapshot(Path(args.snapshot)))
        f.flush()

        def search(queries: list[str], k: int = 10) -> list[list[str]]:
            out = subprocess.run([str(EMBED), args.model, "--search", f.name, "--k", str(4 * k)], check=True,
                                 input="\n".join(q.replace("\n", " ") for q in queries) + "\n",
                                 capture_output=True, text=True).stdout
            lists = []
            for line in out.splitlines():
                seen, hits = set(), []
                for hit in line.split("\t")[1].split():
                    h = hash_of.get(hit.rsplit(":", 1)[0])
                    if h and h not in seen:
                        seen.add(h)
                        hits.append(h)
                lists.append(hits[:k])
            return lists

        known = [r for r in read_jsonl(DATA / "eval-known.jsonl.gz") if r["text_hash"] in group_of]
        stats = defaultdict(lambda: [0, 0.0, 0, 0])
        for r, hits in zip(known, search([r["query"] for r in known])):
            rank = next((i + 1 for i, h in enumerate(hits) if h == r["text_hash"] or group_of[h] == group_of[r["text_hash"]]), 0)
            for name in (r["kind"], "all"):
                s = stats[name]
                s[0] += 1
                s[1] += 1 / rank if rank else 0
                s[2] += rank == 1
                s[3] += rank > 0
        print(f"known-item ({len({r['text_hash'] for r in known})} modules)")
        for name, (n, rr, r1, r10) in sorted(stats.items()):
            print(f"  {name:12s} n={n:5d}  R@1 {r1 / n:.3f}  R@10 {r10 / n:.3f}  MRR {rr / n:.3f}")

        judged: dict[str, dict[str, int]] = defaultdict(dict)
        for r in read_jsonl(DATA / "eval-judgments.jsonl.gz"):
            judged[r["query"]][r["text_hash"]] = r["grade"]
        sets: dict[str, list[str]] = defaultdict(list)
        for r in read_jsonl(DATA / "eval-open.jsonl.gz"):
            if r["query"] in judged:
                sets["a, b (realistic)" if r["set"] in "ab" else "c, d, e (personas)"].append(r["query"])
        print("open queries")
        for name, queries in sorted(sets.items()):
            ndcg, p10, good, holes = [], [], [], 0
            for q, hits in zip(queries, search(queries)):
                grades = []
                for h in hits:
                    g = judged[q].get(h)
                    holes += g is None
                    grades.append(g or 0)
                dcg = sum((2 ** g - 1) / np.log2(i + 2) for i, g in enumerate(grades))
                ideal = sum((2 ** g - 1) / np.log2(i + 2) for i, g in enumerate(sorted(judged[q].values(), reverse=True)[:10])) or 1
                ndcg.append(dcg / ideal)
                p10.append(sum(g >= 1 for g in grades) / 10)
                good.append(sum(g == 2 for g in grades))
            print(f"  {name:20s} n={len(queries):3d}  P@10 {np.mean(p10):.3f}  good@10 {np.mean(good):.2f}  "
                  f"nDCG@10 {np.mean(ndcg):.3f}  holes {holes / (10 * len(queries)):.3f}")


if __name__ == "__main__":
    main()
