//! Ranking for the pickers (program, lecturer, department): forgiving, but predictable.
//!
//! A query is split into words and every word has to match the target; how well it matches
//! decides the rank. From best to worst a query word may be: a whole word of the target, the
//! start of one, the initials of consecutive words („ti" finds „Theoretische Informatik"), a
//! part of a word, the start of a word with a typo („infomatik", „informaitk"), or letters
//! that occur in a word in this order. Text is folded first (`search::fold`), so case,
//! umlauts and accents do not matter, and an abbreviation with dots also counts as one word
//! („bsc" finds „B.Sc."). Among equal matches the shorter target wins.

use crate::search::{fold, FILLERS};

const WHOLE_WORD: i64 = 1000;
const WORD_START: i64 = 800;
const INITIALS: i64 = 600;
const WORD_PART: i64 = 500;
const TYPO: i64 = 380;
const LETTERS_IN_ORDER: i64 = 200;
/// Every word further back in the target costs this much, up to `MAX_POSITION` words.
const PER_POSITION: i64 = 8;
const MAX_POSITION: i64 = 10;

/// A query, prepared once and then scored against many targets.
pub struct Matcher {
    words: Vec<Vec<char>>,
}

fn words_of(text: &str) -> Vec<Vec<char>> {
    fold(text).split(|c: char| !c.is_alphanumeric()).filter(|word| !word.is_empty()).map(|word| word.chars().collect()).collect()
}

/// A word of the target. What is written without a space but with punctuation („B.Sc.",
/// „Bau-Ing.") is there in parts and once more as a whole, which has no initial of its own.
struct TargetWord {
    letters: Vec<char>,
    position: usize,
    whole: bool,
}

fn target_words(text: &str) -> Vec<TargetWord> {
    let mut words = Vec::new();
    let mut position = 0;
    for chunk in fold(text).split_whitespace() {
        let parts: Vec<Vec<char>> = chunk.split(|c: char| !c.is_alphanumeric()).filter(|part| !part.is_empty()).map(|part| part.chars().collect()).collect();
        let first = position;
        let whole: Vec<char> = parts.iter().flatten().copied().collect();
        let several = parts.len() > 1;
        for letters in parts {
            words.push(TargetWord { letters, position, whole: false });
            position += 1;
        }
        if several {
            words.push(TargetWord { letters: whole, position: first, whole: true });
        }
    }
    words
}

impl Matcher {
    pub fn new(query: &str) -> Self {
        Self { words: words_of(query) }
    }

    /// An empty query matches everything (with the same score), so lists keep their order.
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// `None`: no match. Otherwise the higher the better.
    pub fn score(&self, target: &str) -> Option<i64> {
        let target_words = target_words(target);
        if target_words.is_empty() {
            return None;
        }
        let mut total = 0;
        for word in &self.words {
            total += word_score(word, &target_words)?;
        }
        let length = target.chars().count().min(999) as i64;
        Some(total * 1000 - length)
    }
}

/// The best way `query` matches one of the words of the target.
fn word_score(query: &[char], target: &[TargetWord]) -> Option<i64> {
    let mut best: Option<i64> = None;
    let mut consider = |score: i64, position: usize| {
        let score = score - (position as i64).min(MAX_POSITION) * PER_POSITION;
        if best.is_none_or(|b| score > b) {
            best = Some(score);
        }
    };

    for TargetWord { letters: word, position, .. } in target {
        let position = *position;
        if word.as_slice() == query {
            consider(WHOLE_WORD, position);
        } else if word.starts_with(query) {
            consider(WORD_START, position);
        } else if contains(word, query) {
            consider(WORD_PART, position);
        } else if let Some(typos) = typos_at_start(query, word) {
            consider(TYPO - (typos as i64 - 1) * 120, position);
        } else if query.len() >= 3 && word.first() == query.first() && in_order(query, word) {
            consider(LETTERS_IN_ORDER, position);
        }
    }

    if query.len() >= 2 {
        let initials: Vec<(usize, char)> = target
            .iter()
            .filter(|word| !word.whole && !FILLERS.contains(&word.letters.iter().collect::<String>().as_str()))
            .filter_map(|word| word.letters.first().map(|c| (word.position, *c)))
            .collect();
        for start in 0..initials.len() {
            let run = initials.iter().skip(start).take(query.len());
            if run.len() == query.len() && run.map(|(_, c)| c).eq(query.iter()) {
                if let Some((position, _)) = initials.get(start) {
                    consider(INITIALS, *position);
                }
                break;
            }
        }
    }
    best
}

