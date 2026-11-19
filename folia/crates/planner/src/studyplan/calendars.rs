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
//! in a sheet the head's „Kalender" opens, in both states (`CalendarSheet`; owner, 2026-11-18: „Dann
//! fehlt am Handy das Feature für weitere Kalender komplett bitte baue das wieder ein"; until then
//! a line over the week, there only once a calendar of one's own was; no chips, owner, 2026-10-26).
//! A new calendar comes by its name alone or from a link pasted into its form (`NewCalendarForm`,
//! while the plan is edited, in the phone's sheet always), or from the offer of a shared plan
//! (`share.rs`, „Als Kalender"); its menu, there while the plan is edited (in the sheet always),
//! renames it and deletes it, with „Rückgängig". The module list beside the week has a part per
//! calendar (`CalendarModules`).
//!
//! A calendar may follow a share of someone else's (Betula Sync, step 3; folia/design/teilen/
//! teilen.html, `share.rs`, „Als Kalender folgen"): its row says „aktuell" while every change
//! arrives, and „nicht mehr geteilt" once the share was taken back, with „Behalten" (it stays with
//! what it showed last) and „Löschen" under it. Its menu hands a calendar on into a calendar app
//! („In Kalender-App", the dialog „Teilen" on that side; owner, 2026-11-08: to persons only the
//! plan of one's own). While the plan is edited „+ Modul" takes a module into a calendar (`add.rs`);
//! on a phone it is a sheet of its own, also from the calendars' sheet's „Modul aufnehmen", whose
//! way out leads back to that sheet (`AddSheet`).
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
use folia_design::haptic::TapTick;
use folia_design::modal::{kept, Modal, ModalHead, ModalSize, Sheet};
use folia_design::ui::Icon;
use folia_pages::ask::StudyplanAsk;
use folia_plans::studyplan::{Calendar, CalendarView, PlanUndo, MAX_CALENDARS};
use folia_routes::url::{PlanAddress, StudyplanUrl};
use folia_stores::studyplan::Studyplan;
use folia_timetable::model::Timetable;
use folia_widgets::week::{GridBand, GridSlot};
use leptos::prelude::*;

use super::add::{AddPanel, AddTo};
use super::head::{weekday_name, MarkedModule, PhoneSheet};
use super::note::PlanNote;
use super::share_dialog::{ShareOpen, ShareTab};
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
    follow: Option<Following>,
}

/// What a calendar that follows a share of someone else's shows of it (`PlanDoc::follow_of`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Following {
    /// It stays current: every change of the share arrives.
    Live,
    /// The share was taken back (since when, seconds since 1970): it keeps what it showed last.
    Ended(u64),
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
    /// Whether it follows a share, and how that stands.
    pub follow: Option<Following>,
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
    let follow = calendar.follow.as_ref().map(|follow| if follow.ended == 0 { Following::Live } else { Following::Ended(follow.ended) });
    Entry { key, id: calendar.id, name: calendar.name.clone(), view: calendar.view, program: calendar.program.clone(), modules, selection: calendar.selection(key), elsewhere, follow }
}

