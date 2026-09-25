//! The top of one semester of the plan: its head (‹ „WiSe 2026/27" › and the numbers under it),
//! the planned modules as a legend of their tones, the marked modules that could still be planned
//! into it, the notes (what overlaps, what is still to choose, what is missing), and the one muted
//! line that says what the page derived rather than read (R12).
//!
//! Every part reads the memos of `PlanCtx` it needs. Where a part needs two things, it takes them
//! from sibling memos (`key` and `data`, `table` and a memo of `selection`), never a memo together
//! with the memo it is derived from (R16); what depends on the plan's placeholders works the
//! semester out itself (`key_of`), as `wanted` does. Texts are the plan's own terms, numbers first
//! (design A.3, A.5).

use std::collections::{BTreeMap, BTreeSet};

use catalog::filter::{CatalogQuery, FitsFilter, ProgramScope};
use catalog::labels::{Campus, Code, Rhythm, TurnusSeason};
use catalog::pages::{self, BookmarksData, StudyplanData};
use catalog::studyplan::PlanDoc;
use catalog::timetable::day::{clock, Day};
use catalog::timetable::exams::{self, ExamWarning, Termin, TerminAt, WarningKind};
use catalog::timetable::kind::EventKind;
use catalog::timetable::model::{Attendance, Basis, Event, Timetable};
use catalog::timetable::rowkey::RowKey;
use catalog::timetable::select::TownChoice;
use catalog::timetable::semester::{fachsemester, SemesterKey};
use catalog::url::{self, BookmarkSort, CatalogUrl, PlanView, Season, StudyplanUrl};
use leptos::prelude::*;

use super::{key_of, PlanCtx, SheetToggle};
use crate::bookmarks::Bookmarks;
use crate::format;
use crate::myprogram::{MineResolved, MyProgram};
use crate::pending::{Change, Pending};
use crate::ui::Icon;

/// The tones of the plan's modules, in the order of `app.css`'s `t-…` classes: the first planned
/// module of a semester is ice, the ninth ice again (the timetable's `Event::tone`, 1–8).
const HUES: [&str; 8] = ["t-ice", "t-sun", "t-violet", "t-teal", "t-green", "t-coral", "t-rose", "t-slate"];

/// The class of a module's tone (`Event::tone`, 1–8); a tone out of range is the first.
pub(super) fn hue(tone: u8) -> &'static str {
    HUES.get(usize::from(tone.saturating_sub(1)) % HUES.len()).copied().unwrap_or("t-ice")
}

/// The tone of the module at `position` in the semester's plan: the tone the timetable gives the
/// module's events.
pub(super) fn tone_at(position: usize) -> u8 {
    u8::try_from(position % HUES.len()).map_or(1, |tone| tone + 1)
}

/// How many notes stand before „+2 weitere".
const NOTES_SHOWN: usize = 4;

/// „Mo" … „So" by `Day::weekday` (1 = Monday).
pub(super) fn weekday_name(weekday: u8) -> &'static str {
    match weekday {
        1 => "Mo",
        2 => "Di",
        3 => "Mi",
        4 => "Do",
        5 => "Fr",
        6 => "Sa",
        _ => "So",
    }
}

/// „Mo 08.02.2027".
fn day_name(day: Day) -> String {
    format!("{} {}", weekday_name(day.weekday()), day.german())
}

/// The address of a semester in the view shown now; from the Übersicht a semester is its week.
/// `None` past the years a semester key can have.
pub(super) fn semester_href(view: PlanView, key: Option<SemesterKey>) -> Option<String> {
    let view = if view == PlanView::Overview { PlanView::Week } else { view };
    key.map(|key| StudyplanUrl { sem: Some(key.key()), view, ..Default::default() }.path())
}

/// Where „+ Modul" leads: the catalog's modules that fit the semester (`fits=`), within „Mein
/// Studiengang" where the stored program is in the snapshot (the one kind of address its slug may
/// stand in, A.10). Tracked.
pub(super) fn add_module_href(resolved: Option<MineResolved>, key: SemesterKey) -> String {
    let program = resolved
        .and_then(|resolved| resolved.0.get())
        .filter(|info| info.exact)
        .map(|info| ProgramScope { program_slug: info.program.slug, ..Default::default() });
    let query = CatalogQuery { program, fits: Some(FitsFilter::all(&key.key())), ..Default::default() };
    CatalogUrl { query, ..Default::default() }.path()
}

/// Seconds since 1970, for when a module was planned; 0 outside the browser, which plans nothing.
fn now_secs() -> u64 {
    #[cfg(feature = "csr")]
    {
        let millis = web_sys::js_sys::Date::now();
        if millis.is_finite() && millis > 0.0 {
            return (millis / 1000.0) as u64;
        }
    }
    0
}

// ---------- the head ----------

/// The lower end of a placeholder's credits as the plan states them („6", „10–24", „7,5").
fn lower_credits(text: &str) -> Option<f64> {
    let number: String = text.trim().chars().take_while(|c| c.is_ascii_digit() || *c == ',' || *c == '.').collect();
    number.replace(',', ".").parse::<f64>().ok().filter(|credits| credits.is_finite() && *credits >= 0.0)
}

/// The placeholders standing in a semester that count for it: those of this semester alone (a row
/// spanning several stands under its first and is left out of its sum, A.4) that no module fills
/// yet (a filler's own credits count instead). Their number, and their lower credits.
fn open_placeholders(doc: &PlanDoc, key: SemesterKey) -> (usize, f64) {
    doc.placeholders_in(key)
        .into_iter()
        .filter(|p| p.span.0 == p.span.1 && doc.fillers(p.pid).is_empty())
        .fold((0, 0.0), |(count, credits), p| (count + 1, credits + p.credits.as_deref().and_then(lower_credits).unwrap_or(0.0)))
}

/// The numbers under the head, numbers first: „1. FS · 5 Module · 32 LP". The Fachsemester only
/// with a known Studienbeginn, placeholders only where there are any, credits only where some are
/// known. Empty with nothing to count and no Fachsemester.
pub(super) fn sum_line(fs: Option<u8>, modules: usize, placeholders: usize, credits: f64) -> String {
    let mut parts = Vec::new();
    if let Some(fs) = fs {
        parts.push(format!("{fs}. FS"));
    }
    if modules > 0 {
        parts.push(format::modules(i64::try_from(modules).unwrap_or(i64::MAX)));
    }
    if placeholders > 0 {
        parts.push(format!("{placeholders} Platzhalter"));
    }
    if (modules > 0 || placeholders > 0) && credits > 0.0 {
        parts.push(format!("{} LP", format::number(credits)));
    }
    parts.join(" · ")
}

/// What the head says under its numbers, if anything.
#[derive(Clone, Debug, PartialEq)]
enum HeadLine {
    /// No module in the semester; `placeholders`: but rows of the Regelstudienplan stand in it.
    Empty { placeholders: bool },
    /// Before the current semester: its past dates are gone from the data.
    Past,
    /// Not a Termin of the semester is published yet.
    Unpublished,
}

/// Whether the data's semester lies before the snapshot's current one. Read from the data's own
/// `meta`: a memo of the data then needs no `current` beside it, from which the data is derived
/// (R16).
fn is_past(data: &StudyplanData) -> bool {
    data.meta.current_semester.as_deref().and_then(SemesterKey::parse).is_some_and(|current| data.key < current)
}

