//! Text folding for every search of the app, so „okologie" finds „Ökologie" and „strasse"
//! finds „Straße" on the server and in the browser alike; and the search of the module catalog.
//!
//! The catalog's search (`rank`, owner, 2026-10-19; folia/design/suche/konzept.html) finds the words
//! of a query in the names of a module — its number, its abbreviations, its two titles — and in
//! the names of its lecturers, not in the texts of its description. Every word has to be found,
//! in any order, and fillers like „für" do not count. `Plan` hands what a query finds, in the
//! order of the list, to the catalog's SQL. Where the words as typed find no module at all,
//! `resolve` corrects their typos from the words of the titles, and where that finds none either,
//! the modules with the most of the words are listed. What a query finds depends on the query
//! alone; the visitor's program changes the order only.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod fuzzy;
pub mod rank;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use folia_model::db::{Database, DbError, Value};
use folia_model::rows::SearchMarks;
use serde::{Deserialize, Serialize};

/// Lower case, `ß` → `ss`, umlauts and the common Latin diacritics removed.
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        match c {
            'ä' | 'à' | 'á' | 'â' | 'ã' | 'å' => out.push('a'),
            'ö' | 'ò' | 'ó' | 'ô' | 'õ' | 'ø' => out.push('o'),
            'ü' | 'ù' | 'ú' | 'û' => out.push('u'),
            'è' | 'é' | 'ê' | 'ë' => out.push('e'),
            'ì' | 'í' | 'î' | 'ï' => out.push('i'),
            'ç' => out.push('c'),
            'ñ' => out.push('n'),
            'ß' => out.push_str("ss"),
            other => out.push(other),
        }
    }
    out
}

/// True if every word of `query` occurs in `text` (both folded).
pub fn matches(text: &str, query: &str) -> bool {
    let text = fold(text);
    fold(query).split_whitespace().all(|word| text.contains(word))
}

/// Words the pickers' initials skip („Algorithmen und Datenstrukturen" is „ad", `fuzzy`). Radix
/// skips the same in the initials it folds (`normalize.SearchFillers`, held to this list by
/// radix/internal/normalize/testdata/search.tsv). The catalog's search has a list of its own
/// (`rank::FILLERS`).
pub const FILLERS: &[&str] = &["of", "and", "und", "der", "die", "das", "in", "im", "fur", "the", "zur", "zum", "von", "mit"];

/// The words of a text as the search sees them: folded, and parted at every character that is
/// neither a letter nor a digit („Python-Programmierung" is python and programmierung), as Radix
/// parts the titles (`normalize.SearchWords`).
pub fn words(text: &str) -> Vec<String> {
    fold(text).split(|c: char| !c.is_alphanumeric()).filter(|word| !word.is_empty()).map(str::to_string).collect()
}

/// Roman numerals up to ten: a number is found as its numeral and the other way round.
pub(crate) const ROMAN: [&str; 10] = ["i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x"];

/// Whether a text searches at all: it has a letter or a digit.
pub fn has_words(text: &str) -> bool {
    text.chars().any(char::is_alphanumeric)
}

/// How the catalog searched a text where the text as typed found no module (`resolve`); the
/// default is the text as typed. Derived by the page, never part of a URL.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Resolution {
    /// Words that found nothing, each with the word of a title searched instead.
    pub corrected: Vec<Correction>,
    /// No module has all the words: the list holds the modules with the most of them.
    pub most_words: bool,
}

/// A word of a query that found nothing, and the word of a title it was taken for.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Correction {
    /// The word as the search saw it (folded).
    pub typed: String,
    /// The word searched instead (folded).
    pub word: String,
    /// That word as a title writes it („Algorithmen").
    pub shown: String,
}