/// A calendar as the page shows it, its timetable asked of the local catalog; while the answer is
/// on its way, the one it had before stands.
fn shown_of(entry: &Entry, source: StoredValue<Option<DataClient>>, before: Option<&Vec<Shown>>, t: &'static i18n::Texts) -> Shown {
    let name = if entry.name.is_empty() { (t.studyplan_calendars.unnamed)(entry.id) } else { entry.name.clone() };
    let initial = name.chars().next().map(|first| first.to_uppercase().collect::<String>()).unwrap_or_default();
    let ask = |source: &DataClient| source.now(&StudyplanAsk { key: entry.key, ids: entry.modules.clone(), program: entry.program.clone(), locale: t.locale });
    // A calendar of Termine of one's own alone has a timetable too, of no module.
    let answer = (!entry.modules.is_empty() || !entry.selection.own.is_empty()).then(|| source.with_value(|source| source.as_ref().map(ask))).flatten();
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
        follow: entry.follow,
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

/// Makes a calendar of `plan` named `name` (one without a name is named by its number), with its
/// groups and its changes of Termine of QIS; its number, or `None` at the cap. A click runs it
/// after the next frame (R21).
pub(super) fn create(ctx: PlanCtx, plan: &SharedPlan, name: &str) -> Option<u8> {
    let store = ctx.plan?;
    let key = plan.key()?;
    let (modules, chosen, changes, program) = (plan.module_ids(), plan.chosen_rows(), plan.change_list(), plan.program.clone());
    store.update(|doc| doc.add_calendar(name, program.as_deref(), key, &modules, &chosen, &changes))
}

/// A calendar of one's own without a link („Neuer Kalender" takes a name alone): empty, of the
/// semester shown, for modules one adds to it („+ Modul"); its number, or `None` at the cap.
fn create_empty(ctx: PlanCtx, name: &str) -> Option<u8> {
    let (store, key) = (ctx.plan?, ctx.key.get_untracked());
    store.update(|doc| doc.add_calendar(name, None, key, &[], &[], &[]))
}

/// What the note calls a calendar just made: the name typed, else „Kalender 2" (its number).
fn made_name(typed: &str, id: u8, t: &i18n::Texts) -> String {
    match typed.trim() {
        "" => (t.studyplan_calendars.unnamed)(id),
        name => name.to_string(),
    }
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
/// and Esc leave it) and „Löschen", which takes it away with „Rückgängig" (`remove_calendar`).
/// The menu closes at once and the plan changes after the next frame (R21).
#[component]
fn CalendarMenu(ctx: PlanCtx, id: u8, name: String, open: RwSignal<Option<u8>>) -> impl IntoView {
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
        open.set(None);
        remove_calendar(ctx, id);
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
                // „+ Modul" stands under the week in the calendar's part of the list; on a phone it
                // is a sheet in the calendars' sheet's place, whose way out leads back to it.
                <button class="mini" type="button" on:click=move |_| {
                    open.set(None);
                    if ctx.phone.get_untracked() {
                        ctx.sheet.set(Some(PhoneSheet::Adding { id, back: true }));
                    } else {
                        ctx.adding.set(Some(AddTo::Calendar(id)));
                    }
                }><Icon name="plus"/>{c.add_module}</button>
                <button class="mini" type="button" on:click=move |_| renaming.set(true)><Icon name="pencil"/>{c.rename}</button>
                // Into a calendar app, where the site offers Betula Sync, whose service keeps it
                // current (the dialog offers to save a private profile).
                {folia_stores::sync::offered().is_some().then(|| view! {
                    <button class="mini" type="button" title=c.to_app_title on:click=move |_| { ctx.sharing.set(Some(ShareOpen { calendar: id, tab: ShareTab::App })); open.set(None); }>
                        <Icon name="calendar-plus"/>{c.to_app}
                    </button>
                })}
                <button class="mini" type="button" on:click=remove><Icon name="trash-2"/>{c.remove}</button>
            </div>
        }
        .into_any(),
    }
}

/// Where a calendar's modules came from, said only where the calendar shows it: ticked groups, a
/// share it follows, a program of its own come with a link alone. A calendar made by its name (its
/// modules taken in by „+ Modul") says nothing: the plan keeps no word of where one came from.
/// Without a module: that it has none in the semester.
fn came_of(calendar: &Shown, semester: &str, t: &'static i18n::Texts) -> Option<String> {
    let c = &t.studyplan_calendars;
    match calendar.modules.is_empty() {
        true => Some((c.none_in)(semester)),
        false if calendar.groups > 0 => Some(c.from_link_groups.to_string()),
        false if calendar.follow.is_some() || calendar.program.is_some() => Some(c.from_link.to_string()),
        false => None,
    }
}

/// A row of a calendar of one's own: its colour (a square in the way it stands), its name and
/// modules, its switch („Voll · Spur · Aus", in both states: it is no setting of the plan's
/// Termine), and while `manage` says so its menu („…"), in a place that is there without it
/// (`span.sp-cal-end`), so that the row is not built anew with the mode: in the sidebar while the
/// plan is edited, in a phone's sheet always. `phone`: the switch with its words, under the name.
fn calendar_row(ctx: PlanCtx, calendar: Shown, semester: String, phone: bool, manage: Signal<bool>, open: RwSignal<Option<u8>>, t: &'static i18n::Texts) -> impl IntoView {
    let c = &t.studyplan_calendars;
    let id = calendar.id;
    let line = calendar_line(&calendar, &semester, t);
    let way = match calendar.view {
        CalendarView::Full => "full",
        CalendarView::Strip => "strip",
        CalendarView::Off => "off",
    };
    let name = calendar.name.clone();
    let title = calendar.name.clone();
    let menu = move || {
        (open.get() == Some(id) && manage.get()).then(|| {
            let name = name.clone();
            view! { <CalendarMenu ctx id name open/> }
        })
    };
    let toggle = move |_| open.update(|open| *open = if *open == Some(id) { None } else { Some(id) });
    let following = calendar.follow.map(|follow| match follow {
        Following::Live => view! { <span class="live" title=c.live_title>{c.live}</span>" · " }.into_any(),
        Following::Ended(_) => view! { <span class="live off" title=c.ended_title>{c.ended}</span>" · " }.into_any(),
    });
    let ended = matches!(calendar.follow, Some(Following::Ended(_))).then(|| ended_note(ctx, id, t));
    view! {
        <div class=format!("sp-cal {}", calendar.hue) class:off=calendar.view == CalendarView::Off>
            <i class=format!("sw {way}") aria-hidden="true"></i>
            <span class="t"><b>{calendar.name.clone()}</b><small>{following}{line}</small></span>
            {(!phone).then(|| view_switch(ctx, &calendar, false, t))}
            <span class="sp-cal-end">
                {move || {
                    let label = (c.menu)(&title);
                    manage.get().then(|| view! {
                        <button class="icon-btn sp-cal-more" type="button" aria-expanded=move || if open.get() == Some(id) { "true" } else { "false" } aria-label=label.clone() title=label on:click=toggle>
                            <Icon name="ellipsis"/>
                        </button>
                    })
                }}
            </span>
            {phone.then(|| view_switch(ctx, &calendar, true, t))}
        </div>
        {ended}
        {menu}
    }
}

