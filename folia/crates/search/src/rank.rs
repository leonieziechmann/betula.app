//! The catalog's search (owner, 2026-10-19; folia/design/suche/konzept.html, whose prototype
//! `engine.mjs` this follows rule for rule): which modules a query finds and in which order.
//!
//! A query is found in the names of a module — its number, its abbreviations, its two titles —
//! and in the names of its lecturers, never in the texts of its description. Every word of the
//! query gets the points of its best find (`points`): the whole module named, a whole word or a
//! short form („TI" for Theoretische Informatik), the start of a word, a part inside a word, a
//! lecturer's name; changed by where the find stands (the other language's title counts less,
//! capitals as typed count more, every word before it a little less). A module has the points of
//! its weakest word, and of „the whole module named" where the words name every word of a title.
//!
//! What a query finds depends on the query alone („die gefundenen Module bei allen gleich"): no
//! limit of points, and the typos are corrected against the whole catalog. The program of the
//! visitor's profile raises the points of its modules, which changes the order only.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use folia_model::db::{Database, DbError};
use folia_model::rows::{MarkedText, SearchMarks};

use crate::{fold, fuzzy, Resolution, Vocabulary, ROMAN};

/// The points of a find, and what changes them (folia/design/suche/konzept.html, „Punkte").
pub mod points {
    /// The query names the whole module: its number, …
    pub const NUMBER: f64 = 125.0;
    /// … an abbreviation of its own or of a program („ThI"), …
    pub const ABBREV: f64 = 125.0;
    /// … the abbreviation of a part without its number („BS" for BS1), …
    pub const ABBREV_PART: f64 = 120.0;
    /// … or every word of a title but fillers and numerals („TI", „theo inf", „mathe 1").
    pub const TITLE: f64 = 120.0;
    /// A whole word of a title.
    pub const WORD: f64 = 80.0;
    /// The short form of words that follow each other („KI" in „Einführung in die Künstliche
    /// Intelligenz").
    pub const SHORT: f64 = 80.0;
    /// The start of a word.
    pub const PREFIX: f64 = 60.0;
    /// A short form that leaves out a filler („TI" for Technologie- und Innovationsmanagement).
    pub const SHORT_SKIP: f64 = 55.0;
    /// The start of a module number, from three digits on.
    pub const NUMBER_START: f64 = 60.0;
    /// Inside a word, from four letters on.
    pub const INSIDE: f64 = 35.0;
    /// A lecturer's surname whole (with the first name too).
    pub const NAME: f64 = 75.0;
    /// The start of a surname, from four letters on.
    pub const NAME_START: f64 = 55.0;
    /// A surname written alike, from four letters on.
    pub const NAME_LIKE: f64 = 50.0;
    /// A find in the title of the other language, which the list does not show.
    pub const OTHER: f64 = 0.75;
    /// Capitals as typed, where the query has capitals („TI" in „TIN*", not in „Tissue").
    pub const CASE: f64 = 1.1;
    /// Every word before the find in its title, …
    pub const PER_WORD: f64 = 0.03;
    /// … up to this many.
    pub const MAX_WORDS: usize = 5;
    /// The words of the query side by side, in their order.
    pub const PHRASE: f64 = 1.05;
    /// A lecturer who teaches the module, not responsible for it.
    pub const INSTRUCTOR: f64 = 0.9;
    /// A module of the visitor's program: the order only (owner, 2026-10-19: „Vlt score einfach x
    /// 1.2 oder so (Da muss man nochmal gucken, welche werte da gut sind)"). Measured, ×1.2 was too
    /// little: a module of one's own named in the other language (90–99 points) has to come before
    /// another's named in the language shown (120–132), or „Masterarbeit" lists one's own „Master
    /// Thesis" behind the „Master-Arbeit" of 32 other programs. Not before one named by its own
    /// abbreviation as typed (137.5): with Informatik M.Sc. „SE" is „Sicherheitsengineering" (SE),
    /// then „Softwaretechnik".
    pub const PROGRAM: f64 = 1.35;
}

/// Fillers, for the query and for the titles: a query leaves them out, a short form may take their
/// letter („AuD", „MfI") or leave them out, and they end a group of words. „Prof." and „Dr." too,
/// so that „Prof. Hofstedt" is Hofstedt.
pub const FILLERS: &[&str] = &[
    "und", "oder", "sowie", "der", "die", "das", "des", "dem", "den", "ein", "eine", "einer", "eines", "einem", "einen",
    "in", "im", "am", "an", "auf", "aus", "bei", "fur", "mit", "nach", "uber", "unter", "von", "vom", "zu", "zur", "zum",
    "als", "durch", "ist", "sind", "ich", "wir", "wie", "was", "kann", "auch", "ihr", "ihre", "ihrer", "ihren", "sein",
    "seine", "seiner", "seinen", "zwischen", "gegen", "ohne", "um", "vor", "bis", "innerhalb", "ausserhalb",
    "of", "and", "or", "the", "to", "for", "from", "with", "by", "into", "on", "at", "as", "s", "is", "are", "its",
    "their", "our", "your", "this", "these", "that", "how", "what", "may", "can", "have", "has", "between", "within",
    "without", "about", "across", "through", "towards", "under", "over", "after", "before", "beyond", "among", "per",
    "via", "vs", "versus", "prof", "dr",
];

/// Short forms everybody knows (Radix's `abbrev.knownForms`): a word that is or contains the long
/// form is found by the short one too.
const KNOWN: &[(&str, &str)] =
    &[("bwl", "betriebswirtschaftslehre"), ("vwl", "volkswirtschaftslehre"), ("sw", "software"), ("hw", "hardware")];

/// Parts of a lecturer's name that are no name: titles, and the particles of a surname.
const TITLE_WORDS: &[&str] = &[
    "prof", "dr", "rer", "nat", "habil", "ing", "phil", "med", "medic", "oec", "pol", "jur", "techn", "paed", "apl", "pd", "dipl",
    "mult", "h", "c", "hc", "sc", "soc", "publ", "theol", "agr", "des", "univ", "em",
];
const PARTICLES: &[&str] = &["von", "van", "de", "der", "den", "del", "della", "la", "le", "zu", "zur", "di", "da", "dos", "du", "ten", "ter", "vom"];

fn is_filler(word: &str) -> bool {
    FILLERS.contains(&word)
}

fn is_digits(word: &[char]) -> bool {
    !word.is_empty() && word.iter().all(char::is_ascii_digit)
}

