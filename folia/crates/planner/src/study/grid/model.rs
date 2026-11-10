//! What the Gesamtplan shows, worked out from the study (`Layout::of`): its columns (the semesters,
//! „Bisher" where those before the current one are folded) under the band of the study years, and
//! its rows — each area of the progress with its bar over the semesters, its lanes (a module each,
//! the rows of the plan of one name one) and its subsections.
//!
//! An area's subsections are its subgroups as the workbench and the Wahlpflicht have them
//! (`folia_plans::bench`, two or more of them; owner, 2026-11-09: „subsections für bereiche, wie zum
//! beispiel bei informatik wo im fachstudium den bereichen praktische informatik, grundlagen der
//! informatik und technische informatik zugeordnet wird. Orientiere dich daran, wie die wp
//! bereiche das darstellen mit den requirements, also min 10lp max 24lp mit einem der bereiche mit
//! 30 erlaubten lp"): its Pflicht, each of its choices, a group's member with its room above its
//! most (`choices::Group::members`), a choice that others of the area cover none of them
//! (`Bench::shown`). Each has its own bar and its own lanes; what of the area falls in none of
//! them is its „Weitere". The view (`super`) and the picture (`super::picture`) draw it alike.

use std::collections::{BTreeMap, BTreeSet};

use folia_calendar::day::Day;
use folia_calendar::semester::SemesterKey;
use folia_model::labels::{ModuleKind, Season};
use folia_plans::bench::{self, Bench, GroupKind};
use folia_plans::choices::{self, PlanView, Room, RowState, Stand};
use folia_plans::study::{credits_range, Input, Item, Line, Pick, Semester, Subject, When};

use super::super::picker::short_title;
use super::super::{n, Ready};
use crate::i18n::Texts;

/// What a module without credits takes of its semester's bar, as if it had 4.
const NO_CREDITS: f64 = 4.0;
/// Less than this is no credit (sums of halves).
const EPSILON: f64 = 1e-6;
/// The column of the rows' names (px; app.css scrolls what is focused to beside it): wide enough
/// for a subsection's name, „Grundlagen der Informatik", beside its indent and its ▸.
const NAMES: usize = 212;

/// How far a piece is: its fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::study) enum Fill {
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
    pub(in crate::study) fn class(self) -> &'static str {
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

/// A module, a row of the plan or what the plan has open, as a piece of a bar.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::study) struct Piece {
    /// `Item::key`; empty for what the plan has open.
    pub key: String,
    /// The semester it is in (or would be planned into).
    pub semester: SemesterKey,
    /// The abbreviation; a row's first word; „+10" for open rows of the plan.
    pub label: String,
    /// What it is, for its title and its accessible name.
    pub title: String,
    /// Its share of the semester: its credits.
    pub grow: f64,
    pub fill: Fill,
    /// In the current semester (a placeholder there runs).
    pub now: bool,
    pub retake: bool,
    pub unoffered: bool,
    /// Beyond need: none of its credits count (`Item::spare`, or a choice's module counted by none,
    /// `PlanView::spare`).
    pub over: bool,
    /// The row of the plan it stands for (a placeholder, or one the plan has open): in its lane,
    /// whose name says which, it says only what it is („Platzhalter", „noch offen").
    pub row: Option<RowOf>,
    /// What a click on an open piece plans; empty where it stands for several rows (the click
    /// unfolds its row, whose lanes have them one by one).
    pub picks: Vec<Pick>,
}

/// A row of the plan as its lane counts it: which (its caption and `ord`) and its credits (low,
/// high). The rows of one name share a lane, which adds their credits up, each row once (two rows
/// „Anwendungsfach" of 6 are one lane of 12; owner, 2026-11-04: „warum gibt es hier zwei mal
/// anwendungsfach als ein eintrag mit jeweils 6 punkten. Das sollte einmal mit 12 sein").
#[derive(Clone, Debug, PartialEq)]
pub(in crate::study) struct RowOf {
    pub id: String,
    pub credits: Option<(f64, f64)>,
}

/// A cell of a row's bar.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::study) enum Cell {
    /// The semesters before the current one, folded: what is passed there, how much was not.
    Past { passed: f64, any_passed: bool, failed: usize, title: String },
    Semester(SemesterCell),
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::study) struct SemesterCell {
    pub key: SemesterKey,
    pub when: When,
    pub leave: bool,
    pub pieces: Vec<Piece>,
}

