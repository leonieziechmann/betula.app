//! The view „Prüfungen" of one semester of the Studienplan: every exam date of the planned
//! modules by day („Fr 12.03.2027 · 11:00–13:00 · Entwicklung von Softwaresystemen · Audimax 1
//! (Zentralcampus)"), a module's later sittings marked „2. Termin", and a warning directly above
//! the later sitting of each pair that collides. Windows („08.–19.02.2027 · nach Absprache") and
//! deadlines („So 14.02.2027 · bis 24:00") stand at their first day; what QIS lists without a date
//! stands under „Ohne festen Termin"; one line names the planned modules the data has no exam for.
//!
//! Each sitting has an eye: hiding a sitting is how a student takes the other one (a module's
//! sittings are alternatives of each other, `catalog::timetable::exams`), and the warnings follow
//! at once (the timetable is worked out again in Rust, no query, R21). A hidden sitting stays in
//! the list, dimmed, so the eye can bring it back; one the kind „Prüfung" or the Standort hides
//! says so, and its eye shows the kind again or (the Standort decides) does nothing.
//!
//! The view reads the timetable and the address without the module beside the plan, two
//! siblings (R16), and the titles through a memo of the data. What it shows is built as plain
//! values first (`Line`, `State`), which the tests read without rendering. A sitting's line is
//! keyed by what does not change when it is hidden, and reads its state from a memo of its own
//! (R5): the eye that was clicked stays the same element and keeps the focus.

use std::collections::{BTreeMap, BTreeSet};

use catalog::labels::{Campus, Code};
use catalog::pages::StudyplanData;
use catalog::timetable::day::{clock, Day};
use catalog::timetable::exams::{self, Exam, ExamRow, ExamShape, ExamWarning, Termin, TerminAt, WarningKind};
use catalog::timetable::kind::EventKind;
use catalog::timetable::model::Timetable;
use catalog::timetable::rowkey::RowKey;
use catalog::timetable::select::HiddenBy;
use catalog::timetable::semester::SemesterKey;
use catalog::url::StudyplanUrl;
use leptos::prelude::*;

use super::head::weekday_name;
use super::PlanCtx;
use crate::ui::Icon;

/// „Prüfungen": the sittings by date, the warnings above them, and what has no date.
#[component]
pub(super) fn ExamsView(ctx: PlanCtx) -> impl IntoView {
    let base = Memo::new(move |_| ctx.url.with(|url| url.with_open(None, None)));
    let about = Memo::new(move |_| ctx.data.with(|data| data.as_ref().map(About::of).unwrap_or_default()));
    let built = Memo::new(move |_| {
        let base = base.get();
        ctx.table.with(|table| table.as_ref().map(|table| about.with(|about| exam_lines(table, about, &base))))
    });
    let lines = Memo::new(move |_| built.with(|built| built.as_ref().map(|(lines, _)| lines.clone()).unwrap_or_default()));
    let states = Memo::new(move |_| built.with(|built| built.as_ref().map(|(_, states)| states.clone()).unwrap_or_default()));
    // Nothing to list where the data could have had exams: said once. A past semester and one
    // whose dates are not out yet say so in the head already.
    let none = Memo::new(move |_| built.with(|built| built.as_ref().is_some_and(|(lines, _)| lines.is_empty())) && about.with(|about| about.listed));

    view! {
        {move || none.get().then(|| view! { <p class="hint">"Keine Prüfungstermine im Datenstand."</p> })}
        <div class="sp-exams">
            <For each=move || lines.get() key=|line| line.clone() children=move |line: Line| line_view(ctx, states, line)/>
        </div>
    }
}

// ---------- what the view shows ----------

/// What the lines need of the semester's data besides its timetable: the planned modules'
/// titles, and whether a module without any exam is worth a line (the semester's dates are
/// published and not past, when the data could have had one).
#[derive(Clone, Debug, Default, PartialEq)]
struct About {
    titles: BTreeMap<String, String>,
    listed: bool,
}

impl About {
    fn of(data: &StudyplanData) -> Self {
        let past = data.meta.current_semester.as_deref().and_then(SemesterKey::parse).is_some_and(|current| data.key < current);
        About { titles: data.titles(), listed: !data.counts.is_empty() && !past }
    }