fn is_letters(word: &[char]) -> bool {
    !word.is_empty() && word.iter().all(|c| c.is_alphabetic())
}

fn is_roman(word: &[char]) -> bool {
    ROMAN.iter().any(|roman| roman.chars().eq(word.iter().copied()))
}

fn is_numeral(word: &[char]) -> bool {
    is_roman(word) || word.iter().any(char::is_ascii_digit)
}

fn starts_with(word: &[char], start: &[char]) -> bool {
    word.len() >= start.len() && word.iter().zip(start).all(|(a, b)| a == b)
}

/// Where `part` first stands in `word`.
fn find(word: &[char], part: &[char]) -> Option<usize> {
    if part.is_empty() || part.len() > word.len() {
        return None;
    }
    (0..=word.len() - part.len()).find(|&i| word.get(i..i + part.len()) == Some(part))
}

fn has_capital(text: &str) -> bool {
    text.chars().any(char::is_uppercase)
}

/// The forms a word of a query is found as: itself, and a number up to ten also as its Roman
/// numeral and the other way round.
fn forms(word: &[char]) -> Vec<Vec<char>> {
    let mut forms = vec![word.to_vec()];
    let text: String = word.iter().collect();
    if is_digits(word) {
        if let Some(roman) = text.parse::<usize>().ok().and_then(|n| n.checked_sub(1)).and_then(|i| ROMAN.get(i)) {
            forms.push(roman.chars().collect());
        }
    }
    if let Some(i) = ROMAN.iter().position(|roman| *roman == text) {
        forms.push((i + 1).to_string().chars().collect());
    }
    forms
}

// ---------------------------------------------------------------- words of a name

/// What lies before a word: a space („Kinder- und" too), the inside of a word written in parts
/// („Real-Time", „B.Sc."), a weak mark („/", „&", „+") or any other mark („:", „,", „(", „ - ").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gap {
    None,
    Compound,
    Weak,
    Strong,
}

fn gap_of(gap: &str, first: bool) -> Gap {
    if first {
        return if gap.trim().is_empty() { Gap::None } else { Gap::Strong };
    }
    let all = |f: fn(char) -> bool| !gap.is_empty() && gap.chars().all(f);
    let hyphen_then_spaces = gap.strip_prefix('-').is_some_and(|rest| !rest.is_empty() && rest.chars().all(char::is_whitespace));
    if all(char::is_whitespace) || hyphen_then_spaces {
        return Gap::None;
    }
    if all(|c| matches!(c, '-' | '.' | '\'' | '’')) {
        return Gap::Compound;
    }
    fn weak_mark(c: char) -> bool {
        matches!(c, '&' | '+' | '/')
    }
    let hyphen_then_mark = gap.strip_prefix('-').is_some_and(|rest| {
        let rest = rest.trim();
        rest.chars().count() == 1 && rest.chars().all(weak_mark)
    });
    if all(|c| c.is_whitespace() || matches!(c, '&' | '+' | '/')) || hyphen_then_mark {
        return Gap::Weak;
    }
    Gap::Strong
}

/// A word of a text: folded, with where it stands in the text (byte offsets) and where each of
/// its folded letters comes from, for the marks.
#[derive(Clone, Debug)]
struct Word {
    text: Vec<char>,
    start: usize,
    end: usize,
    map: Vec<usize>,
    gap: Gap,
    filler: bool,
    numeral: bool,
    content: bool,
    /// How many words that are no filler and no numeral stand before it.
    pos: usize,
}

impl Word {
    /// The byte range of its first `len` folded letters from letter `from` on.
    fn span(&self, from: usize, len: usize) -> (usize, usize) {
        let start = self.map.get(from).copied().unwrap_or(self.end);
        let end = self.map.get(from + len).copied().unwrap_or(self.end);
        (start, end)
    }
}

/// A word written in parts, as one word („E-Technik" is etechnik, „Master-Arbeit" masterarbeit).
#[derive(Clone, Debug)]
struct Joined {
    text: Vec<char>,
    start: usize,
    end: usize,
    map: Vec<usize>,
    from: usize,
    to: usize,
}

/// A title (or a name) in words.
#[derive(Clone, Debug)]
pub(crate) struct Text {
    raw: String,
    words: Vec<Word>,
    joined: Vec<Joined>,
}

impl Text {
    fn new(raw: &str) -> Self {
        let mut words: Vec<Word> = Vec::new();
        let mut current: Option<(usize, Vec<char>, Vec<usize>)> = None;
        let mut end = 0;
        let close = |current: &mut Option<(usize, Vec<char>, Vec<usize>)>, end: usize, words: &mut Vec<Word>| {
            if let Some((start, text, map)) = current.take() {
                words.push(Word { text, start, end, map, gap: Gap::None, filler: false, numeral: false, content: false, pos: 0 });
            }
        };
        for (at, c) in raw.char_indices() {
            if c.is_alphanumeric() {
                let (_, text, map) = current.get_or_insert_with(|| (at, Vec::new(), Vec::new()));
                for folded in fold(c.encode_utf8(&mut [0; 4])).chars() {
                    text.push(folded);
                    map.push(at);
                }
                end = at + c.len_utf8();
            } else {
                close(&mut current, end, &mut words);
            }
        }
        close(&mut current, end, &mut words);
        let mut content = 0;
        let mut before = 0;
        for (i, word) in words.iter_mut().enumerate() {
            word.gap = gap_of(raw.get(before..word.start).unwrap_or(""), i == 0);
            before = word.end;
            let text: String = word.text.iter().collect();
            word.filler = is_filler(&text);
            word.numeral = is_numeral(&word.text);
            word.content = !word.filler && !word.numeral;
            word.pos = content;
            if word.content {
                content += 1;
            }
        }
        let mut joined = Vec::new();
        let mut i = 0;
        while i < words.len() {
            let mut j = i + 1;
            while words.get(j).is_some_and(|word| word.gap == Gap::Compound) {
                j += 1;
            }
            if j - i > 1 {
                let parts = words.get(i..j).unwrap_or(&[]);
                joined.push(Joined {
                    text: parts.iter().flat_map(|part| part.text.iter().copied()).collect(),
                    start: parts.first().map_or(0, |part| part.start),
                    end: parts.last().map_or(0, |part| part.end),
                    map: parts.iter().flat_map(|part| part.map.iter().copied()).collect(),
                    from: i,
                    to: j - 1,
                });
            }
            i = j;
        }
        Self { raw: raw.to_string(), words, joined }
    }