/// Which line applies: the empty semester, else the past one, else one without published dates.
fn head_line(data: &StudyplanData, placeholders: usize) -> Option<HeadLine> {
    if data.ids.is_empty() {
        return Some(HeadLine::Empty { placeholders: placeholders > 0 });
    }
    if is_past(data) {
        return Some(HeadLine::Past);
    }
    data.counts.is_empty().then_some(HeadLine::Unpublished)
}

/// The head of one semester: ‹ „WiSe 2026/27" › (the semesters around it, in the view shown), on a
/// phone „Anpassen" at its right end, and the numbers under it. Then, where it applies, the line of
/// an empty semester (with „+ Modul"), of a past one, or of one whose dates are not out yet.
#[component]
pub(super) fn SemesterHead(ctx: PlanCtx) -> impl IntoView {
    let key = ctx.key;
    let view = Memo::new(move |_| ctx.url.with(|url| url.view));
    let prev = Memo::new(move |_| semester_href(view.get(), key.get().plus(-1)));
    let next = Memo::new(move |_| semester_href(view.get(), key.get().plus(1)));
    // The placeholders come from the plan itself, so the semester is worked out from the address
    // and the plan here, as `wanted` does it (R16), not read from `key`.
    let held = Memo::new(move |_| {
        let (url, current) = (ctx.url.get(), ctx.current.get());
        ctx.plan.map(|plan| plan.with(|doc| open_placeholders(doc, key_of(&url, current, doc, ctx.today)))).unwrap_or_default()
    });
    let fs = Memo::new(move |_| {
        let start = ctx.mine.and_then(MyProgram::start)?;
        fachsemester(key.get(), start)
    });
    let sum = Memo::new(move |_| {
        let (placeholders, open_credits) = held.get();
        let fs = fs.get();
        ctx.data.with(|data| match data {
            Ok(data) => {
                let credits = data
                    .ids
                    .iter()
                    .filter_map(|id| data.modules.iter().find(|row| row.id == *id).and_then(|row| row.credits))
                    .fold(open_credits, |sum, credits| sum + credits);
                sum_line(fs, data.ids.len(), placeholders, credits)
            }
            Err(_) => sum_line(fs, 0, 0, 0.0),
        })
    });
    let line = Memo::new(move |_| {
        let (placeholders, _) = held.get();
        ctx.data.with(|data| data.as_ref().ok().and_then(|data| head_line(data, placeholders)))
    });
    let resolved = MineResolved::expect();

    view! {
        <div class="sp-head">
            <a class="icon-btn" href=move || prev.get() aria-disabled=move || prev.with(Option::is_none).then_some("true") aria-label="Voriges Semester" title="Voriges Semester">
                <Icon name="chevron-left"/>
            </a>
            <h2>{move || key.get().label()}</h2>
            <a class="icon-btn" href=move || next.get() aria-disabled=move || next.with(Option::is_none).then_some("true") aria-label="Nächstes Semester" title="Nächstes Semester">
                <Icon name="chevron-right"/>
            </a>
            <SheetToggle/>
        </div>
        {move || {
            let text = sum.get();
            (!text.is_empty()).then(|| view! { <p class="sp-sum">{text}</p> })
        }}
        {move || {
            let label = key.get().label();
            match line.get() {
                Some(HeadLine::Empty { placeholders }) => {
                    let text = if placeholders { ": noch kein Modul geplant." } else { ": nichts geplant." };
                    let add = move || add_module_href(resolved, key.get());
                    // „+ Modul" as it ends the legend of a semester with modules.
                    view! {
                        <p class="hint">{label}{text}</p>
                        <ul class="sp-mods"><li><a class="mini" href=add>"+ Modul"</a></li></ul>
                    }
                    .into_any()
                }
                Some(HeadLine::Past) => view! {
                    <p class="note quiet"><span>{format!("{label} ist vorbei; vergangene Termine fehlen im Datenstand.")}</span></p>
                }
                .into_any(),
                Some(HeadLine::Unpublished) => view! {
                    <p class="note quiet"><span>{format!("{label}: noch keine Termine veröffentlicht.")}</span></p>
                }
                .into_any(),
                None => ().into_any(),
            }
        }}
    }
}

// ---------- the modules ----------

/// One planned module in the legend.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct LegendItem {
    id: String,
    title: Option<String>,
    /// „8 LP", „8 LP · keine Termine", „nicht im Modulkatalog".
    small: Option<String>,
    hue: &'static str,
}

/// The legend of a semester: its planned modules in plan order, each in its tone. A module without
/// a dated Termin says so where the semester's dates are there to be had (neither past, whose dates
/// are gone, nor unpublished, which the head says for all of them).
fn legend(data: &StudyplanData) -> Vec<LegendItem> {
    let dated = !data.counts.is_empty() && !is_past(data);
    data.ids
        .iter()
        .enumerate()
        .map(|(position, id)| {
            let row = data.modules.iter().find(|row| row.id == *id);
            let credits = row.and_then(|row| row.credits).map(|credits| format!("{} LP", format::number(credits)));
            let undated = dated && !data.schedule.iter().any(|date| date.module_id == *id && date.ord.is_some());
            let small = match (row, credits, undated) {
                (None, _, _) => Some("nicht im Modulkatalog".to_string()),
                (Some(_), Some(credits), true) => Some(format!("{credits} · keine Termine")),
                (Some(_), None, true) => Some("keine Termine".to_string()),
                (Some(_), credits, false) => credits,
            };
            LegendItem { id: id.clone(), title: row.map(|row| row.title.clone()), small, hue: hue(tone_at(position)) }
        })
        .collect()
}

/// The module beside the plan, and while one is on its way there (`pending`), that one: what
/// links to it is marked in the next frame.
fn open_shown(ctx: PlanCtx) -> Memo<Option<String>> {
    let going = Pending::expect();
    Memo::new(move |_| {
        let target = going.filter(|going| going.change() == Some(Change::Aside)).and_then(|going| going.search_on(url::STUDYPLAN));
        match target {
            Some(search) => StudyplanUrl::parse(&search).open,
            None => ctx.url.with(|url| url.open.clone()),
        }
    })
}

/// The semester's planned modules: tone, number, title and credits, each a link to the module
/// beside the plan; „+ Modul" at the end finds another one that fits.
#[component]
pub(super) fn ModuleLegend(ctx: PlanCtx) -> impl IntoView {
    let items = Memo::new(move |_| ctx.data.with(|data| data.as_ref().map(legend).unwrap_or_default()));
    let open = open_shown(ctx);
    let resolved = MineResolved::expect();
    let add = move || add_module_href(resolved, ctx.key.get());
    view! {
        <ul class="sp-mods">
            <For
                each=move || items.get()
                key=|item| item.clone()
                children=move |item: LegendItem| {
                    let id = item.id.clone();
                    let current = Memo::new(move |_| open.with(|open| open.as_deref() == Some(id.as_str())));
                    let id = item.id.clone();
                    let href = move || ctx.url.with(|url| url.with_open(Some(&id), None).path());
                    view! {
                        <li>
                            <a class=item.hue href=href aria-current=move || current.get().then_some("true") data-noscroll="">
                                <i></i>
                                <span class="mono">{item.id.clone()}</span>
                                {item.title.clone().map(|title| view! { <span>{title}</span> })}
                                {item.small.clone().map(|small| view! { <small>{small}</small> })}
                            </a>
                        </li>
                    }
                }
            />
            <li><a class="mini" href=add>"+ Modul"</a></li>
        </ul>
    }
}