/// A lane of an unfolded row: a module, what the plan has open of it included, or the rows of the
/// plan of one name (`RowOf`), over the semesters they are in.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::study) struct Lane {
    pub name: String,
    /// The name in full, for its title.
    pub title: String,
    pub credits: Option<String>,
    /// Per column: its pieces there (one, but for two rows of its name in one semester), each with
    /// what it says of itself („bestanden").
    pub bars: Vec<Vec<(Piece, &'static str)>>,
    /// The keys of its pieces (`Item::key`): where a module dragged lands in it.
    pub keys: BTreeSet<String>,
    /// A retake: the columns its arrow runs through from the try before, and the one it ends at.
    pub through: Vec<usize>,
    pub into: Option<usize>,
}

/// A column of the grid: a semester, or the ones before the current one together.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::study) enum Column {
    Past { span: String, credits: String },
    Semester(Head),
}

impl Column {
    /// The semester of a column; none for „Bisher".
    pub(in crate::study) fn semester(&self) -> Option<SemesterKey> {
        match self {
            Column::Semester(head) => Some(head.key),
            Column::Past { .. } => None,
        }
    }
}

/// The head of a semester's column.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::study) struct Head {
    pub key: SemesterKey,
    pub label: String,
    pub now: bool,
    pub past: bool,
    pub leave: bool,
    pub fs: Option<String>,
    pub figure: String,
    pub heavy: bool,
    pub load: (f64, f64),
}

/// The band over the heads: a study year over its semesters, or „Bisher".
#[derive(Clone, Debug, PartialEq)]
pub(in crate::study) struct Band {
    pub label: String,
    pub span: usize,
}

/// An area's row: its head, its bar, and what it unfolds into — its subsections where it has
/// them, else its lanes.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::study) struct AreaRow {
    pub area: Option<usize>,
    pub name: String,
    pub tone: &'static str,
    pub figure: String,
    pub done: bool,
    /// Something of it is beyond need (`Progress::spare`).
    pub over: bool,
    /// passed, planned, open, beyond need.
    pub bar: (f64, f64, f64, f64),
    pub cells: Vec<Cell>,
    pub lanes: Vec<Lane>,
    pub subs: Vec<SubRow>,
}

impl AreaRow {
    /// Whether it unfolds: into its subsections, or into lanes that say more than its bar
    /// (`unfolds`). One that would show a single lane, the bar over it again, does not (owner,
    /// 2026-11-10: „dass ein platzhalter, der nur für ein modul ist kein ausklappbares element
    /// wird. Also zum Beispiel seminar oder Praktikum Oder BA").
    pub(in crate::study) fn unfolds(&self) -> bool {
        !self.subs.is_empty() || unfolds(&self.lanes)
    }
}

/// A subsection of an area: a subgroup with what it holds against what it asks, as the
/// Wahlpflicht shows a choice („16 / 10–24"), its bar over the semesters and its lanes.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::study) struct SubRow {
    /// What keeps it unfolded: its area's name and its own (`sub_key`).
    pub key: String,
    /// Its subgroup (`Bench::groups`) and what it is, where „+" opens the workbench; none for the
    /// area's „Weitere".
    pub group: Option<usize>,
    pub kind: Option<GroupKind>,
    pub name: String,
    /// The name in full, and what it asks, for its title.
    pub title: String,
    /// „16/10–24"; for „Weitere" its credits.
    pub figure: String,
    pub done: bool,
    /// Its stand and a group member's room above its most (`Room`, the room): the Wahlpflicht's
    /// bar. None for „Weitere".
    pub stand: Option<(Stand, Option<(Room, f64)>)>,
    pub cells: Vec<Cell>,
    pub lanes: Vec<Lane>,
}

impl SubRow {
    /// Whether it unfolds (`AreaRow::unfolds`): „Seminar oder Praktikum", a place for one module,
    /// does not.
    pub(in crate::study) fn unfolds(&self) -> bool {
        unfolds(&self.lanes)
    }
}

/// Whether `lanes` say more than the bar of their row: two lanes or more, or one with two pieces
/// in a semester (two rows of its name there, which the bar shows as one piece, „+12").
fn unfolds(lanes: &[Lane]) -> bool {
    lanes.len() > 1 || lanes.iter().any(|lane| lane.bars.iter().any(|bar| bar.len() > 1))
}

/// What keeps a subsection unfolded (`super::OPEN_KEY`): its area's name and its own, a tab
/// between.
pub(in crate::study) fn sub_key(area: &str, sub: &str) -> String {
    format!("{area}\t{sub}")
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::study) struct Layout {
    pub columns: Vec<Column>,
    pub bands: Vec<Band>,
    pub rows: Vec<AreaRow>,
    /// The column of the current semester and how much of it is gone by (0–1).
    pub today: Option<(usize, f64)>,
    /// The flag of the line of today: „heute, 03.11.".
    pub today_flag: String,
}

