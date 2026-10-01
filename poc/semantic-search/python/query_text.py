"""The text the browser's (query) model's vocabulary is built from: the training queries
(../data/train-queries.jsonl.gz) and the modules' titles of a snapshot, one per line.

    python query_text.py catalog.db ../model/query-text.txt
    python build_vocab.py --size 12000 --text ../model/query-text.txt --out ../model/vocab-query.json

build_vocab.py keeps every piece this text needs (10,697 of them) and fills up to 12,000 by word
frequency. The query model reads short queries, not module descriptions, so it does without the
pieces only descriptions need: 12,000 pieces instead of 27,625, 3.5 MB less model, and the same
search within a point (semantic/README.md, „Quality“). The server's model, which reads the
descriptions, keeps the large vocabulary.
"""

from __future__ import annotations

import argparse
import sqlite3
from pathlib import Path

from finetune import DATA, read_jsonl


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("snapshot", help="a catalog snapshot (catalog-*.db)")
    ap.add_argument("out")
    args = ap.parse_args()
    queries = [r["query"] for r in read_jsonl(DATA / "train-queries.jsonl.gz")]
    con = sqlite3.connect(f"file:{args.snapshot}?mode=ro", uri=True)
    titles = [t for row in con.execute("SELECT COALESCE(title_de, title), title_en FROM v_module") for t in row if t]
    lines = [" ".join(s.split()) for s in queries + titles]
    Path(args.out).write_text("\n".join(lines) + "\n")
    print(f"{len(queries)} queries, {len(titles)} titles → {args.out}")


if __name__ == "__main__":
    main()