/// Under a calendar whose share was taken back: what that means, „Behalten" (it follows nothing any
/// more and stays as it is) and „Löschen" (with „Rückgängig", as from its menu); each after the next
/// frame (R21). Owner, 2026-11-08: Lena keeps the last state and can keep or delete it.
fn ended_note(ctx: PlanCtx, id: u8, t: &'static i18n::Texts) -> impl IntoView {
    let c = &t.studyplan_calendars;
    let keep = move |_| {
        if let Some(plan) = ctx.plan {
            plan.update_after_paint(move |doc| {
                doc.unfollow(id);
            });
        }
    };
    let remove = move |_| remove_calendar(ctx, id);
    view! {
        <p class="sp-cal-ended" role="status">
            <span>{c.ended_hint}</span>
            <button class="mini hit" type="button" title=c.keep_title on:click=keep><Icon name="check"/>{c.keep}</button>
            <button class="mini hit" type="button" on:click=remove><Icon name="trash-2"/>{c.remove}</button>
        </p>
    }
}

/// „Löschen" of a calendar of one's own (its menu, the line of a share taken back): it goes after
/// the next frame (R21), and the page's note says so with „Rückgängig", which puts it back where it
/// stood with its Termine and its shares (`PlanUndo`); a note that lasts (`note.rs`), since what the
/// calendar held is gone with its „Rückgängig". Until 2026-11-19 a line under the calendars said it.
fn remove_calendar(ctx: PlanCtx, id: u8) {
    let t = i18n::t();
    let Some(plan) = ctx.plan else { return };
    folia_design::nav::after_paint(move || {
        let before = plan.with_untracked(Clone::clone);
        let Some(name) = before.calendar(id).map(|calendar| if calendar.name.is_empty() { (t.studyplan_calendars.unnamed)(id) } else { calendar.name.clone() }) else { return };
        let undo = PlanUndo::made(before, |doc| {
            doc.remove_calendar(id);
        });
        plan.update(|doc| {
            doc.remove_calendar(id);
        });
        let _ = ctx.undo.try_set(Some((format!("{CALENDAR_GONE}{}", (t.studyplan_calendars.removed)(&name)), undo)));
    });
}

/// What the group opens over itself while the plan is edited — a calendar's menu („…"), the form
/// „Neuer Kalender" — is a layer of the page while `shown` (`folia_shell::nav::layer_while`): Esc
/// closes it first, before the mode (a field typed in gives up the keyboard before that), and on a
/// phone Back. It closes when the mode is left, so that entering the mode again finds the group as
/// it is drawn, never with a form open.
fn layer_of_mode(ctx: PlanCtx, shown: impl Fn() -> bool + Copy + Send + Sync + 'static, close: impl Fn() + Copy + Send + Sync + 'static) {
    folia_shell::nav::layer_while(move || shown() && ctx.edit.get(), close, folia_shell::nav::Kind::Over);
    Effect::new(move |_| {
        if !ctx.edit.get() {
            untrack(close);
        }
    });
    #[cfg(feature = "csr")]
    {
        let front = folia_design::stage::front();
        Effect::new(move |_| {
            let keys = window_event_listener(leptos::ev::keydown, move |ev| {
                // A dialog over the page (a card, the form of a Termin, the keys' overview) takes
                // its Esc itself: one Esc, the layer in front.
                if ev.key() != "Escape" || ev.default_prevented() || folia_shell::nav::watched() || !front.get_untracked() || !untrack(shown) || folia_design::modal::dialog_in_front(None) {
                    return;
                }
                // One Esc, one thing: the Esc that takes the keyboard out of a field (enhance.js)
                // leaves the form open.
                let typed = ev.target().and_then(|target| leptos::wasm_bindgen::JsCast::dyn_into::<leptos::web_sys::Element>(target).ok()).is_some_and(|element| element.matches("input, textarea, select, [contenteditable]").unwrap_or(false));
                if typed {
                    return;
                }
                ev.prevent_default();
                close();
            });
            on_cleanup(move || keys.remove());
        });
    }
}

