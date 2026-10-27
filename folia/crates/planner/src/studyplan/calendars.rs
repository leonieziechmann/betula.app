//! Calendars of one's own beside „Mein Studiengang" (folia/design/kalender/konzepte.html and
//! handy.html; owner, 2026-10-26: „Das sind Kallender, die die nutzer selbst erstellen können, um
//! zum beispiel für ihre freunde einen speraten kallender zu haben", „A mit Fokus und Spur aus C").
//!
//! A calendar of one's own is somebody's plan handed on by a link (`folia_calendar::share`, with
//! the groups ticked since 2026-10-26), kept in the plan's text (`PlanDoc::calendars`) and nowhere
//! else, and counting for nothing of the study (owner: „module, die nicht in Mein studium
//! eingetragen werden zählen nicht zum studium"). The week shows it beside the plan's Termine: in
//! full, outlined in its colour with its initial; as a strip at the edge of each day, so that the
//! plan keeps its width; or not at all (`CalendarView`). A new one stands in full while it holds
//! at most eight Termine a week and as a strip beyond, until its switch says otherwise; „Mein
//! Studiengang" is always there and always in full. Once another calendar stands in the week,
//! „Gemeinsam frei" in the week's head draws the time every one of them has free, from 90 minutes
//! between 8 and 18 o'clock (all decided 2026-10-26).
//!
//! The list stands at the top of the sidebar where the page has one (`CalendarsGroup`), on a phone
//! in a line over the week that opens into the same rows (`CalendarLine`; no chips, owner,
//! 2026-10-26). A new calendar comes from a link pasted into its form (`NewCalendar`) or from the
//! offer of a shared plan (`share.rs`, „Als Kalender"); its menu renames it and deletes it, with
//! „Rückgängig". The module list beside the week has a part per calendar (`CalendarModules`).
//! „+ Modul" in a calendar is the next step (folia/design/kalender: „+ Modul" and „nur besuchen").
//!
//! What the parts show is built as plain values first (`Shown`, `free_time`), which the tests read
//! without rendering.

use std::collections::BTreeMap;

use folia_timetable::clash::Weeks;
use folia_calendar::day::{clock, Day};
use folia_calendar::select::Selection;
use folia_calendar::semester::SemesterKey;
use folia_calendar::share::SharedPlan;
use folia_data::DataClient;
use folia_design::ui::Icon;
use folia_pages::ask::StudyplanAsk;
use folia_plans::studyplan::{Calendar, CalendarView, MAX_CALENDARS};
use folia_routes::url::StudyplanUrl;
use folia_stores::studyplan::Studyplan;
use folia_timetable::model::Timetable;
use folia_widgets::week::{GridBand, GridSlot};
use leptos::prelude::*;

use super::add::{AddPanel, AddTo};
use super::head::{weekday_name, MarkedModule};
use super::{key_of, PlanCtx};
use crate::i18n;

/// The colours of the calendars of one's own (`k-1` … `k-6`, app.css), by their number. A
/// calendar stands apart from the modules by its form (an outline, a strip), not by its hue alone.
const HUES: [&str; 6] = ["k-1", "k-2", "k-3", "k-4", "k-5", "k-6"];

/// How many Termine a week a new calendar holds at most to stand in full (owner, 2026-10-26: „Ein
/// neuer Kalender steht voll, ab mehr als acht Terminen in der Woche als Spur").
const FULL_UP_TO: usize = 8;

/// Where the time free in every calendar is looked for, and the least a stretch of it is (owner,
/// 2026-10-26: „ab 90 Minuten").
const FREE_FROM: u16 = 8 * 60;
const FREE_UNTIL: u16 = 18 * 60;
const FREE_LEAST: u16 = 90;

/// The colour class of the calendar `id`.
pub(super) fn hue_of(id: u8) -> &'static str {
    HUES.get(usize::from(id.saturating_sub(1)) % HUES.len()).copied().unwrap_or("k-1")
}

// ---------- what the page shows ----------

/// What the store says of a calendar for the semester shown.
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    key: SemesterKey,
    id: u8,
    name: String,
    view: Option<CalendarView>,
    program: Option<String>,
    modules: Vec<String>,
    selection: Selection,
    /// The first semester it holds modules in, where it holds none in the one shown.
    elsewhere: Option<SemesterKey>,
}

/// Whether a calendar's Termine are there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    /// No module in the semester shown.
    Empty,
    Loading,
    Failed,
    Ready,
}

/// A calendar of one's own as the page shows it in the semester shown (`PlanCtx::calendars`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Shown {
    pub id: u8,
    /// The semester shown, as the plan works it out (`key_of`).
    pub key: SemesterKey,
    /// Its name; one without a name is named by its number („Kalender 2").
    pub name: String,
    /// Its colour (`hue_of`) and the initial a slot in full carries.
    pub hue: &'static str,
    pub initial: String,
    /// How it stands: as its switch says (`set`), else as a new one stands (`FULL_UP_TO`).
    pub view: CalendarView,
    pub set: bool,
    /// Its modules in the semester shown, and their abbreviations in the same order.
    pub modules: Vec<String>,
    pub names: Vec<String>,
    /// How many groups are ticked in it in the semester shown.
    pub groups: usize,
    /// Its timetable once the catalog answers, what its slots name the modules by, and the
    /// modules' titles.
    pub table: Option<Timetable>,
    pub titles: BTreeMap<String, String>,
    pub full: BTreeMap<String, String>,
    /// Its own ticks of the semester (`Calendar::selection`) and its program: how „+ Modul" holds
    /// a module against it (`add.rs`).
    pub selection: Selection,
    pub program: Option<String>,
    /// Its Termine a week, the groups of a choice still open left out.
    pub termine: usize,
    pub state: State,
    pub elsewhere: Option<SemesterKey>,
}

