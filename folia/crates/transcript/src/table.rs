//! The table of a Notenübersicht, and what it is read as.

use crate::pdf::content::{Page, Run};
use crate::Error;

/// The most modules a Notenübersicht is read with; a long study has a hundred.
pub const MAX_ENTRIES: usize = 500;

/// The most characters of a title, a heading or the program's name.
const MAX_TEXT: usize = 300;

/// A semester as the Notenübersicht writes it: „WS 2024/25", „SS 2023".
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Term {
    /// The year it begins in.
    pub year: u16,
    pub winter: bool,
}

impl Term {
    /// „WS 2024/25", „WiSe 2024/25", „SS 2023", „SoSe 2023".
    pub fn parse(text: &str) -> Option<Term> {
        let mut words = text.split_whitespace();
        let season = words.next()?;
        let year = words.next()?;
        if words.next().is_some() {
            return None;
        }
        let winter = match season.to_ascii_lowercase().as_str() {
            "ws" | "wise" | "ws." => true,
            "ss" | "sose" | "ss." => false,
            _ => return None,
        };
        let year: u16 = year.split('/').next()?.parse().ok()?;
        (1900..=2200).contains(&year).then_some(Term { year, winter })
    }

    /// „2024W", „2023S": as `folia_calendar`'s semester keys are written.
    pub fn code(self) -> String {
        format!("{}{}", self.year, if self.winter { 'W' } else { 'S' })
    }

    pub fn from_code(code: &str) -> Option<Term> {
        let (year, season) = code.split_at_checked(4)?;
        let winter = match season {
            "W" => true,
            "S" => false,
            _ => return None,
        };
        let year: u16 = year.parse().ok()?;
        (1900..=2200).contains(&year).then_some(Term { year, winter })
    }
}

/// A day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl Date {
    /// „2022-10-01".
    pub fn code(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    pub fn from_code(code: &str) -> Option<Date> {
        let mut parts = code.split('-');
        let date = Date { year: parts.next()?.parse().ok()?, month: parts.next()?.parse().ok()?, day: parts.next()?.parse().ok()? };
        (parts.next().is_none() && (1..=12).contains(&date.month) && (1..=31).contains(&date.day)).then_some(date)
    }

    /// The semester the day falls in: the winter from October to March, the summer from April to
    /// September.
    pub fn term(self) -> Term {
        match self.month {
            4..=9 => Term { year: self.year, winter: false },
            10..=12 => Term { year: self.year, winter: true },
            _ => Term { year: self.year.saturating_sub(1), winter: true },
        }
    }
}

/// The status of a module as the column „Status" writes it (its legend: „BE = bestanden / passed,
/// NB = nicht bestanden / failed, EN = endgültig nicht bestanden / finally failed, AN = angemeldet
/// / registered").
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    Passed,
    Failed,
    FinallyFailed,
    Registered,
    /// A code the legend does not have, as written.
    Other(String),
}

impl Status {
    pub fn parse(code: &str) -> Status {
        match code {
            "BE" => Status::Passed,
            "NB" => Status::Failed,
            "EN" => Status::FinallyFailed,
            "AN" => Status::Registered,
            other => Status::Other(other.to_string()),
        }
    }

    pub fn code(&self) -> &str {
        match self {
            Status::Passed => "BE",
            Status::Failed => "NB",
            Status::FinallyFailed => "EN",
            Status::Registered => "AN",
            Status::Other(code) => code,
        }
    }
}

/// A module of the table.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Its title as the first line of its cell writes it: in the language it is taught in.
    pub title: String,
    /// The second line: the title in the other language, where it has one.
    pub other: Option<String>,
    pub status: Status,
    /// Its Leistungspunkte as written (0 where it is not passed).
    pub credits: Option<f64>,
    /// Which attempt the status is of.
    pub attempt: Option<u8>,
    pub term: Option<Term>,
    /// The column „Anerkennung" says something: recognised from elsewhere.
    pub recognised: bool,
    /// The heading it stands under, in German: „Komplex Informatik", „Zusatzmodule". The table
    /// does not say how its headings nest (all are set alike), so only the nearest is kept.
    pub heading: Option<String>,
}