/// The id of the field „Name" of „Neuer Kalender", which takes the keyboard when the form opens.
const NEW_NAME_ID: &str = "sp-cal-new-name";

/// The calendars beside the week, at the top of the sidebar (a page wider than a phone), in both
/// states: „Mein Studiengang", each calendar of one's own with its switch; while the plan is
/// edited „+ Kalender" in the group's head, the form „Neuer Kalender" under it, and each calendar's
/// menu.
#[component]
pub(super) fn CalendarsGroup(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let c = &t.studyplan_calendars;
    let adding = RwSignal::new(false);
    let open = RwSignal::new(None::<u8>);
    layer_of_mode(ctx, move || adding.get(), move || adding.set(false));
    layer_of_mode(ctx, move || open.with(Option::is_some), move || open.set(None));
    let semester = Memo::new(move |_| ctx.key.get().label(t.locale));
    // The sidebar's rows: the switch by its signs beside the name (a phone's sheet has its words).
    let rows = move || {
        let semester = semester.get();
        let manage = Signal::derive(move || ctx.edit.get());
        ctx.calendars.get().into_iter().map(|calendar| calendar_row(ctx, calendar, semester.clone(), false, manage, open, t)).collect_view()
    };
    view! {
        // A section of the keyboard's mode (`FrameKeys`), whose rows typing finds (`data-find`).
        <div class="fgroup sp-cals" data-section="" data-find="" role="group" aria-label=c.heading>
            <div class="sp-cals-head">
                <p class="flabel label">{c.heading}</p>
                {move || ctx.edit.get().then(|| view! { <NewCalendarButton ctx open=adding/> })}
            </div>
            {move || ctx.edit.get().then(|| view! { <NewCalendarForm ctx open=adding/> })}
            <div class="sp-cal mine">
                <i class="sw full" aria-hidden="true"></i>
                <span class="t"><b>{c.mine}</b><small>{move || mine_line(ctx, t)}</small></span>
            </div>
            {rows}
        </div>
    }
}

/// The calendars on a phone, in a sheet the head's „Kalender" opens (owner, 2026-11-18: „Dann fehlt
/// am Handy das Feature für weitere Kalender komplett bitte baue das wieder ein"), their one place
/// there, in both states — it is opened on purpose: „Mein Studiengang", each calendar of one's own
/// with its switch in words as tall as a finger and its menu („…": „Modul aufnehmen", „Umbenennen",
/// „Löschen" with „Rückgängig"), „Gemeinsam frei" once another calendar stands in the week, and „+
/// Kalender" with the form „Neuer Kalender" under it. Until then a phone had a line „Kalender" over
/// the week, there only once a calendar of one's own was, and „+ Kalender" as the last row of the
/// module list while editing. Esc closes what stands open in it first, then the sheet.
#[component]
pub(super) fn CalendarSheet(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let c = &t.studyplan_calendars;
    let open = Signal::derive(move || ctx.sheet.get() == Some(super::head::PhoneSheet::Calendars));
    let adding = RwSignal::new(false);
    let menu = RwSignal::new(None::<u8>);
    // Its way out closes it alone: closed because another sheet took its place, it leaves that be.
    let close = Callback::new(move |()| {
        if ctx.sheet.get_untracked() == Some(PhoneSheet::Calendars) {
            ctx.sheet.set(None);
        }
    });
    let cancel = Callback::new(move |()| {
        if menu.with_untracked(Option::is_some) {
            menu.set(None);
            return true;
        }
        if adding.get_untracked() {
            adding.set(false);
            return true;
        }
        false
    });
    // Opened again, the sheet is as it is drawn: no menu, no form.
    Effect::new(move |_| {
        if !open.get() {
            menu.set(None);
            adding.set(false);
        }
    });
    let semester = Memo::new(move |_| ctx.key.get().label(t.locale));
    let manage = Signal::stored(true);
    let rows = move || {
        let semester = semester.get();
        ctx.calendars.get().into_iter().map(|calendar| calendar_row(ctx, calendar, semester.clone(), true, manage, menu, t)).collect_view()
    };
    view! {
        <Modal open on_close=close size=ModalSize::M sheet=Sheet::Auto class="sp-cal-sheet" labelled_by="sp-cal-sheet-title" on_cancel=cancel>
            <ModalHead id="sp-cal-sheet-title" title=c.heading sub=Some(t.studyplan_head.calendars_sub.to_string()) on_close=close/>
            <div class="st-dlg-body">
                <div class="sp-cals sp-cal-rows">
                    <div class="sp-cal mine">
                        <i class="sw full" aria-hidden="true"></i>
                        <span class="t"><b>{c.mine}</b><small>{move || mine_line(ctx, t)}</small></span>
                    </div>
                    {rows}
                    <TogetherSwitch ctx row=true/>
                    <div class="sp-cal-newrow">
                        <NewCalendarButton ctx open=adding/>
                        <NewCalendarForm ctx open=adding/>
                    </div>
                </div>
            </div>
            // What is done in the sheet is said at its foot (`note.rs`).
            <PlanNote ctx shown=open/>
        </Modal>
    }
}

