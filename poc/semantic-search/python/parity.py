"""Does the Rust runtime compute what the model in the file says?

Reads the packed file in Python, puts its dequantised weights into the PyTorch BERT, builds the
Hugging Face tokenizer over its pieces, and compares both with the `embed` binary of the crate `semantic/` on
German and English questions and on a list of awkward inputs: the ids must be the same, the
embeddings the same up to float rounding.

    python parity.py ../model/e5-de-en.bin --embed ../../../target/release/embed
"""

from __future__ import annotations

import argparse
import json
import struct
import subprocess
from pathlib import Path

import numpy as np

from common import HIDDEN, LAYERS, full_tokenizer, model_dir

AWKWARD = [
    "query: Einführung in die Programmierung",
    "query: maschinelles lernen",
    "query: Thermodynamik und Wärmeübertragung für Maschinenbauer",
    "query: renewable energy systems",
    "query: Straße, Fußgänger & Ökologie",
    "query: ÄÖÜ äöü ß ẞ",
    "query: e = mc² … ﬁnal ½ µm",
    "query:   lots   of\tspace and​zero width  ",
    "query: Software-Entwicklung (Grundlagen)",
    "query: „Anführungszeichen“ und ‚halbe‘ – Gedankenstrich — lang",
    "query: 中文 und Emoji 😀🎓 im Text",
    "query: C++/C#, Node.js; SQL-Datenbanken: 100 % sicher?",
    "query: Prof. Dr.-Ing. Müller-Lüdenscheidt",
    "query: α-β-Pruning, λ-Kalkül, ∑ und ∫",
    "passage: Die Studierenden lernen die Grundlagen der objektorientierten Programmierung in Java.",
    "query: x",
    "query:",
]


def read_packed(path: Path):
    data = path.read_bytes()
    at = 0

    def take(n):
        nonlocal at
        part = data[at:at + n]
        assert len(part) == n, "the file ends early"
        at += n
        return part

    assert take(4) == b"E5Q1"
    hidden, layers, heads, intermediate, positions, vocab = struct.unpack("<6I", take(24))
    pieces = []
    for _ in range(vocab):
        n = take(1)[0]
        piece = take(n).decode()
        pieces.append((piece, struct.unpack("<f", take(4))[0]))
    (count,) = struct.unpack("<I", take(4))
    for _ in range(count):
        take(4)
        take(take(1)[0])

    def tensor():
        kind, rows, cols = struct.unpack("<BII", take(9))
        n = rows * cols
        if kind == 0:
            return np.frombuffer(take(4 * n), "<f4").reshape(rows, cols).copy()
        if kind == 3:
            return np.frombuffer(take(2 * n), "<f2").astype(np.float32).reshape(rows, cols)
        (block,) = struct.unpack("<I", take(4))
        if kind == 1:
            levels = np.frombuffer(take(64), "<f4")
            scales = np.frombuffer(take(2 * (n // block)), "<f2").astype(np.float32)
            packed = np.frombuffer(take(n // 2), np.uint8)
            codes = np.empty(n, np.uint8)
            codes[0::2], codes[1::2] = packed & 15, packed >> 4
            return (levels[codes].reshape(-1, block) * scales[:, None]).reshape(rows, cols)
        scales = np.frombuffer(take(2 * (n // block)), "<f2").astype(np.float32)
        codes = np.frombuffer(take(n), np.int8).astype(np.float32)
        return (codes.reshape(-1, block) * scales[:, None]).reshape(rows, cols)

    weights = {"embeddings.word_embeddings.weight": tensor(), "positions": tensor(),
               "embeddings.LayerNorm.weight": tensor()[0], "embeddings.LayerNorm.bias": tensor()[0]}
    for layer in range(layers):
        p = f"encoder.layer.{layer}."
        for name in ["attention.self.query", "attention.self.key", "attention.self.value", "attention.output.dense"]:
            weights[p + name + ".weight"] = tensor()
            weights[p + name + ".bias"] = tensor()[0]
        weights[p + "attention.output.LayerNorm.weight"] = tensor()[0]
        weights[p + "attention.output.LayerNorm.bias"] = tensor()[0]
        for name in ["intermediate.dense", "output.dense"]:
            weights[p + name + ".weight"] = tensor()
            weights[p + name + ".bias"] = tensor()[0]
        weights[p + "output.LayerNorm.weight"] = tensor()[0]
        weights[p + "output.LayerNorm.bias"] = tensor()[0]
    assert at == len(data), f"{len(data) - at} bytes left over"
    return pieces, weights, positions


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("packed")
    ap.add_argument("--model")
    ap.add_argument("--embed", default=str(Path(__file__).resolve().parents[3] / "target/release/embed"))
    ap.add_argument("--questions", type=int, default=300, help="XQuAD questions per language")
    args = ap.parse_args()

    import pandas as pd
    import torch
    from huggingface_hub import hf_hub_download
    from tokenizers import Tokenizer
    from transformers import AutoModel

    mdir = model_dir(args.model)
    pieces, weights, positions = read_packed(Path(args.packed))

    spec = json.loads((mdir / "tokenizer.json").read_text())
    spec["model"]["vocab"] = [[p, s] for p, s in pieces]
    spec["added_tokens"] = [t for t in spec["added_tokens"] if t["id"] < 4]
    tok = Tokenizer.from_str(json.dumps(spec))

    torch.set_grad_enabled(False)
    model = AutoModel.from_pretrained(str(mdir))
    model.eval()
    model.embeddings.word_embeddings = torch.nn.Embedding(len(pieces), HIDDEN)
    state = model.state_dict()
    for name, value in weights.items():
        if name == "positions":
            state["embeddings.position_embeddings.weight"][:positions] = torch.from_numpy(value)
            state["embeddings.token_type_embeddings.weight"][:] = 0
        else:
            state[name] = torch.from_numpy(value)
    model.load_state_dict(state)

    texts = list(AWKWARD)
    for lang in ["de", "en"]:
        path = hf_hub_download("google/xquad", f"xquad.{lang}/validation-00000-of-00001.parquet", repo_type="dataset")
        texts += ["query: " + q for q in pd.read_parquet(path).question[:args.questions]]
    texts = [t.replace("\n", " ") for t in texts]

    run = subprocess.run([args.embed, args.packed], input="\n".join(texts) + "\n", capture_output=True, text=True, check=True)
    rust = [json.loads(line) for line in run.stdout.splitlines()]
    assert len(rust) == len(texts)

    id_mismatch, cosines, worst = 0, [], (1.0, "")
    for text, r in zip(texts, rust):
        # The runtime trims and collapses white space; so does the text handed to `tokenizers` here.
        ids = tok.encode(" ".join(tok.normalizer.normalize_str(text).split())).ids
        if ids != r["ids"]:
            id_mismatch += 1
            print("ids differ:", repr(text), "\n  python", ids, "\n  rust  ", r["ids"])
            continue
        t = torch.tensor([ids])
        hidden = model(input_ids=t, attention_mask=torch.ones_like(t)).last_hidden_state[0]
        ref = torch.nn.functional.normalize(hidden.mean(0), dim=0).numpy()
        got = np.array(r["embedding"], dtype=np.float32)
        cos = float(ref @ got / np.linalg.norm(got))
        cosines.append(cos)
        worst = min(worst, (cos, text))
    print(f"{len(texts)} texts: ids differ for {id_mismatch}; embeddings: cosine min {min(cosines):.7f}, "
          f"mean {np.mean(cosines):.7f}; worst: {worst[1]!r}")


if __name__ == "__main__":
    main()