impl Shown {
    /// It stands in the week.
    pub fn in_week(&self) -> bool {
        self.view != CalendarView::Off && self.table.is_some()
    }
}

/// The calendars of one's own of the semester shown, in their order, each with its timetable as
/// the local catalog answers it. Built where `PlanCtx` is, from its sources: the semester is worked
/// out of the plan as `wanted` works it out (`key_of`), not read from `key`, which is derived from
/// the plan too (R16).
pub(super) fn shown_calendars(url: Memo<StudyplanUrl>, current: Memo<Option<SemesterKey>>, plan: Option<Studyplan>, source: StoredValue<Option<DataClient>>, today: Option<Day>) -> Memo<Vec<Shown>> {
    let t = i18n::t();
    let entries = Memo::new(move |_| {
        let (url, current) = (url.get(), current.get());
        let Some(plan) = plan else { return Vec::new() };
        plan.with(|doc| {
            let key = key_of(&url, current, doc, today);
            doc.calendars.iter().map(|calendar| entry_of(calendar, key)).collect::<Vec<Entry>>()
        })
    });
    Memo::new(move |before: Option<&Vec<Shown>>| entries.with(|entries| entries.iter().map(|entry| shown_of(entry, source, before, t)).collect()))
}

fn entry_of(calendar: &Calendar, key: SemesterKey) -> Entry {
    let modules = calendar.modules_in(key);
    let elsewhere = modules.is_empty().then(|| calendar.modules.iter().map(|(semester, _)| *semester).min()).flatten();
    Entry { key, id: calendar.id, name: calendar.name.clone(), view: calendar.view, program: calendar.program.clone(), modules, selection: calendar.selection(key), elsewhere }
}

/// A calendar as the page shows it, its timetable asked of the local catalog; while the answer is
/// on its way, the one it had before stands.
fn shown_of(entry: &Entry, source: StoredValue<Option<DataClient>>, before: Option<&Vec<Shown>>, t: &'static i18n::Texts) -> Shown {
    let name = if entry.name.is_empty() { (t.studyplan_calendars.unnamed)(entry.id) } else { entry.name.clone() };
    let initial = name.chars().next().map(|first| first.to_uppercase().collect::<String>()).unwrap_or_default();
    let ask = |source: &DataClient| source.now(&StudyplanAsk { key: entry.key, ids: entry.modules.clone(), program: entry.program.clone(), locale: t.locale });
    let answer = (!entry.modules.is_empty()).then(|| source.with_value(|source| source.as_ref().map(ask))).flatten();
    let earlier = before.and_then(|before| before.iter().find(|shown| shown.id == entry.id && shown.modules == entry.modules));
    let (table, titles, full, state) = match answer {
        None => (None, BTreeMap::new(), BTreeMap::new(), State::Empty),
        Some(Ok(data)) => (Some(data.timetable(&entry.selection)), data.slot_names(), data.titles(), State::Ready),
        Some(Err(error)) if error.is_pending() => match earlier {
            Some(earlier) => (earlier.table.clone(), earlier.titles.clone(), earlier.full.clone(), earlier.state),
            None => (None, BTreeMap::new(), BTreeMap::new(), State::Loading),
        },
        Some(Err(_)) => (None, BTreeMap::new(), BTreeMap::new(), State::Failed),
    };
    let termine = table.as_ref().map_or(0, |table| table.regular_week().iter().filter(|item| item.alt.is_none() && !item.extra).count());
    let view = entry.view.unwrap_or(if termine > FULL_UP_TO { CalendarView::Strip } else { CalendarView::Full });
    let names = entry.modules.iter().map(|id| titles.get(id).cloned().unwrap_or_else(|| id.clone())).collect();
    Shown {
        id: entry.id,
        key: entry.key,
        name,
        hue: hue_of(entry.id),
        initial,
        view,
        set: entry.view.is_some(),
        modules: entry.modules.clone(),
        names,
        groups: entry.selection.chosen_rows.len(),
        table,
        titles,
        full,
        selection: entry.selection.clone(),
        program: entry.program.clone(),
        termine,
        state,
        elsewhere: entry.elsewhere,
    }
}

// ---------- the week ----------

/// The Termine of `table` that stand firm in the week `shown`, as day, start and end: no group of
/// a choice still open, no extra, nothing of single dates.
fn firm(table: &Timetable, shown: Weeks) -> Vec<(u8, u16, u16)> {
    table
        .regular_week()
        .iter()
        .filter(|item| item.alt.is_none() && !item.extra && item.weeks.is_some() && item.in_week(shown))
        .map(|item| (item.day, item.from, item.to))
        .collect()
}

