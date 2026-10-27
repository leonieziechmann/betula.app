//! Termine of one's own in the Stundenplan (folia/design/kalender/termine.html, its first step;
//! owner, 2026-10-27: „Passt alles, bau Schritt 1 jetzt in Folia"): what a student adds beside what
//! QIS lists — a job, sport, a Lerngruppe, the Termin of a module QIS has none for.
//!
//! The ways in: „+ Termin" in the week's head, under the modules of each calendar (that calendar
//! chosen), „Termin eintragen" at a module without Termine (that module chosen), a click on a free
//! stretch of a day of the week (its day and time filled in), and on a phone the round button over
//! the bar and a finger held on a free stretch of the week (owner, the first question: „+ Termin im
//! Kopf der Woche, am Handy als runder Knopf über der Leiste; ein Klick auf eine freie Stelle der
//! Woche, am Handy langes Drücken"). Each opens the form (`OwnForm`), one dialog, on a phone a sheet
//! (`folia_design::modal`): what it is, when — „Jede Woche", „A-Woche", „B-Woche" or „Einmal", which
//! stand above what they change, the day or the dates (owner, 2026-10-27: „Wenn man solche Buttons
//! hat, dann ändern die nur den Inhalt unter ihnen") —, the time, the period (the lecture period
//! until it is changed), where, the calendar and the module it belongs to. Nothing in it is
//! required; what cannot be a Termin (an end before its beginning, a time or a date missing) says so
//! under the time. Under it all, what the Termin meets of the week: „Passt in deine Woche", or
//! „Trifft Ü MIT1 (Mi 09:15). Anlegen geht trotzdem." — it is firm, as a module's Termin is (the
//! third question), and Betula does not say no. While the form is open the week shows the draft
//! where it will stand, dashed (`draft_slot`).
//!
//! „Anlegen" and „Speichern" close the form at once and change the plan after the next frame (R21),
//! and a note over the page says so with „Rückgängig" (`OwnNote`), as „Löschen" in the Termin's view
//! does. Under a calendar's modules its Termine of one's own are listed (`OwnList`): a row of the
//! plan's opens it in its view (`termin=`), one of a calendar of one's own its form.

use std::collections::BTreeMap;

use folia_calendar::day::{clock, minutes, Day};
use folia_calendar::own::{OwnRhythm, OwnTermin, MAX_OWN_DATES, MAX_OWN_NUMBER, MAX_OWN_TEXT, OWN_EVENT_BASE};
use folia_calendar::rowkey::RowKey;
use folia_design::modal::{kept, Modal, ModalHead, ModalSize};
use folia_design::ui::Icon;
use folia_design::waitnote::WaitNote;
use folia_plans::studyplan::{MAX_OWN, MAX_OWN_ALL};
use folia_shell::pending::Pending;
use folia_timetable::clash::{self, Weeks};
use folia_timetable::model::Timetable;
use folia_timetable::views::{kind_short, short_title};
use folia_widgets::week::GridSlot;
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use crate::i18n::{self, Texts};
use super::PlanCtx;

/// How the note of a Termin of one's own begins (`PlanCtx::undo`, which the import, „Plan leeren"
/// and the lists share): a mark of its own, never shown.
pub(super) const OWN_NOTE: &str = "Eigener Termin: ";

/// A new Termin lasts this long (minutes) until its end is changed: a slot of the lectures.
const NEW_LENGTH: u16 = 90;

/// What the form is open with (`PlanCtx::own`): the Termin changed by its number (`None` for a new
/// one), and what its fields hold.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OwnAsk {
    pub number: Option<u32>,
    pub draft: Draft,
}

/// The fields of the form as they stand, as typed: times as `HH:MM` and dates as `YYYY-MM-DD`, as
/// their inputs hold them.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Draft {
    pub name: String,
    pub rhythm: OwnRhythm,
    /// The weekday of one that recurs, 1 = Monday.
    pub weekday: u8,
    /// The dates of one on dates of its own, one field each.
    pub dates: Vec<String>,
    pub from: String,
    pub to: String,
    /// A period of its own; `None`: the lecture period.
    pub period: Option<(String, String)>,
    pub place: String,
    /// 0 the plan's calendar, else one of one's own.
    pub calendar: u8,
    /// The module it belongs to, by id; empty for none.
    pub module: String,
    /// The days it skips, kept from the Termin changed.
    pub skip: Vec<Day>,
}

/// What keeps a draft from being a Termin: what cannot be (owner, 2026-10-18: a no needs a reason
/// that holds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Trouble {
    NoTime,
    EndBefore,
    NoDate,
    BadPeriod,
}

impl Trouble {
    fn text(self, t: &'static Texts) -> &'static str {
        let o = &t.studyplan_own;
        match self {
            Trouble::NoTime => o.no_time,
            Trouble::EndBefore => o.end_before,
            Trouble::NoDate => o.no_date,
            Trouble::BadPeriod => o.bad_period,
        }
    }
}

impl Draft {
    /// A new Termin of calendar `calendar` every week on `weekday` from `from` (minutes), a lecture
    /// slot long.
    pub(super) fn at(calendar: u8, weekday: u8, from: u16) -> Draft {
        let to = from.saturating_add(NEW_LENGTH).min(24 * 60);
        Draft {
            name: String::new(),
            rhythm: OwnRhythm::Week,
            weekday: weekday.clamp(1, 7),
            dates: Vec::new(),
            from: clock(from),
            to: clock(to),
            period: None,
            place: String::new(),
            calendar,
            module: String::new(),
            skip: Vec::new(),
        }
    }

    /// The fields of a Termin of calendar `calendar`, to change it.
    pub(super) fn of(calendar: u8, own: &OwnTermin) -> Draft {
        Draft {
            name: own.name.clone(),
            rhythm: own.rhythm,
            weekday: if own.weekday == 0 { own.dates.first().map_or(1, |day| day.weekday()) } else { own.weekday },
            dates: own.dates.iter().map(|day| day.iso()).collect(),
            from: clock(own.from),
            to: clock(own.to),
            period: own.period.map(|(first, last)| (first.iso(), last.iso())),
            place: own.place.clone(),
            calendar,
            module: own.module.clone().unwrap_or_default(),
            skip: own.skip.clone(),
        }
    }

