"""Fine-tune the QUERY side of e5 for the module catalog: the browser's model (pack.py packs the
result), while the modules' vectors stay those of the original model (Radix computes them with
the server's model, e5-de-en-server.bin). Every module of a snapshot is a candidate of the
softmax; the modules of the evaluation sample are left out entirely, so that evaluate_search.py
measures on modules the model never saw.

    python finetune.py catalog.db ../model/ft --model <e5-small dir>   # 2 epochs, 10 min on 4 cores
    python pack.py --model ../model/ft --vocab ../model/vocab.json --weights gptq-q4 --embeddings q4 --out ../model/e5-de-en.bin

The training queries (../data/train-queries.jsonl.gz) are what students would type to find a
module, five per module text, written by a language model (Claude) from each module's
description; `text_hash` is semantic.Text.Hash of Radix (internal/semantic). The documents are
the modules' passages without summaries (`semantic.Passage` with none): the summaries are
Radix's and not in a snapshot, and a model trained so finds the modules as well through passages
with summaries (semantic/README.md, „Quality“).

Contrastive loss over all modules (InfoNCE, temperature 0.02), every module with the query's
text a positive; plus a pull towards the original model's query embedding (weight 0.1), so that
queries unlike the training ones stay where they were. The word embeddings are frozen: the
vocabulary of the packed model is a part of them, and a query's rare words keep their meaning.
"""

from __future__ import annotations

import argparse
import gzip
import json
import random
import re
import shutil
import sqlite3
import time
from pathlib import Path

import numpy as np

from common import embed, full_tokenizer, load_torch_model, model_dir

DATA = Path(__file__).resolve().parent.parent / "data"

# Go's unicode.IsSpace, which semantic.Text.Clean splits on (strings.Fields).
_SPACE = re.compile("[\t\n\v\f\r \x85\xa0  -     　]+")


def collapse(s: str | None) -> str:
    return " ".join(w for w in _SPACE.split(s or "") if w)


def text_hash(title_de, title_en, contents, outcomes) -> str:
    """semantic.Text.Hash (internal/semantic/text.go)."""
    import hashlib
    s = "text\x1f" + "\x1f".join(collapse(x) for x in (title_de, title_en, contents, outcomes))
    return hashlib.sha256(s.encode()).hexdigest()


def passage(title_de, title_en, contents, outcomes) -> str:
    """semantic.Passage without a summary (= semantic::module_text of the crate)."""
    de, en = collapse(title_de), collapse(title_en)
    titles = en if not de else (de if not en or en == de else f"{de} / {en}")
    c, o = collapse(contents), collapse(outcomes)
    return titles + ". " + (c + " " + o if c and o else c or o)


def snapshot_modules(db: Path) -> list[dict]:
    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    rows = con.execute("SELECT id, COALESCE(title_de, title), COALESCE(title_en, ''), COALESCE(contents, ''), "
                       "COALESCE(learning_outcomes, '') FROM v_module ORDER BY id").fetchall()
    return [{"id": r[0], "hash": text_hash(*r[1:]), "passage": passage(*r[1:])} for r in rows]