/// What a Notenübersicht is read as.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Transcript {
    /// The program as the sentence above the table names it, without its degree and its addition:
    /// „Informatik".
    pub program: Option<String>,
    /// The degree it names: „Bachelor", „Master".
    pub degree: Option<String>,
    /// The addition in parentheses: „universitäres Profil".
    pub profile: Option<String>,
    /// Since when the student studies it („seit 1. Oktober 2022").
    pub since: Option<Date>,
    pub entries: Vec<Entry>,
}

/// A field of a line: no tab, no line break, at most `MAX_TEXT` characters.
fn field(text: &str) -> String {
    text.chars().map(|c| if c.is_control() { ' ' } else { c }).take(MAX_TEXT).collect::<String>().trim().to_string()
}

impl Transcript {
    /// As lines of fields separated by tabs, as the Web Worker answers it:
    ///
    /// ```text
    /// program  <degree>  <name>  <profile>
    /// since    <yyyy-mm-dd>
    /// module   <status>  <semester>  <credits>  <attempt>  <recognised>  <heading>  <title>  <other title>
    /// ```
    ///
    /// The semester as `2024W`, empty fields for what the table leaves empty, the recognised mark
    /// `1` or empty.
    pub fn lines(&self) -> String {
        let mut out = String::new();
        if self.program.is_some() || self.degree.is_some() {
            out.push_str(&format!(
                "program\t{}\t{}\t{}\n",
                field(self.degree.as_deref().unwrap_or_default()),
                field(self.program.as_deref().unwrap_or_default()),
                field(self.profile.as_deref().unwrap_or_default())
            ));
        }
        if let Some(since) = self.since {
            out.push_str(&format!("since\t{}\n", since.code()));
        }
        for entry in &self.entries {
            out.push_str(&format!(
                "module\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                field(entry.status.code()),
                entry.term.map(Term::code).unwrap_or_default(),
                entry.credits.map(credits_text).unwrap_or_default(),
                entry.attempt.map(|attempt| attempt.to_string()).unwrap_or_default(),
                if entry.recognised { "1" } else { "" },
                field(entry.heading.as_deref().unwrap_or_default()),
                field(&entry.title),
                field(entry.other.as_deref().unwrap_or_default()),
            ));
        }
        out
    }

    /// The lines of `lines` read back, or what the worker answered instead of them (`error
    /// <code>`). A line that does not read is left out; at most `MAX_ENTRIES` modules.
    pub fn from_lines(text: &str) -> Result<Transcript, Error> {
        let mut transcript = Transcript::default();
        let some = |text: &str| -> Option<String> { Some(field(text)).filter(|text| !text.is_empty()) };
        for line in text.lines() {
            let fields: Vec<&str> = line.split('\t').collect();
            match fields.as_slice() {
                ["error", code, ..] => return Err(Error::from_code(code).unwrap_or(Error::NotTranscript)),
                ["program", degree, name, profile, ..] => {
                    transcript.degree = some(degree);
                    transcript.program = some(name);
                    transcript.profile = some(profile);
                }
                ["since", date, ..] => transcript.since = Date::from_code(date),
                ["module", status, term, credits, attempt, recognised, heading, title, other, ..] => {
                    let Some(title) = some(title) else { continue };
                    if transcript.entries.len() >= MAX_ENTRIES {
                        continue;
                    }
                    transcript.entries.push(Entry {
                        title,
                        other: some(other),
                        status: Status::parse(&field(status)),
                        credits: number(credits).filter(|credits| (0.0..=1000.0).contains(credits)),
                        attempt: attempt.parse().ok(),
                        term: Term::from_code(term),
                        recognised: *recognised == "1",
                        heading: some(heading),
                    });
                }
                _ => {}
            }
        }
        if transcript.entries.is_empty() {
            return Err(Error::NotTranscript);
        }
        Ok(transcript)
    }
}

/// What a column of the table holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Column {
    Title,
    Grade,
    Status,
    Credits,
    Attempt,
    Term,
    Date,
    Recognition,
}

/// The heads of the columns, German over English, as the table writes them on every page.
const HEADS: [(&str, Column); 14] = [
    ("Modultitel", Column::Title),
    ("Module Title", Column::Title),
    ("Note", Column::Grade),
    ("Grade", Column::Grade),
    ("Status", Column::Status),
    ("Leistungspunkte", Column::Credits),
    ("Credits", Column::Credits),
    ("Versuch", Column::Attempt),
    ("Attempt", Column::Attempt),
    ("Semester", Column::Term),
    ("Datum", Column::Date),
    ("Date", Column::Date),
    ("Anerkennung", Column::Recognition),
    ("Recognition", Column::Recognition),
];