    /// The Termin it makes, numbered `number`, or what keeps it from being one.
    pub(super) fn termin(&self, number: u32) -> Result<OwnTermin, Trouble> {
        let (Some(from), Some(to)) = (minutes(self.from.trim()), minutes(self.to.trim())) else { return Err(Trouble::NoTime) };
        if to <= from {
            return Err(Trouble::EndBefore);
        }
        let (weekday, dates) = match self.rhythm {
            OwnRhythm::Dates => {
                let mut days: Vec<Day> = self.dates.iter().filter_map(|date| Day::parse(date.trim())).collect();
                days.sort_unstable();
                days.dedup();
                days.truncate(MAX_OWN_DATES);
                if days.is_empty() {
                    return Err(Trouble::NoDate);
                }
                (0, days)
            }
            _ => (self.weekday.clamp(1, 7), Vec::new()),
        };
        let period = match (&self.period, self.rhythm.recurs()) {
            (Some((first, last)), true) => {
                let (Some(first), Some(last)) = (Day::parse(first.trim()), Day::parse(last.trim())) else { return Err(Trouble::BadPeriod) };
                if last < first {
                    return Err(Trouble::BadPeriod);
                }
                Some((first, last))
            }
            _ => None,
        };
        let name: String = self.name.trim().chars().take(MAX_OWN_TEXT).collect();
        let place: String = self.place.trim().chars().take(MAX_OWN_TEXT).collect();
        let module = Some(self.module.clone()).filter(|module| !module.is_empty());
        let skip = self.skip.iter().copied().take(MAX_OWN_DATES).collect();
        Ok(OwnTermin { number: number.clamp(1, MAX_OWN_NUMBER), rhythm: self.rhythm, weekday, dates, from, to, period, module, name, place, skip })
    }
}

/// Where the dates of one on dates of its own begin: today within the lecture period of the
/// semester shown, else its first day (of a semester to come or gone), else today.
fn first_from(today: Option<Day>, lectures: Option<(Day, Day)>) -> Day {
    match (today, lectures) {
        (Some(today), Some((first, last))) if (first..=last).contains(&today) => today,
        (_, Some((first, _))) => first,
        (Some(today), None) => today,
        (None, None) => Day(0),
    }
}

/// The form for a new Termin of calendar `calendar` on `weekday` from `from` (minutes), belonging
/// to `module` where it is given.
pub(super) fn ask_new(ctx: PlanCtx, calendar: u8, weekday: u8, from: u16, module: Option<String>) {
    let mut draft = Draft::at(calendar, weekday, from);
    draft.module = module.unwrap_or_default();
    ctx.own.set(Some(OwnAsk { number: None, draft }));
}

/// „+ Termin": the form for a new Termin of calendar `calendar` today's weekday (Monday at the
/// weekend) at 16:00, belonging to `module` where it is given.
pub(super) fn ask_today(ctx: PlanCtx, calendar: u8, module: Option<String>) {
    let weekday = ctx.today.map(Day::weekday).filter(|day| *day <= 5).unwrap_or(1);
    ask_new(ctx, calendar, weekday, 16 * 60, module);
}

/// „Bearbeiten": the form of the Termin numbered `number`.
pub(super) fn ask_edit(ctx: PlanCtx, number: u32) {
    let Some(plan) = ctx.plan else { return };
    let draft = plan.with_untracked(|doc| doc.own_termin(number).map(|(calendar, own)| Draft::of(calendar, &own.termin)));
    if let Some(draft) = draft {
        ctx.own.set(Some(OwnAsk { number: Some(number), draft }));
    }
}

/// „Löschen" in the Termin's view: the view closes, the Termin goes after the next frame, and the
/// note says so with „Rückgängig".
pub(super) fn remove(ctx: PlanCtx, number: u32) {
    let t = i18n::t();
    let Some(plan) = ctx.plan else { return };
    let before = plan.with_untracked(Clone::clone);
    let Some(name) = before.own_termin(number).map(|(calendar, own)| shown_name(&own.termin, &names_now(ctx, calendar), t)) else { return };
    ctx.undo.set(Some((format!("{OWN_NOTE}{}", (t.studyplan_own.removed)(&name)), before)));
    if let Some(going) = Pending::expect() {
        let away = ctx.url.with_untracked(|url| t.path(&url.with_termin(None).path()));
        going.go(&away, NavigateOptions { replace: true, scroll: false, ..Default::default() });
    }
    plan.update_after_paint(move |doc| {
        doc.remove_own(number);
    });
}

/// A Termin of one's own by its name; without one by the module it belongs to, as `names` call the
/// modules (the slots' names: „PBS"), else „Eigener Termin".
pub(super) fn shown_name(own: &OwnTermin, names: &BTreeMap<String, String>, t: &'static Texts) -> String {
    match (own.name.trim(), own.module.as_ref().and_then(|module| names.get(module))) {
        ("", Some(module)) => short_title(module),
        ("", None) => t.studyplan_own.unnamed.to_string(),
        (name, _) => name.to_string(),
    }
}

/// What the slots of calendar `calendar` (0 the plan's) name its modules by, for a Termin of one's
/// own without a name, read once (in a handler).
fn names_now(ctx: PlanCtx, calendar: u8) -> BTreeMap<String, String> {
    match calendar {
        0 => ctx.data.with_untracked(|data| data.as_ref().map(|data| data.slot_names()).unwrap_or_default()),
        id => ctx.calendars.with_untracked(|calendars| calendars.iter().find(|shown| shown.id == id).map(|shown| shown.titles.clone()).unwrap_or_default()),
    }
}

/// When a Termin of one's own is held, as the list says it: „Fr 13:00", „Di 17:15 · A", „Do 18:00
/// · ab 15.10.", „2 Tage · 12.11.".
pub(super) fn when_text(own: &OwnTermin, t: &'static Texts) -> String {
    let o = &t.studyplan_own;
    let day = |weekday: u8| t.data.weekday_short(i64::from(weekday)).unwrap_or_default();
    let mut parts = match own.rhythm {
        OwnRhythm::Dates => {
            let first = own.dates.first().map(|day| day.day_month(t.locale)).unwrap_or_default();
            match own.dates.len() {
                1 => vec![format!("{} {first} {}", own.dates.first().map_or("", |first| day(first.weekday())), clock(own.from))],
                n => vec![(o.days)(n), format!("{first} {}", clock(own.from))],
            }
        }
        rhythm => {
            let mut parts = vec![format!("{} {}", day(own.weekday), clock(own.from))];
            match rhythm {
                OwnRhythm::AWeek => parts.push(o.weeks_a.to_string()),
                OwnRhythm::BWeek => parts.push(o.weeks_b.to_string()),
                _ => {}
            }
            parts
        }
    };
    if let Some((first, last)) = own.period {
        parts.push(format!("{}–{}", first.day_month(t.locale), last.day_month(t.locale)));
    }
    parts.join(" · ")
}

/// The key of the Termin the form changes: its slots give way to its draft while the form is open.
pub(super) fn editing_key(ask: &OwnAsk) -> Option<RowKey> {
    ask.number.map(|number| RowKey { event: OWN_EVENT_BASE + number, fp: 0 })
}

/// The draft of the form as slots of the plan's week `week` (dashed, `draft`): where it will stand
/// once it is made, on each of its weekdays, in the weeks it is held in.
pub(super) fn draft_slots(ask: &OwnAsk, week: Weeks, t: &'static Texts) -> Vec<GridSlot> {
    let held = match ask.draft.rhythm {
        OwnRhythm::AWeek => Weeks::A,
        OwnRhythm::BWeek => Weeks::B,
        OwnRhythm::Week | OwnRhythm::Dates => Weeks::All,
    };
    if !held.in_week(week) {
        return Vec::new();
    }
    let label = match ask.draft.name.trim() {
        "" if ask.number.is_some() => t.studyplan_own.unnamed.to_string(),
        "" => t.studyplan_own.draft.to_string(),
        name => name.to_string(),
    };
    draft_days(ask)
        .into_iter()
        .map(|(day, from, to)| GridSlot {
            day,
            from,
            to,
            label: label.clone(),
            small: format!("{}–{}", clock(from), clock(to)),
            title: format!("{label} · {}–{}", clock(from), clock(to)),
            place: ask.draft.place.trim().to_string(),
            class: "draft",
            own: true,
            ..GridSlot::default()
        })
        .collect()
}

/// The draft of the form open in the week shown (`Timetable`'s semester): where it will stand, its
/// weekday and minutes, of the plan's calendar; `None` for another calendar or a draft that is no
/// Termin yet. A Termin on dates stands on the weekday of each of them.
pub(super) fn draft_days(ask: &OwnAsk) -> Vec<(u8, u16, u16)> {
    let Ok(termin) = ask.draft.termin(1) else { return Vec::new() };
    if ask.draft.calendar != 0 {
        return Vec::new();
    }
    let mut days: Vec<u8> = match termin.rhythm {
        OwnRhythm::Dates => termin.dates.iter().map(|day| day.weekday()).collect(),
        _ => vec![termin.weekday],
    };
    days.sort_unstable();
    days.dedup();
    days.into_iter().map(|day| (day, termin.from, termin.to)).collect()
}

/// What the draft meets of the plan's week, by name and when („Ü MIT1 (Mi 09:15)"), each once: the
/// timetable of the semester with the draft in it, the draft's hard overlaps.
fn meets_of(table: &Timetable, termin: &OwnTermin, names: &BTreeMap<String, String>, t: &'static Texts) -> Vec<String> {
    let Some(at) = table.events.iter().position(|event| event.own.as_ref().is_some_and(|own| own.number == termin.number)) else { return Vec::new() };
    let mut met: Vec<String> = Vec::new();
    for overlap in clash::overlaps(&table.events) {
        let other = match (overlap.a.0 == at, overlap.b.0 == at) {
            (true, _) => overlap.b,
            (_, true) => overlap.a,
            _ => continue,
        };
        let Some(event) = table.events.get(other.0) else { continue };
        let name = match &event.own {
            Some(own) => shown_name(own, names, t),
            None => {
                let module = event.modules.first().and_then(|id| names.get(id)).map_or_else(|| short_title(&event.title), |name| name.clone());
                format!("{} {module}", kind_short(event, t.locale)).trim().to_string()
            }
        };
        let day = t.data.weekday_short(i64::from(overlap.weekday)).unwrap_or_default();
        let from = event.rows.get(other.1).and_then(|row| row.from).unwrap_or(overlap.from);
        let said = format!("{name} ({day} {})", clock(from));
        if !met.contains(&said) {
            met.push(said);
        }
    }
    met
}

/// The day and the minute (down to a quarter of an hour) of a click on a free stretch of a day of
/// the week grid (`.week .col`, whose hours `data-first` and `data-last` name): not on a slot or a
/// button; `None` elsewhere.
pub(super) fn free_at(ev: &leptos::ev::MouseEvent) -> Option<(u8, u16)> {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let element = ev.target()?.dyn_into::<web_sys::Element>().ok()?;
        spot_of(&element, f64::from(ev.client_y()))
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = ev;
        None
    }
}

