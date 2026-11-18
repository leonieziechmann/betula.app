//! Termine of one's own in the Stundenplan (folia/design/kalender/termine.html, its first step;
//! owner, 2026-10-27: „Passt alles, bau Schritt 1 jetzt in Folia"): what a student adds beside what
//! QIS lists — a job, sport, a Lerngruppe, the Termin of a module QIS has none for.
//!
//! The ways in, while the plan is edited (owner, 2026-11-17: the edit mode „weil man dadurch
//! unnötiges ui ausblenden kann"): „+ Termin" in the mode's bar and on a phone in its dock, under the
//! modules of each calendar (that calendar chosen), a click on a free stretch of a day of the week
//! (its day and time filled in), a span drawn there by a drag (its day, start and end; `Drag`), and
//! on a phone a finger held on a free stretch of the week, which then stretches the span (owner, the
//! first question: „ein Klick auf eine freie Stelle der Woche, am Handy langes Drücken"); in
//! both states „Termin eintragen" at a module without Termine (that module chosen) and the empty
//! week's „+ Termin". Each opens the form (`OwnForm`), one dialog, on a phone a sheet
//! (`folia_design::modal`): what it is, the calendar, the module it belongs to and its kind („Art"),
//! when — „Jede Woche", „A-Woche", „B-Woche" or „Einmal", which
//! stand above what they change, the day or the dates (owner, 2026-10-27: „Wenn man solche Buttons
//! hat, dann ändern die nur den Inhalt unter ihnen") —, the time, the period (the lecture period
//! until it is changed), where. Nothing in it is
//! required; what cannot be a Termin (an end before its beginning, a time or a date missing) says so
//! under the time. Under it all, what the Termin meets of the week: „Passt in deine Woche", or
//! „Trifft Ü MIT1 (Mi 09:15). Anlegen geht trotzdem." — it is firm, as a module's Termin is (the
//! third question), and Betula does not say no. While the form is open the week shows the draft
//! where it will stand, dashed (`draft_slot`).
//!
//! „Anlegen" and „Speichern" close the form at once and change the plan after the next frame (R21),
//! and a note over the page says so with „Rückgängig" (`OwnNote`), as „Löschen" in the Termin's view
//! does. A Termin of a module of the week stands under its module in the list beside the week
//! (`modules.rs`, „von dir"); the others are listed after the modules of their calendar (`OwnList`):
//! a row of the plan's opens its card (`termin=`), one of a calendar of one's own its form while the
//! plan is edited. The form asks what it is first, then where it belongs — „Kalender" above
//! „Gehört zu", whose modules it changes, and „Art" —, then when and where.

use std::collections::BTreeMap;

use folia_calendar::change::Change;
use folia_calendar::day::{clock, minutes, Day};
use folia_calendar::kind::EventKind;
use folia_calendar::own::{OwnRhythm, OwnTermin, MAX_OWN_DATES, MAX_OWN_NUMBER, MAX_OWN_TEXT, OWN_EVENT_BASE};
use folia_calendar::rowkey::RowKey;
use folia_calendar::semester::SemesterKey;
use folia_design::modal::{kept, Modal, ModalHead, ModalSize};
use folia_design::haptic::{self, Haptic};
use folia_design::ui::Icon;
use folia_design::waitnote::WaitNote;
use folia_plans::studyplan::{PlanUndo, MAX_OWN, MAX_OWN_ALL};
use folia_shell::pending::Pending;
use folia_timetable::clash::{self, Weeks};
use folia_timetable::model::Timetable;
use folia_timetable::views::{kind_short, own_label, short_title};
use folia_widgets::week::GridSlot;
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use crate::i18n::{self, Texts};
use super::PlanCtx;

/// How the note of a Termin of one's own begins (`PlanCtx::undo`, which the page's other notes
/// share): a mark of its own, never shown.
pub(super) const OWN_NOTE: &str = "Eigener Termin: ";

/// How the note of a Termin of one's own deleted begins: a note that stays until it is put away
/// (`WaitNote`'s `lasting`), since what the student typed by hand is gone with its „Rückgängig".
pub(super) const OWN_GONE: &str = "Eigener Termin gelöscht: ";

/// The id of the note's „Rückgängig", where the keyboard goes when a deletion took what it was on.
const OWN_UNDO_ID: &str = "own-note-undo";

/// The latest end the form's start moves it to (minutes): 23:55, the last of its field's steps.
const LAST_END: u16 = 24 * 60 - 5;

/// A new Termin lasts this long (minutes) until its end is changed: a slot of the lectures.
const NEW_LENGTH: u16 = 90;

/// The kinds the form's „Art" offers: those a module's Termine are mostly of. A kind it does not
/// offer (one „Nachholtermin" took over from a „Sonstiges") is offered as well while it is the draft's.
const KINDS: [EventKind; 6] = [EventKind::Lecture, EventKind::Exercise, EventKind::Seminar, EventKind::Practical, EventKind::Tutorial, EventKind::Project];

/// What the form is open with (`PlanCtx::own`): the Termin changed by its number (`None` for a new
/// one), and what its fields hold.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OwnAsk {
    pub number: Option<u32>,
    pub draft: Draft,
    /// Opened by a button of the plan's calendar („+ Termin" of the bar, the dock, a module's row),
    /// not from the week: the form says quietly that a span drawn in the week is quicker (`OwnForm`).
    pub hint: bool,
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
    /// Its kind („Art"); none until one is picked.
    pub kind: Option<EventKind>,
    /// What the week's slots call its module („EvS"), not stored: the form's sub-line and the
    /// draft's slot name it so without the week handing names in. Empty without a module.
    pub module_name: String,
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
            kind: None,
            module_name: String::new(),
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
            kind: own.kind,
            module_name: String::new(),
        }
    }

    /// The start typed (`from`, as its field holds it): the end follows by the length the Termin had
    /// (09:15 for 16:00–17:30 makes 09:15–10:45), as a calendar's form does — the end left behind
    /// made „Endet vor dem Beginn" or a Termin of eight hours. No later than 23:55, the last time
    /// its field takes; an end before the start, or a time half typed, keeps the end as it is.
    pub(super) fn start_at(&mut self, from: String) {
        if let (Some(before), Some(to), Some(now)) = (minutes(self.from.trim()), minutes(self.to.trim()), minutes(from.trim())) {
            if to > before {
                self.to = clock(now.saturating_add(to - before).min(LAST_END));
            }
        }
        self.from = from;
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
        Ok(OwnTermin { number: number.clamp(1, MAX_OWN_NUMBER), rhythm: self.rhythm, weekday, dates, from, to, period, module, name, place, skip, kind: self.kind })
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
    draft.module_name = module_called(ctx, calendar, &draft.module);
    ctx.own.set(Some(OwnAsk { number: None, draft, hint: false }));
}

/// „+ Termin": the form for a new Termin of calendar `calendar` today's weekday (Monday at the
/// weekend) at 16:00, belonging to `module` where it is given.
pub(super) fn ask_today(ctx: PlanCtx, calendar: u8, module: Option<String>) {
    let weekday = ctx.today.map(Day::weekday).filter(|day| *day <= 5).unwrap_or(1);
    ask_new(ctx, calendar, weekday, 16 * 60, module);
    // A span drawn in the week is the plan's calendar's.
    if calendar == 0 {
        ctx.own.update(|ask| {
            if let Some(ask) = ask {
                ask.hint = true;
            }
        });
    }
}

/// „Bearbeiten": the form of the Termin numbered `number`.
pub(super) fn ask_edit(ctx: PlanCtx, number: u32) {
    let Some(plan) = ctx.plan else { return };
    let draft = plan.with_untracked(|doc| doc.own_termin(number).map(|(calendar, own)| Draft::of(calendar, &own.termin)));
    if let Some(mut draft) = draft {
        draft.module_name = module_called(ctx, draft.calendar, &draft.module);
        ctx.own.set(Some(OwnAsk { number: Some(number), draft, hint: false }));
    }
}

/// „Löschen" in the Termin's view and the bin on its slot while the plan is edited: the view closes
/// where one is open (the bin on a slot has none to close, and asks the router for nothing), the
/// Termin goes after the next frame, and the note says so with „Rückgängig".
pub(super) fn remove(ctx: PlanCtx, number: u32) {
    let t = i18n::t();
    let Some(plan) = ctx.plan else { return };
    let before = plan.with_untracked(Clone::clone);
    let Some(name) = before.own_termin(number).map(|(calendar, own)| said_name(&own.termin, &names_now(ctx, calendar), t)) else { return };
    let undo = PlanUndo::made(before, |doc| {
        doc.remove_own(number);
    });
    ctx.back_to.set_value(slot_in_focus());
    ctx.undo.set(Some((format!("{OWN_GONE}{}", (t.studyplan_own.removed)(&name)), undo)));
    haptic::play(Haptic::Tap);
    if let Some(going) = Pending::expect().filter(|_| ctx.url.with_untracked(|url| url.termin.is_some())) {
        let away = ctx.url.with_untracked(|url| url.with_termin(None).path());
        going.go(&away, NavigateOptions { replace: true, scroll: false, ..Default::default() });
    }
    plan.update_after_paint(move |doc| {
        doc.remove_own(number);
    });
}