    fn word(&self, i: usize) -> Option<&Word> {
        self.words.get(i)
    }

    /// Whether word `i` is written in capitals only, two at least („DBS", „IT").
    fn in_capitals(&self, i: usize) -> bool {
        self.word(i).and_then(|word| self.raw.get(word.start..word.end)).is_some_and(|raw| {
            raw.chars().count() >= 2 && raw.chars().all(char::is_uppercase)
        })
    }

    // A short form names the end of a group of words: it ends at the title's end, a filler, a
    // numeral or a mark („Explainable Machine Learning" is ML, „Power Plant Technology" no PP). In
    // the title one sees it may begin anywhere but inside a word written in parts; in the other
    // one only where a group begins too.
    fn starts_group(&self, i: usize, strict: bool) -> bool {
        let gap = self.word(i).map_or(Gap::None, |word| word.gap);
        if !strict {
            return gap != Gap::Compound;
        }
        i == 0 || matches!(gap, Gap::Strong | Gap::Weak) || !self.word(i - 1).is_some_and(|word| word.content)
    }

    fn ends_group(&self, i: usize) -> bool {
        match self.word(i + 1) {
            None => true,
            Some(next) => matches!(next.gap, Gap::Strong | Gap::Weak) || !next.content,
        }
    }
}

// ---------------------------------------------------------------- the index

/// A lecturer's name in words: the surname, the first names.
#[derive(Debug)]
struct Name {
    text: Text,
    surname: Vec<usize>,
    first: Vec<usize>,
}

impl Name {
    fn new(raw: &str) -> Self {
        let comma = raw.find(',');
        let text = Text::new(raw);
        let mut surname = Vec::new();
        let mut first = Vec::new();
        for (i, word) in text.words.iter().enumerate() {
            let folded: String = word.text.iter().collect();
            let title = TITLE_WORDS.contains(&folded.as_str());
            match comma {
                Some(comma) if word.start > comma => {
                    if !title {
                        first.push(i);
                    }
                }
                _ => {
                    if !title && !PARTICLES.contains(&folded.as_str()) && !word.text.iter().any(char::is_ascii_digit) {
                        surname.push(i);
                    }
                }
            }
        }
        Self { text, surname, first }
    }
}

#[derive(Debug)]
struct Abbrev {
    raw: String,
    folded: Vec<char>,
}

#[derive(Debug)]
struct Module {
    id: String,
    /// 0 offered, 1 phase-out, 2 no longer offered.
    status: u8,
    /// The programs (their newest regulations) whose curriculum has it.
    programs: u32,
    shown: Text,
    other: Option<Text>,
    abbrevs: Vec<Abbrev>,
    /// The index of a name in `Index::names`, and whether that lecturer only teaches it.
    lecturers: Vec<(usize, bool)>,
}

/// What the search knows of a snapshot, made once for it.
#[derive(Debug)]
pub(crate) struct Index {
    modules: Vec<Module>,
    names: Vec<Name>,
    vocabulary: Vocabulary,
}

impl Index {
    fn read(db: &dyn Database) -> Result<Self, DbError> {
        let mut abbrevs: HashMap<String, Vec<String>> = HashMap::new();
        for (id, abbrev) in crate::search_abbrevs(db)? {
            abbrevs.entry(id).or_default().push(abbrev);
        }
        let programs: HashMap<String, u32> = crate::search_programs(db)?.into_iter().collect();
        let mut names: Vec<Name> = Vec::new();
        let mut name_index: HashMap<String, usize> = HashMap::new();
        let mut lecturers: HashMap<String, Vec<(usize, bool)>> = HashMap::new();
        for lecturer in crate::search_lecturers(db)? {
            let i = *name_index.entry(lecturer.name.clone()).or_insert_with(|| {
                names.push(Name::new(&lecturer.name));
                names.len() - 1
            });
            let teaching = !lecturer.responsible;
            let list = lecturers.entry(lecturer.module_id).or_default();
            match list.iter_mut().find(|(n, _)| *n == i) {
                Some(entry) => entry.1 = entry.1 && teaching,
                None => list.push((i, teaching)),
            }
        }
        let mut modules = Vec::new();
        let mut titles = Vec::new();
        for row in crate::search_modules(db)? {
            let other = [&row.title_de, &row.title_en].into_iter().flatten().find(|t| **t != row.title).cloned();
            let status = match row.offer_status.as_deref() {
                Some("active") => 0,
                Some("phase_out") => 1,
                _ => 2,
            };
            let mut own: Vec<String> = row.abbrev.into_iter().chain(abbrevs.remove(&row.id).unwrap_or_default()).collect();
            own.sort();
            own.dedup();
            let mut list = lecturers.remove(&row.id).unwrap_or_default();
            list.sort_by_key(|(i, teaching)| (*teaching, *i));
            modules.push(Module {
                programs: programs.get(&row.id).copied().unwrap_or(0),
                abbrevs: own
                    .into_iter()
                    .map(|raw| Abbrev { folded: crate::words(&raw).concat().chars().collect(), raw })
                    .filter(|abbrev| !abbrev.folded.is_empty())
                    .collect(),
                lecturers: list,
                shown: Text::new(&row.title),
                other: other.as_deref().map(Text::new),
                status,
                id: row.id,
            });
            titles.push((row.title_de, row.title_en));
        }
        Ok(Self { modules, names, vocabulary: Vocabulary::new(&titles) })
    }
}

/// The index of the snapshot `db` is (its `content_digest`), made once for it. A snapshot that
/// does not say its digest gets one made for the search alone.
pub(crate) fn index(db: &dyn Database) -> Result<Arc<Index>, DbError> {
    static KEPT: Mutex<Option<(String, Arc<Index>)>> = Mutex::new(None);
    let digest = folia_model::rows::meta(db)?.content_digest;
    let kept = |digest: &str| KEPT.lock().ok()?.as_ref().filter(|(of, _)| of == digest).map(|(_, index)| index.clone());
    if let Some(index) = digest.as_deref().and_then(kept) {
        return Ok(index);
    }
    let index = Arc::new(Index::read(db)?);
    if let (Some(digest), Ok(mut kept)) = (digest, KEPT.lock()) {
        *kept = Some((digest, index.clone()));
    }
    Ok(index)
}

// ---------------------------------------------------------------- the query

/// A word of the query.
#[derive(Clone, Debug)]
pub(crate) struct QueryWord {
    /// Folded; corrected where the resolution corrects it.
    pub(crate) q: Vec<char>,
    /// As typed (as the title writes the correction).
    raw: String,
    /// Read as a short form too: one or two letters, capitals inside or only capitals („TI",
    /// „AuD", „TheoInf", „LinA"), or no word of a title starts with it („theoinf").
    strong: bool,
}

