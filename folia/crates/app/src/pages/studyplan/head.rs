//! The top of the Stundenplan: its head („Stundenplan WiSe 2026/27" and the numbers under it), the
//! exams that collide as red alerts, the one quiet line of overlaps per week, open choices and what
//! is hidden (opened, a line each), the marked modules that could still be planned, what the empty
//! week says in its middle, and the one muted line under the calendar that says what the page
//! derived rather than read (R12). Owner's redesign of 2026-09-25: no stack of cards, numbers
//! first. The planned modules are a list of their own beside the week (`modules.rs`).
//!
//! Every part reads the memos of `PlanCtx` it needs. Where a part needs two things, it takes them
//! from sibling memos (`key` and `data`, `table` and a memo of `selection`), never a memo together
//! with the memo it is derived from (R16); what depends on the plan's placeholders works the
//! semester out itself (`key_of`), as `wanted` does. Texts are the plan's own terms, numbers first
//! (design A.3, A.5).

use std::collections::{BTreeMap, BTreeSet};

use folia_calendar::day::{clock, Day};
use folia_calendar::rowkey::RowKey;
use folia_calendar::select::TownChoice;
use folia_calendar::semester::SemesterKey;
use folia_model::labels::{Campus, Code, Rhythm};
use folia_pages::ask::BookmarksAsk;
use folia_pages::{BookmarksData, StudyplanData};
use folia_plans::studyplan::PlanDoc;
use folia_routes::filter::{CatalogQuery, ProgramScope};
use folia_routes::url::{BookmarkSort, CatalogUrl, PlanView, Season};
use folia_timetable::clash::{self, Overlap, Weeks, When};
use folia_timetable::exams::{self, ExamWarning, Termin, TerminAt, WarningKind};
use folia_timetable::model::{Attendance, Basis, Event, Row, Timetable};
use folia_timetable::views::short_title;
use leptos::prelude::*;

use crate::bookmarks::Bookmarks;
use folia_design::format;
use crate::i18n::{self, Locale};
use crate::myprogram::MineResolved;
use folia_design::nav;
use crate::pages::catalog::finder_on;
use folia_design::ui::Icon;
use super::week::{has_ab, AllSwitch, WeekSwitch};
use super::{key_of, PlanCtx, SheetToggle};

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

/// „Mo" … „So", "Mon" … "Sun" by `Day::weekday` (1 = Monday); nothing for what is no weekday.
pub(super) fn weekday_name(weekday: u8, locale: Locale) -> &'static str {
    locale.texts().weekday_short(i64::from(weekday)).unwrap_or_default()
}

/// „Mo 08.02.2027", "Mon 8 Feb 2027".
fn day_name(day: Day, locale: Locale) -> String {
    format!("{} {}", weekday_name(day.weekday(), locale), day.date(locale))
}