/// The stretches of Monday to Friday between 8 and 18 o'clock that none of `busy` touches, of 90
/// minutes and more: as day, start and end.
pub(super) fn free_time(busy: &[(u8, u16, u16)]) -> Vec<(u8, u16, u16)> {
    let mut out = Vec::new();
    for day in 1..=5u8 {
        let mut spans: Vec<(u16, u16)> = busy.iter().filter(|(on, _, _)| *on == day).map(|(_, from, to)| (*from, *to)).collect();
        spans.sort_unstable();
        let mut at = FREE_FROM;
        for (from, to) in spans.into_iter().chain([(FREE_UNTIL, FREE_UNTIL)]) {
            let end = from.min(FREE_UNTIL);
            if end >= at + FREE_LEAST {
                out.push((day, at, end));
            }
            at = at.max(to);
            if at >= FREE_UNTIL {
                break;
            }
        }
    }
    out
}

/// What the calendars of one's own add to the week `shown` beside the plan's timetable `plan`:
/// their slots, in full or as a strip, all of them stepping back while a choice is in focus
/// (`dim`); and with „Gemeinsam frei" (`together`) the stretches every calendar in the week has free.
pub(super) fn layer(calendars: &[Shown], plan: Option<&Timetable>, shown: Weeks, together: bool, dim: bool, t: &'static i18n::Texts) -> (Vec<GridSlot>, Vec<GridBand>) {
    let visible: Vec<&Shown> = calendars.iter().filter(|calendar| calendar.in_week()).collect();
    let slots = visible
        .iter()
        .filter_map(|calendar| Some((calendar, calendar.table.as_ref()?)))
        .flat_map(|(calendar, table)| super::week::calendar_slots(table, &calendar.titles, shown, calendar, dim, t))
        .collect();
    if !together || visible.is_empty() {
        return (slots, Vec::new());
    }
    let mut busy: Vec<(u8, u16, u16)> = plan.map(|plan| firm(plan, shown)).unwrap_or_default();
    for table in visible.iter().filter_map(|calendar| calendar.table.as_ref()) {
        busy.extend(firm(table, shown));
    }
    let c = &t.studyplan_calendars;
    let label = if visible.len() == 1 { c.both_free } else { c.all_free };
    let bands = free_time(&busy)
        .into_iter()
        .map(|(day, from, to)| {
            let when = format!("{} {}–{}", weekday_name(day, t.locale), clock(from), clock(to));
            GridBand { day, from, to, label: label.to_string(), title: (c.free_title)(&when) }
        })
        .collect();
    (slots, bands)
}

// ---------- making one ----------

/// The shared plan a pasted text names: a link with `share=`, in any language and on any host, or
/// the code alone.
pub(super) fn plan_of_text(text: &str) -> Option<SharedPlan> {
    let text = text.trim();
    let code = match text.split_once("share=") {
        Some((_, rest)) => rest.split(['&', '#', ' ']).next().unwrap_or_default(),
        None => text,
    };
    SharedPlan::from_code(code)
}

/// Makes a calendar of `plan` named `name` (one without a name is named by its number); its
/// number, or `None` at the cap. A click runs it after the next frame (R21).
pub(super) fn create(ctx: PlanCtx, plan: &SharedPlan, name: &str) -> Option<u8> {
    let store = ctx.plan?;
    let key = plan.key()?;
    let (modules, chosen, program) = (plan.module_ids(), plan.chosen_rows(), plan.program.clone());
    store.update(|doc| doc.add_calendar(name, program.as_deref(), key, &modules, &chosen))
}

// ---------- the parts ----------

/// The way a calendar stands, as its switch shows it: its icon and its words.
fn view_parts(view: CalendarView, t: &'static i18n::Texts) -> (&'static str, &'static str, &'static str) {
    let c = &t.studyplan_calendars;
    match view {
        CalendarView::Full => ("square", c.full, c.full_title),
        CalendarView::Strip => ("panel-right", c.strip, c.strip_title),
        CalendarView::Off => ("eye-off", c.off, c.off_title),
    }
}

/// „Mein Studiengang"'s line: the program the plan is of, else „dein Studium".
fn mine_line(ctx: PlanCtx, t: &'static i18n::Texts) -> String {
    ctx.mine.and_then(|mine| mine.with(|doc| doc.name.clone())).unwrap_or_else(|| t.studyplan_calendars.your_study.to_string())
}

/// What a calendar's line under its name says: its modules, else why it shows nothing.
fn calendar_line(calendar: &Shown, semester: &str, t: &'static i18n::Texts) -> String {
    let c = &t.studyplan_calendars;
    match calendar.state {
        State::Empty => (c.nothing_here)(semester),
        State::Loading => c.loading.to_string(),
        State::Failed => c.failed.to_string(),
        State::Ready => (c.modules)(calendar.modules.len(), &calendar.names.join(", ")),
    }
}

/// A calendar's switch: „Voll", „Spur", „Aus", the one it stands in pressed; a click sets it
/// after the next frame (R21). On a phone each with its words, as tall as a finger.
fn view_switch(ctx: PlanCtx, calendar: &Shown, words: bool, t: &'static i18n::Texts) -> impl IntoView {
    let (id, now) = (calendar.id, calendar.view);
    let buttons = CalendarView::ALL
        .into_iter()
        .map(|view| {
            let (icon, word, title) = view_parts(view, t);
            let set = move |_| {
                if let Some(plan) = ctx.plan {
                    plan.update_after_paint(move |doc| doc.set_calendar_view(id, view));
                }
            };
            view! {
                <button type="button" role="radio" aria-checked=if view == now { "true" } else { "false" } title=title aria-label=title on:click=set>
                    <Icon name=icon/>
                    {words.then_some(word)}
                </button>
            }
        })
        .collect_view();
    view! { <span class="sp-cal-seg" role="radiogroup" aria-label=(t.studyplan_calendars.in_week)(&calendar.name)>{buttons}</span> }
}