/// The words of a query (fillers left out, a query of fillers alone keeps them), each once.
pub(crate) fn query_words(index: &Index, text: &str, resolution: Option<&Resolution>) -> Vec<QueryWord> {
    let mut parts: Vec<(Vec<char>, String)> = Vec::new();
    let mut seen = HashSet::new();
    for raw in text.split(|c: char| !c.is_alphanumeric()).filter(|part| !part.is_empty()) {
        let q = fold(raw);
        if !q.is_empty() && seen.insert(q.clone()) {
            parts.push((q.chars().collect(), raw.to_string()));
        }
    }
    let content: Vec<(Vec<char>, String)> = parts.iter().filter(|(q, _)| !is_filler(&q.iter().collect::<String>())).cloned().collect();
    let chosen = if content.is_empty() { parts } else { content };
    chosen
        .into_iter()
        .map(|(typed, raw)| {
            let typed_text: String = typed.iter().collect();
            let correction = resolution.and_then(|r| r.corrected.iter().find(|c| c.typed == typed_text));
            let (q, raw) = match correction {
                Some(c) => (c.word.chars().collect::<Vec<char>>(), c.shown.clone()),
                None => (typed, raw),
            };
            let capitals = correction.is_none() && raw.chars().skip(1).any(char::is_uppercase);
            let strong = q.len() <= 2 || capitals || (correction.is_none() && !index.vocabulary.starts_some_word(&q.iter().collect::<String>()));
            QueryWord { q, raw, strong }
        })
        .collect()
}

// ---------------------------------------------------------------- finds

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Id,
    IdStart,
    Abbrev,
    AbbrevPart,
    Word,
    Prefix,
    Inside,
    Short,
    Name,
    NameStart,
    NameLike,
    FirstName,
}

/// Where a find is: in the title one sees, in the other one, or neither (a number, an
/// abbreviation, a lecturer).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Field {
    Shown,
    Other,
    Elsewhere,
}

/// One way a word of the query is found in a module.
#[derive(Clone, Debug)]
pub(crate) struct Find {
    pub(crate) kind: Kind,
    pub(crate) points: f64,
    pub(crate) field: Field,
    /// The words of the title it covers (with `field` Shown or Other).
    from: usize,
    to: usize,
    /// Byte ranges in the title, in the lecturer's name or in the module's number.
    pub(crate) spans: Vec<(usize, usize)>,
    case: bool,
    skipped: usize,
    /// A lecturer's index in `Index::names`, and whether only teaching.
    pub(crate) lecturer: Option<(usize, bool)>,
    /// The abbreviations a find by abbreviation stands for, as written.
    pub(crate) abbrev: Option<String>,
}

impl Find {
    fn elsewhere(kind: Kind, points: f64) -> Self {
        Self { kind, points, field: Field::Elsewhere, from: 0, to: 0, spans: Vec::new(), case: false, skipped: 0, lecturer: None, abbrev: None }
    }

    fn in_title(&self) -> bool {
        self.field != Field::Elsewhere
    }

    /// How many words it covers, less one: the narrower find first among equals.
    fn width(&self) -> usize {
        if self.in_title() {
            self.to.saturating_sub(self.from)
        } else {
            0
        }
    }
}

/// Capitals as typed: where the query word has a capital, the letters a find marks are written
/// as typed. `None` where the query has no capital.
fn case_of(word: &QueryWord, raw: &str, spans: &[(usize, usize)]) -> Option<bool> {
    if !has_capital(&word.raw) {
        return None;
    }
    let typed: String = spans.iter().filter_map(|(a, b)| raw.get(*a..*b)).collect();
    Some(typed == word.raw)
}

/// Every way `q` is the short form of words that follow each other: at least two pieces, each the
/// start of a word, none longer than the one before („TheoInf", „LinA", „AuD", „TI"; not „b|au");
/// no word left out but fillers; a numeral never a piece („Technik I" is no „TI").
struct ShortForm {
    pieces: Vec<(usize, usize)>,
    from: usize,
    to: usize,
    skipped: usize,
}

fn short_forms(q: &[char], title: &Text, strict: bool) -> Vec<ShortForm> {
    struct Walk<'a> {
        q: &'a [char],
        title: &'a Text,
        pieces: Vec<(usize, usize)>,
        out: Vec<ShortForm>,
    }
    impl Walk<'_> {
        /// Piece by piece from word `i` on, `off` letters of the query taken.
        fn from(&mut self, i: usize, off: usize, start: usize, skipped: usize) {
            let Some(word) = self.title.word(i) else { return };
            if word.numeral {
                return;
            }
            let before = self.pieces.last().map_or(usize::MAX, |(_, len)| *len);
            let most = (self.q.len() - off).min(word.text.len()).min(before);
            for len in (1..=most).rev() {
                let Some(piece) = self.q.get(off..off + len) else { continue };
                if !starts_with(&word.text, piece) {
                    continue;
                }
                self.pieces.push((i, len));
                if off + len == self.q.len() {
                    if self.pieces.len() >= 2 && word.content && self.title.ends_group(i) {
                        self.out.push(ShortForm { pieces: self.pieces.clone(), from: start, to: i, skipped });
                    }
                } else {
                    // the next word, or past fillers; never past a mark or past a word that is no filler
                    let mut skip = 0;
                    let mut j = i + 1;
                    while let Some(next) = self.title.word(j) {
                        if next.gap == Gap::Strong {
                            break;
                        }
                        let filler = next.filler;
                        self.from(j, off + len, start, skipped + skip);
                        if !filler {
                            break;
                        }
                        skip += 1;
                        j += 1;
                    }
                }
                self.pieces.pop();
            }
        }
    }
    let mut walk = Walk { q, title, pieces: Vec::new(), out: Vec::new() };
    for (s, word) in title.words.iter().enumerate() {
        if word.content && title.starts_group(s, strict) {
            walk.from(s, 0, s, 0);
        }
    }
    walk.out
}

