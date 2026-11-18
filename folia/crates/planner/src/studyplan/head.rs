//! The top of the Stundenplan: its head („Stundenplan WiSe 2026/27" and the numbers under it), the
//! exams that collide as red alerts, the overlaps per week (a figure that unfolds a line each), the
//! marked modules a calendar of one's own can take in (for „+ Modul", `add.rs`), what the empty
//! week says in its middle, and the one muted line under the calendar that says what the page
//! derived rather than read (R12). Owner's redesign of 2026-09-25: no stack of cards, numbers
//! first. The planned modules are a list of their own beside the week (`modules.rs`).
//!
//! Every part reads the memos of `PlanCtx` it needs. Where a part needs two things, it takes them
//! from sibling memos (`key` and `data`, `table` and a memo of `selection`), never a memo together
//! with the memo it is derived from (R16). Texts are the plan's own terms, numbers first (design
//! A.3, A.5).

use std::collections::BTreeMap;

use folia_calendar::day::{clock, Day};
use folia_calendar::rowkey::RowKey;
use folia_model::labels::Labelled;
use folia_calendar::semester::SemesterKey;
use folia_model::labels::{Campus, Code};
use folia_model::labels::OfferStatus;
use folia_pages::ask::BookmarksAsk;
use folia_pages::{BookmarksData, StudyplanData};
use folia_routes::filter::{CatalogQuery, ProgramScope};
use folia_routes::url::{BookmarkSort, CatalogUrl, PlanView, Season};
use folia_timetable::clash::{self, Overlap, Weeks, When};
use folia_timetable::exams::{self, ExamWarning, Termin, TerminAt, WarningKind};
use folia_timetable::model::{Across, Attendance, Basis, Event, ShownBy, TakenBy, Timetable};
use folia_timetable::views::short_title;
use leptos::prelude::*;

use folia_stores::bookmarks::Bookmarks;
use folia_design::format;
use crate::i18n::{self, Locale};
use folia_stores::myprogram::MineResolved;
use folia_widgets::finder::finder_on;
use folia_design::ui::Icon;
use super::calendars::TogetherSwitch;
use super::week::{has_ab, WeekSwitch};
use super::edit::{said, EditToggle, StatusRow};
use super::PlanCtx;

/// The tones of the plan's modules, in the order of `app.css`'s `t-…` classes: the first planned
/// module of a semester is ice, the ninth ice again (the timetable's `Event::tone`, 1–8).
const HUES: [&str; 8] = ["t-ice", "t-sun", "t-violet", "t-teal", "t-green", "t-coral", "t-rose", "t-slate"];

/// The class of a module's tone (`Event::tone`, 1–8); a tone out of range is the first.
pub(super) fn hue(tone: u8) -> &'static str {
    match tone {
        // A Termin of one's own of no module in the week (`Event::own`): its own grey.
        0 => "t-own",
        tone => HUES.get(usize::from(tone - 1) % HUES.len()).copied().unwrap_or("t-ice"),
    }
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

/// Where the empty week's „Zum Katalog" leads: the catalog's modules that fit the semester
/// (`fits=`, comparing what the finder compared the last time it was on, `finder_on`), within „Mein
/// Studiengang" where the stored program is in the snapshot (the one kind of address its slug may
/// stand in, A.10). Tracked. A path of the app: a link writes it as `t.path(…)`.
pub(super) fn add_module_href(resolved: Option<MineResolved>, key: SemesterKey) -> String {
    let program = resolved
        .and_then(|resolved| resolved.0.get())
        .filter(|info| info.exact)
        .map(|info| ProgramScope { program_slug: info.program.slug, ..Default::default() });
    let query = CatalogQuery { program, fits: Some(finder_on(key)), ..Default::default() };
    CatalogUrl { query, ..Default::default() }.path()
}

// ---------- the head ----------