/// The slot of the week the keyboard is on — by the bin on it, or the slot of the card open — as a
/// selector that finds it again once „Rückgängig" put it back: by its lines (`data-lines`), which
/// are its Termine's and come back with them, else by its address (a Termin of one's own's).
fn slot_in_focus() -> Option<String> {
    #[cfg(feature = "csr")]
    {
        let active = leptos::web_sys::window()?.document()?.active_element()?;
        let slot = match active.closest(".slot-acts").ok().flatten() {
            Some(acts) => acts.previous_element_sibling(),
            None => folia_design::stage::select(".sp-week .week a.slot[aria-current=\"true\"]"),
        }?;
        let plain = |value: &String| !value.is_empty() && !value.contains(['"', '\\']);
        if let Some(lines) = slot.get_attribute("data-lines").filter(plain) {
            return Some(format!(".sp-week .week a.slot[data-lines=\"{lines}\"]"));
        }
        let href = slot.get_attribute("href").filter(plain)?;
        Some(format!(".sp-week .week a.slot[href=\"{href}\"]"))
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// The keyboard to the first element of the page in front that `selectors` find, in their order,
/// where it is nowhere (on the page's body, or on what went): never taken from where it is.
fn focus_where_lost(selectors: &[&str]) {
    #[cfg(feature = "csr")]
    {
        use leptos::wasm_bindgen::JsCast;
        if !super::edit::lost_focus() {
            return;
        }
        let found = selectors.iter().find_map(|selector| folia_design::stage::select(selector)).and_then(|element| element.dyn_into::<leptos::web_sys::HtmlElement>().ok());
        if let Some(element) = found {
            let options = leptos::web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = element.focus_with_options(&options);
        }
    }
    #[cfg(not(feature = "csr"))]
    let _ = selectors;
}

/// A Termin of one's own as a note says it: its name in quotes, where the student wrote one; else
/// as the week calls it, bare („VL EvS gelöscht").
pub(super) fn said_name(own: &OwnTermin, names: &BTreeMap<String, String>, t: &'static Texts) -> String {
    let name = shown_name(own, names, t);
    if own.name.trim().is_empty() { name } else { (t.studyplan_own.said)(&name) }
}

/// A Termin of one's own by its name; without one by its kind and the module it belongs to, as
/// `names` call the modules (the slots' names: „VL PBS", `folia_timetable::views::own_label`), else
/// „Eigener Termin". The week's slots, the list of days, „Offen" and the notes call it so.
pub(super) fn shown_name(own: &OwnTermin, names: &BTreeMap<String, String>, t: &'static Texts) -> String {
    let module = own.module.as_ref().and_then(|module| names.get(module)).map(String::as_str);
    own_label(own, module, t.locale).unwrap_or_else(|| t.studyplan_own.unnamed.to_string())
}

/// What the slots of calendar `calendar` (0 the plan's) name its modules by, for a Termin of one's
/// own without a name, read once (in a handler).
fn names_now(ctx: PlanCtx, calendar: u8) -> BTreeMap<String, String> {
    match calendar {
        0 => ctx.data.with_untracked(|data| data.as_ref().map(|data| data.slot_names()).unwrap_or_default()),
        id => ctx.calendars.with_untracked(|calendars| calendars.iter().find(|shown| shown.id == id).map(|shown| shown.titles.clone()).unwrap_or_default()),
    }
}

/// What the form's line calls the module `module` of calendar `calendar`: as the slots do („EvS"),
/// else by its title, cut („Für Entwicklung von Softwaresy… · WiSe 2026/27") — never by its number,
/// which says nothing to a student (a module that left the week keeps its Termine of one's own).
fn module_called(ctx: PlanCtx, calendar: u8, module: &str) -> String {
    if let Some(name) = names_now(ctx, calendar).get(module).filter(|name| !name.is_empty()) {
        return name.clone();
    }
    let title = match calendar {
        0 => ctx.data.with_untracked(|data| data.as_ref().ok().and_then(|data| data.modules.iter().find(|row| row.id == module).map(|row| row.title.clone()))),
        id => ctx.calendars.with_untracked(|calendars| calendars.iter().find(|shown| shown.id == id).and_then(|shown| shown.full.get(module).cloned())),
    };
    title.map(|title| cut(&title, 28)).unwrap_or_default()
}

/// `text` within `most` letters, cut at a word with „…" where it is longer.
fn cut(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_string();
    }
    let head: String = text.chars().take(most).collect();
    let at = head.rfind(' ').filter(|at| *at > most / 2).unwrap_or(head.len());
    format!("{}…", head[..at].trim_end())
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
    // As it will be called once it is made (`shown_name`): its name, else its kind and module.
    let named = OwnTermin { number: 0, rhythm: ask.draft.rhythm, weekday: ask.draft.weekday, dates: Vec::new(), from: 0, to: 0, period: None, module: None, name: ask.draft.name.clone(), place: String::new(), skip: Vec::new(), kind: ask.draft.kind };
    let module = Some(ask.draft.module_name.as_str()).filter(|name| !name.is_empty() && !ask.draft.module.is_empty());
    let label = match own_label(&named, module, t.locale) {
        Some(label) => label,
        None if ask.number.is_some() => t.studyplan_own.unnamed.to_string(),
        None => t.studyplan_own.draft.to_string(),
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
    met_by(table, |(event, _)| event == at, names, t)
}

/// What the rows `mine` takes (by event and row) meet of `table`'s week, by name and when („Ü MIT1
/// (Mi 09:15)"), each once.
fn met_by(table: &Timetable, mine: impl Fn((usize, usize)) -> bool, names: &BTreeMap<String, String>, t: &'static Texts) -> Vec<String> {
    let mut met: Vec<String> = Vec::new();
    for overlap in clash::overlaps(&table.events) {
        let other = match (mine(overlap.a), mine(overlap.b)) {
            (true, false) => overlap.b,
            (false, true) => overlap.a,
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

/// What a change of a Termin of QIS being made meets of the plan's week (`ChangeActs::fit`): the
/// semester's timetable with it in the place of the one of its Termin and scope, what the rows it
/// makes meet; whether that is anything, and the line that says so („Trifft Ü MIT1 (Mi 09:15).
/// Speichern geht trotzdem.", „Passt in deine Woche").
pub(super) fn change_fit(ctx: PlanCtx, change: Change) -> (bool, String) {
    let t = i18n::t();
    let o = &t.studyplan_own;
    let met = ctx.selection.with_untracked(|(key, selection)| {
        ctx.data.with_untracked(|data| {
            let data = data.as_ref().ok().filter(|data| data.key == *key)?;
            let mut selection = selection.clone();
            selection.changes.retain(|other| (other.key, other.scope) != (change.key, change.scope));
            selection.changes.push(change.clone());
            let table = data.timetable(&selection);
            let mine = |(event, row): (usize, usize)| {
                let row = table.events.get(event).and_then(|event| event.rows.get(row));
                row.is_some_and(|row| row.changed.as_ref().is_some_and(|changed| changed.key == change.key && changed.scope == change.scope))
            };
            Some(met_by(&table, mine, &data.slot_names(), t))
        })
    });
    match met.unwrap_or_default() {
        met if met.is_empty() => (false, o.fits.to_string()),
        met => (true, format!("{} {}", (o.meets)(&met.join(", ")), o.meets_anyway_edit)),
    }
}

/// „Nachholtermin" of the Termin `key` on `day` (folia/design/kalender/termine.html, „Ein Tag
/// anders": „ein eigener Termin zum Modul, einmal, mit Art und Raum"): its view closes, and the form
/// opens for a Termin of one's own of the plan's calendar, of its module, on that date alone, at its
/// time and in its room, of its kind and without a name: the week calls it as its slot is („VL
/// EvS", `shown_name`), and the kind stays a word of its own (owner, 2026-11-17: „dass man zu
/// Modulen custom termine hinzufügen und auch editieren kann").
pub(super) fn ask_makeup(ctx: PlanCtx, key: RowKey, day: Day) {
    let seed = ctx.table.with_untracked(|table| {
        let event = table.as_ref()?.events.iter().find(|event| event.rows.iter().any(|row| row.key == Some(key)))?;
        let row = event.rows.iter().find(|row| row.key == Some(key) && row.from.is_some() && row.to.is_some())?;
        let module = event.modules.first()?.clone();
        // The first kind, as a „Vorlesung/Übung" is named by its first word elsewhere.
        let kind = event.kinds.iter().next();
        Some((module, kind, row.from?, row.to?, row.date.room_shown().unwrap_or_default().to_string()))
    });
    let Some((module, kind, from, to, place)) = seed else { return };
    if let Some(going) = Pending::expect() {
        let away = ctx.url.with_untracked(|url| url.with_termin(None).path());
        going.go(&away, NavigateOptions { replace: true, scroll: false, ..Default::default() });
    }
    let module_name = module_called(ctx, 0, &module);
    let draft = Draft { kind, rhythm: OwnRhythm::Dates, dates: vec![day.iso()], to: clock(to), place, module, module_name, ..Draft::at(0, day.weekday(), from) };
    ctx.own.set(Some(OwnAsk { number: None, draft, hint: false }));
}

/// The day and the minute (down to a quarter of an hour) of a click on a free stretch of a day of
/// the week grid (`.week .col`, whose hours `data-first` and `data-last` name): not on a slot or a
/// button; `None` elsewhere.
pub(super) fn free_at(ev: &leptos::ev::MouseEvent) -> Option<(u8, u16)> {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        // What the pointer is over: the week holds a mouse's pointer from its press on
        // (`Drag::down`), and the click that ends it is the week's then, not the column's.
        let element = ev.target()?.dyn_into::<web_sys::Element>().ok()?;
        let element = match element.closest(".week .col").ok().flatten() {
            Some(_) => element,
            None => web_sys::window()?.document()?.element_from_point(ev.client_x() as f32, ev.client_y() as f32)?,
        };
        let (column, _) = column_of(&element)?;
        Some((column.day, quarter(column.minute(f64::from(ev.client_y())))))
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = ev;
        None
    }
}

/// A day's column of the week grid as a press on its free stretch found it: the day, where the
/// column stands in the window (its top and height) and the minutes its hours begin and end at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Column {
    pub day: u8,
    pub top: f64,
    pub height: f64,
    pub first: u16,
    pub last: u16,
}

impl Column {
    /// The minute at the height `y` of the window, not rounded, within the grid's hours.
    pub(super) fn minute(&self, y: f64) -> f64 {
        let share = ((y - self.top) / self.height.max(1.0)).clamp(0.0, 1.0);
        f64::from(self.first) + share * f64::from(self.last.saturating_sub(self.first))
    }
}

/// A quarter of an hour, the step a span is drawn in and the least it is.
const QUARTER: u16 = 15;

/// `minute` down to its quarter of an hour, no later than the day's last quarter.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
fn quarter(minute: f64) -> u16 {
    let at = ((minute / f64::from(QUARTER)).floor() * f64::from(QUARTER)).clamp(0.0, f64::from(24 * 60 - QUARTER));
    // A whole number of minutes within the day.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let at = at as u16;
    at
}

/// The column of the week grid `element` is a free stretch of (not a slot, its buttons or a link),
/// and its element.
#[cfg(feature = "csr")]
fn column_of(element: &web_sys::Element) -> Option<(Column, web_sys::Element)> {
    if element.closest(".slot, .slot-acts, .slot-frame, a, button").ok().flatten().is_some() {
        return None;
    }
    let column = element.closest(".week .col").ok().flatten()?;
    let week = column.closest(".week").ok().flatten()?;
    let first: u16 = week.get_attribute("data-first")?.parse().ok()?;
    let last: u16 = week.get_attribute("data-last")?.parse().ok()?;
    let columns = week.query_selector_all(".col").ok()?;
    let day = (0..columns.length()).find(|at| columns.item(*at).is_some_and(|node| node.is_same_node(Some(column.as_ref()))))?;
    let rect = column.get_bounding_client_rect();
    Some((Column { day: u8::try_from(day + 1).ok()?, top: rect.top(), height: rect.height(), first, last }, column))
}

/// The span a drag on a free stretch draws, as its start and end in minutes (owner, 2026-11-18:
/// „bei den custom terminen mach die option für drag placement um gleich eine Zeitspanne zu
/// machen"): from the quarter of an hour the press began in (`anchor`) to where the pointer is
/// (`at`), in quarters of an hour — downward to the quarter the pointer is in, upward from it, the
/// quarter pressed always in it, so that a drag upward is one downward the other way round. A
/// quarter of an hour at least; never beyond the grid's hours (`first`, `last`).
pub(super) fn span_of(anchor: u16, at: f64, first: u16, last: u16) -> (u16, u16) {
    let low = first.div_ceil(QUARTER) * QUARTER;
    let high = (last / QUARTER) * QUARTER;
    if high < low + QUARTER {
        return (anchor, anchor.saturating_add(QUARTER));
    }
    let anchor = anchor.clamp(low, high - QUARTER);
    let at = at.clamp(f64::from(low), f64::from(high));
    let step = f64::from(QUARTER);
    // Whole quarters of an hour within the grid's hours.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let whole = |minute: f64| minute as u16;
    if at >= f64::from(anchor) {
        let to = whole((at / step).ceil() * step).clamp(anchor + QUARTER, high);
        (anchor, to)
    } else {
        let from = whole((at / step).floor() * step).max(low);
        (from, anchor + QUARTER)
    }
}

/// The span a finger held still draws before it moves: a new Termin's length from the quarter of
/// an hour it is held on (what the form opened with until 2026-11-18), within the grid's hours.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
pub(super) fn held_span(anchor: u16, first: u16, last: u16) -> (u16, u16) {
    let (from, least) = span_of(anchor, f64::from(anchor), first, last);
    let high = (last / QUARTER) * QUARTER;
    (from, from.saturating_add(NEW_LENGTH).min(high).max(least))
}

/// The span a finger draws with the pointer at `at`, the span `before` standing: the end goes to
/// the neighbouring quarter of an hour only once the pointer is `margin` (minutes) past the line
/// between them, and a move over more than one quarter goes there at once. A real finger resting on
/// a phone's grid trembles by a pixel or two, and a quarter of an hour is about 11 px there, so that
/// the end flipped between two quarters under a finger at rest (owner, 2026-11-18: „Ja das dragging
/// funktioniert nicht dass ist richtig buggy."). The spans `span_of` gives stand in one order as the
/// pointer goes down, in which the sum of the start and the end grows: `before` stays while it lies
/// between the spans of the pointer `margin` above and below.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
pub(super) fn steady_span(before: Option<(u16, u16)>, anchor: u16, at: f64, margin: f64, first: u16, last: u16) -> (u16, u16) {
    let now = span_of(anchor, at, first, last);
    let Some(before) = before else { return now };
    let order = |(from, to): (u16, u16)| u32::from(from) + u32::from(to);
    let (above, below) = (span_of(anchor, at - margin, first, last), span_of(anchor, at + margin, first, last));
    if order(above) <= order(before) && order(before) <= order(below) {
        before
    } else {
        now
    }
}

/// How long (ms) before the lift a change of a finger's span is the lift's own shift, and how long
/// (ms) what stood before it must have stood for the finger to have rested on it.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
const LIFT_MS: f64 = 80.0;
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
const RESTED_MS: f64 = 150.0;

/// The span a finger's lift at `lift` (ms) gives, of the spans that stood while it drew (`stood`:
/// since when each stood, in order) and what stands (`standing`). A real finger shifts by one to
/// three millimetres as it lifts, within the last few dozen milliseconds, and so the span changed
/// under a finger that had rested on what it wanted (owner, 2026-11-18: „Ja das dragging funktioniert
/// nicht dass ist richtig buggy."): where the span changed within `LIFT_MS` before the lift and what
/// stood before those changes had stood `RESTED_MS` at least, the lift gives that. A finger still
/// moving as it lifts ended a drag in motion: what stands, as its tag said.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
pub(super) fn lifted_span(stood: &[(f64, (u16, u16))], lift: f64, standing: (u16, u16)) -> (u16, u16) {
    let Some(rested) = stood.iter().rposition(|(since, _)| *since <= lift - LIFT_MS) else { return standing };
    match (stood.get(rested), stood.get(rested + 1)) {
        (Some((since, span)), Some((changed, _))) if changed - since >= RESTED_MS => *span,
        _ => standing,
    }
}

/// The span a drag draws in the week while it lasts (`PlanCtx::sketch`): its day and minutes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Sketch {
    pub day: u8,
    pub from: u16,
    pub to: u16,
}

/// A box in the window's pixels: a day's column, the week that holds the span drawn, what the
/// window shows.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Boxed {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

#[cfg_attr(not(feature = "csr"), allow(dead_code))]
impl Boxed {
    fn right(self) -> f64 {
        self.left + self.width
    }

    fn bottom(self) -> f64 {
        self.top + self.height
    }
}

/// The span being drawn as it stands over its column (`SketchView`), in pixels of the week that
/// holds it: its box, its times written in it where they fit whole (`label`), and the tag with its
/// times that stands outside it (`tag_x`, `tag_y`, `times`).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Drawn {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub label: Option<String>,
    pub tag_x: f64,
    pub tag_y: f64,
    pub times: String,
}

/// The tag's size (px), as the stylesheet draws it (`.sp-sketch-tag`), and its distance from the
/// span; further from an edge a finger is at, whose pad reaches over its point.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
const TAG_WIDTH: f64 = 96.0;
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
const TAG_HEIGHT: f64 = 24.0;
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
const TAG_GAP: f64 = 4.0;
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
const TAG_GAP_FINGER: f64 = 14.0;

/// Where the span `from`–`to` drawn in `column` (whose hours are `hours`) stands in `area` (the
/// week that holds it), the pointer at the height `pointer`, the window showing `seen`. The times
/// are always readable while a span is drawn (owner, 2026-11-18: of a phone's narrow column, where
/// a short span showed „16:00 …" alone): a tag with them stands above the span's top edge, inside
/// the week as far as it is seen; where there is no room above, below its bottom edge where the
/// pointer is at the top, else beside the column at the top edge — never where the finger is. The
/// span's own text only where it fits whole: a line of a wide column, two of a narrow one.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
pub(super) fn drawn_at(column: Boxed, area: Boxed, seen: Boxed, hours: (u16, u16), span: (u16, u16), pointer: f64) -> Drawn {
    let (first, last) = hours;
    let (from, to) = span;
    let minutes = f64::from(last.saturating_sub(first)).max(1.0);
    let at = |minute: u16| column.top + (f64::from(minute) - f64::from(first)) / minutes * column.height;
    let (top, bottom) = (at(from), at(to));
    let height = (bottom - top).max(1.0);
    // Lines of 13 px and their padding (`.sp-sketch`); a narrow column breaks „09:15 – 10:45" at its
    // dash.
    let lines = if column.width >= 92.0 { 1.0 } else { 2.0 };
    let label = (height >= lines * 13.0 + 6.0).then(|| format!("{} – {}", clock(from), clock(to)));
    let (ceiling, floor) = (area.top.max(seen.top), area.bottom().min(seen.bottom()));
    let (lo, hi) = (area.left.max(seen.left) + 2.0, area.right().min(seen.right()) - 2.0 - TAG_WIDTH);
    let inside = |x: f64| x.clamp(lo, hi.max(lo));
    let finger_at_top = (pointer - top).abs() < (pointer - bottom).abs();
    let above = top - (if finger_at_top { TAG_GAP_FINGER } else { TAG_GAP }) - TAG_HEIGHT;
    let centred = inside(column.left + column.width / 2.0 - TAG_WIDTH / 2.0);
    let (tag_x, tag_y) = if above >= ceiling {
        (centred, above)
    } else if finger_at_top && bottom + TAG_GAP + TAG_HEIGHT <= floor {
        (centred, bottom + TAG_GAP)
    } else {
        // Beside the column at the span's top edge: right of it, or left where the week ends.
        let right = column.right() + TAG_GAP;
        let x = if right <= hi { right } else { column.left - TAG_GAP - TAG_WIDTH };
        (inside(x), top.max(ceiling))
    };
    Drawn {
        x: column.left - area.left,
        y: top - area.top,
        width: column.width,
        height,
        label,
        tag_x: tag_x - area.left,
        tag_y: tag_y - area.top,
        times: format!("{}–{}", clock(from), clock(to)),
    }
}

/// The form for a new Termin of the plan's calendar on `weekday` from `from` to `to` (minutes): what
/// a drag drew.
pub(super) fn ask_span(ctx: PlanCtx, weekday: u8, from: u16, to: u16) {
    ask_new(ctx, 0, weekday, from, None);
    ctx.own.update(|ask| {
        if let Some(ask) = ask {
            ask.draft.to = clock(to);
        }
    });
}

/// How far (px) a mouse moves before its press draws a span: less is a click.
const DRAG_SLOP: f64 = 4.0;

/// How far (px, about 2.5 mm) a held finger moves from where it was held before the span follows
/// it: less is the finger's own trembling, and the 90 minutes stand. A real finger trembles and
/// drifts by a millimetre or two while it is held (owner, 2026-11-18: „Ja das dragging funktioniert
/// nicht dass ist richtig buggy."), and Android reports no move under 8 px at all, so that with a
/// smaller slop the first move it reports draws a quarter of an hour or two in place of the 90
/// minutes. A shorter span is still drawn: past this and back.
const FOLLOW_SLOP: f64 = 16.0;

/// How far (px) a finger goes past the line between two quarters of an hour before the end steps
/// over it (`steady_span`): a finger at rest trembles by a pixel or two over a phone's 11 px quarter.
const STEP_SLOP: f64 = 3.0;

/// A press on a free stretch of the week while it lasts: the pointer and whether it is a finger, the
/// column it began in, the quarter of an hour pressed, where it began, whether the span is drawn
/// (`live`), whether it follows the pointer yet (a finger's hold draws a new Termin's length
/// until the finger moves) and the span it drew when it followed last (`drew`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Press {
    pointer: i32,
    touch: bool,
    column: Column,
    anchor: u16,
    y: f64,
    live: bool,
    follows: bool,
    drew: Option<(u16, u16)>,
}