/// A calendar's menu, open under its row: „Modul aufnehmen" (its „+ Modul" in the list beside the
/// week, `add.rs`), „Umbenennen" (the name in a field, „Sichern" and Enter keep it, „Abbrechen"
/// and Esc leave it) and „Löschen", which takes it away with „Rückgängig".
/// The menu closes at once and the plan changes after the next frame (R21).
#[component]
fn CalendarMenu(ctx: PlanCtx, id: u8, name: String, open: RwSignal<Option<u8>>, removed: RwSignal<Option<(usize, Calendar)>>) -> impl IntoView {
    let t = i18n::t();
    let c = &t.studyplan_calendars;
    let renaming = RwSignal::new(false);
    let text = RwSignal::new(name.clone());
    let save = move || {
        let name = text.get_untracked();
        if let Some(plan) = ctx.plan {
            plan.update_after_paint(move |doc| {
                doc.rename_calendar(id, &name);
            });
        }
        renaming.set(false);
        open.set(None);
    };
    let remove = move |_| {
        if let Some(plan) = ctx.plan {
            folia_design::nav::after_paint(move || {
                let gone = plan.update(|doc| doc.remove_calendar(id));
                let _ = removed.try_set(gone);
            });
        }
        open.set(None);
    };
    move || match renaming.get() {
        true => view! {
            <form class="sp-cal-rename" on:submit=move |ev: leptos::ev::SubmitEvent| { ev.prevent_default(); save(); }>
                <input
                    type="text"
                    aria-label=c.name
                    maxlength=folia_plans::studyplan::MAX_CALENDAR_NAME
                    prop:value=move || text.get()
                    on:input=move |ev| text.set(event_target_value(&ev))
                    on:keydown=move |ev: leptos::ev::KeyboardEvent| if ev.key() == "Escape" { ev.stop_propagation(); renaming.set(false); }
                    autofocus=true
                />
                <button class="mini primary" type="submit">{c.save}</button>
                <button class="mini" type="button" on:click=move |_| renaming.set(false)>{c.cancel}</button>
            </form>
        }
        .into_any(),
        false => view! {
            <div class="sp-cal-menu">
                <button class="mini" type="button" on:click=move |_| { ctx.adding.set(Some(AddTo::Calendar(id))); open.set(None); }><Icon name="plus"/>{c.add_module}</button>
                <button class="mini" type="button" on:click=move |_| renaming.set(true)><Icon name="pencil"/>{c.rename}</button>
                <button class="mini" type="button" on:click=remove><Icon name="trash-2"/>{c.remove}</button>
            </div>
        }
        .into_any(),
    }
}

/// A row of a calendar of one's own: its colour (a square in the way it stands), its name and
/// modules, its switch and its menu. `phone`: the switch with its words, under the name.
fn calendar_row(ctx: PlanCtx, calendar: Shown, semester: String, phone: bool, open: RwSignal<Option<u8>>, removed: RwSignal<Option<(usize, Calendar)>>, t: &'static i18n::Texts) -> impl IntoView {
    let c = &t.studyplan_calendars;
    let id = calendar.id;
    let line = calendar_line(&calendar, &semester, t);
    let way = match calendar.view {
        CalendarView::Full => "full",
        CalendarView::Strip => "strip",
        CalendarView::Off => "off",
    };
    let name = calendar.name.clone();
    let menu = move || {
        (open.get() == Some(id)).then(|| {
            let name = name.clone();
            view! { <CalendarMenu ctx id name open removed/> }
        })
    };
    let toggle = move |_| open.update(|open| *open = if *open == Some(id) { None } else { Some(id) });
    view! {
        <div class=format!("sp-cal {}", calendar.hue) class:off=calendar.view == CalendarView::Off>
            <i class=format!("sw {way}") aria-hidden="true"></i>
            <span class="t"><b>{calendar.name.clone()}</b><small>{line}</small></span>
            {(!phone).then(|| view_switch(ctx, &calendar, false, t))}
            <button class="icon-btn sp-cal-more" type="button" aria-expanded=move || if open.get() == Some(id) { "true" } else { "false" } aria-label=(c.menu)(&calendar.name) title=(c.menu)(&calendar.name) on:click=toggle>
                <Icon name="ellipsis"/>
            </button>
            {phone.then(|| view_switch(ctx, &calendar, true, t))}
        </div>
        {menu}
    }
}

/// The note after „Löschen", with „Rückgängig", which puts the calendar back where it stood after
/// the next frame (R21).
fn removed_note(ctx: PlanCtx, removed: RwSignal<Option<(usize, Calendar)>>, t: &'static i18n::Texts) -> impl IntoView {
    move || {
        removed.get().map(|(_, calendar)| {
            let name = if calendar.name.is_empty() { (t.studyplan_calendars.unnamed)(calendar.id) } else { calendar.name.clone() };
            let undo = move |_| {
                if let (Some(plan), Some((at, calendar))) = (ctx.plan, removed.get_untracked()) {
                    plan.update_after_paint(move |doc| {
                        doc.put_calendar(at, calendar);
                    });
                }
                removed.set(None);
            };
            view! {
                <p class="sp-cal-note" role="status">
                    <span>{(t.studyplan_calendars.removed)(&name)}</span>
                    <button class="mini hit" type="button" on:click=undo>{t.common.undo}</button>
                </p>
            }
        })
    }
}