/// Where the columns of a page's table are.
struct Layout {
    /// Where the titles begin.
    left: f64,
    /// A run that begins left of this is a title's.
    title_end: f64,
    /// The middle of each column's head: a value is centred in its column as its head is, so
    /// the head nearest to it names its column.
    columns: Vec<(f64, Column)>,
    /// The lowest line of the heads: the rows are below it.
    bottom: f64,
    /// The highest: what the page says above the table is above it.
    top: f64,
}

/// The table's layout on `page`, from the heads of its columns.
fn layout(page: &Page) -> Option<Layout> {
    let title = page.runs.iter().find(|run| run.bold && run.text.trim() == "Modultitel")?;
    let band = |run: &Run| run.y <= title.y + 0.5 * title.size && run.y >= title.y - 1.6 * title.size;
    let heads: Vec<(&Run, Column)> =
        page.runs.iter().filter(|run| run.bold && band(run)).filter_map(|run| HEADS.iter().find(|(head, _)| run.text.trim() == *head).map(|(_, column)| (run, *column))).collect();
    if !heads.iter().any(|(_, column)| *column == Column::Status) {
        return None;
    }
    let values = heads.iter().filter(|(_, column)| *column != Column::Title);
    let first = values.clone().map(|(run, _)| run.x).fold(f64::INFINITY, f64::min);
    Some(Layout {
        left: title.x,
        title_end: first - 2.0,
        columns: values.map(|(run, column)| ((run.x + run.end) / 2.0, *column)).collect(),
        bottom: heads.iter().map(|(run, _)| run.y).fold(f64::INFINITY, f64::min),
        top: heads.iter().map(|(run, _)| run.y + run.size).fold(f64::NEG_INFINITY, f64::max),
    })
}

/// A row of the table: its runs.
type Row<'a> = Vec<&'a Run>;

/// The rows below the heads: between the rules that run under the titles, where the page draws
/// them (the BTU's draws every cell's lower edge); else by the gaps between the lines of the
/// titles. What lies below the last rule (or after a line wider than the title's column) is the
/// text under the table.
fn rows<'a>(page: &'a Page, layout: &Layout) -> Vec<Row<'a>> {
    let mut lines: Vec<f64> = page.rules.iter().filter(|rule| rule.x0 <= layout.left + 5.0 && rule.x1 >= layout.left + 30.0 && rule.y < layout.bottom).map(|rule| rule.y).collect();
    lines.sort_by(|a, b| b.total_cmp(a));
    lines.dedup_by(|a, b| (*a - *b).abs() < 1.0);
    let below: Vec<&Run> = page.runs.iter().filter(|run| run.y < layout.bottom - 1.0).collect();
    if lines.len() >= 2 {
        return lines.windows(2).filter_map(|pair| if let [upper, lower] = pair { Some((*upper, *lower)) } else { None }).map(|(upper, lower)| below.iter().copied().filter(|run| run.y < upper && run.y > lower).collect()).filter(|row: &Row| !row.is_empty()).collect();
    }
    // Without rules: the titles' lines, a new row where the gap is wider than a line's or the
    // weight changes; every value to the row it stands beside.
    let mut titles: Vec<&Run> = below.iter().copied().filter(|run| run.x < layout.title_end).collect();
    titles.sort_by(|a, b| b.y.total_cmp(&a.y).then(a.x.total_cmp(&b.x)));
    if let Some(end) = titles.iter().position(|run| run.end > layout.title_end + 10.0) {
        titles.truncate(end);
    }
    let floor = titles.last().map_or(f64::NEG_INFINITY, |run| run.y - run.size);
    let mut rows: Vec<Row> = Vec::new();
    let mut last: Option<&Run> = None;
    for run in titles {
        let same_line = last.is_some_and(|last| (last.y - run.y).abs() < 0.4 * run.size);
        let next_line = last.is_some_and(|last| last.y - run.y <= 1.35 * run.size && last.bold == run.bold);
        match rows.last_mut() {
            Some(row) if same_line || next_line => row.push(run),
            _ => rows.push(vec![run]),
        }
        last = Some(run);
    }
    for value in below.iter().copied().filter(|run| run.x >= layout.title_end && run.y > floor) {
        let distance = |row: &Row| {
            let (top, bottom) = row.iter().fold((f64::NEG_INFINITY, f64::INFINITY), |(top, bottom), run| (top.max(run.y), bottom.min(run.y)));
            (value.y - (top + bottom) / 2.0).abs()
        };
        let nearest = rows.iter_mut().min_by(|a, b| distance(a).total_cmp(&distance(b)));
        if let Some(row) = nearest {
            row.push(value);
        }
    }
    rows
}