/// The time (ms) on the page's clock, which an event's `timeStamp` is on too: when a finger's hold
/// completed. A move and the lift go by their event's time, when the finger did it, not by when the
/// page came to handle it.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
fn now_ms() -> f64 {
    #[cfg(feature = "csr")]
    {
        web_sys::window().and_then(|window| window.performance()).map_or(0.0, |performance| performance.now())
    }
    #[cfg(not(feature = "csr"))]
    {
        0.0
    }
}

/// A span drawn on a free stretch of the week while the plan is edited (owner, 2026-11-18: „drag
/// placement um gleich eine Zeitspanne zu machen"): a mouse or a pen pressed and moved draws it from
/// the quarter of an hour pressed to the pointer, a finger after it was held (`Hold`); letting go
/// opens the form with the day, the start and the end, and the drawn span stays in the week as the
/// form's draft. A press that does not move is a click, which opens the form at its time as before.
/// While it is drawn the span lies over its column, not in its lanes, so that nothing under it moves
/// (`SketchView`), its times in a tag outside it (`drawn_at`).
///
/// The week holds the pointer from the press on (`setPointerCapture`), so that leaving the column,
/// or a first move already past the grid's end, does not lose it; the span stays in its day's column
/// and the grid's hours. Every way a drag can end without its pointer let go over the week gives it
/// up and leaves nothing behind — no press, no hold of the pointer, no span, no click swallowed: Esc,
/// a mouse moving with its button up (its up went to a menu or another window), the browser taking
/// the pointer (`pointercancel`, `lostpointercapture`), a second pointer coming down, the window
/// losing the focus, the mode left, another semester or week, the page turning a phone's or a wide
/// one's.
///
/// A finger is no mouse: it trembles while it rests, drifts while it is held and shifts by one to
/// three millimetres as it lifts, and the checks' finger, which stood still to the pixel, did none of
/// it (owner, 2026-11-18, of a real Android phone: „Ja das dragging funktioniert nicht dass ist
/// richtig buggy."). So for a finger the span follows only once it is `FOLLOW_SLOP` from where it was
/// held, its end steps to the next quarter of an hour only `STEP_SLOP` past the line
/// (`steady_span`), and the lift gives what stood while the finger rested, not the shift of lifting
/// (`stood`, `lifted_span`). A mouse's and a pen's drawing are as exact as their pointer.
#[derive(Clone, Copy)]
pub(super) struct Drag {
    ctx: PlanCtx,
    press: StoredValue<Option<Press>>,
    /// The click that ends a press that drew opens nothing: the form is open already, or the drag
    /// was given up while the button was still down.
    swallow: StoredValue<bool>,
    /// The span as it stands over its column, and its tag.
    drawn: RwSignal<Option<Drawn>>,
    /// The spans a finger's drag drew and since when each stood (ms, `now_ms` and the events'
    /// `timeStamp`), for its lift.
    stood: StoredValue<Vec<(f64, (u16, u16))>>,
    /// What holds the pointer (the week) and the column pressed, while a press lasts.
    #[cfg(feature = "csr")]
    held_by: StoredValue<Option<send_wrapper::SendWrapper<(web_sys::Element, web_sys::Element)>>>,
}

