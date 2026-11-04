//! The Gesamtplan as a roadmap (owner, 2026-11-03, on folia/design/gesamtplan/konzepte.html:
//! „Option a ist perfekt bitte setze das um"; asked for as: „so dass es sich eher wie ein progress
//! tracker verhält. Halt so ähnlich wie bei star citizen … Idealer weise trotzdem noch so, dass man
//! die module sehen kann. Also zummindest die abkürzungen", „die progress bars können auch so sein,
//! dass sie unterbrochen sind, wenn man mal ein semester kein modul belegt").
//!
//! The areas as rows, the semesters as columns under the band of the study years, the line of
//! today through the current one. An area is one bar over the semesters, broken where it has
//! nothing: a piece per module with its abbreviation in the program (`ModuleAbbrevsAsk`), as wide
//! as its share of the semester's credits and never narrower than its label; where the labels do
//! not fit beside each other the semester says „4 Module · 24 LP" (`dom::fit_pieces`). The fuller,
//! the further: passed full in the area's tone, what runs strong, what is planned light, what the
//! plan has open dashed (a click plans it), not passed red with ✕, a retake with ↻, not offered ⚠,
//! beyond need hatched. ▸ unfolds an area into a lane per module, a retake tied to its first try by
//! an arrow; „Alle aufklappen" and „Alle zuklappen" over the plan, what is unfolded kept
//! (`OPEN_KEY`). A row of the plan that a module fills is carried by that module („für
//! „Anwendungsfach“"). The semesters before the current one stand open; „Bisher zuklappen" folds
//! them into one column. A click on a module (or a right click) opens its menu (`menu.rs`), a
//! module dragged into another column moves there, „+" in a cell opens the workbench for that
//! semester at that area (`bench.rs`). A desktop's view; until Folia 1.69.0 its cells listed the
//! modules' names (the mockup's concept C, owner 2026-10-04).

use std::collections::{BTreeMap, BTreeSet};

use folia_calendar::day::Day;
use folia_calendar::semester::SemesterKey;
use folia_model::labels::{ModuleKind, Season};
use folia_pages::ask::ModuleAbbrevsAsk;
use folia_plans::study::{Item, Pick, Subject, When};
use leptos::prelude::*;

use folia_design::nav;
use folia_design::ui::Icon;

use super::bench::{self, BenchPick};
use super::dom;
use super::focus::add_pick;
use super::menu::{self, Act, MenuFor};
use super::picker::short_title;
use super::{n, Dialog, Ready, StudyCtx};
use crate::i18n::{self, Texts};

/// Where the browser keeps which areas are unfolded (`localStorage`, a view setting as the view of
/// the semesters is): their names, one a line.
const OPEN_KEY: &str = "betula.study.open";
/// The grid's panel: what `dom::fit_pieces` measures in, and what a change of width is watched on.
const PANEL_ID: &str = "st-grid";
/// What a module without credits takes of its semester's bar, as if it had 4.
const NO_CREDITS: f64 = 4.0;

/// How far a piece is: its fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fill {
    Passed,
    Failed,
    /// Planned into the current semester.
    Running,
    Planned,
    /// A row of the plan without a module (a placeholder).
    Row,
    /// What the plan has open there (`study::open_lines`).
    Open,
}

impl Fill {
    fn class(self) -> &'static str {
        match self {
            Fill::Passed => "passed",
            Fill::Failed => "failed",
            Fill::Running => "running",
            Fill::Planned => "planned",
            // Not `row`: that is the catalog's list row (app.css), whose grid and height it took
            // until Folia 1.70.1.
            Fill::Row => "is-ph",
            Fill::Open => "open",
        }
    }
}

/// A module, a row of the plan or what the plan has open, as a piece of an area's bar.
#[derive(Clone, Debug, PartialEq)]
struct Piece {
    /// `Item::key`; empty for what the plan has open.
    key: String,
    /// The semester it is in (or would be planned into).
    semester: SemesterKey,
    /// The abbreviation; a row's first word; „+10" for open rows of the plan.
    label: String,
    /// What it is, for its title and its accessible name.
    title: String,
    /// Its share of the semester: its credits.
    grow: f64,
    fill: Fill,
    /// In the current semester (a placeholder there runs).
    now: bool,
    retake: bool,
    unoffered: bool,
    over: bool,
    /// A row of the plan (a placeholder, or one the plan has open): in its lane, whose name says
    /// which, it says only what it is („Platzhalter", „noch offen").
    row: bool,
    /// What a click on an open piece plans; empty where it stands for several rows (the click
    /// unfolds the area, whose lanes have them one by one).
    picks: Vec<Pick>,
}

/// A cell of an area's row.
#[derive(Clone, Debug, PartialEq)]
enum Cell {
    /// The semesters before the current one, folded: what is passed there, how much was not.
    Past { passed: f64, any_passed: bool, failed: usize, title: String },
    Semester(SemesterCell),
}

#[derive(Clone, Debug, PartialEq)]
struct SemesterCell {
    key: SemesterKey,
    when: When,
    leave: bool,
    pieces: Vec<Piece>,
}