/// A line of a title: its text and whether it is set in italic.
struct Line {
    text: String,
    italic: bool,
    bold: bool,
}

/// The lines of a cell's runs, top to bottom; on one line the runs from left to right, a space
/// where they stand apart.
fn lines_of(runs: &[&Run]) -> Vec<Line> {
    let mut runs = runs.to_vec();
    runs.sort_by(|a, b| b.y.total_cmp(&a.y).then(a.x.total_cmp(&b.x)));
    let mut lines: Vec<(f64, Vec<&Run>)> = Vec::new();
    for run in runs {
        match lines.last_mut() {
            Some((y, line)) if (*y - run.y).abs() < 0.4 * run.size => line.push(run),
            _ => lines.push((run.y, vec![run])),
        }
    }
    lines
        .into_iter()
        .map(|(_, runs)| {
            let mut text = String::new();
            let mut end: Option<f64> = None;
            for run in &runs {
                if end.is_some_and(|end| run.x - end > 0.15 * run.size) && !text.ends_with(' ') && !run.text.starts_with(' ') {
                    text.push(' ');
                }
                text.push_str(&run.text);
                end = Some(run.end);
            }
            Line { text, italic: runs.first().is_some_and(|run| run.italic), bold: runs.iter().all(|run| run.bold) }
        })
        .collect()
}