impl Drag {
    pub(super) fn new(ctx: PlanCtx) -> Self {
        let drag = Drag {
            ctx,
            press: StoredValue::new(None),
            swallow: StoredValue::new(false),
            drawn: RwSignal::new(None),
            stood: StoredValue::new(Vec::new()),
            #[cfg(feature = "csr")]
            held_by: StoredValue::new(None),
        };
        #[cfg(feature = "csr")]
        drag.listen();
        // What the press was made in changes: the mode, the page's layout (a phone's carousel or the
        // wide grid), the semester, the week shown, a choice in focus. Each its own effect: the
        // semester and the mode are made of the address (R16).
        let give_up = move |was: Option<bool>, now: bool| {
            if was.is_some_and(|was| was != now) {
                untrack(|| drag.cancel());
            }
            now
        };
        Effect::new(move |was: Option<bool>| give_up(was, ctx.edit.get()));
        Effect::new(move |was: Option<bool>| give_up(was, ctx.phone.get()));
        Effect::new(move |was: Option<bool>| give_up(was, ctx.url.with(|url| url.wahl.is_some())));
        Effect::new(move |was: Option<(SemesterKey, Weeks)>| {
            let now = (ctx.key.get(), ctx.weeks.get());
            if was.is_some_and(|was| was != now) {
                untrack(|| drag.cancel());
            }
            now
        });
        drag
    }

    /// Whether a press may draw: only while the plan is edited, and not with a choice in focus,
    /// whose groups are the week's boxes then.
    fn may(self) -> bool {
        self.ctx.edit.get_untracked() && self.ctx.url.with_untracked(|url| url.wahl.is_none())
    }

    /// A drag is drawing.
    pub(super) fn live(self) -> bool {
        self.press.try_get_value().flatten().is_some_and(|press| press.live)
    }

    /// A finger's drag is drawing: the page holds still under it, and its long press opens no menu.
    pub(super) fn live_touch(self) -> bool {
        self.press.try_get_value().flatten().is_some_and(|press| press.live && press.touch)
    }

    /// A pointer came down on the week. A press that never heard its pointer let go gives way, and
    /// a second pointer while one draws gives that drawing up and starts none (a finger on a
    /// laptop's screen while the mouse draws). A mouse's or a pen's press on a free stretch is
    /// noted and drawn once it moves; the week holds its pointer from now on.
    pub(super) fn down(self, ev: &leptos::ev::PointerEvent) {
        // A new press: the click of the one before is past.
        self.swallow.set_value(false);
        let another = self.press.get_value().map(|press| {
            self.cancel();
            press.pointer != ev.pointer_id()
        });
        #[cfg(feature = "csr")]
        if another != Some(true) && ev.pointer_type() != "touch" && ev.button() == 0 && self.may() {
            use wasm_bindgen::JsCast;
            let Some(element) = ev.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()) else { return };
            let Some((column, at)) = column_of(&element) else { return };
            let Some(holder) = ev.current_target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()) else { return };
            let _ = holder.set_pointer_capture(ev.pointer_id());
            self.held_by.set_value(Some(send_wrapper::SendWrapper::new((holder, at))));
            let y = f64::from(ev.client_y());
            let anchor = quarter(column.minute(y));
            self.press.set_value(Some(Press { pointer: ev.pointer_id(), touch: false, column, anchor, y, live: false, follows: false, drew: None }));
        }
        #[cfg(not(feature = "csr"))]
        let _ = (ev, another);
    }

    /// A finger held on a free stretch at `x`, `y` (`Hold`): the span of a new Termin's length is
    /// drawn there, and follows the finger once it moves. Whether it is a free stretch.
    fn held(self, pointer: i32, x: i32, y: i32) -> bool {
        #[cfg(feature = "csr")]
        {
            let Some(element) = web_sys::window().and_then(|window| window.document()).and_then(|document| document.element_from_point(x as f32, y as f32)) else { return false };
            let Some((column, at)) = column_of(&element) else { return false };
            let anchor = quarter(column.minute(f64::from(y)));
            // The week holds the finger's pointer, wherever it goes and whatever is drawn under it:
            // a phone's carousel, which the span drawn lies in (`SketchView`), not its slide.
            let Some(holder) = element.closest(".sp-carousel").ok().flatten().or_else(|| element.closest(".sp-week").ok().flatten()) else { return false };
            let _ = holder.set_pointer_capture(pointer);
            self.held_by.set_value(Some(send_wrapper::SendWrapper::new((holder, at))));
            let press = Press { pointer, touch: true, column, anchor, y: f64::from(y), live: true, follows: false, drew: None };
            self.press.set_value(Some(press));
            self.stood.set_value(Vec::new());
            self.draw(press, press.y, now_ms());
            true
        }
        #[cfg(not(feature = "csr"))]
        {
            let _ = (pointer, x, y);
            false
        }
    }

    /// A move of the pointer: whether the drag took it (then nothing else acts on it — a phone's
    /// carousel does not carry its weeks).
    pub(super) fn moved(self, ev: &leptos::ev::PointerEvent) -> bool {
        let Some(mut press) = self.press.get_value() else { return false };
        if ev.pointer_id() != press.pointer {
            return false;
        }
        // A mouse that moves with its button up was let go where the week did not hear it (a menu,
        // another window): the press is over, and nothing follows the bare pointer.
        if !press.touch && ev.buttons() & 1 == 0 {
            self.cancel();
            return false;
        }
        let y = f64::from(ev.client_y());
        if !press.follows {
            let slop = if press.live { FOLLOW_SLOP } else { DRAG_SLOP };
            if (y - press.y).abs() < slop {
                return press.live;
            }
            press.follows = true;
            press.live = true;
        }
        ev.prevent_default();
        press.drew = Some(self.draw(press, y, ev.time_stamp()));
        self.press.set_value(Some(press));
        true
    }

    /// The column of `press` where it stands now: the page may have scrolled since the press (a
    /// wheel while the mouse draws), and every minute with it.
    fn column_now(self, press: Press) -> Column {
        #[cfg(feature = "csr")]
        {
            if let Some(held) = self.held_by.get_value() {
                let rect = held.1.get_bounding_client_rect();
                if held.1.is_connected() && rect.height() > 0.0 {
                    return Column { top: rect.top(), height: rect.height(), ..press.column };
                }
            }
        }
        press.column
    }

    /// The span of `press` with the pointer at `y` at the time `when`, in the week and over its
    /// column; which it is. A finger's end steps only on a clear move (`steady_span`), and when its
    /// span changes is kept for the lift (`stood`).
    fn draw(self, press: Press, y: f64, when: f64) -> (u16, u16) {
        let column = self.column_now(press);
        let (from, to) = if !press.follows {
            held_span(press.anchor, column.first, column.last)
        } else if press.touch {
            let margin = STEP_SLOP * f64::from(column.last.saturating_sub(column.first)) / column.height.max(1.0);
            steady_span(press.drew, press.anchor, column.minute(y), margin, column.first, column.last)
        } else {
            span_of(press.anchor, column.minute(y), column.first, column.last)
        };
        let next = Some(Sketch { day: column.day, from, to });
        if self.ctx.sketch.get_untracked() != next {
            self.ctx.sketch.set(next);
            if press.touch {
                self.stood.update_value(|stood| stood.push((when, (from, to))));
            }
        }
        #[cfg(feature = "csr")]
        {
            let Some(held) = self.held_by.get_value() else { return (from, to) };
            let (holder, at) = &*held;
            let boxed = |rect: web_sys::DomRect| Boxed { left: rect.left(), top: rect.top(), width: rect.width(), height: rect.height() };
            let seen = web_sys::window().map_or(Boxed { left: 0.0, top: 0.0, width: f64::MAX, height: f64::MAX }, |window| Boxed {
                left: 0.0,
                top: 0.0,
                width: window.inner_width().ok().and_then(|width| width.as_f64()).unwrap_or(f64::MAX),
                height: window.inner_height().ok().and_then(|height| height.as_f64()).unwrap_or(f64::MAX),
            });
            let drawn = drawn_at(boxed(at.get_bounding_client_rect()), boxed(holder.get_bounding_client_rect()), seen, (column.first, column.last), (from, to), y);
            if self.drawn.get_untracked().as_ref() != Some(&drawn) {
                self.drawn.set(Some(drawn));
            }
        }
        (from, to)
    }

    /// The pointer let go: where a span was drawn, the form opens with it (and the click that
    /// follows opens nothing); a finger's with what stood while it rested (`lifted_span`). Whether
    /// a drag ended.
    pub(super) fn up(self, ev: &leptos::ev::PointerEvent) -> bool {
        let Some(press) = self.press.get_value() else { return false };
        if ev.pointer_id() != press.pointer {
            return false;
        }
        let drawn = self.ctx.sketch.get_untracked();
        let stood = self.stood.get_value();
        self.cancel();
        if !press.live {
            return false;
        }
        if let Some(sketch) = drawn {
            let (from, to) = if press.touch { lifted_span(&stood, ev.time_stamp(), (sketch.from, sketch.to)) } else { (sketch.from, sketch.to) };
            ask_span(self.ctx, sketch.day, from, to);
        }
        true
    }

    /// The browser took the pointer from the week (`lostpointercapture` at the element that holds
    /// it): its drag is given up. The same event of another element bubbles up to the week, and is
    /// none of the drag's: a finger's pointer is held by the element it came down on until the week
    /// takes it as the hold completes, and that element's loss of it comes at the finger's next
    /// event; taken for the week's, it gives the drag up whenever the finger moved before the hold
    /// completed, and the lift opens nothing.
    pub(super) fn lost(self, ev: &leptos::ev::PointerEvent) {
        if !self.press.get_value().is_some_and(|press| press.pointer == ev.pointer_id()) {
            return;
        }
        #[cfg(feature = "csr")]
        {
            use wasm_bindgen::JsCast;
            let target = ev.target().and_then(|target| target.dyn_into::<web_sys::Node>().ok());
            if let (Some(held), Some(target)) = (self.held_by.get_value(), target) {
                if !held.0.is_same_node(Some(&target)) {
                    return;
                }
            }
        }
        self.cancel();
    }

    /// The drag given up: no press, no hold of the pointer, nothing drawn, nothing opened. Safe
    /// from a listener that outlives the week.
    pub(super) fn cancel(self) {
        let press = self.press.try_get_value().flatten();
        let _ = self.press.try_set_value(None);
        let _ = self.stood.try_update_value(Vec::clear);
        // The click that ends a press that drew opens nothing (the form is open, or the drag was
        // given up); the next press clears it, so that no later click is lost.
        if press.is_some_and(|press| press.live) {
            let _ = self.swallow.try_set_value(true);
        }
        #[cfg(feature = "csr")]
        {
            if let (Some(press), Some(held)) = (press, self.held_by.try_get_value().flatten()) {
                if held.0.has_pointer_capture(press.pointer) {
                    let _ = held.0.release_pointer_capture(press.pointer);
                }
            }
            let _ = self.held_by.try_set_value(None);
        }
        #[cfg(not(feature = "csr"))]
        let _ = press;
        if self.ctx.sketch.try_with_untracked(Option::is_some) == Some(true) {
            self.ctx.sketch.set(None);
        }
        if self.drawn.try_with_untracked(Option::is_some) == Some(true) {
            self.drawn.set(None);
        }
    }

    /// Whether `ev` is the click that ends a drag, which opens nothing. A click of the keyboard
    /// (`detail` 0) ends no press.
    pub(super) fn swallow(self, ev: &leptos::ev::MouseEvent) -> bool {
        if !self.swallow.get_value() || ev.detail() == 0 {
            return false;
        }
        self.swallow.set_value(false);
        ev.prevent_default();
        ev.stop_propagation();
        true
    }

    /// The span being drawn as it stands, for `SketchView`.
    pub(super) fn drawn(self) -> Option<Drawn> {
        self.drawn.get()
    }

    /// A phone's week holds still under a finger that draws: one listener on it, not passive, there
    /// as long as the week (`node`) is, which cancels the page's scrolling only while a finger's span
    /// is drawn. A listener put on at the press itself would come too late for WebKit, which decides
    /// at the touch's start whether its moves may be cancelled.
    #[cfg(feature = "csr")]
    pub(super) fn keep_still(self, node: NodeRef<leptos::html::Div>) {
        use send_wrapper::SendWrapper;
        use wasm_bindgen::{closure::Closure, JsCast};
        let still = Closure::<dyn Fn(web_sys::Event)>::new(move |ev: web_sys::Event| {
            if self.live_touch() && ev.cancelable() {
                ev.prevent_default();
            }
        });
        let still: web_sys::js_sys::Function = still.into_js_value().unchecked_into();
        let still = SendWrapper::new(still);
        let on = StoredValue::new(None::<SendWrapper<web_sys::Element>>);
        let off = {
            let still = still.clone();
            move || {
                if let Some(element) = on.try_get_value().flatten() {
                    let _ = element.remove_event_listener_with_callback("touchmove", &still);
                }
                let _ = on.try_set_value(None);
            }
        };
        Effect::new({
            let off = off.clone();
            move |_| {
                let Some(element) = node.get() else { return };
                let element: web_sys::Element = element.into();
                if on.get_value().is_some_and(|before| before.is_same_node(Some(element.as_ref()))) {
                    return;
                }
                off();
                let options = web_sys::AddEventListenerOptions::new();
                options.set_passive(false);
                let _ = element.add_event_listener_with_callback_and_add_event_listener_options("touchmove", &still, &options);
                on.set_value(Some(SendWrapper::new(element)));
            }
        });
        on_cleanup(off);
    }

    /// Esc while drawing gives the span up, before anything else hears it (the mode would be left);
    /// so does the window losing the focus. Both taken off with the week.
    #[cfg(feature = "csr")]
    fn listen(self) {
        use wasm_bindgen::{closure::Closure, JsCast};
        let keys = Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |ev: web_sys::KeyboardEvent| {
            if ev.key() != "Escape" || !self.live() {
                return;
            }
            ev.prevent_default();
            ev.stop_propagation();
            self.cancel();
        });
        let keys: web_sys::js_sys::Function = keys.into_js_value().unchecked_into();
        let blur = Closure::<dyn Fn(web_sys::Event)>::new(move |_: web_sys::Event| self.cancel());
        let blur: web_sys::js_sys::Function = blur.into_js_value().unchecked_into();
        if let Some(window) = web_sys::window() {
            let _ = window.add_event_listener_with_callback_and_bool("keydown", &keys, true);
            let _ = window.add_event_listener_with_callback("blur", &blur);
        }
        let held = send_wrapper::SendWrapper::new((keys, blur));
        on_cleanup(move || {
            if let Some(window) = web_sys::window() {
                let _ = window.remove_event_listener_with_callback_and_bool("keydown", &held.0, true);
                let _ = window.remove_event_listener_with_callback("blur", &held.1);
            }
        });
    }
}