/// A lane of an unfolded area: a module (or a row of the plan, or what the plan has open) over the
/// semesters it is in.
#[derive(Clone, Debug, PartialEq)]
struct Lane {
    name: String,
    /// The name in full, for its title.
    title: String,
    credits: Option<String>,
    /// Per column: its piece there and what it says of itself („bestanden").
    bars: Vec<Option<(Piece, &'static str)>>,
    /// A retake: the columns its arrow runs through from the try before, and the one it ends at.
    through: Vec<usize>,
    into: Option<usize>,
}

/// A column of the grid: a semester, or the ones before the current one together.
#[derive(Clone, Debug, PartialEq)]
enum Column {
    Past { span: String, credits: String },
    Semester(Head),
}

/// The head of a semester's column.
#[derive(Clone, Debug, PartialEq)]
struct Head {
    key: SemesterKey,
    label: String,
    now: bool,
    past: bool,
    leave: bool,
    fs: Option<String>,
    figure: String,
    heavy: bool,
    load: (f64, f64),
}

/// The band over the heads: a study year over its semesters, or „Bisher".
#[derive(Clone, Debug, PartialEq)]
struct Band {
    label: String,
    span: usize,
}

/// An area's row.
#[derive(Clone, Debug, PartialEq)]
struct AreaRow {
    area: Option<usize>,
    name: String,
    tone: &'static str,
    figure: String,
    done: bool,
    over: bool,
    /// passed, planned, open, beyond need.
    bar: (f64, f64, f64, f64),
    cells: Vec<Cell>,
    lanes: Vec<Lane>,
}

#[derive(Clone, Debug, PartialEq)]
struct Layout {
    columns: Vec<Column>,
    bands: Vec<Band>,
    rows: Vec<AreaRow>,
    /// The column of the current semester and how much of it is gone by (0–1).
    today: Option<(usize, f64)>,
    /// The flag of the line of today: „heute, 03.11.".
    today_flag: String,
}

/// How much of semester `s` is gone by on `today`: 0 at its first day, 1 after its last.
fn gone_by(s: SemesterKey, today: Day) -> f64 {
    let (from, to) = s.bounds();
    let days = f64::from(to.0 - from.0 + 1);
    (f64::from(today.0 - from.0) + 0.5).clamp(0.0, days) / days
}

/// „1. Studienjahr · 2025/26" of semester `s` of a study begun in `start`: the years where the
/// study year begins in a winter, as the university's academic year does.
fn study_year(s: SemesterKey, start: SemesterKey, t: &Texts) -> String {
    let at = i32::from(s.index()) - i32::from(start.index());
    if at < 0 {
        return t.study.before_study.to_string();
    }
    let first = start.plus(at - at % 2).unwrap_or(s);
    let years = first.winter.then(|| format!("{}/{:02}", first.year, (first.year % 100 + 1) % 100));
    (t.study.study_year)(at / 2 + 1, years.as_deref())
}

/// A row of the plan's name without what such rows say first: „Modul aus dem Bereich Praktische
/// Mathematik" is „Praktische Mathematik" in its lane.
fn row_name(name: &str) -> &str {
    ["Modul aus dem Bereich ", "Module aus dem Bereich ", "Modul aus ", "Module aus "].iter().find_map(|lead| name.strip_prefix(lead)).unwrap_or(name)
}

/// What a piece says of a module without an abbreviation, or of a row of the plan: its name up to
/// the first colon or bracket where that is short, else its first word, cut („Proseminar…"); a
/// row's „Modul aus dem Bereich" goes first („Praktische…").
fn short_label(name: &str) -> String {
    let name = row_name(name);
    let head = name.split([':', '(', '/']).next().unwrap_or(name).trim();
    if head.chars().count() <= 14 {
        return head.to_string();
    }
    let first = head.split_whitespace().next().unwrap_or(head);
    if first.chars().count() <= 12 {
        format!("{first}…")
    } else {
        format!("{}…", first.chars().take(11).collect::<String>())
    }
}

/// The season of a semester as a row's marks say it: „WiSe", „SoSe".
fn season_word(s: SemesterKey, t: &Texts) -> &'static str {
    if s.winter { t.format.winter_short } else { t.format.summer_short }
}

/// What an item of semester `s` says of itself: its name, credits and state, and what Betula says
/// beside it, for its title and its accessible name.
fn title_of(ready: &Ready, item: &Item, s: SemesterKey, when: When, fills: Option<&str>, t: &Texts) -> String {
    let st = &t.study;
    let mut parts = vec![item.name.clone()];
    let credits = match &item.subject {
        Subject::Row { credits: Some(text), .. } => Some(folia_plans::plan::credits_in(text, t.locale)),
        _ => item.credits.map(|credits| n(credits, t)),
    };
    if let Some(credits) = credits {
        parts.push((st.credits)(&credits));
    }
    parts.push(if item.passed {
        st.bar_passed.to_string()
    } else if item.failed {
        match item.again {
            Some(later) => (st.failed_again)(&later.label(t.locale)),
            None => st.failed_open.to_string(),
        }
    } else if matches!(item.subject, Subject::Row { .. }) {
        st.placeholder.to_string()
    } else if when == When::Now {
        st.bar_running.to_string()
    } else {
        st.bar_planned.to_string()
    });
    if let Some(row) = fills {
        parts.push((st.fills_row)(row));
    }
    if let Some(before) = item.retake {
        parts.push((st.chip_retake)(&ready.fs_label(before, t).unwrap_or_else(|| before.short(t.locale))));
    }
    if item.unoffered && !item.passed {
        let only = match item.offer.only() {
            Some(Season::Winter) => format!(" · {}", st.only_winter),
            Some(Season::Summer) => format!(" · {}", st.only_summer),
            None => String::new(),
        };
        parts.push(format!("{}{only}", (st.unoffered)(season_word(s, t))));
    }
    if item.over > 0.0 {
        let required = item.area.and_then(|area| ready.study.progress.get(area)).map_or(0.0, |progress| progress.area.required);
        parts.push((st.over_full)(&ready.area_name(item.area, t), &n(required, t)));
    }
    parts.join(" · ")
}

/// The rows of the plan that modules fill in a semester (`Item::fillers`): a module's id and the
/// name of the row it fills.
fn filled_rows(items: &[Item]) -> BTreeMap<&str, &str> {
    items
        .iter()
        .filter(|item| matches!(item.subject, Subject::Row { .. }))
        .flat_map(|row| row.fillers.iter().map(move |id| (id.as_str(), row.name.as_str())))
        .collect()
}

/// An item as a piece: a row that modules fill is none (its modules carry it).
fn piece_of(ready: &Ready, item: &Item, s: SemesterKey, when: When, fills: Option<&str>, abbrevs: &BTreeMap<String, String>, t: &Texts) -> Option<Piece> {
    let row = matches!(item.subject, Subject::Row { .. });
    if row && !item.fillers.is_empty() {
        return None;
    }
    let label = match item.module_id() {
        Some(id) => abbrevs.get(id).cloned().unwrap_or_else(|| short_label(short_title(&item.name))),
        None if item.kind == Some(ModuleKind::Fues) => t.study.fues.to_string(),
        None => short_label(&item.name),
    };
    let fill = if item.passed {
        Fill::Passed
    } else if item.failed {
        Fill::Failed
    } else if row {
        Fill::Row
    } else if when == When::Now {
        Fill::Running
    } else {
        Fill::Planned
    };
    Some(Piece {
        key: item.key(),
        semester: s,
        label,
        title: title_of(ready, item, s, when, fills, t),
        grow: item.credits.unwrap_or(NO_CREDITS).max(1.0),
        fill,
        now: when == When::Now,
        retake: item.retake.is_some(),
        unoffered: item.unoffered && !item.passed,
        over: item.over > 0.0,
        row,
        picks: Vec::new(),
    })
}