/// The day and the minute of the point at height `y` in the window over `element`, where it is a
/// free stretch of a day of the week grid.
#[cfg(feature = "csr")]
fn spot_of(element: &web_sys::Element, y: f64) -> Option<(u8, u16)> {
    if element.closest(".slot, .slot-acts, .slot-frame, a, button").ok().flatten().is_some() {
        return None;
    }
    let column = element.closest(".week .col").ok().flatten()?;
    let week = column.closest(".week").ok().flatten()?;
    let first: f64 = week.get_attribute("data-first")?.parse().ok()?;
    let last: f64 = week.get_attribute("data-last")?.parse().ok()?;
    let columns = week.query_selector_all(".col").ok()?;
    let day = (0..columns.length()).find(|at| columns.item(*at).is_some_and(|node| node.is_same_node(Some(column.as_ref()))))?;
    let rect = column.get_bounding_client_rect();
    let share = ((y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0);
    let minute = ((first + share * (last - first)) / 15.0).floor() * 15.0;
    let minute = minute.clamp(0.0, 24.0 * 60.0 - 15.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some((u8::try_from(day + 1).ok()?, minute as u16))
}

/// How long (ms) a finger held still on a phone's week is a hold, and how far (px) it may move.
const HOLD_MS: u64 = 500;
const HOLD_SLOP: i32 = 10;

/// A finger held on a free stretch of a phone's week (owner, the first question: „am Handy langes
/// Drücken"): after `HOLD_MS` without moving, the form opens for a Termin there; the click that ends
/// the hold opens nothing. A mouse or a pen does not hold: a click is theirs.
#[derive(Clone, Copy)]
pub(super) struct Hold {
    ctx: PlanCtx,
    at: StoredValue<Option<(i32, i32)>>,
    timer: StoredValue<Option<TimeoutHandle>>,
    held: StoredValue<bool>,
}

impl Hold {
    pub(super) fn new(ctx: PlanCtx) -> Self {
        Hold { ctx, at: StoredValue::new(None), timer: StoredValue::new(None), held: StoredValue::new(false) }
    }

    pub(super) fn down(self, ev: &leptos::ev::PointerEvent) {
        self.up();
        self.held.set_value(false);
        if ev.pointer_type() != "touch" {
            return;
        }
        let (x, y) = (ev.client_x(), ev.client_y());
        self.at.set_value(Some((x, y)));
        let hold = self;
        let timer = set_timeout_with_handle(
            move || {
                hold.timer.set_value(None);
                if hold.at.get_value().is_none() {
                    return;
                }
                if let Some((day, minute)) = spot_at(x, y) {
                    hold.held.set_value(true);
                    ask_new(hold.ctx, 0, day, minute, None);
                }
            },
            std::time::Duration::from_millis(HOLD_MS),
        );
        self.timer.set_value(timer.ok());
    }

    pub(super) fn moved(self, ev: &leptos::ev::PointerEvent) {
        if let Some((x, y)) = self.at.get_value() {
            if (ev.client_x() - x).abs() > HOLD_SLOP || (ev.client_y() - y).abs() > HOLD_SLOP {
                self.up();
            }
        }
    }

    pub(super) fn up(self) {
        self.at.set_value(None);
        if let Some(timer) = self.timer.get_value() {
            timer.clear();
            self.timer.set_value(None);
        }
    }

    /// Whether `ev` is the click that ends a hold, which opens nothing.
    pub(super) fn swallow(self, ev: &leptos::ev::MouseEvent) -> bool {
        if !self.held.get_value() {
            return false;
        }
        self.held.set_value(false);
        ev.prevent_default();
        ev.stop_propagation();
        true
    }
}

/// The day and the minute of the point `x`, `y` of the window, where it is a free stretch of a day.
fn spot_at(x: i32, y: i32) -> Option<(u8, u16)> {
    #[cfg(feature = "csr")]
    {
        let element = web_sys::window()?.document()?.element_from_point(x as f32, y as f32)?;
        spot_of(&element, f64::from(y))
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = (x, y);
        None
    }
}

/// The form of a Termin of one's own (`PlanCtx::own`): a dialog, on a phone a sheet. What it shows
/// stays while it closes (`kept`).
#[component]
pub(super) fn OwnForm(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let o = &t.studyplan_own;
    let open = Signal::derive(move || ctx.own.with(Option::is_some));
    let close = Callback::new(move |()| ctx.own.set(None));
    let shown = kept(move || ctx.own.get());
    // The field a change goes to.
    let set = move |change: Box<dyn FnOnce(&mut Draft)>| ctx.own.update(|ask| {
        if let Some(ask) = ask {
            change(&mut ask.draft);
        }
    });
    let editing = Memo::new(move |_| shown.with(|ask| ask.as_ref().and_then(|ask| ask.number)));
    let draft = Memo::new(move |_| shown.with(|ask| ask.as_ref().map(|ask| ask.draft.clone())));
    let rhythm = Memo::new(move |_| draft.with(|draft| draft.as_ref().map_or(OwnRhythm::Week, |draft| draft.rhythm)));
    let calendar = Memo::new(move |_| draft.with(|draft| draft.as_ref().map_or(0, |draft| draft.calendar)));
    let title = Memo::new(move |_| match (editing.get(), draft.with(|draft| draft.as_ref().map(|draft| draft.name.trim().to_string()))) {
        (None, _) => o.form_new.to_string(),
        (Some(_), Some(name)) if !name.is_empty() => (o.form_edit)(&name),
        (Some(_), _) => o.form_edit_unnamed.to_string(),
    });
    // The calendars it may stand in: the plan's and those of one's own.
    let calendars = Memo::new(move |_| {
        let mut all = vec![(0_u8, o.mine.to_string())];
        ctx.calendars.with(|calendars| all.extend(calendars.iter().map(|calendar| (calendar.id, calendar.name.clone()))));
        all
    });
    let sub = Memo::new(move |_| {
        let name = calendars.with(|all| all.iter().find(|(id, _)| *id == calendar.get()).map(|(_, name)| name.clone())).unwrap_or_default();
        (o.form_sub)(&name, &ctx.key.get().label(t.locale))
    });
    // The modules it may belong to: those of its calendar's week, by their titles.
    let modules = Memo::new(move |_| {
        let id = calendar.get();
        if id == 0 {
            return ctx.data.with(|data| data.as_ref().map(|data| data.ids.iter().map(|module| (module.clone(), data.modules.iter().find(|row| row.id == *module).map_or_else(|| module.clone(), |row| row.title.clone()))).collect::<Vec<_>>()).unwrap_or_default());
        }
        ctx.calendars.with(|calendars| {
            calendars.iter().find(|calendar| calendar.id == id).map(|calendar| calendar.modules.iter().map(|module| (module.clone(), calendar.full.get(module).cloned().unwrap_or_else(|| module.clone()))).collect()).unwrap_or_default()
        })
    });
    let lectures = Memo::new(move |_| ctx.table.with(|table| table.as_ref().and_then(|table| table.facts.lecture)));
    // What the draft is, or what keeps it from being one.
    let made = Memo::new(move |_| draft.with(|draft| draft.as_ref().map(|draft| draft.termin(editing.get().unwrap_or(MAX_OWN_NUMBER)))));
    // What it meets of the plan's week: the semester's timetable with the draft in it, in the place
    // of the Termin changed (the data and the selection are siblings, R16).
    let fit = Memo::new(move |_| {
        let Some(Ok(termin)) = made.get() else { return None };
        if calendar.get() != 0 {
            return Some(Err(()));
        }
        let names = ctx.data.with(|data| data.as_ref().map(|data| data.slot_names()).unwrap_or_default());
        ctx.selection.with(|(key, selection)| {
            ctx.data.with(|data| {
                let data = data.as_ref().ok().filter(|data| data.key == *key)?;
                let mut selection = selection.clone();
                selection.own.retain(|own| own.number != termin.number);
                selection.own.push(termin.clone());
                Some(Ok(meets_of(&data.timetable(&selection), &termin, &names, t)))
            })
        })
    });
    // A calendar's semester holds a hundred at most: of the plan's selection and the calendars',
    // siblings (R16).
    let full = Memo::new(move |_| {
        if editing.get().is_some() {
            return false;
        }
        let here = match calendar.get() {
            0 => ctx.selection.with(|(_, selection)| selection.own.len()),
            id => ctx.calendars.with(|calendars| calendars.iter().find(|shown| shown.id == id).map_or(0, |shown| shown.selection.own.len())),
        };
        here >= MAX_OWN
    });

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let (Some(plan), Some(ask)) = (ctx.plan, ctx.own.get_untracked()) else { return };
        let Ok(termin) = ask.draft.termin(ask.number.unwrap_or(1)) else { return };
        // All of the plan's together hold two hundred at most.
        let all = plan.with_untracked(|doc| doc.own.len() + doc.calendars.iter().map(|calendar| calendar.own.len()).sum::<usize>());
        if full.get_untracked() || (ask.number.is_none() && all >= MAX_OWN_ALL) {
            return;
        }
        let name = shown_name(&termin, &names_now(ctx, ask.draft.calendar), t);
        let key = ctx.key.get_untracked();
        let undo = ctx.undo;
        ctx.own.set(None);
        folia_design::nav::after_paint(move || {
            let (done, before) = plan.update(|doc| {
                let before = doc.clone();
                let done = match ask.number {
                    Some(number) => doc.set_own(number, ask.draft.calendar, termin),
                    None => doc.add_own(ask.draft.calendar, key, termin).is_some(),
                };
                (done, before)
            });
            if done {
                let said = if ask.number.is_some() { (o.saved)(&name) } else { (o.created)(&name) };
                let _ = undo.try_set(Some((format!("{OWN_NOTE}{said}"), before)));
            }
        });
    };

    // „Wann": what picks stands above what it changes (the day or the dates, the period).
    let when = move || {
        let pick = |value: OwnRhythm, label: &'static str| {
            let on = Memo::new(move |_| rhythm.get() == value);
            view! {
                <button type="button" role="radio" aria-checked=move || if on.get() { "true" } else { "false" } class:on=move || on.get() on:click=move |_| {
                    let from = first_from(ctx.today, lectures.get_untracked());
                    set(Box::new(move |draft| {
                        draft.rhythm = value;
                        // Dates of its own begin with the next day of its weekday.
                        if value == OwnRhythm::Dates && draft.dates.iter().all(|date| Day::parse(date).is_none()) {
                            let next = from.plus(i32::from((draft.weekday + 7 - from.weekday()) % 7));
                            draft.dates = vec![next.iso()];
                        }
                    }));
                }>{label}</button>
            }
        };
        view! {
            <div class="own-fld wide">
                <span class="own-lb" id="own-when-l">{o.when}</span>
                <div class="seg own-seg" role="radiogroup" aria-labelledby="own-when-l">
                    {pick(OwnRhythm::Week, o.every_week)}
                    {pick(OwnRhythm::AWeek, o.a_week)}
                    {pick(OwnRhythm::BWeek, o.b_week)}
                    {pick(OwnRhythm::Dates, o.on_dates)}
                </div>
            </div>
        }
    };
    // The day of one that recurs, or the dates of one that does not.
    let day_or_dates = move || match rhythm.get() {
        OwnRhythm::Dates => {
            let count = Memo::new(move |_| draft.with(|draft| draft.as_ref().map_or(1, |draft| draft.dates.len().max(1))));
            view! {
                <div class="own-fld">
                    <span class="own-lb">{o.date}</span>
                    <div class="own-dates">
                        {move || (0..count.get()).map(|at| view! {
                            <span class="own-date">
                                <input type="date" aria-label=o.date prop:value=move || draft.with(|draft| draft.as_ref().and_then(|draft| draft.dates.get(at).cloned()).unwrap_or_default())
                                    on:input=move |ev| {
                                        let value = event_target_value(&ev);
                                        set(Box::new(move |draft| {
                                            if draft.dates.len() <= at {
                                                draft.dates.resize(at + 1, String::new());
                                            }
                                            if let Some(date) = draft.dates.get_mut(at) {
                                                *date = value;
                                            }
                                        }));
                                    }/>
                                {(count.get_untracked() > 1).then(|| view! {
                                    <button class="icon-btn" type="button" title=o.remove_date aria-label=o.remove_date on:click=move |_| set(Box::new(move |draft| { if at < draft.dates.len() { draft.dates.remove(at); } }))><Icon name="x"/></button>
                                })}
                            </span>
                        }).collect_view()}
                        {move || (count.get() < MAX_OWN_DATES).then(|| view! {
                            <button class="mini" type="button" on:click=move |_| set(Box::new(|draft| { let last = draft.dates.last().and_then(|date| Day::parse(date)); draft.dates.push(last.map(|day| day.plus(7).iso()).unwrap_or_default()); }))><Icon name="plus"/>{o.add_date}</button>
                        })}
                    </div>
                </div>
            }
            .into_any()
        }
        _ => view! {
            <div class="own-fld">
                <label for="own-day">{o.day}</label>
                <select id="own-day" on:change=move |ev| { let value = event_target_value(&ev).parse::<u8>().unwrap_or(1); set(Box::new(move |draft| draft.weekday = value)); }>
                    {(1..=7_u8).map(|day| {
                        let name = t.data.weekday(i64::from(day)).unwrap_or_default();
                        view! { <option value=day.to_string() selected=move || draft.with(|draft| draft.as_ref().is_some_and(|draft| draft.weekday == day))>{name}</option> }
                    }).collect_view()}
                </select>
            </div>
        }
        .into_any(),
    };
    // The period of one that recurs: the lecture period, or one of its own.
    let period = move || {
        rhythm.get().recurs().then(|| {
            let own = Memo::new(move |_| draft.with(|draft| draft.as_ref().and_then(|draft| draft.period.clone())));
            let span = move || lectures.get().map(|(first, last)| (o.lectures)(&format!("{}–{}", first.day_month(t.locale), last.day_month(t.locale)))).unwrap_or_else(|| o.lectures_unknown.to_string());
            view! {
                <div class="own-fld">
                    <span class="own-lb">{o.period}</span>
                    {move || match own.get() {
                        None => view! {
                            <span class="own-hint">{span}" · "<button class="link" type="button" on:click=move |_| {
                                let (first, last) = lectures.get_untracked().map(|(first, last)| (first.iso(), last.iso())).unwrap_or_default();
                                set(Box::new(move |draft| draft.period = Some((first, last))));
                            }>{o.change}</button></span>
                        }
                        .into_any(),
                        Some(_) => view! {
                            <span class="own-period">
                                <span class="own-two">
                                    <input type="date" aria-label=o.from prop:value=move || own.get().map(|(first, _)| first).unwrap_or_default() on:input=move |ev| { let value = event_target_value(&ev); set(Box::new(move |draft| { if let Some(period) = draft.period.as_mut() { period.0 = value; } })); }/>
                                    <span>{o.to}</span>
                                    <input type="date" aria-label=o.to prop:value=move || own.get().map(|(_, last)| last).unwrap_or_default() on:input=move |ev| { let value = event_target_value(&ev); set(Box::new(move |draft| { if let Some(period) = draft.period.as_mut() { period.1 = value; } })); }/>
                                </span>
                                <button class="link" type="button" on:click=move |_| set(Box::new(|draft| draft.period = None))>{o.back_to_lectures}</button>
                            </span>
                        }
                        .into_any(),
                    }}
                </div>
            }
        })
    };
    // What it meets, or what keeps it from being a Termin.
    let line = move || {
        let trouble = made.with(|made| match made {
            Some(Err(trouble)) => Some(trouble.text(t)),
            _ => None,
        });
        if let Some(trouble) = trouble {
            return view! { <p class="own-fit no" role="status"><Icon name="triangle-alert"/><span>{trouble}</span></p> }.into_any();
        }
        if full.get() {
            return view! { <p class="own-fit no" role="status"><Icon name="triangle-alert"/><span>{o.full_up}</span></p> }.into_any();
        }
        match fit.get() {
            Some(Ok(met)) if met.is_empty() => view! { <p class="own-fit ok" role="status"><Icon name="check"/><span>{o.fits}</span></p> }.into_any(),
            Some(Ok(met)) => {
                let anyway = if editing.get().is_some() { o.meets_anyway_edit } else { o.meets_anyway_new };
                view! { <p class="own-fit warn" role="status"><Icon name="triangle-alert"/><span>{(o.meets)(&met.join(", "))}" "{anyway}</span></p> }.into_any()
            }
            Some(Err(())) => {
                let name = calendars.with(|all| all.iter().find(|(id, _)| *id == calendar.get()).map(|(_, name)| name.clone())).unwrap_or_default();
                view! { <p class="own-fit ok" role="status"><Icon name="users-round"/><span>{(o.in_calendar)(&name)}</span></p> }.into_any()
            }
            None => ().into_any(),
        }
    };

    view! {
        <Modal open on_close=close size=ModalSize::M class="own-form" labelled_by="own-form-title">
            // The title names the Termin as it was opened; the line follows „Kalender".
            <ModalHead id="own-form-title" title=title.get_untracked() on_close=close>
                <p>{move || sub.get()}</p>
            </ModalHead>
            <form class="st-dlg-body own-body" id="own-form" on:submit=submit>
                <div class="own-fld">
                    <label for="own-what">{o.what}</label>
                    <input id="own-what" type="text" autocomplete="off" maxlength=MAX_OWN_TEXT placeholder=o.what_placeholder prop:value=move || draft.with(|draft| draft.as_ref().map(|draft| draft.name.clone()).unwrap_or_default()) on:input=move |ev| { let value = event_target_value(&ev); set(Box::new(move |draft| draft.name = value)); }/>
                </div>
                {when}
                {day_or_dates}
                <div class="own-fld">
                    <span class="own-lb">{o.time}</span>
                    <span class="own-two">
                        <input type="time" step="300" aria-label=o.begin prop:value=move || draft.with(|draft| draft.as_ref().map(|draft| draft.from.clone()).unwrap_or_default()) on:input=move |ev| { let value = event_target_value(&ev); set(Box::new(move |draft| draft.from = value)); }/>
                        <span>{o.until}</span>
                        <input type="time" step="300" aria-label=o.end prop:value=move || draft.with(|draft| draft.as_ref().map(|draft| draft.to.clone()).unwrap_or_default()) on:input=move |ev| { let value = event_target_value(&ev); set(Box::new(move |draft| draft.to = value)); }/>
                    </span>
                </div>
                {period}
                <div class="own-fld">
                    <label for="own-where">{o.place}</label>
                    <input id="own-where" type="text" autocomplete="off" maxlength=MAX_OWN_TEXT placeholder=o.place_placeholder prop:value=move || draft.with(|draft| draft.as_ref().map(|draft| draft.place.clone()).unwrap_or_default()) on:input=move |ev| { let value = event_target_value(&ev); set(Box::new(move |draft| draft.place = value)); }/>
                </div>
                {move || (calendars.with(Vec::len) > 1).then(|| view! {
                    <div class="own-fld">
                        <label for="own-cal">{o.calendar}</label>
                        <select id="own-cal" on:change=move |ev| { let value = event_target_value(&ev).parse::<u8>().unwrap_or(0); set(Box::new(move |draft| { draft.calendar = value; draft.module = String::new(); })); }>
                            {move || calendars.get().into_iter().map(|(id, name)| view! { <option value=id.to_string() selected=move || calendar.get() == id>{name}</option> }).collect_view()}
                        </select>
                    </div>
                })}
                <div class="own-fld">
                    <label for="own-module">{o.module}</label>
                    <select id="own-module" on:change=move |ev| { let value = event_target_value(&ev); set(Box::new(move |draft| draft.module = value)); }>
                        <option value="" selected=move || draft.with(|draft| draft.as_ref().is_none_or(|draft| draft.module.is_empty()))>{o.no_module}</option>
                        {move || modules.get().into_iter().map(|(id, title)| {
                            let value = id.clone();
                            view! { <option value=value selected=move || draft.with(|draft| draft.as_ref().is_some_and(|draft| draft.module == id))>{title}</option> }
                        }).collect_view()}
                    </select>
                </div>
                {line}
            </form>
            <footer class="st-dlg-foot">
                <button class="mini" type="button" on:click=move |_| close.run(())>{o.cancel}</button>
                <button class="mini primary" type="submit" form="own-form">{move || if editing.get().is_some() { o.save } else { o.create }}</button>
            </footer>
        </Modal>
    }
}