/// The span being drawn, over its column (`Drag`): dashed as the form's draft, its times written in
/// it where they fit whole, and the tag with its times outside it. It lies over the week, not in a
/// day's lanes, so that no slot under it changes its width while it grows; it takes a lane once it
/// is the form's draft. Gone as the pointer lets go: the form says the times then.
#[component]
pub(super) fn SketchView(drag: Drag) -> impl IntoView {
    move || {
        drag.drawn().map(|drawn| {
            let place = format!("--x:{:.1}px;--y:{:.1}px;--w:{:.1}px;--h:{:.1}px", drawn.x, drawn.y, drawn.width, drawn.height);
            let tag = format!("--x:{:.1}px;--y:{:.1}px", drawn.tag_x, drawn.tag_y);
            view! {
                <div class="sp-sketch" style=place aria-hidden="true">{drawn.label.map(|label| view! { <span class="l">{label}</span> })}</div>
                <div class="sp-sketch-tag" style=tag aria-hidden="true">{drawn.times}</div>
            }
        })
    }
}

/// How long (ms) a finger held still on a phone's week is a hold, and how far (px) it may move: as
/// far as the carousel lets it go before it carries a week (`week::DRAG`), and Android lets it go
/// before it scrolls the page itself and takes the pointer (`pointercancel`). A real finger drifts
/// while it is held (owner, 2026-11-18: „Ja das dragging funktioniert nicht dass ist richtig
/// buggy."): a slop wider than the carousel's would let a drift sideways carry the week while the
/// hold goes on, and both would have the finger.
const HOLD_MS: u64 = 500;
const HOLD_SLOP: i32 = 8;

/// A finger held on a free stretch of a phone's week (owner, 2026-11-18: „Das soll so sein, dass man
/// lange gedrückt hält, dann bekommt man ein vib feedback, und danach kann man draggen oder einfach
/// los lassen. Und erst veim los lassen wird der termin erstellt. Beim draggen in dem bereich und
/// wenn man direkt los lässt dann ein 90min termin."): after `HOLD_MS` without moving the phone
/// vibrates and the span of a new Termin's 90 minutes stands there, which the finger stretches as it
/// moves; only the lift opens the form with what stands, whose „Anlegen" makes the Termin. A finger
/// that moves before the hold completes scrolls the page. The click that ends the hold opens
/// nothing. A mouse or a pen does not hold: a click and a drag are theirs.
///
/// One of the two has the finger, the hold or a phone's carousel, never both (owner, 2026-11-18,
/// of a real finger, which drifts while it is held: „Ja das dragging funktioniert nicht dass ist
/// richtig buggy."): while the hold is `pending` the carousel carries no week; a finger that leaves
/// `HOLD_SLOP` ends the hold, and the carousel may carry or the page scroll; once the hold completed
/// the drag has the finger, and nothing is carried.
#[derive(Clone, Copy)]
pub(super) struct Hold {
    drag: Drag,
    at: StoredValue<Option<(i32, i32)>>,
    timer: StoredValue<Option<TimeoutHandle>>,
    held: StoredValue<bool>,
}

impl Hold {
    pub(super) fn new(drag: Drag) -> Self {
        Hold { drag, at: StoredValue::new(None), timer: StoredValue::new(None), held: StoredValue::new(false) }
    }