/// What lane a piece's word is („bestanden", „Wiederholung", „noch offen").
fn word_of(piece: &Piece, t: &Texts) -> &'static str {
    let st = &t.study;
    match piece.fill {
        Fill::Passed => st.bar_passed,
        Fill::Failed => st.bar_failed,
        Fill::Row => st.placeholder,
        Fill::Open => st.bar_open,
        _ if piece.retake => st.bar_retake,
        Fill::Running => st.bar_running,
        Fill::Planned => st.bar_planned,
    }
}

impl Layout {
    fn of(ready: &Ready, fold: bool, abbrevs: &BTreeMap<String, String>, today: Option<Day>, t: &Texts) -> Self {
        let s = &t.study;
        let study = &ready.study;
        let past: Vec<_> = study.semesters.iter().filter(|semester| semester.when == When::Past).collect();
        let fold = fold && !past.is_empty();
        let shown: Vec<&folia_plans::study::Semester> = study.semesters.iter().filter(|semester| !(fold && semester.when == When::Past)).collect();

        // The columns: „Bisher" first where folded, then the semesters.
        let mut columns = Vec::new();
        if fold {
            let fs: Vec<u8> = past.iter().filter_map(|semester| semester.fs).collect();
            let span = match (fs.first(), fs.last()) {
                (Some(from), Some(to)) => (s.fs_span)(*from, *to),
                _ => String::new(),
            };
            columns.push(Column::Past { span, credits: (s.credits)(&n(past.iter().map(|semester| semester.passed).sum(), t)) });
        }
        for semester in &shown {
            let heavy = semester.planned.is_some_and(|plan| semester.credits > plan + 0.5);
            let figure = match (semester.when, semester.planned) {
                (When::Past, _) => format!("{} / {}", n(semester.passed, t), (s.credits)(&n(semester.credits, t))),
                (_, Some(plan)) => format!("{} / {}", n(semester.credits, t), (s.credits)(&n(plan, t))),
                (_, None) => (s.credits)(&n(semester.credits, t)),
            };
            let load = match semester.when {
                When::Past => (semester.passed, semester.credits.max(1.0)),
                _ => (semester.credits, semester.planned.unwrap_or(semester.credits).max(semester.credits).max(1.0)),
            };
            columns.push(Column::Semester(Head {
                key: semester.key,
                label: semester.key.short(t.locale),
                now: semester.when == When::Now,
                past: semester.when == When::Past,
                leave: semester.leave,
                fs: ready.fs_label(semester.key, t),
                figure,
                heavy,
                load,
            }));
        }

        // The band of the study years over the heads.
        let mut bands: Vec<Band> = Vec::new();
        for column in &columns {
            let label = match column {
                Column::Past { .. } => s.so_far.to_string(),
                Column::Semester(head) => study_year(head.key, study.start, t),
            };
            match bands.last_mut() {
                Some(band) if band.label == label => band.span += 1,
                _ => bands.push(Band { label, span: 1 }),
            }
        }

        let day = today;
        let today = day.and_then(|day| {
            let at = columns.iter().position(|column| matches!(column, Column::Semester(head) if head.now))?;
            Some((at, gone_by(study.now, day)))
        });
        let today_flag = day.filter(|_| today.is_some()).map_or_else(String::new, |day| (s.today_at)(&day.day_month(t.locale)));

        // What each area still asks for that is not planned: the open rows of the plan say no more.
        let mut open_left: Vec<f64> = study.progress.iter().map(|progress| progress.open()).collect();
        let mut areas: Vec<Option<usize>> = (0..study.progress.len()).map(Some).collect();
        let outside = study.semesters.iter().any(|semester| semester.items.iter().any(|item| item.area.is_none()));
        if outside || study.progress.is_empty() {
            areas.push(None);
        }

        let rows = areas
            .into_iter()
            .map(|area| {
                let mut cells = Vec::new();
                // The lanes by `Item::key`, in the order they first come.
                let mut lanes: Vec<(String, Lane)> = Vec::new();
                let columns_n = columns.len();
                let lane_of = |lanes: &mut Vec<(String, Lane)>, key: String, name: &str, row: bool, credits: Option<String>| -> usize {
                    if let Some(at) = lanes.iter().position(|(known, _)| *known == key) {
                        return at;
                    }
                    let short = short_title(if row { row_name(name) } else { name });
                    lanes.push((key, Lane { name: short.to_string(), title: name.to_string(), credits, bars: vec![None; columns_n], through: Vec::new(), into: None }));
                    lanes.len() - 1
                };
                if fold {
                    let items: Vec<&Item> = past.iter().flat_map(|semester| semester.items.iter()).filter(|item| item.area == area && item.fillers.is_empty()).collect();
                    let passed: f64 = items.iter().filter(|item| item.passed).filter_map(|item| item.credits).sum();
                    let failed = items.iter().filter(|item| item.failed).count();
                    let mut title = Vec::new();
                    if items.iter().any(|item| item.passed) {
                        title.push((s.past_cell)(&n(passed, t), &folia_design::format::modules(i64::try_from(items.iter().filter(|item| item.passed).count()).unwrap_or(0), t.locale)));
                    }
                    if failed > 0 {
                        title.push((s.past_failed)(failed));
                    }
                    cells.push(Cell::Past { passed, any_passed: items.iter().any(|item| item.passed), failed, title: title.join(" · ") });
                    // In the lanes the folded semesters are one column too: the last try counts.
                    for semester in &past {
                        let fills = filled_rows(&semester.items);
                        for item in semester.items.iter().filter(|item| item.area == area) {
                            let Some(piece) = piece_of(ready, item, semester.key, When::Past, item.module_id().and_then(|id| fills.get(id).copied()), abbrevs, t) else { continue };
                            let at = lane_of(&mut lanes, item.key(), &item.name, piece.row, piece_credits(item, t));
                            let word = word_of(&piece, t);
                            if let Some(slot) = lanes.get_mut(at).and_then(|(_, lane)| lane.bars.first_mut()) {
                                if slot.as_ref().is_none_or(|(before, _)| before.fill != Fill::Passed) {
                                    *slot = Some((piece, word));
                                }
                            }
                        }
                    }
                }
                let first = usize::from(fold);
                for (offset, semester) in shown.iter().enumerate() {
                    let column = first + offset;
                    let fills = filled_rows(&semester.items);
                    let mut pieces = Vec::new();
                    for item in semester.items.iter().filter(|item| item.area == area) {
                        let Some(piece) = piece_of(ready, item, semester.key, semester.when, item.module_id().and_then(|id| fills.get(id).copied()), abbrevs, t) else { continue };
                        let at = lane_of(&mut lanes, item.key(), &item.name, piece.row, piece_credits(item, t));
                        set_bar(&mut lanes, at, column, (piece.clone(), word_of(&piece, t)));
                        pieces.push(piece);
                    }
                    // What the plan has open here: a module by its abbreviation, the rows of an area
                    // together by what the area still asks for („+10"), none of them where it asks
                    // for nothing more.
                    let lines: Vec<&folia_plans::study::Line> = ready.lines.iter().filter(|line| line.suggestion.area == area && line.semester == semester.key).collect();
                    let mut rows_open: Vec<&folia_plans::study::Line> = Vec::new();
                    let mut row_lanes: Vec<String> = Vec::new();
                    for line in lines {
                        let title = match line.failed_in {
                            Some(failed) => (s.line_retake)(&failed.label(t.locale)),
                            None => (s.line_title)(&line.plan_fs.map(s.fs).unwrap_or_default()),
                        };
                        let credits = line.suggestion.credits_text.clone().map(|text| folia_plans::plan::credits_in(&text, t.locale)).or_else(|| line.suggestion.credits.map(|credits| n(credits, t)));
                        let piece = |label: String| Piece {
                            key: String::new(),
                            semester: semester.key,
                            label,
                            title: format!("{} · {}", line.suggestion.name, title),
                            grow: line.suggestion.credits.unwrap_or(NO_CREDITS).max(1.0),
                            fill: Fill::Open,
                            now: semester.when == When::Now,
                            retake: line.failed_in.is_some(),
                            unoffered: !line.suggestion.offer.offered(semester.key),
                            over: false,
                            row: matches!(line.suggestion.pick, Pick::Row { .. }),
                            picks: vec![line.suggestion.pick.clone()],
                        };
                        match &line.suggestion.pick {
                            Pick::Module { id, .. } => {
                                let open = piece(abbrevs.get(id).cloned().unwrap_or_else(|| short_label(short_title(&line.suggestion.name))));
                                let at = lane_of(&mut lanes, format!("open:{id}"), &line.suggestion.name, false, credits);
                                set_bar(&mut lanes, at, column, (open.clone(), s.bar_open));
                                pieces.push(open);
                            }
                            Pick::Row { caption, ord } => {
                                let open = piece(short_label(&line.suggestion.name));
                                let lane = format!("open:{caption}:{ord}:{}", semester.key.key());
                                let at = lane_of(&mut lanes, lane.clone(), &line.suggestion.name, true, credits);
                                set_bar(&mut lanes, at, column, (open, s.bar_open));
                                rows_open.push(line);
                                row_lanes.push(lane);
                            }
                        }
                    }
                    if !rows_open.is_empty() {
                        let wanted: f64 = rows_open.iter().filter_map(|line| line.suggestion.credits).sum();
                        let left = area.and_then(|area| open_left.get_mut(area));
                        let credits = match left {
                            Some(left) => {
                                let take = if wanted > 0.0 { wanted.min(*left) } else { *left };
                                *left -= take;
                                take
                            }
                            None => wanted,
                        };
                        if credits > 0.0 {
                            let names: Vec<&str> = rows_open.iter().map(|line| line.suggestion.name.as_str()).collect();
                            let click = if rows_open.len() > 1 { format!(" · {}", s.open_rows_click) } else { String::new() };
                            pieces.push(Piece {
                                key: String::new(),
                                semester: semester.key,
                                label: format!("+{}", n(credits, t)),
                                title: format!("{} · {}{click}", (s.open_credits)(&(s.credits)(&n(credits, t))), names.join(", ")),
                                grow: credits,
                                fill: Fill::Open,
                                now: semester.when == When::Now,
                                retake: false,
                                unoffered: false,
                                over: false,
                                row: true,
                                picks: match rows_open.as_slice() {
                                    [one] => vec![one.suggestion.pick.clone()],
                                    _ => Vec::new(),
                                },
                            });
                        } else {
                            // Covered already: their lanes go too.
                            lanes.retain(|(key, _)| !row_lanes.contains(key));
                        }
                    }
                    cells.push(Cell::Semester(SemesterCell { key: semester.key, when: semester.when, leave: semester.leave, pieces }));
                }

                // A retake's arrow: from the column it was not passed in to the one it is taken
                // again in.
                let mut lanes: Vec<Lane> = lanes.into_iter().map(|(_, lane)| lane).collect();
                for lane in &mut lanes {
                    let failed = lane.bars.iter().position(|bar| bar.as_ref().is_some_and(|(piece, _)| piece.fill == Fill::Failed));
                    let again = lane.bars.iter().enumerate().skip(failed.map_or(usize::MAX, |at| at + 1)).find(|(_, bar)| bar.as_ref().is_some_and(|(piece, _)| piece.retake)).map(|(at, _)| at);
                    if let (Some(from), Some(to)) = (failed, again) {
                        lane.through = (from + 1..to).collect();
                        lane.into = Some(to);
                    }
                }

                match area.and_then(|i| study.progress.get(i)) {
                    Some(progress) => {
                        let counted = progress.passed + progress.planned + progress.over;
                        AreaRow {
                            area,
                            name: ready.area_name(area, t),
                            tone: ready.tone(area),
                            figure: format!("{}/{}", n(counted, t), n(progress.area.required, t)),
                            done: progress.passed + 0.01 >= progress.area.required && progress.area.required > 0.0,
                            over: progress.over > 0.0,
                            bar: (progress.passed, progress.planned, progress.open(), progress.over),
                            cells,
                            lanes,
                        }
                    }
                    None => {
                        let (passed, planned) = study.outside;
                        AreaRow { area: None, name: s.outside.to_string(), tone: "var(--text-3)", figure: n(passed + planned, t), done: false, over: false, bar: (passed, planned, 0.0, 0.0), cells, lanes }
                    }
                }
            })
            .collect();
        Layout { columns, bands, rows, today, today_flag }
    }
}

