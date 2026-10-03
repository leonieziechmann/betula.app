//! Text folding for every search of the app, so „okologie" finds „Ökologie" and „strasse"
//! finds „Straße" on the server and in the browser alike; and the search of the module catalog.
//!
//! The catalog compares the words of a query with the names of a module that Radix folded the
//! same way (`v_module_folded`, docs/radix/schema-v2.md „Search"): its titles, their initials and its
//! abbreviations, not the texts of its description (owner, 2026-09-30). Every word has to be
//! found, in any order, and fillers like „für" do not count. How a word is found ranks the
//! module, best first: as its number, as one of its abbreviations („AuP"), as a word of a title
//! (its first word counts a little more), as the start of a module number, as the start of a
//! word, as the initials of words that follow each other („ti", Theoretische Informatik), and,
//! from four letters on, inside a word („netz" in „Stromnetze", but „ki" not in
//! „Schlüsselqualifikationen"). A number of one or two digits and a Roman numeral up to ten are
//! found as a whole word of a title only, the one as the other („Analysis 1" finds „Analysis I",
//! and „Analysis I" not „Analysis für Informatiker"). Where the words as typed find no
//! module at all, `resolve` corrects their typos from the words of the titles, and where that
//! finds none either, the modules with the most of the words are listed.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod fuzzy;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use folia_model::db::{fetch_count, Database, DbError, Value};
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

/// Words a query leaves out and initials skip („Algorithmen und Datenstrukturen" is „ad"). Radix
/// skips the same in the initials it folds (`normalize.SearchFillers`, held to this list by
/// radix/internal/normalize/testdata/search.tsv).
pub const FILLERS: &[&str] = &["of", "and", "und", "der", "die", "das", "in", "im", "fur", "the", "zur", "zum", "von", "mit"];

/// The words of a text as the search sees them: folded, and parted at every character that is
/// neither a letter nor a digit („Python-Programmierung" is python and programmierung), as Radix
/// parts the titles (`normalize.SearchWords`).
pub fn words(text: &str) -> Vec<String> {
    fold(text).split(|c: char| !c.is_alphanumeric()).filter(|word| !word.is_empty()).map(str::to_string).collect()
}

/// The words a query searches for: its words without the fillers (a query of fillers alone keeps
/// them), each once.
fn query_words(text: &str) -> Vec<String> {
    let all = words(text);
    let content: Vec<String> = all.iter().filter(|word| !FILLERS.contains(&word.as_str())).cloned().collect();
    let mut chosen = if content.is_empty() { all } else { content };
    let mut seen = HashSet::new();
    chosen.retain(|word| seen.insert(word.clone()));
    chosen
}

// How a word is found in a module, best first; a module's score is the sum over the words.
const ID: i64 = 1200;
const ABBREV: i64 = 1100;
const FIRST_WORD: i64 = 1050;
const WHOLE_WORD: i64 = 1000;
const ID_START: i64 = 900;
const FIRST_WORD_START: i64 = 850;
const WORD_START: i64 = 800;
const INITIALS: i64 = 600;
const WORD_PART: i64 = 500;
/// A word is found inside another from this many letters on.
const PART_FROM: usize = 4;
/// A number is (the start of) a module number from this many digits on; a shorter one is a word
/// of a title („Mathematik 1").
const NUMBER_FROM: usize = 3;
/// The numerals a number up to ten is also found as, and the other way round.
const ROMAN: [&str; 10] = ["i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x"];

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

/// What the SQL of a search asks: for every word the forms it may be found as.
pub struct Plan {
    words: Vec<Vec<Form>>,
    most_words: bool,
}

struct Form {
    text: String,
    /// Only as a whole word: a short number, or a numeral standing in for a number.
    whole: bool,
}

impl Plan {
    /// The plan of `text` as `resolution` has it searched (as typed without one); `None` for a
    /// blank text, which is no search.
    pub fn new(text: &str, resolution: Option<&Resolution>) -> Option<Self> {
        if text.trim().is_empty() {
            return None;
        }
        let words = query_words(text)
            .into_iter()
            .map(|typed| {
                let corrected = resolution.and_then(|r| r.corrected.iter().find(|c| c.typed == typed));
                forms(corrected.map_or(typed, |c| c.word.clone()))
            })
            .collect();
        Some(Self { words, most_words: resolution.is_some_and(|r| r.most_words) })
    }