/// How much of semester `s` is gone by on `today`: 0 at its first day, 1 after its last.
pub(in crate::study) fn gone_by(s: SemesterKey, today: Day) -> f64 {
    let (from, to) = s.bounds();
    let days = f64::from(to.0 - from.0 + 1);
    (f64::from(today.0 - from.0) + 0.5).clamp(0.0, days) / days
}

/// „1. Studienjahr · 2025/26" of semester `s` of a study begun in `start`: the years where the
/// study year begins in a winter, as the university's academic year does.
pub(in crate::study) fn study_year(s: SemesterKey, start: SemesterKey, t: &Texts) -> String {
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
pub(in crate::study) fn row_name(name: &str) -> &str {
    ["Modul aus dem Bereich ", "Module aus dem Bereich ", "Modul aus ", "Module aus "].iter().find_map(|lead| name.strip_prefix(lead)).unwrap_or(name)
}

/// What a piece says of a module without an abbreviation, or of a row of the plan: its name up to
/// the first colon or bracket where that is short, else its first word, cut („Proseminar…"); a
/// row's „Modul aus dem Bereich" goes first („Praktische…").
pub(in crate::study) fn short_label(name: &str) -> String {
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

/// What a module's piece says of itself in its lane („bestanden", „Wiederholung", „noch offen").
pub(in crate::study) fn word_of(piece: &Piece, t: &Texts) -> &'static str {
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

/// The rows of the plan that modules fill in a semester (`Item::fills`): a module's id and the
/// name of the row it fills.
fn filled_rows(items: &[Item]) -> BTreeMap<&str, &str> {
    items.iter().filter_map(|item| Some((item.module_id()?, item.fills.as_deref()?))).collect()
}

/// The lane of an item: a module's by its id, a row of the plan's by its name (`RowOf`).
fn lane_key(item: &Item) -> String {
    match &item.subject {
        Subject::Module { id } => format!("m:{id}"),
        Subject::Row { .. } => format!("row:{}", item.name),
    }
}

/// What a lane of rows of the plan says of its credits: theirs added up, each row once („12",
/// „16–30"); nothing for a module's lane.
fn rows_credits(lane: &Lane, t: &Texts) -> Option<String> {
    let rows: BTreeMap<&str, Option<(f64, f64)>> = lane.bars.iter().flatten().filter_map(|(piece, _)| piece.row.as_ref()).map(|row| (row.id.as_str(), row.credits)).collect();
    if rows.is_empty() || rows.values().any(Option::is_none) {
        return None;
    }
    let (low, high) = rows.values().flatten().fold((0.0, 0.0), |(low, high), (l, h)| (low + l, high + h));
    Some(if high - low < 0.01 { n(low, t) } else { format!("{}–{}", n(low, t), n(high, t)) })
}

/// What a module's lane says of its credits: the plan's text for a row („10–24"), else its credits.
fn piece_credits(item: &Item, t: &Texts) -> Option<String> {
    match &item.subject {
        Subject::Row { credits: Some(text), .. } => Some(folia_plans::plan::credits_in(text, t.locale)),
        _ => item.credits.map(|credits| n(credits, t)),
    }
}

/// What a stand asks: „6", „10–24".
pub(in crate::study) fn range_of(stand: &Stand, t: &Texts) -> String {
    if stand.most > stand.least + EPSILON {
        format!("{}–{}", n(stand.least, t), n(stand.most, t))
    } else {
        n(stand.least, t)
    }
}

/// Where the study's items and the plan's open rows fall among an area's subsections: the
/// subgroups of the workbench (`folia_plans::bench`), from the plan's view of the study.
pub(in crate::study) struct Subs {
    view: PlanView,
    bench: Bench,
    /// The subgroups shown of each area of the progress: two or more, else none.
    shown: Vec<Vec<usize>>,
}

impl Subs {
    pub(in crate::study) fn of(input: &Input, ready: &Ready) -> Self {
        let view = choices::plan_view(input, &ready.study);
        let bench = Bench::of(&ready.study, &view);
        let shown = (0..ready.study.progress.len())
            .map(|area| {
                let groups = bench.shown(&view, area);
                if groups.len() > 1 { groups } else { Vec::new() }
            })
            .collect();
        Subs { view, bench, shown }
    }

    /// The subgroups shown of `area`.
    fn shown(&self, area: Option<usize>) -> &[usize] {
        area.and_then(|area| self.shown.get(area)).map_or(&[], Vec::as_slice)
    }

    /// The subgroup of row `at` of the plan's view: its choice's, else its area's Pflicht.
    fn group_of_line(&self, at: usize) -> Option<usize> {
        let line = self.view.lines.get(at)?;
        match line.choice {
            Some(choice) => self.bench.of_choice(choice),
            None => self.bench.groups.iter().position(|group| group.kind == GroupKind::Pflicht && group.lines.contains(&at)),
        }
    }

    /// The subgroup shown of `area` that module `id` of semester `s` falls in: the one it counts
    /// for (`bench::counted_in`), for a group's extra the member whose list has it (as the
    /// Wahlpflicht counts it, `choices::Group::members`), else the row of the plan that names it,
    /// else the first whose list has it; none for the area's „Weitere".
    fn of_module(&self, area: Option<usize>, id: &str, s: SemesterKey) -> Option<usize> {
        let shown = self.shown(area);
        if shown.is_empty() {
            return None;
        }
        if let Some(found) = bench::counted_in(&self.bench, &self.view, id, s) {
            if shown.contains(&found) {
                return Some(found);
            }
            let member = self.bench.groups.get(found).and_then(|group| group.choice()).and_then(|choice| self.member_with(choice, id));
            if let Some(member) = member.filter(|member| shown.contains(member)) {
                return Some(member);
            }
        }
        if let Some(found) = self.view.lines.iter().position(|line| line.module_id.as_deref() == Some(id)).and_then(|at| self.group_of_line(at)) {
            if shown.contains(&found) {
                return Some(found);
            }
        }
        shown.iter().copied().find(|at| self.bench.groups.get(*at).and_then(|group| group.choice()).and_then(|choice| self.view.choices.get(choice)).is_some_and(|choice| choice.list_of(id).is_some()))
    }

    /// Of the group choice `choice` is part of, the member whose list has `id`.
    fn member_with(&self, choice: usize, id: &str) -> Option<usize> {
        let group = self.view.group_of(choice)?;
        let member = group.members.iter().copied().find(|member| self.view.choices.get(*member).is_some_and(|choice| choice.list_of(id).is_some()))?;
        self.bench.of_choice(member)
    }

    /// The subgroup shown of `area` a row of the plan falls in.
    fn of_row(&self, area: Option<usize>, caption: &str, ord: i64) -> Option<usize> {
        let at = self.view.lines.iter().position(|line| line.caption == caption && line.ord == ord)?;
        self.group_of_line(at).filter(|group| self.shown(area).contains(group))
    }

    /// The subgroup shown of its area an item of semester `s` falls in.
    pub(in crate::study) fn of_item(&self, item: &Item, s: SemesterKey) -> Option<usize> {
        match &item.subject {
            Subject::Module { id } => self.of_module(item.area, id, s),
            Subject::Row { caption, ord, .. } => self.of_row(item.area, caption, *ord),
        }
    }

    /// The subgroup shown of its area an open row of the plan falls in.
    fn of_line(&self, line: &Line) -> Option<usize> {
        match &line.suggestion.pick {
            Pick::Module { id, .. } => self.of_module(line.suggestion.area, id, line.semester),
            Pick::Row { caption, ord } => self.of_row(line.suggestion.area, caption, *ord),
        }
    }

    /// What a subgroup holds against what it asks, as the Wahlpflicht shows it: a group's member
    /// with its room above its most (and that room), a choice by itself, an area's Pflicht by its
    /// rows and what the student let count for it.
    fn stand_of(&self, group: usize) -> (Stand, Option<(Room, f64)>) {
        let Some(found) = self.bench.groups.get(group) else { return (Stand::default(), None) };
        match found.kind {
            GroupKind::Choice(choice) => {
                if let Some(of) = self.view.group_of(choice).filter(|of| of.members.contains(&choice)) {
                    let room = of.extra(&self.view.choices).1;
                    if let Some(member) = of.members(&self.view).into_iter().find(|member| member.choice == choice) {
                        return (member.stand, (member.room != Room::None).then_some((member.room, room)));
                    }
                }
                (self.view.stand_of(choice), None)
            }
            GroupKind::Pflicht => {
                let mut stand = Stand::default();
                for line in found.lines.iter().filter_map(|at| self.view.lines.get(*at)) {
                    let credits = line.credits.unwrap_or(0.0);
                    stand.least += credits;
                    stand.most += credits;
                    match line.state {
                        RowState::Passed(_) => stand.passed += credits,
                        RowState::Planned(_) | RowState::Again { .. } => stand.planned += credits,
                        RowState::Failed(_) | RowState::Open => {}
                    }
                }
                for (taken, _) in self.view.pflicht.iter().filter(|(_, at)| found.lines.contains(at)) {
                    match taken.passed {
                        true => stand.passed += taken.credits.unwrap_or(0.0),
                        false => stand.planned += taken.credits.unwrap_or(0.0),
                    }
                }
                (stand, None)
            }
        }
    }

    /// The name of a subgroup („Praktische Informatik", „Pflichtmodule") and its name in full.
    fn names_of(&self, group: usize, t: &Texts) -> (String, String) {
        match self.bench.groups.get(group).map(|group| group.kind) {
            Some(GroupKind::Choice(choice)) => self.view.choices.get(choice).map_or_else(Default::default, |choice| {
                let short = if choice.fues { t.study.fues.to_string() } else { bench::short_name(&choice.name) };
                (short, choice.name.clone())
            }),
            _ => (t.study_bench.pflicht.to_string(), t.study_bench.pflicht.to_string()),
        }
    }

    /// What Betula says of an item beyond need: none of its credits count in its area
    /// (`Item::spare`), or, a module of a choice's list, it counts for no choice
    /// (`PlanView::spare`) — „über Bedarf · Fachstudium hat schon 54 LP".
    fn over_text(&self, ready: &Ready, item: &Item, s: SemesterKey, t: &Texts) -> Option<String> {
        let st = &t.study;
        if item.spare {
            let required = item.area.and_then(|area| ready.study.progress.get(area)).map_or(0.0, |progress| progress.area.required);
            return Some((st.over_full)(&ready.area_name(item.area, t), &n(required, t)));
        }
        let id = item.module_id()?;
        if !self.view.spare.iter().any(|taken| taken.id == id && taken.semester == s) {
            return None;
        }
        let at = self.view.choices.iter().position(|choice| choice.list_of(id).is_some())?;
        match self.view.group_of(at) {
            Some(group) => Some((st.over_full)(&group.label, &n(group.target(&self.view.choices), t))),
            None => self.view.choices.get(at).map(|choice| (st.over_full)(&bench::short_name(&choice.name), &n(choice.most.max(choice.least), t))),
        }
    }
}

/// What a row of the grid is worked out from: the study, the subsections, the abbreviations, the
/// semesters it shows.
struct Frame<'a> {
    ready: &'a Ready,
    subs: &'a Subs,
    abbrevs: &'a BTreeMap<String, String>,
    t: &'a Texts,
    /// The semesters before the current one, folded into „Bisher"; none where they are not.
    past: Vec<&'a Semester>,
    shown: Vec<&'a Semester>,
    columns: usize,
}

impl Frame<'_> {
    /// What an item of semester `s` says of itself: its name, credits and state, and what Betula
    /// says beside it, for its title and its accessible name.
    fn title_of(&self, item: &Item, s: SemesterKey, when: When, fills: Option<&str>) -> String {
        let (ready, t) = (self.ready, self.t);
        let st = &t.study;
        let mut parts = vec![item.name.clone()];
        let credits = match &item.subject {
            Subject::Row { size: Some(size), .. } => Some(folia_plans::plan::credits_in(size, t.locale)),
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
            match item.credits.filter(|_| !item.fillers.is_empty()) {
                // Modules fill it in part: how far they are.
                Some(asks) => format!("{} · {}", st.placeholder, (st.filled_of)(&n(item.filled, t), &n(asks, t), item.range)),
                None => st.placeholder.to_string(),
            }
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
        if let Some(over) = self.subs.over_text(ready, item, s, t) {
            parts.push(over);
        }
        parts.join(" · ")
    }

    /// An item as a piece. A row its modules fill is none (`study::Item::fills`: its modules carry
    /// it); one they fill in part is a piece of what it still keeps.
    fn piece_of(&self, item: &Item, s: SemesterKey, when: When, fills: Option<&str>) -> Option<Piece> {
        let row = matches!(item.subject, Subject::Row { .. });
        let label = match item.module_id() {
            Some(id) => self.abbrevs.get(id).cloned().unwrap_or_else(|| short_label(short_title(&item.name))),
            None if item.kind == Some(ModuleKind::Fues) => self.t.study.fues.to_string(),
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
            title: self.title_of(item, s, when, fills),
            grow: item.counted().or(item.credits).unwrap_or(NO_CREDITS).max(1.0),
            fill,
            now: when == When::Now,
            retake: item.retake.is_some(),
            unoffered: item.unoffered && !item.passed,
            over: !item.failed && self.subs.over_text(self.ready, item, s, self.t).is_some(),
            row: match &item.subject {
                Subject::Row { caption, ord, credits, .. } => Some(RowOf { id: format!("{caption}:{ord}"), credits: credits.as_deref().and_then(credits_range).or(item.credits.map(|credits| (credits, credits))) }),
                Subject::Module { .. } => None,
            },
            picks: Vec::new(),
        })
    }

    /// The cells and the lanes of a row: the items `holds` of each semester, the open rows of the
    /// plan `holds_line`, the open rows of a semester together as what the row still asks for
    /// („+10", at most `budget` in all where it says; none of them where it asks for nothing more).
    fn bars(&self, holds: impl Fn(&Item, SemesterKey) -> bool, holds_line: impl Fn(&Line) -> bool, mut budget: Option<f64>) -> (Vec<Cell>, Vec<Lane>) {
        let (t, s) = (self.t, &self.t.study);
        let mut cells = Vec::new();
        // The lanes by `lane_key`, in the order they first come.
        let mut lanes: Vec<(String, Lane)> = Vec::new();
        let columns = self.columns;
        let lane_of = |lanes: &mut Vec<(String, Lane)>, key: String, name: &str, row: bool, credits: Option<String>| -> usize {
            if let Some(at) = lanes.iter().position(|(known, _)| *known == key) {
                return at;
            }
            let short = short_title(if row { row_name(name) } else { name });
            lanes.push((key, Lane { name: short.to_string(), title: name.to_string(), credits, bars: vec![Vec::new(); columns], keys: BTreeSet::new(), through: Vec::new(), into: None }));
            lanes.len() - 1
        };
        let fold = !self.past.is_empty();
        if fold {
            let items: Vec<&Item> = self.past.iter().flat_map(|semester| semester.items.iter().filter(|item| holds(item, semester.key))).filter(|item| item.fillers.is_empty()).collect();
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
            for semester in &self.past {
                let fills = filled_rows(&semester.items);
                for item in semester.items.iter().filter(|item| holds(item, semester.key)) {
                    let Some(piece) = self.piece_of(item, semester.key, When::Past, item.module_id().and_then(|id| fills.get(id).copied())) else { continue };
                    let at = lane_of(&mut lanes, lane_key(item), &item.name, piece.row.is_some(), piece_credits(item, t));
                    let word = word_of(&piece, t);
                    if let Some(slot) = lanes.get_mut(at).and_then(|(_, lane)| lane.bars.first_mut()) {
                        match slot.iter_mut().find(|(before, _)| before.key == piece.key) {
                            Some(before) if before.0.fill != Fill::Passed => *before = (piece, word),
                            Some(_) => {}
                            None => slot.push((piece, word)),
                        }
                    }
                }
            }
        }
        let first = usize::from(fold);
        for (offset, semester) in self.shown.iter().enumerate() {
            let column = first + offset;
            let fills = filled_rows(&semester.items);
            let mut pieces = Vec::new();
            for item in semester.items.iter().filter(|item| holds(item, semester.key)) {
                let Some(piece) = self.piece_of(item, semester.key, semester.when, item.module_id().and_then(|id| fills.get(id).copied())) else { continue };
                let at = lane_of(&mut lanes, lane_key(item), &item.name, piece.row.is_some(), piece_credits(item, t));
                set_bar(&mut lanes, at, column, (piece.clone(), word_of(&piece, t)));
                pieces.push(piece);
            }
            // What the plan has open here: a module by its abbreviation, the rows together by what
            // the row still asks for („+10"), none of them where it asks for nothing more.
            let lines: Vec<&Line> = self.ready.lines.iter().filter(|line| line.semester == semester.key && holds_line(line)).collect();
            let mut rows_open: Vec<&Line> = Vec::new();
            // The lanes of the open rows, and which row each is.
            let mut row_lanes: Vec<(String, String)> = Vec::new();
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
                    row: match &line.suggestion.pick {
                        Pick::Row { caption, ord } => Some(RowOf {
                            id: format!("{caption}:{ord}"),
                            credits: line.suggestion.credits_text.as_deref().and_then(credits_range).or(line.suggestion.credits.map(|credits| (credits, credits))),
                        }),
                        Pick::Module { .. } => None,
                    },
                    picks: vec![line.suggestion.pick.clone()],
                };
                match &line.suggestion.pick {
                    Pick::Module { id, .. } => {
                        let open = piece(self.abbrevs.get(id).cloned().unwrap_or_else(|| short_label(short_title(&line.suggestion.name))));
                        let at = lane_of(&mut lanes, format!("m:{id}"), &line.suggestion.name, false, credits);
                        set_bar(&mut lanes, at, column, (open.clone(), s.bar_open));
                        pieces.push(open);
                    }
                    Pick::Row { caption, ord } => {
                        let open = piece(short_label(&line.suggestion.name));
                        let lane = format!("row:{}", line.suggestion.name);
                        let at = lane_of(&mut lanes, lane.clone(), &line.suggestion.name, true, credits);
                        set_bar(&mut lanes, at, column, (open, s.bar_open));
                        rows_open.push(line);
                        row_lanes.push((lane, format!("{caption}:{ord}")));
                    }
                }
            }
            if !rows_open.is_empty() {
                let wanted: f64 = rows_open.iter().filter_map(|line| line.suggestion.credits).sum();
                let credits = match budget.as_mut() {
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
                        row: None,
                        picks: match rows_open.as_slice() {
                            [one] => vec![one.suggestion.pick.clone()],
                            _ => Vec::new(),
                        },
                    });
                } else {
                    // Covered already: they leave their lanes here too.
                    for (key, row) in &row_lanes {
                        if let Some(slot) = lanes.iter_mut().find(|(known, _)| known == key).and_then(|(_, lane)| lane.bars.get_mut(column)) {
                            slot.retain(|(piece, _)| !(piece.fill == Fill::Open && piece.row.as_ref().is_some_and(|of| of.id == *row)));
                        }
                    }
                }
            }
            cells.push(Cell::Semester(SemesterCell { key: semester.key, when: semester.when, leave: semester.leave, pieces }));
        }

        // A lane of rows says their credits added up; a retake's arrow runs from the column it
        // was not passed in to the one it is taken again in.
        let mut lanes: Vec<Lane> = lanes.into_iter().map(|(_, lane)| lane).filter(|lane| lane.bars.iter().any(|bar| !bar.is_empty())).collect();
        for lane in &mut lanes {
            if let Some(credits) = rows_credits(lane, t) {
                lane.credits = Some(credits);
            }
            lane.keys = lane.bars.iter().flatten().map(|(piece, _)| piece.key.clone()).filter(|key| !key.is_empty()).collect();
            let failed = lane.bars.iter().position(|bar| bar.iter().any(|(piece, _)| piece.fill == Fill::Failed));
            let again = lane.bars.iter().enumerate().skip(failed.map_or(usize::MAX, |at| at + 1)).find(|(_, bar)| bar.iter().any(|(piece, _)| piece.retake)).map(|(at, _)| at);
            if let (Some(from), Some(to)) = (failed, again) {
                lane.through = (from + 1..to).collect();
                lane.into = Some(to);
            }
        }
        (cells, lanes)
    }

    /// The subsections of `area`, of the name `area_name`: each subgroup shown with its bar and its
    /// lanes, and „Weitere" for what of it falls in none of them, where something does.
    fn subs_of(&self, area: usize, area_name: &str) -> Vec<SubRow> {
        let (subs, t) = (self.subs, self.t);
        let shown = subs.shown(Some(area));
        if shown.is_empty() {
            return Vec::new();
        }
        let mut rows: Vec<SubRow> = shown
            .iter()
            .map(|group| {
                let group = *group;
                let (stand, room) = subs.stand_of(group);
                // What it still asks for: of a choice its least, else nothing to cap by.
                let budget = matches!(subs.bench.groups.get(group).map(|found| found.kind), Some(GroupKind::Choice(_))).then(|| (stand.least - stand.counted()).max(0.0));
                let (cells, lanes) = self.bars(|item, s| item.area == Some(area) && subs.of_item(item, s) == Some(group), |line| line.suggestion.area == Some(area) && subs.of_line(line) == Some(group), budget);
                let (name, full) = subs.names_of(group, t);
                let range = range_of(&stand, t);
                SubRow {
                    key: sub_key(area_name, &name),
                    group: Some(group),
                    kind: subs.bench.groups.get(group).map(|found| found.kind),
                    title: format!("{full} · {}", (t.study.credits)(&range)),
                    figure: format!("{}/{range}", n(stand.taken(), t)),
                    done: stand.least > 0.0 && stand.counted() + EPSILON >= stand.least,
                    stand: Some((stand, room)),
                    name,
                    cells,
                    lanes,
                }
            })
            .collect();
        // What falls in none of them: modules counted for the area by its tree alone, a group's
        // extra's placeholder.
        let others = |item: &Item, s: SemesterKey| item.area == Some(area) && subs.of_item(item, s).is_none();
        let any = self.past.iter().chain(self.shown.iter()).any(|semester| semester.items.iter().any(|item| others(item, semester.key)));
        if any {
            let (cells, lanes) = self.bars(others, |_| false, None);
            let credits: f64 = self.past.iter().chain(self.shown.iter()).flat_map(|semester| semester.items.iter().filter(|item| others(item, semester.key))).filter(|item| !item.failed).filter_map(Item::counted).sum();
            let name = t.study.other.to_string();
            rows.push(SubRow { key: sub_key(area_name, &name), group: None, kind: None, title: name.clone(), figure: n(credits, t), done: false, stand: None, name, cells, lanes });
        }
        rows
    }
}