/// Every way one word of the query is found in one title, with its points.
fn finds_in(word: &QueryWord, title: &Text, other: bool) -> Vec<Find> {
    use points::*;
    let q = &word.q;
    let whole = (is_digits(q) && q.len() < 3) || is_roman(q);
    let alts = forms(q);
    let known: Option<Vec<char>> = KNOWN.iter().find(|(short, _)| short.chars().eq(q.iter().copied())).map(|(_, long)| long.chars().collect());
    let field = if other { Field::Other } else { Field::Shown };
    let mut out: Vec<Find> = Vec::new();
    let mut add = |kind: Kind, base: f64, from: usize, to: usize, spans: Vec<(usize, usize)>, skipped: usize| {
        let pos = title.word(from).map_or(0, |w| w.pos);
        let case = if kind == Kind::Inside { None } else { case_of(word, &title.raw, &spans) };
        let points = base * (1.0 - PER_WORD * pos.min(MAX_WORDS) as f64) * (if other { OTHER } else { 1.0 }) * (if case == Some(true) { CASE } else { 1.0 });
        out.push(Find { kind, points, field, from, to, spans, case: case == Some(true), skipped, lecturer: None, abbrev: None });
    };
    for (i, w) in title.words.iter().enumerate() {
        if alts.contains(&w.text) || known.as_ref().is_some_and(|long| find(&w.text, long).is_some()) {
            add(Kind::Word, WORD, i, i, vec![(w.start, w.end)], 0);
        } else if !whole && starts_with(&w.text, q) {
            add(Kind::Prefix, PREFIX, i, i, vec![w.span(0, q.len())], 0);
        } else if !whole && q.len() >= 4 {
            if let Some(at) = find(&w.text, q).filter(|at| *at > 0) {
                add(Kind::Inside, INSIDE, i, i, vec![w.span(at, q.len())], 0);
            }
        }
    }
    for j in &title.joined {
        let span = |from: usize, len: usize| {
            (j.map.get(from).copied().unwrap_or(j.end), j.map.get(from + len).copied().unwrap_or(j.end))
        };
        if j.text == *q {
            add(Kind::Word, WORD, j.from, j.to, vec![(j.start, j.end)], 0);
        } else if !whole && starts_with(&j.text, q) && !title.word(j.from).is_some_and(|w| starts_with(&w.text, q)) {
            add(Kind::Prefix, PREFIX, j.from, j.to, vec![span(0, q.len())], 0);
        }
    }
    if word.strong && !whole && is_letters(q) && q.len() >= 2 {
        for short in short_forms(q, title, other) {
            let spans = short.pieces.iter().filter_map(|(i, len)| title.word(*i).map(|w| w.span(0, *len))).collect();
            add(Kind::Short, if short.skipped > 0 { SHORT_SKIP } else { SHORT }, short.from, short.to, spans, short.skipped);
        }
    }
    // In the title one does not see, a word of one or two letters finds no word's start („ti" not
    // in „Timber"), only a whole word, a short form, or the start of a word in capitals („DB" in
    // „DBS").
    if other && q.len() <= 2 {
        out.retain(|f| match f.kind {
            Kind::Word => true,
            Kind::Short => f.skipped == 0,
            Kind::Prefix => title.in_capitals(f.from),
            _ => false,
        });
    }
    out
}

/// Do the query's words, in their order, name every word of the title but fillers and numerals?
/// One word only as a whole word or a short form („Datenbanken", „TI"), several also as starts
/// („theo inf", „mathe 1"). `Some(true)` with capitals as typed, `Some(false)` without, `None`: no.
fn covers(words: &[QueryWord], title: &Text, all: &[Vec<Find>]) -> Option<bool> {
    let run = |strict_case: bool| -> bool {
        let mut memo: HashMap<(usize, usize), bool> = HashMap::new();
        fn go(k: usize, t: usize, words: &[QueryWord], title: &Text, all: &[Vec<Find>], strict_case: bool, memo: &mut HashMap<(usize, usize), bool>) -> bool {
            if let Some(known) = memo.get(&(k, t)) {
                return *known;
            }
            let mut ok = false;
            if k == words.len() {
                ok = title.words.iter().skip(t).all(|w| !w.content);
            } else {
                if title.word(t).is_some_and(|w| !w.content) {
                    ok = go(k, t + 1, words, title, all, strict_case, memo);
                }
                for f in all.get(k).map(Vec::as_slice).unwrap_or(&[]) {
                    if ok {
                        break;
                    }
                    if f.from != t || f.kind == Kind::Inside || (f.kind == Kind::Prefix && words.len() == 1) || (f.kind == Kind::Short && f.skipped > 0) {
                        continue;
                    }
                    if strict_case && !f.case {
                        continue;
                    }
                    ok = go(k + 1, f.to + 1, words, title, all, strict_case, memo);
                }
            }
            memo.insert((k, t), ok);
            ok
        }
        go(0, 0, words, title, all, strict_case, &mut memo)
    };
    if words.iter().any(|w| has_capital(&w.raw)) && run(true) {
        return Some(true);
    }
    run(false).then_some(false)
}

// ---------------------------------------------------------------- lecturers
// Restricted, but forgiving with spelling: a surname whole (from three letters), its start from
// four, or a similar spelling from four (one slip, two from eight letters, the first letter right,
// after ä/ae, dt/tt/t, ph/f, th/t, ck/k, tz/z, ie/i, y/i, ei/ai/ey/ay, a silent h and doubled
// letters are made alike); a first name only with the same person's surname. Nothing inside a
// word, no short forms.

/// Spellings made alike, for similar names only.
pub fn alike(word: &str) -> String {
    let mut s = word.replace("ae", "a").replace("oe", "o").replace("ue", "u");
    s = s.replace("dt", "t").replace("th", "t").replace("ph", "f").replace("ck", "k").replace("tz", "z");
    s = s.replace("ie", "i").replace('y', "i");
    let chars: Vec<char> = s.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        if matches!(c, 'e' | 'a') && chars.get(i + 1) == Some(&'i') {
            out.extend(['a', 'i']);
            i += 2;
        } else {
            out.push(c);
            i += 1;
        }
    }
    let mut silent: Vec<char> = Vec::with_capacity(out.len());
    let mut i = 0;
    while let Some(&c) = out.get(i) {
        silent.push(c);
        if matches!(c, 'a' | 'e' | 'i' | 'o' | 'u') && out.get(i + 1) == Some(&'h') {
            i += 2;
        } else {
            i += 1;
        }
    }
    silent.dedup();
    silent.into_iter().collect()
}

fn similar_name(q: &[char], name: &[char]) -> bool {
    if q.len() < 4 || q.first() != name.first() {
        return false;
    }
    let a: Vec<char> = alike(&q.iter().collect::<String>()).chars().collect();
    let b: Vec<char> = alike(&name.iter().collect::<String>()).chars().collect();
    if a.first() != b.first() {
        return false;
    }
    fuzzy::edit_distance(&a, &b) <= if a.len() >= 8 { 2 } else { 1 }
}