/// The note after „Anlegen", „Speichern" and „Löschen" of a Termin of one's own, over the page
/// (`WaitNote`, as „Einplanen"'s): what happened, and „Rückgängig", which puts the plan back as it
/// was (`PlanCtx::undo`).
#[component]
pub(super) fn OwnNote(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let note = Memo::new(move |_| ctx.undo.with(|undo| undo.as_ref().and_then(|(note, _)| note.strip_prefix(OWN_NOTE).map(str::to_string))));
    // A new note is a new one: what it says comes and goes with it.
    let restore = move |_| {
        let (Some(plan), Some((_, before))) = (ctx.plan, ctx.undo.get_untracked()) else { return };
        let undo = ctx.undo;
        undo.set(None);
        plan.update_after_paint(move |doc| *doc = before);
    };
    let gone = Callback::new(move |()| {
        if note.get_untracked().is_some() {
            ctx.undo.set(None);
        }
    });
    move || {
        note.get().map(|said| {
            view! {
                <WaitNote on_gone=gone class="planning-note own-note">
                    <div class="plan-toast out">
                        <div class="plan-toast-main">
                            <span class="plan-toast-mark" aria-hidden="true"><Icon name="check"/></span>
                            <span class="plan-toast-said"><span class="plan-toast-said-name">{said}</span></span>
                        </div>
                        <div class="plan-toast-end">
                            <span class="plan-toast-sep" aria-hidden="true"></span>
                            <button class="plan-toast-undo" type="button" on:click=restore>{t.common.undo}</button>
                        </div>
                    </div>
                </WaitNote>
            }
        })
    }
}

