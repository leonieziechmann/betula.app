//! The top of the Stundenplan: its head („Stundenplan WiSe 2026/27" and the numbers under it), the
//! planned modules as a legend of their tones, the exams that collide as red alerts, the one quiet
//! line of overlaps per week and open choices (opened, a line each), the marked modules that could
//! still be planned, and the one muted line under the calendar that says what the page derived
//! rather than read (R12). Owner's redesign of 2026-09-25: no stack of cards, numbers first.
//!
//! Every part reads the memos of `PlanCtx` it needs. Where a part needs two things, it takes them
//! from sibling memos (`key` and `data`, `table` and a memo of `selection`), never a memo together
//! with the memo it is derived from (R16); what depends on the plan's placeholders works the
//! semester out itself (`key_of`), as `wanted` does. Texts are the plan's own terms, numbers first
//! (design A.3, A.5).

use std::collections::{BTreeMap, BTreeSet};

use catalog::filter::{CatalogQuery, FitsFilter, ProgramScope};
use catalog::labels::{Campus, Code};
use catalog::pages::{self, BookmarksData, StudyplanData};
use catalog::studyplan::PlanDoc;
use catalog::timetable::clash::{self, Overlap, Weeks, When};
use catalog::timetable::day::{clock, Day};
use catalog::timetable::exams::{self, ExamWarning, Termin, TerminAt, WarningKind};
use catalog::timetable::kind::EventKind;
use catalog::timetable::model::{Attendance, Basis, Event, Timetable};
use catalog::timetable::rowkey::RowKey;
use catalog::timetable::select::TownChoice;
use catalog::timetable::semester::SemesterKey;
use catalog::timetable::views::short_title;
use catalog::url::{self, BookmarkSort, CatalogUrl, PlanView, Season, StudyplanUrl};
use leptos::prelude::*;

use super::{key_of, PlanCtx, SheetToggle};
use crate::bookmarks::Bookmarks;
use crate::format;
use crate::myprogram::MineResolved;
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