/// How the words of the query are found in the names of a module's lecturers: per word its best.
fn finds_in_lecturers(index: &Index, module: &Module, words: &[QueryWord]) -> Vec<Option<Find>> {
    use points::*;
    let mut best: Vec<Option<Find>> = vec![None; words.len()];
    for &(n, teaching) in &module.lecturers {
        let Some(name) = index.names.get(n) else { continue };
        let role = if teaching { INSTRUCTOR } else { 1.0 };
        let mut own: Vec<Option<Find>> = words
            .iter()
            .map(|word| {
                let q = &word.q;
                if !is_letters(q) {
                    return None;
                }
                let mut found: Option<Find> = None;
                for &i in &name.surname {
                    let Some(w) = name.text.word(i) else { continue };
                    let kind = if *q == w.text && q.len() >= 3 {
                        Kind::Name
                    } else if q.len() >= 4 && starts_with(&w.text, q) {
                        Kind::NameStart
                    } else if similar_name(q, &w.text) {
                        Kind::NameLike
                    } else {
                        continue;
                    };
                    let base = match kind {
                        Kind::Name => NAME,
                        Kind::NameStart => NAME_START,
                        _ => NAME_LIKE,
                    };
                    let points = base * role;
                    let span = if kind == Kind::NameStart { w.span(0, q.len()) } else { (w.start, w.end) };
                    if found.as_ref().is_none_or(|f| points > f.points) {
                        found = Some(Find { spans: vec![span], lecturer: Some((n, teaching)), ..Find::elsewhere(kind, points) });
                    }
                }
                found
            })
            .collect();
        // a first name (or its initial) counts with the surname of the same person
        if own.iter().any(Option::is_some) {
            for (k, word) in words.iter().enumerate() {
                if own.get(k).is_some_and(Option::is_some) {
                    continue;
                }
                let q = &word.q;
                let first = name.first.iter().filter_map(|&i| name.text.word(i)).find(|w| w.text == *q || (q.len() == 1 && w.text.first() == q.first()));
                if let (Some(w), Some(slot)) = (first, own.get_mut(k)) {
                    let span = if q.len() == 1 { w.span(0, 1) } else { (w.start, w.end) };
                    *slot = Some(Find { spans: vec![span], lecturer: Some((n, teaching)), ..Find::elsewhere(Kind::FirstName, NAME * role) });
                }
            }
        }
        for (slot, mine) in best.iter_mut().zip(own) {
            if let Some(mine) = mine {
                if slot.as_ref().is_none_or(|f| mine.points > f.points) {
                    *slot = Some(mine);
                }
            }
        }
    }
    best
}

// ---------------------------------------------------------------- a module

/// How a module answers a query.
#[derive(Clone, Debug)]
pub(crate) struct Answer {
    /// How many of the words are found.
    pub(crate) found: usize,
    pub(crate) score: f64,
    /// Words left over in the title one sees (the shorter title first among equals).
    rest: usize,
    /// Per word of the query, its best find.
    pub(crate) best: Vec<Option<Find>>,
}

fn answer(index: &Index, module: &Module, words: &[QueryWord]) -> Answer {
    use points::*;
    let titles: Vec<(&Text, bool)> = std::iter::once((&module.shown, false)).chain(module.other.as_ref().map(|t| (t, true))).collect();
    let in_titles: Vec<Vec<Vec<Find>>> = titles.iter().map(|(title, other)| words.iter().map(|w| finds_in(w, title, *other)).collect()).collect();
    let in_names = finds_in_lecturers(index, module, words);
    let best: Vec<Option<Find>> = words
        .iter()
        .enumerate()
        .map(|(k, word)| {
            let q = &word.q;
            let mut candidates: Vec<Find> = Vec::new();
            // the module's number and its abbreviations, whichever title
            let id: Vec<char> = module.id.chars().collect();
            // (a number is digits, a byte each: the span of `q` in it is its length)
            if is_digits(q) && q.len() >= 3 && id == *q {
                candidates.push(Find { spans: vec![(0, q.len())], ..Find::elsewhere(Kind::Id, NUMBER) });
            } else if is_digits(q) && q.len() >= 3 && starts_with(&id, q) {
                candidates.push(Find { spans: vec![(0, q.len())], ..Find::elsewhere(Kind::IdStart, NUMBER_START) });
            }
            if q.len() >= 2 {
                let exact: Vec<&Abbrev> = module.abbrevs.iter().filter(|a| a.folded == *q).collect();
                let part: Vec<&Abbrev> = module
                    .abbrevs
                    .iter()
                    .filter(|a| is_letters(q) && starts_with(&a.folded, q) && is_digits(a.folded.get(q.len()..).unwrap_or(&[])))
                    .collect();
                let typed = |list: &[&Abbrev], prefix: bool| {
                    has_capital(&word.raw) && list.iter().any(|a| if prefix { a.raw.starts_with(&word.raw) } else { a.raw == word.raw })
                };
                if let Some(first) = exact.first() {
                    let case = typed(&exact, false);
                    candidates.push(Find { abbrev: Some(first.raw.clone()), case, ..Find::elsewhere(Kind::Abbrev, ABBREV * if case { CASE } else { 1.0 }) });
                } else if !part.is_empty() {
                    let case = typed(&part, true);
                    let raws: Vec<&str> = part.iter().map(|a| a.raw.as_str()).collect();
                    candidates.push(Find { abbrev: Some(raws.join(", ")), case, ..Find::elsewhere(Kind::AbbrevPart, ABBREV_PART * if case { CASE } else { 1.0 }) });
                }
            }
            for title in &in_titles {
                candidates.extend(title.get(k).cloned().unwrap_or_default());
            }
            if let Some(Some(find)) = in_names.get(k) {
                candidates.push(find.clone());
            }
            // the most points; among equals the narrower find in a title, else the first
            candidates.sort_by(|a, b| {
                b.points.partial_cmp(&a.points).unwrap_or(std::cmp::Ordering::Equal).then_with(|| {
                    if a.in_title() && b.in_title() {
                        a.width().cmp(&b.width())
                    } else {
                        std::cmp::Ordering::Equal
                    }
                })
            });
            candidates.into_iter().next()
        })
        .collect();
    let found = best.iter().filter(|find| find.is_some()).count();
    if found == 0 {
        return Answer { found: 0, score: 0.0, rest: 0, best };
    }

    // the whole module named: its number, its abbreviation, or every word of one of its titles
    let mut named = 0.0;
    let mut named_by: Option<Kind> = None;
    let single = if words.len() == 1 { best.first().and_then(Option::as_ref).map(|f| f.kind) } else { None };
    if let Some(kind @ (Kind::Id | Kind::Abbrev | Kind::AbbrevPart)) = single {
        named = best.first().and_then(Option::as_ref).map_or(0.0, |f| f.points);
        named_by = Some(kind);
    }
    let mut named_by_title = false;
    if found == words.len() {
        for ((title, other), all) in titles.iter().zip(&in_titles) {
            let Some(case) = covers(words, title, all) else { continue };
            let points = TITLE * (if *other { OTHER } else { 1.0 }) * (if case { CASE } else { 1.0 });
            if points > named {
                named = points;
                named_by = None;
                named_by_title = true;
            }
        }
    }

    // else the weakest word, more where the words stand side by side in their order
    let hits: Vec<&Find> = best.iter().flatten().collect();
    let mut phrase = false;
    if words.len() > 1 && hits.len() == words.len() && hits.iter().all(|f| f.in_title() && Some(f.field) == hits.first().map(|h| h.field)) {
        let title = if hits.first().is_some_and(|h| h.field == Field::Other) { module.other.as_ref() } else { Some(&module.shown) };
        if let Some(title) = title {
            phrase = true;
            for pair in hits.windows(2) {
                let (Some(before), Some(next)) = (pair.first(), pair.get(1)) else { continue };
                let mut t = before.to + 1;
                while title.word(t).is_some_and(|w| w.filler) {
                    t += 1;
                }
                if next.from != t {
                    phrase = false;
                    break;
                }
            }
        }
    }
    let weakest = hits.iter().map(|f| f.points).fold(f64::INFINITY, f64::min) * if phrase { PHRASE } else { 1.0 };
    let score = named.max(weakest);
    let mut covered = HashSet::new();
    for f in &hits {
        if f.field == Field::Shown {
            covered.extend(f.from..=f.to);
        }
    }
    let by_number_or_abbrev = !named_by_title && named_by.is_some();
    let rest = if by_number_or_abbrev { 0 } else { module.shown.words.iter().enumerate().filter(|(i, w)| w.content && !covered.contains(i)).count() };
    Answer { found, score, rest, best }
}