/// Lines of one text joined: a word broken at a hyphen at the end of a line goes on without a
/// space, everything else with one.
fn join(lines: &[&Line]) -> String {
    let mut text = String::new();
    for line in lines {
        if !text.is_empty() && !(text.ends_with('-') && !text.ends_with(" -")) {
            text.push(' ');
        }
        text.push_str(&line.text);
        // A line that ends in a space ends in a space, whatever comes next.
        if line.text.ends_with(' ') {
            text.push(' ');
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A number as the table writes it, with a decimal comma (or a point).
fn number(text: &str) -> Option<f64> {
    crate::pdf::syntax::decimal(text.trim().replace(',', ".").as_bytes())
}

/// Credits as a line writes them: „8", „7.5" (to hundredths), without the general writer of
/// floats.
fn credits_text(credits: f64) -> String {
    let hundredths = (credits * 100.0).round() as i64;
    let (whole, part) = (hundredths / 100, hundredths % 100);
    match part {
        0 => whole.to_string(),
        part if part % 10 == 0 => format!("{whole}.{}", part / 10),
        part => format!("{whole}.{part:02}"),
    }
}

/// „25.03.2025".
fn date(text: &str) -> Option<Date> {
    let mut parts = text.trim().split('.');
    let day: u8 = parts.next()?.parse().ok()?;
    let month: u8 = parts.next()?.parse().ok()?;
    let year: u16 = parts.next()?.parse().ok()?;
    (parts.next().is_none() && (1..=12).contains(&month) && (1..=31).contains(&day)).then_some(Date { year, month, day })
}

/// The Notenübersicht on `pages`; `None` where they hold no table of it.
pub fn read(pages: &[Page]) -> Option<Transcript> {
    let mut transcript = Transcript::default();
    let mut heading: Option<String> = None;
    let mut tables = 0;
    for (index, page) in pages.iter().enumerate() {
        let Some(layout) = layout(page) else { continue };
        tables += 1;
        if index == 0 || transcript.since.is_none() {
            sentence(page, &layout, &mut transcript);
        }
        for row in rows(page, &layout) {
            let (titles, values): (Vec<&Run>, Vec<&Run>) = row.iter().partition(|run| run.x < layout.title_end);
            let column = |run: &Run| {
                let middle = (run.x + run.end) / 2.0;
                layout.columns.iter().min_by(|a, b| (a.0 - middle).abs().total_cmp(&(b.0 - middle).abs())).map(|(_, column)| *column)
            };
            let value = |wanted: Column| -> Option<String> {
                let text = values.iter().filter(|run| column(run) == Some(wanted)).map(|run| run.text.trim()).collect::<Vec<_>>().join(" ");
                Some(text).filter(|text| !text.is_empty())
            };
            let lines = lines_of(&titles);
            if lines.is_empty() {
                continue;
            }
            let Some(status) = value(Column::Status) else {
                // A heading: bold, and nothing in the columns (the „Gesamtkonto" has its sums).
                if values.is_empty() && lines.iter().all(|line| line.bold) {
                    let german: Vec<&Line> = lines.iter().take_while(|line| !line.italic).collect();
                    heading = Some(join(&german)).filter(|text| !text.is_empty()).map(|text| field(&text));
                }
                continue;
            };
            // The title in its first style, the other language in the other.
            let first_italic = lines.first().is_some_and(|line| line.italic);
            let (own, other): (Vec<&Line>, Vec<&Line>) = {
                let split = lines.iter().position(|line| line.italic != first_italic).unwrap_or(lines.len());
                (lines.iter().take(split).collect(), lines.iter().skip(split).collect())
            };
            let title = field(&join(&own));
            if title.is_empty() || transcript.entries.len() >= MAX_ENTRIES {
                continue;
            }
            let term = value(Column::Term).and_then(|text| Term::parse(&text)).or_else(|| value(Column::Date).and_then(|text| date(&text)).map(Date::term));
            transcript.entries.push(Entry {
                title,
                other: Some(field(&join(&other))).filter(|text| !text.is_empty()),
                status: Status::parse(&status),
                credits: value(Column::Credits).and_then(|text| number(&text)),
                attempt: value(Column::Attempt).and_then(|text| text.trim().parse().ok()),
                term,
                recognised: value(Column::Recognition).is_some(),
                heading: heading.clone(),
            });
        }
    }
    (tables > 0 && !transcript.entries.is_empty()).then_some(transcript)
}

/// The German months as the sentence above the table names them.
const MONTHS: [&str; 12] = ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"];

/// The program and since when from the sentence above the table: „… studiert seit 1. Oktober
/// 2022 im Bachelor-Studiengang Informatik (universitäres Profil) und erzielte bisher folgende
/// Leistungen". What stands before „studiert seit" (the name, the date of birth) is not read.
fn sentence(page: &Page, layout: &Layout, transcript: &mut Transcript) {
    let above: Vec<&Run> = page.runs.iter().filter(|run| run.y > layout.top && !run.italic).collect();
    let text = lines_of(&above).iter().map(|line| line.text.trim().to_string()).collect::<Vec<_>>().join(" ");
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let Some(at) = text.find("studiert seit ") else { return };
    let rest = text.get(at + "studiert seit ".len()..).unwrap_or_default();
    // „seit dem 1. Oktober 2022" as well.
    let rest = rest.strip_prefix("dem ").unwrap_or(rest);
    let mut words = rest.split(' ');
    let since = (|| {
        let day: u8 = words.next()?.strip_suffix('.')?.parse().ok()?;
        let name = words.next()?;
        let month = MONTHS.iter().position(|month| *month == name)?;
        let year: u16 = words.next()?.parse().ok()?;
        Some(Date { year, month: u8::try_from(month + 1).ok()?, day })
    })();
    transcript.since = since.filter(|date| (1..=31).contains(&date.day));
    let Some(start) = rest.find(" im ").or_else(|| rest.find(" in ")) else { return };
    let phrase = rest.get(start + 4..).unwrap_or_default();
    let phrase = phrase.split(" und erzielte").next().unwrap_or(phrase).trim_end_matches(['.', ':', ' ']);
    let (degree, program) = match phrase.split_once("-Studiengang ") {
        Some((degree, program)) => (Some(degree.trim()), program.trim()),
        None => (None, phrase.strip_prefix("Studiengang ").unwrap_or(phrase).trim()),
    };
    let (program, profile) = match (program.rfind(" ("), program.strip_suffix(')')) {
        (Some(open), Some(_)) => (program.get(..open).unwrap_or(program), program.get(open + 2..program.len() - 1)),
        _ => (program, None),
    };
    transcript.degree = degree.map(field).filter(|text| !text.is_empty());
    transcript.program = Some(field(program)).filter(|text| !text.is_empty());
    transcript.profile = profile.map(field).filter(|text| !text.is_empty());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{self, Options};

    fn entry(transcript: &Transcript, title: &str) -> Entry {
        transcript.entries.iter().find(|entry| entry.title == title).cloned().unwrap_or_else(|| panic!("{title} not read: {transcript:?}"))
    }

    fn check_example(transcript: &Transcript) {
        assert_eq!(transcript.degree.as_deref(), Some("Bachelor"));
        assert_eq!(transcript.program.as_deref(), Some("Informatik"));
        assert_eq!(transcript.profile.as_deref(), Some("universitäres Profil"));
        assert_eq!(transcript.since, Some(Date { year: 2022, month: 10, day: 1 }));
        assert_eq!(transcript.entries.len(), 12, "the headings and the Gesamtkonto are no modules: {transcript:#?}");

        let theory = entry(transcript, "Theoretische Informatik");
        assert_eq!(theory.other.as_deref(), Some("Theoretical Computer Science"));
        assert_eq!((theory.status, theory.credits, theory.attempt, theory.term), (Status::Passed, Some(8.0), Some(2), Some(Term { year: 2024, winter: true })));
        assert_eq!(theory.heading.as_deref(), Some("Komplex Informatik"));

        // Over two lines each, the German and the English.
        let wrapped = entry(transcript, "Elektrische und elektronische Grundlagen der Informatik");
        assert_eq!(wrapped.other.as_deref(), Some("Electrical and Electronic Foundation of Computer Engineering"));

        assert_eq!(entry(transcript, "Mathematik IT-2 (Lineare Algebra)").status, Status::Failed);
        let thesis = entry(transcript, "Bachelor-Arbeit");
        assert_eq!((thesis.status, thesis.term, thesis.heading.as_deref()), (Status::Registered, Some(Term { year: 2026, winter: false }), Some("Fachstudium")));
        let retrieval = entry(transcript, "Einführung Information Retrieval");
        assert_eq!((retrieval.status, retrieval.attempt), (Status::FinallyFailed, Some(3)));

        // Taught in English: its English title first, in italic.
        let graphs = entry(transcript, "Graph Theory");
        assert_eq!(graphs.other.as_deref(), Some("Graphentheorie"));

        // Recognised, without a semester.
        let security = entry(transcript, "Sicherheitsengineering");
        assert!(security.recognised);
        assert_eq!(security.term, None);
        assert_eq!(entry(transcript, "Einführung in die Constraint-Programmierung").heading.as_deref(), Some("Zusatzmodule"));
        // A heading at the end of a page counts on the next.
        assert_eq!(entry(transcript, "Datenbanken").heading.as_deref(), Some("Praktische Informatik"));
    }

    #[test]
    fn the_example_as_the_btu_writes_it() {
        check_example(&crate::read(&fixture::example()).unwrap());
    }

    #[test]
    fn without_encryption_and_object_streams() {
        check_example(&crate::read(&fixture::pdf(&fixture::example_pages(), Options { encrypted: false, packed: false, rules: true })).unwrap());
    }

    #[test]
    fn without_rules_by_the_gaps_between_the_rows() {
        check_example(&crate::read(&fixture::pdf(&fixture::example_pages(), Options { rules: false, ..Options::default() })).unwrap());
    }

    #[test]
    fn nothing_of_the_student_is_read_but_the_study() {
        let lines = crate::read(&fixture::example()).unwrap().lines();
        for private in ["Mustermann", "Musterstadt", "Musterweg", "2003", "August", "3,0", "2,3", "25.03.2025", "Total Account"] {
            assert!(!lines.contains(private), "{private} in {lines}");
        }
    }

    #[test]
    fn the_lines_read_back() {
        let transcript = crate::read(&fixture::example()).unwrap();
        assert_eq!(Transcript::from_lines(&transcript.lines()), Ok(transcript));
        assert_eq!(Transcript::from_lines("error\tlocked\n"), Err(Error::Locked));
        assert_eq!(Transcript::from_lines(""), Err(Error::NotTranscript));
        // A line that does not read is left out, the others stay.
        let read = Transcript::from_lines("module\tBE\t2024W\tacht\t\t\t\tDatenbanken\t\nmodule\tBE\nnonsense\n").unwrap();
        assert_eq!(read.entries.len(), 1);
        assert_eq!(read.entries[0].credits, None);
    }

    #[test]
    fn a_file_with_a_password_is_locked() {
        let mut pdf = fixture::example();
        let at = crate::pdf::syntax::find(&pdf, b"/U <", 0).unwrap() + 4;
        pdf[at] = if pdf[at] == b'0' { b'1' } else { b'0' };
        assert_eq!(crate::read(&pdf), Err(Error::Locked));
    }

    #[test]
    fn what_is_no_transcript() {
        assert_eq!(crate::read(b"not a pdf at all"), Err(Error::NotPdf));
        assert_eq!(crate::read(b"%PDF-1.4\n%%EOF\n"), Err(Error::NotPdf));
        let empty = fixture::pdf(&[vec![]], Options::default());
        assert_eq!(crate::read(&empty), Err(Error::NotTranscript));
    }

    #[test]
    fn broken_files_never_panic() {
        let pdf = fixture::example();
        // Cut anywhere, and bytes changed all over (a fixed sequence, the same on every run).
        for end in (0..pdf.len()).step_by(997) {
            let _ = crate::read(&pdf[..end]);
        }
        let mut seed: u32 = 2_166_136_261;
        for _ in 0..200 {
            let mut broken = pdf.clone();
            for _ in 0..8 {
                seed = seed.wrapping_mul(16_777_619) ^ 0x9e37_79b9;
                let at = seed as usize % broken.len();
                broken[at] = (seed >> 24) as u8;
            }
            let _ = crate::read(&broken);
        }
    }

    /// A page of the sentence `text` over the table's heads.
    fn sentence_page(text: &str) -> Page {
        let run = |x: f64, y: f64, bold: bool, italic: bool, text: &str| Run { x, end: x + 5.0 * text.len() as f64, y, size: 10.0, bold, italic, text: text.to_string() };
        let mut runs = vec![run(56.0, 600.0, false, false, text), run(56.0, 588.0, false, true, "is currently enrolled in the Master programme")];
        for (x, head) in [(56.0, "Modultitel"), (260.0, "Note"), (286.0, "Status"), (314.0, "Leistungspunkte"), (382.0, "Versuch"), (423.0, "Semester"), (479.0, "Datum"), (515.0, "Anerkennung")] {
            runs.push(run(x, 423.0, true, false, head));
        }
        Page { runs, rules: Vec::new() }
    }

    #[test]
    fn the_sentence_in_its_forms() {
        let read = |text: &str| {
            let page = sentence_page(text);
            let mut transcript = Transcript::default();
            sentence(&page, &layout(&page).unwrap(), &mut transcript);
            (transcript.degree, transcript.program, transcript.profile, transcript.since)
        };
        assert_eq!(
            read("Erika Mustermann, geb. am 1. Mai 2000 in Cottbus, studiert seit dem 1. April 2025 im Master-Studiengang Wirtschaftsingenieurwesen und erzielte bisher folgende Leistungen:"),
            (Some("Master".into()), Some("Wirtschaftsingenieurwesen".into()), None, Some(Date { year: 2025, month: 4, day: 1 }))
        );
        assert_eq!(read("Ohne Satz.").1, None);
        // No date of birth is read for the Studienbeginn.
        assert_eq!(read("Erika Mustermann, geb. am 12. März 2003, studiert seit 15. Oktober 2021 im Bachelor-Studiengang Informatik (universitäres Profil)").3, Some(Date { year: 2021, month: 10, day: 15 }));
    }

    #[test]
    fn terms_and_dates() {
        assert_eq!(Term::parse("WS 2024/25"), Some(Term { year: 2024, winter: true }));
        assert_eq!(Term::parse("SoSe 2023"), Some(Term { year: 2023, winter: false }));
        assert_eq!(Term::parse("2023"), None);
        assert_eq!(Term::from_code("2024W").map(Term::code).as_deref(), Some("2024W"));
        assert_eq!(Date { year: 2025, month: 3, day: 25 }.term(), Term { year: 2024, winter: true });
        assert_eq!(Date { year: 2025, month: 9, day: 30 }.term(), Term { year: 2025, winter: false });
        assert_eq!(date("18.09.2023"), Some(Date { year: 2023, month: 9, day: 18 }));
    }
}