/// The calendars beside the week, at the top of the sidebar (a page wider than a phone):
/// „Mein Studiengang" with its lock, each calendar of one's own with its switch and its menu, and
/// „Neuer Kalender".
#[component]
pub(super) fn CalendarsGroup(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let c = &t.studyplan_calendars;
    let open = RwSignal::new(None::<u8>);
    let removed = RwSignal::new(None::<(usize, Calendar)>);
    let semester = Memo::new(move |_| ctx.key.get().label(t.locale));
    // On a phone (the sheet „Anpassen") the switch with its words, as tall as a finger.
    let rows = move || {
        let (semester, phone) = (semester.get(), ctx.phone.get());
        ctx.calendars.get().into_iter().map(|calendar| calendar_row(ctx, calendar, semester.clone(), phone, open, removed, t)).collect_view()
    };
    view! {
        <div class="fgroup sp-cals">
            <p class="flabel label">{c.heading}</p>
            <div class="sp-cal mine">
                <i class="sw full" aria-hidden="true"></i>
                <span class="t"><b>{c.mine}</b><small>{move || mine_line(ctx, t)}</small></span>
                <span class="lock" title=c.always_full aria-label=c.always_full><Icon name="lock"/></span>
            </div>
            {rows}
            {removed_note(ctx, removed, t)}
            <NewCalendar ctx/>
        </div>
    }
}

/// The calendars on a phone, where there are any of one's own (no sidebar beside the week; owner,
/// 2026-10-26: no chips): one line over the week, „Kalender" and how each stands („Mein
/// Studiengang, Lena als Spur"), that opens into the rows with their switch as tall as a finger and
/// „Gemeinsam frei". „Neuer Kalender" is in „Anpassen", with the same rows (`CalendarsGroup`).
#[component]
pub(super) fn CalendarLine(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let c = &t.studyplan_calendars;
    let open = RwSignal::new(None::<u8>);
    let removed = RwSignal::new(None::<(usize, Calendar)>);
    let semester = Memo::new(move |_| ctx.key.get().label(t.locale));
    let any = Memo::new(move |_| ctx.calendars.with(|calendars| !calendars.is_empty()) || removed.with(Option::is_some));
    let summary = move || {
        let mut parts = vec![c.mine.to_string()];
        for calendar in ctx.calendars.get() {
            parts.push(match calendar.view {
                CalendarView::Full => (c.line_full)(&calendar.name),
                CalendarView::Strip => (c.line_strip)(&calendar.name),
                CalendarView::Off => (c.line_off)(&calendar.name),
            });
        }
        parts.join(", ")
    };
    let rows = move || {
        let semester = semester.get();
        ctx.calendars.get().into_iter().map(|calendar| calendar_row(ctx, calendar, semester.clone(), true, open, removed, t)).collect_view()
    };
    move || {
        any.get().then(|| {
            view! {
                <details class="sp-cal-line">
                    <summary>
                        <span class="ic" aria-hidden="true"><Icon name="layers"/></span>
                        <span class="t"><b>{c.heading}</b><small>{summary}</small></span>
                        <Icon name="chevron-down" class="chev"/>
                    </summary>
                    <div class="sp-cal-rows">
                        <div class="sp-cal mine">
                            <i class="sw full" aria-hidden="true"></i>
                            <span class="t"><b>{c.mine}</b><small>{move || mine_line(ctx, t)}</small></span>
                            <span class="lock" title=c.always_full aria-label=c.always_full><Icon name="lock"/></span>
                        </div>
                        {rows}
                        {removed_note(ctx, removed, t)}
                        <TogetherSwitch ctx row=true/>
                    </div>
                </details>
            }
        })
    }
}

/// „Gemeinsam frei": in the week's head once another calendar stands in the week (`row`: on a
/// phone a row of its own with a switch); off until it is switched on, a setting of the page.
#[component]
pub(super) fn TogetherSwitch(ctx: PlanCtx, #[prop(optional)] row: bool) -> impl IntoView {
    let t = i18n::t();
    let c = &t.studyplan_calendars;
    let shown = Memo::new(move |_| ctx.calendars.with(|calendars| calendars.iter().any(Shown::in_week)));
    move || {
        shown.get().then(|| {
            let toggle = move |_| ctx.together.update(|on| *on = !*on);
            match row {
                true => view! {
                    <button class="sp-cal-free" type="button" role="switch" aria-checked=move || if ctx.together.get() { "true" } else { "false" } title=c.together_title on:click=toggle>
                        <span class="t"><b>{c.together}</b><small>{c.together_small}</small></span>
                        <i class="tg" aria-hidden="true"><i></i></i>
                    </button>
                }
                .into_any(),
                false => view! {
                    <button class="ghost sp-together" class:on=move || ctx.together.get() type="button" aria-pressed=move || if ctx.together.get() { "true" } else { "false" } title=c.together_title on:click=toggle>
                        <Icon name="users-round"/>{c.together}
                    </button>
                }
                .into_any(),
            }
        })
    }
}