/// A title's place in a series: what stands before its first number, and the number.
fn series(module: &Module) -> Option<(String, usize)> {
    let words = &module.shown.words;
    let i = words.iter().position(|w| is_digits(&w.text) || is_roman(&w.text))?;
    if i == 0 {
        return None;
    }
    let base: Vec<String> = words.get(..i)?.iter().map(|w| w.text.iter().collect()).collect();
    let number = words.get(i)?;
    let text: String = number.text.iter().collect();
    let n = if is_digits(&number.text) { text.parse().ok()? } else { ROMAN.iter().position(|r| *r == text)? + 1 };
    Some((base.join(" "), n))
}

fn round(x: f64) -> f64 {
    (x * 1e4).round() / 1e4
}

/// `a` before `b` as the list compares titles: case-blind in ASCII, as SQLite's NOCASE.
fn compare_titles(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase())
}

/// A module the query finds.
#[derive(Clone, Debug)]
pub(crate) struct Hit {
    pub(crate) module: usize,
    pub(crate) found: usize,
    /// The points of the query alone (`order` raises those of a program's modules).
    pub(crate) points: f64,
    pub(crate) answer: Answer,
}

/// What `words` find, in no order: with `most_words` every module that has one of the words, else
/// those that have all. Their points are those of the query alone, the same for everyone.
pub(crate) fn evaluate(index: &Index, words: &[QueryWord], most_words: bool) -> Vec<Hit> {
    if words.is_empty() {
        return Vec::new();
    }
    index
        .modules
        .iter()
        .enumerate()
        .filter_map(|(i, module)| {
            let answer = answer(index, module, words);
            let enough = if most_words { answer.found > 0 } else { answer.found == words.len() };
            enough.then(|| Hit { module: i, found: answer.found, points: round(answer.score), answer })
        })
        .collect()
}

/// The order of the list of `hits` (their places in it): the most words found, the most points
/// (those of the modules in `mine` raised, `points::PROGRAM`); then offered before phase-out, the
/// fewer words left over, the more programs (a module number's start: the numbers in their order),
/// a series in its order, A–Z.
pub(crate) fn order(index: &Index, hits: &[Hit], words: &[QueryWord], mine: &HashSet<String>) -> Vec<usize> {
    let points: Vec<f64> = hits
        .iter()
        .map(|hit| {
            let raised = index.modules.get(hit.module).is_some_and(|m| mine.contains(&m.id));
            if raised { round(hit.answer.score * points::PROGRAM) } else { hit.points }
        })
        .collect();
    let number = words.len() == 1 && words.first().is_some_and(|w| is_digits(&w.q));
    // The parts of a series stand together, in their order („Elektrotechnik 1", „2"): what decides
    // after the points and the offer is the series' best (its fewest words left, its most programs).
    let mut groups: HashMap<String, (usize, u32)> = HashMap::new();
    for hit in hits {
        let Some(module) = index.modules.get(hit.module) else { continue };
        if let Some((base, _)) = series(module) {
            let group = groups.entry(base).or_insert((usize::MAX, 0));
            group.0 = group.0.min(hit.answer.rest);
            group.1 = group.1.max(module.programs);
        }
    }
    struct Tie {
        rest: usize,
        order: i64,
        base: String,
        n: usize,
    }
    let ties: Vec<Tie> = hits
        .iter()
        .map(|hit| {
            let module = index.modules.get(hit.module);
            let series = module.and_then(series);
            let group = series.as_ref().and_then(|(base, _)| groups.get(base));
            let programs = group.map_or_else(|| module.map_or(0, |m| m.programs), |g| g.1);
            Tie {
                rest: group.map_or(hit.answer.rest, |g| g.0),
                order: if number { module.and_then(|m| m.id.parse::<i64>().ok()).unwrap_or(0) } else { -i64::from(programs) },
                base: series.as_ref().map_or_else(|| module.map_or_else(String::new, |m| fold(&m.shown.raw)), |(base, _)| base.clone()),
                n: series.map_or(0, |(_, n)| n),
            }
        })
        .collect();
    let mut order: Vec<usize> = (0..hits.len()).collect();
    order.sort_by(|&a, &b| {
        let (Some(x), Some(y), Some(tx), Some(ty), Some(px), Some(py)) = (hits.get(a), hits.get(b), ties.get(a), ties.get(b), points.get(a), points.get(b)) else {
            return std::cmp::Ordering::Equal;
        };
        let (mx, my) = (index.modules.get(x.module), index.modules.get(y.module));
        y.found
            .cmp(&x.found)
            .then_with(|| py.partial_cmp(px).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| mx.map(|m| m.status).cmp(&my.map(|m| m.status)))
            .then_with(|| tx.rest.cmp(&ty.rest))
            .then_with(|| tx.order.cmp(&ty.order))
            .then_with(|| tx.base.cmp(&ty.base))
            .then_with(|| tx.n.cmp(&ty.n))
            .then_with(|| compare_titles(mx.map_or("", |m| m.shown.raw.as_str()), my.map_or("", |m| m.shown.raw.as_str())))
            .then_with(|| mx.map(|m| m.id.as_str()).cmp(&my.map(|m| m.id.as_str())))
    });
    order
}

