"""Write the trimmed, quantised model into one file the runtime (`../runtime`) reads.

    python pack.py --vocab ../model/vocab.json --weights gptq-q4 --embeddings q4 --out ../model/e5-de-en.bin

Layout (little endian; every tensor with a header, so the runtime needs no table of offsets):

    b"E5Q1"
    u32 hidden, layers, heads, intermediate, positions, vocab
    tokenizer: vocab × (u8 length, UTF-8 piece, f32 score)
    normaliser: u32 count, count × (u32 character, u8 length, UTF-8 replacement): what the
                tokenizer's normaliser (NFKC, control characters dropped, other white space made a
                space) does to a single character of the scripts German and English text uses
    tensor word_embeddings (vocab × hidden)
    tensor positions (positions × hidden): position embedding + token type 0, added up here
    tensor, tensor: embedding LayerNorm weight, bias
    per layer: query, key, value, attention output, intermediate, output — each a matrix and
               its bias —, and the two LayerNorms after attention and after the feed-forward part:
      Wq bq Wk bk Wv bv Wo bo ln1.w ln1.b Wi bi Wout bout ln2.w ln2.b

    tensor: u8 kind, u32 rows, u32 cols, then by kind
      0 f32:  rows·cols f32
      1 q4:   u32 block, 16 × f32 levels, (rows·cols/block) × f16 scales,
              rows·cols/2 bytes of codes (two per byte, the first in the low nibble)
      2 q8:   u32 block, (rows·cols/block) × f16 scales, rows·cols × i8
      3 f16:  rows·cols f16

Only the first `--positions` positions are kept: a query is short (128 by default).
"""

from __future__ import annotations

import argparse
import json
import struct
from pathlib import Path

import numpy as np

from common import HEADS, HIDDEN, LAYERS, QTensor, build_variant, full_tokenizer, model_dir, pieces_and_scores, quantize

# Latin, Greek, Cyrillic, phonetic and Latin extended, punctuation, super- and subscripts,
# letter-like symbols, number forms, arrows, mathematical operators, …, the ideographic space,
# ligatures, variation selectors, compatibility forms, the byte order mark, full-width forms.
NORMALISED = [(0x0000, 0x0530), (0x1D00, 0x1F00), (0x2000, 0x2C00), (0x3000, 0x3001), (0xFB00, 0xFB50),
              (0xFE00, 0xFE10), (0xFE30, 0xFE70), (0xFEFF, 0xFF00), (0xFF00, 0xFFF0)]


def char_map(mdir) -> list[tuple[int, str]]:
    """Every character of `NORMALISED` the tokenizer's normaliser changes, with what it makes of
    it. A browser composes what is left („e“ + U+0301 → „é“) with String.normalize("NFC")."""
    normalizer = full_tokenizer(mdir).normalizer
    out = []
    for start, end in NORMALISED:
        for code in range(start, end):
            if 0xD800 <= code < 0xE000:
                continue
            replacement = normalizer.normalize_str(chr(code))
            if replacement != chr(code):
                out.append((code, replacement))
    return out


def tensor_bytes(t) -> bytes:
    if isinstance(t, QTensor):
        rows, cols = t.shape if len(t.shape) == 2 else (1, t.shape[0])
        if t.bits == 4:
            codes = t.codes.reshape(-1)
            packed = (codes[0::2] | (codes[1::2] << 4)).astype(np.uint8)
            return (struct.pack("<BIII", 1, rows, cols, t.block) + t.levels.astype("<f4").tobytes()
                    + t.scales.astype("<f2").tobytes() + packed.tobytes())
        return (struct.pack("<BIII", 2, rows, cols, t.block) + t.scales.astype("<f2").tobytes()
                + t.codes.tobytes())
    a = np.asarray(t)
    rows, cols = a.shape if a.ndim == 2 else (1, a.shape[0])
    if a.dtype == np.float16:
        return struct.pack("<BII", 3, rows, cols) + a.astype("<f2").tobytes()
    return struct.pack("<BII", 0, rows, cols) + a.astype("<f4").tobytes()


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--model")
    ap.add_argument("--vocab", required=True)
    ap.add_argument("--weights", default="gptq-q4", help="spec of the 72 matrices (common.quantize, gptq-…)")
    ap.add_argument("--embeddings", default="q4", help="spec of the word embeddings")
    ap.add_argument("--positions", type=int, default=128)
    ap.add_argument("--position-spec", default="q8", help="spec of the position embeddings")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    mdir = model_dir(args.model)
    kept = json.loads(Path(args.vocab).read_text())["kept"]
    model, tensors = build_variant(mdir, kept, args.weights, args.embeddings)
    sd = {k: v.numpy() for k, v in model.state_dict().items()}
    config = model.config

    out = bytearray(b"E5Q1")
    out += struct.pack("<6I", HIDDEN, LAYERS, HEADS, config.intermediate_size, args.positions, len(kept))
    vocab = pieces_and_scores(mdir)
    for i in kept:
        piece, score = vocab[i]
        raw = piece.encode()
        assert len(raw) < 256
        out += struct.pack("<B", len(raw)) + raw + struct.pack("<f", score)
    mapping = char_map(mdir)
    out += struct.pack("<I", len(mapping))
    for code, replacement in mapping:
        raw = replacement.encode()
        out += struct.pack("<IB", code, len(raw)) + raw
    sizes = {"tokenizer": len(out)}

    def put(key, t):
        data = tensor_bytes(t)
        sizes[key] = sizes.get(key, 0) + len(data)
        out.extend(data)

    put("word_embeddings", tensors["embeddings.word_embeddings.weight"])
    positions = sd["embeddings.position_embeddings.weight"][:args.positions] + sd["embeddings.token_type_embeddings.weight"][0]
    put("positions", quantize(positions, args.position_spec) if args.position_spec != "f16" else positions.astype(np.float16))
    put("norms+biases", sd["embeddings.LayerNorm.weight"])
    put("norms+biases", sd["embeddings.LayerNorm.bias"])
    for layer in range(LAYERS):
        p = f"encoder.layer.{layer}."
        for name in ["attention.self.query", "attention.self.key", "attention.self.value", "attention.output.dense"]:
            put("layers", tensors[p + name + ".weight"])
            put("norms+biases", sd[p + name + ".bias"])
        put("norms+biases", sd[p + "attention.output.LayerNorm.weight"])
        put("norms+biases", sd[p + "attention.output.LayerNorm.bias"])
        for name in ["intermediate.dense", "output.dense"]:
            put("layers", tensors[p + name + ".weight"])
            put("norms+biases", sd[p + name + ".bias"])
        put("norms+biases", sd[p + "output.LayerNorm.weight"])
        put("norms+biases", sd[p + "output.LayerNorm.bias"])

    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_bytes(bytes(out))
    import brotli
    compressed = len(brotli.compress(bytes(out), quality=11))
    report = {"file": args.out, "bytes": len(out), "brotli_bytes": compressed, "parts": sizes,
              "vocab": len(kept), "weights": args.weights, "embeddings": args.embeddings}
    print(json.dumps(report, indent=1))
    Path(args.out + ".json").write_text(json.dumps(report, indent=1))


if __name__ == "__main__":
    main()
