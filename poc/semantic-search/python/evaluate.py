"""How much retrieval quality the trimmed and quantised model keeps.

The documents are embedded once with the original model (in Betula the server or the build
would do that), the queries with each variant: what a browser would compute. Tasks, all public:

- XQuAD (240 Wikipedia paragraphs, 1,190 questions, the same in German and English): question
  → paragraph, German → German, English → English, and across: English question → German
  paragraph and the other way round.
- GermanDPR (2,876 German passages, 1,025 questions).
- SciFact (5,183 English abstracts, 300 claims), nDCG@10 as in MTEB.

- With `--catalog catalog.db`, Betula's own modules: a module's title (German, or English
  where it differs) → its contents and learning outcomes, among those of all modules, without
  titles. Only titles that name one module and descriptions of some length count; 1,000 each.

Besides the ranks: how close each variant's query embedding stays to the original's (cosine).

    python evaluate.py --vocab ../model/vocab.json --variants f32 q8 q4 nf4-64 --out results.json
"""

from __future__ import annotations

import argparse
import json
import time
from pathlib import Path

import numpy as np
import pandas as pd

from common import full_tokenizer, load_torch_model, model_dir, trimmed_tokenizer, variant_model, embed

HERE = Path(__file__).parent


def catalog_tasks(db: str, per_task: int = 1000) -> dict:
    import random
    import sqlite3
    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    rows = con.execute("SELECT title_de, title_en, contents, learning_outcomes FROM v_module ORDER BY id").fetchall()
    corpus = [" ".join(t for t in (c, o) if t) for _, _, c, o in rows]
    count = {}
    for de, _, _, _ in rows:
        count[de] = count.get(de, 0) + 1
    tasks = {}
    for name, pick in [("betula title de→text", lambda r: r[0]),
                       ("betula title en→text", lambda r: r[1] if r[1] and r[1] != r[0] else None)]:
        pairs = [(pick(r), i) for i, r in enumerate(rows)
                 if pick(r) and count.get(r[0]) == 1 and len(corpus[i]) >= 200]
        pairs = random.Random(0).sample(pairs, min(per_task, len(pairs)))
        tasks[name] = {"queries": [q for q, _ in pairs], "corpus": corpus, "corpus_key": "betula",
                       "relevant": [{i} for _, i in pairs]}
    return tasks


def load_tasks(data: Path) -> dict:
    from huggingface_hub import hf_hub_download

    def get(repo, name):
        return hf_hub_download(repo, name, repo_type="dataset", cache_dir=str(data))

    tasks = {}
    xq = {lang: pd.read_parquet(get("google/xquad", f"xquad.{lang}/validation-00000-of-00001.parquet"))
          for lang in ["de", "en"]}
    assert (xq["de"].id == xq["en"].id).all()
    # One paragraph per context of the German side; the English side is aligned row by row.
    contexts = list(dict.fromkeys(xq["de"].context))
    ctx_index = {c: i for i, c in enumerate(contexts)}
    target = [ctx_index[c] for c in xq["de"].context]
    en_contexts = [None] * len(contexts)
    for c_de, c_en in zip(xq["de"].context, xq["en"].context):
        en_contexts[ctx_index[c_de]] = c_en
    corpus = {"de": contexts, "en": en_contexts}
    for q_lang, d_lang in [("de", "de"), ("en", "en"), ("en", "de"), ("de", "en")]:
        tasks[f"xquad {q_lang}→{d_lang}"] = {
            "queries": list(xq[q_lang].question), "corpus": corpus[d_lang], "corpus_key": f"xquad-{d_lang}",
            "relevant": [{t} for t in target]}

    c = pd.read_parquet(get("mteb/GermanDPR", "corpus/test-00000-of-00001.parquet"))
    q = pd.read_parquet(get("mteb/GermanDPR", "queries/test-00000-of-00001.parquet"))
    r = pd.read_parquet(get("mteb/GermanDPR", "qrels/test-00000-of-00001.parquet"))
    doc_index = {d: i for i, d in enumerate(c.id)}
    rel = r[r.score > 0].groupby("query-id")["corpus-id"].apply(lambda s: {doc_index[d] for d in s}).to_dict()
    q = q[q.id.isin(rel)]
    tasks["germandpr de→de"] = {
        "queries": list(q.text), "corpus": [f"{t}. {x}" for t, x in zip(c.title, c.text)],
        "corpus_key": "germandpr", "relevant": [rel[i] for i in q.id]}

    c = pd.read_json(get("mteb/scifact", "corpus.jsonl"), lines=True)
    q = pd.read_json(get("mteb/scifact", "queries.jsonl"), lines=True)
    r = pd.read_csv(get("mteb/scifact", "qrels/test.tsv"), sep="\t")
    c["_id"] = c["_id"].astype(str); q["_id"] = q["_id"].astype(str)
    r["query-id"] = r["query-id"].astype(str); r["corpus-id"] = r["corpus-id"].astype(str)
    doc_index = {d: i for i, d in enumerate(c["_id"])}
    rel = r[r.score > 0].groupby("query-id")["corpus-id"].apply(lambda s: {doc_index[d] for d in s}).to_dict()
    q = q[q["_id"].isin(rel)]
    tasks["scifact en→en"] = {
        "queries": list(q.text), "corpus": [f"{t}. {x}" for t, x in zip(c.title, c.text)],
        "corpus_key": "scifact", "relevant": [rel[i] for i in q["_id"]]}
    return tasks


