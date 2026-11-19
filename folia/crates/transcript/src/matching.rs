//! Which module of the catalog a row of the Notenübersicht is. The Notenübersicht names no module
//! numbers, only titles, and it takes them from the same QIS the catalog does: a row is the module
//! one of whose titles is one of the row's two, compared by their keys (`key`: case, accents,
//! hyphens, punctuation and spaces aside). Titles repeat across the university („Proseminar", 68
//! „Bachelor-Arbeit"), so among the modules of a title the program's own come first (its
//! curriculum and its FÜS list; then another PO's of the same program), then the one whose main
//! title is the row's first line (the title in the language the module is taught in: „Graph
//! Theory" over „Graphentheorie"), then the one that has the row's second title as well
//! („Grundlagen der Elektrotechnik" as „General Electrical Engineering" or as „Fundamentals in
//! Electrical Engineering"), then the one of the row's credits, then one still offered. A row none
//! of whose titles the catalog has stays without a module.
//!
//! The rows of one Notenübersicht are as many modules (`Matcher::assign`): a program can have a
//! title several times, as slots alike (owner, 2026-11-19, of a friend's Notenübersicht: „zwei ihrer
//! module nicht importiert"; the Informatik Master has six modules „Seminar" and two „Praktikum"),
//! and every row of that title takes one of them; until then all took the same, and taking them
//! over left one of them.

use std::collections::{BTreeMap, BTreeSet};

/// A module of the catalog, as the matching sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub id: String,
    pub title: String,
    pub title_de: Option<String>,
    pub title_en: Option<String>,
    pub credits: Option<f64>,
    /// Still offered (neither phased out nor gone).
    pub offered: bool,
    /// How much it is the program's own: 2 in its curriculum or its FÜS list, 1 in another PO's of
    /// the same program, 0 not.
    pub claimed: u8,
}

/// A title as it is compared: lower case, without accents, „ß" as „ss", without hyphens (a word
/// broken at the end of a line, „IT-1" and „IT1" alike), other punctuation a space, the spaces
/// single.
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut gap = false;
    for c in text.chars().flat_map(char::to_lowercase) {
        let plain = match c {
            'ä' | 'á' | 'à' | 'â' | 'ã' | 'å' => Some('a'),
            'ö' | 'ó' | 'ò' | 'ô' | 'õ' | 'ø' => Some('o'),
            'ü' | 'ú' | 'ù' | 'û' => Some('u'),
            'é' | 'è' | 'ê' | 'ë' => Some('e'),
            'í' | 'ì' | 'î' | 'ï' => Some('i'),
            'ç' => Some('c'),
            'ñ' => Some('n'),
            '-' | '\u{2010}' | '\u{2011}' | '\u{00ad}' => continue,
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        };
        match plain {
            Some(c) => {
                if gap && !out.is_empty() {
                    out.push(' ');
                }
                gap = false;
                if c == 'ß' {
                    out.push_str("ss");
                } else {
                    out.push(c);
                }
            }
            None => gap = true,
        }
    }
    out
}

/// What a title is compared by: folded, without spaces. A line of a title can end after a hyphen
/// that stands for a word's end („Kosten-" over „und Leistungsrechnung"), and where it was read
/// without the space after it, its key is the same.
pub fn key(text: &str) -> String {
    fold(text).replace(' ', "")
}

/// A row as it is matched: its first line, its second, its credits (0 where it is not passed,
/// which then says nothing).
pub type Asked<'a> = (&'a str, Option<&'a str>, Option<f64>);

/// The catalog's modules by the keys of their titles.
pub struct Matcher {
    candidates: Vec<Candidate>,
    /// Each candidate's keys: its main title's, then its others'.
    keys: Vec<Vec<String>>,
    by_title: BTreeMap<String, Vec<usize>>,
}

impl Matcher {
    pub fn new(candidates: Vec<Candidate>) -> Matcher {
        let mut by_title: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut keys = Vec::with_capacity(candidates.len());
        for (index, candidate) in candidates.iter().enumerate() {
            let own: Vec<String> = [Some(&candidate.title), candidate.title_de.as_ref(), candidate.title_en.as_ref()].into_iter().flatten().map(|title| key(title)).collect();
            for title in own.iter().filter(|title| !title.is_empty()) {
                let list = by_title.entry(title.clone()).or_default();
                if !list.contains(&index) {
                    list.push(index);
                }
            }
            keys.push(own);
        }
        Matcher { candidates, keys, by_title }
    }