/// Puts `bar` into column `column` of lane `at`: the lane's piece there and its word.
fn set_bar(lanes: &mut [(String, Lane)], at: usize, column: usize, bar: (Piece, &'static str)) {
    if let Some(slot) = lanes.get_mut(at).and_then(|(_, lane)| lane.bars.get_mut(column)) {
        *slot = Some(bar);
    }
}

/// What a module's lane says of its credits: the plan's text for a row („10–24"), else its credits.
fn piece_credits(item: &Item, t: &Texts) -> Option<String> {
    match &item.subject {
        Subject::Row { credits: Some(text), .. } => Some(folia_plans::plan::credits_in(text, t.locale)),
        _ => item.credits.map(|credits| n(credits, t)),
    }
}

/// An area's tone as the style of what is drawn in it: `--c`, and on the sun's full surface near
/// black text (`--c-on`; on the other tones the panel's colour, white by day).
fn tone_style(tone: &str) -> String {
    if tone == "var(--t-sun)" {
        format!("--c: {tone}; --c-on: var(--rm-on-sun)")
    } else {
        format!("--c: {tone}")
    }
}

/// The class of a column's cells: the semesters that are over grey, the current one green, one of
/// leave hatched.
fn column_class(column: &Column) -> &'static str {
    match column {
        Column::Past { .. } => "past",
        Column::Semester(head) if head.leave => "leave",
        Column::Semester(head) if head.now => "is-now",
        Column::Semester(head) if head.past => "past",
        Column::Semester(_) => "",
    }
}