    /// How many words the plan searches for.
    pub fn len(&self) -> usize {
        self.words.len()
    }

    /// Whether the plan searches for no word at all.
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// The matches of every module, `(SELECT module_id, score, matched, w0, w1 … FROM …)`, with
    /// its parameters: `wN` is how well word N is found (0: not at all), `matched` how many of the
    /// words are found, `score` the sum. `None` for a text without a word, which finds nothing.
    pub fn table(&self) -> Option<(String, Vec<Value>)> {
        if self.words.is_empty() {
            return None;
        }
        let mut params = Vec::new();
        let mut columns = Vec::with_capacity(self.words.len());
        for (i, forms) in self.words.iter().enumerate() {
            let cases: Vec<String> = forms.iter().map(|form| form.case(&mut params)).collect();
            let score = if cases.len() == 1 { cases.concat() } else { format!("MAX({})", cases.join(", ")) };
            columns.push(format!("{score} AS w{i}"));
        }
        // Only a module whose names contain a form of the words at all is scored: that is what
        // every way of finding a form needs, and it spares the scoring of nearly every module (in
        // sql.js on a phone, the difference between a list that follows the typing and one that
        // lags behind it).
        let mut prefilter = Vec::with_capacity(self.words.len());
        for forms in &self.words {
            let found: Vec<&str> = forms
                .iter()
                .map(|form| {
                    params.extend(std::iter::repeat_n(Value::from(form.text.as_str()), 5));
                    "instr(title_de, ?) > 0 OR instr(title_en, ?) > 0 OR instr(abbrevs, ?) > 0 OR instr(initials, ?) > 0 \
                     OR instr(module_id, ?) > 0"
                })
                .collect();
            prefilter.push(format!("({})", found.join(" OR ")));
        }
        let names: Vec<String> = (0..self.words.len()).map(|i| format!("w{i}")).collect();
        let matched: Vec<String> = names.iter().map(|name| format!("({name} > 0)")).collect();
        let sql = format!(
            "(SELECT module_id, {} AS score, {} AS matched, {} FROM (SELECT module_id, {} FROM \
             (SELECT module_id, ' ' || IFNULL(title_de, '') || ' ' AS de, ' ' || IFNULL(title_en, '') || ' ' AS en, \
             ' ' || IFNULL(initials, '') || ' ' AS ini, ' ' || IFNULL(abbrevs, '') || ' ' AS ab FROM v_module_folded \
             WHERE {})))",
            names.join(" + "),
            matched.join(" + "),
            names.join(", "),
            columns.join(", "),
            prefilter.join(if self.most_words { " OR " } else { " AND " })
        );
        Some((sql, params))
    }

    /// The condition on `table` joined as `alias`: every word found, or, for the most words, one
    /// of them; nothing for a text without a word.
    pub fn condition(&self, alias: &str) -> String {
        match (self.words.len(), self.most_words) {
            (0, _) => "0".to_string(),
            (_, true) => format!("{alias}.matched > 0"),
            (n, false) => format!("{alias}.matched = {n}"),
        }
    }
}

/// The forms a word may be found as: itself, and a number up to ten also as its Roman numeral,
/// and the other way round. A number of up to two digits and a numeral are found as a whole word
/// only.
fn forms(word: String) -> Vec<Form> {
    let whole = (word.chars().all(|c| c.is_ascii_digit()) && word.chars().count() < NUMBER_FROM) || ROMAN.contains(&word.as_str());
    let mut others = Vec::new();
    if let Some(roman) = word.parse::<usize>().ok().and_then(|n| n.checked_sub(1)).and_then(|i| ROMAN.get(i)) {
        others.push(Form { text: roman.to_string(), whole: true });
    }
    if let Some(i) = ROMAN.iter().position(|roman| *roman == word) {
        others.push(Form { text: (i + 1).to_string(), whole: true });
    }
    let mut forms = vec![Form { text: word, whole }];
    forms.extend(others);
    forms
}