    /// A module's title; its number where the catalog has none.
    fn title(&self, module: &str) -> String {
        self.titles.get(module).cloned().unwrap_or_else(|| format!("Modul {module}"))
    }
}

/// One line of the view.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Line {
    /// Two exams that collide, above the later sitting: `warn` when no combination of the two
    /// modules' sittings avoids it, else quiet with what does. `at`: its place among the
    /// timetable's warnings, which keeps two of the same words apart.
    Warning { warn: bool, text: String, at: usize },
    /// „Ohne festen Termin", before what has no date.
    Undated,
    Sitting(Sitting),
    /// The planned modules the data has no exam for, by title.
    Without(Vec<String>),
}

/// A sitting as it stays while it is hidden and shown again: its date, the module and where it is.
/// What changes with that (`State`) is read apart.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Sitting {
    /// `<event>-<row key>`: what its state is found by.
    id: String,
    /// „Fr 12.03.2027", „08.–19.02.2027", „Termin offen".
    when: String,
    /// „11:00–13:00", „bis 24:00", „nach Absprache", „Zeit offen"; empty for an open one.
    time: String,
    title: String,
    /// „Audimax 1 (Zentralcampus)", „Ort offen"; empty for an open one without a room.
    place: String,
    /// The module beside the plan, at this sitting.
    href: String,
    /// The Termin the eye hides; `None` where there is no key to store.
    key: Option<RowKey>,
    /// The exam event, when it is a number the store can keep.
    event: Option<u32>,
}

/// What a sitting's line changes when something is hidden: whether it is and why, and whether it
/// is a module's later sitting (ranks count the sittings shown).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct State {
    hidden: Option<HiddenBy>,
    later: bool,
}

impl State {
    /// Why the sitting is not shown, where its own eye did not hide it.
    fn reason(&self) -> Option<String> {
        match self.hidden {
            Some(HiddenBy::Kinds) => Some("Prüfungen ausgeblendet".to_string()),
            Some(HiddenBy::Town(town)) => Some(format!("Standort {}", town.label())),
            _ => None,
        }
    }

    /// What the sitting's eye does now; `keyed`: the sitting has a key the store can keep.
    fn eye(&self, keyed: bool) -> Eye {
        match self.hidden {
            None if keyed => Eye::Hide,
            Some(HiddenBy::Row | HiddenBy::Event) => Eye::Show,
            Some(HiddenBy::Kinds) => Eye::ShowKind,
            _ => Eye::Fixed,
        }
    }
}

/// What the eye of a sitting does. Every sitting has one, so the line keeps its shape (and on a
/// phone its column) whatever hides it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Eye {
    /// Hides the sitting: the student takes another one.
    Hide,
    /// Brings back what the eye of the sitting or of its exam hid.
    Show,
    /// Brings the kind „Prüfung" back, whose chip hides every exam.
    ShowKind,
    /// Nothing to do here: the Standort decides, or the sitting has no key to keep.
    Fixed,
}

/// A sitting of the list, with what places it: when it is, and the Termin a warning names (the
/// planned modules of its exam, its day and start, while it is shown).
struct Placed {
    order: (Day, u16, usize, String),
    sitting: Sitting,
    state: State,
    termin: Option<(Vec<String>, Day, u16)>,
}