/// A row of the list of a calendar's Termine of one's own.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct OwnRow {
    number: u32,
    key: RowKey,
    name: String,
    when: String,
    place: String,
    hue: &'static str,
}

/// „Eigene Termine" under the modules of calendar `calendar` (0 the plan's): a row per Termin of
/// the semester in the order they came, in its tone with the mark „von dir" — of the plan's a link
/// to its view, of a calendar of one's own a button to its form — and „+ Termin" with the calendar
/// chosen.
#[component]
pub(super) fn OwnList(ctx: PlanCtx, calendar: u8, #[prop(into)] hue: Signal<&'static str>) -> impl IntoView {
    let t = i18n::t();
    let o = &t.studyplan_own;
    // Of the plan's selection and of the calendar's: siblings of the data (R16).
    let rows = Memo::new(move |_| {
        // The Termine and what the calendar's slots name its modules by.
        let (held, names): (Vec<OwnTermin>, BTreeMap<String, String>) = match calendar {
            0 => (ctx.selection.with(|(_, selection)| selection.own.clone()), ctx.data.with(|data| data.as_ref().map(|data| data.slot_names()).unwrap_or_default())),
            id => ctx.calendars.with(|calendars| {
                calendars.iter().find(|shown| shown.id == id).map(|shown| (shown.selection.own.clone(), shown.titles.clone())).unwrap_or_default()
            }),
        };
        // The tones of the week's modules, by their place in its plan (the data, a sibling).
        let tones = ctx.data.with(|data| data.as_ref().map(|data| data.ids.clone()).unwrap_or_default());
        held.iter()
            .map(|own| {
                let tone = own.module.as_ref().filter(|_| calendar == 0).and_then(|module| tones.iter().position(|id| id == module)).map(super::head::tone_at);
                OwnRow { number: own.number, key: own.key(), name: shown_name(own, &names, t), when: when_text(own, t), place: own.place.clone(), hue: tone.map_or("t-own", super::head::hue) }
            })
            .collect::<Vec<_>>()
    });
    let row = move |row: OwnRow| {
        let hue = if calendar == 0 { row.hue } else { hue.get_untracked() };
        let small = view! {
            <small><span class="sp-key own" aria-hidden="true"><Icon name="user-round"/></span><span>{row.when.clone()}</span>{(!row.place.is_empty()).then(|| view! { <span>{row.place.clone()}</span> })}</small>
        };
        match calendar {
            0 => {
                let href = move || ctx.url.with(|url| t.path(&url.with_termin(Some((row.key, None))).path()));
                let current = Memo::new(move |_| ctx.url.with(|url| url.termin_on().map(|(key, _)| key) == Some(row.key)));
                view! {
                    <a class=format!("sp-row own {hue}") href=href aria-current=move || current.get().then_some("true") data-noscroll="">
                        <span class="t"><b>{row.name.clone()}</b>{small}</span>
                    </a>
                }
                .into_any()
            }
            _ => view! {
                <button class=format!("sp-row own {hue}") type="button" on:click=move |_| ask_edit(ctx, row.number)>
                    <span class="t"><b>{row.name.clone()}</b>{small}</span>
                    <Icon name="pencil"/>
                </button>
            }
            .into_any(),
        }
    };
    view! {
        <div class="sp-own" role="group" aria-label=o.heading>
            {move || rows.with(|rows| !rows.is_empty()).then(|| view! { <h3 class="sp-own-h">{o.heading}</h3> })}
            <For each=move || rows.get() key=|row| row.clone() children=row/>
            <button class="sp-own-add" type="button" title=o.add_title on:click=move |_| ask_today(ctx, calendar, None)>
                <Icon name="plus"/>{o.add}
            </button>
        </div>
    }
}