/// The areas unfolded, as the browser keeps them.
fn kept_open() -> BTreeSet<String> {
    nav::local_get(OPEN_KEY).map(|text| text.lines().map(str::to_string).filter(|name| !name.is_empty()).collect()).unwrap_or_default()
}

#[component]
pub(super) fn Grid(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    // The semesters before the current one stand open: with bars they are narrow and say what is
    // done already.
    let fold = RwSignal::new(false);
    let open = RwSignal::new(kept_open());
    let today = crate::studyplan::today();
    // The modules' abbreviations in the program: of every module planned, passed or open in the plan.
    let abbrevs = Memo::new(move |before: Option<&BTreeMap<String, String>>| {
        let ask = ctx.with_ready(|ready| {
            let mut ids: Vec<String> = ready.study.semesters.iter().flat_map(|semester| semester.items.iter().filter_map(|item| item.module_id().map(str::to_string))).collect();
            ids.extend(ready.lines.iter().filter_map(|line| match &line.suggestion.pick {
                Pick::Module { id, .. } => Some(id.clone()),
                Pick::Row { .. } => None,
            }));
            ids.sort();
            ids.dedup();
            ModuleAbbrevsAsk { ids, program: Some(ready.program.id.clone()) }
        });
        let Some(ask) = ask.filter(|ask| !ask.ids.is_empty()) else { return BTreeMap::new() };
        let Some(now) = ctx.source.with_value(|source| source.as_ref().map(|source| source.now(&ask))) else { return BTreeMap::new() };
        folia_pages::ask::unless_pending(now, before, |now| now.unwrap_or_default())
    });
    // The study before what is made of it (R16).
    let layout = Memo::new(move |_| {
        let fold = fold.get();
        ctx.with_ready(|ready| abbrevs.with(|abbrevs| Layout::of(ready, fold, abbrevs, today, t)))
    });
    let any_past = Memo::new(move |_| ctx.with_ready(|ready| ready.study.semesters.iter().any(|semester| semester.when == When::Past)).unwrap_or(false));
    let names = Memo::new(move |_| layout.with(|layout| layout.as_ref().map(|layout| layout.rows.iter().map(|row| row.name.clone()).collect::<Vec<_>>()).unwrap_or_default()));
    let keep = move |names: BTreeSet<String>| {
        nav::local_set(OPEN_KEY, &names.iter().cloned().collect::<Vec<_>>().join("\n"));
        open.set(names);
    };
    // Whether the pieces fit beside each other: measured after every change and whenever the
    // grid's width changes (the window, the sidebar).
    Effect::new(move |_| {
        layout.track();
        open.track();
        request_animation_frame(|| dom::fit_pieces(PANEL_ID));
    });
    Effect::new(move |_| {
        let watch = nav::watch_size(PANEL_ID, || dom::fit_pieces(PANEL_ID));
        on_cleanup(move || drop(watch));
    });
    view! {
        <section class="panel st-grid-panel st-rm" id=PANEL_ID aria-label=s.grid_title>
            <div class="st-sems-head">
                <h2>{s.grid_title}</h2>
                <span class="st-grow"></span>
                <button class="st-link st-rm-all" type="button" on:click=move |_| keep(names.get_untracked().into_iter().collect())><Icon name="chevrons-down"/>{s.expand_all}</button>
                <button class="st-link st-rm-all" type="button" on:click=move |_| keep(BTreeSet::new())><Icon name="chevrons-up"/>{s.collapse_all}</button>
                {move || any_past.get().then(|| view! {
                    <button class="st-link" type="button" aria-expanded=move || if fold.get() { "false" } else { "true" } on:click=move |_| fold.update(|fold| *fold = !*fold)>
                        {move || if fold.get() { s.expand_past } else { s.collapse_past }}
                    </button>
                })}
            </div>
            {move || layout.get().map(|layout| {
                let past_column = layout.columns.iter().any(|column| matches!(column, Column::Past { .. }));
                let semesters = layout.columns.len() - usize::from(past_column);
                let today_at = layout.today.map(|(at, _)| at);
                let style = format!(
                    "grid-template-columns: 196px {}repeat({semesters}, minmax(140px, 1fr)); --today: {}",
                    if past_column { "96px " } else { "" },
                    layout.today.map_or(0.0, |(_, gone)| gone),
                );
                let columns = layout.columns.clone();
                let kinds: Vec<&'static str> = columns.iter().map(column_class).collect();
                let opened = open.get();
                view! {
                    <div class="st-grid-scroll">
                        <div class="st-grid st-rm-grid" style=style>
                            <div class="st-rm-corner"></div>
                            {layout.bands.iter().map(|band| view! { <div class="st-rm-band" style=format!("grid-column: span {}", band.span)>{band.label.clone()}</div> }).collect_view()}
                            <div class="st-grid-corner"></div>
                            {columns.iter().cloned().enumerate().map(|(at, column)| head_view(ctx, column, today_at == Some(at), t)).collect_view()}
                            {layout.rows.into_iter().map(|row| {
                                let unfolded = opened.contains(&row.name);
                                row_view(ctx, row, unfolded, today_at, &kinds, open, keep, fold, t)
                            }).collect_view()}
                            {today_at.map(|at| view! {
                                <div class="st-rm-foot rm-now" style=format!("grid-column: {}", at + 2)>
                                    <span class="st-rm-flag">{layout.today_flag.clone()}</span>
                                </div>
                            })}
                        </div>
                    </div>
                }
            })}
            <p class="st-grid-legend st-rm-legend">
                <span><i class="st-rm-sw passed"></i>{s.legend_passed}</span>
                <span><i class="st-rm-sw running"></i>{s.legend_running}</span>
                <span><i class="st-rm-sw planned"></i>{s.legend_planned}</span>
                <span><i class="st-rm-sw open"></i>{s.legend_plan_open}</span>
                <span><Icon name="x" class="no"/>{s.legend_failed}</span>
                <span><Icon name="repeat"/>{s.legend_retake}</span>
                <span class="warn"><Icon name="triangle-alert"/>{s.legend_offer}</span>
                <span><i class="st-rm-sw over"></i>{s.legend_over}</span>
                <span><i class="st-rm-sw today"></i>{s.legend_today}</span>
                <span class="st-grow"></span>
                <span>{s.grid_help}</span>
            </p>
        </section>
    }
}

