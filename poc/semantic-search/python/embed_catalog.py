"""Embed the modules of a Betula snapshot with the original model: the index a browser searches.

    python embed_catalog.py catalog.db --out ../model/          # index.bin + index.json
    python embed_catalog.py catalog.db --export-text catalog.txt # the texts, for build_vocab --text

Each module is one passage: its title (German and, where it differs, English), its contents and
its learning outcomes (`v_module`, docs/schema-v2.md), cut off at 512 tokens. The index holds one
int8 row per module with its scale (`demo/e5.js` reads it): 384 bytes a module.
"""

from __future__ import annotations

import argparse
import json
import sqlite3
import struct
from pathlib import Path

import numpy as np

from common import embed, full_tokenizer, load_torch_model, model_dir


def module_texts(db: Path) -> list[tuple[str, str, str]]:
    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    rows = con.execute("SELECT id, title, title_de, title_en, contents, learning_outcomes FROM v_module ORDER BY id")
    out = []
    for mid, title, de, en, contents, outcomes in rows:
        titles = [t for t in dict.fromkeys([de or title, en]) if t]
        body = " ".join(t for t in [contents, outcomes] if t)
        out.append((str(mid), title or de or en or "", " / ".join(titles) + ". " + body))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("db")
    ap.add_argument("--model")
    ap.add_argument("--out", help="directory for index.bin and index.json")
    ap.add_argument("--export-text", help="write one text per line instead")
    args = ap.parse_args()

    modules = module_texts(Path(args.db))
    if args.export_text:
        Path(args.export_text).write_text("\n".join(" ".join(t.split()) for _, _, t in modules))
        print(f"{len(modules)} texts → {args.export_text}")
        return
    mdir = model_dir(args.model)
    vectors = embed(load_torch_model(mdir), full_tokenizer(mdir), ["passage: " + t for _, _, t in modules])
    scales = np.abs(vectors).max(1) / 127
    codes = np.round(vectors / scales[:, None]).astype(np.int8)
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    (out / "index.bin").write_bytes(b"E5I1" + struct.pack("<II", *codes.shape)
                                    + scales.astype("<f4").tobytes() + codes.tobytes())
    (out / "index.json").write_text(json.dumps([{"id": m, "title": t} for m, t, _ in modules], ensure_ascii=False))
    print(f"{len(modules)} modules → {out}/index.bin ({(out / 'index.bin').stat().st_size / 1e6:.1f} MB)")


if __name__ == "__main__":
    main()