/// „Neuer Kalender": a button that opens the form — the link someone handed on and a name — and
/// says what the link holds before „Anlegen"; the eighth calendar is the last (`MAX_CALENDARS`).
#[component]
pub(super) fn NewCalendar(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let c = &t.studyplan_calendars;
    let open = RwSignal::new(false);
    let link = RwSignal::new(String::new());
    let name = RwSignal::new(String::new());
    let done = RwSignal::new(None::<String>);
    let full = Memo::new(move |_| ctx.calendars.with(Vec::len) >= MAX_CALENDARS);
    let page = Memo::new(move |_| ctx.key.get());
    // What the link holds, as it is typed.
    let holds = Memo::new(move |_| {
        let text = link.get();
        if text.trim().is_empty() {
            return None;
        }
        Some(match plan_of_text(&text) {
            None => Err(c.link_wrong.to_string()),
            Some(plan) => {
                let what = (c.link_holds)(plan.modules.len(), plan.chosen.len());
                match plan.key() {
                    Some(key) if key != page.get() => Ok(format!("{what} {}", (c.link_elsewhere)(&key.label(t.locale)))),
                    _ => Ok(what),
                }
            }
        })
    });
    // The form closes at once; the calendar comes after the next frame (R21), and the note with it.
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(plan) = plan_of_text(&link.get_untracked()) else { return };
        let typed = name.get_untracked();
        link.set(String::new());
        name.set(String::new());
        open.set(false);
        folia_design::nav::after_paint(move || {
            if let Some(id) = create(ctx, &plan, &typed) {
                let shown = if typed.trim().is_empty() { (c.unnamed)(id) } else { typed.trim().to_string() };
                let _ = done.try_set(Some((c.created)(&shown)));
            }
        });
    };
    move || {
        if full.get() {
            return view! { <p class="hint sp-cal-hint">{c.full_up}</p> }.into_any();
        }
        if !open.get() {
            return view! {
                {move || done.get().map(|note| view! { <p class="sp-cal-note" role="status"><Icon name="check"/><span>{note}</span></p> })}
                <button class="sp-cal-add" type="button" on:click=move |_| { done.set(None); open.set(true); }>
                    <Icon name="plus"/>{c.new_calendar}
                </button>
            }
            .into_any();
        }
        let ready = move || holds.with(|holds| matches!(holds, Some(Ok(_))));
        view! {
            <form class="sp-cal-new" on:submit=submit>
                <p class="hint">{c.new_hint}</p>
                <label>
                    <span>{c.link}</span>
                    <input type="text" inputmode="url" autocomplete="off" placeholder=c.link_placeholder prop:value=move || link.get() on:input=move |ev| link.set(event_target_value(&ev))/>
                </label>
                {move || holds.get().map(|holds| match holds {
                    Ok(what) => view! { <p class="sp-cal-holds ok"><Icon name="check"/><span>{what}</span></p> }.into_any(),
                    Err(wrong) => view! { <p class="sp-cal-holds no"><Icon name="triangle-alert"/><span>{wrong}</span></p> }.into_any(),
                })}
                <label>
                    <span>{c.name}</span>
                    <input type="text" maxlength=folia_plans::studyplan::MAX_CALENDAR_NAME placeholder=c.name_placeholder prop:value=move || name.get() on:input=move |ev| name.set(event_target_value(&ev))/>
                </label>
                <div class="sp-cal-actions">
                    <button class="mini primary" type="submit" disabled=move || !ready()>{c.create}</button>
                    <button class="mini" type="button" on:click=move |_| open.set(false)>{c.cancel}</button>
                </div>
            </form>
        }
        .into_any()
    }
}

/// How the note of a module taken out of a calendar of one's own begins (`PlanCtx::undo`, which
/// the import, „Plan leeren" and the list's × share), the calendar's number after it: never shown.
const CALENDAR_REMOVED: &str = "Aus dem Kalender: ";

/// The calendars' parts of the module list beside the week, one per calendar in their order: a
/// part keeps its place (and its „+ Modul" what was typed) while the calendar does.
#[component]
pub(super) fn CalendarModules(ctx: PlanCtx, marked: Memo<Vec<MarkedModule>>) -> impl IntoView {
    let ids = Memo::new(move |_| ctx.calendars.with(|calendars| calendars.iter().map(|calendar| calendar.id).collect::<Vec<u8>>()));
    view! { <For each=move || ids.get() key=|id| *id children=move |id| view! { <CalendarPart ctx id marked/> }/> }
}