impl Index {
    pub(crate) fn module_id(&self, i: usize) -> Option<&str> {
        self.modules.get(i).map(|m| m.id.as_str())
    }

    pub(crate) fn vocabulary(&self) -> &Vocabulary {
        &self.vocabulary
    }

    /// The marks of a module's answer: what of its title, its other title, a lecturer's name, its
    /// number and its abbreviations the query found (`folia_model::rows::SearchMarks`).
    pub(crate) fn marks(&self, i: usize, answer: &Answer) -> Option<SearchMarks> {
        let module = self.modules.get(i)?;
        let finds: Vec<&Find> = answer.best.iter().flatten().collect();
        let spans = |keep: &dyn Fn(&Find) -> bool| -> Vec<(u32, u32)> {
            finds.iter().filter(|f| keep(f)).flat_map(|f| f.spans.iter().map(|(a, b)| (*a as u32, *b as u32))).collect()
        };
        let title = spans(&|f| f.field == Field::Shown);
        let id = spans(&|f| matches!(f.kind, Kind::Id | Kind::IdStart));
        let other_spans = spans(&|f| f.field == Field::Other);
        let other = module.other.as_ref().filter(|_| !other_spans.is_empty()).map(|t| MarkedText { text: t.raw.clone(), spans: other_spans });
        let person = finds.iter().find_map(|f| f.lecturer);
        let lecturer = person.and_then(|(n, _)| {
            let spans = spans(&|f| f.lecturer.map(|(m, _)| m) == Some(n));
            self.names.get(n).map(|name| MarkedText { text: name.text.raw.clone(), spans })
        });
        let abbrev = finds.iter().find_map(|f| f.abbrev.clone());
        Some(SearchMarks { title, other, lecturer, teaching: person.is_some_and(|(_, teaching)| teaching), id, abbrev })
    }

    pub(crate) fn position(&self, id: &str) -> Option<usize> {
        self.modules.binary_search_by(|m| m.id.as_str().cmp(id)).ok()
    }

    pub(crate) fn answer_of(&self, i: usize, words: &[QueryWord]) -> Option<Answer> {
        self.modules.get(i).map(|module| answer(self, module, words))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spellings_are_made_alike() {
        assert_eq!(alike("hofstedt"), "hofstet");
        assert_eq!(alike("hoffstedt"), "hofstet");
        assert_eq!(alike("doering"), "doring");
        assert_eq!(alike("dorring"), "doring");
        assert_eq!(alike("meyer"), "maier");
        assert_eq!(alike("lehmann"), "leman");
        let chars = |s: &str| s.chars().collect::<Vec<char>>();
        assert!(similar_name(&chars("hofstatt"), &chars("hofstedt")));
        assert!(similar_name(&chars("hofstet"), &chars("hofstedt")));
        assert!(!similar_name(&chars("hof"), &chars("hofstedt")), "from four letters");
        assert!(!similar_name(&chars("kofstedt"), &chars("hofstedt")), "the first letter right");
    }

    #[test]
    fn a_title_in_words() {
        let text = Text::new("TIN* - trans*, inter*, nicht-binär");
        let words: Vec<String> = text.words.iter().map(|w| w.text.iter().collect()).collect();
        assert_eq!(words, ["tin", "trans", "inter", "nicht", "binar"]);
        let gaps: Vec<Gap> = text.words.iter().map(|w| w.gap).collect();
        assert_eq!(gaps, [Gap::None, Gap::Strong, Gap::Strong, Gap::Strong, Gap::Compound]);
        let text = Text::new("Kinder- und Jugendhilfe / Stahl- & Holzbau");
        let gaps: Vec<Gap> = text.words.iter().map(|w| w.gap).collect();
        assert_eq!(gaps, [Gap::None, Gap::None, Gap::None, Gap::Weak, Gap::Weak]);
        let text = Text::new("Straße");
        let w = text.word(0).unwrap();
        assert_eq!(w.text.iter().collect::<String>(), "strasse");
        assert_eq!(w.span(0, 6), (0, "Straß".len()), "the folded letters point at the letters they come from");
    }

    #[test]
    fn short_forms_of_words_that_follow_each_other() {
        let forms = |q: &str, title: &str, strict: bool| -> Vec<(usize, usize, usize)> {
            let q: Vec<char> = q.chars().collect();
            short_forms(&q, &Text::new(title), strict).into_iter().map(|s| (s.from, s.to, s.skipped)).collect()
        };
        assert_eq!(forms("ti", "Theoretische Informatik", false), [(0, 1, 0)]);
        assert_eq!(forms("theoinf", "Theoretische Informatik", false), [(0, 1, 0)]);
        assert_eq!(forms("aud", "Algorithmen und Datenstrukturen", false), [(0, 2, 0)]);
        assert_eq!(forms("ad", "Algorithmen und Datenstrukturen", false), [(0, 2, 1)]);
        assert!(forms("ti", "Power Plant Technology I", false).is_empty(), "a numeral is no initial");
        assert!(forms("pp", "Power Plant Technology", false).is_empty(), "a short form ends a group of words");
        assert_eq!(forms("ml", "Explainable Machine Learning", false), [(1, 2, 0)]);
        assert!(forms("ml", "Explainable Machine Learning", true).is_empty(), "in the other title it begins a group too");
        assert!(forms("bau", "Betriebliche Ausbildung", false).is_empty(), "no piece longer than the one before");
    }
}
