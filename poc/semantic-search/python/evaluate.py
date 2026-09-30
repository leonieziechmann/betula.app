"""How much retrieval quality the trimmed and quantised model keeps.

The documents are embedded once with the original model (in Betula the server or the build
would do that), the queries with each variant: what a browser would compute. Tasks, all public:

- XQuAD (240 Wikipedia paragraphs, 1,190 questions, the same in German and English): question
  → paragraph, German → German, English → English, and across: English question → German
  paragraph and the other way round.
- GermanDPR (2,876 German passages, 1,025 questions).
- SciFact (5,183 English abstracts, 300 claims), nDCG@10 as in MTEB.

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
    ap.add_argument("--out")
    args = ap.parse_args()

    mdir = model_dir(args.model)
    data = Path(args.data); data.mkdir(parents=True, exist_ok=True)
    tasks = load_tasks(data)
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