/// The line under the head: the program, then the numbers („Informatik B.Sc. · 5 Module · 1
/// Besuch · 2 eigene Termine"). Visits and Termine of one's own only where there are any; the
/// placeholders are „Mein Studium"'s (the calendar lists the study's modules and manages none of
/// them, owner 2026-11-17). Empty with nothing to count and no program.
pub(super) fn sum_line(program: Option<&str>, modules: usize, visits: usize, own: usize, t: &i18n::Texts) -> String {
    let mut parts = Vec::new();
    if let Some(program) = program.filter(|program| !program.is_empty()) {
        parts.push(program.to_string());
    }
    if modules > 0 {
        parts.push(format::modules(i64::try_from(modules).unwrap_or(i64::MAX), t.locale));
    }
    if visits > 0 {
        parts.push((t.studyplan_head.visits)(visits));
    }
    if own > 0 {
        parts.push((t.studyplan_head.own)(own));
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

/// The sheet a phone's head opens (`PlanCtx::sheet`): the calendars, or „Export"; or „+ Modul" of
/// the calendar of one's own numbered so (`calendars::AddSheet`), opened from the calendars' sheet
/// (`back`: closing it returns there) or from the list beside the week.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PhoneSheet {
    Calendars,
    Export,
    Adding { id: u8, back: bool },
}

/// The head: „Stundenplan WiSe 2026/27" (the one semester the page shows, owner's redesign of
/// 2026-09-25), then the same controls in both states, so that it is one row and wraps the same
/// whether the plan is looked at or edited: „Gemeinsam frei" where another calendar stands in
/// the week, the switch of A and B weeks where it applies, and „Bearbeiten" (`EditToggle`). Under
/// the head the status row (`StatusRow`): the program and the numbers, and what the state says.
/// Then, where it applies, the line of a past semester, or of one whose dates are not out yet.
///
/// On a phone the head is two calm rows of one grid (owner, 2026-11-18: „Dann muss oben noch
/// aufgeräumt werden, also der header ist ja eine absolute Katastrophe"): the title with the
/// semester small beside it and „Bearbeiten" at the right; under it a quiet toolbar, the switch of
/// the weeks as „A · B · A/B" at the left where the plan has Termine of A or B weeks (no word
/// „Woche": its name says it), and at the right „Kalender" and „Export", which open their sheets
/// (`PhoneSheets`). The same two rows in both states. The line of the program and the numbers is
/// the wide status row's alone; under a phone's head one band says what is open and the overlaps
/// (`PhoneBand`). Until then a phone had three rows that shared no line: the title, „Woche [A|B|A/B]
/// … Bearbeiten", and „Informatik B.Sc. · 5 Module".
#[component]
pub(super) fn SemesterHead(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let key = ctx.key;
    let resolved = MineResolved::expect();
    // The plan's program: „Mein Studiengang" for now, as „Informatik B.Sc.".
    let program = Memo::new(move |_| {
        let info = resolved.and_then(|resolved| resolved.0.get())?;
        Some(format!("{} {}", info.program.name, info.program.degree()).trim().to_string())
    });
    let sum = Memo::new(move |_| {
        let program = program.get();
        // The week's modules, of which the selection's visits (a sibling of the data, R16).
        let (visits, own) = ctx.selection.with(|(_, selection)| (selection.visits.len(), selection.own.len()));
        let modules = ctx.data.with(|data| data.as_ref().map_or(0, |data| data.ids.len())).saturating_sub(visits);
        sum_line(program.as_deref(), modules, visits, own, t)
    });
    let line = Memo::new(move |_| ctx.data.with(|data| data.as_ref().ok().and_then(head_line)));
    // „A-Woche · B-Woche · A/B" beside the week it switches, where the plan has Termine of one kind
    // of week (the room above the week is the week's, owner 2026-09-25: the whole week in view).
    let weekly = Memo::new(move |_| ctx.url.with(|url| matches!(url.view, PlanView::Week | PlanView::Overview)));
    let ab = Memo::new(move |_| ctx.table.with(|table| table.as_ref().is_some_and(has_ab)));
    let phone = ctx.phone;
    // The mode said to a screen reader: an element that is always there (a region that comes with
    // its text is not announced).
    let told = said(ctx);

    view! {
        <div class="sp-head" data-commands=t.studyplan.title>
            <h1>{t.studyplan.title}" "<span>{move || key.get().label(t.locale)}</span></h1>
            // „Gemeinsam frei" once another calendar stands in the week (`calendars.rs`; on a phone
            // in the calendars' sheet).
            {move || (weekly.get() && !phone.get()).then(|| view! { <TogetherSwitch ctx/> })}
            {move || (weekly.get() && ab.get() && !phone.get()).then(|| view! { <WeekSwitch weeks=ctx.weeks/> })}
            <EditToggle ctx/>
            {move || phone.get().then(|| view! { <PhoneTools ctx ab=Signal::derive(move || weekly.get() && ab.get())/> })}
        </div>
        <p class="visually-hidden" role="status">{move || told.get()}</p>
        // On a phone the overlaps' figure stands in the band over the week, beside the line of what
        // is open (`mod.rs`, `PhoneBand`).
        {move || (!phone.get()).then(|| view! { <StatusRow ctx sum/> })}
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

/// The second row of a phone's head, a quiet toolbar: the switch of the weeks („A · B · A/B") at its
/// left where `ab` says the plan has Termine of A or B weeks, and at its right „Kalender" and
/// „Export", each with its sign and word, which open their sheets (`PlanCtx::sheet`). Its place
/// is there in both states and whatever the plan holds, so that the week does not move.
#[component]
fn PhoneTools(ctx: PlanCtx, ab: Signal<bool>) -> impl IntoView {
    let t = i18n::t();
    let h = &t.studyplan_head;
    let open = move |sheet: PhoneSheet| move |_| ctx.sheet.set(Some(sheet));
    let expanded = move |sheet: PhoneSheet| move || if ctx.sheet.get() == Some(sheet) { "true" } else { "false" };
    // Calendars of one's own: their number as a badge at the sign, as the tab bar counts, and in
    // the button's name („Kalender, 1 weiterer"); the button no wider for it.
    let more = Memo::new(move |_| ctx.calendars.with(Vec::len));
    view! {
        {move || ab.get().then(|| view! { <WeekSwitch weeks=ctx.weeks short=true/> })}
        // A group of two buttons with a name of its own (a toolbar would promise arrow keys).
        <div class="sp-tools" role="group" aria-label=h.tools>
            <button class="ghost sp-tool" type="button" data-action="calendars" aria-haspopup="dialog" aria-expanded=expanded(PhoneSheet::Calendars) aria-label=move || (more.get() > 0).then(|| (h.calendars_more)(more.get())) title=h.calendars_title on:click=open(PhoneSheet::Calendars)>
                <span class="sp-tool-ic"><Icon name="layers"/>{move || (more.get() > 0).then(|| view! { <span class="sp-badge" aria-hidden="true">{more.get()}</span> })}</span>
                {t.studyplan_calendars.heading}
            </button>
            <button class="ghost sp-tool" type="button" data-action="export" aria-haspopup="dialog" aria-expanded=expanded(PhoneSheet::Export) title=h.export_title on:click=open(PhoneSheet::Export)>
                <Icon name="download"/>{t.studyplan_export.calendar}
            </button>
        </div>
    }
}

/// The sheets a phone's head opens: the calendars (`calendars::CalendarSheet`), „+ Modul" of a
/// calendar of one's own in its place (`calendars::AddSheet`) and „Export" (`side::ExportSheet`).
/// (They close when the page is no longer a phone's, whose sidebar holds them: `mod.rs`.)
#[component]
pub(super) fn PhoneSheets(ctx: PlanCtx, marked: Memo<Vec<MarkedModule>>) -> impl IntoView {
    view! {
        <super::calendars::CalendarSheet ctx/>
        <super::calendars::AddSheet ctx marked/>
        <super::side::ExportSheet ctx/>
    }
}

/// One band under a phone's head, there while it has something to say: the amber line of what is
/// still open (`OpenLine`) and at its right the overlaps' figure, compact (the sign and the number);
/// the figure alone with its words where nothing is open. What the figure unfolds opens under the
/// band, over the week (`Overlaps`; owner, 2026-10-27: „Wenn man solche Buttons hat, dann ändern
/// die nur den Inhalt unter ihnen"). Until 2026-11-18 the figure had a row of its own between the
/// head and the amber line: two bands, and the week moved by a row with the first overlap.
#[component]
pub(super) fn PhoneBand(ctx: PlanCtx) -> impl IntoView {
    view! {
        <div class="sp-band">
            <super::open::OpenLine ctx/>
            <ClashFigure ctx compact=true/>
        </div>
    }
}

// ---------- the Merkliste ----------

/// A marked module: its number, its title, and why the semester shown does not offer it (`None`: it
/// does).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct MarkedModule {
    pub id: String,
    pub title: String,
    pub unoffered: Option<String>,
}

/// The marked modules, in the order of the Merkliste, each with why `key`'s half of the year does
/// not offer it, where it does not: no longer offered, only in the other half („nur SoSe"), or the
/// turnus the catalog states („unregelmäßig").
fn marked_of(data: &BookmarksData, key: SemesterKey, t: &'static i18n::Texts) -> Vec<MarkedModule> {
    let (season, other) = if key.winter { (Season::Winter, Season::Summer) } else { (Season::Summer, Season::Winter) };
    data.rows
        .iter()
        .map(|row| {
            let unoffered = (!data.offered_in(season, &row.id)).then(|| {
                if row.offer_status.known() == Some(OfferStatus::NotOffered) {
                    OfferStatus::NotOffered.label(t.locale).to_string()
                } else if data.offered_in(other, &row.id) {
                    (t.studyplan_head.only_in)(if key.winter { t.format.summer_short } else { t.format.winter_short })
                } else {
                    format::turnus(row.turnus_season.as_ref(), row.turnus_parity.as_ref(), t.locale)
                }
            });
            MarkedModule { id: row.id.clone(), title: row.title.clone(), unoffered }
        })
        .collect()
}

/// The marked modules for the semester shown (`marked_of`), for „+ Modul"'s „Merkliste" of a
/// calendar of one's own (`add.rs`). The empty week takes none of them over any more (owner,
/// 2026-11-17: „man kann gerade noch module im Kalender verwalten, das ist aber keine gute idee,
/// weil man das ja in mein Studium macht"); until then it offered „3 gemerkte Module übernehmen".
pub(super) fn marked_offered(ctx: PlanCtx) -> Memo<Vec<MarkedModule>> {
    let t = i18n::t();
    let bookmarks = Bookmarks::expect();
    let marked = Memo::new(move |_| bookmarks.map(|bookmarks| bookmarks.marks().into_iter().map(|mark| mark.id).collect::<Vec<_>>()).unwrap_or_default());
    let loaded = Memo::new(move |before| {
        let ids = marked.get();
        if ids.is_empty() {
            return None;
        }
        let now = ctx.source.with_value(|source| source.as_ref().map(|source| source.now(&BookmarksAsk { ids: ids.clone(), sort: BookmarkSort::Added, descending: false, query: Default::default(), also: Vec::new() })))?;
        folia_pages::ask::unless_pending(now, before, Result::ok)
    });
    // The semester, from the semester's data.
    Memo::new(move |_| {
        ctx.data.with(|data| match data {
            Ok(data) => loaded.with(|marked| marked.as_ref().map(|marked| marked_of(marked, data.key, t)).unwrap_or_default()),
            Err(_) => Vec::new(),
        })
    })
}

// ---------- nothing planned yet ----------

/// A semester without anything in its week (owner, 2026-09-25): „Noch keine Termine" in the
/// middle of its empty week, a line on where its Termine come from, and three ways — to „Mein
/// Studium", where the modules of the study are planned, to the catalog's modules that fit the week,
/// and „+ Termin", a Termin of one's own (it fills a gap, so it is there while the plan is looked
/// at). Nothing here plans a module (owner, 2026-11-17: „Im Kalender geht es
/// dann nur noch darum auszuwählen, was jetzt genau gezeigt werden soll. Oder halt termine selbst zu
/// verwalten.").
#[component]
pub(super) fn NothingPlanned(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let m = &t.studyplan_modules;
    let resolved = MineResolved::expect();
    let catalog = move || t.path(&add_module_href(resolved, ctx.key.get()));
    let study = t.path(&folia_routes::url::StudyUrl { plan: true, ..Default::default() }.path());
    view! {
        <div class="sp-empty">
            <p class="state-title">{t.studyplan_head.no_dates}</p>
            <p class="sp-empty-line">{m.empty_week}</p>
            <div class="state-actions">
                // Lent to the calendar's stack on a phone, as the list's way (`data-lend`).
                <a class="btn primary" href=study data-lend="">{m.to_my_study}</a>
                <a class="btn secondary" href=catalog>{t.studyplan_head.to_catalog}</a>
                <button class="btn secondary" type="button" title=t.studyplan_own.add_title on:click=move |_| super::own::ask_today(ctx, 0, None)>
                    <Icon name="plus"/>{t.studyplan_own.add}
                </button>
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

    /// The title an event goes by in a line: its first planned module's; a Termin of one's own's
    /// name.
    fn title_of(&self, event: &Event, t: &i18n::Texts) -> String {
        if let Some(own) = &event.own {
            return super::own::shown_name(own, &self.titles, i18n::texts(t.locale));
        }
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
}

/// What an event is called in a line: what Betula calls its kind wherever it writes it
/// (`views::type_text`: QIS's word in German, its kinds' labels in English), else „Termin".
pub(super) fn kind_word(event: &Event, locale: Locale) -> String {
    Some(folia_timetable::views::type_text(event, locale)).filter(|kind| !kind.is_empty()).unwrap_or_else(|| folia_plans::i18n::texts(locale).session.to_string())
}

/// „0 von 2 Terminen frei: Laborausbildung · Programmierpraktikum": a choice none of whose options
/// is free, in the words of „1 von 4 wählen: …" — the neutral „Terminen", and the type as QIS
/// writes it after the colon, never glued to it („Laborausbildungterminen"). The notes of the
/// semester name the module (`title`), the module beside the plan its event.
pub(super) fn blocked_line(event: &Event, title: &str, locale: Locale) -> String {
    (i18n::texts(locale).studyplan_head.blocked)(event.visible_options().len(), &kind_word(event, locale), title.trim())
}

/// An option of a choice among a module's events in a line: the type of its first event and the
/// weekday and start of each of its Termine („Übung Di 07:30", „Projekt Mo 11:00, Mi 13:30").
pub(super) fn option_text(table: &Timetable, across: &Across, option: usize, locale: Locale) -> Option<String> {
    let events: Vec<&Event> = across.options.get(option)?.iter().filter_map(|event| table.events.get(*event)).collect();
    let kind = kind_word(events.first()?, locale);
    let mut times: Vec<(u8, u16)> = events.iter().flat_map(|event| &event.rows).filter_map(|row| Some((row.slot_day()?, row.from?))).collect();
    times.sort_unstable();
    times.dedup();
    let when: Vec<String> = times.iter().map(|(day, from)| format!("{} {}", weekday_name(*day, locale), clock(*from))).collect();
    Some(match when.is_empty() {
        true => kind,
        false => format!("{kind} {}", when.join(", ")),
    })
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

/// The one quiet line of overlaps (owner, 2026-09-25: „es gibt x Überschneidungen pro Woche",
/// aufklappbar): its parts, and opened, a line each.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Summary {
    /// „2 Überschneidungen pro Woche", „1 an einzelnen Tagen" (`true`: an overlap, drawn red).
    pub parts: Vec<(bool, String)>,
    pub lines: Vec<Note>,
    /// How many overlaps the parts count, for the figure of the status row and the mode's bar.
    pub count: usize,
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

/// The line of overlaps of a timetable. Overlaps count per week (`clash::per_week`: a weekly
/// overlap once, in the week `shown`, for „A/B" the larger of the A and the B week) and on single
/// days apart; opened, the line lists the overlaps of that week and those of single days, and the
/// Standort to pick (`choice`: the one the visitor chose). The choices still open are „Offen"'s
/// (`open.rs`). Empty parts: no line.
pub(super) fn summary(table: &Timetable, about: &About, overlaps: &[Overlap], shown: Weeks, t: &i18n::Texts) -> Summary {
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
    let lines: Vec<Note> = overlaps.iter().filter(shows).filter_map(|overlap| overlap_line(table, about, overlap, t)).collect();
    Summary { parts, lines, count: weekly + days }
}

/// The overlaps of the plan's timetable as `summary` says them. In „Woche" it counts the week the
/// A/B switch shows; elsewhere „A/B".
pub(super) fn overlap_summary(ctx: PlanCtx) -> Memo<Summary> {
    let t = i18n::t();
    let about = Memo::new(move |_| ctx.data.with(|data| data.as_ref().map(About::of).unwrap_or_default()));
    let shown = Memo::new(move |_| {
        let week = ctx.url.with(|url| matches!(url.view, PlanView::Week | PlanView::Overview));
        if week {
            ctx.weeks.get()
        } else {
            Weeks::All
        }
    });
    Memo::new(move |_| {
        let shown = shown.get();
        ctx.table.with(|table| {
            table
                .as_ref()
                .map(|table| about.with(|about| summary(table, about, &clash::overlaps(&table.events), shown, t)))
                .unwrap_or_default()
        })
    })
}

/// The id of the overlaps' list, which the figure unfolds (`aria-controls`).
const OVERLAPS_ID: &str = "sp-overlaps";

/// A line of the unfolded overlaps as the page shows it: its overlap, or one that went since
/// (`gone`), pale in its place.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct KeptNote {
    pub note: Note,
    pub gone: bool,
}

/// The unfolded overlaps' lines while the plan is edited (`keeping`): those that stood there keep
/// their places, a line whose overlap went stays, pale, and a line that comes is added at the end —
/// as „Offen"'s rows stay (`open::keep`). One unfolds the list to see what overlaps and then hides
/// one of the two: the list going with the last overlap took the week up under the pointer by its
/// height, and the next click in the same place opened „Neuer Termin". Else the lines as they are.
pub(super) fn keep_notes(before: &[KeptNote], now: &[Note], keeping: bool) -> Vec<KeptNote> {
    if !keeping {
        return now.iter().map(|note| KeptNote { note: note.clone(), gone: false }).collect();
    }
    let mut kept: Vec<KeptNote> = before.iter().map(|was| KeptNote { note: was.note.clone(), gone: !now.contains(&was.note) }).collect();
    for note in now {
        if !kept.iter().any(|was| was.note == *note) {
            kept.push(KeptNote { note: note.clone(), gone: false });
        }
    }
    kept
}

/// The overlaps' lines as `keep_notes` keeps them: kept while the plan is edited and the list is
/// unfolded. Its closure reads the lines, `edit` and `clashes`: siblings (R16); a reader that reads
/// `edit` or `clashes` too reads them first.
fn kept_notes(ctx: PlanCtx, built: Memo<Summary>) -> Memo<Vec<KeptNote>> {
    Memo::new(move |before: Option<&Vec<KeptNote>>| {
        let keeping = ctx.edit.get() && ctx.clashes.get();
        built.with(|built| keep_notes(before.map_or(&[][..], Vec::as_slice), &built.lines, keeping))
    })
}

/// The overlaps as a figure of the status row and of the mode's bar (on a phone of the line of
/// numbers under the head): „1 Überschneidung", red, a button that unfolds their list under the row
/// (`PlanCtx::clashes`). The row is always there and as tall, so what the plan's content adds or
/// takes moves nothing under it: a line of overlaps over the week went with the Termin hidden that
/// met another, and the week moved up under the pointer. Nothing without an overlap — but while the
/// plan is edited and the list stays unfolded after the last overlap went, „Keine Überschneidung
/// mehr", the button that folds it.
#[component]
pub(super) fn ClashFigure(ctx: PlanCtx, #[prop(optional)] compact: bool) -> impl IntoView {
    let t = i18n::t();
    let built = overlap_summary(ctx);
    let kept = kept_notes(ctx, built);
    let count = Memo::new(move |_| built.with(|built| built.count));
    let said = Memo::new(move |_| built.with(|built| built.parts.iter().map(|(_, text)| text.clone()).collect::<Vec<_>>().join(" · ")));
    move || {
        let open = ctx.clashes.get();
        let staying = open && kept.with(|kept| !kept.is_empty());
        let n = count.get();
        (n > 0 || staying).then(|| {
            let words = if n > 0 { (t.studyplan_head.clashes)(n) } else { t.studyplan_head.no_clash_left.to_string() };
            // Compact (a phone's band, beside the line of what is open): the sign and the number,
            // its name the whole words; the stylesheet shows the words where it stands alone.
            let number = (compact && n > 0).then(|| view! { <span class="n" aria-hidden="true">{n}</span> });
            let name = compact.then(|| words.clone());
            view! {
                <button
                    class="sp-clash-fig"
                    class:none=n == 0
                    class:compact=compact
                    type="button"
                    aria-label=name
                    aria-expanded=move || if ctx.clashes.get() { "true" } else { "false" }
                    aria-controls=OVERLAPS_ID
                    title=move || said.get()
                    on:click=move |_| ctx.clashes.update(|open| *open = !*open)
                >
                    <Icon name="triangle-alert"/>
                    {number}
                    <span class="w">{words}</span>
                    <Icon name="chevron-down" class="chev"/>
                </button>
            }
        })
    }
}

/// The overlaps' list under the status row (on a phone under the line of numbers), once their
/// figure has unfolded it — the student's own click (owner, 2026-09-25: „es gibt x Überschneidungen
/// pro Woche", aufklappbar): what the figure counts, then a compact line each, which opens its
/// module beside the plan. What is hidden is not said here: the mode shows it faint in its place
/// and counts it in its bar (`edit.rs`). While the plan is edited the unfolded list keeps its box
/// (`keep_notes`) until the student folds it or leaves the mode; looking, it goes with the last
/// overlap, and leaving the mode where nothing overlaps folds it.
#[component]
pub(super) fn Overlaps(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let built = overlap_summary(ctx);
    let kept = kept_notes(ctx, built);
    let parts = Memo::new(move |_| built.with(|built| built.parts.clone()));
    let said = Memo::new(move |_| match parts.with(|parts| parts.iter().map(|(_, text)| text.clone()).collect::<Vec<_>>().join(" · ")) {
        said if said.is_empty() => t.studyplan_head.no_clash_left.to_string(),
        said => said,
    });
    let none = Memo::new(move |_| built.with(|built| built.count == 0));
    Effect::new(move |_| {
        if !ctx.edit.get() && none.get() && ctx.clashes.get_untracked() {
            ctx.clashes.set(false);
        }
    });
    let head = move || {
        let parts = parts.get();
        if parts.is_empty() {
            return view! { <span>{t.studyplan_head.no_clash_left}</span> }.into_any();
        }
        parts.into_iter().enumerate().map(|(i, (warn, text))| view! { {(i > 0).then_some(" · ")}<span class:no=warn>{text}</span> }).collect_view().into_any()
    };
    move || {
        let open = ctx.clashes.get();
        let any = kept.with(|kept| !kept.is_empty());
        (open && any).then(|| {
            view! {
                <div class="sp-overlaps open" id=OVERLAPS_ID role="region" aria-label=move || said.get()>
                    <p class="sp-overlaps-head">{head}</p>
                    <ul><For each=move || kept.get() key=|kept| kept.clone() children=move |kept: KeptNote| overlap_item(ctx, kept, t)/></ul>
                </div>
            }
        })
    }
}

/// One line of the opened overlaps: a link to its module beside the plan, or plain text where it
/// names none.
fn overlap_item(ctx: PlanCtx, kept: KeptNote, t: &'static i18n::Texts) -> impl IntoView {
    let KeptNote { note, gone } = kept;
    let class = match (gone, note.warn) {
        (true, _) => "gone",
        (false, true) => "warn",
        (false, false) => "",
    };
    // Gone: pale and struck through in the box it had (a word after it would wrap the line and
    // move the week); the words for a screen reader.
    let after = gone.then(|| view! { <span class="visually-hidden">{format!(" · {}", t.studyplan_head.clash_gone)}</span> });
    match note.module {
        Some(module) => {
            let row = note.row;
            let href = move || t.path(&ctx.url.with(|url| url.with_open(Some(&module), row).path()));
            view! { <li><a class=class href=href data-noscroll="">{note.text}{after}</a></li> }.into_any()
        }
        None => view! { <li><span class=class>{note.text}{after}</span></li> }.into_any(),
    }
}

// ---------- what is derived ----------

/// The one line that says what the page derived rather than read (R12), with only the parts that
/// apply: „Abgeleitet: Vorlesungszeit 05.10.2026–31.01.2027, vorlesungsfrei 21.12.–03.01.,
/// Übungsgruppen, Standort Cottbus."
pub(super) fn derived_line(table: &Timetable, titles: &BTreeMap<String, String>, t: &i18n::Texts) -> Option<String> {
    let (h, locale) = (&t.studyplan_head, t.locale);
    // A module by its title, or by the title of its first event where the catalog has none.
    let title = |module: &String| titles.get(module).cloned().or_else(|| table.events.iter().find(|event| event.modules.first() == Some(module)).map(|event| event.title.clone()));
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
    // module is held in both towns and nothing else decides.
    if let Some(town) = table.town.filter(|_| table.town_derived && table.shown.values().any(|shown| shown.by == ShownBy::Derived)) {
        parts.push((h.town_derived)(town.label()));
    }
    // The courses the program takes (owner, 2026-10-19): „Programmierpraktikum in
    // Cottbus-Sachsendorf laut Studiengang"; and the options of a choice among a module's events
    // it takes: „Höhere Mathematik - T3: Übung Di 07:30".
    let mut courses: Vec<String> = table
        .shown
        .iter()
        .filter(|(_, shown)| shown.by == ShownBy::Program)
        .filter_map(|(module, shown)| {
            let campuses: Vec<&str> = shown.campuses.iter().map(|campus| campus.label(locale)).collect();
            Some((h.course_in)(&title(module)?, &campuses.join(", ")))
        })
        .collect();
    for across in table.choices.iter().filter(|across| across.by == Some(TakenBy::Program)) {
        let Some(title) = title(&across.module) else { continue };
        let options: Vec<String> = across.taken.iter().filter_map(|option| option_text(table, across, *option, locale)).collect();
        if !options.is_empty() {
            courses.push((h.option_of)(&title, &options.join(", ")));
        }
    }
    if !courses.is_empty() {
        parts.push((h.courses_derived)(&courses.join(", ")));
    }
    (!parts.is_empty()).then(|| (h.derived)(&parts.join(", ")))
}

/// „Abgeleitet: …": one small muted line under the calendar.
#[component]
pub(super) fn DerivedLine(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let titles = Memo::new(move |_| ctx.data.with(|data| data.as_ref().map(StudyplanData::titles).unwrap_or_default()));
    let line = Memo::new(move |_| titles.with(|titles| ctx.table.with(|table| table.as_ref().and_then(|table| derived_line(table, titles, t)))));
    move || line.get().map(|text| view! { <p class="hint sp-derived">{text}</p> })
}

#[cfg(test)]
mod tests {
    use folia_calendar::day::minutes;
    use folia_calendar::kind::EventKind;
    use folia_calendar::kind::{kinds_of, Class};
    use folia_calendar::select::Town;
    use folia_model::labels::{Campus, ChoiceKind};
    use folia_model::rows_detail::EventDate;
    use folia_timetable::model::Shown;
    use std::collections::BTreeSet;
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
            series: None,
            track: None,
            choice: None,
            option: None,
            extra: false,
        };
        Row {
            key: Some(RowKey { event: event.parse().unwrap(), fp }),
            ord: Some(1),
            every: folia_timetable::occur::Every::of(&date),
            date,
            cancelled_dates: None,
            occ: Occurrences::default(),
            from: minutes(time.0),
            to: minutes(time.1),
            option,
            hidden: None,
            changed: None,
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
            course: None,
            tone: 1,
            attendance,
            across: None,
            extra: false,
            visit: false,
            chosen: BTreeSet::new(),
            rows,
            hidden: None,
            source_url: None,
            own: None,
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
            visits: BTreeSet::new(),
            events,
            exams: Vec::new(),
            visit_exams: Vec::new(),
            tracks: BTreeSet::new(),
            courses: Default::default(),
            shown: Default::default(),
            town: Some(Town::Cottbus),
            town_derived: true,
            clashes: vec![Clash { a: (0, 0), b: (3, 0), first: day("2026-10-06"), days: 8 }],
            blocked: vec![1],
            blocked_choices: Vec::new(),
            choices: Vec::new(),
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
            changes: Vec::new(),
            notes: Default::default(),
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
    fn the_unfolded_overlaps_keep_their_box_while_the_plan_is_edited() {
        let note = |text: &str| Note { warn: true, text: text.to_string(), module: Some("12104".to_string()), row: None };
        let (a, b) = (note("Di 09:15–10:45 · VL EEG × VL EvS"), note("Do 13:45–15:15 · Ü MIT1 × Prak PP"));
        let fresh = |notes: &[Note]| notes.iter().map(|note| KeptNote { note: note.clone(), gone: false }).collect::<Vec<_>>();
        let before = fresh(&[a.clone(), b.clone()]);
        // Editing with the list unfolded: the line whose overlap went stays, pale, in its place.
        let kept = keep_notes(&before, std::slice::from_ref(&b), true);
        assert_eq!(kept, [KeptNote { note: a.clone(), gone: true }, KeptNote { note: b.clone(), gone: false }]);
        // The last one gone too: the box stays with both lines.
        let kept = keep_notes(&kept, &[], true);
        assert!(kept.len() == 2 && kept.iter().all(|kept| kept.gone));
        // One back takes its place again; a new one comes at the end.
        let c = note("Fr 11:30–13:00 · VL MIT1 × VL PP");
        let kept = keep_notes(&kept, &[a.clone(), c.clone()], true);
        assert_eq!(kept.iter().map(|kept| (kept.note.text.as_str(), kept.gone)).collect::<Vec<_>>(), [(a.text.as_str(), false), (b.text.as_str(), true), (c.text.as_str(), false)]);
        // Folded, or looking: the lines as they are.
        assert!(keep_notes(&kept, &[], false).is_empty());
        assert_eq!(keep_notes(&kept, std::slice::from_ref(&c), false), fresh(std::slice::from_ref(&c)));
    }

    #[test]
    fn one_line_counts_the_overlaps_per_week() {
        let (table, about) = first_semester();
        let summary = summary(&table, &about, &[a_week_overlap()], Weeks::All, &DE);
        // The choices still open are „Offen"'s, not the line's.
        assert_eq!(
            texts(&summary),
            (vec![(true, "1 Überschneidung pro Woche")], vec![(true, "Di 07:30–09:00 · Vorlesung Elektrische und elektronische … × Vorlesung Programmierpraktikum · A-Woche")])
        );
        // An overlap opens the later module at its Termin.
        assert_eq!((summary.lines[0].module.as_deref(), summary.lines[0].row), (Some("12102"), Some(RowKey { event: 149408, fp: 2 })));
        // The B week has no overlap: it counts and lists none.
        let b = summary_of(&table, &about, &[a_week_overlap()], Weeks::B);
        assert!(b.parts.is_empty() && b.lines.is_empty());
        // A single day counts apart, beside a weekly one or alone.
        let day = Overlap { when: When::Day(day("2026-11-03")), ..a_week_overlap() };
        let both = summary_of(&table, &about, &[a_week_overlap(), day.clone()], Weeks::All);
        assert_eq!(both.parts[..2], [(true, "1 Überschneidung pro Woche".to_string()), (true, "1 an einzelnen Tagen".to_string())]);
        assert!(both.lines[1].text.starts_with("Di 03.11. 07:30–09:00 · "));
        let alone = summary_of(&table, &about, &[day.clone(), day.clone()], Weeks::All);
        assert_eq!(alone.parts[0], (true, "2 Überschneidungen an einzelnen Tagen".to_string()));
        // The figure counts what the parts say: per week and on single days, nothing in the B week.
        assert_eq!((summary.count, both.count, alone.count, b.count), (1, 2, 2, 0));
        // In English, the weekdays and dates too, and the kinds by their English labels; the titles
        // stay as they are.
        let english = super::summary(&table, &about, &[a_week_overlap(), day], Weeks::All, &EN);
        assert_eq!(
            texts(&english),
            (
                vec![(true, "1 clash per week"), (true, "1 on single days")],
                vec![
                    (true, "Tue 07:30–09:00 · Lecture Elektrische und elektronische … × Lecture Programmierpraktikum · week A"),
                    (true, "Tue 3 Nov 07:30–09:00 · Lecture Elektrische und elektronische … × Lecture Programmierpraktikum"),
                ]
            )
        );
        // Nothing to say: no line.
        assert!(summary_of(&table, &about, &[], Weeks::All).parts.is_empty());
    }

    fn summary_of(table: &Timetable, about: &About, overlaps: &[Overlap], shown: Weeks) -> Summary {
        summary(table, about, overlaps, shown, &DE)
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
        let blocked = |table: &Timetable| blocked_line(&table.events[1], &about.title_of(&table.events[1], &DE), Locale::De);
        // QIS's „Laborausbildung" stands after the colon as QIS writes it, never glued to the
        // Termine („Laborausbildungterminen").
        retype(&mut table, "Laborausbildung");
        assert_eq!(blocked(&table), "0 von 4 Terminen frei: Laborausbildung · Entwicklung von Softwaresystemen");
        retype(&mut table, "Vorlesung/Übung");
        assert_eq!(blocked(&table), "0 von 4 Terminen frei: Vorlesung/Übung · Entwicklung von Softwaresystemen");
    }

    #[test]
    fn the_derived_line_names_only_what_applies() {
        let (mut table, _) = first_semester();
        let mut titles = BTreeMap::new();
        // The town is derived in every plan, but decides something only with a module of both towns.
        assert_eq!(derived_line(&table, &titles, &DE).as_deref(), Some("Abgeleitet: Vorlesungszeit 05.10.2026–31.01.2027, vorlesungsfrei 21.12.–03.01., Übungsgruppen."));
        table.tracks = ["12104".to_string()].into_iter().collect();
        let derived = Shown { campuses: BTreeSet::new(), towns: BTreeSet::from([Town::Cottbus]), by: ShownBy::Derived };
        table.shown = [("12104".to_string(), derived)].into_iter().collect();
        assert_eq!(
            derived_line(&table, &titles, &DE).as_deref(),
            Some("Abgeleitet: Vorlesungszeit 05.10.2026–31.01.2027, vorlesungsfrei 21.12.–03.01., Übungsgruppen, Standort Cottbus.")
        );
        assert_eq!(
            derived_line(&table, &titles, &EN).as_deref(),
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
        assert_eq!(derived_line(&table, &titles, &DE).as_deref(), Some("Abgeleitet: Standort Cottbus."));
        // The course a program takes is named with its module (owner, 2026-10-19).
        let taken = Shown { campuses: BTreeSet::from([Campus::Sachsendorf]), towns: BTreeSet::from([Town::Cottbus]), by: ShownBy::Program };
        table.shown.insert("12102".to_string(), taken);
        assert_eq!(derived_line(&table, &titles, &DE).as_deref(), Some("Abgeleitet: Standort Cottbus, Veranstaltung 149408 in Cottbus-Sachsendorf laut Studiengang."));
        assert_eq!(derived_line(&table, &titles, &EN).as_deref(), Some("Derived: location Cottbus, Veranstaltung 149408 in Cottbus-Sachsendorf by your program."));
        // By the module's title where the catalog has one, not by its first event's: that can be
        // an extra („Aufbaukurs …").
        titles.insert("12102".to_string(), "Programmierpraktikum".to_string());
        assert_eq!(derived_line(&table, &titles, &DE).as_deref(), Some("Abgeleitet: Standort Cottbus, Programmierpraktikum in Cottbus-Sachsendorf laut Studiengang."));
        // The option of a choice among a module's events the program takes.
        let option = |id: &str, on: &str| {
            let mut held = row(id, 30, i64::from(day(on).weekday()), ("09:15", "10:45"), "weekly", None);
            held.occ.days = vec![day(on)];
            event(id, "Übung", "12102", vec![held], Attendance::All)
        };
        table.events.extend([option("149500", "2026-10-19"), option("149501", "2026-10-21")]);
        let across = Across {
            module: "12102".into(),
            number: 1,
            kind: ChoiceKind::Program,
            pick: 1,
            options: vec![vec![4], vec![5]],
            numbers: vec![1, 2],
            taken: BTreeSet::from([0]),
            by: Some(TakenBy::Program),
            narrowed: None,
        };
        table.choices = vec![across.clone()];
        assert_eq!(
            derived_line(&table, &titles, &DE).as_deref(),
            Some("Abgeleitet: Standort Cottbus, Programmierpraktikum in Cottbus-Sachsendorf, Programmierpraktikum: Übung Mo 09:15 laut Studiengang.")
        );
        // What the visitor chose is not derived.
        table.choices = vec![Across { by: Some(TakenBy::Chosen), ..across }];
        assert_eq!(derived_line(&table, &titles, &DE).as_deref(), Some("Abgeleitet: Standort Cottbus, Programmierpraktikum in Cottbus-Sachsendorf laut Studiengang."));
        table.choices.clear();
        table.tracks.clear();
        table.shown.clear();
        assert_eq!(derived_line(&table, &titles, &DE), None);
    }

    #[test]
    fn each_module_keeps_its_tone() {
        // A position's tone in the legend is the tone the timetable gives the module's events.
        let hues: Vec<&str> = (0..10).map(|position| hue(tone_at(position))).collect();
        assert_eq!(hues, vec!["t-ice", "t-sun", "t-violet", "t-teal", "t-green", "t-coral", "t-rose", "t-slate", "t-ice", "t-sun"]);
        // Tone 0 is a Termin of one's own of no module in the week: its own grey.
        assert_eq!((hue(0), hue(9)), ("t-own", "t-ice"));
    }

    #[test]
    fn the_head_counts_numbers_first() {
        let informatik = Some("Informatik B.Sc.");
        assert_eq!(sum_line(informatik, 5, 0, 0, &DE), "Informatik B.Sc. · 5 Module");
        assert_eq!(sum_line(None, 1, 0, 0, &DE), "1 Modul");
        assert_eq!(sum_line(informatik, 4, 2, 0, &DE), "Informatik B.Sc. · 4 Module · 2 Besuche");
        assert_eq!(sum_line(informatik, 4, 0, 3, &DE), "Informatik B.Sc. · 4 Module · 3 eigene Termine");
        assert_eq!(sum_line(None, 0, 0, 1, &EN), "1 event of your own");
        assert_eq!(sum_line(None, 0, 1, 0, &EN), "1 visit");
        assert_eq!(sum_line(informatik, 4, 0, 0, &EN), "Informatik B.Sc. · 4 modules");
        // Nothing planned: the program alone, or nothing.
        assert_eq!((sum_line(informatik, 0, 0, 0, &DE), sum_line(None, 0, 0, 0, &DE)), ("Informatik B.Sc.".to_string(), String::new()));
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
            kind_source: None,
            kind_basis: None,
            plan_semester: None,
            area: None,
            courses: 0,
            choices: Vec::new(),
            marks: None,
            fues_note: None,
        };
        let data = BookmarksData {
            rows: vec![row("11103"), row("12330"), row("12104")],
            winter: vec!["11103".into(), "12104".into()],
            summer: vec!["12330".into()],
            missing: Vec::new(),
        };
        // Every marked module in the Merkliste's order: what its half of the year offers, the rest
        // with why not.
        let marked = |id: &str, unoffered: Option<&str>| MarkedModule { id: id.into(), title: format!("Modul {id}"), unoffered: unoffered.map(str::to_string) };
        assert_eq!(marked_of(&data, key("2026W"), &DE), vec![marked("11103", None), marked("12330", Some("nur SoSe")), marked("12104", None)]);
        assert_eq!(marked_of(&data, key("2027S"), &DE), vec![marked("11103", Some("nur WiSe")), marked("12330", None), marked("12104", Some("nur WiSe"))]);
    }
}
