"""Pick the pieces of XLM-R's vocabulary that German and English text needs.

The frequent words of both languages (wordfreq's large lists: 634k German, 321k English words)
are cut into pieces the way the original tokenizer cuts them — lower case, capitalised (German
nouns, the start of a sentence), in capitals when short (acronyms), and after a hyphen or a
bracket (the pieces without the leading „▁“). Every piece used is ranked by the words that use
it, each word counting with its frequency to the power `--alpha`: 1 ranks by frequency alone
(the best coverage of running text, but a piece only rare words need never makes it), 0 by the
number of words alone (the pieces of junk from the lists' tail crowd out common ones).

A unigram tokenizer that is left only a subset of its pieces still cuts a word exactly as the
full one does if every piece of the full cut is in the subset: the best path is still there,
and no path through the pieces that are gone can beat it. A word with a missing piece is cut
into other, shorter pieces. Coverage is therefore measured as the share of words whose cut is
unchanged.

    python build_vocab.py --coverage 0.99 --out ../model/vocab.json [--text catalog.txt ...]
    python build_vocab.py --size 32000 --out ../model/vocab.json

`--coverage` keeps the fewest pieces with which that share of the running text of either
language (by word frequency) is cut exactly as before.

`--text` adds the pieces of any text (one document per line), e.g. the module texts of a
catalog exported by `embed_catalog.py --export-text`, with a weight that keeps all of them.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np
import wordfreq

from common import SPECIALS, full_tokenizer, model_dir, pieces_and_scores

# Characters German and English text (and the names of lecturers) is written in. A piece that
# is one of them, with or without the leading „▁“, is always kept: with them every word can
# still be cut, whatever else is missing.
CHARSET = (
    [chr(c) for c in range(0x20, 0x7F)]
    + [chr(c) for c in range(0xA0, 0x180)]           # Latin-1, Latin Extended-A
    + [chr(c) for c in range(0x391, 0x3CA)]          # Greek (α, β, λ, Ω …)
    + list("–—‘’‚“”„†•…‰′″‹›€™←↑→↓↔⇒⇔∀∂∃∅∆∇∈∉∑−∓√∞∠∧∨∩∪∫≈≠≡≤≥⊂⊆⊕⊗⋅")
)
PREFIXES = ["query: ", "passage: "]  # E5 reads every text behind one of them


def variants(word: str, lang: str, freq: float, rank: int):
    """The forms of a (lower-cased) word of wordfreq's list with their weight."""
    yield word, freq
    cap = word[:1].upper() + word[1:]
    if cap != word:
        yield cap, freq * (1.0 if lang == "de" else 0.3)
    if len(word) <= 5 and word.upper() not in (word, cap):
        yield word.upper(), freq * 0.05
    if rank < 60_000:  # after a hyphen or a bracket a word has no „▁“: „Software-Entwicklung“
        yield "-" + cap, freq * 0.02
        yield "(" + word, freq * 0.02


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--model", help="directory of multilingual-e5-small (default: HF cache)")
    size = ap.add_mutually_exclusive_group(required=True)
    size.add_argument("--size", type=int, help="pieces to keep, the special ones included")
    size.add_argument("--coverage", type=float, help="share of word frequency (either language) cut as before")
    ap.add_argument("--alpha", type=float, default=1.0, help="ranking: word frequency to this power")
    ap.add_argument("--text", nargs="*", default=[], help="further text files whose pieces are all kept")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    mdir = model_dir(args.model)
    tok = full_tokenizer(mdir)
    vocab = pieces_and_scores(mdir)
    piece_id = {p: i for i, (p, _) in enumerate(vocab)}

    forms, weights, langs = [], [], []
    for lang in ["de", "en"]:
        for rank, word in enumerate(wordfreq.iter_wordlist(lang, "large")):
            freq = wordfreq.word_frequency(word, lang, "large")
            if freq <= 0:
                continue
            for form, weight in variants(word, lang, freq, rank):
                forms.append(form)
                weights.append(weight)
                langs.append(lang)
    print(f"{len(forms):,} word forms")
    cuts = [e.ids for e in tok.encode_batch(forms, add_special_tokens=False)]

    score = np.zeros(len(vocab))
    for cut, weight in zip(cuts, weights):
        for i in set(cut):
            score[i] += weight ** args.alpha

    must = set(range(len(SPECIALS)))
    for c in CHARSET:
        for p in (c, "▁" + c):
            if p in piece_id:
                must.add(piece_id[p])
    for text in PREFIXES + [str(n) for n in range(0, 2101)]:
        must.update(tok.encode(text, add_special_tokens=False).ids)
    for path in args.text:
        lines = [line for line in Path(path).read_text().splitlines() if line.strip()]
        for enc in tok.encode_batch(lines, add_special_tokens=False):
            must.update(enc.ids)
        print(f"{path}: {len(lines):,} texts")
    must.discard(250001)  # <mask>

    ranked = [int(i) for i in np.argsort(-score, kind="stable") if score[i] > 0 and i not in must]
    w = np.array(weights)
    de = np.array(langs) == "de"
    if args.coverage:
        # The last ranked piece a form needs decides from which size on its cut stays.
        rank = np.full(len(vocab), -1)
        rank[ranked] = np.arange(len(ranked))
        needs = np.array([rank[c].max() if len(c) else -1 for c in cuts])
        room = 0
        for side in (de, ~de):
            order = np.argsort(needs[side], kind="stable")
            share = np.cumsum(w[side][order]) / w[side].sum()
            room = max(room, int(needs[side][order][np.searchsorted(share, args.coverage)]) + 1)
    else:
        room = args.size - len(must)
        if room < 0:
            raise SystemExit(f"{len(must)} pieces are required, more than --size {args.size}")
    kept_set = must | set(int(i) for i in ranked[:room])
    kept = list(range(len(SPECIALS))) + sorted(kept_set - set(range(len(SPECIALS))))

    # How many word forms keep their cut, by frequency and by count.
    keep = np.zeros(len(vocab), dtype=bool)
    keep[kept] = True
    same = np.array([keep[c].all() for c in cuts])
    stats = {
        "pieces": len(kept),
        "pieces_used_by_word_lists": int((score > 0).sum()),
        "required": len(must),
        "same_cut_by_frequency": {"de": float(w[same & de].sum() / w[de].sum()),
                                  "en": float(w[same & ~de].sum() / w[~de].sum())},
        "same_cut_by_count": {"de": float(same[de].mean()), "en": float(same[~de].mean())},
    }
    print(json.dumps(stats, indent=1))
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps({"kept": kept, "stats": stats}))


if __name__ == "__main__":
    main()