/// The line under the head: the program, then the numbers („Informatik B.Sc. · 5 Module · 32 LP").
/// Placeholders only where there are any, credits only where some are known. Empty with nothing
/// to count and no program.
pub(super) fn sum_line(program: Option<&str>, modules: usize, placeholders: usize, credits: f64) -> String {
    let mut parts = Vec::new();
    if let Some(program) = program.filter(|program| !program.is_empty()) {
        parts.push(program.to_string());
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

/// The head: „Stundenplan WiSe 2026/27" (the one semester the page shows, owner's redesign of
/// 2026-09-25), on a phone „Anpassen" at its right end, and the program and the numbers under it.
/// Then, where it applies, the line of an empty semester (with „+ Modul"), of a past one, or of
/// one whose dates are not out yet.
#[component]
pub(super) fn SemesterHead(ctx: PlanCtx) -> impl IntoView {
    let key = ctx.key;
    // The placeholders come from the plan itself, so the semester is worked out from the address
    // and the plan here, as `wanted` does it (R16), not read from `key`.
    let held = Memo::new(move |_| {
        let (url, current) = (ctx.url.get(), ctx.current.get());
        ctx.plan.map(|plan| plan.with(|doc| open_placeholders(doc, key_of(&url, current, doc, ctx.today)))).unwrap_or_default()
    });
    let resolved = MineResolved::expect();
    // The plan's program: „Mein Studiengang" for now, as „Informatik B.Sc.".
    let program = Memo::new(move |_| {
        let info = resolved.and_then(|resolved| resolved.0.get())?;
        Some(format!("{} {}", info.program.name, info.program.degree()).trim().to_string())
    });
    let sum = Memo::new(move |_| {
        let (placeholders, open_credits) = held.get();
        let program = program.get();
        ctx.data.with(|data| match data {
            Ok(data) => {
                let credits = data
                    .ids
                    .iter()
                    .filter_map(|id| data.modules.iter().find(|row| row.id == *id).and_then(|row| row.credits))
                    .fold(open_credits, |sum, credits| sum + credits);
                sum_line(program.as_deref(), data.ids.len(), placeholders, credits)
            }
            Err(_) => sum_line(program.as_deref(), 0, 0, 0.0),
        })
    });
    let line = Memo::new(move |_| {
        let (placeholders, _) = held.get();
        ctx.data.with(|data| data.as_ref().ok().and_then(|data| head_line(data, placeholders)))
    });

    view! {
        <div class="sp-head">
            <h1>"Stundenplan "<span>{move || key.get().label()}</span></h1>
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
    /// What the week names the module by, small before its title as the grid's key: its
    /// abbreviation („EvS"), else its number.
    key: String,
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
    let names = data.slot_names();
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
            let abbrev = data.abbrevs.get(id).filter(|abbrev| names.get(id) == Some(*abbrev));
            let key = abbrev.cloned().unwrap_or_else(|| id.clone());
            LegendItem { id: id.clone(), key, title: row.map(|row| row.title.clone()), small, hue: hue(tone_at(position)) }
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
                                <span class="mono" title=item.id.clone()>{item.key.clone()}</span>
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

/// What the notes need of the semester's data besides its timetable: the planned modules' titles.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct About {
    pub titles: BTreeMap<String, String>,
}

impl About {
    fn of(data: &StudyplanData) -> Self {
        About { titles: data.titles() }
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

/// One line of the opened overlaps: a warning (an overlap, a choice without a free option) or a
/// quiet line, and the module (and Termin) its link opens beside the plan.
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

// ---------- exams that collide ----------

/// What avoids a soft exam warning, and whose Termin that is: a Termin on the `avoid` day of one of
/// the two modules, free of the other module's Termin in the warning — „Mathematik IT-1:
/// Zweittermin 11.03. passt", „…: Erstermin 25.02. passt" where it is that module's earliest. Else
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

/// An exam problem as a red alert under the head (owner, 2026-09-25: „groß rot"): two exams at
/// once, or too close for the way between their campuses. `text` names both, `when` says the day
/// and their times, and where another sitting avoids it, which. It opens the later exam's module,
/// or the module whose sitting avoids it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct Alert {
    pub text: String,
    pub when: String,
    pub module: String,
}

/// The exam warnings of a timetable as alerts, in its order.
pub(super) fn exam_alerts(table: &Timetable, about: &About) -> Vec<Alert> {
    let termine = exams::termine(&table.exams, &table.modules);
    table.exam_warnings.iter().map(|warning| exam_alert(warning, about, &termine)).collect()
}

/// „Prüfung A und Prüfung B überschneiden sich" / „Mo 08.02.2027, beide 11:00–13:00";
/// „Prüfung A und Prüfung B: nur 0 min zwischen Zentralcampus und Senftenberg" / „Mo 08.02.2027,
/// 08:00–10:00 und 10:00–12:00".
fn exam_alert(warning: &ExamWarning, about: &About, termine: &[(String, Vec<TerminAt>)]) -> Alert {
    let (a, b) = (about.title(&warning.a.module_id), about.title(&warning.b.module_id));
    let span = |termin: &Termin| format!("{}–{}", clock(termin.from), clock(termin.to));
    let times = if (warning.a.from, warning.a.to) == (warning.b.from, warning.b.to) {
        format!("beide {}", span(&warning.a))
    } else {
        format!("{} und {}", span(&warning.a), span(&warning.b))
    };
    let text = match &warning.kind {
        WarningKind::Overlap => format!("Prüfung {a} und Prüfung {b} überschneiden sich"),
        WarningKind::Tight { gap, from, to } => {
            format!("Prüfung {a} und Prüfung {b}: nur {gap} min zwischen {} und {}", campus_name(from), campus_name(to))
        }
    };
    let mut when = format!("{}, {times}", day_name(warning.day));
    let mut module = warning.b.module_id.clone();
    if let Some(avoid) = warning.avoid.filter(|_| !warning.hard) {
        let (avoids, whose) = avoid_text(warning, avoid, termine, about);
        when.push_str(" · ");
        when.push_str(&avoids);
        module = whose.unwrap_or(module);
    }
    Alert { text, when, module }
}

/// The exams that collide, red, directly under the head in every view; each opens its module
/// beside the plan.
#[component]
pub(super) fn ExamAlerts(ctx: PlanCtx) -> impl IntoView {
    let about = Memo::new(move |_| ctx.data.with(|data| data.as_ref().map(About::of).unwrap_or_default()));
    let alerts = Memo::new(move |_| ctx.table.with(|table| table.as_ref().map(|table| about.with(|about| exam_alerts(table, about))).unwrap_or_default()));
    let any = Memo::new(move |_| alerts.with(|alerts| !alerts.is_empty()));
    let alert = move |alert: Alert| {
        let module = alert.module.clone();
        let href = move || ctx.url.with(|url| url.with_open(Some(&module), None).path());
        view! {
            <a class="sp-alert" href=href data-noscroll="">
                <Icon name="triangle-alert"/>
                <span><b>{alert.text}</b><small>{alert.when}</small></span>
            </a>
        }
    };
    move || {
        any.get().then(|| {
            view! {
                <div class="sp-alerts">
                    <For each=move || alerts.get() key=|alert| alert.clone() children=alert/>
                </div>
            }
        })
    }
}

// ---------- overlaps and open choices ----------

/// The one quiet line of overlaps and open choices (owner, 2026-09-25: „es gibt x
/// Überschneidungen pro Woche", aufklappbar): its parts, and opened, a line each.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Summary {
    /// „2 Überschneidungen pro Woche", „1 an einzelnen Tagen" (`true`: an overlap, drawn red),
    /// „3 Wahlen offen", „Standort offen".
    pub parts: Vec<(bool, String)>,
    pub lines: Vec<Note>,
}

/// „Vorlesung Programmierpraktikum": the kind as QIS names it and the short name of its module;
/// the name alone where it begins with its kind („Tutorium Mathematik IT-1").
fn named(event: &Event, about: &About) -> String {
    let (kind, title) = (kind_word(event), short_title(&about.title_of(event)));
    if title.starts_with(&kind) {
        return title;
    }
    format!("{kind} {title}")
}

/// „Di 07:30–09:00 · Vorlesung Programmierpraktikum × Vorlesung Elektrische … · A-Woche", „Do
/// 12.11. 13:45–15:15 · …" for a single day. It opens the later side at its Termin.
fn overlap_line(table: &Timetable, about: &About, overlap: &Overlap) -> Option<Note> {
    let (a, b) = (table.events.get(overlap.a.0)?, table.events.get(overlap.b.0)?);
    let (when, weeks) = match overlap.when {
        When::Weekly(weeks) => (weekday_name(overlap.weekday).to_string(), weeks),
        When::Day(day) => (format!("{} {}", weekday_name(day.weekday()), day.short()), Weeks::All),
    };
    let weeks = match weeks {
        Weeks::A => " · A-Woche",
        Weeks::B => " · B-Woche",
        Weeks::All => "",
    };
    let text = format!("{when} {}–{} · {} × {}{weeks}", clock(overlap.from), clock(overlap.to), named(a, about), named(b, about));
    let row = b.rows.get(overlap.b.1).and_then(|row| row.key);
    Some(Note::warn(text, b.modules.first().cloned(), row))
}

/// „1 Überschneidung", „2 Überschneidungen"; „1 Wahl", „2 Wahlen".
fn counted(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// The line of overlaps and open choices of a timetable. Overlaps count per week (`clash::
/// per_week`: a weekly overlap once, in the week `shown`, for „A/B" the larger of the A and the B
/// week) and on single days apart; opened, the line lists the overlaps of that week and those of
/// single days, then the choices without a free option, the choices still open, and the Standort
/// to pick (`choice`: the one the visitor chose). Empty parts: no line.
pub(super) fn summary(table: &Timetable, about: &About, overlaps: &[Overlap], shown: Weeks, choice: TownChoice) -> Summary {
    let mut parts = Vec::new();
    let (weekly, days) = (clash::per_week(overlaps, shown), clash::on_days(overlaps));
    if weekly > 0 {
        parts.push((true, format!("{} pro Woche", counted(weekly, "Überschneidung", "Überschneidungen"))));
    }
    if days > 0 {
        let text = match weekly {
            0 => format!("{} an einzelnen Tagen", counted(days, "Überschneidung", "Überschneidungen")),
            _ => format!("{days} an einzelnen Tagen"),
        };
        parts.push((true, text));
    }
    let shows = |overlap: &&Overlap| match overlap.when {
        When::Weekly(weeks) => weeks.in_week(shown),
        When::Day(_) => true,
    };
    let mut lines: Vec<Note> = overlaps.iter().filter(shows).filter_map(|overlap| overlap_line(table, about, overlap)).collect();

    let mut open = 0;
    for event in table.blocked.iter().filter_map(|index| table.events.get(*index)) {
        open += 1;
        lines.push(Note::warn(blocked_line(event, &about.title_of(event)), event.modules.first().cloned(), first_option_row(event)));
    }
    for (index, event) in table.events.iter().enumerate() {
        if event.hidden.is_some() || !event.unresolved() || table.blocked.contains(&index) {
            continue;
        }
        open += 1;
        let text = format!("1 von {} wählen: {} · {}", event.visible_options().len(), kind_word(event), about.title_of(event));
        lines.push(Note::quiet(text, event.modules.first().cloned(), first_option_row(event)));
    }
    if open > 0 {
        parts.push((false, format!("{} offen", counted(open, "Wahl", "Wahlen"))));
    }
    if !table.tracks.is_empty() && table.town.is_none() && choice == TownChoice::Derive {
        let names: Vec<String> = table.tracks.iter().map(|module| about.title(module)).collect();
        parts.push((false, "Standort offen".to_string()));
        let text = format!("Standort wählen: {} · Cottbus oder Senftenberg", names.join(", "));
        lines.push(Note::quiet(text, table.tracks.iter().next().cloned(), None));
    }
    Summary { parts, lines }
}

/// The one quiet line under the modules, closed until asked: „2 Überschneidungen pro Woche · 3
/// Wahlen offen"; opened, a compact line each, which opens its module beside the plan. In „Woche"
/// it counts the week the A/B switch shows; elsewhere „A/B". No line with nothing to say.
#[component]
pub(super) fn Overlaps(ctx: PlanCtx) -> impl IntoView {
    let about = Memo::new(move |_| ctx.data.with(|data| data.as_ref().map(About::of).unwrap_or_default()));
    let choice = Memo::new(move |_| ctx.selection.with(|(_, selection)| selection.town));
    let shown = Memo::new(move |_| {
        let week = ctx.url.with(|url| matches!(url.view, PlanView::Week | PlanView::Overview));
        if week {
            ctx.weeks.get()
        } else {
            Weeks::All
        }
    });
    let built = Memo::new(move |_| {
        let (shown, choice) = (shown.get(), choice.get());
        ctx.table.with(|table| {
            table
                .as_ref()
                .map(|table| about.with(|about| summary(table, about, &clash::overlaps(&table.events), shown, choice)))
                .unwrap_or_default()
        })
    });
    let any = Memo::new(move |_| built.with(|built| !built.parts.is_empty()));
    let parts = Memo::new(move |_| built.with(|built| built.parts.clone()));
    let lines = Memo::new(move |_| built.with(|built| built.lines.clone()));
    let head = move || {
        parts
            .get()
            .into_iter()
            .enumerate()
            .map(|(i, (warn, text))| view! { {(i > 0).then_some(" · ")}<span class:no=warn>{text}</span> })
            .collect_view()
    };
    move || {
        any.get().then(|| {
            view! {
                <details class="sp-overlaps">
                    <summary>{head}</summary>
                    <ul>
                        <For each=move || lines.get() key=|note| note.clone() children=move |note: Note| overlap_item(ctx, note)/>
                    </ul>
                </details>
            }
        })
    }
}

/// One line of the opened overlaps: a link to its module beside the plan, or plain text where it
/// names none.
fn overlap_item(ctx: PlanCtx, note: Note) -> impl IntoView {
    let class = if note.warn { "warn" } else { "" };
    match note.module {
        Some(module) => {
            let row = note.row;
            let href = move || ctx.url.with(|url| url.with_open(Some(&module), row).path());
            view! { <li><a class=class href=href data-noscroll="">{note.text}</a></li> }.into_any()
        }
        None => view! { <li><span class=class>{note.text}</span></li> }.into_any(),
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

/// „Abgeleitet: …": one small muted line under the calendar.
#[component]
pub(super) fn DerivedLine(ctx: PlanCtx) -> impl IntoView {
    let line = Memo::new(move |_| ctx.table.with(|table| table.as_ref().and_then(derived_line)));
    move || line.get().map(|text| view! { <p class="hint sp-derived">{text}</p> })
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
            room_short: None,
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
        let about = About { titles: titles.iter().map(|(id, title)| (id.to_string(), title.to_string())).collect() };
        (table, about)
    }

    /// The fixture's overlap: 148134's A-week lecture against 149408 at 07:30 on Tuesdays.
    fn a_week_overlap() -> Overlap {
        Overlap { a: (0, 0), b: (3, 0), weekday: 2, from: 450, to: 540, when: When::Weekly(Weeks::A) }
    }

    /// A part or a line as `(drawn red, text)`.
    type Texts<'a> = Vec<(bool, &'a str)>;

    fn texts(summary: &Summary) -> (Texts<'_>, Texts<'_>) {
        let parts = summary.parts.iter().map(|(warn, text)| (*warn, text.as_str())).collect();
        let lines = summary.lines.iter().map(|note| (note.warn, note.text.as_str())).collect();
        (parts, lines)
    }

    #[test]
    fn one_line_counts_the_overlaps_per_week_and_the_open_choices() {
        let (table, about) = first_semester();
        let summary = summary(&table, &about, &[a_week_overlap()], Weeks::All, TownChoice::Derive);
        assert_eq!(
            texts(&summary),
            (
                vec![(true, "1 Überschneidung pro Woche"), (false, "2 Wahlen offen")],
                vec![
                    (true, "Di 07:30–09:00 · Vorlesung Elektrische und elektronische … × Vorlesung Programmierpraktikum · A-Woche"),
                    (true, "0 von 4 Terminen frei: Übung · Entwicklung von Softwaresystemen"),
                    (false, "1 von 3 wählen: Übung · Mathematik IT-1"),
                ]
            )
        );
        // An overlap opens the later module at its Termin, a choice its first option.
        assert_eq!((summary.lines[0].module.as_deref(), summary.lines[0].row), (Some("12102"), Some(RowKey { event: 149408, fp: 2 })));
        assert_eq!((summary.lines[2].module.as_deref(), summary.lines[2].row), (Some("11112"), Some(RowKey { event: 148304, fp: 20 })));
        // The B week has no overlap: it counts and lists none.
        let b = summary_of(&table, &about, &[a_week_overlap()], Weeks::B);
        assert_eq!(b.parts.first().map(|(_, text)| text.as_str()), Some("2 Wahlen offen"));
        assert!(!b.lines.iter().any(|note| note.text.contains('×')));
        // A single day counts apart, beside a weekly one or alone.
        let day = Overlap { when: When::Day(day("2026-11-03")), ..a_week_overlap() };
        let both = summary_of(&table, &about, &[a_week_overlap(), day.clone()], Weeks::All);
        assert_eq!(both.parts[..2], [(true, "1 Überschneidung pro Woche".to_string()), (true, "1 an einzelnen Tagen".to_string())]);
        assert!(both.lines[1].text.starts_with("Di 03.11. 07:30–09:00 · "));
        let alone = summary_of(&table, &about, &[day.clone(), day], Weeks::All);
        assert_eq!(alone.parts[0], (true, "2 Überschneidungen an einzelnen Tagen".to_string()));
        // Nothing to say: no line.
        let mut calm = table.clone();
        calm.blocked.clear();
        calm.events.retain(|event| !event.unresolved());
        assert!(summary_of(&calm, &about, &[], Weeks::All).parts.is_empty());
    }

    fn summary_of(table: &Timetable, about: &About, overlaps: &[Overlap], shown: Weeks) -> Summary {
        summary(table, about, overlaps, shown, TownChoice::Derive)
    }

    #[test]
    fn exams_that_collide_are_red_alerts() {
        let (table, about) = first_semester();
        let alerts = exam_alerts(&table, &about);
        let texts: Vec<(&str, &str, &str)> = alerts.iter().map(|alert| (alert.text.as_str(), alert.when.as_str(), alert.module.as_str())).collect();
        assert_eq!(
            texts,
            vec![
                ("Prüfung Mathematik IT-1 und Prüfung Entwicklung von Softwaresystemen überschneiden sich", "Mo 08.02.2027, beide 11:00–13:00", "12104"),
                (
                    "Prüfung Elektrische und elektronische Grundlagen der Informatik und Prüfung Programmierpraktikum: nur 0 min zwischen Zentralcampus und Senftenberg",
                    "Mo 08.02.2027, 08:00–10:00 und 10:00–12:00 · andere Termine passen",
                    "12102"
                ),
            ]
        );
    }

    fn at(day_text: &str, termin: Termin) -> TerminAt {
        TerminAt { day: day(day_text), events: vec![termin.event_id.clone()], termin, unplaced: false }
    }

    #[test]
    fn a_soft_exam_warning_names_the_module_whose_termin_passes() {
        let about = About { titles: [("11405", "Algorithmische Graphentheorie"), ("11112", "Mathematik IT-1")].iter().map(|(id, title)| (id.to_string(), title.to_string())).collect() };
        let (first, second) = (termin("11405", "7", "10:00", "12:00", "zentralcampus"), termin("11112", "8", "11:00", "13:00", "zentralcampus"));
        let warning = |avoid: &str| ExamWarning {
            kind: WarningKind::Overlap,
            day: day("2027-03-10"),
            a: first.clone(),
            b: second.clone(),
            hard: false,
            avoid: Some(day(avoid)),
        };
        let lead = "Mi 10.03.2027, 10:00–12:00 und 11:00–13:00 · ";
        // The earlier module's Erstermin: the alert opens that module, not the later one.
        let termine = vec![
            ("11405".to_string(), vec![at("2027-02-25", termin("11405", "6", "10:00", "12:00", "zentralcampus")), at("2027-03-10", first.clone())]),
            ("11112".to_string(), vec![at("2027-03-10", second.clone()), at("2027-03-24", termin("11112", "9", "11:00", "13:00", "zentralcampus"))]),
        ];
        let alert = exam_alert(&warning("2027-02-25"), &about, &termine);
        assert_eq!(alert.text, "Prüfung Algorithmische Graphentheorie und Prüfung Mathematik IT-1 überschneiden sich");
        assert_eq!((alert.when.as_str(), alert.module.as_str()), (&*format!("{lead}Algorithmische Graphentheorie: Erstermin 25.02. passt"), "11405"));
        // The later module's Zweittermin.
        let alert = exam_alert(&warning("2027-03-24"), &about, &termine);
        assert_eq!((alert.when.as_str(), alert.module.as_str()), (&*format!("{lead}Mathematik IT-1: Zweittermin 24.03. passt"), "11112"));
        // Only a change of both: the later module.
        let alert = exam_alert(&warning("2027-03-31"), &about, &termine);
        assert_eq!((alert.when.as_str(), alert.module.as_str()), (&*format!("{lead}andere Termine passen"), "11112"));
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
        let blocked = |table: &Timetable| summary_of(table, &about, &[], Weeks::All).lines[0].text.clone();
        // QIS's „Laborausbildung" stands after the colon as QIS writes it, never glued to the
        // Termine („Laborausbildungterminen").
        retype(&mut table, "Laborausbildung");
        assert_eq!(blocked(&table), "0 von 4 Terminen frei: Laborausbildung · Entwicklung von Softwaresystemen");
        retype(&mut table, "Vorlesung/Übung");
        assert_eq!(blocked(&table), "0 von 4 Terminen frei: Vorlesung/Übung · Entwicklung von Softwaresystemen");
    }

    #[test]
    fn the_standort_is_asked_only_while_nothing_decides_it() {
        let (mut table, about) = first_semester();
        table.tracks = ["12104".to_string()].into_iter().collect();
        let asks = |table: &Timetable, choice| {
            let summary = summary(table, &about, &[], Weeks::All, choice);
            summary.parts.contains(&(false, "Standort offen".to_string()))
                && summary.lines.iter().any(|note| note.text == "Standort wählen: Entwicklung von Softwaresystemen · Cottbus oder Senftenberg")
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
        let informatik = Some("Informatik B.Sc.");
        assert_eq!(sum_line(informatik, 5, 0, 32.0), "Informatik B.Sc. · 5 Module · 32 LP");
        assert_eq!(sum_line(informatik, 4, 1, 30.0), "Informatik B.Sc. · 4 Module · 1 Platzhalter · 30 LP");
        assert_eq!(sum_line(None, 1, 0, 7.5), "1 Modul · 7,5 LP");
        // Nothing known of the credits, or nothing planned: no „0 LP".
        assert_eq!(sum_line(None, 2, 0, 0.0), "2 Module");
        assert_eq!((sum_line(informatik, 0, 0, 0.0), sum_line(None, 0, 0, 0.0)), ("Informatik B.Sc.".to_string(), String::new()));
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