impl Form {
    /// How well the form is found in a module: a `CASE` over the columns of `Plan::table`, its
    /// parameters appended to `params`.
    fn case(&self, params: &mut Vec<Value>) -> String {
        let text = self.text.as_str();
        let length = text.chars().count();
        let number = text.chars().all(|c| c.is_ascii_digit());
        let letters = text.chars().all(char::is_alphabetic);
        // (condition, how often it binds the form, score)
        let mut arms: Vec<(&str, usize, i64)> = Vec::new();
        if number && length >= NUMBER_FROM {
            arms.push(("module_id = ?", 1, ID));
        }
        if length >= 2 {
            arms.push(("instr(ab, ' ' || ? || ' ') > 0", 1, ABBREV));
        }
        arms.push(("instr(de, ' ' || ? || ' ') = 1 OR instr(en, ' ' || ? || ' ') = 1", 2, FIRST_WORD));
        arms.push(("instr(de, ' ' || ? || ' ') > 0 OR instr(en, ' ' || ? || ' ') > 0", 2, WHOLE_WORD));
        if number && length >= NUMBER_FROM {
            arms.push(("instr(module_id, ?) = 1", 1, ID_START));
        }
        if !self.whole {
            arms.push(("instr(de, ' ' || ?) = 1 OR instr(en, ' ' || ?) = 1", 2, FIRST_WORD_START));
            arms.push(("instr(de, ' ' || ?) > 0 OR instr(en, ' ' || ?) > 0", 2, WORD_START));
            if letters && length >= 2 {
                arms.push(("instr(ini, ?) > 0", 1, INITIALS));
            }
            if length >= PART_FROM {
                arms.push(("instr(de, ?) > 0 OR instr(en, ?) > 0", 2, WORD_PART));
            }
        }
        let mut case = String::from("CASE");
        for (condition, uses, score) in arms {
            case.push_str(&format!(" WHEN {condition} THEN {score}"));
            params.extend(std::iter::repeat_n(Value::from(text), uses));
        }
        case.push_str(" ELSE 0 END");
        case
    }
}

/// What the catalog searches `text` for (`pages::catalog`): the text as typed where some module
/// has all its words; else with every word no module has corrected from the words of the titles,
/// where that finds a module; else, for two words and more, the modules with the most of them.
/// A text that finds nothing in any of these ways is searched as typed. Filters play no part:
/// what the text finds outside them, the list says below its rows.
pub fn resolve(db: &dyn Database, text: &str) -> Result<Resolution, DbError> {
    let typed = query_words(text);
    if typed.is_empty() || search_count(db, text, None)? > 0 {
        return Ok(Resolution::default());
    }
    let found = search_words_found(db, text)?;
    let mut corrected = Vec::new();
    if found.iter().any(|found| !found) {
        let vocabulary = vocabulary(db)?;
        for (word, found) in typed.iter().zip(&found) {
            if let (false, Some((instead, shown))) = (*found, vocabulary.correct(word)) {
                corrected.push(Correction { typed: word.clone(), word: instead, shown });
            }
        }
    }
    if !corrected.is_empty() {
        let resolution = Resolution { corrected: corrected.clone(), most_words: false };
        if search_count(db, text, Some(&resolution))? > 0 {
            return Ok(resolution);
        }
    }
    if typed.len() > 1 {
        let resolution = Resolution { corrected, most_words: true };
        if search_count(db, text, Some(&resolution))? > 0 {
            return Ok(resolution);
        }
    }
    Ok(Resolution::default())
}

/// The vocabulary of the titles of the snapshot `db` is (its `content_digest`), made once for it:
/// a search that types a word no title has would otherwise make it again with every letter (the
/// titles of 4,938 modules, 40 ms of a laptop and four times that of a phone). A snapshot that
/// does not say its digest gets one made for the search alone.
fn vocabulary(db: &dyn Database) -> Result<Arc<Vocabulary>, DbError> {
    static KEPT: Mutex<Option<(String, Arc<Vocabulary>)>> = Mutex::new(None);
    let digest = folia_model::rows::meta(db)?.content_digest;
    let kept = |digest: &str| KEPT.lock().ok()?.as_ref().filter(|(of, _)| of == digest).map(|(_, vocabulary)| vocabulary.clone());
    if let Some(vocabulary) = digest.as_deref().and_then(kept) {
        return Ok(vocabulary);
    }
    let vocabulary = Arc::new(Vocabulary::new(&search_titles(db)?));
    if let (Some(digest), Ok(mut kept)) = (digest, KEPT.lock()) {
        *kept = Some((digest, vocabulary.clone()));
    }
    Ok(vocabulary)
}