/// The lines of the view and the state of each sitting, by its id. The sittings of the planned
/// modules in date order, each warning above the later sitting of its pair, then „Ohne festen
/// Termin": the exams without a date and the modules without an exam (only where `about.listed`).
fn exam_lines(table: &Timetable, about: &About, base: &StudyplanUrl) -> (Vec<Line>, BTreeMap<String, State>) {
    let position = |module: &str| table.modules.iter().position(|planned| planned == module).unwrap_or(usize::MAX);
    let mut dated: Vec<Placed> = Vec::new();
    let mut open: Vec<Placed> = Vec::new();
    for exam in &table.exams {
        let Some(module) = exam.modules.first() else { continue };
        for (key, rows) in sittings_of(exam) {
            let Some(first) = rows.first() else { continue };
            let (when, time, day, from) = when_of(&rows);
            let place = place_of(&rows, !matches!(first.shape, ExamShape::Open));
            let sitting = Sitting {
                id: format!("{}-{key}", exam.event_id),
                when,
                time,
                title: about.title(module),
                place,
                href: base.with_open(Some(module.as_str()), first.key).path(),
                key: first.key,
                event: exam.event_id.parse().ok(),
            };
            let state = State { hidden: first.hidden, later: rows.iter().any(|row| row.rank == 2) };
            let termin = match first.shape {
                ExamShape::Sitting { day, from, .. } if first.hidden.is_none() => Some((exam.modules.clone(), day, from)),
                _ => None,
            };
            let placed = |day: Day, from: u16| Placed { order: (day, from, position(module), sitting.title.clone()), sitting: sitting.clone(), state: state.clone(), termin: termin.clone() };
            match day {
                Some(day) => dated.push(placed(day, from)),
                None => open.push(placed(Day(0), 0)),
            }
        }
    }
    dated.sort_by(|a, b| a.order.cmp(&b.order));
    open.sort_by(|a, b| a.order.cmp(&b.order));

    let termine = exams::termine(&table.exams, &table.modules);
    let mut warnings: Vec<(&ExamWarning, Line)> = table.exam_warnings.iter().enumerate().map(|(at, warning)| (warning, warning_line(at, warning, about, &termine))).collect();
    let mut lines = Vec::new();
    let mut states = BTreeMap::new();
    for placed in dated {
        // A warning names the later Termin by its module, day and start: this sitting, when it is
        // one of that module's exam and shown.
        let names = |warning: &ExamWarning| {
            placed.termin.as_ref().is_some_and(|(modules, day, from)| modules.contains(&warning.b.module_id) && *day == warning.day && *from == warning.b.from)
        };
        let (here, rest): (Vec<_>, Vec<_>) = warnings.into_iter().partition(|(warning, _)| names(warning));
        warnings = rest;
        lines.extend(here.into_iter().map(|(_, line)| line));
        states.insert(placed.sitting.id.clone(), placed.state);
        lines.push(Line::Sitting(placed.sitting));
    }
    // A warning whose sitting the list does not show (it cannot, but a warning is never lost).
    lines.extend(warnings.into_iter().map(|(_, line)| line));

    if !open.is_empty() {
        lines.push(Line::Undated);
    }
    for placed in open {
        states.insert(placed.sitting.id.clone(), placed.state);
        lines.push(Line::Sitting(placed.sitting));
    }
    let exams_of: BTreeSet<&str> = table.exams.iter().flat_map(|exam| exam.modules.iter().map(String::as_str)).collect();
    let without: Vec<String> = table
        .modules
        .iter()
        .filter(|module| about.listed && !exams_of.contains(module.as_str()) && about.titles.contains_key(*module))
        .map(|module| about.title(module))
        .collect();
    if !without.is_empty() {
        lines.push(Line::Without(without));
    }
    (lines, states)
}

/// The rows of an exam by the sitting they are: rows that share a key (the same Termin in two
/// rooms) are one; a row without a key is one of its own. In the exam's order.
fn sittings_of(exam: &Exam) -> Vec<(String, Vec<&ExamRow>)> {
    let mut sittings: Vec<(String, Vec<&ExamRow>)> = Vec::new();
    for row in &exam.rows {
        let id = match (row.key, row.ord) {
            (Some(key), _) => key.text(),
            (None, Some(ord)) => format!("o{ord}"),
            (None, None) => "o".to_string(),
        };
        match sittings.iter_mut().find(|(known, _)| *known == id) {
            Some((_, rows)) => rows.push(row),
            None => sittings.push((id, vec![row])),
        }
    }
    sittings
}