/// Where „+ Modul" leads: the catalog's modules that fit the semester (`fits=`, comparing what the
/// finder compared the last time it was on, `finder_on`), within „Mein Studiengang" where the
/// stored program is in the snapshot (the one kind of address its slug may stand in, A.10).
/// Tracked. A path of the app: a link writes it as `t.path(…)`.
pub(super) fn add_module_href(resolved: Option<MineResolved>, key: SemesterKey) -> String {
    let program = resolved
        .and_then(|resolved| resolved.0.get())
        .filter(|info| info.exact)
        .map(|info| ProgramScope { program_slug: info.program.slug, ..Default::default() });
    let query = CatalogQuery { program, fits: Some(finder_on(key)), ..Default::default() };
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

/// The placeholders standing in a semester that nothing counts for yet, in any semester of their
/// row (a row over several Fachsemester stands in each it was taken over into): the dashed rows
/// of the list of modules. Their number.
fn open_placeholders(doc: &PlanDoc, key: SemesterKey) -> usize {
    doc.placeholders_in(key).into_iter().filter(|p| doc.same_row(p).iter().all(|q| doc.fillers(q.pid).is_empty())).count()
}

/// The line under the head: the program, then the numbers („Informatik B.Sc. · 5 Module · 1
/// Platzhalter"). Placeholders only where there are any. The credits are the head's of the list of
/// modules (owner, 2026-09-25: „die Summe aus allen geplanten Modulen", oben rechts), so they are
/// not said here again. Empty with nothing to count and no program.
pub(super) fn sum_line(program: Option<&str>, modules: usize, placeholders: usize, t: &i18n::Texts) -> String {
    let mut parts = Vec::new();
    if let Some(program) = program.filter(|program| !program.is_empty()) {
        parts.push(program.to_string());
    }
    if modules > 0 {
        parts.push(format::modules(i64::try_from(modules).unwrap_or(i64::MAX), t.locale));
    }
    if placeholders > 0 {
        parts.push((t.studyplan_head.placeholders)(placeholders));
    }
    parts.join(" · ")
}

/// What the head says under its numbers, if anything. (A semester without a module says so in
/// the middle of its empty week, `NothingPlanned`.)
#[derive(Clone, Debug, PartialEq)]
enum HeadLine {
    /// Before the current semester: its past dates are gone from the data.
    Past,
    /// Not a Termin of the semester is published yet.
    Unpublished,
}

/// Whether the data's semester lies before the snapshot's current one. Read from the data's own
/// `meta`: a memo of the data then needs no `current` beside it, from which the data is derived
/// (R16).
pub(super) fn is_past(data: &StudyplanData) -> bool {
    data.meta.current_semester.as_deref().and_then(SemesterKey::parse).is_some_and(|current| data.key < current)
}

/// Which line applies to a semester with modules: the past one, else one without published dates.
fn head_line(data: &StudyplanData) -> Option<HeadLine> {
    if data.ids.is_empty() {
        return None;
    }
    if is_past(data) {
        return Some(HeadLine::Past);
    }
    data.counts.is_empty().then_some(HeadLine::Unpublished)
}

/// The head: „Stundenplan WiSe 2026/27" (the one semester the page shows, owner's redesign of
/// 2026-09-25), on a phone „Anpassen" at its right end, in „Woche" with modules planned „Plan ·
/// Alle Termine" and the switch of A and B weeks where it applies (on a phone the tabs of the
/// week's carousel), and the program and the numbers under it. Then, where it applies, the line
/// of a past semester, or of one whose dates are not out yet.
#[component]
pub(super) fn SemesterHead(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
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
        let (placeholders, program) = (held.get(), program.get());
        let modules = ctx.data.with(|data| data.as_ref().map_or(0, |data| data.ids.len()));
        sum_line(program.as_deref(), modules, placeholders, t)
    });
    let line = Memo::new(move |_| ctx.data.with(|data| data.as_ref().ok().and_then(head_line)));
    // „A-Woche · B-Woche · A/B" at the head's right end, beside the week it switches, where the
    // plan has Termine of one kind of week (the room above the week is the week's, owner
    // 2026-09-25: the whole week in view). On a phone the week is a carousel of the three with
    // their tabs under it (`WeekCarousel`).
    let weekly = Memo::new(move |_| ctx.url.with(|url| matches!(url.view, PlanView::Week | PlanView::Overview)));
    let ab = Memo::new(move |_| ctx.table.with(|table| table.as_ref().is_some_and(has_ab)));
    let planned = Memo::new(move |_| ctx.wanted.with(|wanted| !wanted.1.is_empty()));

    view! {
        <div class="sp-head">
            <h1>{t.studyplan.title}" "<span>{move || key.get().label(t.locale)}</span></h1>
            <SheetToggle/>
            {move || (weekly.get() && planned.get()).then(|| view! { <AllSwitch all=ctx.all/> })}
            {move || (weekly.get() && ab.get() && !ctx.phone.get()).then(|| view! { <WeekSwitch weeks=ctx.weeks/> })}
        </div>
        {move || {
            let text = sum.get();
            (!text.is_empty()).then(|| view! { <p class="sp-sum">{text}</p> })
        }}
        {move || {
            let label = key.get().label(t.locale);
            match line.get() {
                Some(HeadLine::Past) => view! {
                    <p class="note quiet"><span>{(t.studyplan_head.past)(&label)}</span></p>
                }
                .into_any(),
                Some(HeadLine::Unpublished) => view! {
                    <p class="note quiet"><span>{(t.studyplan_head.unpublished)(&label)}</span></p>
                }
                .into_any(),
                None => ().into_any(),
            }
        }}
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

/// How the note of the marked modules taken over at once begins (`PlanCtx::undo`, which the
/// import, „Plan leeren" and the list's × share): a mark of its own, the same in every language;
/// only what follows it is shown.
pub(super) const FROM_MARKED: &str = "Merkliste: ";

/// The marked modules the semester shown could still take (`offered`), for the page's two ways to
/// plan them: the empty week's „übernehmen" and the list's „Aus der Merkliste". One memo for both.
pub(super) fn marked_offered(ctx: PlanCtx) -> Memo<Vec<(String, String)>> {
    let bookmarks = Bookmarks::expect();
    let marked = Memo::new(move |_| bookmarks.map(|bookmarks| bookmarks.marks().into_iter().map(|mark| mark.id).collect::<Vec<_>>()).unwrap_or_default());
    let loaded = Memo::new(move |before| {
        let ids = marked.get();
        if ids.is_empty() {
            return None;
        }
        let now = ctx.source.with_value(|source| source.as_ref().map(|source| source.now(&BookmarksAsk { ids: ids.clone(), sort: BookmarkSort::Added, descending: false })))?;
        folia_pages::ask::unless_pending(now, before, Result::ok)
    });
    // The semester and what is planned into it, from the semester's data (its ids are the plan's).
    Memo::new(move |_| {
        ctx.data.with(|data| match data {
            Ok(data) => loaded.with(|marked| marked.as_ref().map(|marked| offered(marked, data.key, &data.ids)).unwrap_or_default()),
            Err(_) => Vec::new(),
        })
    })
}

/// „Aus der Merkliste (n)": the marked modules this semester could still take (`list`,
/// `marked_offered`), closed until asked, each with „Einplanen"; and after the empty week's
/// „übernehmen", its note and „Rückgängig". A past semester has it too, as it has „+ Modul": a plan
/// taken over from a later Fachsemester fills its first semesters with what was taken.
#[component]
pub(super) fn FromBookmarks(ctx: PlanCtx, list: Memo<Vec<(String, String)>>) -> impl IntoView {
    let t = i18n::t();
    let any = Memo::new(move |_| list.with(|list| !list.is_empty()));
    let note = Memo::new(move |_| ctx.undo.with(|undo| undo.as_ref().and_then(|(note, _)| note.strip_prefix(FROM_MARKED).map(str::to_string))));
    let restoring = RwSignal::new(false);
    let restore = move |_| {
        if restoring.get_untracked() {
            return;
        }
        let (Some(plan), Some((_, before))) = (ctx.plan, ctx.undo.get_untracked()) else { return };
        restoring.set(true);
        let undo = ctx.undo;
        plan.update_after_paint(move |doc| {
            *doc = before;
            let _ = undo.try_set(None);
            let _ = restoring.try_set(false);
        });
    };
    // The modules whose „Einplanen" was clicked and whose write has not landed yet: the button
    // answers at once, the plan follows after the next frame (R21), and then the module leaves the
    // list. The write takes the module out of this set again, so wherever the list offers it next
    // (another semester, or this one after it was taken out) it can be planned once more.
    let planning = RwSignal::new(BTreeSet::<String>::new());

    let taken = move || {
        note.get().map(|note| {
            view! {
                <p class="action note-action sp-marked-note">
                    <Icon name="check"/>
                    <span>{note}</span>
                    <button class="mini hit" type="button" aria-busy=move || restoring.get().then_some("true") on:click=restore>{t.common.undo}</button>
                </p>
            }
        })
    };
    let open = move || {
        any.get().then(|| {
            view! {
                <details class="sp-marked">
                    <summary>{move || (t.studyplan_head.from_bookmarks)(list.with(Vec::len))}</summary>
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
                                            {move || if busy.get() { t.studyplan_head.planned } else { t.studyplan_head.plan }}
                                        </button>
                                    </li>
                                }
                            }
                        />
                    </ul>
                </details>
            }
        })
    };
    view! { {taken}{open} }
}