/// „+ Termin" in the week's head (owner, the first question: „„+ Termin“ im Kopf der Woche"), the
/// plan's calendar chosen.
#[component]
pub(super) fn AddOwn(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let o = &t.studyplan_own;
    view! {
        <button class="mini sp-own-head" type="button" title=o.add_title on:click=move |_| ask_today(ctx, 0, None)>
            <Icon name="plus"/>{o.add}
        </button>
    }
}

/// On a phone the round button over the bar (owner, the first question: „am Handy als runder Knopf
/// über der Leiste").
#[component]
pub(super) fn OwnFab(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let o = &t.studyplan_own;
    view! {
        <button class="sp-own-fab" type="button" title=o.add_title on:click=move |_| ask_today(ctx, 0, None)>
            <Icon name="plus"/><span>{o.add}</span>
        </button>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    #[test]
    fn dates_of_its_own_begin_in_the_semester_shown() {
        let winter = Some((d("2026-10-12"), d("2027-02-05")));
        assert_eq!(first_from(Some(d("2026-11-03")), winter), d("2026-11-03"), "today, within its lectures");
        assert_eq!(first_from(Some(d("2026-11-03")), Some((d("2027-04-12"), d("2027-07-16")))), d("2027-04-12"), "a semester to come");
        assert_eq!(first_from(Some(d("2026-09-01")), winter), d("2026-10-12"), "before its lectures");
        assert_eq!(first_from(Some(d("2026-11-03")), None), d("2026-11-03"), "no lecture period known");
    }

    #[test]
    fn a_draft_is_a_termin_where_it_can_be() {
        let draft = Draft { name: "  Chor  ".into(), place: "Aula".into(), ..Draft::at(0, 1, 18 * 60) };
        let termin = draft.termin(4).unwrap();
        assert_eq!((termin.number, termin.weekday, termin.from, termin.to, termin.name.as_str(), termin.place.as_str()), (4, 1, 1080, 1170, "Chor", "Aula"));
        assert_eq!(Draft::at(0, 3, 23 * 60).termin(1).map(|termin| termin.to), Ok(24 * 60), "up to midnight");
        assert_eq!(Draft { from: String::new(), ..draft.clone() }.termin(1), Err(Trouble::NoTime));
        assert_eq!(Draft { to: "17:00".into(), ..draft.clone() }.termin(1), Err(Trouble::EndBefore));
        let dates = Draft { rhythm: OwnRhythm::Dates, dates: vec!["2026-11-19".into(), "".into(), "2026-11-12".into(), "2026-11-12".into()], ..draft.clone() };
        assert_eq!(dates.termin(1).map(|termin| (termin.weekday, termin.dates)), Ok((0, vec![d("2026-11-12"), d("2026-11-19")])));
        assert_eq!(Draft { dates: vec![String::new()], ..dates.clone() }.termin(1), Err(Trouble::NoDate));
        let period = Draft { period: Some(("2026-10-15".into(), "2027-01-28".into())), ..draft.clone() };
        assert_eq!(period.termin(1).map(|termin| termin.period), Ok(Some((d("2026-10-15"), d("2027-01-28")))));
        assert_eq!(Draft { period: Some(("2027-01-28".into(), "2026-10-15".into())), ..draft.clone() }.termin(1), Err(Trouble::BadPeriod));
        assert_eq!(Draft { period: Some(("2027-01-28".into(), String::new())), ..draft.clone() }.termin(1), Err(Trouble::BadPeriod));
        // A period is a recurring one's alone.
        assert_eq!(Draft { period: Some(("x".into(), "y".into())), ..dates }.termin(1).map(|termin| termin.period), Ok(None));
    }

    #[test]
    fn a_termin_changed_keeps_what_its_form_does_not_show() {
        let own = OwnTermin {
            number: 7,
            rhythm: OwnRhythm::AWeek,
            weekday: 2,
            dates: Vec::new(),
            from: 17 * 60 + 15,
            to: 18 * 60 + 45,
            period: None,
            module: Some("11112".into()),
            name: "Lerngruppe".into(),
            place: "IKMZ 2.12".into(),
            skip: vec![d("2026-11-03")],
        };
        let draft = Draft::of(3, &own);
        assert_eq!((draft.calendar, draft.from.as_str(), draft.to.as_str(), draft.module.as_str()), (3, "17:15", "18:45", "11112"));
        assert_eq!(draft.termin(7), Ok(own));
    }

    #[test]
    fn the_draft_stands_on_its_days_in_the_plans_week() {
        let ask = OwnAsk { number: None, draft: Draft::at(0, 3, 16 * 60) };
        assert_eq!(draft_days(&ask), [(3, 960, 1050)]);
        let dates = OwnAsk { number: None, draft: Draft { rhythm: OwnRhythm::Dates, dates: vec!["2026-11-12".into(), "2026-11-19".into(), "2026-11-20".into()], ..Draft::at(0, 3, 16 * 60) } };
        assert_eq!(draft_days(&dates), [(4, 960, 1050), (5, 960, 1050)]);
        let lenas = OwnAsk { number: None, draft: Draft::at(1, 3, 16 * 60) };
        assert!(draft_days(&lenas).is_empty(), "another calendar's draft stands in its own");
        let broken = OwnAsk { number: None, draft: Draft { to: "15:00".into(), ..Draft::at(0, 3, 16 * 60) } };
        assert!(draft_days(&broken).is_empty());
    }
}