/// A semester's head: its name, Fachsemester, figure and load; a click shows it as the semester.
fn head_view(ctx: StudyCtx, column: Column, now_line: bool, t: &'static Texts) -> AnyView {
    let s = &t.study;
    match column {
        Column::Past { span, credits } => view! {
            <div class="st-col-head past"><div class="st-col-name">{s.so_far}</div><div class="st-sub"><span>{span}</span><b class="ok">"✓ "{credits}</b></div></div>
        }
        .into_any(),
        Column::Semester(head) => {
            let key = head.key;
            view! {
                <button type="button" class="st-col-head" class:is-now=head.now class:past=head.past class:rm-now=now_line on:click=move |_| {
                    ctx.focus.set(Some(key));
                    ctx.view.set(super::View::Semester);
                }>
                    <div class="st-col-name">{head.label}{head.now.then(|| view! { <span class="st-now">{s.now}</span> })}</div>
                    <div class="st-sub"><span>{head.fs}</span><b class:heavy=head.heavy>{head.figure}</b></div>
                    <span class="st-load"><span style=format!("flex-grow: {}", head.load.0)></span><span style=format!("flex-grow: {}", (head.load.1 - head.load.0).max(0.0))></span></span>
                </button>
            }
            .into_any()
        }
    }
}

/// An area: its head and its bar over the semesters, then its lanes where it is unfolded.
#[allow(clippy::too_many_arguments)]
fn row_view(ctx: StudyCtx, row: AreaRow, unfolded: bool, today_at: Option<usize>, kinds: &[&'static str], open: RwSignal<BTreeSet<String>>, keep: impl Fn(BTreeSet<String>) + Copy + 'static, fold: RwSignal<bool>, t: &'static Texts) -> AnyView {
    let s = &t.study;
    let (passed, planned, rest, over) = row.bar;
    let area = row.area;
    let name = row.name.clone();
    let toggle = {
        let name = name.clone();
        move |_| {
            let mut now = open.get_untracked();
            if !now.remove(&name) {
                now.insert(name.clone());
            }
            keep(now);
        }
    };
    let unfold = {
        let name = name.clone();
        move || {
            let mut now = open.get_untracked();
            now.insert(name.clone());
            keep(now);
        }
    };
    let figure = if row.done {
        view! { <span class="st-card-lp done"><Icon name="check"/>{row.figure.clone()}</span> }.into_any()
    } else {
        view! { <span class="st-card-lp" class:over=row.over>{row.figure.clone()}</span> }.into_any()
    };
    let label = if unfolded { (s.collapse_area)(&name) } else { (s.expand_area)(&name) };
    let last_lane = row.lanes.len();
    let cells = row.cells.clone();
    view! {
        <div class="st-row-head st-rm-head" class:is-open=unfolded style=tone_style(row.tone)>
            <button type="button" class="st-rm-tog" aria-expanded=if unfolded { "true" } else { "false" } aria-label=label on:click=toggle><Icon name="chevron-right"/></button>
            <button type="button" class="st-rm-area" on:click=move |_| ctx.open(Dialog::Area(area))>
                <span class="st-card-head"><span class="st-dot"></span><span class="st-card-name">{row.name.clone()}</span>{figure}</span>
                <span class="st-mini">
                    <span class="passed" style=format!("flex-grow: {passed}")></span>
                    <span class="planned" style=format!("flex-grow: {planned}")></span>
                    <span class="open" style=format!("flex-grow: {rest}")></span>
                    <span class="over" style=format!("flex-grow: {over}")></span>
                </span>
            </button>
        </div>
        {cells.into_iter().enumerate().map(|(at, cell)| cell_view(ctx, cell, area, &name, row.tone, today_at == Some(at), unfold.clone(), fold, t)).collect_view()}
        {unfolded.then(|| row.lanes.into_iter().enumerate().map(|(i, lane)| lane_view(ctx, lane, row.tone, today_at, kinds, i + 1 == last_lane, t)).collect_view())}
    }
    .into_any()
}

/// A cell of an area's bar: its pieces, „n Module" where they do not fit (`dom::fit_pieces`), and
/// „+" for the workbench.
#[allow(clippy::too_many_arguments)]
fn cell_view(ctx: StudyCtx, cell: Cell, area: Option<usize>, area_name: &str, tone: &'static str, now_line: bool, unfold: impl Fn() + Clone + 'static, fold: RwSignal<bool>, t: &'static Texts) -> AnyView {
    let s = &t.study;
    match cell {
        Cell::Past { passed, any_passed, failed, title } => view! {
            <div class="st-cell st-rm-cell past" style=tone_style(tone)>
                {(any_passed || failed > 0).then(|| view! {
                    <button type="button" class="st-rm-p stub" class:passed=any_passed class:failed=!any_passed title=title.clone() aria-label=title on:click=move |_| fold.set(false)>
                        {any_passed.then(|| view! { <Icon name="check"/><span class="lb">{n(passed, t)}</span> })}
                        {(failed > 0).then(|| view! { <span class="st-rm-badge">"✕ "{failed}</span> })}
                    </button>
                })}
            </div>
        }
        .into_any(),
        Cell::Semester(cell) => {
            let semester = cell.key;
            let over = RwSignal::new(false);
            let drop = move |ev: leptos::ev::DragEvent| {
                ev.prevent_default();
                over.set(false);
                if let Some((from, keys)) = ctx.drag.get_untracked() {
                    ctx.drag.set(None);
                    menu::act(ctx, Act::Move { semester: from, keys, to: semester }, t);
                }
            };
            let real: Vec<&Piece> = cell.pieces.iter().filter(|piece| piece.fill != Fill::Open).collect();
            let open: Vec<&Piece> = cell.pieces.iter().filter(|piece| piece.fill == Fill::Open).collect();
            let empty = cell.pieces.is_empty();
            // What stands for them all where they do not fit: „4 Module · 24 LP", what is open
            // beside it.
            let sum_fill = if !real.is_empty() && real.iter().all(|piece| piece.fill == Fill::Passed) {
                Fill::Passed
            } else if !real.is_empty() && real.iter().all(|piece| piece.fill == Fill::Row) {
                // Rows of the plan alone: dashed, as they are.
                Fill::Row
            } else if cell.when == When::Now {
                Fill::Running
            } else if cell.when == When::Past {
                Fill::Passed
            } else {
                Fill::Planned
            };
            let failed = real.iter().filter(|piece| piece.fill == Fill::Failed).count();
            let credits: f64 = real.iter().map(|piece| piece.grow).sum();
            // The modules by their number, a row of the plan without one as what it is: „3 Module",
            // „2 Platzhalter", „1 Modul · 1 Platzhalter".
            let places = real.iter().filter(|piece| piece.fill == Fill::Row).count();
            let modules = real.len() - places;
            let count = match (modules, places) {
                (_, 0) => folia_design::format::modules(i64::try_from(modules).unwrap_or(0), t.locale),
                (0, _) => (s.n_placeholders)(places),
                _ => format!("{} · {}", folia_design::format::modules(i64::try_from(modules).unwrap_or(0), t.locale), (s.n_placeholders)(places)),
            };
            let sum_title = real.iter().map(|piece| format!("{} · {}", piece.label, piece.title)).collect::<Vec<_>>().join("\n");
            let open_title = open.iter().map(|piece| piece.title.clone()).collect::<Vec<_>>().join("\n");
            let open_count = open.len();
            // One open piece says what it is („+10", „BS1"), several how many they are.
            let open_label = match open.as_slice() {
                [one] => one.label.clone(),
                _ => (s.n_open)(open_count),
            };
            let sum_credits = (s.credits)(&n(credits, t));
            let pieces = cell.pieces.clone();
            let for_cell = (s.add_in_cell_for)(area_name, &semester.label(t.locale));
            let unfold_sum = unfold.clone();
            let unfold_open = unfold.clone();
            view! {
                <div
                    class="st-cell st-rm-cell"
                    class:is-now=cell.when == When::Now
                    class:past=cell.when == When::Past
                    class:leave=cell.leave
                    class:rm-now=now_line
                    class:is-drop=move || over.get()
                    style=tone_style(tone)
                    on:dragover=move |ev: leptos::ev::DragEvent| if ctx.drag.with_untracked(|drag| drag.as_ref().is_some_and(|(from, _)| *from != semester)) { ev.prevent_default(); over.set(true); }
                    on:dragleave=move |_| over.set(false)
                    on:drop=drop
                >
                    {(!empty).then(|| view! {
                        <div class="st-rm-seg">
                            {pieces.into_iter().map(|piece| piece_view(ctx, piece, semester, unfold.clone(), t)).collect_view()}
                        </div>
                        <div class="st-rm-sum">
                            {(!real.is_empty()).then(|| view! {
                                <button type="button" class=format!("st-rm-p sum {}{}", sum_fill.class(), if cell.when == When::Now { " now" } else { "" }) title=sum_title.clone() aria-label=format!("{count} · {sum_credits}") on:click=move |_| unfold_sum()>
                                    <span class="lb">{count.clone()}<span class="st-rm-lp">" · "{sum_credits.clone()}</span></span>
                                    {(failed > 0).then(|| view! { <span class="st-rm-badge">"✕ "{failed}</span> })}
                                </button>
                            })}
                            {(open_count > 0).then(|| view! {
                                <button type="button" class="st-rm-p open sum-open" title=open_title.clone() aria-label=open_title on:click=move |_| unfold_open()>
                                    {(open_count > 1).then(|| view! { <Icon name="plus"/> })}<span class="lb">{open_label.clone()}</span>
                                </button>
                            })}
                        </div>
                    })}
                    <button type="button" class="st-cell-add" class:st-rm-add=!empty title=for_cell.clone() aria-label=for_cell on:click=move |_| bench::enter(ctx, semester, area.map_or(BenchPick::Plan, BenchPick::Area))>
                        <Icon name="plus"/>{empty.then_some(s.add_in_cell)}
                    </button>
                </div>
            }
            .into_any()
        }
    }
}

/// A piece of an area's bar: its menu, its drag; what the plan has open plans itself, or unfolds the
/// area where it stands for several rows.
fn piece_view(ctx: StudyCtx, piece: Piece, semester: SemesterKey, unfold: impl Fn() + Clone + 'static, t: &'static Texts) -> AnyView {
    let class = format!("st-rm-p {}{}{}", piece.fill.class(), if piece.now { " now" } else { "" }, if piece.over { " over" } else { "" });
    let style = format!("flex-grow: {}", piece.grow);
    let inner = piece_inner(&piece, None);
    if piece.fill == Fill::Open {
        let picks = piece.picks.clone();
        return view! {
            <button type="button" class=class style=style title=piece.title.clone() aria-label=format!("{} – {}", piece.label, piece.title) on:click=move |_| {
                if picks.is_empty() { unfold() } else { add_pick(ctx, semester, picks.clone(), t) }
            }>{inner}</button>
        }
        .into_any();
    }
    let key = StoredValue::new(piece.key.clone());
    view! {
        <button
            type="button"
            class=class
            style=style
            title=piece.title.clone()
            aria-label=format!("{}, {}", piece.label, piece.title)
            aria-haspopup="menu"
            draggable="true"
            on:dragstart=move |ev: leptos::ev::DragEvent| {
                let keys = vec![key.get_value()];
                dom::drag_data(&ev, &keys);
                ctx.drag.set(Some((semester, keys)));
            }
            on:dragend=move |_| ctx.drag.set(None)
            on:click=move |ev: leptos::ev::MouseEvent| menu::open_at_button(ctx, MenuFor::Item { semester, key: key.get_value(), view: true }, &ev)
            on:contextmenu=move |ev: leptos::ev::MouseEvent| {
                ev.prevent_default();
                menu::open_at_pointer(ctx, MenuFor::Item { semester, key: key.get_value(), view: true }, &ev);
            }
        >{inner}</button>
    }
    .into_any()
}

/// What a piece shows: ✕, + or ? before its label, ↻ and ⚠ after it; in a lane its word between,
/// a row of the plan's only its word (the lane's name says which row: „? Platzhalter").
fn piece_inner(piece: &Piece, word: Option<&'static str>) -> AnyView {
    let pre = match piece.fill {
        Fill::Failed => Some("x"),
        Fill::Open => Some(if piece.retake { "repeat" } else { "plus" }),
        Fill::Row => Some("circle-help"),
        _ => None,
    };
    let retake = piece.retake && piece.fill != Fill::Open;
    let check = word.is_some() && piece.fill == Fill::Passed;
    let (label, word) = match word {
        Some(word) if piece.row => (word.to_string(), None),
        _ => (piece.label.clone(), word),
    };
    view! {
        {pre.map(|name| view! { <Icon name=name/> })}
        <span class="lb">{label}{word.map(|word| view! { <span class="st-rm-w">{word}</span> })}</span>
        {retake.then(|| view! { <Icon name="repeat"/> })}
        {piece.unoffered.then(|| view! { <Icon name="triangle-alert" class="warn"/> })}
        {check.then(|| view! { <Icon name="check"/> })}
    }
    .into_any()
}

/// A lane: the module's name and credits, its piece in each semester it is in, a retake's arrow.
fn lane_view(ctx: StudyCtx, lane: Lane, tone: &'static str, today_at: Option<usize>, kinds: &[&'static str], last: bool, t: &'static Texts) -> AnyView {
    let (name, title) = (lane.name.clone(), lane.title.clone());
    view! {
        <div class="st-rm-lane" class:last=last style=tone_style(tone)>
            <span class="st-rm-lane-name" title=title>{name}</span>
            <span class="st-rm-lane-lp">{lane.credits.clone()}</span>
        </div>
        {lane.bars.into_iter().enumerate().map(|(at, bar)| {
            let through = lane.through.contains(&at);
            let into = lane.into == Some(at);
            view! {
                <div class=format!("st-rm-lc {}", kinds.get(at).copied().unwrap_or_default()) class:last=last class:rm-now=today_at == Some(at) class:link=through class:link-in=into style=tone_style(tone)>
                    {bar.map(|(piece, word)| lane_bar(ctx, piece, word, t))}
                </div>
            }
        }).collect_view()}
    }
    .into_any()
}

