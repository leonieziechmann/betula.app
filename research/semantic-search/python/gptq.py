"""GPTQ: 4-bit weights that make up for their own rounding error.

Rounding every weight to its nearest level (RTN) ignores that the errors of a row add up in its
product with an input. GPTQ (Frantar et al., 2022, arXiv:2210.17323) quantises the input
dimensions of a matrix one after the other and moves the error of each onto the dimensions not
yet quantised, weighted by how the inputs move together (the Hessian XᵀX of the layer's inputs
on calibration text). The format stays the same — the same codes and block scales, only chosen
better — so the runtime does not change. The layers are quantised in order, each calibrated on
the output of the layers before it as they are after quantisation.
"""

from __future__ import annotations

import random

import numpy as np
import torch

from common import LAYERS, QTensor, CODEBOOKS

# The six matrices of a layer and the input each of them reads.
MATRICES = [
    ("attention.self.query", "attention.self.query"),
    ("attention.self.key", "attention.self.query"),
    ("attention.self.value", "attention.self.query"),
    ("attention.output.dense", "attention.output.dense"),
    ("intermediate.dense", "intermediate.dense"),
    ("output.dense", "output.dense"),
]


def calibration_texts(per_source: int = 200, seed: int = 0) -> list[str]:
    """Query-like German and English text, none of it from the evaluation sets: STS-B sentences
    in both languages, English questions (SQuAD train), German news (newstest 2013) and short
    keyword queries of frequent words, all as E5 queries."""
    import pandas as pd
    import wordfreq
    from huggingface_hub import hf_hub_download

    rng = random.Random(seed)

    def sample(items, n):
        items = [x for x in dict.fromkeys(items) if isinstance(x, str) and x.strip()]
        return rng.sample(items, min(n, len(items)))

    texts = []
    for lang in ["de", "en"]:
        df = pd.read_parquet(hf_hub_download("PhilipMay/stsb_multi_mt", f"{lang}/train-00000-of-00001.parquet",
                                             repo_type="dataset"))
        texts += sample(list(df.sentence1), per_source)
    squad = pd.read_parquet(hf_hub_download("rajpurkar/squad", "plain_text/train-00000-of-00001.parquet",
                                            repo_type="dataset"))
    texts += sample(list(squad.question), per_source)
    news = pd.read_parquet(hf_hub_download("wmt/wmt14", "de-en/validation-00000-of-00001.parquet",
                                           repo_type="dataset"))
    texts += sample([t["de"] for t in news.translation], per_source)
    words = {lang: [w for w in wordfreq.top_n_list(lang, 30000) if w.isalpha() and len(w) > 3]
             for lang in ["de", "en"]}
    for _ in range(per_source):
        lang = rng.choice(["de", "en"])
        picked = rng.sample(words[lang], rng.randint(1, 3))
        if lang == "de" or rng.random() < 0.3:
            picked = [w.capitalize() for w in picked]
        texts.append(" ".join(picked))
    return ["query: " + t for t in texts]


def _nearest(values: torch.Tensor, levels: torch.Tensor) -> torch.Tensor:
    return torch.bucketize(values, (levels[1:] + levels[:-1]) / 2)


def _scales(group: torch.Tensor, levels: torch.Tensor, search: bool) -> torch.Tensor:
    """One scale per row of `group` (rows × block): as `common.quantize4`, per row."""
    index = group.abs().argmax(1, keepdim=True)
    extreme = group.gather(1, index).squeeze(1)
    top = torch.where(extreme < 0, levels[0], levels[-1])
    base = torch.where(top != 0, extreme / top, torch.zeros_like(extreme))
    base = torch.where(base == 0, torch.full_like(base, 1e-8), base)
    best = base.clone()
    best_err = torch.full_like(base, float("inf"))
    for f in (np.linspace(0.70, 1.10, 41) if search else [1.0]):
        s = (base * float(f)).half().float()
        s = torch.where(s.abs() < 1e-12, torch.full_like(s, 1e-8), s)
        err = ((levels[_nearest(group / s[:, None], levels)] * s[:, None] - group) ** 2).sum(1)
        better = err < best_err
        best = torch.where(better, s, best)
        best_err = torch.where(better, err, best_err)
    return best


def quantize_matrix(w: torch.Tensor, hessian: torch.Tensor, block: int, levels: torch.Tensor,
                    search: bool = True, damp: float = 0.01) -> QTensor:
    w = w.clone().float()
    rows, cols = w.shape
    h = hessian.clone().float()
    dead = torch.diag(h) == 0
    h[dead, dead] = 1
    w[:, dead] = 0
    h += damp * torch.mean(torch.diag(h)) * torch.eye(cols)
    # Upper Cholesky factor of H⁻¹: row i holds how the error of column i spreads to the rest.
    u = torch.linalg.cholesky(torch.cholesky_inverse(torch.linalg.cholesky(h)), upper=True)
    codes = torch.zeros((rows, cols), dtype=torch.uint8)
    scales = torch.zeros((rows, cols // block))
    for start in range(0, cols, block):
        s = _scales(w[:, start:start + block], levels, search)
        scales[:, start // block] = s
        for i in range(start, start + block):
            c = _nearest(w[:, i] / s, levels)
            codes[:, i] = c.to(torch.uint8)
            err = (w[:, i] - levels[c] * s) / u[i, i]
            w[:, i:] -= err[:, None] * u[i, i:][None, :]
    return QTensor((rows, cols), block, 4, codes.reshape(-1, block).numpy(),
                   scales.reshape(-1).half().numpy(), levels.numpy())


def quantize_model(model, tokenizer, texts: list[str], block: int, codebook: str, search: bool = True,
                   batch: int = 64) -> dict[str, QTensor]:
    """Quantises the 72 matrices of `model` in place (their dequantised values) and returns them."""
    levels = torch.from_numpy(CODEBOOKS[codebook])
    encs = tokenizer.encode_batch(texts)
    out = {}
    for layer in range(LAYERS):
        prefix = f"encoder.layer.{layer}."
        modules = dict(model.named_modules())
        hessians: dict[str, torch.Tensor] = {}
        mask = {}

        def hook(name):
            def capture(_module, inputs):
                x = inputs[0][mask["now"]].double()  # the tokens, not the padding
                hessians[name] = hessians.get(name, 0) + x.T @ x
            return capture

        handles = [modules[prefix + name].register_forward_pre_hook(hook(name))
                   for name in {reader for _, reader in MATRICES}]
        for start in range(0, len(encs), batch):
            part = encs[start:start + batch]
            width = max(len(e.ids) for e in part)
            ids = torch.full((len(part), width), 1, dtype=torch.long)
            att = torch.zeros((len(part), width), dtype=torch.long)
            for row, e in enumerate(part):
                ids[row, :len(e.ids)] = torch.tensor(e.ids)
                att[row, :len(e.ids)] = 1
            mask["now"] = att.bool()
            model(input_ids=ids, attention_mask=att)
        for handle in handles:
            handle.remove()
        for name, reader in MATRICES:
            param = modules[prefix + name].weight
            q = quantize_matrix(param.data, hessians[reader], block, levels, search)
            param.data = torch.from_numpy(q.dequant().astype(np.float32))
            out[prefix + name + ".weight"] = q
    return out
