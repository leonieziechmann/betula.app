"""Shared pieces of the proof of concept: the model, the trimmed tokenizer, quantisation.

multilingual-e5-small is a BERT encoder (12 layers, 384 wide, 118 M parameters) with XLM-R's
SentencePiece vocabulary of 250,002 pieces for 100 languages. 96 M of its parameters are the
embedding matrix, one row per piece; German and English use about a fifth of the pieces.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

import numpy as np

MODEL_ID = "intfloat/multilingual-e5-small"
HIDDEN = 384
LAYERS = 12
HEADS = 12
SPECIALS = ["<s>", "<pad>", "</s>", "<unk>"]  # ids 0..3 in XLM-R and in the trimmed vocabulary
MODEL_FILES = ["config.json", "model.safetensors", "tokenizer.json", "tokenizer_config.json",
               "special_tokens_map.json", "sentencepiece.bpe.model"]


def model_dir(path: str | None = None) -> Path:
    """The model's files: `path` if given, otherwise downloaded once into the HF cache."""
    if path:
        return Path(path)
    from huggingface_hub import snapshot_download
    return Path(snapshot_download(MODEL_ID, allow_patterns=MODEL_FILES))


# ---------------------------------------------------------------------------------------------
# Tokenizers


def full_tokenizer(mdir: Path):
    from tokenizers import Tokenizer
    return Tokenizer.from_file(str(mdir / "tokenizer.json"))


def trimmed_tokenizer(mdir: Path, kept: list[int]):
    """The original tokenizer (normaliser, pre-tokeniser, template) over the kept pieces only.

    `kept` lists original ids; the new id of a piece is its place in `kept`, so `kept` starts
    with the four special tokens and the template's `<s>` / `</s>` stay 0 and 2.
    """
    from tokenizers import Tokenizer
    spec = json.loads((mdir / "tokenizer.json").read_text())
    vocab = spec["model"]["vocab"]
    assert kept[:4] == [0, 1, 2, 3], "the special tokens come first"
    spec["model"]["vocab"] = [vocab[i] for i in kept]
    spec["model"]["unk_id"] = 3
    # `<mask>` is not kept; the template refers to <s> and </s> by id.
    spec["added_tokens"] = [t for t in spec["added_tokens"] if t["content"] in SPECIALS]
    return Tokenizer.from_str(json.dumps(spec))


def pieces_and_scores(mdir: Path) -> list[tuple[str, float]]:
    spec = json.loads((mdir / "tokenizer.json").read_text())
    return [(p, float(s)) for p, s in spec["model"]["vocab"]]


# ---------------------------------------------------------------------------------------------
# Quantisation. Every weight matrix is cut into blocks of `block` consecutive values of a row
# (its input dimension); a block keeps one scale and a 4-bit code per value, which picks one of
# 16 levels of a codebook shared by the whole tensor. The runtime needs nothing but a table
# lookup and a multiplication, whatever the levels are.

# Uniform levels (-8 … 7), as llama.cpp's q4_0.
UNIFORM4 = np.arange(-8, 8, dtype=np.float32) / 8.0
# NormalFloat4 (QLoRA): the quantiles of a normal distribution, so that every level is used
# about equally often by normally distributed weights.
NF4 = np.array([-1.0, -0.6961928009986877, -0.5250730514526367, -0.39491748809814453,
                -0.28444138169288635, -0.18477343022823334, -0.09105003625154495, 0.0,
                0.07958029955625534, 0.16093020141124725, 0.24611230194568634, 0.33791524171829224,
                0.44070982933044434, 0.5626170039176941, 0.7229568362236023, 1.0], dtype=np.float32)
CODEBOOKS = {"uniform": UNIFORM4, "nf4": NF4}