/// A module's piece in its lane: as in the bar, with what it is („bestanden").
fn lane_bar(ctx: StudyCtx, piece: Piece, word: &'static str, t: &'static Texts) -> AnyView {
    // A lane's piece is the bar's: the same menu and drag.
    let semester = piece.semester;
    let class = format!("st-rm-bar st-rm-p {}{}{}", piece.fill.class(), if piece.now { " now" } else { "" }, if piece.over { " over" } else { "" });
    let inner = piece_inner(&piece, Some(word));
    if piece.fill == Fill::Open {
        let picks = piece.picks.clone();
        return view! {
            <button type="button" class=class title=piece.title.clone() aria-label=format!("{} – {}", piece.label, piece.title) on:click=move |_| add_pick(ctx, semester, picks.clone(), t)>{inner}</button>
        }
        .into_any();
    }
    let key = StoredValue::new(piece.key.clone());
    view! {
        <button
            type="button"
            class=class
            title=piece.title.clone()
            aria-label=format!("{}, {}", piece.label, piece.title)
            aria-haspopup="menu"
            draggable="true"
            on:dragstart=move |ev: leptos::ev::DragEvent| {
                let keys = vec![key.get_value()];
                dom::drag_data(&ev, &keys);
                ctx.drag.set(Some((semester, keys)));
            }
            on:dragend=move |_| ctx.drag.set(None)
            on:click=move |ev: leptos::ev::MouseEvent| menu::open_at_button(ctx, MenuFor::Item { semester, key: key.get_value(), view: true }, &ev)
            on:contextmenu=move |ev: leptos::ev::MouseEvent| {
                ev.prevent_default();
                menu::open_at_pointer(ctx, MenuFor::Item { semester, key: key.get_value(), view: true }, &ev);
            }
        >{inner}</button>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    #[test]
    fn a_study_year_is_a_winter_and_the_summer_after_it() {
        let t = i18n::texts(folia_locale::Locale::De);
        assert_eq!(study_year(key("2025W"), key("2025W"), t), "1. Studienjahr · 2025/26");
        assert_eq!(study_year(key("2026S"), key("2025W"), t), "1. Studienjahr · 2025/26");
        assert_eq!(study_year(key("2026W"), key("2025W"), t), "2. Studienjahr · 2026/27");
        // A study begun in a summer: its years cross the university's, so they go unnamed.
        assert_eq!(study_year(key("2026W"), key("2026S"), t), "1. Studienjahr");
        assert_eq!(study_year(key("2027S"), key("2026S"), t), "2. Studienjahr");
        assert_eq!(study_year(key("2025S"), key("2025W"), t), "Vor dem Studium");
        let en = i18n::texts(folia_locale::Locale::En);
        assert_eq!(study_year(key("2028S"), key("2025W"), en), "Year 3 · 2027/28");
    }

    #[test]
    fn today_is_how_much_of_its_semester_is_gone() {
        let winter = key("2026W");
        assert!((gone_by(winter, Day::from_ymd(2026, 11, 3).unwrap()) - 33.5 / 182.0).abs() < 1e-9);
        assert_eq!(gone_by(winter, Day::from_ymd(2026, 9, 1).unwrap()), 0.0);
        assert_eq!(gone_by(winter, Day::from_ymd(2027, 5, 1).unwrap()), 1.0);
    }

    #[test]
    fn a_row_says_its_first_words() {
        assert_eq!(short_label("Anwendungsfach"), "Anwendungsfach");
        assert_eq!(short_label("Wahlpflicht: Komplex Grundlagen der Informatik / Komplex Praktische Informatik"), "Wahlpflicht");
        assert_eq!(short_label("Proseminar oder Praktikum"), "Proseminar…");
        assert_eq!(short_label("Fachübergreifendes Studium"), "Fachübergre…");
        assert_eq!(short_label("Modul aus dem Bereich Praktische Mathematik"), "Praktische…");
        assert_eq!(row_name("Modul aus dem Bereich Praktische Mathematik"), "Praktische Mathematik");
        assert_eq!(row_name("Anwendungsfach"), "Anwendungsfach");
    }
}