/// When a sitting is: the day („Fr 12.03.2027", a window's „08.–19.02.2027") and the time
/// („11:00–13:00" from the earliest start to the latest end of its rows, „bis 24:00", „nach
/// Absprache", „Zeit offen"); with the day and start it is sorted by. An open one has no day.
fn when_of(rows: &[&ExamRow]) -> (String, String, Option<Day>, u16) {
    let Some(first) = rows.first() else {
        return ("Termin offen".to_string(), String::new(), None, 0);
    };
    match first.shape {
        ExamShape::Sitting { day, .. } => {
            let times = rows.iter().filter_map(|row| match row.shape {
                ExamShape::Sitting { from, to, .. } => Some((from, to)),
                _ => None,
            });
            let (from, to) = times.fold((u16::MAX, 0), |(from, to), (f, t)| (from.min(f), to.max(t)));
            (day_name(day), format!("{}–{}", clock(from), clock(to)), Some(day), from)
        }
        ExamShape::Deadline { day } => {
            let end = first.reading.shown.end_time.clone().filter(|end| !end.trim().is_empty()).unwrap_or_else(|| "24:00".to_string());
            (day_name(day), format!("bis {end}"), Some(day), 0)
        }
        ExamShape::Window { first: from, last } => (span(from, last), "nach Absprache".to_string(), Some(from), 0),
        ExamShape::DayOnly { day } => (day_name(day), "Zeit offen".to_string(), Some(day), 0),
        ExamShape::Open => ("Termin offen".to_string(), String::new(), None, 0),
    }
}

/// Where a sitting is: its rooms as QIS writes them, each once, and its campuses in short where
/// the rooms do not name them already („Audimax 1 (Zentralcampus)", „HG 0.20 / HG 0.19",
/// „Senftenberg"); „Ort offen" for a dated sitting without either.
fn place_of(rows: &[&ExamRow], dated: bool) -> String {
    let mut rooms: Vec<&str> = Vec::new();
    let mut campuses: Vec<String> = Vec::new();
    for row in rows {
        if let Some(room) = row.date.room.as_deref().map(str::trim).filter(|room| !room.is_empty()) {
            if !rooms.contains(&room) {
                rooms.push(room);
            }
        }
        if let Some(campus) = row.date.campus.as_ref().map(campus_name) {
            if !campuses.contains(&campus) {
                campuses.push(campus);
            }
        }
    }
    // QIS often writes the campus into the room („… - Audimax 1 - Zentralcampus"): once is enough.
    let named = rooms.join(" / ");
    campuses.retain(|campus| !named.to_lowercase().contains(&campus.to_lowercase()));
    match (rooms.is_empty(), campuses.is_empty()) {
        (false, false) => format!("{named} ({})", campuses.join(" / ")),
        (false, true) => named,
        (true, false) => campuses.join(" / "),
        (true, true) if dated => "Ort offen".to_string(),
        (true, true) => String::new(),
    }
}

/// A campus as a hop between two exams names it: „Zentralcampus", „Sachsendorf", „Senftenberg".
fn campus_name(campus: &Code<Campus>) -> String {
    match campus.known() {
        Some(Campus::Zentralcampus) => "Zentralcampus".to_string(),
        Some(Campus::Sachsendorf) => "Sachsendorf".to_string(),
        Some(Campus::Senftenberg) => "Senftenberg".to_string(),
        _ => campus.label().to_string(),
    }
}

/// „Fr 12.03.2027".
fn day_name(day: Day) -> String {
    format!("{} {}", weekday_name(day.weekday()), day.german())
}

/// „08.–19.02.2027", „25.02.–05.03.2027", „28.12.2026–08.01.2027".
fn span(first: Day, last: Day) -> String {
    let ((y1, m1, d1), (y2, m2, _)) = (first.ymd(), last.ymd());
    if (y1, m1) == (y2, m2) {
        format!("{d1:02}.–{}", last.german())
    } else if y1 == y2 {
        format!("{}–{}", first.short(), last.german())
    } else {
        format!("{}–{}", first.german(), last.german())
    }
}

// ---------- warnings ----------