/// A calendar's part of the module list: its name in its colour with „+ Modul" (`add.rs`), its
/// modules of the semester as rows of the list (its key in its colour; a click shows the module
/// beside the plan, its × takes it out of the calendar with „Rückgängig"), and where they came from
/// („aus einem Link · mit den Gruppen"), or that it holds none in the semester.
#[component]
fn CalendarPart(ctx: PlanCtx, id: u8, marked: Memo<Vec<MarkedModule>>) -> impl IntoView {
    let t = i18n::t();
    let c = &t.studyplan_calendars;
    let shown = Memo::new(move |_| ctx.calendars.with(|calendars| calendars.iter().find(|calendar| calendar.id == id).cloned()));
    let name = Memo::new(move |_| shown.with(|shown| shown.as_ref().map(|calendar| calendar.name.clone()).unwrap_or_default()));
    let hue = Memo::new(move |_| shown.with(|shown| shown.as_ref().map_or("k-1", |calendar| calendar.hue)));
    let semester = Memo::new(move |_| ctx.key.get().label(t.locale));
    let adding = Memo::new(move |_| ctx.adding.get() == Some(AddTo::Calendar(id)));
    let add = move |_| ctx.adding.update(|adding| *adding = if *adding == Some(AddTo::Calendar(id)) { None } else { Some(AddTo::Calendar(id)) });

    // The note of a module taken out of this calendar.
    let note = Memo::new(move |_| {
        ctx.undo.with(|undo| {
            let rest = undo.as_ref()?.0.strip_prefix(CALENDAR_REMOVED)?;
            let (of, text) = rest.split_once(' ')?;
            (of.parse::<u8>().ok() == Some(id)).then(|| text.to_string())
        })
    });
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
    let remove = move |module: String, title: String| {
        let Some(plan) = ctx.plan else { return };
        let key = ctx.key.get_untracked();
        let events = shown.with_untracked(|shown| shown.as_ref().and_then(|calendar| calendar.table.as_ref()).map(|table| super::aside::only_its_events(table, &module)).unwrap_or_default());
        let text = (c.module_removed)(&title, &name.get_untracked());
        ctx.undo.set(Some((format!("{CALENDAR_REMOVED}{id} {text}"), plan.with_untracked(Clone::clone))));
        plan.update_after_paint(move |doc| doc.remove_from_calendar(id, key, &module, &events));
    };

    // The address without a module beside the plan, the links' start: a sibling of `shown`, both
    // following the address, never read with the address itself (R16).
    let base = Memo::new(move |_| ctx.url.with(|url| url.with_open(None, None)));
    let rows = move || {
        shown.with(|shown| {
            let Some(calendar) = shown.as_ref() else { return ().into_any() };
            if calendar.modules.is_empty() {
                return view! { <p class="sp-cal-came">{(c.none_in)(&semester.get())}</p> }.into_any();
            }
            let came = if calendar.groups > 0 { c.from_link_groups } else { c.from_link };
            let rows = calendar
                .modules
                .iter()
                .zip(&calendar.names)
                .map(|(module, short)| {
                    let title = calendar.full.get(module).cloned().unwrap_or_else(|| (t.studyplan_modules.module_numbered)(module));
                    let href = base.with(|base| t.path(&base.with_open(Some(module), None).path()));
                    let label = (c.remove_module)(&title, &calendar.name);
                    let (module, gone) = (module.clone(), title.clone());
                    view! {
                        <div class="sp-row-wrap">
                            <a class="sp-row" href=href data-noscroll="">
                                <span class="t">
                                    <b>{title}</b>
                                    <small><span class="sp-key">{short.clone()}</span><span class="mono">{module.clone()}</span></small>
                                </span>
                            </a>
                            <button class="icon-btn sp-remove" type="button" title=label.clone() aria-label=label on:click=move |_| remove(module.clone(), gone.clone())>
                                <Icon name="x"/>
                            </button>
                        </div>
                    }
                })
                .collect_view();
            view! { {rows}<p class="sp-cal-came">{came}</p> }.into_any()
        })
    };
    let add_label = move || (t.studyplan_modules.add_named)(&name.get());
    view! {
        <section class=move || format!("sp-cal-mods {}", hue.get()) aria-label=move || name.get()>
            <div class="sp-list-head">
                <h2 class="label"><i class="sw" aria-hidden="true"></i>{move || name.get()}</h2>
                <button class="mini sp-add-mod" type="button" aria-expanded=move || if adding.get() { "true" } else { "false" } aria-label=add_label title=add_label on:click=add>
                    <Icon name="plus"/>{t.studyplan_modules.add}
                </button>
            </div>
            {move || adding.get().then(|| view! { <AddPanel ctx to=AddTo::Calendar(id) marked/> })}
            {move || note.get().map(|note| view! {
                <p class="action note-action sp-marked-note">
                    <Icon name="check"/>
                    <span>{note}</span>
                    <button class="mini hit" type="button" aria-busy=move || restoring.get().then_some("true") on:click=restore>{t.common.undo}</button>
                </p>
            })}
            {rows}
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_time_is_what_no_calendar_holds_from_ninety_minutes_on() {
        // Monday: lectures 09:15–10:45 and 11:30–13:00, a friend's exercise 15:30–17:00.
        let busy = [(1, 555, 645), (1, 690, 780), (1, 930, 1020), (3, 480, 1080), (4, 400, 500)];
        let free = free_time(&busy);
        assert_eq!(
            free,
            [
                // Monday 13:00–15:30 alone: 08:00–09:15 and 10:45–11:30 are too short, 17:00–18:00 too.
                (1, 780, 930),
                // Tuesday free all day.
                (2, 480, 1080),
                // Wednesday held all day; Thursday from 08:20 on.
                (4, 500, 1080),
                (5, 480, 1080),
            ]
        );
        // Overlapping Termine of two calendars count as one stretch.
        assert_eq!(free_time(&[(2, 480, 700), (2, 600, 900), (2, 950, 1080)]), [(1, 480, 1080), (3, 480, 1080), (4, 480, 1080), (5, 480, 1080)]);
    }

    #[test]
    fn a_calendar_stands_in_full_or_as_a_strip_and_says_where_all_are_free() {
        use folia_calendar::select::TownChoice;
        use folia_model::rows_detail::DateRow;
        use folia_plans::studyplan::PlanDoc;
        use folia_timetable::model::Input;

        use super::super::week::tests::{key, row, winter};
        let facts = winter();
        let modules = vec!["12104".to_string()];
        let built = |schedule: &[DateRow]| {
            let input = Input { key: key(), semester: None, facts: &facts, modules: &modules, schedule, exams: &[], sws: &[], taken: &[], choices: &[], listed: &[] };
            Timetable::build(&input, &PlanDoc::default().selection(key(), TownChoice::Derive))
        };
        // The plan: a lecture on Monday 09:15–10:45. Lena's: an exercise on Monday 11:30–13:00, and
        // a practical in two groups, Tuesday and Wednesday 13:45, none ticked.
        let plan = built(&[row("148702", 1, "Vorlesung", "weekly", 1, ("09:15", "10:45"), ("2026-10-05", "2027-01-25"))]);
        let group = |ord: i64, weekday: i64, first: &str| {
            let mut row = row("148370", ord, "Praktikum", "weekly", weekday, ("13:45", "15:15"), (first, "2027-01-29"));
            row.date.group_name = Some(format!("{ord}-Gruppe"));
            row
        };
        let theirs = built(&[row("150210", 1, "Übung", "weekly", 1, ("11:30", "13:00"), ("2026-10-05", "2027-01-25")), group(1, 2, "2026-10-06"), group(2, 3, "2026-10-07")]);
        let lena = |view: CalendarView| Shown {
            id: 1,
            key: SemesterKey::parse("2026W").unwrap(),
            name: "Lena".into(),
            hue: hue_of(1),
            initial: "L".into(),
            view,
            set: true,
            modules: modules.clone(),
            names: vec!["EvS".into()],
            groups: 0,
            table: Some(theirs.clone()),
            titles: BTreeMap::new(),
            full: BTreeMap::new(),
            selection: Selection::default(),
            program: None,
            termine: 1,
            state: State::Ready,
            elsewhere: None,
        };
        // In full: all three, outlined in its colour with its initial, its name first in the tooltip.
        let (slots, bands) = layer(&[lena(CalendarView::Full)], Some(&plan), Weeks::All, false, false, &i18n::DE);
        assert_eq!(slots.iter().map(|slot| (slot.day, slot.class, slot.alt, slot.badge.as_deref())).collect::<Vec<_>>(), [(1, "cal", false, Some("L")), (2, "cal", true, Some("L")), (3, "cal", true, Some("L"))]);
        assert!(slots.iter().all(|slot| slot.hue == Some("k-1") && slot.href.is_none() && slot.acts.is_empty() && slot.title.starts_with("Lena · ") && !slot.dim));
        assert!(bands.is_empty());
        // As a strip what stands firm, without its initial; off nothing; behind a choice in focus,
        // stepping back.
        let (slots, _) = layer(&[lena(CalendarView::Strip)], Some(&plan), Weeks::All, false, true, &i18n::DE);
        assert_eq!(slots.iter().map(|slot| (slot.day, slot.class, slot.badge.is_some(), slot.dim)).collect::<Vec<_>>(), [(1, "strip", false, true)]);
        assert_eq!(layer(&[lena(CalendarView::Off)], Some(&plan), Weeks::All, true, false, &i18n::DE), (Vec::new(), Vec::new()));
        // „Gemeinsam frei": where neither the plan nor Lena has a firm Termin, from 90 minutes on.
        let (_, bands) = layer(&[lena(CalendarView::Strip)], Some(&plan), Weeks::All, true, false, &i18n::DE);
        let said: Vec<(u8, String, &str)> = bands.iter().map(|band| (band.day, format!("{}–{}", clock(band.from), clock(band.to)), band.label.as_str())).collect();
        assert_eq!(said[0], (1, "13:00–18:00".to_string(), "beide frei"));
        assert_eq!(said.iter().map(|(day, ..)| *day).collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
        assert_eq!(bands[0].title, "Mo 13:00–18:00 frei, in jedem Kalender in der Woche");
        // Two calendars beside the plan: „alle frei".
        let mut other = lena(CalendarView::Full);
        other.id = 2;
        let (_, bands) = layer(&[lena(CalendarView::Strip), other], Some(&plan), Weeks::All, true, false, &i18n::DE);
        assert!(bands.iter().all(|band| band.label == "alle frei"));
    }

    #[test]
    fn a_link_is_read_in_any_shape() {
        let key = SemesterKey::parse("2026W").unwrap();
        let code = SharedPlan::of(key, &["12104".to_string()], None).unwrap().code().unwrap();
        for text in [code.clone(), format!("https://betula.app/studyplan?share={code}"), format!("  betula.app/en/studyplan?share={code}&x=1 "), format!("/studyplan?share={code}#top")] {
            assert_eq!(plan_of_text(&text).map(|plan| plan.module_ids()), Some(vec!["12104".to_string()]), "{text}");
        }
        assert!(plan_of_text("https://betula.app/studyplan").is_none());
        assert!(plan_of_text("").is_none());
        assert_eq!(hue_of(1), "k-1");
        assert_eq!(hue_of(7), "k-1");
        assert_eq!(hue_of(6), "k-6");
    }
}