fn contains(word: &[char], part: &[char]) -> bool {
    !part.is_empty() && word.windows(part.len()).any(|window| window == part)
}

fn in_order(query: &[char], word: &[char]) -> bool {
    let mut rest = word.iter();
    query.iter().all(|c| rest.any(|w| w == c))
}

/// How many typos turn `query` into the start of `word`: one for four letters and more, two
/// for eight and more. The first letter has to be right, which keeps nonsense out. The catalog's
/// search corrects a word with it too (`search::resolve`).
pub(crate) fn typos_at_start(query: &[char], word: &[char]) -> Option<usize> {
    if query.len() < 4 || query.first() != word.first() {
        return None;
    }
    let allowed = if query.len() >= 8 { 2 } else { 1 };
    let lengths = [query.len().saturating_sub(1), query.len(), query.len() + 1];
    lengths
        .iter()
        .filter(|length| **length <= word.len())
        .filter_map(|length| word.get(..*length))
        .map(|start| edit_distance(query, start))
        .min()
        .filter(|typos| (1..=allowed).contains(typos))
}

/// Edits that turn `a` into `b`: insert, delete, replace, or swap two neighbours.
fn edit_distance(a: &[char], b: &[char]) -> usize {
    let width = b.len() + 1;
    let mut before: Vec<usize> = vec![0; width];
    let mut previous: Vec<usize> = (0..width).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut current = vec![i + 1; width];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            let up = previous.get(j + 1).copied().unwrap_or(usize::MAX - 1) + 1;
            let left = current.get(j).copied().unwrap_or(usize::MAX - 1) + 1;
            let diagonal = previous.get(j).copied().unwrap_or(usize::MAX - 1) + cost;
            let mut value = up.min(left).min(diagonal);
            let swapped = i > 0 && j > 0 && a.get(i - 1) == Some(cb) && b.get(j - 1) == Some(ca);
            if swapped {
                value = value.min(before.get(j - 1).copied().unwrap_or(usize::MAX - 1) + 1);
            }
            if let Some(cell) = current.get_mut(j + 1) {
                *cell = value;
            }
        }
        before = previous;
        previous = current;
    }
    previous.last().copied().unwrap_or(0)
}