/// A warning as it stands above the later sitting of its pair, in the words of the notes without
/// the day the sittings under it show: „Prüfungen gleichzeitig: Mathematik W-1 · ERP - …", „0 min
/// von Zentralcampus nach Senftenberg: Kraftwerkstechnik I bis 10:00 · Gentechnik ab 10:00". A
/// soft one says what avoids it: „… · Mathematik IT-1: Zweittermin 11.03. passt".
fn warning_line(at: usize, warning: &ExamWarning, about: &About, termine: &[(String, Vec<TerminAt>)]) -> Line {
    let (a, b) = (about.title(&warning.a.module_id), about.title(&warning.b.module_id));
    let mut text = match &warning.kind {
        WarningKind::Overlap => format!("Prüfungen gleichzeitig: {a} · {b}"),
        WarningKind::Tight { gap, from, to } => {
            format!("{gap} min von {} nach {}: {a} bis {} · {b} ab {}", campus_name(from), campus_name(to), clock(warning.a.to), clock(warning.b.from))
        }
    };
    if !warning.hard {
        if let Some(avoid) = warning.avoid {
            text.push_str(" · ");
            text.push_str(&avoid_text(warning, avoid, termine, about));
        }
    }
    Line::Warning { warn: warning.hard, text, at }
}

/// What avoids a soft warning: a Termin on the `avoid` day of one of the two modules that is free
/// of the other module's Termin in the warning („Mathematik IT-1: Zweittermin 11.03. passt", „…:
/// Erstermin 25.02. passt" where it is that module's earliest), else „andere Termine passen" (only
/// a change of both avoids it). The notes of the semester say it in the same words.
fn avoid_text(warning: &ExamWarning, avoid: Day, termine: &[(String, Vec<TerminAt>)], about: &About) -> String {
    let list = |module: &str| termine.iter().find(|(id, _)| id == module).map_or(&[][..], |(_, list)| list.as_slice());
    let issue = |termin: &Termin| list(&termin.module_id).iter().find(|at| at.day == warning.day && at.termin == *termin);
    if let (Some(a), Some(b)) = (issue(&warning.a), issue(&warning.b)) {
        for (mine, other) in [(a, b), (b, a)] {
            let module = &mine.termin.module_id;
            if let Some(index) = list(module).iter().position(|at| at.day == avoid && at != mine && exams::collision(at, other).is_none()) {
                let rank = if index == 0 { "Erstermin" } else { "Zweittermin" };
                return format!("{}: {rank} {} passt", about.title(module), avoid.short());
            }
        }
    }
    "andere Termine passen".to_string()
}

// ---------- the lines ----------

fn line_view(ctx: PlanCtx, states: Memo<BTreeMap<String, State>>, line: Line) -> AnyView {
    match line {
        Line::Warning { warn: true, text, .. } => view! { <p class="note"><Icon name="triangle-alert"/><span>{text}</span></p> }.into_any(),
        Line::Warning { warn: false, text, .. } => view! { <p class="note quiet"><span>{text}</span></p> }.into_any(),
        Line::Undated => view! { <h3 class="label">"Ohne festen Termin"</h3> }.into_any(),
        Line::Without(titles) => view! { <p class="hint">{format!("Keine Prüfung im Datenstand: {}", titles.join(", "))}</p> }.into_any(),
        Line::Sitting(sitting) => sitting_view(ctx, states, sitting).into_any(),
    }
}