/// „+ Modul" of a calendar of one's own on a phone (`AddPanel`), a sheet of its own: from the
/// calendars' sheet's „Modul aufnehmen" in that sheet's place, and its way out — ×, Esc, Back —
/// leads back to the calendars' sheet; from the list beside the week („+ Modul" while editing) it
/// closes to the page. Until then the panel opened under the week, far below the head, and the
/// keyboard stayed on the head's „Kalender".
#[component]
pub(super) fn AddSheet(ctx: PlanCtx, marked: Memo<Vec<MarkedModule>>) -> impl IntoView {
    let t = i18n::t();
    let asked = Memo::new(move |_| match ctx.sheet.get() {
        Some(PhoneSheet::Adding { id, back }) => Some((id, back)),
        _ => None,
    });
    let open = Signal::derive(move || asked.with(Option::is_some));
    // What it shows while it glides away.
    let shown = kept(move || asked.get());
    let close = Callback::new(move |()| {
        let Some((_, back)) = asked.get_untracked() else { return };
        ctx.sheet.set(back.then_some(PhoneSheet::Calendars));
    });
    let name = Memo::new(move |_| {
        let id = shown.with(|shown| shown.map(|(id, _)| id));
        ctx.calendars.with(|calendars| calendars.iter().find(|calendar| Some(calendar.id) == id).map(|calendar| calendar.name.clone()).unwrap_or_default())
    });
    let follows = Memo::new(move |_| {
        let id = shown.with(|shown| shown.map(|(id, _)| id));
        ctx.calendars.with(|calendars| calendars.iter().any(|calendar| Some(calendar.id) == id && calendar.follow == Some(Following::Live)))
    });
    let id = Memo::new(move |_| shown.with(|shown| shown.map(|(id, _)| id)));
    view! {
        <Modal open on_close=close size=ModalSize::M sheet=Sheet::Full class="sp-add-sheet" labelled_by="sp-add-sheet-title">
            <ModalHead id="sp-add-sheet-title" title=(t.studyplan_add.title)(&name.get_untracked()) on_close=close/>
            <div class="st-dlg-body">
                {move || follows.get().then(|| view! { <p class="hint sp-cal-follows">{(t.studyplan_add.follows)(&name.get())}</p> })}
                {move || id.get().map(|id| view! { <AddPanel ctx to=AddTo::Calendar(id) marked sheet=close/> })}
            </div>
            // What „Aufnehmen" did is said at the sheet's foot (`note.rs`).
            <PlanNote ctx shown=open/>
        </Modal>
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
                    <button class="sp-cal-free tap-host" type="button" role="switch" aria-checked=move || if ctx.together.get() { "true" } else { "false" } title=c.together_title on:click=toggle>
                        <span class="t"><b>{c.together}</b><small>{c.together_small}</small></span>
                        <i class="tg" aria-hidden="true"><i></i></i>
                        <TapTick/>
                    </button>
                }
                .into_any(),
                false => view! {
                    <button class="ghost sp-together" class:on=move || ctx.together.get() type="button" aria-pressed=move || if ctx.together.get() { "true" } else { "false" } title=c.together_title on:click=toggle>
                        <Icon name="users-round"/><span class="tg-text">{c.together}</span>
                    </button>
                }
                .into_any(),
            }
        })
    }
}

/// „+ Kalender" in the group's head while the plan is edited, and last in a phone's sheet of the
/// calendars: opens the form „Neuer Kalender" under it. At the cap (`MAX_CALENDARS`) nothing: the
/// form's place says why.
#[component]
fn NewCalendarButton(ctx: PlanCtx, open: RwSignal<bool>) -> impl IntoView {
    let t = i18n::t();
    let c = &t.studyplan_calendars;
    let full = Memo::new(move |_| ctx.calendars.with(Vec::len) >= MAX_CALENDARS);
    move || {
        (!full.get()).then(|| {
            view! {
                <button class="sp-cal-add" type="button" title=c.new_calendar aria-expanded=move || if open.get() { "true" } else { "false" } on:click=move |_| {
                    let opening = !open.get_untracked();
                    open.set(opening);
                    // The form takes the keyboard into „Name" (`autofocus` acts on a page's load
                    // alone, not on a field put in later).
                    if opening {
                        request_animation_frame(|| folia_design::nav::focus_by_id(NEW_NAME_ID));
                    }
                }>
                    <Icon name="plus"/>{c.add_calendar}
                </button>
            }
        })
    }
}

