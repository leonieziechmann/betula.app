//! Which module of the catalog a row of the Notenübersicht is. The Notenübersicht names no module
//! numbers, only titles, and it takes them from the same QIS the catalog does: a row is the module
//! one of whose titles is one of the row's two, compared folded (case, accents, hyphens and
//! punctuation aside). Titles repeat across the university („Proseminar", 68 „Bachelor-Arbeit"),
//! so among the modules of a title the program's own come first (its curriculum and its FÜS list),
//! then the one whose main title is the row's first line (the title in the language the module is
//! taught in: „Graph Theory" over „Graphentheorie"), then the one of the row's credits, then one
//! still offered. A row none of whose titles the catalog has stays without a module.

use std::collections::BTreeMap;

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
    /// The program's own: its curriculum or its FÜS list.
    pub claimed: bool,
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

/// The catalog's modules by their folded titles.
pub struct Matcher {
    candidates: Vec<Candidate>,
    by_title: BTreeMap<String, Vec<usize>>,
}

impl Matcher {
    pub fn new(candidates: Vec<Candidate>) -> Matcher {
        let mut by_title: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, candidate) in candidates.iter().enumerate() {
            for title in [Some(&candidate.title), candidate.title_de.as_ref(), candidate.title_en.as_ref()].into_iter().flatten() {
                let folded = fold(title);
                if folded.is_empty() {
                    continue;
                }
                let list = by_title.entry(folded).or_default();
                if !list.contains(&index) {
                    list.push(index);
                }
            }
        }
        Matcher { candidates, by_title }
    }

    /// The module of a row: its first line `title`, its second `other`, its `credits` (0 where
    /// it is not passed, which then says nothing).
    pub fn find(&self, title: &str, other: Option<&str>, credits: Option<f64>) -> Option<&Candidate> {
        let own = fold(title);
        let other = other.map(fold).filter(|other| !other.is_empty());
        let mut found: Vec<usize> = Vec::new();
        for key in [Some(&own), other.as_ref()].into_iter().flatten() {
            for index in self.by_title.get(key).into_iter().flatten() {
                if !found.contains(index) {
                    found.push(*index);
                }
            }
        }
        let score = |candidate: &Candidate| -> u32 {
            let titles = [Some(&candidate.title), candidate.title_de.as_ref(), candidate.title_en.as_ref()];
            let first = if fold(&candidate.title) == own {
                4
            } else if titles.iter().flatten().any(|title| fold(title) == own) {
                2
            } else {
                0
            };
            let worth = credits.filter(|credits| *credits > 0.0).is_some_and(|credits| candidate.credits.is_some_and(|own| (own - credits).abs() < 1e-6));
            8 * u32::from(candidate.claimed) + first + 2 * u32::from(worth) + u32::from(candidate.offered)
        };
        found
            .into_iter()
            .filter_map(|index| self.candidates.get(index))
            // The highest score; of equals the lowest number, so that a row finds the same module
            // on every device.
            .max_by(|a, b| score(a).cmp(&score(b)).then_with(|| b.id.cmp(&a.id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, title: &str, de: Option<&str>, en: Option<&str>, credits: f64, offered: bool, claimed: bool) -> Candidate {
        Candidate { id: id.into(), title: title.into(), title_de: de.map(Into::into), title_en: en.map(Into::into), credits: Some(credits), offered, claimed }
    }

    #[test]
    fn titles_folded() {
        assert_eq!(fold("Audio- und Signalverarbeitung"), "audio und signalverarbeitung");
        assert_eq!(fold("Mathematik IT-1 (Diskrete Mathematik)"), "mathematik it1 diskrete mathematik");
        assert_eq!(fold("Grundzüge der Kognition und Wahrnehmung"), "grundzuge der kognition und wahrnehmung");
        assert_eq!(fold("Mikroelektronik: Entwurfsautomatisierung"), "mikroelektronik entwurfsautomatisierung");
        assert_eq!(fold("  Straße  "), "strasse");
        assert_eq!(fold("Bachelor-Arbeit"), fold("Bachelorarbeit"));
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
}