/// A sitting: when, what and where, and its eye. What hiding changes is read from the sitting's
/// own memo, so the line and its eye stay the same elements.
fn sitting_view(ctx: PlanCtx, states: Memo<BTreeMap<String, State>>, sitting: Sitting) -> impl IntoView {
    let id = sitting.id.clone();
    let state = Memo::new(move |_| states.with(|states| states.get(&id).cloned().unwrap_or_default()));
    let hidden = Memo::new(move |_| state.with(|state| state.hidden.is_some()));
    let eye = Memo::new(move |_| state.with(|state| state.eye(sitting.key.is_some())));
    let small = Memo::new(move |_| {
        let (later, reason) = state.with(|state| (state.later, state.reason()));
        let parts: Vec<String> = later.then(|| "2. Termin".to_string()).into_iter().chain(reason).chain(Some(sitting.place.clone()).filter(|place| !place.is_empty())).collect();
        parts.join(" · ")
    });
    let (key, event) = (sitting.key, sitting.event);
    // What is hidden or shown changes at once: the timetable is worked out again, no query (R21).
    let toggle = move |_| {
        let Some(plan) = ctx.plan else { return };
        let (semester, hidden, eye) = (ctx.key.get_untracked(), state.with_untracked(|state| state.hidden), eye.get_untracked());
        plan.update(|doc| match (eye, hidden, key, event) {
            (Eye::Hide, _, Some(key), _) => doc.set_row(semester, key, true),
            (Eye::Show, Some(HiddenBy::Event), _, Some(event)) => doc.set_event(semester, event, false),
            (Eye::Show, _, Some(key), _) => doc.set_row(semester, key, false),
            (Eye::ShowKind, ..) => doc.set_kind(semester, EventKind::Exam, false),
            _ => {}
        });
    };
    let label = move || match eye.get() {
        Eye::Hide => "Prüfungstermin ausblenden".to_string(),
        Eye::Show => "Prüfungstermin einblenden".to_string(),
        Eye::ShowKind => "Prüfungen einblenden".to_string(),
        Eye::Fixed => state.with(State::reason).unwrap_or_else(|| "Prüfungstermin".to_string()),
    };
    let time = (!sitting.time.is_empty()).then(|| view! { <small>{sitting.time.clone()}</small> });
    view! {
        <div class="sp-exam" data-hidden=move || state.with(|state| state.hidden.is_some()).then_some("")>
            <span class="when">{sitting.when.clone()}{time}</span>
            <span>
                <a href=sitting.href.clone() data-noscroll="">{sitting.title.clone()}</a>
                {move || small.with(|small| (!small.is_empty()).then(|| view! { <small>{small.clone()}</small> }))}
            </span>
            <button
                class="eye"
                type="button"
                aria-pressed=move || if hidden.get() { "true" } else { "false" }
                aria-label=label
                title=label
                disabled=move || eye.get() == Eye::Fixed
                on:click=toggle
            >
                {move || if hidden.get() { view! { <Icon name="eye-off"/> } } else { view! { <Icon name="eye"/> } }}
            </button>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use catalog::rows_detail::{DateRow, EventDate};
    use catalog::timetable::facts::SemesterFacts;
    use catalog::timetable::model::Input;
    use catalog::timetable::select::Selection;

    use super::*;

    fn key() -> SemesterKey {
        SemesterKey::parse("2026W").unwrap()
    }

    /// An exam row of `module`: its event and ord, one day (or a window, or none), times, room
    /// and campus.
    fn exam(module: &str, event: &str, ord: i64, dates: Option<(&str, &str)>, time: (&str, &str), room: Option<&str>, campus: Option<&str>) -> DateRow {
        let text = |value: &str| (!value.is_empty()).then(|| value.to_string());
        DateRow {
            module_id: module.into(),
            ord: Some(ord),
            cancelled_dates: None,
            date: EventDate {
                semester_key: "2026W".into(),
                semester_label: "WiSe 2026/27".into(),
                event_id: event.into(),
                event_number: None,
                event_title: format!("Prüfung {module}"),
                event_type: None,
                group_name: None,
                weekday: None,
                start_time: text(time.0),
                end_time: text(time.1),
                rhythm: None,
                rhythm_raw: None,
                first_date: dates.map(|(first, _)| first.into()),
                last_date: dates.map(|(_, last)| last.into()),
                room: room.map(str::to_string),
                campus: campus.map(Code::parse),
                instructor: None,
                comment: None,
                source_url: None,
            },
        }
    }

    fn table(modules: &[&str], exams: &[DateRow], selection: &Selection) -> Timetable {
        let facts = SemesterFacts::derive(key(), None, &[]);
        let modules: Vec<String> = modules.iter().map(|module| module.to_string()).collect();
        let input = Input { key: key(), semester: None, facts: &facts, modules: &modules, schedule: &[], exams, sws: &[] };
        Timetable::build(&input, selection)
    }

    fn about(listed: bool) -> About {
        let titles = [("1", "Mathematik W-1"), ("2", "ERP - Integrierte betriebliche Systeme"), ("3", "Kraftwerkstechnik I"), ("4", "Gentechnik")];
        About { titles: titles.iter().map(|(id, title)| (id.to_string(), title.to_string())).collect(), listed }
    }

    fn texts(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|line| match line {
                Line::Warning { warn, text, .. } => format!("{} {text}", if *warn { "!" } else { "~" }),
                Line::Undated => "## Ohne festen Termin".to_string(),
                Line::Sitting(s) => format!("{} {} · {} · {}", s.when, s.time, s.title, s.place),
                Line::Without(titles) => format!("ohne: {}", titles.join(", ")),
            })
            .collect()
    }

    #[test]
    fn the_sittings_stand_by_date_and_a_warning_above_the_later_one() {
        let rows = [
            exam("1", "10", 1, Some(("2027-02-08", "2027-02-08")), ("11:00", "13:00"), Some("Audimax 1"), Some("zentralcampus")),
            exam("2", "20", 1, Some(("2027-02-08", "2027-02-08")), ("11:00", "12:30"), None, Some("zentralcampus")),
            exam("3", "30", 1, Some(("2027-02-15", "2027-02-15")), ("08:00", "10:00"), Some("HG 0.20"), Some("zentralcampus")),
            exam("4", "40", 1, Some(("2027-02-15", "2027-02-15")), ("10:00", "11:30"), Some("14C.105 - Campus Senftenberg"), Some("senftenberg")),
            // A window, a deadline, and one without a date.
            exam("3", "31", 1, Some(("2027-03-08", "2027-03-19")), ("", ""), None, None),
            exam("4", "41", 1, Some(("2027-02-14", "2027-02-14")), ("", "24:00"), None, None),
            exam("4", "42", 1, None, ("", ""), None, None),
        ];
        let table = table(&["1", "2", "3", "4"], &rows, &Selection::default());
        let (lines, states) = exam_lines(&table, &about(true), &StudyplanUrl::default());
        assert_eq!(
            texts(&lines),
            vec![
                "Mo 08.02.2027 11:00–13:00 · Mathematik W-1 · Audimax 1 (Zentralcampus)",
                "! Prüfungen gleichzeitig: Mathematik W-1 · ERP - Integrierte betriebliche Systeme",
                "Mo 08.02.2027 11:00–12:30 · ERP - Integrierte betriebliche Systeme · Zentralcampus",
                "So 14.02.2027 bis 24:00 · Gentechnik · Ort offen",
                "Mo 15.02.2027 08:00–10:00 · Kraftwerkstechnik I · HG 0.20 (Zentralcampus)",
                "! 0 min von Zentralcampus nach Senftenberg: Kraftwerkstechnik I bis 10:00 · Gentechnik ab 10:00",
                // The room names its campus already.
                "Mo 15.02.2027 10:00–11:30 · Gentechnik · 14C.105 - Campus Senftenberg",
                "08.–19.03.2027 nach Absprache · Kraftwerkstechnik I · Ort offen",
                "## Ohne festen Termin",
                "Termin offen  · Gentechnik · ",
            ]
        );
        // Every sitting has its state, none of them hidden or later.
        assert_eq!(states.len(), 7);
        assert!(states.values().all(|state| *state == State::default()));
        // Each sitting opens its module beside the plan, at its Termin.
        let Some(Line::Sitting(first)) = lines.first() else { panic!("a sitting first") };
        assert!(first.href.starts_with("/studyplan?open=1&row=10-"), "{}", first.href);
    }

    #[test]
    fn a_hidden_sitting_stays_dimmed_and_its_twin_is_the_later_one_no_more() {
        let rows = [
            exam("1", "10", 1, Some(("2027-02-08", "2027-02-08")), ("11:00", "13:00"), Some("Audimax 1"), None),
            exam("1", "10", 2, Some(("2027-03-10", "2027-03-10")), ("11:00", "13:00"), Some("Audimax 1"), None),
            exam("2", "20", 1, Some(("2027-02-08", "2027-02-08")), ("12:00", "13:00"), None, None),
            exam("2", "20", 2, Some(("2027-03-11", "2027-03-11")), ("12:00", "13:00"), None, None),
        ];
        let all = table(&["1", "2"], &rows, &Selection::default());
        let (lines, states) = exam_lines(&all, &about(true), &StudyplanUrl::default());
        // Both modules sit twice: the clash of the first days is avoidable, so it is quiet.
        let quiet: Vec<&String> = lines.iter().filter_map(|line| match line {
            Line::Warning { warn: false, text, .. } => Some(text),
            _ => None,
        }).collect();
        assert_eq!(quiet, vec!["Prüfungen gleichzeitig: Mathematik W-1 · ERP - Integrierte betriebliche Systeme · Mathematik W-1: Zweittermin 10.03. passt"]);
        let later: Vec<bool> = lines.iter().filter_map(|line| match line {
            Line::Sitting(s) => states.get(&s.id).map(|state| state.later),
            _ => None,
        }).collect();
        assert_eq!(later, vec![false, false, true, true]);

        // The student takes Mathematik's second sitting: the first is hidden, stays in the list,
        // and the second is its first sitting now; the warning is gone.
        let Some(Line::Sitting(first)) = lines.first() else { panic!("a sitting first") };
        let selection = Selection { hidden_rows: first.key.into_iter().collect(), ..Default::default() };
        let hidden = table(&["1", "2"], &rows, &selection);
        let (lines, states) = exam_lines(&hidden, &about(true), &StudyplanUrl::default());
        assert!(!lines.iter().any(|line| matches!(line, Line::Warning { .. })));
        assert_eq!(states.get(&first.id).and_then(|state| state.hidden), Some(HiddenBy::Row));
        assert_eq!(states.get(&first.id).map(|state| state.eye(true)), Some(Eye::Show));
        let second = lines.iter().filter_map(|line| match line {
            Line::Sitting(s) if s.title == "Mathematik W-1" && s.id != first.id => states.get(&s.id),
            _ => None,
        });
        assert_eq!(second.map(|state| state.later).collect::<Vec<_>>(), vec![false]);
    }

    #[test]
    fn the_eye_brings_back_what_hid_the_sitting_where_it_can() {
        let state = |hidden| State { hidden, later: false };
        assert_eq!((state(None).eye(true), state(None).eye(false)), (Eye::Hide, Eye::Fixed));
        assert_eq!((state(Some(HiddenBy::Row)).eye(true), state(Some(HiddenBy::Event)).eye(true)), (Eye::Show, Eye::Show));
        // The kind's chip hides every exam; the eye shows them again, and the line says why.
        let kinds = state(Some(HiddenBy::Kinds));
        assert_eq!((kinds.eye(true), kinds.reason().as_deref()), (Eye::ShowKind, Some("Prüfungen ausgeblendet")));
        // The Standort decides: nothing for the eye to do.
        let town = state(Some(HiddenBy::Town(catalog::timetable::select::Town::Cottbus)));
        assert_eq!((town.eye(true), town.reason().as_deref()), (Eye::Fixed, Some("Standort Cottbus")));
        assert_eq!(state(Some(HiddenBy::Event)).reason(), None);
    }

    #[test]
    fn a_module_without_an_exam_is_named_where_the_data_could_have_one() {
        let rows = [exam("1", "10", 1, Some(("2027-02-08", "2027-02-08")), ("11:00", "13:00"), None, None)];
        let table = table(&["1", "2", "9"], &rows, &Selection::default());
        let (lines, _) = exam_lines(&table, &about(true), &StudyplanUrl::default());
        // „9" is not in the catalog: the legend says so, not this view.
        assert_eq!(
            texts(&lines),
            vec!["Mo 08.02.2027 11:00–13:00 · Mathematik W-1 · Ort offen", "ohne: ERP - Integrierte betriebliche Systeme"]
        );
        // A past semester, or one whose dates are not out yet: no such line.
        let (lines, _) = exam_lines(&table, &about(false), &StudyplanUrl::default());
        assert_eq!(texts(&lines), vec!["Mo 08.02.2027 11:00–13:00 · Mathematik W-1 · Ort offen"]);
    }

    #[test]
    fn a_window_names_its_days() {
        let d = |text: &str| Day::parse(text).unwrap();
        assert_eq!(span(d("2027-02-08"), d("2027-02-19")), "08.–19.02.2027");
        assert_eq!(span(d("2027-02-25"), d("2027-03-05")), "25.02.–05.03.2027");
        assert_eq!(span(d("2026-12-28"), d("2027-01-08")), "28.12.2026–08.01.2027");
    }
}