/// The indices of the matching `targets`, best first; equal scores keep the given order.
/// An empty query returns every index in order.
pub fn rank<'a>(query: &str, targets: impl IntoIterator<Item = (&'a str, i64)>) -> Vec<usize> {
    let matcher = Matcher::new(query);
    let mut scored: Vec<(usize, i64)> = targets
        .into_iter()
        .enumerate()
        .filter_map(|(index, (text, bonus))| {
            if matcher.is_empty() {
                Some((index, 0))
            } else {
                matcher.score(text).map(|score| (index, score + bonus * 1000))
            }
        })
        .collect();
    scored.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
    scored.into_iter().map(|(index, _)| index).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order<'a>(query: &str, targets: &[&'a str]) -> Vec<&'a str> {
        rank(query, targets.iter().map(|t| (*t, 0))).into_iter().map(|i| targets[i]).collect()
    }

    #[test]
    fn the_start_of_a_word_beats_the_middle() {
        let programs = ["Wirtschaftsinformatik Bachelor", "Medizininformatik Master", "Informatik Bachelor", "Architektur Bachelor"];
        assert_eq!(order("inf", &programs), vec!["Informatik Bachelor", "Medizininformatik Master", "Wirtschaftsinformatik Bachelor"]);
        assert_eq!(order("informatik", &programs).first(), Some(&"Informatik Bachelor"));
    }

    #[test]
    fn every_word_of_the_query_has_to_match() {
        let programs = ["Informatik Bachelor of Science PO 2008", "Informatik Master of Science PO 2008", "Physik Master of Science"];
        assert_eq!(order("inf master", &programs), vec!["Informatik Master of Science PO 2008"]);
        assert_eq!(order("master inf", &programs), vec!["Informatik Master of Science PO 2008"]);
        assert_eq!(order("inf 2008 xyz", &programs), Vec::<&str>::new());
    }

    #[test]
    fn typos_are_forgiven_when_the_first_letter_is_right() {
        let programs = ["Informatik", "Mathematik", "Physik"];
        for typo in ["infomatik", "informaitk", "infotmatik", "informatikk"] {
            assert_eq!(order(typo, &programs), vec!["Informatik"], "{typo}");
        }
        assert_eq!(order("onformatik", &programs), Vec::<&str>::new(), "the first letter has to be right");
        assert_eq!(order("phx", &programs), Vec::<&str>::new(), "short words get no typo");
    }

    #[test]
    fn umlauts_case_and_punctuation_do_not_matter() {
        let people = ["Köhler, Ekkehard", "Kohl, Anna", "Meer, Klaus"];
        assert_eq!(order("koehler", &people), vec!["Köhler, Ekkehard"], "the spelled-out umlaut counts as one typo");
        assert_eq!(order("KOHLER", &people), vec!["Köhler, Ekkehard"]);
        assert_eq!(order("köhler e", &people), vec!["Köhler, Ekkehard"]);
        assert_eq!(order("klaus meer", &people), vec!["Meer, Klaus"]);
    }

    #[test]
    fn abbreviations_with_dots_are_one_word_too() {
        let programs = ["Informatik B.Sc. · PO 2008", "Informatik M.Sc. · PO 2008", "Wirtschaftsinformatik B.Sc. · PO 2017"];
        assert_eq!(order("informatik bsc", &programs), vec!["Informatik B.Sc. · PO 2008", "Wirtschaftsinformatik B.Sc. · PO 2017"]);
        assert_eq!(order("infomatik msc", &programs), vec!["Informatik M.Sc. · PO 2008"]);
        assert_eq!(order("inf b.sc 2008", &programs), vec!["Informatik B.Sc. · PO 2008"]);
    }

    #[test]
    fn initials() {
        let modules = ["Theoretische Informatik", "Technische Mechanik", "Bachelor of Science"];
        assert_eq!(order("ti", &modules).first(), Some(&"Theoretische Informatik"));
        assert_eq!(order("bs", &modules), vec!["Bachelor of Science"]);
    }

    #[test]
    fn an_empty_query_keeps_the_order_and_ties_prefer_the_shorter() {
        let items = ["b", "a", "c"];
        assert_eq!(order("  ", &items), vec!["b", "a", "c"]);
        assert_eq!(order("bau", &["Bauingenieurwesen dual", "Bauingenieurwesen"]), vec!["Bauingenieurwesen", "Bauingenieurwesen dual"]);
    }

    #[test]
    fn a_bonus_lifts_equal_matches() {
        let targets = [("Informatik PO 2008", 0), ("Informatik PO 2023", 1)];
        assert_eq!(rank("informatik", targets), vec![1, 0]);
    }

    #[test]
    fn distances() {
        let d = |a: &str, b: &str| edit_distance(&a.chars().collect::<Vec<_>>(), &b.chars().collect::<Vec<_>>());
        assert_eq!((d("abc", "abc"), d("abc", "acb"), d("abc", "ab"), d("abc", "xbc"), d("", "ab")), (0, 1, 1, 1, 2));
    }
}