// ---------- the Merkliste ----------

/// The marked modules offered in `key`'s half of the year and not planned into it, in the order
/// of the Merkliste: number and title.
fn offered(data: &BookmarksData, key: SemesterKey, planned: &[String]) -> Vec<(String, String)> {
    let season = if key.winter { Season::Winter } else { Season::Summer };
    data.rows
        .iter()
        .filter(|row| data.offered_in(season, &row.id) && !planned.contains(&row.id))
        .map(|row| (row.id.clone(), row.title.clone()))
        .collect()
}

/// „Aus der Merkliste (n)": the marked modules this semester could still take, closed until asked,
/// each with „Einplanen". A past semester has it too, as it has „+ Modul": a plan taken over from
/// a later Fachsemester fills its first semesters with what was taken.
#[component]
pub(super) fn FromBookmarks(ctx: PlanCtx) -> impl IntoView {
    let bookmarks = Bookmarks::expect();
    let marked = Memo::new(move |_| bookmarks.map(|bookmarks| bookmarks.marks().into_iter().map(|mark| mark.id).collect::<Vec<_>>()).unwrap_or_default());
    let loaded = Memo::new(move |_| {
        let ids = marked.get();
        if ids.is_empty() {
            return None;
        }
        ctx.source.with_value(|source| source.as_ref().and_then(|source| source.run(|db| pages::bookmarks(db, &ids, BookmarkSort::Added, false)).ok()))
    });
    // The semester and what is planned into it, from the semester's data (its ids are the plan's).
    let list = Memo::new(move |_| {
        ctx.data.with(|data| match data {
            Ok(data) => loaded.with(|marked| marked.as_ref().map(|marked| offered(marked, data.key, &data.ids)).unwrap_or_default()),
            Err(_) => Vec::new(),
        })
    });
    let any = Memo::new(move |_| list.with(|list| !list.is_empty()));
    // The modules whose „Einplanen" was clicked and whose write has not landed yet: the button
    // answers at once, the plan follows after the next frame (R21), and then the module leaves the
    // list. The write takes the module out of this set again, so wherever the list offers it next
    // (another semester, or this one after it was taken out) it can be planned once more.
    let planning = RwSignal::new(BTreeSet::<String>::new());

    move || {
        any.get().then(|| {
            view! {
                <details class="sp-marked">
                    <summary>"Aus der Merkliste ("{move || list.with(Vec::len)}")"</summary>
                    <ul>
                        <For
                            each=move || list.get()
                            key=|(id, _)| id.clone()
                            children=move |(id, title): (String, String)| {
                                let busy = Memo::new({
                                    let id = id.clone();
                                    move |_| planning.with(|planning| planning.contains(&id))
                                });
                                let plan_it = {
                                    let id = id.clone();
                                    move |_| {
                                        let Some(plan) = ctx.plan else { return };
                                        if busy.get_untracked() {
                                            return;
                                        }
                                        planning.update(|planning| {
                                            planning.insert(id.clone());
                                        });
                                        let key = ctx.key.get_untracked();
                                        let id = id.clone();
                                        plan.update_after_paint(move |doc| {
                                            doc.plan(key, &id, now_secs(), None);
                                            // The pages hear of the write in the same pass as of
                                            // this, so a planned module leaves the list without a
                                            // flash of „Einplanen"; one the plan refused (a full
                                            // semester) is offered again.
                                            planning.update(|planning| {
                                                planning.remove(&id);
                                            });
                                        });
                                    }
                                };
                                view! {
                                    <li>
                                        <span class="mono">{id}</span>
                                        <span>{title}</span>
                                        <button class="mini" type="button" aria-busy=move || busy.get().then_some("true") on:click=plan_it>
                                            {move || if busy.get() { "Eingeplant" } else { "Einplanen" }}
                                        </button>
                                    </li>
                                }
                            }
                        />
                    </ul>
                </details>
            }
        })
    }
}

// ---------- the notes ----------

/// What the notes need of the semester's data besides its timetable: the planned modules' titles
/// and halves of the year, whether any Termin of the semester is published at all, and whether
/// the semester is past.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct About {
    pub titles: BTreeMap<String, String>,
    pub turnus: BTreeMap<String, TurnusSeason>,
    pub dated: bool,
    pub past: bool,
}

impl About {
    fn of(data: &StudyplanData) -> Self {
        About {
            titles: data.titles(),
            turnus: data.modules.iter().filter_map(|row| Some((row.id.clone(), row.turnus_season.as_ref()?.known()?))).collect(),
            dated: !data.counts.is_empty(),
            past: is_past(data),
        }
    }

    /// A module's title; its number where the catalog has none.
    fn title(&self, module: &str) -> String {
        self.titles.get(module).cloned().unwrap_or_else(|| format!("Modul {module}"))
    }

    /// The title an event goes by in a line: its first planned module's.
    fn title_of(&self, event: &Event) -> String {
        event.modules.first().map_or_else(|| event.title.clone(), |module| self.title(module))
    }
}

/// One line of the notes: a warning or a quiet line, and the module (and Termin) its link opens
/// beside the plan.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct Note {
    pub warn: bool,
    pub text: String,
    pub module: Option<String>,
    pub row: Option<RowKey>,
}

impl Note {
    fn warn(text: String, module: Option<String>, row: Option<RowKey>) -> Self {
        Note { warn: true, text, module, row }
    }

    fn quiet(text: String, module: Option<String>, row: Option<RowKey>) -> Self {
        Note { warn: false, text, module, row }
    }
}

/// What an event is called in a line: its type as QIS writes it, else its first kind.
pub(super) fn kind_word(event: &Event) -> String {
    match event.type_raw.as_deref().map(str::trim).filter(|kind| !kind.is_empty()) {
        Some(kind) => kind.to_string(),
        None => event.kinds.iter().next().map_or("Termin", EventKind::label).to_string(),
    }
}

/// „0 von 2 Terminen frei: Laborausbildung · Programmierpraktikum": a choice none of whose options
/// is free, in the words of „1 von 4 wählen: …" — the neutral „Terminen", and the type as QIS
/// writes it after the colon, never glued to it („Laborausbildungterminen"). The notes of the
/// semester name the module (`title`), the module beside the plan its event.
pub(super) fn blocked_line(event: &Event, title: &str) -> String {
    format!("0 von {} Terminen frei: {} · {}", event.visible_options().len(), kind_word(event), title.trim())
}

/// „, A-Woche" for a row held in A weeks, „, B-Woche" in B weeks.
fn week_of(event: &Event, row: usize) -> &'static str {
    match event.rows.get(row).and_then(|row| row.date.rhythm.as_ref()) {
        Some(rhythm) if rhythm.is(Rhythm::WeekA) => ", A-Woche",
        Some(rhythm) if rhythm.is(Rhythm::WeekB) => ", B-Woche",
        _ => "",
    }
}