// ---------- nothing planned yet ----------

/// „3 gemerkte Module übernehmen", „1 gemerktes Modul übernehmen".
fn take_marked_label(count: usize, t: &i18n::Texts) -> String {
    (t.studyplan_head.take_marked)(count)
}

/// „3 gemerkte Module übernommen", after `FROM_MARKED`.
fn marked_note(count: usize, t: &i18n::Texts) -> String {
    format!("{FROM_MARKED}{}", (t.studyplan_head.marked_taken)(count))
}

/// A semester without a module (owner, 2026-09-25): „Noch keine Termine" in the middle of its empty
/// week, and under it the way to the catalog's modules that fit the week („Zum Katalog", as
/// „Modul hinzufügen" of the list leads); where the Merkliste holds modules the semester could take
/// (`marked`), „3 gemerkte Module übernehmen" first, which plans them all at once. The click
/// answers at once and the plan follows after the next frame (R21); the note and its „Rückgängig"
/// stand where the list offers the Merkliste (`FromBookmarks`).
#[component]
pub(super) fn NothingPlanned(ctx: PlanCtx, marked: Memo<Vec<(String, String)>>) -> impl IntoView {
    let t = i18n::t();
    let resolved = MineResolved::expect();
    let count = Memo::new(move |_| marked.with(Vec::len));
    let busy = RwSignal::new(false);
    let take = move |_| {
        let Some(plan) = ctx.plan else { return };
        if busy.get_untracked() {
            return;
        }
        let ids: Vec<String> = marked.with_untracked(|marked| marked.iter().map(|(id, _)| id.clone()).collect());
        if ids.is_empty() {
            return;
        }
        busy.set(true);
        let (key, undo) = (ctx.key.get_untracked(), ctx.undo);
        nav::after_paint(move || {
            let (taken, before) = plan.update(|doc| {
                let before = doc.clone();
                let at = now_secs();
                // A semester the plan holds no more of takes what fits, in the Merkliste's order.
                (ids.iter().filter(|id| doc.plan(key, id, at, None)).count(), before)
            });
            if taken > 0 {
                let _ = undo.try_set(Some((marked_note(taken, t), before)));
            }
            let _ = busy.try_set(false);
        });
    };
    let catalog = move || t.path(&add_module_href(resolved, ctx.key.get()));
    view! {
        <div class="sp-empty">
            <p class="state-title">{t.studyplan_head.no_dates}</p>
            <div class="state-actions">
                {move || match count.get() {
                    0 => view! { <a class="btn primary" href=catalog>{t.studyplan_head.to_catalog}</a> }.into_any(),
                    count => view! {
                        <button class="btn primary" type="button" aria-busy=move || busy.get().then_some("true") on:click=take>{take_marked_label(count, t)}</button>
                        <a class="btn secondary" href=catalog>{t.studyplan_head.to_catalog}</a>
                    }
                    .into_any(),
                }}
            </div>
        </div>
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
    fn title(&self, module: &str, t: &i18n::Texts) -> String {
        self.titles.get(module).cloned().unwrap_or_else(|| (t.studyplan_head.module_numbered)(module))
    }

    /// The title an event goes by in a line: its first planned module's.
    fn title_of(&self, event: &Event, t: &i18n::Texts) -> String {
        event.modules.first().map_or_else(|| event.title.clone(), |module| self.title(module, t))
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

/// What an event is called in a line: its type as QIS writes it (in every language), else its
/// first kind, else „Termin".
pub(super) fn kind_word(event: &Event, locale: Locale) -> String {
    match event.type_raw.as_deref().map(str::trim).filter(|kind| !kind.is_empty()) {
        Some(kind) => kind.to_string(),
        None => event.kinds.iter().next().map_or(folia_plans::i18n::texts(locale).session, |kind| kind.label(locale)).to_string(),
    }
}

/// „0 von 2 Terminen frei: Laborausbildung · Programmierpraktikum": a choice none of whose options
/// is free, in the words of „1 von 4 wählen: …" — the neutral „Terminen", and the type as QIS
/// writes it after the colon, never glued to it („Laborausbildungterminen"). The notes of the
/// semester name the module (`title`), the module beside the plan its event.
pub(super) fn blocked_line(event: &Event, title: &str, locale: Locale) -> String {
    (i18n::texts(locale).studyplan_head.blocked)(event.visible_options().len(), &kind_word(event, locale), title.trim())
}

/// The first row of the first option still shown of a choice: where its line points.
fn first_option_row(event: &Event) -> Option<RowKey> {
    let option = event.visible_options().into_iter().next()?;
    event.rows.iter().find(|row| row.option == Some(option) && row.hidden.is_none()).and_then(|row| row.key)
}

/// A campus as a hop between two exams names it: „Zentralcampus", „Sachsendorf", „Senftenberg".
fn campus_name(campus: &Code<Campus>, t: &i18n::Texts) -> String {
    match campus.known() {
        Some(Campus::Zentralcampus) => t.studyplan_head.central_campus.to_string(),
        Some(Campus::Sachsendorf) => "Sachsendorf".to_string(),
        Some(Campus::Senftenberg) => "Senftenberg".to_string(),
        _ => campus.label(t.locale).to_string(),
    }
}

// ---------- exams that collide ----------

/// What avoids a soft exam warning, and whose Termin that is: a Termin on the `avoid` day of one of
/// the two modules, free of the other module's Termin in the warning — „Mathematik IT-1:
/// Zweittermin 11.03. passt", „…: Erstermin 25.02. passt" where it is that module's earliest. Else
/// („andere Termine passen", no module) only a change of both avoids it. (`ExamWarning::avoid` is
/// only a day, so the Termine on it are looked up.)
fn avoid_text(warning: &ExamWarning, avoid: Day, termine: &[(String, Vec<TerminAt>)], about: &About, t: &i18n::Texts) -> (String, Option<String>) {
    let list = |module: &str| termine.iter().find(|(id, _)| id == module).map_or(&[][..], |(_, list)| list.as_slice());
    let issue = |termin: &Termin| list(&termin.module_id).iter().find(|at| at.day == warning.day && at.termin == *termin);
    let both = || (t.plans_data.other_dates_fit.to_string(), None);
    let (Some(a), Some(b)) = (issue(&warning.a), issue(&warning.b)) else {
        return both();
    };
    for (mine, other) in [(a, b), (b, a)] {
        let module = &mine.termin.module_id;
        if let Some(index) = list(module).iter().position(|at| at.day == avoid && at != mine && exams::collision(at, other).is_none()) {
            let day = avoid.day_month(t.locale);
            let fits = if index == 0 { (t.plans_data.first_sitting_fits)(&day) } else { (t.plans_data.second_sitting_fits)(&day) };
            return (format!("{}: {fits}", about.title(module, t)), Some(module.clone()));
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
pub(super) fn exam_alerts(table: &Timetable, about: &About, t: &i18n::Texts) -> Vec<Alert> {
    let termine = exams::termine(&table.exams, &table.modules);
    table.exam_warnings.iter().map(|warning| exam_alert(warning, about, &termine, t)).collect()
}

/// „Prüfung A und Prüfung B überschneiden sich" / „Mo 08.02.2027, beide 11:00–13:00";
/// „Prüfung A und Prüfung B: nur 0 min zwischen Zentralcampus und Senftenberg" / „Mo 08.02.2027,
/// 08:00–10:00 und 10:00–12:00".
fn exam_alert(warning: &ExamWarning, about: &About, termine: &[(String, Vec<TerminAt>)], t: &i18n::Texts) -> Alert {
    let h = &t.studyplan_head;
    let (a, b) = (about.title(&warning.a.module_id, t), about.title(&warning.b.module_id, t));
    let span = |termin: &Termin| format!("{}–{}", clock(termin.from), clock(termin.to));
    let times = if (warning.a.from, warning.a.to) == (warning.b.from, warning.b.to) {
        (h.both_at)(&span(&warning.a))
    } else {
        (h.each_at)(&span(&warning.a), &span(&warning.b))
    };
    let text = match &warning.kind {
        WarningKind::Overlap => (h.exams_clash)(&a, &b),
        WarningKind::Tight { gap, from, to } => (h.exams_tight)(&a, &b, *gap, &campus_name(from, t), &campus_name(to, t)),
    };
    let mut when = format!("{}, {times}", day_name(warning.day, t.locale));
    let mut module = warning.b.module_id.clone();
    if let Some(avoid) = warning.avoid.filter(|_| !warning.hard) {
        let (avoids, whose) = avoid_text(warning, avoid, termine, about, t);
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
    let t = i18n::t();
    let about = Memo::new(move |_| ctx.data.with(|data| data.as_ref().map(About::of).unwrap_or_default()));
    let alerts = Memo::new(move |_| ctx.table.with(|table| table.as_ref().map(|table| about.with(|about| exam_alerts(table, about, t))).unwrap_or_default()));
    let any = Memo::new(move |_| alerts.with(|alerts| !alerts.is_empty()));
    let alert = move |alert: Alert| {
        let module = alert.module.clone();
        let href = move || t.path(&ctx.url.with(|url| url.with_open(Some(&module), None).path()));
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
fn named(event: &Event, about: &About, t: &i18n::Texts) -> String {
    let (kind, title) = (kind_word(event, t.locale), short_title(&about.title_of(event, t)));
    if title.starts_with(&kind) {
        return title;
    }
    format!("{kind} {title}")
}

/// „Di 07:30–09:00 · Vorlesung Programmierpraktikum × Vorlesung Elektrische … · A-Woche", „Do
/// 12.11. 13:45–15:15 · …" for a single day. It opens the later side at its Termin.
fn overlap_line(table: &Timetable, about: &About, overlap: &Overlap, t: &i18n::Texts) -> Option<Note> {
    let (a, b) = (table.events.get(overlap.a.0)?, table.events.get(overlap.b.0)?);
    let (when, weeks) = match overlap.when {
        When::Weekly(weeks) => (weekday_name(overlap.weekday, t.locale).to_string(), weeks),
        When::Day(day) => (format!("{} {}", weekday_name(day.weekday(), t.locale), day.day_month(t.locale)), Weeks::All),
    };
    let weeks = match weeks {
        Weeks::A => format!(" · {}", t.studyplan_head.week_a),
        Weeks::B => format!(" · {}", t.studyplan_head.week_b),
        Weeks::All => String::new(),
    };
    let text = format!("{when} {}–{} · {} × {}{weeks}", clock(overlap.from), clock(overlap.to), named(a, about, t), named(b, about, t));
    let row = b.rows.get(overlap.b.1).and_then(|row| row.key);
    Some(Note::warn(text, b.modules.first().cloned(), row))
}

/// The line of overlaps and open choices of a timetable. Overlaps count per week (`clash::
/// per_week`: a weekly overlap once, in the week `shown`, for „A/B" the larger of the A and the B
/// week) and on single days apart; opened, the line lists the overlaps of that week and those of
/// single days, then the choices without a free option, the choices still open, and the Standort
/// to pick (`choice`: the one the visitor chose). Empty parts: no line.
pub(super) fn summary(table: &Timetable, about: &About, overlaps: &[Overlap], shown: Weeks, choice: TownChoice, t: &i18n::Texts) -> Summary {
    let h = &t.studyplan_head;
    let mut parts = Vec::new();
    let (weekly, days) = (clash::per_week(overlaps, shown), clash::on_days(overlaps));
    if weekly > 0 {
        parts.push((true, (h.clashes_per_week)(weekly)));
    }
    if days > 0 {
        let text = match weekly {
            0 => (h.clashes_on_days)(days),
            _ => (h.more_on_days)(days),
        };
        parts.push((true, text));
    }
    let shows = |overlap: &&Overlap| match overlap.when {
        When::Weekly(weeks) => weeks.in_week(shown),
        When::Day(_) => true,
    };
    let mut lines: Vec<Note> = overlaps.iter().filter(shows).filter_map(|overlap| overlap_line(table, about, overlap, t)).collect();

    let mut open = 0;
    for event in table.blocked.iter().filter_map(|index| table.events.get(*index)) {
        open += 1;
        lines.push(Note::warn(blocked_line(event, &about.title_of(event, t), t.locale), event.modules.first().cloned(), first_option_row(event)));
    }
    for (index, event) in table.events.iter().enumerate() {
        if event.hidden.is_some() || !event.unresolved() || table.blocked.contains(&index) {
            continue;
        }
        open += 1;
        let text = (h.choose_one)(event.visible_options().len(), &kind_word(event, t.locale), &about.title_of(event, t));
        lines.push(Note::quiet(text, event.modules.first().cloned(), first_option_row(event)));
    }
    if open > 0 {
        parts.push((false, (h.choices_open)(open)));
    }
    if !table.tracks.is_empty() && table.town.is_none() && choice == TownChoice::Derive {
        let names: Vec<String> = table.tracks.iter().map(|module| about.title(module, t)).collect();
        parts.push((false, h.town_open.to_string()));
        let text = (h.choose_town)(&names.join(", "));
        lines.push(Note::quiet(text, table.tracks.iter().next().cloned(), None));
    }
    Summary { parts, lines }
}

/// The one quiet line under the head, closed until asked: „2 Überschneidungen pro Woche · 3
/// Wahlen offen · 2 ausgeblendet"; opened, a compact line each, which opens its module beside the
/// plan, and what is hidden with its „Einblenden" (owner, 2026-09-25: not in the sidebar, whose
/// groups keep their places). In „Woche" it counts the week the A/B switch shows; elsewhere „A/B".
/// No line with nothing to say.
#[component]
pub(super) fn Overlaps(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
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
                .map(|table| about.with(|about| summary(table, about, &clash::overlaps(&table.events), shown, choice, t)))
                .unwrap_or_default()
        })
    });
    // What the semester hides (its events and Termine) and has chosen: siblings of the timetable,
    // both derived from the selection (R16).
    let hides = Memo::new(move |_| ctx.selection.with(|(_, selection)| (selection.hidden_events.clone(), selection.hidden_rows.clone())));
    let hidden = Memo::new(move |_| {
        hides.with(|(events, rows)| about.with(|about| ctx.table.with(|table| table.as_ref().map(|table| hidden_lines(table, events, rows, &about.titles, t)).unwrap_or_default())))
    });
    let parts = Memo::new(move |_| {
        let count = hidden.with(Vec::len);
        built.with(|built| {
            let mut parts = built.parts.clone();
            if count > 0 {
                parts.push((false, (t.studyplan_head.hidden)(count)));
            }
            parts
        })
    });
    let any = Memo::new(move |_| parts.with(|parts| !parts.is_empty()));
    let lines = Memo::new(move |_| built.with(|built| built.lines.clone()));
    let several = Memo::new(move |_| hidden.with(|hidden| hidden.len() > 1));
    let head = move || {
        parts
            .get()
            .into_iter()
            .enumerate()
            .map(|(i, (warn, text))| view! { {(i > 0).then_some(" · ")}<span class:no=warn>{text}</span> })
            .collect_view()
    };
    let unhide = move |what: Unhide| {
        let key = ctx.key.get_untracked();
        if let Some(plan) = ctx.plan {
            plan.update(|doc| match what {
                Unhide::Event(id) => doc.set_event(key, id, false),
                Unhide::Row(row) => doc.set_row(key, row, false),
                Unhide::Choice(id) => doc.choose(key, id, None),
            });
        }
    };
    let show_all = move |_| {
        let key = ctx.key.get_untracked();
        if let Some(plan) = ctx.plan {
            plan.update(|doc| doc.show_all(key));
        }
    };
    move || {
        any.get().then(|| {
            view! {
                <details class="sp-overlaps">
                    <summary>{head}</summary>
                    <ul>
                        <For each=move || lines.get() key=|note| note.clone() children=move |note: Note| overlap_item(ctx, note, t)/>
                        <For
                            each=move || hidden.get()
                            key=|line| line.clone()
                            children=move |line: HiddenLine| {
                                let what = line.unhide;
                                view! {
                                    <li class="hidden"><span>{line.text}</span><button class="mini hit" type="button" on:click=move |_| unhide(what)>{t.studyplan_head.unhide}</button></li>
                                }
                            }
                        />
                        {move || several.get().then(|| view! {
                            <li class="hidden"><button class="mini hit" type="button" on:click=show_all>{t.studyplan_head.unhide_all}</button></li>
                        })}
                    </ul>
                </details>
            }
        })
    }
}

// ---------- what is hidden ----------

/// What „Einblenden" takes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Unhide {
    Event(u32),
    Row(RowKey),
    Choice(u32),
}

/// One line of what is hidden: „Tutorium Mathematik IT-1 · Di 15:30".
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct HiddenLine {
    text: String,
    unhide: Unhide,
}

/// When a Termin meets, in a word: „Di 15:30"; a single date with its day („Di 23.02. 11:45").
fn row_when(row: &Row, locale: Locale) -> String {
    let weekday = row.date.weekday.and_then(|day| u8::try_from(day).ok()).filter(|day| (1..=7).contains(day));
    let single = row.date.rhythm.as_ref().is_some_and(|rhythm| rhythm.is(Rhythm::Single));
    let date = row.date.first_date.as_deref().and_then(Day::parse).map(|day| day.day_month(locale)).filter(|_| single || weekday.is_none());
    let parts: Vec<String> = [weekday.map(|day| weekday_name(day, locale).to_string()), date, row.from.map(clock)].into_iter().flatten().collect();
    parts.join(" ")
}

/// The lines of what is hidden (A.3): each hidden event, each hidden Termin and each made choice
/// of the semester's timetable, exams included, in the timetable's order. `events` and `rows` are
/// what the semester hides; a choice is the timetable's own (`Event::chosen`). Hidden kinds keep
/// their chips in the sidebar. Titles are the modules' (`titles`).
fn hidden_lines(table: &Timetable, events: &BTreeSet<u32>, rows: &BTreeSet<RowKey>, titles: &BTreeMap<String, String>, t: &i18n::Texts) -> Vec<HiddenLine> {
    let title = |modules: &[String], fallback: &str| modules.first().and_then(|module| titles.get(module)).cloned().unwrap_or_else(|| fallback.to_string());
    let mut lines = Vec::new();
    let mut seen = BTreeSet::new();
    for event in &table.events {
        let name = format!("{} {}", kind_word(event, t.locale), title(&event.modules, &event.title));
        let id = event.id.parse::<u32>().ok();
        if let Some(id) = id.filter(|id| events.contains(id)) {
            lines.push(HiddenLine { text: name.clone(), unhide: Unhide::Event(id) });
        }
        for row in &event.rows {
            if let Some(key) = row.key.filter(|key| rows.contains(key) && seen.insert(*key)) {
                lines.push(HiddenLine { text: format!("{name} · {}", row_when(row, t.locale)), unhide: Unhide::Row(key) });
            }
        }
        if let (Some(option), Some(id)) = (event.chosen, id) {
            let when = event.rows.iter().find(|row| row.option == Some(option)).map(|row| row_when(row, t.locale)).unwrap_or_default();
            lines.push(HiddenLine { text: format!("{name} · {}", (t.studyplan_head.only)(&when)), unhide: Unhide::Choice(id) });
        }
    }
    for exam in &table.exams {
        let name = (t.studyplan_head.exam_of)(&title(&exam.modules, &exam.title));
        if let Some(id) = exam.event_id.parse::<u32>().ok().filter(|id| events.contains(id)) {
            lines.push(HiddenLine { text: name.clone(), unhide: Unhide::Event(id) });
        }
        for row in &exam.rows {
            if let Some(key) = row.key.filter(|key| rows.contains(key) && seen.insert(*key)) {
                let day = row.date.first_date.as_deref().and_then(Day::parse).map(|day| day.day_month(t.locale)).unwrap_or_default();
                lines.push(HiddenLine { text: format!("{name} · {day}"), unhide: Unhide::Row(key) });
            }
        }
    }
    lines
}

/// One line of the opened overlaps: a link to its module beside the plan, or plain text where it
/// names none.
fn overlap_item(ctx: PlanCtx, note: Note, t: &'static i18n::Texts) -> impl IntoView {
    let class = if note.warn { "warn" } else { "" };
    match note.module {
        Some(module) => {
            let row = note.row;
            let href = move || t.path(&ctx.url.with(|url| url.with_open(Some(&module), row).path()));
            view! { <li><a class=class href=href data-noscroll="">{note.text}</a></li> }.into_any()
        }
        None => view! { <li><span class=class>{note.text}</span></li> }.into_any(),
    }
}

// ---------- what is derived ----------

/// The one line that says what the page derived rather than read (R12), with only the parts that
/// apply: „Abgeleitet: Vorlesungszeit 05.10.2026–31.01.2027, vorlesungsfrei 21.12.–03.01.,
/// Übungsgruppen, Standort Cottbus."
pub(super) fn derived_line(table: &Timetable, t: &i18n::Texts) -> Option<String> {
    let (h, locale) = (&t.studyplan_head, t.locale);
    let mut parts = Vec::new();
    if let Some((first, last)) = table.facts.lecture {
        parts.push((h.lecture_period)(&first.date(locale), &last.date(locale)));
    }
    if !table.facts.breaks.is_empty() {
        let breaks: Vec<String> = table.facts.breaks.iter().map(|(first, last)| format!("{}–{}", first.day_month(locale), last.day_month(locale))).collect();
        parts.push((h.lecture_break)(&breaks.join(", ")));
    }
    // Groups QIS names are read; parallel slots of which the SWS need one are derived.
    let groups = table
        .events
        .iter()
        .any(|event| event.hidden.is_none() && matches!(event.attendance, Attendance::OneOf { basis: Basis::Sws { .. }, .. }));
    if groups {
        parts.push(h.groups_derived.to_string());
    }
    // Every plan derives a town from its modules, but the town decides something only where a
    // module is taught in both towns.
    if let Some(town) = table.town.filter(|_| table.town_derived && !table.tracks.is_empty()) {
        parts.push((h.town_derived)(town.label()));
    }
    (!parts.is_empty()).then(|| (h.derived)(&parts.join(", ")))
}

/// „Abgeleitet: …": one small muted line under the calendar.
#[component]
pub(super) fn DerivedLine(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let line = Memo::new(move |_| ctx.table.with(|table| table.as_ref().and_then(|table| derived_line(table, t))));
    move || line.get().map(|text| view! { <p class="hint sp-derived">{text}</p> })
}

#[cfg(test)]
mod tests {
    use folia_calendar::day::minutes;
    use folia_calendar::kind::EventKind;
    use folia_calendar::kind::{kinds_of, Class};
    use folia_calendar::select::Town;
    use folia_model::rows_detail::EventDate;
    use folia_plans::studyplan::Placeholder;
    use folia_timetable::clash::Clash;
    use folia_timetable::facts::SemesterFacts;
    use folia_timetable::model::Row;
    use folia_timetable::occur::Occurrences;

    use crate::i18n::{DE, EN};
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
        let summary = summary(&table, &about, &[a_week_overlap()], Weeks::All, TownChoice::Derive, &DE);
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
        let alone = summary_of(&table, &about, &[day.clone(), day.clone()], Weeks::All);
        assert_eq!(alone.parts[0], (true, "2 Überschneidungen an einzelnen Tagen".to_string()));
        // In English, the weekdays and dates too; QIS's types and the titles stay as they are.
        let english = super::summary(&table, &about, &[a_week_overlap(), day], Weeks::All, TownChoice::Derive, &EN);
        assert_eq!(
            texts(&english),
            (
                vec![(true, "1 clash per week"), (true, "1 on single days"), (false, "2 choices open")],
                vec![
                    (true, "Tue 07:30–09:00 · Vorlesung Elektrische und elektronische … × Vorlesung Programmierpraktikum · week A"),
                    (true, "Tue 3 Nov 07:30–09:00 · Vorlesung Elektrische und elektronische … × Vorlesung Programmierpraktikum"),
                    (true, "0 of 4 sessions free: Übung · Entwicklung von Softwaresystemen"),
                    (false, "Choose 1 of 3: Übung · Mathematik IT-1"),
                ]
            )
        );
        // Nothing to say: no line.
        let mut calm = table.clone();
        calm.blocked.clear();
        calm.events.retain(|event| !event.unresolved());
        assert!(summary_of(&calm, &about, &[], Weeks::All).parts.is_empty());
    }

    fn summary_of(table: &Timetable, about: &About, overlaps: &[Overlap], shown: Weeks) -> Summary {
        summary(table, about, overlaps, shown, TownChoice::Derive, &DE)
    }

    #[test]
    fn exams_that_collide_are_red_alerts() {
        let (table, about) = first_semester();
        let alerts = exam_alerts(&table, &about, &DE);
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
        let alerts = exam_alerts(&table, &about, &EN);
        assert_eq!(
            alerts.iter().map(|alert| (alert.text.as_str(), alert.when.as_str())).collect::<Vec<_>>(),
            vec![
                ("The exams of Mathematik IT-1 and Entwicklung von Softwaresystemen clash", "Mon 8 Feb 2027, both 11:00–13:00"),
                (
                    "The exams of Elektrische und elektronische Grundlagen der Informatik and Programmierpraktikum: only 0 min between Central Campus and Senftenberg",
                    "Mon 8 Feb 2027, 08:00–10:00 and 10:00–12:00 · other dates fit"
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
        let alert = exam_alert(&warning("2027-02-25"), &about, &termine, &DE);
        assert_eq!(alert.text, "Prüfung Algorithmische Graphentheorie und Prüfung Mathematik IT-1 überschneiden sich");
        assert_eq!((alert.when.as_str(), alert.module.as_str()), (&*format!("{lead}Algorithmische Graphentheorie: Erstermin 25.02. passt"), "11405"));
        // The later module's Zweittermin.
        let alert = exam_alert(&warning("2027-03-24"), &about, &termine, &DE);
        assert_eq!((alert.when.as_str(), alert.module.as_str()), (&*format!("{lead}Mathematik IT-1: Zweittermin 24.03. passt"), "11112"));
        let alert = exam_alert(&warning("2027-03-24"), &about, &termine, &EN);
        assert_eq!(alert.when, "Wed 10 Mar 2027, 10:00–12:00 and 11:00–13:00 · Mathematik IT-1: second sitting on 24 Mar fits");
        // Only a change of both: the later module.
        let alert = exam_alert(&warning("2027-03-31"), &about, &termine, &DE);
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
            let summary = summary(table, &about, &[], Weeks::All, choice, &DE);
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
    fn what_is_hidden_is_named_as_the_plan_names_it() {
        let dated = |row: Row, first: &str| Row { date: EventDate { first_date: Some(first.into()), last_date: Some(first.into()), ..row.date }, ..row };
        let choice = Attendance::OneOf { options: vec![vec![0], vec![1]], basis: Basis::Groups };
        let (table, about) = first_semester();
        let table = Timetable {
            events: vec![
                event("150132", "Tutorium", "11112", vec![dated(row("150132", 7, 2, ("15:30", "17:00"), "weekly", None), "2026-10-13")], Attendance::All),
                Event {
                    chosen: Some(0),
                    ..event(
                        "148369",
                        "Übung",
                        "12104",
                        vec![dated(row("148369", 1, 1, ("15:30", "17:00"), "weekly", Some(0)), "2026-10-12"), dated(row("148369", 2, 2, ("11:30", "13:00"), "weekly", Some(1)), "2026-10-13")],
                        choice,
                    )
                },
                event("148019", "Übung", "12104", vec![dated(row("148019", 3, 2, ("11:45", "13:15"), "single", None), "2027-02-23")], Attendance::All),
            ],
            ..table
        };
        let events: BTreeSet<u32> = [150132].into_iter().collect();
        let rows: BTreeSet<RowKey> = [RowKey { event: 150132, fp: 7 }, RowKey { event: 148019, fp: 3 }].into_iter().collect();
        let lines = hidden_lines(&table, &events, &rows, &about.titles, &DE);
        let texts: Vec<(&str, Unhide)> = lines.iter().map(|line| (line.text.as_str(), line.unhide)).collect();
        assert_eq!(
            texts,
            vec![
                ("Tutorium Mathematik IT-1", Unhide::Event(150132)),
                ("Tutorium Mathematik IT-1 · Di 15:30", Unhide::Row(RowKey { event: 150132, fp: 7 })),
                ("Übung Entwicklung von Softwaresystemen · nur Mo 15:30", Unhide::Choice(148369)),
                ("Übung Entwicklung von Softwaresystemen · Di 23.02. 11:45", Unhide::Row(RowKey { event: 148019, fp: 3 })),
            ]
        );
        let english: Vec<String> = hidden_lines(&table, &events, &rows, &about.titles, &EN).into_iter().map(|line| line.text).collect();
        assert_eq!(english[1..], ["Tutorium Mathematik IT-1 · Tue 15:30", "Übung Entwicklung von Softwaresystemen · only Mon 15:30", "Übung Entwicklung von Softwaresystemen · Tue 23 Feb 11:45"]);
        // Nothing hidden, nothing chosen: no line.
        let calm = Timetable { events: table.events.iter().cloned().map(|event| Event { chosen: None, ..event }).collect(), ..table };
        assert!(hidden_lines(&calm, &BTreeSet::new(), &BTreeSet::new(), &about.titles, &DE).is_empty());
    }

    #[test]
    fn the_derived_line_names_only_what_applies() {
        let (mut table, _) = first_semester();
        // The town is derived in every plan, but decides something only with a module of both towns.
        assert_eq!(derived_line(&table, &DE).as_deref(), Some("Abgeleitet: Vorlesungszeit 05.10.2026–31.01.2027, vorlesungsfrei 21.12.–03.01., Übungsgruppen."));
        table.tracks = ["12104".to_string()].into_iter().collect();
        assert_eq!(
            derived_line(&table, &DE).as_deref(),
            Some("Abgeleitet: Vorlesungszeit 05.10.2026–31.01.2027, vorlesungsfrei 21.12.–03.01., Übungsgruppen, Standort Cottbus.")
        );
        assert_eq!(
            derived_line(&table, &EN).as_deref(),
            Some("Derived: lecture period 5 Oct 2026–31 Jan 2027, lecture break 21 Dec–3 Jan, exercise groups, location Cottbus.")
        );
        // Groups QIS names are not derived.
        for event in &mut table.events {
            if let Attendance::OneOf { basis, .. } = &mut event.attendance {
                *basis = Basis::Groups;
            }
        }
        table.facts.breaks.clear();
        table.facts.lecture = None;
        assert_eq!(derived_line(&table, &DE).as_deref(), Some("Abgeleitet: Standort Cottbus."));
        table.tracks.clear();
        assert_eq!(derived_line(&table, &DE), None);
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
        assert_eq!(sum_line(informatik, 5, 0, &DE), "Informatik B.Sc. · 5 Module");
        assert_eq!(sum_line(informatik, 4, 1, &DE), "Informatik B.Sc. · 4 Module · 1 Platzhalter");
        assert_eq!(sum_line(None, 1, 0, &DE), "1 Modul");
        assert_eq!((sum_line(informatik, 4, 1, &EN), sum_line(None, 1, 2, &EN)), ("Informatik B.Sc. · 4 modules · 1 placeholder".to_string(), "1 module · 2 placeholders".to_string()));
        // Nothing planned: the program alone, or nothing.
        assert_eq!((sum_line(informatik, 0, 0, &DE), sum_line(None, 0, 0, &DE)), ("Informatik B.Sc.".to_string(), String::new()));
        // A placeholder counts in its own semester, a row over several in each it stands in, and
        // only while nothing counts for its row.
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
        assert_eq!(open_placeholders(&doc, key("2026W")), 3);
        assert_eq!(open_placeholders(&doc, key("2027S")), 0);
        // Row 2 in the summer as well: open there too, until a module counts for it in either.
        doc.placeholders.push(Placeholder { pid: 5, semester: key("2027S"), ..placeholder(2, (1, 6), "60") });
        assert_eq!(open_placeholders(&doc, key("2027S")), 1);
        assert!(doc.plan(key("2026W"), "12107", 1, Some(2)));
        assert_eq!((open_placeholders(&doc, key("2026W")), open_placeholders(&doc, key("2027S"))), (2, 0));
    }

    #[test]
    fn the_merkliste_offers_what_the_semester_could_take() {
        let row = |id: &str| folia_model::rows::CatalogRow {
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
        // The empty week takes them over at once, and says how many it took.
        assert_eq!((take_marked_label(1, &DE), take_marked_label(3, &DE)), ("1 gemerktes Modul übernehmen".to_string(), "3 gemerkte Module übernehmen".to_string()));
        assert_eq!((marked_note(1, &DE), marked_note(3, &DE)), ("Merkliste: 1 gemerktes Modul übernommen".to_string(), "Merkliste: 3 gemerkte Module übernommen".to_string()));
        assert_eq!((take_marked_label(1, &EN), marked_note(3, &EN)), ("Add 1 saved module".to_string(), "Merkliste: 3 saved modules added".to_string()));
    }
}