impl Resolution {
    /// The text as it was searched: `text` with every corrected word as a title writes it, in
    /// lower case where the text had no capital there („maschinelles lernen").
    pub fn searched_text(&self, text: &str) -> String {
        let correction = |word: &str| self.corrected.iter().find(|c| c.typed == word);
        text.split_whitespace()
            .map(|token| {
                let parts = words(token);
                if !parts.iter().any(|word| correction(word).is_some()) {
                    return token.to_string();
                }
                let lower = !token.chars().any(char::is_uppercase);
                parts
                    .iter()
                    .map(|word| match correction(word) {
                        Some(c) if lower => c.shown.to_lowercase(),
                        Some(c) => c.shown.clone(),
                        None => word.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// What a search finds, in the order of the list: the table the catalog's SQL joins
/// (`folia_query`), one row per module found with how many of the words it has and its place.
pub struct Plan {
    /// `[["11787",1,0],…]`: the module, its words found, its place.
    hits: Arc<str>,
    words: usize,
    most_words: bool,
}

impl Plan {
    /// The plan of `text` as `resolution` has it searched (as typed without one), the modules of
    /// `program` raised (their order only, `rank::points::PROGRAM`); `None` for a blank text, which
    /// is no search.
    pub fn new(db: &dyn Database, text: &str, resolution: Option<&Resolution>, program: Option<&str>) -> Result<Option<Self>, DbError> {
        if text.trim().is_empty() {
            return Ok(None);
        }
        let most_words = resolution.is_some_and(|r| r.most_words);
        let ranked = ranked(db, text, resolution, program)?;
        let mut hits = String::with_capacity(ranked.hits.len() * 16 + 2);
        hits.push('[');
        for (place, (id, found)) in ranked.hits.iter().enumerate() {
            if place > 0 {
                hits.push(',');
            }
            hits.push_str(&format!("[\"{id}\",{found},{place}]"));
        }
        hits.push(']');
        Ok(Some(Self { hits: hits.into(), words: ranked.words, most_words }))
    }

    /// How many words the plan searches for.
    pub fn len(&self) -> usize {
        self.words
    }

    /// Whether the plan searches for no word at all.
    pub fn is_empty(&self) -> bool {
        self.words == 0
    }

    /// The modules found, `(SELECT module_id, matched, rank FROM …)`, with its parameter:
    /// `matched` is how many of the words a module has, `rank` its place in the order of the list.
    /// `None` for a text without a word, which finds nothing.
    pub fn table(&self) -> Option<(String, Vec<Value>)> {
        if self.words == 0 {
            return None;
        }
        Some((
            "(SELECT json_extract(value, '$[0]') AS module_id, json_extract(value, '$[1]') AS matched, \
             json_extract(value, '$[2]') AS rank FROM json_each(?))"
                .to_string(),
            vec![Value::Text(self.hits.to_string())],
        ))
    }

    /// The condition on `table` joined as `alias`: every word found, or, for the most words, one
    /// of them; nothing for a text without a word.
    pub fn condition(&self, alias: &str) -> String {
        match (self.words, self.most_words) {
            (0, _) => "0".to_string(),
            (_, true) => format!("{alias}.matched > 0"),
            (n, false) => format!("{alias}.matched = {n}"),
        }
    }
}

/// What a query finds, in the order of the list: each module's id and how many words it has.
struct Ranked {
    hits: Vec<(String, usize)>,
    words: usize,
}

/// What a query finds, in no order, with the points of the query alone, and its words.
struct Evaluated {
    hits: Vec<rank::Hit>,
    words: Vec<rank::QueryWord>,
}

/// The snapshot a search runs on (its `content_digest`), the text, and how it is searched: the
/// text as typed is `None`, whether no resolution was made or it found the text as typed.
type Searched = (Option<String>, String, Option<Resolution>);

fn searched(db: &dyn Database, text: &str, resolution: Option<&Resolution>) -> Result<Searched, DbError> {
    let digest = folia_model::rows::meta(db)?.content_digest;
    Ok((digest, text.to_string(), resolution.filter(|r| **r != Resolution::default()).cloned()))
}

/// What `text` finds as `resolution` has it searched, in no order; the last one kept: while
/// typing, a page resolves the text (with the list and with the counts of its filters) and orders
/// what it finds by the program (`ranked`), all from one evaluation.
fn evaluated(db: &dyn Database, text: &str, resolution: Option<&Resolution>) -> Result<(Arc<rank::Index>, Arc<Evaluated>), DbError> {
    static LAST: Mutex<Option<(Searched, Arc<Evaluated>)>> = Mutex::new(None);
    let key = searched(db, text, resolution)?;
    let index = rank::index(db)?;
    if key.0.is_some() {
        if let Some(kept) = LAST.lock().ok().and_then(|last| last.as_ref().filter(|(of, _)| *of == key).map(|(_, evaluated)| evaluated.clone())) {
            return Ok((index, kept));
        }
    }
    let words = rank::query_words(&index, text, key.2.as_ref());
    let hits = rank::evaluate(&index, &words, key.2.as_ref().is_some_and(|r| r.most_words));
    let evaluated = Arc::new(Evaluated { hits, words });
    if key.0.is_some() {
        if let Ok(mut last) = LAST.lock() {
            *last = Some((key, evaluated.clone()));
        }
    }
    Ok((index, evaluated))
}

/// What `text` finds as `resolution` has it searched, in the order of the list, `program`'s
/// modules raised; the last one kept, since a page asks for the list, its count and what lies
/// outside its filters in turn.
fn ranked(db: &dyn Database, text: &str, resolution: Option<&Resolution>, program: Option<&str>) -> Result<Arc<Ranked>, DbError> {
    /// A search and the program that orders it.
    type Ordered = (Searched, Option<String>);
    static LAST: Mutex<Option<(Ordered, Arc<Ranked>)>> = Mutex::new(None);
    let program = program.filter(|p| !p.is_empty());
    let key = (searched(db, text, resolution)?, program.map(str::to_string));
    if key.0 .0.is_some() {
        if let Some(kept) = LAST.lock().ok().and_then(|last| last.as_ref().filter(|(of, _)| *of == key).map(|(_, ranked)| ranked.clone())) {
            return Ok(kept);
        }
    }
    let (index, evaluated) = evaluated(db, text, resolution)?;
    let mine = match program {
        Some(program) => search_program_modules(db, program)?,
        None => HashSet::new(),
    };
    let hits = rank::order(&index, &evaluated.hits, &evaluated.words, &mine)
        .into_iter()
        .filter_map(|i| evaluated.hits.get(i))
        .filter_map(|hit| index.module_id(hit.module).map(|id| (id.to_string(), hit.found)))
        .collect();
    let ranked = Arc::new(Ranked { hits, words: evaluated.words.len() });
    if key.0 .0.is_some() {
        if let Ok(mut last) = LAST.lock() {
            *last = Some((key, ranked.clone()));
        }
    }
    Ok(ranked)
}

/// Makes the search's index of the snapshot (`rank::index`) before the first search asks for it:
/// the data worker, as a search field takes the focus. That first search then answers as fast as
/// the next ones (the index takes a quarter of a second of a laptop in the worker, a phone's
/// second).
pub fn prepare(db: &dyn Database) -> Result<(), DbError> {
    rank::index(db).map(|_| ())
}

/// What the catalog searches `text` for (`pages::catalog`): the text as typed where some module
/// has all its words; else with every word no module has corrected from the words of the titles,
/// where that finds a module; else, for two words and more, the modules with the most of them.
/// A text that finds nothing in any of these ways is searched as typed. Filters and the profile
/// play no part: what the text finds outside the filters, the list says below its rows.
pub fn resolve(db: &dyn Database, text: &str) -> Result<Resolution, DbError> {
    // The last one kept: a page resolves its text for the list and for the counts of its filters.
    /// The snapshot (its `content_digest`) and the text.
    type Text = (Option<String>, String);
    static LAST: Mutex<Option<(Text, Resolution)>> = Mutex::new(None);
    let key = (folia_model::rows::meta(db)?.content_digest, text.to_string());
    if key.0.is_some() {
        if let Some(kept) = LAST.lock().ok().and_then(|last| last.as_ref().filter(|(of, _)| *of == key).map(|(_, resolution)| resolution.clone())) {
            return Ok(kept);
        }
    }
    let resolution = resolved(db, text)?;
    if key.0.is_some() {
        if let Ok(mut last) = LAST.lock() {
            *last = Some((key, resolution.clone()));
        }
    }
    Ok(resolution)
}

fn resolved(db: &dyn Database, text: &str) -> Result<Resolution, DbError> {
    let (index, as_typed) = evaluated(db, text, None)?;
    let words = &as_typed.words;
    if words.is_empty() || !as_typed.hits.is_empty() {
        return Ok(Resolution::default());
    }
    let any = rank::evaluate(&index, words, true);
    let mut corrected = Vec::new();
    for (k, word) in words.iter().enumerate() {
        let found = any.iter().any(|hit| hit.answer.best.get(k).is_some_and(Option::is_some));
        if found {
            continue;
        }
        let typed: String = word.q.iter().collect();
        if let Some((instead, shown)) = index.vocabulary().correct(&typed) {
            corrected.push(Correction { typed, word: instead, shown });
        }
    }
    if !corrected.is_empty() {
        let resolution = Resolution { corrected: corrected.clone(), most_words: false };
        if found_count(db, text, Some(&resolution))? > 0 {
            return Ok(resolution);
        }
    }
    if words.len() > 1 {
        let resolution = Resolution { corrected, most_words: true };
        if found_count(db, text, Some(&resolution))? > 0 {
            return Ok(resolution);
        }
    }
    Ok(Resolution::default())
}

/// The words of the titles, for the correction of a typo: every word with the number of modules
/// that have it, and as the first of them writes it.
#[derive(Debug)]
pub(crate) struct Vocabulary {
    words: Vec<(String, u32, String)>,
}

impl Vocabulary {
    pub(crate) fn new(titles: &[Titles]) -> Self {
        let mut counted: HashMap<String, (u32, String)> = HashMap::new();
        for (de, en) in titles {
            let mut of_module = HashSet::new();
            for title in [de, en].into_iter().flatten() {
                for piece in title.split(|c: char| !c.is_alphanumeric()).filter(|piece| !piece.is_empty()) {
                    for word in words(piece) {
                        if of_module.insert(word.clone()) {
                            counted.entry(word).or_insert_with(|| (0, piece.to_string())).0 += 1;
                        }
                    }
                }
            }
        }
        let mut words: Vec<(String, u32, String)> = counted.into_iter().map(|(word, (modules, shown))| (word, modules, shown)).collect();
        words.sort();
        Self { words }
    }

    /// The word of a title that `word` is a typo of, and how the title writes it: of the words that
    /// start with its letter and are within one typo of it or of their start (two from eight letters
    /// on), the one with the fewest, then the one most modules have. Words of fewer than four
    /// letters, and words with a digit, are not corrected.
    pub(crate) fn correct(&self, word: &str) -> Option<(String, String)> {
        let typed: Vec<char> = word.chars().collect();
        if !typed.iter().all(|c| c.is_alphabetic()) {
            return None;
        }
        self.words
            .iter()
            .filter_map(|(candidate, modules, shown)| {
                let letters: Vec<char> = candidate.chars().collect();
                fuzzy::typos_at_start(&typed, &letters).map(|typos| (typos, *modules, candidate, shown))
            })
            .min_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.len().cmp(&b.2.len())).then(a.2.cmp(b.2)))
            .map(|(_, _, candidate, shown)| (candidate.clone(), shown.clone()))
    }

    /// Whether some word of a title starts with `start`.
    pub(crate) fn starts_some_word(&self, start: &str) -> bool {
        let at = self.words.partition_point(|(word, _, _)| word.as_str() < start);
        self.words.get(at).is_some_and(|(word, _, _)| word.starts_with(start))
    }
}

// The statements of the search: the only SQL of this crate besides `Plan`'s.

/// How many modules the search of `text` finds, as `resolution` has it searched, whatever the
/// filters: the whole catalog, offered or not (`search::resolve`).
pub fn found_count(db: &dyn Database, text: &str, resolution: Option<&Resolution>) -> Result<u64, DbError> {
    if text.trim().is_empty() {
        return Ok(0);
    }
    Ok(evaluated(db, text, resolution)?.1.hits.len() as u64)
}

/// Which of the words of `text` some module has, in the order the search takes them
/// (`search::resolve`): a word no module has is one to correct.
pub fn words_found(db: &dyn Database, text: &str) -> Result<Vec<bool>, DbError> {
    let index = rank::index(db)?;
    let words = rank::query_words(&index, text, None);
    let any = rank::evaluate(&index, &words, true);
    Ok((0..words.len()).map(|k| any.iter().any(|hit| hit.answer.best.get(k).is_some_and(Option::is_some))).collect())
}

/// What the search of `text` (as `resolution` has it searched) found in each of the modules of
/// `ids` that it finds: the letters of its title, the other title or a lecturer's name it found,
/// the abbreviation. The rows of the list mark them.
pub fn marks(db: &dyn Database, text: &str, resolution: Option<&Resolution>, ids: &[String]) -> Result<HashMap<String, SearchMarks>, DbError> {
    let mut marks = HashMap::new();
    if text.trim().is_empty() || ids.is_empty() {
        return Ok(marks);
    }
    let index = rank::index(db)?;
    let words = rank::query_words(&index, text, resolution);
    for id in ids {
        let Some(i) = index.position(id) else { continue };
        let Some(answer) = index.answer_of(i, &words).filter(|answer| answer.found > 0) else { continue };
        if let Some(found) = index.marks(i, &answer, &index.shown_finds(i, &words)) {
            marks.insert(id.clone(), found);
        }
    }
    Ok(marks)
}

/// The German and the English title of a module.
pub type Titles = (Option<String>, Option<String>);

// The statements of the search, each read once per snapshot but the last (`rank::index`).

/// The names of a module the search compares a query with, as the snapshot has them.
#[derive(Clone, Debug, PartialEq)]
pub struct ModuleNames {
    pub id: String,
    /// The title the list shows.
    pub title: String,
    pub title_de: Option<String>,
    pub title_en: Option<String>,
    pub offer_status: Option<String>,
    /// The module's own abbreviation („ThI").
    pub abbrev: Option<String>,
}

fn text_of(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::Text(text)) => Some(text.clone()),
        _ => None,
    }
}

/// Every module's names, in the order of the ids.
pub fn search_modules(db: &dyn Database) -> Result<Vec<ModuleNames>, DbError> {
    let rows = db.query("search_modules", "SELECT id, title, title_de, title_en, offer_status, abbrev FROM v_module ORDER BY id", &[])?;
    Ok(rows
        .rows
        .iter()
        .filter_map(|row| {
            Some(ModuleNames {
                id: text_of(row.first())?,
                title: text_of(row.get(1))?,
                title_de: text_of(row.get(2)),
                title_en: text_of(row.get(3)),
                offer_status: text_of(row.get(4)),
                abbrev: text_of(row.get(5)),
            })
        })
        .collect())
}

/// The abbreviations the programs give the modules („AuP"): module, abbreviation.
pub fn search_abbrevs(db: &dyn Database) -> Result<Vec<(String, String)>, DbError> {
    let rows = db.query("search_abbrevs", "SELECT DISTINCT module_id, abbrev FROM v_program_module WHERE abbrev IS NOT NULL", &[])?;
    Ok(rows.rows.iter().filter_map(|row| Some((text_of(row.first())?, text_of(row.get(1))?))).collect())
}

/// How many programs (their newest regulations) have each module in their curriculum.
pub fn search_programs(db: &dyn Database) -> Result<Vec<(String, u32)>, DbError> {
    let rows = db.query(
        "search_programs",
        "SELECT pm.module_id, COUNT(DISTINCT pm.program_id) FROM v_program_module pm JOIN v_program p ON p.id = pm.program_id \
         WHERE pm.relation = 'curricular' AND p.is_latest_po = 1 GROUP BY pm.module_id",
        &[],
    )?;
    Ok(rows
        .rows
        .iter()
        .filter_map(|row| {
            let count = match row.get(1) {
                Some(Value::Integer(n)) => u32::try_from(*n).unwrap_or(0),
                _ => 0,
            };
            Some((text_of(row.first())?, count))
        })
        .collect())
}

/// A lecturer of a module: the module, the name as QIS writes it („Hofstedt, Petra"), and whether
/// responsible for it (else teaching it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LecturerOf {
    pub module_id: String,
    pub name: String,
    pub responsible: bool,
}

/// Every module's lecturers, by module, the responsible first.
pub fn search_lecturers(db: &dyn Database) -> Result<Vec<LecturerOf>, DbError> {
    let rows = db.query("search_lecturers", "SELECT DISTINCT module_id, name, role FROM v_module_lecturer ORDER BY module_id, role DESC, name", &[])?;
    Ok(rows
        .rows
        .iter()
        .filter_map(|row| {
            Some(LecturerOf {
                module_id: text_of(row.first())?,
                name: text_of(row.get(1))?,
                responsible: text_of(row.get(2)).as_deref() == Some("responsible"),
            })
        })
        .collect())
}

/// The modules of a program's curriculum, which the order raises.
pub fn search_program_modules(db: &dyn Database, program: &str) -> Result<HashSet<String>, DbError> {
    let rows = db.query(
        "search_program_modules",
        "SELECT DISTINCT module_id FROM v_program_module WHERE program_id = ? AND relation = 'curricular'",
        &[Value::from(program)],
    )?;
    Ok(rows.rows.iter().filter_map(|row| text_of(row.first())).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folding() {
        assert_eq!(fold("Ökologie & Straße"), "okologie & strasse");
        assert!(!matches("Umweltingenieurwesen (B.Sc.)", "umwelt bsc"));
        assert!(matches("Umweltingenieurwesen B.Sc.", "UMWELT b.sc"));
        assert!(matches("Ökologie", "okolog"));
        assert!(matches("anything", "  "));
    }

    /// Radix folds the titles and Folia the query: radix/internal/normalize/testdata/search.tsv holds both
    /// to the same cases (Radix's `TestSearchTerms` reads it too).
    #[test]
    fn folding_is_radixs() {
        let fixture = include_str!("../../../../radix/internal/normalize/testdata/search.tsv");
        let empty = |s: &str| if s == "∅" { String::new() } else { s.to_string() };
        let mut checked = 0;
        for line in fixture.lines().filter(|line| !line.is_empty() && !line.starts_with('#')) {
            let fields: Vec<&str> = line.split('\t').collect();
            let [kind, input, expected] = fields.as_slice() else { panic!("not kind<TAB>input<TAB>expected: {line:?}") };
            let got = match *kind {
                "fold" => fold(&empty(input)),
                "words" => words(&empty(input)).join(" "),
                "fillers" => FILLERS.join(" "),
                _ => continue,
            };
            assert_eq!(got, empty(expected), "{kind}({input:?})");
            checked += 1;
        }
        assert!(checked > 10, "the fixture holds the cases of fold, words and fillers");
    }

    #[test]
    fn a_text_searches_with_a_letter_or_a_digit() {
        assert!(has_words("TI"));
        assert!(has_words(" 118 "));
        assert!(!has_words("%_\\"));
        assert!(!has_words("   "));
    }

    #[test]
    fn the_searched_text_says_what_was_corrected() {
        let resolution = Resolution {
            corrected: vec![Correction { typed: "lernnen".into(), word: "lernen".into(), shown: "Lernen".into() }],
            most_words: false,
        };
        assert_eq!(resolution.searched_text("maschinelles lernnen"), "maschinelles lernen");
        assert_eq!(resolution.searched_text("Maschinelles Lernnen"), "Maschinelles Lernen");
        assert_eq!(Resolution::default().searched_text("Algebra  I"), "Algebra I");
    }

    #[test]
    fn a_typo_becomes_the_word_most_modules_have() {
        let titles = [
            (Some("Algorithmen und Datenstrukturen".to_string()), Some("Algorithms and Data Structures".to_string())),
            (Some("Algorithmische Geometrie".to_string()), None),
            (Some("Statistik".to_string()), Some("Statistics".to_string())),
            (Some("Statik".to_string()), None),
            (Some("Wirtschaftsinformatik".to_string()), None),
        ];
        let vocabulary = Vocabulary::new(&titles);
        let correct = |word: &str| vocabulary.correct(word).map(|(word, shown)| format!("{word} {shown}"));
        assert_eq!(correct("algoritmen").as_deref(), Some("algorithmen Algorithmen"));
        assert_eq!(correct("statistk").as_deref(), Some("statistik Statistik"));
        assert_eq!(correct("wirtschaftsinfromatik").as_deref(), Some("wirtschaftsinformatik Wirtschaftsinformatik"));
        assert_eq!(correct("xtatistik"), None, "the first letter has to be right");
        assert_eq!(correct("stat"), None, "a word that is the start of one needs no correction and gets none");
        assert_eq!(correct("sta"), None, "short words are not corrected");
        assert_eq!(correct("statistk2"), None, "nor words with a digit");
        assert!(vocabulary.starts_some_word("algo") && vocabulary.starts_some_word("statistik"));
        assert!(!vocabulary.starts_some_word("theoinf") && !vocabulary.starts_some_word("statistiken"));
    }
}