@dataclass
class QTensor:
    shape: tuple[int, ...]
    block: int
    bits: int                 # 4 or 8
    codes: np.ndarray         # uint8 (blocks, block): a level (4 bit) or int8 as uint8 (8 bit)
    scales: np.ndarray        # float16 (blocks,)
    levels: np.ndarray | None  # float32 (16,) for 4 bit

    def dequant(self) -> np.ndarray:
        scale = self.scales.astype(np.float32)[:, None]
        if self.bits == 4:
            values = self.levels[self.codes] * scale
        else:
            values = self.codes.view(np.int8).astype(np.float32) * scale
        return values.reshape(self.shape)

    def nbytes(self) -> int:
        """Bytes in the packed model file: codes (two per byte at 4 bit) and fp16 scales."""
        return self.codes.size * self.bits // 8 + self.scales.size * 2 + (64 if self.bits == 4 else 0)


def _nearest(values: np.ndarray, levels: np.ndarray) -> np.ndarray:
    """Index of the nearest level (levels sorted ascending)."""
    bounds = (levels[1:] + levels[:-1]) / 2
    return np.searchsorted(bounds, values).astype(np.uint8)


def quantize4(w: np.ndarray, block: int = 32, codebook: str = "uniform", search: bool = True) -> QTensor:
    """4-bit codes over a codebook, one fp16 scale per block.

    Without `search` the scale maps the largest magnitude of the block onto the codebook's
    largest magnitude (the sign picks the side, as q4_0 does). With `search` a range of smaller
    and larger scales is tried per block and the one with the least squared error kept: cutting
    off a rare outlier often buys every other value of the block a finer step.
    """
    levels = CODEBOOKS[codebook]
    blocks = w.astype(np.float32).reshape(-1, block)
    index = np.abs(blocks).argmax(1)
    extreme = blocks[np.arange(len(blocks)), index]  # signed value of largest magnitude
    # Map it onto the level of the same sign with the largest magnitude.
    top = np.where(extreme < 0, levels[0], levels[-1])
    base = np.where(top != 0, extreme / top, 0.0)
    base = np.where(base == 0, 1e-8, base)
    factors = np.linspace(0.70, 1.10, 41) if search else np.array([1.0])
    best_err = np.full(len(blocks), np.inf, dtype=np.float64)
    best_scale = base.copy()
    best_codes = np.zeros(blocks.shape, dtype=np.uint8)
    for f in factors:
        scale = (base * f).astype(np.float16).astype(np.float32)
        scale = np.where(np.abs(scale) < 1e-12, 1e-8, scale)
        codes = _nearest(blocks / scale[:, None], levels)
        err = ((levels[codes] * scale[:, None] - blocks) ** 2).sum(1)
        better = err < best_err
        best_err = np.where(better, err, best_err)
        best_scale = np.where(better, scale, best_scale)
        best_codes[better] = codes[better]
    return QTensor(w.shape, block, 4, best_codes, best_scale.astype(np.float16), levels)


def quantize8(w: np.ndarray, block: int = 32) -> QTensor:
    blocks = w.astype(np.float32).reshape(-1, block)
    scale = (np.abs(blocks).max(1) / 127.0).astype(np.float16).astype(np.float32)
    scale = np.where(scale == 0, 1e-8, scale)
    codes = np.clip(np.round(blocks / scale[:, None]), -127, 127).astype(np.int8).view(np.uint8)
    return QTensor(w.shape, block, 8, codes, scale.astype(np.float16), None)


def quantize(w: np.ndarray, spec: str) -> QTensor | np.ndarray:
    """`spec`: "f32", "q8", "q4" / "q4-64" / "nf4-64" / "q4-32-rtn" (no scale search) …"""
    if spec == "f32":
        return w.astype(np.float32)
    parts = spec.split("-")
    kind, block = parts[0], int(parts[1]) if len(parts) > 1 else 32
    search = "rtn" not in parts
    if kind == "q8":
        return quantize8(w, block)
    if kind == "q4":
        return quantize4(w, block, "uniform", search)
    if kind == "nf4":
        return quantize4(w, block, "nf4", search)
    raise ValueError(spec)


def dequant(t) -> np.ndarray:
    return t.dequant() if isinstance(t, QTensor) else t