def read_jsonl(path: Path) -> list[dict]:
    with gzip.open(path, "rt", encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("snapshot", help="a catalog snapshot (catalog-*.db)")
    ap.add_argument("out", help="directory for the fine-tuned model (Hugging Face layout, for pack.py --model)")
    ap.add_argument("--model", help="the original multilingual-e5-small (default: downloaded)")
    ap.add_argument("--epochs", type=int, default=2)
    ap.add_argument("--lr", type=float, default=5e-5)
    ap.add_argument("--tau", type=float, default=0.02)
    ap.add_argument("--anchor", type=float, default=0.1)
    ap.add_argument("--batch", type=int, default=64)
    ap.add_argument("--with-eval-modules", action="store_true",
                    help="train on the evaluation sample's modules too (then evaluate_search.py measures on seen modules)")
    args = ap.parse_args()

    import torch
    torch.manual_seed(0)
    random.seed(0)
    mdir = model_dir(args.model)
    modules = snapshot_modules(Path(args.snapshot))
    eval_hashes = {r["text_hash"] for r in read_jsonl(DATA / "eval-known.jsonl.gz")}
    keep = [m for m in modules if args.with_eval_modules or m["hash"] not in eval_hashes]
    print(f"{len(modules)} modules, {len(keep)} candidates")

    tok = full_tokenizer(mdir)
    original = load_torch_model(mdir)
    t = time.time()
    docs = embed(original, tok, ["passage: " + m["passage"] for m in keep])
    print(f"document vectors in {time.time() - t:.0f} s")
    rows_of: dict[str, list[int]] = {}
    for i, m in enumerate(keep):
        rows_of.setdefault(m["hash"], []).append(i)
    pairs = [(r["query"], r["text_hash"]) for r in read_jsonl(DATA / "train-queries.jsonl.gz")
             if r["text_hash"] in rows_of and (args.with_eval_modules or not r["eval"])]
    print(f"{len(pairs)} training queries")

    model = load_torch_model(mdir)
    torch.set_grad_enabled(True)  # load_torch_model turns it off
    for name, p in model.named_parameters():
        p.requires_grad = not name.startswith("embeddings.")
    model.train()
    opt = torch.optim.AdamW([p for p in model.parameters() if p.requires_grad], lr=args.lr, weight_decay=0.01)
    steps = args.epochs * ((len(pairs) + args.batch - 1) // args.batch)
    sched = torch.optim.lr_scheduler.LambdaLR(opt, lambda s: min(1, (s + 1) / (0.06 * steps)) * max(0.0, (steps - s) / steps))
    D = torch.tensor(docs)
    tok.enable_truncation(128)

    def encode(m, texts):
        encs = tok.encode_batch(["query: " + x for x in texts])
        width = max(len(e.ids) for e in encs)
        ids = torch.full((len(encs), width), 1, dtype=torch.long)
        mask = torch.zeros((len(encs), width), dtype=torch.long)
        for r, e in enumerate(encs):
            ids[r, :len(e.ids)] = torch.tensor(e.ids)
            mask[r, :len(e.ids)] = 1
        h = m(input_ids=ids, attention_mask=mask).last_hidden_state
        w = mask.unsqueeze(-1).float()
        return torch.nn.functional.normalize((h * w).sum(1) / w.sum(1), dim=-1)

    for epoch in range(args.epochs):
        random.shuffle(pairs)
        t, total = time.time(), 0.0
        for b in range(0, len(pairs), args.batch):
            batch = pairs[b:b + args.batch]
            q = encode(model, [x for x, _ in batch])
            logits = q @ D.T / args.tau
            positive = torch.zeros_like(logits, dtype=torch.bool)
            for r, (_, h) in enumerate(batch):
                positive[r, rows_of[h]] = True
            loss = (torch.logsumexp(logits, 1) - torch.logsumexp(logits.masked_fill(~positive, -1e9), 1)).mean()
            if args.anchor:
                with torch.no_grad():
                    q0 = encode(original, [x for x, _ in batch])
                loss = loss + args.anchor * (1 - (q * q0).sum(1)).mean() / args.tau
            opt.zero_grad()
            loss.backward()
            opt.step()
            sched.step()
            total += float(loss)
        print(f"epoch {epoch + 1}: loss {total / ((len(pairs) + args.batch - 1) // args.batch):.3f}, {time.time() - t:.0f} s")

    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    model.eval()
    model.save_pretrained(out)
    for f in ["tokenizer.json", "tokenizer_config.json", "special_tokens_map.json", "sentencepiece.bpe.model"]:
        if (mdir / f).exists():
            shutil.copy(mdir / f, out / f)
    print(f"saved {out}; pack it with pack.py --model {out}")


if __name__ == "__main__":
    main()