/// The form „Neuer Kalender" while `open`: „Name" first, then „Link (optional)" and what the
/// link holds; „Anlegen" is never disabled (CLAUDE.md, „Betula does not say no"): without a link, or
/// with a text that is none (it says so, and that „Anlegen" still makes one), the calendar is empty;
/// with a link it is the plan of the link. An empty name is „Kalender 2". The form closes at once
/// and the calendar comes after the next frame (R21), and the page's note with it, with „Rückgängig"
/// as after „Als Kalender" (`note.rs`; until 2026-11-19 a line in the form's place). At the cap the
/// hint.
#[component]
fn NewCalendarForm(ctx: PlanCtx, open: RwSignal<bool>) -> impl IntoView {
    let t = i18n::t();
    let c = &t.studyplan_calendars;
    let link = RwSignal::new(String::new());
    let name = RwSignal::new(String::new());
    let full = Memo::new(move |_| ctx.calendars.with(Vec::len) >= MAX_CALENDARS);
    let page = Memo::new(move |_| ctx.key.get());
    // What the link holds, as it is typed.
    let holds = Memo::new(move |_| {
        let text = link.get();
        if text.trim().is_empty() {
            return None;
        }
        Some(match plan_of_text(&text) {
            None => Err(format!("{} {}", c.link_wrong, c.link_wrong_still)),
            Some(plan) => {
                let what = (c.link_holds)(plan.modules.len(), plan.chosen.len());
                match plan.key() {
                    Some(key) if key != page.get() => Ok(format!("{what} {}", (c.link_elsewhere)(&key.label(t.locale)))),
                    _ => Ok(what),
                }
            }
        })
    });
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let text = link.get_untracked();
        let plan = plan_of_text(&text);
        // A link typed that names no plan: the calendar is made all the same, empty, and the note
        // says why it is empty (CLAUDE.md, „Betula does not say no": made, and said).
        let unread = plan.is_none() && !text.trim().is_empty();
        let typed = name.get_untracked();
        link.set(String::new());
        name.set(String::new());
        open.set(false);
        folia_design::nav::after_paint(move || {
            let Some(store) = ctx.plan else { return };
            let before = store.with_untracked(Clone::clone);
            let made = match &plan {
                Some(plan) => create(ctx, plan, &typed),
                None => create_empty(ctx, typed.trim()),
            };
            if let Some(id) = made {
                let named = made_name(&typed, id, t);
                let said = if unread { format!("{CALENDAR_UNREAD}{}", (c.created_unread)(&named)) } else { format!("{CALENDAR_MADE}{}", (c.created)(&named)) };
                let _ = ctx.undo.try_set(Some((said, PlanUndo::of(before, store.with_untracked(Clone::clone)))));
            }
        });
    };
    move || {
        if full.get() {
            return view! { <p class="hint sp-cal-hint">{c.full_up}</p> }.into_any();
        }
        if !open.get() {
            return ().into_any();
        }
        view! {
            <form class="sp-cal-new" on:submit=submit>
                <p class="hint">{c.new_hint}</p>
                <label>
                    <span>{c.name}</span>
                    <input type="text" maxlength=folia_plans::studyplan::MAX_CALENDAR_NAME placeholder=c.name_placeholder prop:value=move || name.get() on:input=move |ev| name.set(event_target_value(&ev)) id=NEW_NAME_ID/>
                </label>
                <label>
                    <span>{c.link}</span>
                    <input type="text" inputmode="url" autocomplete="off" placeholder=c.link_placeholder prop:value=move || link.get() on:input=move |ev| link.set(event_target_value(&ev))/>
                </label>
                {move || holds.get().map(|holds| match holds {
                    Ok(what) => view! { <p class="sp-cal-holds ok"><Icon name="check"/><span>{what}</span></p> }.into_any(),
                    Err(wrong) => view! { <p class="sp-cal-holds no"><Icon name="triangle-alert"/><span>{wrong}</span></p> }.into_any(),
                })}
                <div class="sp-cal-actions">
                    <button class="mini primary" type="submit">{c.create}</button>
                    <button class="mini" type="button" on:click=move |_| open.set(false)>{c.cancel}</button>
                </div>
            </form>
        }
        .into_any()
    }
}

/// How the note of a module taken out of a calendar of one's own begins (`PlanCtx::undo`, which
/// the page's other notes share, `note.rs`), the calendar's number after it: never shown.
pub(super) const CALENDAR_REMOVED: &str = "Aus dem Kalender: ";

/// How the note of a calendar of one's own deleted begins: a mark of its own, never shown; a note
/// that lasts (`note.rs`).
pub(super) const CALENDAR_GONE: &str = "Kalender gelöscht: ";

/// How the note of a calendar made by „Neuer Kalender" begins, and of one made empty of a link that
/// could not be read (which it says, in amber): marks of their own, never shown.
pub(super) const CALENDAR_MADE: &str = "Kalender angelegt: ";
pub(super) const CALENDAR_UNREAD: &str = "Kalender leer angelegt: ";