/// The first row of the first option still shown of a choice: where its line points.
fn first_option_row(event: &Event) -> Option<RowKey> {
    let option = event.visible_options().into_iter().next()?;
    event.rows.iter().find(|row| row.option == Some(option) && row.hidden.is_none()).and_then(|row| row.key)
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

/// What avoids a soft exam warning, and whose Termin that is: a Termin on the `avoid` day of one of
/// the two modules, free of the other module's Termin in the warning. The line names two modules,
/// so it names this one — „Mathematik IT-1: Zweittermin 11.03. passt", „…: Erstermin 25.02. passt"
/// where it is that module's earliest — and opens it, where the Termin is to be seen. Else
/// („andere Termine passen", no module) only a change of both avoids it. (`ExamWarning::avoid` is
/// only a day, so the Termine on it are looked up.)
fn avoid_text(warning: &ExamWarning, avoid: Day, termine: &[(String, Vec<TerminAt>)], about: &About) -> (String, Option<String>) {
    let list = |module: &str| termine.iter().find(|(id, _)| id == module).map_or(&[][..], |(_, list)| list.as_slice());
    let issue = |termin: &Termin| list(&termin.module_id).iter().find(|at| at.day == warning.day && at.termin == *termin);
    let both = || ("andere Termine passen".to_string(), None);
    let (Some(a), Some(b)) = (issue(&warning.a), issue(&warning.b)) else {
        return both();
    };
    for (mine, other) in [(a, b), (b, a)] {
        let module = &mine.termin.module_id;
        if let Some(index) = list(module).iter().position(|at| at.day == avoid && at != mine && exams::collision(at, other).is_none()) {
            let rank = if index == 0 { "Erstermin" } else { "Zweittermin" };
            return (format!("{}: {rank} {} passt", about.title(module), avoid.short()), Some(module.clone()));
        }
    }
    both()
}

/// An exam warning as the notes say it: „Prüfungen gleichzeitig: Mo 08.02.2027 11:00 · A · B" (at
/// the start of the overlap, as a clash says it), „0 min von Zentralcampus nach Senftenberg: Mo
/// 08.02.2027 · A bis 10:00 · B ab 10:00". It opens the later exam's module. A soft one is quiet
/// and says what avoids it; it opens the module whose Termin does, where there is one.
fn exam_note(warning: &ExamWarning, about: &About, termine: &[(String, Vec<TerminAt>)]) -> Note {
    let (a, b) = (about.title(&warning.a.module_id), about.title(&warning.b.module_id));
    let mut text = match &warning.kind {
        WarningKind::Overlap => {
            format!("Prüfungen gleichzeitig: {} {} · {a} · {b}", day_name(warning.day), clock(warning.a.from.max(warning.b.from)))
        }
        WarningKind::Tight { gap, from, to } => format!(
            "{gap} min von {} nach {}: {} · {a} bis {} · {b} ab {}",
            campus_name(from),
            campus_name(to),
            day_name(warning.day),
            clock(warning.a.to),
            clock(warning.b.from)
        ),
    };
    let later = Some(warning.b.module_id.clone());
    if warning.hard {
        return Note::warn(text, later, None);
    }
    let Some(avoid) = warning.avoid else {
        return Note::quiet(text, later, None);
    };
    let (avoids, whose) = avoid_text(warning, avoid, termine, about);
    text.push_str(" · ");
    text.push_str(&avoids);
    Note::quiet(text, whose.or(later), None)
}

/// The clashes that open one Termin at one weekday and start, as one line of the notes.
struct ClashLine {
    /// The Termin the line opens, `(event, row)`: the later side of each of its clashes.
    b: (usize, usize),
    weekday: u8,
    /// The start of the overlap.
    from: Option<u16>,
    /// Whether the rows have held days; two patterns meet only on a weekday.
    dated: bool,
    /// The first day of its clashes.
    first: Day,
    /// The held days on the line's weekday that the rows its clashes name share.
    shared: BTreeSet<Day>,
    /// The days of its clashes beyond those: a clash names the rows of its first day, and other
    /// rows of the same two events may meet on further days.
    beyond: usize,
    /// What meets the Termin, by title: each kind of it („Vorlesung, A-Woche") once.
    others: Vec<(String, Vec<String>)>,
}

impl ClashLine {
    /// On how many days the Termin meets another: a day that two others meet it on counts once
    /// (a lecture met by a lecture and a Praktikum on the same 15 Tuesdays is 15, not 30), the
    /// A weeks of one and the B weeks of another add up.
    fn days(&self) -> usize {
        self.shared.len() + self.beyond
    }
}

/// The clashes of a timetable as the notes say them: one line per Termin they open, weekday and
/// start, so a lecture met by another module's lecture in A weeks and by its Übung in B weeks is
/// one line („15 Termine überschneiden sich: Di 07:30 · Programmierpraktikum (Vorlesung) ·
/// Elektrische … (Vorlesung, A-Woche; Übung, B-Woche)"), not two that open the same Termin.
fn clash_lines(table: &Timetable, about: &About) -> Vec<ClashLine> {
    let mut lines: Vec<ClashLine> = Vec::new();
    for clash in &table.clashes {
        let (Some(a), Some(b)) = (table.events.get(clash.a.0), table.events.get(clash.b.0)) else { continue };
        let (Some(row_a), Some(row_b)) = (a.rows.get(clash.a.1), b.rows.get(clash.b.1)) else { continue };
        // The overlap starts with the later of the two.
        let (from, weekday, dated) = (row_a.from.max(row_b.from), clash.first.weekday(), clash.days > 0);
        let same = |line: &ClashLine| line.b == clash.b && line.weekday == weekday && line.from == from && line.dated == dated;
        let index = match lines.iter().position(same) {
            Some(index) => index,
            None => {
                lines.push(ClashLine { b: clash.b, weekday, from, dated, first: clash.first, shared: BTreeSet::new(), beyond: 0, others: Vec::new() });
                lines.len() - 1
            }
        };
        let Some(line) = lines.get_mut(index) else { continue };
        let held: BTreeSet<Day> = row_a.occ.days.iter().copied().filter(|day| day.weekday() == weekday).collect();
        let shared: Vec<Day> = row_b.occ.days.iter().copied().filter(|day| held.contains(day)).collect();
        line.beyond += clash.days.saturating_sub(shared.len());
        line.shared.extend(shared);
        line.first = line.first.min(clash.first);
        let (title, what) = (about.title_of(a), format!("{}{}", kind_word(a), week_of(a, clash.a.1)));
        match line.others.iter_mut().find(|(other, _)| *other == title) {
            Some((_, whats)) if whats.contains(&what) => {}
            Some((_, whats)) => whats.push(what),
            None => line.others.push((title, vec![what])),
        }
    }
    lines
}

/// The notes of a semester's timetable (A.5), warnings first: Termine that overlap, a choice with
/// no free option, exams at once or too close for the way between them; then quiet lines: what is
/// still to choose, the Standort to pick, exams a second sitting avoids, exams whose place is
/// open, and planned modules without Termine (only where the semester's dates are published and
/// not past). `choice`: the Standort the visitor chose.
pub(super) fn notes(table: &Timetable, about: &About, choice: TownChoice) -> Vec<Note> {
    let mut warnings = Vec::new();
    let mut quiet = Vec::new();
    let termine = exams::termine(&table.exams, &table.modules);

    for line in clash_lines(table, about) {
        let Some(b) = table.events.get(line.b.0) else { continue };
        let from = line.from.map(clock).unwrap_or_default();
        let lead = match (line.dated, line.days()) {
            // Two patterns without a date: only the weekday is known.
            (false, _) => format!("Überschneidung: {} {from}", weekday_name(line.weekday)),
            (true, 1) => format!("1 Termin überschneidet sich: {} {from}", day_name(line.first)),
            (true, days) => format!("{days} Termine überschneiden sich: {} {from}", weekday_name(line.weekday)),
        };
        // The Termin the line opens first, then what meets it.
        let others: Vec<String> = line.others.iter().map(|(title, whats)| format!("{title} ({})", whats.join("; "))).collect();
        let text = format!("{lead} · {} ({}{}) · {}", about.title_of(b), kind_word(b), week_of(b, line.b.1), others.join(" · "));
        let row = b.rows.get(line.b.1).and_then(|row| row.key);
        warnings.push(Note::warn(text, b.modules.first().cloned(), row));
    }
    for event in table.blocked.iter().filter_map(|index| table.events.get(*index)) {
        let text = blocked_line(event, &about.title_of(event));
        warnings.push(Note::warn(text, event.modules.first().cloned(), first_option_row(event)));
    }
    let exam_notes: Vec<Note> = table.exam_warnings.iter().map(|warning| exam_note(warning, about, &termine)).collect();
    warnings.extend(exam_notes.iter().filter(|note| note.warn).cloned());

    for (index, event) in table.events.iter().enumerate() {
        if event.hidden.is_some() || !event.unresolved() || table.blocked.contains(&index) {
            continue;
        }
        let text = format!("1 von {} wählen: {} · {}", event.visible_options().len(), kind_word(event), about.title_of(event));
        quiet.push(Note::quiet(text, event.modules.first().cloned(), first_option_row(event)));
    }
    if !table.tracks.is_empty() && table.town.is_none() && choice == TownChoice::Derive {
        let names: Vec<String> = table.tracks.iter().map(|module| about.title(module)).collect();
        let text = format!("Standort wählen: {} · Cottbus oder Senftenberg", names.join(", "));
        quiet.push(Note::quiet(text, table.tracks.iter().next().cloned(), None));
    }
    quiet.extend(exam_notes.into_iter().filter(|note| !note.warn));
    for (day, modules) in &table.place_unknown {
        let text = format!("Ort offen: {} Prüfungen am {}", modules.len(), day_name(*day));
        quiet.push(Note::quiet(text, modules.first().cloned(), None));
    }
    if about.dated && !about.past {
        // A module the catalog does not know has no Termine either; the legend says it is not in
        // the catalog, which is the reason.
        for module in table.without_dates.iter().filter(|module| about.titles.contains_key(*module)) {
            let season = match about.turnus.get(module) {
                Some(TurnusSeason::Summer) if table.key.winter => " (laut Beschreibung im Sommer)",
                Some(TurnusSeason::Winter) if !table.key.winter => " (laut Beschreibung im Winter)",
                _ => "",
            };
            let text = format!("Keine Termine im {}: {}{season}", table.key.label(), about.title(module));
            quiet.push(Note::quiet(text, Some(module.clone()), None));
        }
    }
    warnings.extend(quiet);
    warnings
}

/// The notes under the modules: at most four lines; „+2 weitere" shows the rest in place, until
/// another semester is shown. Each line opens its module beside the plan, at its Termin.
#[component]
pub(super) fn Notes(ctx: PlanCtx) -> impl IntoView {
    let about = Memo::new(move |_| ctx.data.with(|data| data.as_ref().map(About::of).unwrap_or_default()));
    let choice = Memo::new(move |_| ctx.selection.with(|(_, selection)| selection.town));
    let all = Memo::new(move |_| {
        let choice = choice.get();
        ctx.table.with(|table| table.as_ref().map(|table| (table.key, about.with(|about| notes(table, about, choice)))))
    });
    // The semester whose notes are all shown.
    let expanded = RwSignal::new(None::<SemesterKey>);
    let cut = Memo::new(move |_| {
        let open = expanded.get();
        all.with(|all| match all {
            Some((key, notes)) if open != Some(*key) && notes.len() > NOTES_SHOWN => {
                (notes.iter().take(NOTES_SHOWN).cloned().collect(), notes.len() - NOTES_SHOWN)
            }
            Some((_, notes)) => (notes.clone(), 0),
            None => (Vec::new(), 0),
        })
    });
    let any = Memo::new(move |_| cut.with(|(shown, _)| !shown.is_empty()));
    let more = Memo::new(move |_| cut.with(|(_, more)| *more));
    let expand = move |_| expanded.set(all.with_untracked(|all| all.as_ref().map(|(key, _)| *key)));

    move || {
        any.get().then(|| {
            view! {
                <div class="sp-notes">
                    <For each=move || cut.with(|(shown, _)| shown.clone()) key=|note| note.clone() children=move |note: Note| note_line(ctx, note)/>
                    {move || match more.get() {
                        0 => None,
                        more => Some(view! { <button class="mini" type="button" on:click=expand>{format!("+{more} weitere")}</button> }),
                    }}
                </div>
            }
        })
    }
}

/// One note: a link to its module beside the plan, or plain text where it names none.
fn note_line(ctx: PlanCtx, note: Note) -> impl IntoView {
    let icon = note.warn.then(|| view! { <Icon name="triangle-alert"/> });
    let class = if note.warn { "note" } else { "note quiet" };
    match note.module {
        Some(module) => {
            let row = note.row;
            let href = move || ctx.url.with(|url| url.with_open(Some(&module), row).path());
            view! { <a class=class href=href data-noscroll="">{icon}<span>{note.text}</span></a> }.into_any()
        }
        None => view! { <p class=class>{icon}<span>{note.text}</span></p> }.into_any(),
    }
}

// ---------- what is derived ----------

/// The one line that says what the page derived rather than read (R12), with only the parts that
/// apply: „Abgeleitet: Vorlesungszeit 05.10.2026–31.01.2027, vorlesungsfrei 21.12.–03.01.,
/// Übungsgruppen, Standort Cottbus."
pub(super) fn derived_line(table: &Timetable) -> Option<String> {
    let mut parts = Vec::new();
    if let Some((first, last)) = table.facts.lecture {
        parts.push(format!("Vorlesungszeit {}–{}", first.german(), last.german()));
    }
    if !table.facts.breaks.is_empty() {
        let breaks: Vec<String> = table.facts.breaks.iter().map(|(first, last)| format!("{}–{}", first.short(), last.short())).collect();
        parts.push(format!("vorlesungsfrei {}", breaks.join(", ")));
    }
    // Groups QIS names are read; parallel slots of which the SWS need one are derived.
    let groups = table
        .events
        .iter()
        .any(|event| event.hidden.is_none() && matches!(event.attendance, Attendance::OneOf { basis: Basis::Sws { .. }, .. }));
    if groups {
        parts.push("Übungsgruppen".to_string());
    }
    // Every plan derives a town from its modules, but the town decides something only where a
    // module is taught in both towns.
    if let Some(town) = table.town.filter(|_| table.town_derived && !table.tracks.is_empty()) {
        parts.push(format!("Standort {}", town.label()));
    }
    (!parts.is_empty()).then(|| format!("Abgeleitet: {}.", parts.join(", ")))
}

/// „Abgeleitet: …": one muted line under the notes.
#[component]
pub(super) fn DerivedLine(ctx: PlanCtx) -> impl IntoView {
    let line = Memo::new(move |_| ctx.table.with(|table| table.as_ref().and_then(derived_line)));
    move || line.get().map(|text| view! { <p class="hint">{text}</p> })
}

#[cfg(test)]
mod tests {
    use catalog::rows_detail::EventDate;
    use catalog::studyplan::Placeholder;
    use catalog::timetable::clash::Clash;
    use catalog::timetable::day::minutes;
    use catalog::timetable::facts::SemesterFacts;
    use catalog::timetable::kind::{kinds_of, Class};
    use catalog::timetable::model::Row;
    use catalog::timetable::occur::Occurrences;
    use catalog::timetable::select::Town;

    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    fn day(text: &str) -> Day {
        Day::parse(text).unwrap()
    }

    fn row(event: &str, fp: u32, weekday: i64, time: (&str, &str), rhythm: &str, option: Option<usize>) -> Row {
        let date = EventDate {
            semester_key: "2026W".into(),
            semester_label: "WiSe 2026/27".into(),
            event_id: event.into(),
            event_number: None,
            event_title: String::new(),
            event_type: None,
            group_name: None,
            weekday: Some(weekday),
            start_time: Some(time.0.into()),
            end_time: Some(time.1.into()),
            rhythm: Some(Code::parse(rhythm)),
            rhythm_raw: None,
            first_date: None,
            last_date: None,
            room: None,
            campus: None,
            instructor: None,
            comment: None,
            source_url: None,
        };
        Row {
            key: Some(RowKey { event: event.parse().unwrap(), fp }),
            ord: Some(1),
            date,
            cancelled_dates: None,
            occ: Occurrences::default(),
            from: minutes(time.0),
            to: minutes(time.1),
            option,
            hidden: None,
        }
    }

    fn event(id: &str, type_raw: &str, module: &str, rows: Vec<Row>, attendance: Attendance) -> Event {
        let kinds = kinds_of(Some(type_raw));
        Event {
            id: id.into(),
            number: None,
            title: format!("Veranstaltung {id}"),
            type_raw: Some(type_raw.into()),
            kinds,
            class: if kinds.contains(EventKind::Lecture) { Class::Lecture } else { Class::Other },
            modules: vec![module.into()],
            tone: 1,
            attendance,
            chosen: None,
            rows,
            hidden: None,
            source_url: None,
        }
    }

    fn termin(module: &str, event: &str, from: &str, to: &str, campus: &str) -> Termin {
        Termin {
            module_id: module.into(),
            event_id: event.into(),
            from: minutes(from).unwrap(),
            to: minutes(to).unwrap(),
            campus: vec![Code::parse(campus)],
            room: None,
        }
    }

    /// Informatik's first semester as the notes see it: 149408 (12102's Vorlesung at Sachsendorf)
    /// against 148134's A weeks, 148369's four Übungen without a free one, 148304 still to choose,
    /// two exams at once, a hop to Senftenberg, an exam of unknown place and a module without
    /// Termine.
    fn first_semester() -> (Timetable, About) {
        let key = key("2026W");
        let mut facts = SemesterFacts::derive(key, None, &[]);
        facts.lecture = Some((day("2026-10-05"), day("2027-01-31")));
        facts.breaks = vec![(day("2026-12-21"), day("2027-01-03"))];
        let choice = |n: usize| Attendance::OneOf { options: (0..n).map(|option| vec![option]).collect(), basis: Basis::Sws { stated: 2.0 } };
        let events = vec![
            event("148134", "Vorlesung", "12107", vec![row("148134", 1, 2, ("07:30", "09:00"), "week_a", None)], Attendance::All),
            event("148369", "Übung", "12104", (0..4u32).map(|n| row("148369", 10 + n, 1 + i64::from(n), ("15:30", "17:00"), "weekly", Some(n as usize))).collect(), choice(4)),
            event("148304", "Übung", "11112", (0..3u32).map(|n| row("148304", 20 + n, 3, ("09:15", "10:45"), "weekly", Some(n as usize))).collect(), choice(3)),
            event("149408", "Vorlesung", "12102", vec![row("149408", 2, 2, ("07:30", "09:00"), "weekly", None)], Attendance::All),
        ];
        let table = Timetable {
            key,
            facts,
            modules: vec!["12104".into(), "12107".into(), "12102".into(), "11112".into(), "13000".into()],
            events,
            exams: Vec::new(),
            tracks: BTreeSet::new(),
            town: Some(Town::Cottbus),
            town_derived: true,
            clashes: vec![Clash { a: (0, 0), b: (3, 0), first: day("2026-10-06"), days: 8 }],
            blocked: vec![1],
            exam_warnings: vec![
                ExamWarning {
                    kind: WarningKind::Overlap,
                    day: day("2027-02-08"),
                    a: termin("11112", "1", "11:00", "13:00", "zentralcampus"),
                    b: termin("12104", "2", "11:00", "13:00", "zentralcampus"),
                    hard: true,
                    avoid: None,
                },
                ExamWarning {
                    kind: WarningKind::Tight { gap: 0, from: Code::parse("zentralcampus"), to: Code::parse("senftenberg") },
                    day: day("2027-02-08"),
                    a: termin("12107", "3", "08:00", "10:00", "zentralcampus"),
                    b: termin("12102", "4", "10:00", "12:00", "senftenberg"),
                    hard: false,
                    avoid: Some(day("2027-03-11")),
                },
            ],
            place_unknown: vec![(day("2027-02-10"), vec!["12104".into(), "11112".into()])],
            without_dates: vec!["13000".into()],
        };
        let titles = [
            ("12104", "Entwicklung von Softwaresystemen"),
            ("12107", "Elektrische und elektronische Grundlagen der Informatik"),
            ("12102", "Programmierpraktikum"),
            ("11112", "Mathematik IT-1"),
            ("13000", "Algorithmieren und Programmieren"),
        ];
        let about = About {
            titles: titles.iter().map(|(id, title)| (id.to_string(), title.to_string())).collect(),
            turnus: [("13000".to_string(), TurnusSeason::Summer)].into_iter().collect(),
            dated: true,
            past: false,
        };
        (table, about)
    }

    #[test]
    fn the_notes_say_what_overlaps_and_what_is_left_to_choose() {
        let (table, about) = first_semester();
        let lines = notes(&table, &about, TownChoice::Derive);
        let texts: Vec<(bool, &str)> = lines.iter().map(|note| (note.warn, note.text.as_str())).collect();
        assert_eq!(
            texts,
            vec![
                (true, "8 Termine überschneiden sich: Di 07:30 · Programmierpraktikum (Vorlesung) · Elektrische und elektronische Grundlagen der Informatik (Vorlesung, A-Woche)"),
                (true, "0 von 4 Terminen frei: Übung · Entwicklung von Softwaresystemen"),
                (true, "Prüfungen gleichzeitig: Mo 08.02.2027 11:00 · Mathematik IT-1 · Entwicklung von Softwaresystemen"),
                (false, "1 von 3 wählen: Übung · Mathematik IT-1"),
                (false, "0 min von Zentralcampus nach Senftenberg: Mo 08.02.2027 · Elektrische und elektronische Grundlagen der Informatik bis 10:00 · Programmierpraktikum ab 10:00 · andere Termine passen"),
                (false, "Ort offen: 2 Prüfungen am Mi 10.02.2027"),
                (false, "Keine Termine im WiSe 2026/27: Algorithmieren und Programmieren (laut Beschreibung im Sommer)"),
            ]
        );
        // A clash opens the later module at its Termin, a choice its first option.
        assert_eq!((lines[0].module.as_deref(), lines[0].row), (Some("12102"), Some(RowKey { event: 149408, fp: 2 })));
        assert_eq!((lines[3].module.as_deref(), lines[3].row), (Some("11112"), Some(RowKey { event: 148304, fp: 20 })));
        // A past semester, or one without published dates, says nothing of missing Termine.
        let past = About { past: true, ..about.clone() };
        assert!(!notes(&table, &past, TownChoice::Derive).iter().any(|note| note.text.starts_with("Keine Termine")));
        let undated = About { dated: false, ..about.clone() };
        assert!(!notes(&table, &undated, TownChoice::Derive).iter().any(|note| note.text.starts_with("Keine Termine")));
        // Nor of a module the catalog does not know: the legend says that.
        let mut unknown = about.clone();
        unknown.titles.remove("13000");
        assert!(!notes(&table, &unknown, TownChoice::Derive).iter().any(|note| note.text.starts_with("Keine Termine")));
    }

    #[test]
    fn a_termin_met_twice_is_one_line() {
        let (mut table, about) = first_semester();
        // 12107's Übung in B weeks meets 149408 as its lecture does in A weeks.
        table.events.push(event("148135", "Übung", "12107", vec![row("148135", 3, 2, ("07:30", "09:00"), "week_b", None)], Attendance::All));
        table.clashes.push(Clash { a: (4, 0), b: (3, 0), first: day("2026-10-13"), days: 7 });
        let lines = notes(&table, &about, TownChoice::Derive);
        let clashes: Vec<&Note> = lines.iter().filter(|note| note.text.contains("überschneid")).collect();
        assert_eq!(
            clashes.iter().map(|note| note.text.as_str()).collect::<Vec<_>>(),
            vec!["15 Termine überschneiden sich: Di 07:30 · Programmierpraktikum (Vorlesung) · Elektrische und elektronische Grundlagen der Informatik (Vorlesung, A-Woche; Übung, B-Woche)"]
        );
        assert_eq!(clashes.first().map(|note| note.row), Some(Some(RowKey { event: 149408, fp: 2 })));
        // A day two Termine meet it on counts once: 149408 held on 15 Tuesdays, the lecture on the
        // A weeks' 8 of them, the Übung on the same 8 instead of the B weeks' 7.
        let tuesdays: Vec<Day> = (0..15).map(|week| day("2026-10-06").plus(week * 7)).collect();
        let a_weeks: Vec<Day> = tuesdays.iter().copied().step_by(2).collect();
        let b_weeks: Vec<Day> = tuesdays.iter().copied().skip(1).step_by(2).collect();
        let held = |table: &mut Timetable, event: usize, days: &[Day]| {
            if let Some(row) = table.events.get_mut(event).and_then(|event| event.rows.get_mut(0)) {
                row.occ.days = days.to_vec();
            }
        };
        held(&mut table, 3, &tuesdays);
        held(&mut table, 0, &a_weeks);
        held(&mut table, 4, &b_weeks);
        assert!(notes(&table, &about, TownChoice::Derive)[0].text.starts_with("15 Termine überschneiden sich: Di 07:30 · "));
        held(&mut table, 4, &a_weeks);
        table.clashes[1].days = 8;
        assert!(notes(&table, &about, TownChoice::Derive)[0].text.starts_with("8 Termine überschneiden sich: Di 07:30 · "));
        // Another start is another line.
        if let Some(row) = table.events.get_mut(4).and_then(|event| event.rows.get_mut(0)) {
            row.from = minutes("08:00");
        }
        assert_eq!(notes(&table, &about, TownChoice::Derive).iter().filter(|note| note.text.contains("überschneid")).count(), 2);
    }

    fn at(day_text: &str, termin: Termin) -> TerminAt {
        TerminAt { day: day(day_text), events: vec![termin.event_id.clone()], termin, unplaced: false }
    }

    #[test]
    fn a_soft_exam_warning_names_the_module_whose_termin_passes() {
        let about = About {
            titles: [("11405", "Algorithmische Graphentheorie"), ("11112", "Mathematik IT-1")].iter().map(|(id, title)| (id.to_string(), title.to_string())).collect(),
            ..About::default()
        };
        let (first, second) = (termin("11405", "7", "10:00", "12:00", "zentralcampus"), termin("11112", "8", "11:00", "13:00", "zentralcampus"));
        let warning = |avoid: &str| ExamWarning {
            kind: WarningKind::Overlap,
            day: day("2027-03-10"),
            a: first.clone(),
            b: second.clone(),
            hard: false,
            avoid: Some(day(avoid)),
        };
        let lead = "Prüfungen gleichzeitig: Mi 10.03.2027 11:00 · Algorithmische Graphentheorie · Mathematik IT-1 · ";
        // The earlier module's Erstermin: the line opens that module, not the later one.
        let termine = vec![
            ("11405".to_string(), vec![at("2027-02-25", termin("11405", "6", "10:00", "12:00", "zentralcampus")), at("2027-03-10", first.clone())]),
            ("11112".to_string(), vec![at("2027-03-10", second.clone()), at("2027-03-24", termin("11112", "9", "11:00", "13:00", "zentralcampus"))]),
        ];
        let note = exam_note(&warning("2027-02-25"), &about, &termine);
        assert_eq!((note.warn, note.text.as_str(), note.module.as_deref()), (false, &*format!("{lead}Algorithmische Graphentheorie: Erstermin 25.02. passt"), Some("11405")));
        // The later module's Zweittermin.
        let note = exam_note(&warning("2027-03-24"), &about, &termine);
        assert_eq!((note.text.as_str(), note.module.as_deref()), (&*format!("{lead}Mathematik IT-1: Zweittermin 24.03. passt"), Some("11112")));
        // Only a change of both: the later module.
        let note = exam_note(&warning("2027-03-31"), &about, &termine);
        assert_eq!((note.text.as_str(), note.module.as_deref()), (&*format!("{lead}andere Termine passen"), Some("11112")));
    }

    #[test]
    fn a_clash_without_dates_and_one_of_a_single_day_say_so() {
        let (mut table, about) = first_semester();
        table.clashes[0].days = 0;
        table.clashes[0].first = day("1969-12-30");
        assert!(notes(&table, &about, TownChoice::Derive)[0].text.starts_with("Überschneidung: Di 07:30 · "));
        table.clashes[0].days = 1;
        table.clashes[0].first = day("2027-02-23");
        assert!(notes(&table, &about, TownChoice::Derive)[0].text.starts_with("1 Termin überschneidet sich: Di 23.02.2027 07:30 · "));
    }

    #[test]
    fn a_choice_of_several_kinds_or_an_unusual_type_is_named_apart() {
        let (mut table, about) = first_semester();
        let retype = |table: &mut Timetable, type_raw: &str| {
            if let Some(event) = table.events.get_mut(1) {
                event.type_raw = Some(type_raw.into());
                event.kinds = kinds_of(Some(type_raw));
            }
        };
        // QIS's „Laborausbildung" stands after the colon as QIS writes it, never glued to the
        // Termine („Laborausbildungterminen").
        retype(&mut table, "Laborausbildung");
        assert_eq!(notes(&table, &about, TownChoice::Derive)[1].text, "0 von 4 Terminen frei: Laborausbildung · Entwicklung von Softwaresystemen");
        retype(&mut table, "Vorlesung/Übung");
        assert_eq!(notes(&table, &about, TownChoice::Derive)[1].text, "0 von 4 Terminen frei: Vorlesung/Übung · Entwicklung von Softwaresystemen");
    }

    #[test]
    fn the_standort_is_asked_only_while_nothing_decides_it() {
        let (mut table, about) = first_semester();
        table.tracks = ["12104".to_string()].into_iter().collect();
        let asks = |table: &Timetable, choice| {
            notes(table, &about, choice).iter().any(|note| note.text == "Standort wählen: Entwicklung von Softwaresystemen · Cottbus oder Senftenberg")
        };
        // A derived town decides; so does a stored one, „Beide" included.
        assert!(!asks(&table, TownChoice::Derive));
        table.town = None;
        table.town_derived = false;
        assert!(asks(&table, TownChoice::Derive));
        assert!(!asks(&table, TownChoice::Both));
    }

    #[test]
    fn the_derived_line_names_only_what_applies() {
        let (mut table, _) = first_semester();
        // The town is derived in every plan, but decides something only with a module of both towns.
        assert_eq!(derived_line(&table).as_deref(), Some("Abgeleitet: Vorlesungszeit 05.10.2026–31.01.2027, vorlesungsfrei 21.12.–03.01., Übungsgruppen."));
        table.tracks = ["12104".to_string()].into_iter().collect();
        assert_eq!(
            derived_line(&table).as_deref(),
            Some("Abgeleitet: Vorlesungszeit 05.10.2026–31.01.2027, vorlesungsfrei 21.12.–03.01., Übungsgruppen, Standort Cottbus.")
        );
        // Groups QIS names are not derived.
        for event in &mut table.events {
            if let Attendance::OneOf { basis, .. } = &mut event.attendance {
                *basis = Basis::Groups;
            }
        }
        table.facts.breaks.clear();
        table.facts.lecture = None;
        assert_eq!(derived_line(&table).as_deref(), Some("Abgeleitet: Standort Cottbus."));
        table.tracks.clear();
        assert_eq!(derived_line(&table), None);
    }

    #[test]
    fn each_module_keeps_its_tone() {
        // A position's tone in the legend is the tone the timetable gives the module's events.
        let hues: Vec<&str> = (0..10).map(|position| hue(tone_at(position))).collect();
        assert_eq!(hues, vec!["t-ice", "t-sun", "t-violet", "t-teal", "t-green", "t-coral", "t-rose", "t-slate", "t-ice", "t-sun"]);
        assert_eq!((hue(0), hue(9)), ("t-ice", "t-ice"));
    }

    #[test]
    fn the_head_counts_numbers_first() {
        assert_eq!(sum_line(Some(1), 5, 0, 32.0), "1. FS · 5 Module · 32 LP");
        assert_eq!(sum_line(Some(1), 4, 1, 30.0), "1. FS · 4 Module · 1 Platzhalter · 30 LP");
        assert_eq!(sum_line(None, 1, 0, 7.5), "1 Modul · 7,5 LP");
        // Nothing known of the credits, or nothing planned: no „0 LP".
        assert_eq!(sum_line(None, 2, 0, 0.0), "2 Module");
        assert_eq!((sum_line(Some(3), 0, 0, 0.0), sum_line(None, 0, 0, 0.0)), ("3. FS".to_string(), String::new()));
        // A placeholder counts with the lower end of what it states, in its own semester alone, and
        // only while nothing fills it.
        assert_eq!((lower_credits("6"), lower_credits("10–24"), lower_credits("7,5"), lower_credits("")), (Some(6.0), Some(10.0), Some(7.5), None));
        let placeholder = |pid: u32, span: (u8, u8), credits: &str| Placeholder {
            pid,
            semester: key("2026W"),
            program_id: "079-82-2008".into(),
            ord: i64::from(pid),
            span,
            credits: Some(credits.into()),
            kind: None,
            caption: String::new(),
            name: format!("Zeile {pid}"),
        };
        let mut doc = PlanDoc {
            placeholders: vec![placeholder(1, (1, 1), "6"), placeholder(2, (1, 6), "60"), placeholder(3, (1, 1), "10–24"), placeholder(4, (1, 1), "5")],
            ..PlanDoc::default()
        };
        assert!(doc.plan(key("2026W"), "12104", 1, Some(4)));
        assert_eq!(open_placeholders(&doc, key("2026W")), (2, 16.0));
        assert_eq!(open_placeholders(&doc, key("2027S")), (0, 0.0));
    }

    #[test]
    fn the_semester_arrows_keep_the_view() {
        assert_eq!(semester_href(PlanView::Dates, Some(key("2027S"))).as_deref(), Some("/studyplan?sem=2027S&view=dates"));
        assert_eq!(semester_href(PlanView::Overview, Some(key("2026W"))).as_deref(), Some("/studyplan?sem=2026W"));
        assert_eq!(semester_href(PlanView::Week, None), None);
    }

    #[test]
    fn the_merkliste_offers_what_the_semester_could_take() {
        let row = |id: &str| catalog::rows::CatalogRow {
            id: id.into(),
            title: format!("Modul {id}"),
            title_de: None,
            title_en: None,
            credits: Some(6.0),
            turnus_season: None,
            turnus_parity: None,
            offer_status: Code::parse("active"),
            teaches_german: None,
            teaches_english: None,
            is_fues: false,
            is_limited: None,
            department: None,
            teaching_events: 0,
            exam_form: None,
            responsible: None,
            kind: None,
            plan_semester: None,
            area: None,
        };
        let data = BookmarksData {
            rows: vec![row("11103"), row("12330"), row("12104")],
            winter: vec!["11103".into(), "12104".into()],
            summer: vec!["12330".into()],
            missing: Vec::new(),
        };
        // Offered in the winter and not planned into it; the summer's own in a summer.
        assert_eq!(offered(&data, key("2026W"), &["12104".to_string()]), vec![("11103".to_string(), "Modul 11103".to_string())]);
        assert_eq!(offered(&data, key("2027S"), &[]), vec![("12330".to_string(), "Modul 12330".to_string())]);
    }
}