    pub(super) fn down(self, ev: &leptos::ev::PointerEvent) {
        self.up();
        self.held.set_value(false);
        // A finger's alone, and only while the plan is edited: a page for looking makes nothing of
        // a finger held, nor does a choice in focus, whose groups are the week's boxes then.
        if ev.pointer_type() != "touch" || !self.drag.may() {
            return;
        }
        let (x, y, pointer) = (ev.client_x(), ev.client_y(), ev.pointer_id());
        self.at.set_value(Some((x, y)));
        let hold = self;
        let timer = set_timeout_with_handle(
            move || {
                let _ = hold.timer.try_set_value(None);
                if hold.at.try_get_value().flatten().is_none() {
                    return;
                }
                if hold.drag.held(pointer, x, y) {
                    let _ = hold.held.try_set_value(true);
                    haptic::play(Haptic::Hold);
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

    /// A finger is held on the week and the hold has not completed: the carousel carries no week.
    pub(super) fn pending(self) -> bool {
        self.timer.get_value().is_some() && self.at.get_value().is_some()
    }

    /// A finger is held on the week, or draws: its long press opens no menu of the browser's.
    pub(super) fn pressing(self) -> bool {
        self.at.get_value().is_some() || self.drag.live_touch()
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

/// The form of a Termin of one's own (`PlanCtx::own`): a dialog, on a phone a sheet. What it shows
/// stays while it closes (`kept`).
#[component]
pub(super) fn OwnForm(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let o = &t.studyplan_own;
    // A memo: the dialog's content is built once as it opens, not again with every key typed.
    let open = Signal::from(Memo::new(move |_| ctx.own.with(Option::is_some)));
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
    // The name of its calendar: a memo of its own, beside `fit` and `full`, which read `calendar` as
    // well. The line read `calendar` after them and kept the name of the calendar before when the
    // draft went from one calendar of one's own to another (R16).
    let calendar_name = Memo::new(move |_| calendars.with(|all| all.iter().find(|(id, _)| *id == calendar.get()).map(|(_, name)| name.clone())).unwrap_or_default());
    // Opened for a module: „Für EvS · WiSe 2026/27"; else the calendar and the semester.
    let module_name = Memo::new(move |_| draft.with(|draft| draft.as_ref().filter(|draft| !draft.module.is_empty()).map(|draft| if draft.module_name.is_empty() { draft.module.clone() } else { draft.module_name.clone() })));
    let sub = Memo::new(move |_| match module_name.get() {
        Some(module) => (o.form_for)(&module, &ctx.key.get().label(t.locale)),
        None => (o.form_sub)(&calendar_name.get(), &ctx.key.get().label(t.locale)),
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
    // The kinds „Art" offers, and the draft's where the list lacks it.
    let kinds = Memo::new(move |_| {
        let mut kinds = KINDS.to_vec();
        if let Some(kind) = draft.with(|draft| draft.as_ref().and_then(|draft| draft.kind)).filter(|kind| !KINDS.contains(kind)) {
            kinds.push(kind);
        }
        kinds
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

    // The keyboard to „Was" as the form opens, not to its „Schließen" (the dialog's first button):
    // what it is is what one types first. Not on a phone, where it would bring up the screen's
    // keyboard over the sheet before anything was asked. After the frame its fields are drawn in.
    Effect::new(move |was: Option<bool>| {
        let now = open.get();
        if now && was != Some(true) && !ctx.phone.get_untracked() {
            request_animation_frame(|| {
                #[cfg(feature = "csr")]
                {
                    use leptos::wasm_bindgen::JsCast;
                    if let Some(what) = folia_design::stage::by_id("own-what").and_then(|what| what.dyn_into::<leptos::web_sys::HtmlElement>().ok()) {
                        let options = leptos::web_sys::FocusOptions::new();
                        options.set_prevent_scroll(true);
                        let _ = what.focus_with_options(&options);
                    }
                }
            });
        }
        now
    });
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let (Some(plan), Some(ask)) = (ctx.plan, ctx.own.get_untracked()) else { return };
        let Ok(termin) = ask.draft.termin(ask.number.unwrap_or(1)) else { return };
        // All of the plan's together hold two hundred at most.
        let all = plan.with_untracked(|doc| doc.own.len() + doc.calendars.iter().map(|calendar| calendar.own.len()).sum::<usize>());
        if full.get_untracked() || (ask.number.is_none() && all >= MAX_OWN_ALL) {
            haptic::play(Haptic::Error);
            return;
        }
        // Entered: felt as done the moment the form goes.
        haptic::play(Haptic::Success);
        let name = said_name(&termin, &names_now(ctx, ask.draft.calendar), t);
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
                (done, PlanUndo::of(before, doc.clone()))
            });
            if done {
                let said = if ask.number.is_some() { (o.saved)(&name) } else { (o.created)(&name) };
                let _ = undo.try_set(Some((format!("{OWN_NOTE}{said}"), before)));
            }
        });
    };

    // Opened by a button while the plan is edited: one quiet line by „Wann" that a span drawn in the
    // week is quicker (the gesture cannot be seen otherwise, on a phone least); none where the form
    // came from the week.
    let quicker = Memo::new(move |_| shown.with(|ask| ask.as_ref().is_some_and(|ask| ask.hint)) && ctx.edit.get());
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
                {move || quicker.get().then(|| view! { <p class="own-quicker">{move || if ctx.phone.get() { o.quicker_hold } else { o.quicker_drag }}</p> })}
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
                                    <button class="icon-btn" type="button" title=o.remove_date aria-label=o.remove_date on:click=move |_| set(Box::new(move |draft| { if at < draft.dates.len() { draft.dates.remove(at); } }))><Icon name="trash-2"/></button>
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
            Some(Err(())) => view! { <p class="own-fit ok" role="status"><Icon name="users-round"/><span>{(o.in_calendar)(&calendar_name.get())}</span></p> }.into_any(),
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
                // What it is, then where it belongs: „Kalender" above „Gehört zu", whose modules it
                // changes, and „Art" under the module (owner, 2026-10-27: „Wenn man
                // solche Buttons hat, dann ändern die nur den Inhalt unter ihnen"); then when and
                // where. With a module the name may stay empty: the week calls it „VL EvS".
                <div class="own-fld">
                    <label for="own-what">{o.what}</label>
                    <input id="own-what" type="text" autocomplete="off" maxlength=MAX_OWN_TEXT placeholder=move || if module_name.with(Option::is_some) { o.what_optional } else { o.what_placeholder } prop:value=move || draft.with(|draft| draft.as_ref().map(|draft| draft.name.clone()).unwrap_or_default()) on:input=move |ev| { let value = event_target_value(&ev); set(Box::new(move |draft| draft.name = value)); }/>
                </div>
                {move || (calendars.with(Vec::len) > 1).then(|| view! {
                    <div class="own-fld">
                        <label for="own-cal">{o.calendar}</label>
                        <select id="own-cal" on:change=move |ev| { let value = event_target_value(&ev).parse::<u8>().unwrap_or(0); set(Box::new(move |draft| { draft.calendar = value; draft.module = String::new(); draft.module_name = String::new(); })); }>
                            {move || calendars.get().into_iter().map(|(id, name)| view! { <option value=id.to_string() selected=move || calendar.get() == id>{name}</option> }).collect_view()}
                        </select>
                    </div>
                })}
                <div class="own-fld">
                    <label for="own-module">{o.module}</label>
                    <select id="own-module" on:change=move |ev| {
                        let value = event_target_value(&ev);
                        let name = module_called(ctx, calendar.get_untracked(), &value);
                        set(Box::new(move |draft| { draft.module = value; draft.module_name = name; }));
                    }>
                        <option value="" selected=move || draft.with(|draft| draft.as_ref().is_none_or(|draft| draft.module.is_empty()))>{o.no_module}</option>
                        {move || modules.get().into_iter().map(|(id, title)| {
                            let value = id.clone();
                            view! { <option value=value selected=move || draft.with(|draft| draft.as_ref().is_some_and(|draft| draft.module == id))>{title}</option> }
                        }).collect_view()}
                    </select>
                </div>
                // Its kind, a word about it (owner, 2026-11-17: „dass man zu Modulen custom termine
                // hinzufügen … kann"): with its module the week calls it „VL EvS"; it hides nothing.
                <div class="own-fld">
                    <label for="own-kind">{o.kind}</label>
                    <select id="own-kind" on:change=move |ev| { let value = EventKind::from_code(&event_target_value(&ev)); set(Box::new(move |draft| draft.kind = value)); }>
                        <option value="" selected=move || draft.with(|draft| draft.as_ref().is_none_or(|draft| draft.kind.is_none()))>{o.no_kind}</option>
                        {move || kinds.get().into_iter().map(|kind| view! {
                            <option value=kind.code() selected=move || draft.with(|draft| draft.as_ref().is_some_and(|draft| draft.kind == Some(kind)))>{kind.label(t.locale)}</option>
                        }).collect_view()}
                    </select>
                </div>
                {when}
                {day_or_dates}
                <div class="own-fld">
                    <span class="own-lb">{o.time}</span>
                    <span class="own-two">
                        <input type="time" step="300" aria-label=o.begin prop:value=move || draft.with(|draft| draft.as_ref().map(|draft| draft.from.clone()).unwrap_or_default()) on:input=move |ev| { let value = event_target_value(&ev); set(Box::new(move |draft| draft.start_at(value))); }/>
                        <span>{o.until}</span>
                        <input type="time" step="300" aria-label=o.end prop:value=move || draft.with(|draft| draft.as_ref().map(|draft| draft.to.clone()).unwrap_or_default()) on:input=move |ev| { let value = event_target_value(&ev); set(Box::new(move |draft| draft.to = value)); }/>
                    </span>
                </div>
                {period}
                <div class="own-fld">
                    <label for="own-where">{o.place}</label>
                    <input id="own-where" type="text" autocomplete="off" maxlength=MAX_OWN_TEXT placeholder=o.place_placeholder prop:value=move || draft.with(|draft| draft.as_ref().map(|draft| draft.place.clone()).unwrap_or_default()) on:input=move |ev| { let value = event_target_value(&ev); set(Box::new(move |draft| draft.place = value)); }/>
                </div>
                {line}
            </form>
            <footer class="st-dlg-foot">
                // A Termin of a calendar of one's own has no card yet (its slot opens nothing): its
                // form is where it is deleted, with the bin (owner, 2026-11-17: „da wo es entfernt
                // eine Mülltonne"), the note and „Rückgängig". The plan's own are deleted in their
                // card and by the bin on their slot.
                {move || editing.get().filter(|_| calendar.get() != 0).map(|number| view! {
                    <button class="mini own-delete" type="button" data-action="own-delete" on:click=move |_| { close.run(()); remove(ctx, number); }>
                        <Icon name="trash-2"/>{o.delete}
                    </button>
                })}
                <button class="mini" type="button" on:click=move |_| close.run(())>{o.cancel}</button>
                <button class="mini primary" type="submit" form="own-form">{move || if editing.get().is_some() { o.save } else { o.create }}</button>
            </footer>
        </Modal>
    }
}

/// The note after „Anlegen", „Speichern" and „Löschen" of a Termin of one's own, over the page
/// (`WaitNote`, as „Einplanen"'s): what happened, and „Rückgängig", which takes back what was done
/// and nothing that was changed meanwhile elsewhere (`PlanCtx::undo`, `PlanUndo`).
#[component]
pub(super) fn OwnNote(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    // What it says, and whether it is a deletion's, which stays until it is put away.
    let note = Memo::new(move |_| {
        ctx.undo.with(|undo| {
            let (note, _) = undo.as_ref()?;
            note.strip_prefix(OWN_GONE).map(|said| (said.to_string(), true)).or_else(|| note.strip_prefix(OWN_NOTE).map(|said| (said.to_string(), false)))
        })
    });
    // A new note is a new one: what it says comes and goes with it. What a deletion took comes
    // back, and the keyboard with it where the note held it (the slot it stood on, else the week).
    let restore = move |_| {
        let (Some(plan), Some((_, before))) = (ctx.plan, ctx.undo.get_untracked()) else { return };
        let undo = ctx.undo;
        undo.set(None);
        plan.update_after_paint(move |doc| before.restore(doc));
        let back = ctx.back_to.get_value();
        folia_design::nav::after_paint(move || {
            request_animation_frame(move || {
                let back = back.as_deref().into_iter().chain([".sp-week .week a.slot"]).collect::<Vec<_>>();
                focus_where_lost(&back);
            });
        });
    };
    move || {
        note.get().map(|(said, lasting)| {
            // The keyboard was on what the deletion took (the bin, the card's „Löschen", a form
            // that closed): it comes to „Rückgängig", never to the page's body — once the Termin
            // has gone from the week, and once a card or form has closed.
            if lasting {
                folia_design::nav::after_paint(|| request_animation_frame(|| focus_where_lost(&[&format!("#{OWN_UNDO_ID}")])));
                set_timeout(|| focus_where_lost(&[&format!("#{OWN_UNDO_ID}")]), std::time::Duration::from_millis(350));
            }
            // A note that leaves lets go of itself alone, not of a newer one that took its place
            // (the next card's, within the 200 ms the old one takes to go).
            let mine = said.clone();
            let gone = Callback::new(move |()| {
                if note.with_untracked(|now| now.as_ref().map(|(now, _)| now.as_str()) == Some(mine.as_str())) {
                    ctx.undo.set(None);
                }
            });
            view! {
                <WaitNote on_gone=gone lasting class="planning-note own-note">
                    <div class="plan-toast out">
                        <div class="plan-toast-main">
                            <span class="plan-toast-mark" aria-hidden="true"><Icon name="check"/></span>
                            <span class="plan-toast-said"><span class="plan-toast-said-name">{said}</span></span>
                        </div>
                        <div class="plan-toast-end">
                            <span class="plan-toast-sep" aria-hidden="true"></span>
                            <button data-haptic="tap" class="plan-toast-undo" id=OWN_UNDO_ID type="button" on:click=restore>{t.common.undo}</button>
                        </div>
                    </div>
                </WaitNote>
            }
        })
    }
}

/// A row of a Termin of one's own in the list of modules: under its module, or among those of no
/// module of the week („Eigene Termine").
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct OwnRow {
    pub number: u32,
    pub key: RowKey,
    /// What the week calls it (`shown_name`): „VL EvS", „Chor".
    pub name: String,
    /// What a row under its module calls it: its name, else its kind („Vorlesung"), else „Termin".
    pub label: String,
    pub when: String,
    pub place: String,
    pub hue: &'static str,
    /// The module it belongs to, by id.
    pub module: Option<String>,
}

/// The rows of the Termine of one's own `held`, in their order. `names`: what the calendar's slots
/// call its modules; `tones`: the modules of the plan's week in plan order (empty for a calendar of
/// one's own), whose tone a Termin of one of them takes.
pub(super) fn own_rows(held: &[OwnTermin], names: &BTreeMap<String, String>, tones: &[String], t: &'static Texts) -> Vec<OwnRow> {
    held.iter()
        .map(|own| {
            let tone = own.module.as_ref().and_then(|module| tones.iter().position(|id| id == module)).map(super::head::tone_at);
            let label = match (own.name.trim(), own.kind) {
                ("", Some(kind)) => kind.label(t.locale).to_string(),
                ("", None) => t.studyplan_own.no_kind.to_string(),
                (name, _) => name.to_string(),
            };
            OwnRow {
                number: own.number,
                key: own.key(),
                name: shown_name(own, names, t),
                label,
                when: when_text(own, t),
                place: own.place.clone(),
                hue: tone.map_or("t-own", super::head::hue),
                module: own.module.clone(),
            }
        })
        .collect()
}

/// The rows of the plan's Termine of one's own that stand under no module of its week (`week`): of
/// none, or of a module that left it.
pub(super) fn of_no_module(rows: Vec<OwnRow>, week: &[String]) -> Vec<OwnRow> {
    rows.into_iter().filter(|row| row.module.as_ref().is_none_or(|module| !week.contains(module))).collect()
}

/// „Eigene Termine" under the modules of calendar `calendar` (0 the plan's): a row per Termin of
/// the semester in the order they came, in its tone with the mark „von dir". Of the plan's only
/// those of no module of its week (`of_no_module`: a Termin of a module stands under its module,
/// `modules.rs`), each a link to its card; of a calendar of one's own all of them, each a button to
/// its form in both states. While the plan is edited „+ Termin" with the calendar chosen: a
/// calendar's own always, the plan's only under the head of its rows.
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
        let tones = match calendar {
            0 => ctx.data.with(|data| data.as_ref().map(|data| data.ids.clone()).unwrap_or_default()),
            _ => Vec::new(),
        };
        let rows = own_rows(&held, &names, &tones, t);
        match calendar {
            0 => of_no_module(rows, &tones),
            _ => rows,
        }
    });
    let row = move |row: OwnRow| {
        let hue = if calendar == 0 { row.hue } else { hue.get_untracked() };
        let (when, place) = (row.when.clone(), row.place.clone());
        let small = move || {
            let place = (!place.is_empty()).then(|| view! { <span>{place.clone()}</span> });
            view! { <small><span class="sp-key own" aria-hidden="true"><Icon name="user-round"/></span><span>{when.clone()}</span>{place}</small> }
        };
        match calendar {
            0 => {
                let href = move || ctx.url.with(|url| t.path(&url.with_termin(Some((row.key, None))).path()));
                let current = Memo::new(move |_| ctx.url.with(|url| url.termin_on().map(|(key, _)| key) == Some(row.key)));
                view! {
                    <a class=format!("sp-row own {hue}") href=href aria-current=move || current.get().then_some("true") data-noscroll="">
                        <span class="t"><b>{row.name.clone()}</b>{small()}</span>
                    </a>
                }
                .into_any()
            }
            // A calendar of one's own has no card for its slots yet: its form, in both states — it
            // is one's own wherever it stands, changed and deleted there.
            _ => {
                let (name, number) = (row.name.clone(), row.number);
                view! {
                    <button class=format!("sp-row own {hue}") type="button" data-action="own-edit" on:click=move |_| ask_edit(ctx, number)>
                        <span class="t"><b>{name}</b>{small()}</span>
                        <Icon name="pencil"/>
                    </button>
                }
                .into_any()
            }
        }
    };
    // „+ Termin" while editing: a calendar of one's own has its own, named for it; the plan's
    // stands only under the head of its Termine of no module (the mode's bar and the dock hold the
    // plan's „+ Termin", and each module its own).
    let calendar_name = Memo::new(move |_| ctx.calendars.with(|calendars| calendars.iter().find(|shown| shown.id == calendar).map(|shown| shown.name.clone()).unwrap_or_default()));
    let any = Memo::new(move |_| rows.with(|rows| !rows.is_empty()));
    view! {
        <div class="sp-own" role="group" aria-label=o.heading>
            {move || any.get().then(|| view! { <h3 class="sp-own-h">{o.heading}</h3> })}
            <For each=move || rows.get() key=|row| row.clone() children=row/>
            {move || (ctx.edit.get() && (calendar != 0 || any.get())).then(|| {
                let name = match calendar {
                    0 => o.add_title.to_string(),
                    _ => (o.add_in)(&calendar_name.get()),
                };
                view! {
                    <button class="sp-own-add" type="button" data-action="own-add" title=name.clone() aria-label=name on:click=move |_| ask_today(ctx, calendar, None)>
                        <Icon name="plus"/>{o.add}
                    </button>
                }
            })}
        </div>
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
    fn the_end_follows_the_start_by_the_length_it_had() {
        let started = |draft: &Draft, from: &str| {
            let mut draft = draft.clone();
            draft.start_at(from.into());
            (draft.from, draft.to)
        };
        let new = Draft::at(0, 1, 16 * 60);
        assert_eq!((new.from.as_str(), new.to.as_str()), ("16:00", "17:30"));
        assert_eq!(started(&new, "09:15"), ("09:15".into(), "10:45".into()), "earlier: the end comes along");
        assert_eq!(started(&new, "17:00"), ("17:00".into(), "18:30".into()), "later than the old end");
        assert_eq!(started(&new, "23:00"), ("23:00".into(), "23:55".into()), "no later than the field's last time");
        assert_eq!(started(&new, ""), (String::new(), "17:30".into()), "a field emptied keeps the end");
        let backwards = Draft { to: "15:00".into(), ..new.clone() };
        assert_eq!(started(&backwards, "09:00"), ("09:00".into(), "15:00".into()), "no length to keep");
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

    /// Without a name a Termin of one's own goes by its kind and its module, as a slot of QIS does.
    #[test]
    fn a_termin_of_ones_own_is_called_by_its_kind_and_its_module() {
        let names: BTreeMap<String, String> = [("12104".to_string(), "EvS".to_string())].into();
        let own = Draft { module: "12104".into(), kind: Some(EventKind::Lecture), ..Draft::at(0, 5, 9 * 60) }.termin(1).unwrap();
        assert_eq!(shown_name(&own, &names, &i18n::DE), "VL EvS");
        assert_eq!(shown_name(&OwnTermin { kind: None, ..own.clone() }, &names, &i18n::DE), "EvS");
        assert_eq!(shown_name(&OwnTermin { module: None, ..own.clone() }, &names, &i18n::DE), "Vorlesung");
        assert_eq!(shown_name(&OwnTermin { name: "Tafelübung".into(), ..own.clone() }, &names, &i18n::DE), "Tafelübung");
        assert_eq!(shown_name(&OwnTermin { module: None, kind: None, ..own }, &names, &i18n::DE), "Eigener Termin");
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
            kind: Some(EventKind::Exercise),
        };
        let draft = Draft::of(3, &own);
        assert_eq!((draft.calendar, draft.from.as_str(), draft.to.as_str(), draft.module.as_str()), (3, "17:15", "18:45", "11112"));
        assert_eq!(draft.termin(7), Ok(own));
    }

    #[test]
    fn the_draft_stands_on_its_days_in_the_plans_week() {
        let ask = OwnAsk { number: None, draft: Draft::at(0, 3, 16 * 60), hint: false };
        assert_eq!(draft_days(&ask), [(3, 960, 1050)]);
        let dates = OwnAsk { number: None, draft: Draft { rhythm: OwnRhythm::Dates, dates: vec!["2026-11-12".into(), "2026-11-19".into(), "2026-11-20".into()], ..Draft::at(0, 3, 16 * 60) }, hint: false };
        assert_eq!(draft_days(&dates), [(4, 960, 1050), (5, 960, 1050)]);
        let lenas = OwnAsk { number: None, draft: Draft::at(1, 3, 16 * 60), hint: false };
        assert!(draft_days(&lenas).is_empty(), "another calendar's draft stands in its own");
        let broken = OwnAsk { number: None, draft: Draft { to: "15:00".into(), ..Draft::at(0, 3, 16 * 60) }, hint: false };
        assert!(draft_days(&broken).is_empty());
    }

    /// The draft stands in the week as the Termin will be called: its name, else its kind and its
    /// module as the slots call it, else „Neuer Termin".
    #[test]
    fn the_draft_is_named_as_its_termin_will_be() {
        let ask = |draft: Draft| OwnAsk { number: None, draft, hint: false };
        let evs = Draft { module: "12104".into(), module_name: "EvS".into(), kind: Some(EventKind::Lecture), ..Draft::at(0, 2, 9 * 60 + 15) };
        let label = |ask: &OwnAsk| draft_slots(ask, Weeks::All, &i18n::DE).first().map(|slot| slot.label.clone()).unwrap();
        assert_eq!(label(&ask(evs.clone())), "VL EvS");
        assert_eq!(label(&ask(Draft { kind: None, ..evs.clone() })), "EvS");
        assert_eq!(label(&ask(Draft { name: "Tafelübung".into(), ..evs.clone() })), "Tafelübung");
        assert_eq!(label(&ask(Draft::at(0, 2, 600))), "Neuer Termin");
        assert_eq!(label(&OwnAsk { number: Some(3), draft: Draft::at(0, 2, 600), hint: false }), "Eigener Termin");
        // Its module's name is not stored.
        assert_eq!(evs.termin(1).map(|termin| termin.module), Ok(Some("12104".to_string())));
    }

    /// A row of the list calls a Termin by its name, else its kind, else „Termin", in its module's
    /// tone where its module is in the week.
    #[test]
    fn the_rows_of_termine_of_ones_own() {
        let names: BTreeMap<String, String> = [("12104".to_string(), "EvS".to_string())].into();
        let tones = vec!["11112".to_string(), "12104".to_string()];
        let lecture = Draft { module: "12104".into(), kind: Some(EventKind::Lecture), ..Draft::at(0, 2, 9 * 60 + 15) }.termin(1).unwrap();
        let choir = Draft { name: "Chor".into(), ..Draft::at(0, 5, 18 * 60) }.termin(2).unwrap();
        let bare = Draft::at(0, 3, 600).termin(3).unwrap();
        let rows = own_rows(&[lecture, choir, bare], &names, &tones, &i18n::DE);
        let shown: Vec<(&str, &str, &str, &str, Option<&str>)> = rows.iter().map(|row| (row.name.as_str(), row.label.as_str(), row.when.as_str(), row.hue, row.module.as_deref())).collect();
        assert_eq!(
            shown,
            [
                ("VL EvS", "Vorlesung", "Di 09:15", "t-sun", Some("12104")),
                ("Chor", "Chor", "Fr 18:00", "t-own", None),
                ("Eigener Termin", "Termin", "Mi 10:00", "t-own", None),
            ]
        );
        // „Eigene Termine" of the plan holds those of no module of the week: the lecture stands
        // under EvS; once EvS left the week it stands there.
        let loose = |week: &[String]| of_no_module(rows.clone(), week).iter().map(|row| row.number).collect::<Vec<_>>();
        assert_eq!(loose(&tones), [2, 3]);
        assert_eq!(loose(&["11112".to_string()]), [1, 2, 3]);
    }

    #[test]
    fn a_long_title_is_cut_at_a_word() {
        assert_eq!(cut("Entwicklung von Softwaresystemen", 28), "Entwicklung von…");
        assert_eq!(cut("Mathematik IT-1", 28), "Mathematik IT-1");
        assert_eq!(cut("Elektrotechnikgrundlagenvorlesung", 10), "Elektrotec…");
    }

    /// A drag from the quarter of an hour pressed to the pointer, in quarters of an hour: down to the
    /// quarter the pointer is in, up from it, the quarter pressed always in it; a quarter at least;
    /// never past the grid's hours.
    #[test]
    fn a_drag_draws_its_span_in_quarters_of_an_hour() {
        // The grid from 07:00 to 19:00; pressed at 09:20 (the quarter of 09:15).
        let (first, last) = (7 * 60, 19 * 60);
        let anchor = quarter(9.0 * 60.0 + 20.0);
        assert_eq!(anchor, 555);
        // Down to 10:40: 09:15–10:45.
        assert_eq!(span_of(anchor, 640.0, first, last), (555, 645));
        // No further than the press: a quarter of an hour.
        assert_eq!(span_of(anchor, 557.0, first, last), (555, 570));
        assert_eq!(span_of(anchor, 555.0, first, last), (555, 570));
        // Up to 08:05: 08:00–09:30, the quarter pressed in it.
        assert_eq!(span_of(anchor, 485.0, first, last), (480, 570));
        // Past the grid's ends: its first and last hour.
        assert_eq!(span_of(anchor, 2000.0, first, last), (555, 1140));
        assert_eq!(span_of(anchor, -40.0, first, last), (420, 570));
        // Pressed in the last quarter: it stays a quarter within the grid.
        assert_eq!(span_of(1135, 1139.0, first, last), (1125, 1140));
        // A grid whose hours begin between quarters: from its first whole quarter.
        assert_eq!(span_of(450, 400.0, 437, 1140), (450, 465));
    }

    /// A finger held draws a new Termin's length before it moves, within the grid's hours; the
    /// column turns a height of the window into a minute.
    #[test]
    fn a_finger_held_draws_a_new_termins_length() {
        assert_eq!(held_span(930, 420, 1140), (930, 1020));
        assert_eq!(held_span(1110, 420, 1140), (1110, 1140));
        let column = Column { day: 5, top: 100.0, height: 720.0, first: 420, last: 1140 };
        assert!((column.minute(100.0) - 420.0).abs() < 1e-9);
        assert!((column.minute(460.0) - 780.0).abs() < 1e-9);
        assert!((column.minute(5000.0) - 1140.0).abs() < 1e-9);
        assert_eq!(quarter(column.minute(461.0)), 780);
    }

    /// The span being drawn lies over its column; its times stand in a tag outside it, above its
    /// top edge, inside the week and the window — on a phone's last column too —, below or beside it
    /// where there is no room above, never where the pointer is; its own text only where it fits.
    #[test]
    fn a_fingers_end_steps_only_on_a_clear_move() {
        // The grid from 07:00 to 19:00, pressed at 16:05 (the quarter of 16:00); 4 minutes are about
        // 3 px of a phone's 10.8 px quarter.
        let (first, last, anchor, margin) = (7 * 60, 19 * 60, 960, 4.0);
        let to_1715 = Some((960, 1035));
        // Resting 2 minutes past the line of 17:15 and trembling: the end stays.
        assert_eq!(steady_span(to_1715, anchor, 1037.0, margin, first, last), (960, 1035));
        assert_eq!(steady_span(to_1715, anchor, 1033.0, margin, first, last), (960, 1035));
        // Clearly past it: the next quarter, and back clearly before it.
        assert_eq!(steady_span(to_1715, anchor, 1040.0, margin, first, last), (960, 1050));
        assert_eq!(steady_span(Some((960, 1050)), anchor, 1037.0, margin, first, last), (960, 1050));
        assert_eq!(steady_span(Some((960, 1050)), anchor, 1030.0, margin, first, last), (960, 1035));
        // A move over more than one quarter goes there at once, either way.
        assert_eq!(steady_span(to_1715, anchor, 1062.0, margin, first, last), (960, 1065));
        assert_eq!(steady_span(to_1715, anchor, 1005.0, margin, first, last), (960, 1005));
        // Upward the start steps alike: 15:30–16:15 stays at 15:28, goes at 15:25.
        assert_eq!(steady_span(Some((930, 975)), anchor, 928.0, margin, first, last), (930, 975));
        assert_eq!(steady_span(Some((930, 975)), anchor, 925.0, margin, first, last), (915, 975));
        // Nothing before: where the pointer is.
        assert_eq!(steady_span(None, anchor, 1037.0, margin, first, last), (960, 1050));
        // No margin: as a mouse draws.
        assert_eq!(steady_span(to_1715, anchor, 1037.0, 0.0, first, last), span_of(anchor, 1037.0, first, last));
    }

    #[test]
    fn a_fingers_lift_gives_what_stood_while_it_rested() {
        let (held, a, b, c) = ((960, 1050), (960, 1035), (960, 1050), (960, 1065));
        // Rested on 16:00–17:15 for 300 ms, then a jump within the lift's 30 ms: the rested span.
        assert_eq!(lifted_span(&[(0.0, held), (100.0, a), (400.0, b)], 430.0, b), a);
        // Two jumps inside the last 80 ms: walked back over both.
        assert_eq!(lifted_span(&[(0.0, held), (100.0, a), (400.0, b), (420.0, c)], 440.0, c), a);
        // A steady drag, a change every 50 ms up to the lift: what stands, as the tag said.
        assert_eq!(lifted_span(&[(0.0, held), (500.0, a), (550.0, b), (600.0, c)], 615.0, c), c);
        // The 90 minutes held, then one jump as the finger lifts: the 90 minutes.
        assert_eq!(lifted_span(&[(0.0, held), (690.0, a)], 700.0, a), held);
        // No change at all, or none within the last 80 ms: what stands.
        assert_eq!(lifted_span(&[(0.0, held)], 700.0, held), held);
        assert_eq!(lifted_span(&[(0.0, held), (100.0, a)], 700.0, a), a);
        assert_eq!(lifted_span(&[], 700.0, held), held);
        // Every change inside the last 80 ms, nothing rested before: what stands.
        assert_eq!(lifted_span(&[(650.0, a), (680.0, b)], 700.0, b), b);
    }

    #[test]
    fn the_span_drawn_says_its_times_outside_it() {
        let boxed = |left: f64, top: f64, width: f64, height: f64| Boxed { left, top, width, height };
        // Wide: a column of 140 px, a pixel a minute from 07:00 to 19:00, in a week at 360, 80.
        let (column, week, window) = (boxed(500.0, 100.0, 140.0, 720.0), boxed(360.0, 80.0, 730.0, 760.0), boxed(0.0, 0.0, 1440.0, 900.0));
        let drawn = drawn_at(column, week, window, (420, 1140), (960, 1005), 685.0);
        assert_eq!((drawn.x, drawn.y, drawn.width, drawn.height), (140.0, 560.0, 140.0, 45.0));
        assert_eq!((drawn.tag_x, drawn.tag_y, drawn.times.as_str()), (162.0, 532.0, "16:00–16:45"));
        assert_eq!(drawn.label.as_deref(), Some("16:00 – 16:45"));
        // A quarter of an hour: no text cut in half, the tag says it.
        let short = drawn_at(column, week, window, (420, 1140), (960, 975), 975.0);
        assert_eq!((short.label, short.times.as_str()), (None, "16:00–16:15"));
        // Drawn upward, the pointer at the top edge: the tag further up, clear of a finger's pad.
        assert_eq!(drawn_at(column, week, window, (420, 1140), (960, 1005), 640.0).tag_y, 522.0);
        // A phone's last column of 70 px in a carousel that bleeds 26 px past the window: the tag
        // stays inside what the window shows.
        let (column, carousel, phone) = (boxed(310.0, 280.0, 70.0, 560.0), boxed(-26.0, 256.0, 442.0, 560.0), boxed(0.0, 0.0, 390.0, 844.0));
        let last = drawn_at(column, carousel, phone, (420, 1140), (960, 1005), 700.0);
        assert_eq!(last.tag_x + carousel.left, 292.0);
        assert!(last.tag_x + carousel.left + TAG_WIDTH <= 390.0);
        // Two lines in a narrow column: 45 minutes fit them, 30 do not.
        assert_eq!(last.label.as_deref(), Some("16:00 – 16:45"));
        assert_eq!(drawn_at(column, carousel, phone, (420, 1140), (960, 990), 700.0).label, None);
        // At the grid's top, no room above: drawn upward the tag goes below; drawn downward beside
        // the column (left of the last one), at the span's top edge.
        let up = drawn_at(column, carousel, phone, (420, 1140), (420, 480), 280.0);
        assert_eq!(up.tag_x + carousel.left, 292.0);
        assert!((up.tag_y + carousel.top - (280.0 + 60.0 * 560.0 / 720.0 + TAG_GAP)).abs() < 1e-6);
        let down = drawn_at(column, carousel, phone, (420, 1140), (420, 480), 326.0);
        assert_eq!((down.tag_x + carousel.left, down.tag_y + carousel.top), (210.0, 280.0));
        // A first column: beside it to the right.
        let first = drawn_at(boxed(40.0, 280.0, 70.0, 560.0), carousel, phone, (420, 1140), (420, 480), 326.0);
        assert_eq!(first.tag_x + carousel.left, 114.0);
    }
}