    /// The modules a row can be, best first: by the score of the module doc above; of equals the
    /// lowest number, so that a row finds the same module on every device.
    fn ranked(&self, (title, other, credits): Asked) -> Vec<usize> {
        let own = key(title);
        let other = other.map(key).filter(|other| !other.is_empty());
        let mut found: Vec<usize> = Vec::new();
        for title in [Some(&own), other.as_ref()].into_iter().flatten() {
            for index in self.by_title.get(title).into_iter().flatten() {
                if !found.contains(index) {
                    found.push(*index);
                }
            }
        }
        let score = |index: usize| -> u32 {
            let (Some(candidate), Some(keys)) = (self.candidates.get(index), self.keys.get(index)) else { return 0 };
            let first = if keys.first() == Some(&own) {
                4
            } else if keys.contains(&own) {
                2
            } else {
                0
            };
            let second = other.as_ref().is_some_and(|other| keys.contains(other));
            let worth = credits.filter(|credits| *credits > 0.0).is_some_and(|credits| candidate.credits.is_some_and(|own| (own - credits).abs() < 1e-6));
            16 * u32::from(candidate.claimed) + first + u32::from(second) + 2 * u32::from(worth) + u32::from(candidate.offered)
        };
        let id = |index: usize| self.candidates.get(index).map_or("", |candidate| candidate.id.as_str());
        found.sort_by(|a, b| score(*b).cmp(&score(*a)).then_with(|| id(*a).cmp(id(*b))));
        found
    }

    /// The module of one row.
    pub fn find(&self, title: &str, other: Option<&str>, credits: Option<f64>) -> Option<&Candidate> {
        self.ranked((title, other, credits)).first().and_then(|index| self.candidates.get(*index))
    }