/// The calendars' parts of the module list beside the week, one per calendar in their order: a
/// part keeps its place (and its „+ Modul" what was typed) while the calendar does. (On a phone „+
/// Kalender" is in the calendars' sheet, `CalendarSheet`.)
#[component]
pub(super) fn CalendarModules(ctx: PlanCtx, marked: Memo<Vec<MarkedModule>>) -> impl IntoView {
    let ids = Memo::new(move |_| ctx.calendars.with(|calendars| calendars.iter().map(|calendar| calendar.id).collect::<Vec<u8>>()));
    view! {
        <For each=move || ids.get() key=|id| *id children=move |id| view! { <CalendarPart ctx id marked/> }/>
    }
}

/// A calendar's part of the module list: its name in its colour, a line per module of the semester
/// (its key in the calendar's colour, its title) leading to the module's own page — a calendar of
/// one's own is no part of the study, so the panel of the plan's module would only say „Nicht im
/// Studienplan." —, where they came from („aus einem Link · mit den Gruppen"), or that it holds
/// none in the semester; then its Termine of one's own. While the plan is edited the head carries
/// „+ Modul" (`add.rs`) and each module the bin, which takes it out of the calendar with „Rückgängig"
/// (owner, 2026-11-17: „Bei den weitern Kalender soll man btw weiter module hinzufügen könenn, weil
/// das ja nur die Anzeige für die termine sind"; „da wo es entfernt eine Mülltonne"). Its modules
/// have no rows of events yet: nothing of them can be shown, hidden or opened here (a second pass).
#[component]
fn CalendarPart(ctx: PlanCtx, id: u8, marked: Memo<Vec<MarkedModule>>) -> impl IntoView {
    let t = i18n::t();
    let c = &t.studyplan_calendars;
    let shown = Memo::new(move |_| ctx.calendars.with(|calendars| calendars.iter().find(|calendar| calendar.id == id).cloned()));
    let name = Memo::new(move |_| shown.with(|shown| shown.as_ref().map(|calendar| calendar.name.clone()).unwrap_or_default()));
    let hue = Memo::new(move |_| shown.with(|shown| shown.as_ref().map_or("k-1", |calendar| calendar.hue)));
    let follows = Memo::new(move |_| shown.with(|shown| shown.as_ref().is_some_and(|calendar| calendar.follow == Some(Following::Live))));
    let semester = Memo::new(move |_| ctx.key.get().label(t.locale));
    let adding = Memo::new(move |_| ctx.adding.get() == Some(AddTo::Calendar(id)));
    // On a phone „+ Modul" is a sheet of its own (`AddSheet`).
    let add = move |_| {
        if ctx.phone.get_untracked() {
            ctx.sheet.set(Some(PhoneSheet::Adding { id, back: false }));
            return;
        }
        ctx.adding.update(|adding| *adding = if *adding == Some(AddTo::Calendar(id)) { None } else { Some(AddTo::Calendar(id)) });
    };

    // A module taken out of this calendar: the page's note says so with „Rückgängig" (`note.rs`).
    let remove = move |module: String, title: String| {
        let Some(plan) = ctx.plan else { return };
        let key = ctx.key.get_untracked();
        let events = shown.with_untracked(|shown| shown.as_ref().and_then(|calendar| calendar.table.as_ref()).map(|table| super::aside::only_its_events(table, &module)).unwrap_or_default());
        let text = (c.module_removed)(&title, &name.get_untracked());
        let undo = PlanUndo::made(plan.with_untracked(Clone::clone), |doc| doc.remove_from_calendar(id, key, &module, &events));
        ctx.undo.set(Some((format!("{CALENDAR_REMOVED}{id} {text}"), undo)));
        plan.update_after_paint(move |doc| doc.remove_from_calendar(id, key, &module, &events));
    };

    // The address without a module beside the plan, the links' start: a sibling of `shown`, both
    // following the address, never read with the address itself (R16).
    let base = Memo::new(move |_| ctx.url.with(|url| url.with_open(None, None)));
    // The modules as plain values (key in a keyed list, R5): what a line shows, and its address.
    let lines = Memo::new(move |_| {
        shown.with(|shown| {
            let Some(calendar) = shown.as_ref() else { return Vec::new() };
            calendar
                .modules
                .iter()
                .zip(&calendar.names)
                .map(|(module, short)| {
                    let title = calendar.full.get(module).cloned().unwrap_or_else(|| (t.studyplan_modules.module_numbered)(module));
                    let href = base.with(|base| t.path(&PlanAddress { url: base.with_open(Some(module), None), full: true }.path()));
                    (module.clone(), short.clone(), title, href)
                })
                .collect::<Vec<_>>()
        })
    });
    let came = Memo::new(move |_| {
        let semester = semester.get();
        shown.with(|shown| came_of(shown.as_ref()?, &semester, t))
    });
    let line = move |(module, short, title, href): (String, String, String, String)| {
        let label = Signal::derive({
            let title = title.clone();
            move || (c.remove_module)(&title, &name.get())
        });
        let (gone, whole) = (title.clone(), title.clone());
        view! {
            <div class="sp-row-wrap">
                // The module's own page, a page of its own: it opens at its top.
                <a class="sp-row sp-mod-row" href=href title=whole>
                    <span class="sp-key">{short}</span>
                    <b>{title}</b>
                </a>
                // The bin's place is there while looking too, so that no title moves.
                <span class="sp-mod-end">
                    {move || {
                        let (module, gone) = (module.clone(), gone.clone());
                        ctx.edit.get().then(|| view! {
                            <button class="icon-btn sp-remove" type="button" title=label aria-label=label on:click=move |_| remove(module.clone(), gone.clone())>
                                <Icon name="trash-2"/>
                            </button>
                        })
                    }}
                </span>
            </div>
        }
    };
    let add_label = move || (t.studyplan_modules.add_named)(&name.get());
    view! {
        <section class=move || format!("sp-cal-mods {}", hue.get()) aria-label=move || name.get() data-stop="y">
            <div class="sp-list-head">
                <h2 class="label"><i class="sw" aria-hidden="true"></i>{move || name.get()}</h2>
                {move || ctx.edit.get().then(|| view! {
                    <button class="mini sp-add-mod" type="button" aria-expanded=move || if adding.get() { "true" } else { "false" } aria-label=add_label title=add_label on:click=add>
                        <Icon name="plus"/>{t.studyplan_modules.add}
                    </button>
                })}
            </div>
            // While editing a wide page (on a phone a sheet, `AddSheet`).
            {move || (adding.get() && ctx.edit.get() && !ctx.phone.get()).then(|| view! {
                // A calendar that follows a share takes the sharer's next change over what is added.
                {move || follows.get().then(|| view! { <p class="hint sp-cal-follows">{(t.studyplan_add.follows)(&name.get())}</p> })}
                <AddPanel ctx to=AddTo::Calendar(id) marked/>
            })}
            <For each=move || lines.get() key=|line| line.clone() children=line/>
            {move || came.get().map(|came| view! { <p class="sp-cal-came">{came}</p> })}
            // Its Termine of one's own, and while editing „+ Termin" with this calendar chosen (`own.rs`).
            <super::own::OwnList ctx calendar=id hue=Signal::derive(move || hue.get())/>
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
            follow: None,
        };
        // Where its modules came from: said only where the calendar shows it (ticked groups, a
        // program of its own); one made by its name says nothing, one without modules that.
        let t = &i18n::DE;
        assert_eq!(came_of(&lena(CalendarView::Full), "WiSe 26/27", t), None);
        assert_eq!(came_of(&Shown { groups: 2, ..lena(CalendarView::Full) }, "WiSe 26/27", t).as_deref(), Some(t.studyplan_calendars.from_link_groups));
        assert_eq!(came_of(&Shown { program: Some("013-82-2018".into()), ..lena(CalendarView::Full) }, "WiSe 26/27", t).as_deref(), Some(t.studyplan_calendars.from_link));
        assert_eq!(came_of(&Shown { modules: Vec::new(), ..lena(CalendarView::Full) }, "WiSe 26/27", t), Some((t.studyplan_calendars.none_in)("WiSe 26/27")));
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

    /// „Neuer Kalender" by a name alone, by nothing, or by a text that is no link: an empty calendar
    /// each time, called by the name typed, else by its number.
    #[test]
    fn a_calendar_is_made_by_its_name_alone() {
        let key = SemesterKey::parse("2026W").unwrap();
        let mut doc = folia_plans::studyplan::PlanDoc::default();
        assert!(plan_of_text("Das ist kein Link").is_none());
        let sport = doc.add_calendar("Sport", None, key, &[], &[], &[]).unwrap();
        let nameless = doc.add_calendar("", None, key, &[], &[], &[]).unwrap();
        assert_eq!(doc.calendars.len(), 2);
        assert!(doc.calendars.iter().all(|calendar| calendar.modules.is_empty() && calendar.program.is_none()), "{:?}", doc.calendars);
        assert_eq!(doc.calendars.iter().map(|calendar| calendar.name.as_str()).collect::<Vec<_>>(), ["Sport", ""]);
        assert_eq!((made_name(" Sport ", sport, &i18n::DE), made_name("", nameless, &i18n::DE)), ("Sport".to_string(), format!("Kalender {nameless}")));
        assert_eq!(made_name("  ", nameless, &i18n::EN), format!("Calendar {nameless}"));
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
