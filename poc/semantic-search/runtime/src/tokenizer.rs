//! XLM-R's unigram tokenizer over the kept pieces, cutting a text as Hugging Face's
//! `tokenizers` does with the model's tokenizer.json restricted to the same pieces:
//!
//! 1. every character the file's table names is replaced (the normaliser: NFKC, control
//!    characters dropped, other white space made a space); runs of spaces become one, and —
//!    unlike `tokenizers`, which cuts a trailing space into a piece of its own — the text is
//!    trimmed, as a search field's text should be;
//! 2. every word gets the „▁“ that stands for the space before it;
//! 3. each word is cut into the pieces whose scores (log probabilities) add up to the most
//!    (Viterbi); a character no piece covers is `<unk>`, several in a row are one;
//! 4. `<s>` before, `</s>` after.
//!
//! What the table cannot do is compose: „e“ followed by U+0301 stays two characters. A browser
//! does that with `text.normalize("NFC")` before handing the text over.

use std::collections::HashMap;

pub const BOS: u32 = 0;
pub const EOS: u32 = 2;
pub const UNK: u32 = 3;
/// `<s>`, `<pad>`, `</s>`, `<unk>`: never matched in text.
const SPECIALS: usize = 4;
/// What a character no piece covers costs, below the least likely piece (as sentencepiece).
const UNK_PENALTY: f64 = 10.0;

pub struct Tokenizer {
    pieces: HashMap<String, (u32, f64)>,
    longest: usize,
    unk_score: f64,
    replace: HashMap<char, String>,
}

impl Tokenizer {
    /// `pieces` in the order of their ids, with their scores; `replace` the normaliser's table.
    pub fn new(pieces: Vec<(String, f32)>, replace: HashMap<char, String>) -> Self {
        let min = pieces.iter().map(|(_, s)| f64::from(*s)).fold(f64::INFINITY, f64::min);
        let longest = pieces.iter().skip(SPECIALS).map(|(p, _)| p.len()).max().unwrap_or(1);
        let pieces = pieces
            .into_iter()
            .enumerate()
            .skip(SPECIALS)
            .filter_map(|(id, (piece, score))| Some((piece, (u32::try_from(id).ok()?, f64::from(score)))))
            .collect();
        Self { pieces, longest, unk_score: min - UNK_PENALTY, replace }
    }

    pub fn len(&self) -> usize {
        self.pieces.len() + SPECIALS
    }

    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    /// The ids the encoder reads: `<s>`, the pieces of `text`, `</s>`.
    pub fn encode(&self, text: &str) -> Vec<u32> {
        let mut clean = String::with_capacity(text.len());
        for c in text.chars() {
            match self.replace.get(&c) {
                Some(r) => clean.push_str(r),
                None => clean.push(c),
            }
        }
        let mut ids = vec![BOS];
        let mut word = String::new();
        for part in clean.split(' ').filter(|w| !w.is_empty()) {
            word.clear();
            word.push('▁');
            word.push_str(part);
            self.cut(&word, &mut ids);
        }
        ids.push(EOS);
        ids
    }

    /// The best cut of one word („▁…“), appended to `ids`. Candidates are tried from the left
    /// and, from one place, shortest first; a later one wins only with a higher score — the order
    /// in which `tokenizers` walks its trie, so a tie ends the same way.
    fn cut(&self, word: &str, ids: &mut Vec<u32>) {
        // best[end]: score, start and id of the best cut of word[..end].
        let mut best: Vec<Option<(f64, usize, u32)>> = vec![None; word.len() + 1];
        if let Some(first) = best.first_mut() {
            *first = Some((0.0, 0, BOS));
        }
        for (start, c) in word.char_indices() {
            let Some(Some((base, _, _))) = best.get(start).copied() else { continue };
            let single = start + c.len_utf8();
            let mut covered = false;
            let Some(rest) = word.get(start..) else { continue };
            for (offset, next) in rest.char_indices() {
                let end = start + offset + next.len_utf8();
                if end - start > self.longest {
                    break;
                }
                let Some(piece) = word.get(start..end) else { break };
                if let Some(&(id, score)) = self.pieces.get(piece) {
                    relax(&mut best, end, base + score, start, id);
                    covered |= end == single;
                }
            }
            if !covered {
                relax(&mut best, single, base + self.unk_score, start, UNK);
            }
        }
        let mut cut = Vec::new();
        let mut end = word.len();
        while end > 0 {
            let Some(Some((_, start, id))) = best.get(end).copied() else { break };
            if !(id == UNK && cut.last() == Some(&UNK)) {
                cut.push(id);
            }
            end = start;
        }
        ids.extend(cut.iter().rev());
    }
}

fn relax(best: &mut [Option<(f64, usize, u32)>], end: usize, score: f64, start: usize, id: u32) {
    if let Some(slot) = best.get_mut(end) {
        if slot.is_none_or(|(s, _, _)| score > s) {
            *slot = Some((score, start, id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenizer() -> Tokenizer {
        let mut pieces: Vec<(String, f32)> = ["<s>", "<pad>", "</s>", "<unk>"].iter().map(|p| (p.to_string(), 0.0)).collect();
        for (p, s) in [("▁", -3.0), ("▁Infor", -9.0), ("matik", -9.0), ("▁Informatik", -10.0), ("a", -5.0), ("▁a", -4.0), ("▁b", -4.0), ("b", -5.0)] {
            pieces.push((p.to_string(), s));
        }
        Tokenizer::new(pieces, HashMap::from([('\t', " ".to_string()), ('\u{200b}', " ".to_string())]))
    }

    #[test]
    fn the_likeliest_cut_wins() {
        let t = tokenizer();
        assert_eq!(t.encode("Informatik"), vec![BOS, 7, EOS], "one piece (−10) beats two (−18)");
        assert_eq!(t.encode("  a\tb\u{200b} "), vec![BOS, 9, 10, EOS]);
        assert_eq!(t.encode("ab"), vec![BOS, 9, 11, EOS]);
    }

    #[test]
    fn unknown_characters_are_one_unk() {
        let t = tokenizer();
        assert_eq!(t.encode("a€€b"), vec![BOS, 9, UNK, 11, EOS]);
        assert_eq!(t.encode(""), vec![BOS, EOS]);
    }
}