def metrics(scores: np.ndarray, relevant: list[set[int]]) -> dict:
    top = np.argsort(-scores, axis=1)[:, :10]
    mrr, r1, r10, ndcg = [], [], [], []
    for hits, rel in zip(top, relevant):
        ranks = [k for k, d in enumerate(hits) if d in rel]
        mrr.append(1 / (ranks[0] + 1) if ranks else 0)
        r1.append(1.0 if ranks and ranks[0] == 0 else 0.0)
        r10.append(len(ranks) / min(len(rel), 10))
        dcg = sum(1 / np.log2(k + 2) for k in ranks)
        idcg = sum(1 / np.log2(k + 2) for k in range(min(len(rel), 10)))
        ndcg.append(dcg / idcg)
    return {"mrr@10": float(np.mean(mrr)), "r@1": float(np.mean(r1)), "r@10": float(np.mean(r10)),
            "ndcg@10": float(np.mean(ndcg))}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--model")
    ap.add_argument("--vocab", help="vocab.json of build_vocab.py (without: the full vocabulary)")
    ap.add_argument("--variants", nargs="+", default=["f32"],
                    help="weights[/embeddings], e.g. q4 or q4/q4-64 (see common.quantize)")
    ap.add_argument("--data", default=str(HERE / ".cache"), help="datasets and cached document embeddings")
    ap.add_argument("--tasks", nargs="*", help="only these tasks (substring match)")
    ap.add_argument("--catalog", help="a Betula snapshot: adds its modules as tasks")
    ap.add_argument("--packed", help="a packed model for the variants rust:expand, rust:f32, rust:int8: the queries "
                    "embedded by the Rust runtime itself (--embed)")
    ap.add_argument("--embed", default=str(HERE.parent / "runtime/target/release/embed"))
    ap.add_argument("--out")
    args = ap.parse_args()

    mdir = model_dir(args.model)
    data = Path(args.data); data.mkdir(parents=True, exist_ok=True)
    tasks = load_tasks(data)
    if args.catalog:
        tasks.update(catalog_tasks(args.catalog))
    if args.tasks:
        tasks = {k: v for k, v in tasks.items() if any(t in k for t in args.tasks)}

    # Documents: the original model, cached.
    full = load_torch_model(mdir)
    tok = full_tokenizer(mdir)
    docs = {}
    for name, task in tasks.items():
        key = task["corpus_key"]
        if key in docs:
            continue
        path = data / f"docs-{key}.npy"
        if path.exists():
            docs[key] = np.load(path)
        else:
            t = time.time()
            docs[key] = embed(full, tok, ["passage: " + d for d in task["corpus"]])
            np.save(path, docs[key])
            print(f"embedded {len(task['corpus'])} documents of {key} in {time.time() - t:.0f} s")
    reference = {name: embed(full, tok, ["query: " + q for q in task["queries"]]) for name, task in tasks.items()}
    del full

    kept = json.loads(Path(args.vocab).read_text())["kept"] if args.vocab else None
    ttok = trimmed_tokenizer(mdir, kept) if kept else tok
    results = {}
    for variant in ["original"] + args.variants:
        if variant == "original":
            queries = reference
        elif variant.startswith("rust:"):
            import subprocess
            queries = {}
            for name, task in tasks.items():
                texts = ["query: " + " ".join(q.split()) for q in task["queries"]]
                run = subprocess.run([args.embed, args.packed, "--mode", variant[5:]], input="\n".join(texts) + "\n",
                                     capture_output=True, text=True, check=True)
                queries[name] = np.array([json.loads(line)["embedding"] for line in run.stdout.splitlines()], dtype=np.float32)
        else:
            weights, _, embeddings = variant.partition("/")
            model = variant_model(mdir, kept, weights, embeddings or weights)
            queries = {name: embed(model, ttok, ["query: " + q for q in task["queries"]]) for name, task in tasks.items()}
        row = {}
        for name, task in tasks.items():
            m = metrics(queries[name] @ docs[task["corpus_key"]].T, task["relevant"])
            cos = (queries[name] * reference[name]).sum(1)
            m["cos_mean"], m["cos_p01"] = float(cos.mean()), float(np.quantile(cos, 0.01))
            row[name] = m
        results[variant] = row
        print(f"\n== {variant}")
        for name, m in row.items():
            print(f"  {name:18s} mrr@10 {m['mrr@10']:.4f}  r@1 {m['r@1']:.4f}  r@10 {m['r@10']:.4f}  "
                  f"ndcg@10 {m['ndcg@10']:.4f}  cos {m['cos_mean']:.4f} (p01 {m['cos_p01']:.4f})")
    if args.out:
        Path(args.out).write_text(json.dumps({"vocab": args.vocab, "results": results}, indent=1))


if __name__ == "__main__":
    main()