/// The words of the titles, for the correction of a typo: every word with the number of modules
/// that have it, and as the first of them writes it.
struct Vocabulary {
    words: Vec<(String, u32, String)>,
}

impl Vocabulary {
    fn new(titles: &[Titles]) -> Self {
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
    fn correct(&self, word: &str) -> Option<(String, String)> {
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
}

// The statements of the search: the only SQL of this crate besides `Plan`'s.

/// How many modules the search of `text` finds, as `resolution` has it searched, whatever the
/// filters: the whole catalog, offered or not (`search::resolve`).
pub fn search_count(db: &dyn Database, text: &str, resolution: Option<&Resolution>) -> Result<u64, DbError> {
    let Some(plan) = Plan::new(text, resolution) else { return Ok(0) };
    let Some((table, params)) = plan.table() else { return Ok(0) };
    fetch_count(db, "search_count", &format!("SELECT COUNT(*) FROM {table} sr WHERE {}", plan.condition("sr")), &params)
}

/// Which of the words of `text` some module has, in the order the search takes them
/// (`search::resolve`): a word no module has is one to correct.
pub fn search_words_found(db: &dyn Database, text: &str) -> Result<Vec<bool>, DbError> {
    // Every module that has one of the words, as for the most words.
    let any = Resolution { corrected: Vec::new(), most_words: true };
    let Some(plan) = Plan::new(text, Some(&any)) else { return Ok(Vec::new()) };
    let Some((table, params)) = plan.table() else { return Ok(Vec::new()) };
    let columns: Vec<String> = (0..plan.len()).map(|i| format!("MAX(w{i} > 0)")).collect();
    let rows = db.query("search_words_found", &format!("SELECT {} FROM {table}", columns.join(", ")), &params)?;
    let found = |value: &Value| matches!(value, Value::Integer(n) if *n > 0);
    Ok(rows.rows.first().map(|row| row.iter().map(found).collect()).unwrap_or_default())
}

/// The German and the English title of a module.
pub type Titles = (Option<String>, Option<String>);

/// The titles of every module: the words a typo is corrected to (`search::resolve`).
pub fn search_titles(db: &dyn Database) -> Result<Vec<Titles>, DbError> {
    let rows = db.query("search_titles", "SELECT title_de, title_en FROM v_module ORDER BY id", &[])?;
    let text = |value: Option<&Value>| match value {
        Some(Value::Text(text)) => Some(text.clone()),
        _ => None,
    };
    Ok(rows.rows.iter().map(|row| (text(row.first()), text(row.get(1)))).collect())
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
    fn a_query_is_its_words_without_fillers() {
        assert_eq!(query_words("Mathe für Informatiker"), vec!["mathe", "informatiker"]);
        assert_eq!(query_words("  Einführung in die  Python-Programmierung "), vec!["einfuhrung", "python", "programmierung"]);
        assert_eq!(query_words("in der"), vec!["in", "der"], "fillers alone are searched");
        assert_eq!(query_words("Algebra algebra"), vec!["algebra"]);
        assert!(query_words("%_\\").is_empty());
    }

    #[test]
    fn numbers_and_numerals_are_found_as_each_other() {
        let texts = |word: &str| forms(word.to_string()).into_iter().map(|form| (form.text, form.whole)).collect::<Vec<_>>();
        assert_eq!(texts("1"), vec![("1".to_string(), true), ("i".to_string(), true)]);
        assert_eq!(texts("ii"), vec![("ii".to_string(), true), ("2".to_string(), true)]);
        assert_eq!(texts("iii2"), vec![("iii2".to_string(), false)]);
        assert_eq!(texts("10"), vec![("10".to_string(), true), ("x".to_string(), true)]);
        assert_eq!(texts("11"), vec![("11".to_string(), true)]);
        assert_eq!(texts("118"), vec![("118".to_string(), false)]);
        assert_eq!(texts("0"), vec![("0".to_string(), true)]);
    }

    #[test]
    fn a_text_without_a_word_finds_nothing() {
        let plan = Plan::new("%", None).unwrap();
        assert!(plan.table().is_none());
        assert_eq!(plan.condition("sr"), "0");
        assert!(Plan::new("   ", None).is_none());
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
    }
}