# ---------------------------------------------------------------------------------------------
# The model in PyTorch, with the trimmed and quantised weights put back in (fake quantisation):
# what the runtime computes, up to float rounding.


def linear_names() -> list[str]:
    names = []
    for layer in range(LAYERS):
        p = f"encoder.layer.{layer}."
        names += [p + "attention.self.query.weight", p + "attention.self.key.weight",
                  p + "attention.self.value.weight", p + "attention.output.dense.weight",
                  p + "intermediate.dense.weight", p + "output.dense.weight"]
    return names


def load_torch_model(mdir: Path):
    import torch
    from transformers import AutoModel
    torch.set_grad_enabled(False)
    model = AutoModel.from_pretrained(str(mdir), torch_dtype=torch.float32)
    model.eval()
    return model


def build_variant(mdir: Path, kept: list[int] | None, weights: str, embeddings: str):
    """The model with the embedding rows of `kept` only, quantised as `weights` (the 72
    matrices of the layers) and `embeddings` (the word embeddings) say: a spec of `quantize`,
    for the matrices also `gptq-<spec>` (`gptq.py`, calibrated on `gptq.calibration_texts`).

    Returns the model with the dequantised values put in (what the runtime computes, up to float
    rounding) and every quantised tensor by name (what `pack.py` writes)."""
    import torch
    model = load_torch_model(mdir)
    tensors = {}
    name = "embeddings.word_embeddings.weight"
    emb = model.state_dict()[name].numpy()
    if kept is not None:
        emb = emb[kept]
        model.embeddings.word_embeddings = torch.nn.Embedding(len(kept), HIDDEN, padding_idx=1)
        model.config.vocab_size = len(kept)
    tensors[name] = quantize(emb, embeddings)
    model.embeddings.word_embeddings.weight.data = torch.from_numpy(dequant(tensors[name]).copy())
    if weights.startswith("gptq-"):
        import gptq
        spec = weights[len("gptq-"):].split("-")
        codebook = {"q4": "uniform", "nf4": "nf4"}[spec[0]]
        block = int(spec[1]) if len(spec) > 1 else 32
        tokenizer = trimmed_tokenizer(mdir, kept) if kept is not None else full_tokenizer(mdir)
        tensors.update(gptq.quantize_model(model, tokenizer, gptq.calibration_texts(), block, codebook))
    else:
        params = dict(model.named_parameters())
        for name in linear_names():
            tensors[name] = quantize(params[name].data.numpy(), weights)
            params[name].data = torch.from_numpy(dequant(tensors[name]).copy())
    return model, tensors


def variant_model(mdir: Path, kept: list[int] | None, weights: str, embeddings: str):
    return build_variant(mdir, kept, weights, embeddings)[0]


def embed(model, tokenizer, texts: list[str], batch: int = 32, max_len: int = 512) -> np.ndarray:
    """Mean-pooled, L2-normalised embeddings (the prefixes „query: “ / „passage: “ are the
    caller's). Sorted by length so a batch pads little."""
    import torch
    tokenizer.enable_truncation(max_len)
    encs = tokenizer.encode_batch(texts)
    order = np.argsort([len(e.ids) for e in encs])
    out = np.zeros((len(texts), HIDDEN), dtype=np.float32)
    for start in range(0, len(texts), batch):
        idx = order[start:start + batch]
        width = max(len(encs[i].ids) for i in idx)
        ids = torch.full((len(idx), width), 1, dtype=torch.long)  # <pad>
        mask = torch.zeros((len(idx), width), dtype=torch.long)
        for row, i in enumerate(idx):
            n = len(encs[i].ids)
            ids[row, :n] = torch.tensor(encs[i].ids)
            mask[row, :n] = 1
        hidden = model(input_ids=ids, attention_mask=mask).last_hidden_state
        m = mask.unsqueeze(-1).float()
        pooled = (hidden * m).sum(1) / m.sum(1)
        pooled = torch.nn.functional.normalize(pooled, dim=-1)
        out[idx] = pooled.numpy()
    tokenizer.no_truncation()
    return out