    /// The modules of the rows of one Notenübersicht, each a module of its own: a row whose best
    /// module an earlier row has takes the next of its titles (the second „Seminar" the second
    /// seminar), and only where every one is taken the best again.
    pub fn assign(&self, rows: &[Asked]) -> Vec<Option<&Candidate>> {
        let mut taken: BTreeSet<usize> = BTreeSet::new();
        rows.iter()
            .map(|row| {
                let ranked = self.ranked(*row);
                let pick = ranked.iter().find(|index| !taken.contains(*index)).or(ranked.first()).copied();
                pick.and_then(|index| {
                    taken.insert(index);
                    self.candidates.get(index)
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, title: &str, de: Option<&str>, en: Option<&str>, credits: f64, offered: bool, claimed: bool) -> Candidate {
        Candidate { id: id.into(), title: title.into(), title_de: de.map(Into::into), title_en: en.map(Into::into), credits: Some(credits), offered, claimed: if claimed { 2 } else { 0 } }
    }

    #[test]
    fn titles_folded() {
        assert_eq!(fold("Audio- und Signalverarbeitung"), "audio und signalverarbeitung");
        assert_eq!(fold("Mathematik IT-1 (Diskrete Mathematik)"), "mathematik it1 diskrete mathematik");
        assert_eq!(fold("Grundzüge der Kognition und Wahrnehmung"), "grundzuge der kognition und wahrnehmung");
        assert_eq!(fold("Mikroelektronik: Entwurfsautomatisierung"), "mikroelektronik entwurfsautomatisierung");
        assert_eq!(fold("  Straße  "), "strasse");
        assert_eq!(fold("Bachelor-Arbeit"), fold("Bachelorarbeit"));
        // A line broken after a hyphen that stands for a word's end, read without its space.
        assert_eq!(key("Kosten-und Leistungsrechnung"), key("Kosten- und Leistungsrechnung"));
        assert_ne!(key("Kosten- und Leistungsrechnung"), key("Kosten und Leistung"));
    }

    #[test]
    fn the_programs_own_first_then_the_first_line_then_the_credits() {
        let matcher = Matcher::new(vec![
            candidate("11204", "Proseminar", None, None, 4.0, true, false),
            candidate("12111", "Proseminar", None, None, 4.0, true, true),
            candidate("11415", "Graphentheorie", Some("Graphentheorie"), Some("Graph Theory"), 8.0, true, true),
            candidate("14085", "Graph Theory", None, Some("Graph Theory"), 8.0, true, true),
            candidate("11406", "Kryptographie", Some("Kryptographie"), Some("Cryptography"), 6.0, false, false),
            candidate("11859", "Cryptography", None, None, 8.0, true, false),
            candidate("12215", "Theoretische Informatik", None, None, 10.0, false, false),
            candidate("11787", "Theoretische Informatik", None, None, 8.0, true, true),
        ]);
        let id = |title: &str, other: Option<&str>, credits: Option<f64>| matcher.find(title, other, credits).map(|found| found.id.clone());
        assert_eq!(id("Proseminar", Some("Introductory Seminar"), Some(0.0)).as_deref(), Some("12111"));
        assert_eq!(id("Graph Theory", Some("Graphentheorie"), Some(8.0)).as_deref(), Some("14085"));
        assert_eq!(id("Graphentheorie", Some("Graph Theory"), Some(8.0)).as_deref(), Some("11415"));
        assert_eq!(id("Cryptography", Some("Kryptographie"), Some(8.0)).as_deref(), Some("11859"));
        assert_eq!(id("Theoretische Informatik", Some("Theoretical Computer Science"), Some(8.0)).as_deref(), Some("11787"));
        // By the other language's title alone.
        assert_eq!(id("Kryptografie", Some("Cryptography"), None).as_deref(), Some("11859"));
        assert_eq!(id("Unbekanntes Modul", Some("Unknown Module"), None), None);
    }

    /// A title twice in a program, told apart by the second title; a module of another PO of the
    /// program before a module of the catalog that has the second title alone.
    #[test]
    fn the_second_title_and_the_programs_other_pos() {
        let mut other_po = candidate("12481", "Informations- und Codierungstheorie", None, Some("Information and Coding Theory"), 6.0, false, false);
        other_po.claimed = 1;
        let matcher = Matcher::new(vec![
            candidate("12537", "Grundlagen der Elektrotechnik", None, Some("General Electrical Engineering"), 6.0, true, true),
            candidate("12696", "Grundlagen der Elektrotechnik", None, Some("Fundamentals in Electrical Engineering"), 6.0, true, true),
            other_po,
            candidate("11352", "Informations- und Kodierungstheorie", None, Some("Information and Coding Theory"), 6.0, true, false),
        ]);
        let id = |title: &str, other: &str| matcher.find(title, Some(other), Some(6.0)).map(|found| found.id.clone());
        assert_eq!(id("Grundlagen der Elektrotechnik", "Fundamentals in Electrical Engineering").as_deref(), Some("12696"));
        assert_eq!(id("Grundlagen der Elektrotechnik", "General Electrical Engineering").as_deref(), Some("12537"));
        assert_eq!(id("Informations- und Codierungstheorie", "Information and Coding Theory").as_deref(), Some("12481"));
    }

    /// The rows of a Notenübersicht are as many modules: three „Seminar" three of the program's
    /// six, not one three times (owner, 2026-11-19: „zwei ihrer module nicht importiert"); a
    /// fourth where the program has three takes the best again.
    #[test]
    fn every_row_a_module_of_its_own() {
        let matcher = Matcher::new(vec![
            candidate("12461", "Seminar", Some("Seminar"), Some("Seminar"), 4.0, true, true),
            candidate("12462", "Seminar", Some("Seminar"), Some("Seminar"), 4.0, true, true),
            candidate("12463", "Seminar", Some("Seminar"), Some("Seminar"), 4.0, true, true),
            candidate("11204", "Proseminar", None, None, 4.0, true, true),
        ]);
        let seminar: Asked = ("Seminar", Some("Seminar"), Some(4.0));
        let ids: Vec<Option<&str>> = matcher.assign(&[seminar, ("Proseminar", None, Some(4.0)), seminar, seminar, seminar, ("Kein Modul", None, None)]).into_iter().map(|found| found.map(|found| found.id.as_str())).collect();
        assert_eq!(ids, [Some("12461"), Some("11204"), Some("12462"), Some("12463"), Some("12461"), None]);
    }
}