/// Puts `bar` into column `column` of lane `at`: a piece of the lane there and its word.
fn set_bar(lanes: &mut [(String, Lane)], at: usize, column: usize, bar: (Piece, &'static str)) {
    if let Some(slot) = lanes.get_mut(at).and_then(|(_, lane)| lane.bars.get_mut(column)) {
        slot.push(bar);
    }
}

impl Layout {
    /// The style of the grid of the heads and of the areas' alike: the same columns, as wide in
    /// both (at least the sum of what each column takes, never what their content asks), and where
    /// today is in its semester.
    pub(in crate::study) fn style(&self) -> String {
        let past_column = self.columns.iter().any(|column| matches!(column, Column::Past { .. }));
        let semesters = self.columns.len() - usize::from(past_column);
        let least = NAMES + if past_column { 96 } else { 0 } + 140 * semesters;
        format!(
            "grid-template-columns: {NAMES}px {}repeat({semesters}, minmax(140px, 1fr)); min-width: {least}px; --today: {}",
            if past_column { "96px " } else { "" },
            self.today.map_or(0.0, |(_, gone)| gone),
        )
    }

    /// Every row that unfolds (`AreaRow::unfolds`): each such area's name, each such subsection's
    /// key (`sub_key`).
    pub(in crate::study) fn keys(&self) -> BTreeSet<String> {
        self.rows
            .iter()
            .filter(|row| row.unfolds())
            .flat_map(|row| std::iter::once(row.name.clone()).chain(row.subs.iter().filter(|sub| sub.unfolds()).map(|sub| sub.key.clone())))
            .collect()
    }

    pub(in crate::study) fn of(ready: &Ready, subs: &Subs, fold: bool, abbrevs: &BTreeMap<String, String>, today: Option<Day>, t: &Texts) -> Self {
        let s = &t.study;
        let study = &ready.study;
        let past: Vec<&Semester> = study.semesters.iter().filter(|semester| semester.when == When::Past).collect();
        let fold = fold && !past.is_empty();
        let shown: Vec<&Semester> = study.semesters.iter().filter(|semester| !(fold && semester.when == When::Past)).collect();

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

        let frame = Frame { ready, subs, abbrevs, t, past: if fold { past } else { Vec::new() }, shown, columns: columns.len() };
        let mut areas: Vec<Option<usize>> = (0..study.progress.len()).map(Some).collect();
        let outside = study.semesters.iter().any(|semester| semester.items.iter().any(|item| item.area.is_none()));
        if outside || study.progress.is_empty() {
            areas.push(None);
        }
        let rows = areas
            .into_iter()
            .map(|area| {
                let name = ready.area_name(area, t);
                // What the area still asks for that is not planned: the open rows of the plan say
                // no more.
                let budget = area.and_then(|area| study.progress.get(area)).map(folia_plans::study::Progress::open);
                let subs = area.map(|area| frame.subs_of(area, &name)).unwrap_or_default();
                let (cells, lanes) = frame.bars(|item, _| item.area == area, |line| line.suggestion.area == area, budget);
                // An area with subsections unfolds into them; its lanes are theirs.
                let lanes = if subs.is_empty() { lanes } else { Vec::new() };
                match area.and_then(|i| study.progress.get(i)) {
                    Some(progress) => {
                        let counted = progress.passed + progress.planned + progress.over;
                        AreaRow {
                            area,
                            tone: ready.tone(area),
                            figure: format!("{}/{}", n(counted, t), n(progress.area.required, t)),
                            done: progress.passed + 0.01 >= progress.area.required && progress.area.required > 0.0,
                            over: progress.spare > 0.0,
                            bar: (progress.passed, progress.planned, progress.open(), progress.spare),
                            name,
                            cells,
                            lanes,
                            subs,
                        }
                    }
                    None => {
                        let (passed, planned) = study.outside;
                        AreaRow { area: None, name: s.outside.to_string(), tone: "var(--text-3)", figure: n(passed + planned, t), done: false, over: false, bar: (passed, planned, 0.0, 0.0), cells, lanes, subs }
                    }
                }
            })
            .collect();
        Layout { columns, bands, rows, today, today_flag }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n;

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

    #[test]
    fn what_a_subsection_asks() {
        let t = i18n::texts(folia_locale::Locale::De);
        assert_eq!(range_of(&Stand { least: 10.0, most: 24.0, ..Stand::default() }, t), "10–24");
        assert_eq!(range_of(&Stand { least: 6.0, most: 6.0, ..Stand::default() }, t), "6");
        assert_eq!(sub_key("Fachstudium", "Praktische Informatik"), "Fachstudium\tPraktische Informatik");
    }
}
