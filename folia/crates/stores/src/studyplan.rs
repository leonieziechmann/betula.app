//! The Studienplan in this browser: its store, and where „Einplanen" plans a module to.
//!
//! What a visitor plans is theirs alone (R20, owner decision 2026-09-24): the plan lives in this
//! browser's `localStorage` (`betula.studyplan.v1`, the text of `folia_plans::studyplan::PlanDoc`;
//! each profile its own, `profiles`) and
//! nowhere else. It is empty on the server and in a browser that refuses storage (R9), never part
//! of a URL, and it reaches no request: the pages ask the local copy of the catalog about it. Its
//! text is read like a URL (`PlanDoc::restored`): a line that fails is dropped alone, lines of a
//! newer build are kept and written back.
//!
//! Every control reads what it shows through a memo of its own (R5), so a change of the plan
//! changes those controls and what is computed from the plan, not the page around them. A change
//! whose consequences query the catalog or re-run the finder (planning, removing, moving a module,
//! taking a Regelstudienplan over) is written after the next frame (`update_after_paint`): the
//! control that was clicked flips its own state first, so the click answers in the next frame
//! (R21) whatever the work then costs.
//!
//! The program the Stundenplan is planned for (`PlanDoc::program`, picked in its sidebar) is the
//! Stundenplan's alone and kept for the session alone (owner, 2026-10-15: „nur pro Tab
//! (Katalog/Studiengang/Timetable) und Session gespeichert. Ansonsten wird der Mein Studiengang
//! genommen und wenn da keiner hinterlegt ist, dann halt keiner"): in this tab's `sessionStorage`
//! (`betula.studyplan.program`, each profile its own), never in the plan's text. A new session
//! plans for „Mein Studiengang" again; the `g` line an older build wrote is read past.
//!
//! „Einplanen" (`PlanButton`) plans a module from its preview and its page, and a row of the
//! catalog swiped to the right on a phone presses it too (`folia_widgets::swipe`): at once, into the
//! semester `target_semester` aims at — the semester the session plans for (`planning`), for the
//! placeholder the finder was asked for (`PlanHint`) — and a note over the page says where, with ‹ ›
//! that move it and „Rückgängig" (owner, 2026-10-18, concept E of
//! folia/design/einplanen/oberflaeche.html: „Also ich liebe Option E, das ist mit Abstand die
//! eleganteste"). Where no study is set up, a dialog sets it up and plans the module in one
//! (`planning::SetupAsk`, `fit`).

use std::collections::BTreeSet;

use folia_calendar::select::{Selection, TownChoice};
use folia_calendar::semester::{fachsemester, SemesterKey};
use folia_model::labels::{Code, TurnusParity, TurnusSeason};
use folia_model::rows_detail::DateRow;
use folia_plans::study::Offer;
use folia_plans::studyplan::{PlanDoc, SavedPlans};
use folia_routes::url::{self, ModuleHint};
use folia_pages::ask::PlanRowsAsk;
use leptos::prelude::*;

use folia_data::use_data;
use crate::i18n::{self, Texts};
use crate::myprogram::MyProgram;
use crate::profiles::Kept;
use folia_design::nav;
use folia_design::ui::{Icon, Shortcut};

pub mod fit;
mod planning;

pub use planning::{planning_semester, Note, NoteKind, Planning, Pressed, SetupAsk};

/// The first profile's key; another profile's has its number in it (`profiles::key_of`).
pub(crate) const STORAGE_KEY: &str = "betula.studyplan.v1";

/// The first profile's key of the Stundenplan's program in this tab's session (`sessionStorage`,
/// its id); another profile's has its number in it (`profiles::SESSION_KEYS`).
pub(crate) const PROGRAM_KEY: &str = "betula.studyplan.program";

/// The browser app (`csr`): only there is a plan, and only there does a hint aim the button.
const APP: bool = cfg!(feature = "csr");

/// The visitor's Studienplan (the active profile's), shared through context. Empty on the server,
/// always.
#[derive(Clone, Copy)]
pub struct Studyplan(RwSignal<PlanDoc>, Kept);

impl Studyplan {
    /// Reads what this browser has stored for the active profile and provides the plan. Another
    /// tab of the same browser may change it, or another profile becomes the active one; this one
    /// follows.
    pub fn provide() -> Self {
        let kept = Kept::of(STORAGE_KEY);
        let plan = Studyplan(RwSignal::new(load(kept)), kept);
        provide_context(plan);
        kept.follow(move || {
            let stored = load(kept);
            if plan.0.with_untracked(|doc| *doc != stored) {
                plan.0.set(stored);
            }
        });
        plan
    }

    pub fn expect() -> Option<Self> {
        use_context::<Studyplan>()
    }

    /// The profile whose plan this is. Tracked: what a page keeps of the plan for „Rückgängig" is
    /// another profile's once it changes.
    pub fn profile(self) -> u16 {
        self.1.profile()
    }

    /// Reads the plan. Tracked: read it in a memo that keeps only what it needs, never in a list.
    pub fn with<R>(self, f: impl FnOnce(&PlanDoc) -> R) -> R {
        self.0.with(f)
    }

    /// The same without subscribing: for handlers that build the next state on top of it.
    pub fn with_untracked<R>(self, f: impl FnOnce(&PlanDoc) -> R) -> R {
        self.0.with_untracked(f)
    }

    /// Changes the plan and stores it. What depends on the plan hears of it only when something
    /// changed; an empty plan takes the key out of the storage (`nav::local_set`). Its program
    /// goes to the session's storage, not with the plan.
    pub fn update<R>(self, f: impl FnOnce(&mut PlanDoc) -> R) -> R {
        let mut doc = self.0.try_with_untracked(Clone::clone).unwrap_or_default();
        let result = f(&mut doc);
        if self.0.try_with_untracked(|now| *now != doc).unwrap_or(false) {
            if self.0.try_with_untracked(|now| now.program != doc.program).unwrap_or(false) {
                self.1.session_write(PROGRAM_KEY, doc.program.as_deref().unwrap_or_default());
            }
            self.1.write(&stored_text(&doc));
            self.0.set(doc);
        }
        result
    }

    /// For a change whose consequences query the catalog or re-run the finder: `f` runs once
    /// after the next frame (`nav::after_paint`). The control that calls it flips its own state
    /// first, so it answers in the next frame (R21) and the pages follow after.
    pub fn update_after_paint(self, f: impl FnOnce(&mut PlanDoc) + 'static) {
        nav::after_paint(move || self.update(f));
    }

    /// Whether nothing is planned (no module, no placeholder). Tracked.
    pub fn is_empty(self) -> bool {
        self.0.with(PlanDoc::is_empty)
    }

    /// How many modules are planned (`planned_modules`). Tracked: read it in a memo.
    pub fn count(self) -> usize {
        self.0.with(planned_modules)
    }

    /// How many modules are planned into a semester: what the timetable of that semester holds.
    /// Tracked: read it in a memo.
    pub fn count_in(self, s: SemesterKey) -> usize {
        self.0.with(|doc| doc.modules.iter().filter(|m| m.semester == s).count())
    }

    /// The modules planned into a semester, in the order they were planned. Tracked.
    pub fn modules_in(self, s: SemesterKey) -> Vec<String> {
        self.0.with(|doc| doc.modules_in(s))
    }

    /// Whether the module is planned into the semester. Tracked: read it in a memo per button (R5).
    pub fn is_planned(self, s: SemesterKey, id: &str) -> bool {
        self.0.with(|doc| doc.is_planned(s, id))
    }

    /// What the semester hides and has chosen, with the town. Tracked.
    pub fn selection(self, s: SemesterKey, town: TownChoice) -> Selection {
        self.0.with(|doc| doc.selection(s, town))
    }

    /// Plans several modules at once (`plan_picks`: the rows selected on the Merkliste), after
    /// the next frame (R21); `then` hears what was done, for the note and its „Rückgängig".
    pub fn plan_many(self, picks: Vec<(String, SemesterKey)>, current: SemesterKey, moves: bool, then: impl FnOnce(PlannedMany) + 'static) {
        nav::after_paint(move || {
            let done = self.update(|doc| {
                let before = doc.clone();
                let (into, there) = plan_picks(doc, &picks, current, moves, now());
                PlannedMany { into, there, before, after: doc.clone() }
            });
            then(done);
        });
    }

    /// „Rückgängig" of `plan_many`: the plan as it was, while it is what that made of it; changed
    /// besides (another tab), what it planned is taken out again and what it moved goes back.
    pub fn undo_many(self, planned: PlannedMany) {
        self.update_after_paint(move |doc| {
            if *doc == planned.after {
                *doc = planned.before;
                return;
            }
            for (id, to, from) in &planned.into {
                doc.unplan(*to, id, &[]);
                let was = from.and_then(|from| planned.before.modules.iter().find(|m| m.semester == from && m.module_id == *id));
                if let Some(was) = was {
                    doc.plan(was.semester, id, was.at, was.fills);
                }
            }
        });
    }
}

/// Modules planned at once (`Studyplan::plan_many`): what went where (and, moved, where from),
/// how many were planned where they were to go already, and the plan before and after, for
/// „Rückgängig".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlannedMany {
    pub into: Vec<(String, SemesterKey, Option<SemesterKey>)>,
    pub there: usize,
    before: PlanDoc,
    after: PlanDoc,
}

impl PlannedMany {
    /// How many went into each semester, the semesters in their order.
    pub fn by_semester(&self) -> Vec<(SemesterKey, usize)> {
        let mut counts: std::collections::BTreeMap<SemesterKey, usize> = std::collections::BTreeMap::new();
        for (_, to, _) in &self.into {
            *counts.entry(*to).or_default() += 1;
        }
        counts.into_iter().collect()
    }
}

/// Plans each of `picks` into its semester (konzept.html, „Mehrere auf einmal": „jedes dorthin,
/// wohin sein „Einplanen" zielt, oder alle in ein Semester"): one planned there already stays and
/// is counted; with `moves` (all into one semester) a module planned in a semester from `current`
/// on moves there, its time and what it counts for with it, while one planned only behind the
/// student is planned anew, as the switch plans a retake. What was planned, where and from where,
/// and how many were there already.
pub fn plan_picks(doc: &mut PlanDoc, picks: &[(String, SemesterKey)], current: SemesterKey, moves: bool, at: u64) -> (Vec<(String, SemesterKey, Option<SemesterKey>)>, usize) {
    let mut into = Vec::new();
    let mut there = 0;
    for (id, to) in picks {
        if doc.is_planned(*to, id) {
            there += 1;
            continue;
        }
        let from = if moves { doc.planned_in(id).into_iter().filter(|s| *s >= current).min() } else { None };
        match from {
            Some(from) => {
                doc.move_to(from, *to, id);
                if doc.is_planned(*to, id) {
                    into.push((id.clone(), *to, Some(from)));
                }
            }
            None => {
                if doc.plan(*to, id, at, None) {
                    into.push((id.clone(), *to, None));
                }
            }
        }
    }
    (into, there)
}

/// The first profile's key of the saved plans; another profile's has its number in it.
pub(crate) const SAVED_KEY: &str = "betula.studyplan.saved.v1";

/// The plans saved in this browser („Plan speichern", `folia_plans::studyplan::SavedPlans`): kept like
/// the plan (R20: in `localStorage` alone, never in an address or a request, read like anything
/// from outside; each profile its own). Only the Stundenplan's sidebar and the profile page read
/// them, so each makes the store itself; another tab's change and another profile follow.
#[derive(Clone, Copy)]
pub struct Saved(RwSignal<SavedPlans>, Kept);

impl Saved {
    /// Reads what this browser has saved for the active profile; follows another tab and another
    /// profile from then on.
    pub fn open() -> Self {
        let kept = Kept::of(SAVED_KEY);
        let saved = Saved(RwSignal::new(load_saved(kept)), kept);
        kept.follow(move || {
            let stored = load_saved(kept);
            if saved.0.with_untracked(|plans| *plans != stored) {
                saved.0.set(stored);
            }
        });
        saved
    }

    /// Tracked: read it in a memo that keeps only what it needs.
    pub fn with<R>(self, f: impl FnOnce(&SavedPlans) -> R) -> R {
        self.0.with(f)
    }

    pub fn with_untracked<R>(self, f: impl FnOnce(&SavedPlans) -> R) -> R {
        self.0.with_untracked(f)
    }

    /// Changes and stores them, telling what depends on them only when something changed.
    pub fn update<R>(self, f: impl FnOnce(&mut SavedPlans) -> R) -> R {
        let mut plans = self.0.try_with_untracked(Clone::clone).unwrap_or_default();
        let result = f(&mut plans);
        if self.0.try_with_untracked(|now| *now != plans).unwrap_or(false) {
            self.1.write(&plans.stored());
            self.0.set(plans);
        }
        result
    }
}

fn load_saved(kept: Kept) -> SavedPlans {
    kept.read().as_deref().map(SavedPlans::restored).unwrap_or_default()
}

/// What this browser has stored, with the program this tab picked in the session. Nothing on the
/// server, and nothing in a browser that refuses storage (a plan then lasts as long as the page).
fn load(kept: Kept) -> PlanDoc {
    restored(kept.read().as_deref(), kept.session_read(PROGRAM_KEY).as_deref())
}

/// The plan of a stored text and the program of the session, read like anything from outside:
/// garbage is dropped line by line, never an error, and no text reads as more than the caps allow.
/// A program the text names (`g`, an older build's) is not the session's: it is left out.
fn restored(stored: Option<&str>, program: Option<&str>) -> PlanDoc {
    let program = program.map(str::trim).filter(|id| url::is_program_id(id)).map(str::to_string);
    PlanDoc { program, ..stored.map(PlanDoc::restored).unwrap_or_default() }
}

/// How many modules a plan holds, each once however many semesters it is planned into (a retake):
/// the number the rail's „Plan" shows.
fn planned_modules(doc: &PlanDoc) -> usize {
    doc.modules.iter().map(|planned| planned.module_id.as_str()).collect::<BTreeSet<_>>().len()
}

/// The text a plan is stored as: all but its program, which is the session's. Empty only when
/// there is nothing at all to keep, and then the key goes (`nav::local_set`): a plan without
/// modules may still hold hidden Termine, a subscription's code or a newer build's lines, which
/// must survive (`PlanDoc::is_empty` is not the test).
fn stored_text(doc: &PlanDoc) -> String {
    doc.stored_without_program()
}

/// What a page asks „Einplanen" to plan into: the semester that „Passt in meinen Stundenplan" in
/// the catalog was checked against (`plan=`), and the placeholder a module found for it would fill
/// (`fill=`), whose semester wins (`target_semester`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanHint {
    pub semester: Option<SemesterKey>,
    pub fill: Option<u32>,
}

impl PlanHint {
    /// The hint a module's address carries (`?plan=2026W&fill=p3`, the way from the finder on a
    /// phone); `None` when it carries neither.
    pub fn of(hint: &ModuleHint) -> Option<Self> {
        let semester = hint.plan.as_deref().and_then(SemesterKey::parse);
        (semester.is_some() || hint.fill.is_some()).then_some(Self { semester, fill: hint.fill })
    }

    /// `?plan=2026W&fill=p3`, what follows a module's path to carry the hint there.
    pub fn query(&self) -> String {
        ModuleHint { plan: self.semester.map(SemesterKey::key), fill: self.fill }.query()
    }
}

/// The semester „Einplanen" plans a module into (verhalten.html, rules 1–5), the first that
/// applies: the page's hint (the semester of the placeholder it fills while the plan still has it,
/// else the semester the finder checked); else `base`, the semester the session plans for
/// (`Planning::base`), where the module is offered then (by its turnus, or its events are there);
/// else the next semester that offers it or has its events; and for a module whose turnus says no
/// season (irregular, unknown) the semester of its newest events where they are later — unless the
/// student chose the semester (P3: the caller leaves `newest` out then).
///
/// The placeholder wins over the finder's semester: a module planned for a placeholder fills it
/// (`fills=<pid>`, A.7), which means something only in the placeholder's own semester. The
/// finder's switch checks that semester too, also when it is turned off and on again while `fill`
/// stays in the address; the two differ only in an address written by hand.
pub fn target_semester(base: SemesterKey, newest: Option<SemesterKey>, offer: Offer, hint: Option<&PlanHint>, doc: &PlanDoc) -> SemesterKey {
    if let Some(hint) = hint {
        let filled = hint.fill.and_then(|pid| doc.placeholders.iter().find(|p| p.pid == pid)).map(|p| p.semester);
        if let Some(semester) = filled.or(hint.semester) {
            return semester;
        }
    }
    if newest != Some(base) && !offer.offered(base) {
        let next = offer.next(base);
        return match newest {
            Some(newest) if newest > base && newest < next => newest,
            _ => next,
        };
    }
    let seasonless = !matches!(offer.season, Some(TurnusSeason::Winter | TurnusSeason::Summer | TurnusSeason::Both));
    match newest {
        Some(newest) if seasonless && newest > base => newest,
        _ => base,
    }
}

/// Where a plan button stands decides what it looks like.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanLook {
    /// In the line of a module's badges beside „Merken", in the preview and on the module's page:
    /// the switch with its label and its shortcut (R10), and in the app a small line of where it
    /// plans or is planned (`hero_note`).
    Hero,
    /// Among the actions of a module page's sidebar: the semester under the label.
    Action,
}

/// „Einplanen": plans the module at once where it aims (`aim_of`) — for the hint's placeholder,
/// when the finder was asked for one — and the note over the page says where, with ‹ › and
/// „Rückgängig" (`Planning::press`); planned there, a click takes it out again. Same behaviour in
/// every look; `P` presses the one of what the visitor is at (`enhance.js`,
/// `data-action="plan"`). Where no study is set up, the dialog asks for it first.
///
/// Part of server HTML like `MarkButton`: unpressed and aimed as for an empty plan, the same for
/// everybody (R9), kept in its place but not shown until the app runs (`.plan-toggle`), so nothing
/// moves at the takeover (R15). A click flips the button at once and writes the plan after the
/// next frame (R21): what follows from the plan (the week beside the module, the finder, the
/// lists) is computed after the button has answered.
///
/// `current` is the snapshot's current semester, `newest` the semester of the module's newest
/// teaching events (`target_semester`); `turnus` and `parity` say which semesters offer it.
#[component]
pub fn PlanButton(
    #[prop(into)] id: String,
    #[prop(into)] title: String,
    turnus: Option<Code<TurnusSeason>>,
    #[prop(optional_no_strip)] parity: Option<Code<TurnusParity>>,
    current: SemesterKey,
    newest: Option<SemesterKey>,
    #[prop(optional, into)] hint: Signal<Option<PlanHint>>,
    look: PlanLook,
) -> impl IntoView {
    let t = i18n::t();
    let plan = Studyplan::expect().filter(|_| APP);
    let planning = Planning::expect().filter(|_| APP);
    let mine = MyProgram::expect().filter(|_| APP);
    let source = use_data().ok();
    let offer = Offer { season: turnus.and_then(|turnus| turnus.known()), parity: parity.and_then(|parity| parity.known()) };
    // One memo per button (R5), from the plan, the hint and where the session plans.
    let aim = {
        let id = id.clone();
        Memo::new(move |_| match plan {
            Some(plan) => {
                let (base, chosen) = planning.map_or((current, false), |planning| planning.base(current));
                let hint = hint.get();
                plan.with(|doc| aim_of(&id, current, base, newest.filter(|_| !chosen), offer, hint.as_ref(), doc))
            }
            None => aim_of(&id, current, current, newest, offer, None, &PlanDoc::default()),
        })
    };
    // The Studienbeginn, for the Fachsemester the small line says.
    let start = Memo::new(move |_| mine.and_then(MyProgram::start));
    // What the last click said, until the plan it writes after the next frame has it.
    let said = RwSignal::new(None::<bool>);
    let pressed = Memo::new(move |_| said.get().unwrap_or_else(|| aim.with(|aim| aim.pressed)));
    let toggle = {
        let what = Pressed { id: id.clone(), title, current, offer, newest, hint: None };
        move |_: leptos::ev::MouseEvent| {
            let (Some(plan), Some(planning)) = (plan, planning) else { return };
            let was = pressed.get_untracked();
            let aim = Aim { pressed: was, ..aim.get_untracked() };
            let what = Pressed { hint: hint.get_untracked(), ..what.clone() };
            if planning.press(plan, source.clone(), what, aim, move || {
                said.try_set(None);
            }) {
                said.set(Some(!was));
            }
        }
    };
    // The label reads `pressed` alone; the other texts read the click's word and the aim, never
    // `pressed` with the aim it comes from (R16).
    let label = move || label_text(pressed.get(), t);
    let pressed_attr = move || if pressed.get() { "true" } else { "false" };
    let busy = move || said.get().map(|_| "true");
    let icon = move || match pressed.get() {
        true => view! { <Icon name="calendar-check-2"/> }.into_any(),
        false => view! { <Icon name="calendar-plus"/> }.into_any(),
    };
    let tip = move || {
        let said = said.get();
        aim.with(|aim| tooltip_text(aim, said.unwrap_or(aim.pressed), t))
    };
    let (class, words) = match look {
        PlanLook::Hero => {
            // The app's alone: where a session plans depends on the day (R9).
            let note = move || APP.then(|| aim.with(|aim| hero_note(aim, start.get(), t))).map(|note| view! { <small>{note}</small> });
            ("plan-toggle mark-switch hit", view! { <span>{label}</span>{note}<Shortcut keys="P"/> }.into_any())
        }
        PlanLook::Action => {
            let line = move || aim.with(|aim| semester_line(aim, start.get(), t));
            ("plan-toggle action", view! { <span>{label}<small>{line}</small></span> }.into_any())
        }
    };
    view! {
        <button
            class=class
            type="button"
            data-action="plan"
            on:click=toggle
            aria-pressed=pressed_attr
            aria-busy=busy
            title=tip
        >
            {icon}{words}
        </button>
    }
}

/// What a plan button aims at: from the plan, the page's hint and the module's semesters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Aim {
    /// The semester a click plans into (`target_semester`), or takes the module out of where it is
    /// planned (`pressed`).
    target: SemesterKey,
    /// The module is planned there — for the hint's placeholder, when there is one.
    pressed: bool,
    /// The hint's placeholder while the plan still has it: its pid and name.
    fill: Option<(u32, String)>,
    /// The other semesters the plan holds the module in.
    elsewhere: Vec<SemesterKey>,
}

impl Aim {
    /// The module is planned where a press aims: pressing takes it out.
    pub fn pressed(&self) -> bool {
        self.pressed
    }

    /// Where a press plans the module, or takes it out of.
    pub fn target(&self) -> SemesterKey {
        self.target
    }
}

/// What „Einplanen" aims at for a module now, asked once and not through a memo: for what presses
/// it without being its switch, a row swiped to the right (`folia_widgets::swipe`), which asks when
/// the finger starts. `current` and `newest` as for `PlanButton`.
pub fn aim_now(plan: Studyplan, planning: Option<Planning>, id: &str, current: SemesterKey, newest: Option<SemesterKey>, offer: Offer, hint: Option<&PlanHint>) -> Aim {
    let (base, chosen) = planning.map_or((current, false), |planning| planning.base_untracked(current));
    plan.with_untracked(|doc| aim_of(id, current, base, newest.filter(|_| !chosen), offer, hint, doc))
}

/// Where a press plans, or what it takes out (verhalten.html, rule 2): with a page that means a
/// semester, that semester alone; else the module planned where it aims, or from the current
/// semester on (the first of those), which the switch then says and a press takes it out of; else
/// where it aims. A module planned only in semesters behind the student is planned anew (a
/// retake), the small line names them.
fn aim_of(id: &str, current: SemesterKey, base: SemesterKey, newest: Option<SemesterKey>, offer: Offer, hint: Option<&PlanHint>, doc: &PlanDoc) -> Aim {
    let target = target_semester(base, newest, offer, hint, doc);
    // A placeholder the plan no longer has asks for nothing, as in `target_semester`.
    let fill = hint.and_then(|hint| hint.fill).and_then(|pid| doc.placeholders.iter().find(|p| p.pid == pid)).map(|p| (p.pid, p.name.clone()));
    let means_a_semester = hint.is_some_and(|hint| hint.semester.is_some());
    let planned_at = match &fill {
        Some((pid, _)) => doc.modules.iter().any(|m| m.semester == target && m.module_id == id && m.fills == Some(*pid)).then_some(target),
        None if means_a_semester || doc.is_planned(target, id) => doc.is_planned(target, id).then_some(target),
        None => doc.planned_in(id).into_iter().filter(|semester| *semester >= current).min(),
    };
    let target = planned_at.unwrap_or(target);
    let elsewhere = doc.planned_in(id).into_iter().filter(|semester| *semester != target).collect();
    Aim { target, pressed: planned_at.is_some(), fill, elsewhere }
}

/// How long a placeholder's name may be in the button's small line; the tooltip has it whole.
const NAME_CHARS: usize = 28;

/// A placeholder's name as a small line says it: whole when short, else cut at a word, with „…".
fn shortened(name: &str) -> String {
    let name = name.trim();
    if name.chars().count() <= NAME_CHARS {
        return name.to_string();
    }
    let cut: String = name.chars().take(NAME_CHARS).collect();
    let at_word = cut.rfind(' ').filter(|at| *at >= NAME_CHARS / 2).and_then(|at| cut.get(..at)).unwrap_or(&cut);
    format!("{}…", at_word.trim_end_matches([' ', ',', ';', ':', '-', '–', '(', '/']))
}

/// „Einplanen" / „Eingeplant", in every look and whatever the plan: the label keeps the width the
/// server gave it (R15), a click only swaps the word. What a button says beyond it — where it
/// plans, the placeholder it plans for, the other semesters the module is planned in — is its
/// small line (`hero_note`, `semester_line`), which gives way first where space is short.
fn label_text(pressed: bool, t: &'static Texts) -> &'static str {
    if pressed {
        t.planner.planned
    } else {
        t.planner.plan
    }
}

/// „für „Anwendungsfach“" while the hint's placeholder is in the plan.
fn fill_text(aim: &Aim, t: &'static Texts) -> Option<String> {
    aim.fill.as_ref().map(|(_, name)| (t.planner.for_placeholder)(&shortened(name)))
}

/// „geplant: SoSe 2027" when the plan holds the module in other semesters as well, the semesters
/// as `label` names them.
fn elsewhere_text(aim: &Aim, label: impl Fn(SemesterKey) -> String, t: &'static Texts) -> Option<String> {
    (!aim.elsewhere.is_empty()).then(|| (t.planner.planned_in)(&aim.elsewhere.iter().map(|semester| label(*semester)).collect::<Vec<_>>().join(", ")))
}

/// The semester a press plans into, as `label` names it; the one the module is planned in with its
/// Fachsemester where the Studienbeginn is known („WiSe 26/27 · 3. FS").
fn where_text(aim: &Aim, start: Option<SemesterKey>, label: impl Fn(SemesterKey) -> String, t: &'static Texts) -> String {
    let semester = label(aim.target);
    match start.filter(|_| aim.pressed).and_then(|start| fachsemester(aim.target, start)) {
        Some(fs) => format!("{semester} · {}", (t.planner.fs_short)(fs)),
        None => semester,
    }
}

/// The small line of the switch beside „Merken" („WiSe 26/27", „SoSe 27 · 4. FS · für
/// „Anwendungsfach“ · geplant: WiSe 25/26").
fn hero_note(aim: &Aim, start: Option<SemesterKey>, t: &'static Texts) -> String {
    let short = |key: SemesterKey| key.short(t.locale);
    [Some(where_text(aim, start, short, t)), fill_text(aim, t), elsewhere_text(aim, short, t)].into_iter().flatten().collect::<Vec<_>>().join(" · ")
}

/// The line under the sidebar's label: the same, the semesters written out.
fn semester_line(aim: &Aim, start: Option<SemesterKey>, t: &'static Texts) -> String {
    let long = |key: SemesterKey| key.label(t.locale);
    [Some(where_text(aim, start, long, t)), fill_text(aim, t), elsewhere_text(aim, long, t)].into_iter().flatten().collect::<Vec<_>>().join(" · ")
}

/// What a click does, with the placeholder's whole name and the shortcut.
fn tooltip_text(aim: &Aim, pressed: bool, t: &'static Texts) -> String {
    let semester = aim.target.label(t.locale);
    let what = match (&aim.fill, pressed) {
        (_, true) => (t.planner.unplan_hint)(&semester),
        (Some((_, name)), false) => (t.planner.plan_for_hint)(&semester, name),
        (None, false) => (t.planner.plan_hint)(&semester),
    };
    let elsewhere = elsewhere_text(aim, |key| key.label(t.locale), t).map(|text| format!(" · {text}")).unwrap_or_default();
    format!("{what}{elsewhere} (P)")
}

/// What a row swiped to the right says (`folia_widgets::swipe`), as (word, line, word once done): the
/// switch's „Einplanen" with the semester it plans into and the placeholder it plans for, or
/// „Entfernen" with the semester it takes the module out of — what the swipe does, where the
/// switch's label says what the module is.
pub fn swipe_words(aim: &Aim, t: &'static Texts) -> (&'static str, String, &'static str) {
    let semester = aim.target.label(t.locale);
    if aim.pressed {
        return (t.planner.remove, (t.planner.out_of)(&semester), t.planner.removed);
    }
    let line = [Some(semester), fill_text(aim, t)].into_iter().flatten().collect::<Vec<_>>().join(" · ");
    (t.planner.plan, line, t.planner.planned)
}

/// What a click does to the plan, after the next frame: takes the module out of the semester it
/// was pressed for; else plans it in, for the hint's placeholder — which a module planned there
/// already only starts counting for.
fn toggle_in(doc: &mut PlanDoc, id: &str, aim: &Aim, was_pressed: bool, only_its_events: &[u32], at: u64) {
    if was_pressed {
        doc.unplan(aim.target, id, only_its_events);
        return;
    }
    match aim.fill.as_ref().map(|(pid, _)| *pid) {
        Some(pid) if doc.is_planned(aim.target, id) => doc.set_fills(aim.target, id, Some(pid)),
        fills => {
            doc.plan(aim.target, id, at, fills);
        }
    }
}

/// What tells which events of `id` no other module of the plan has (`events_alone`): the rows of
/// the module and of the plan's others in `semester`. Where the catalog cannot say, none, which
/// keeps the semester's own lines (the safe direction).
fn only_its_events_ask(doc: &PlanDoc, semester: SemesterKey, id: &str) -> PlanRowsAsk {
    let others: Vec<String> = doc.modules_in(semester).into_iter().filter(|other| other != id).collect();
    PlanRowsAsk { key: semester, own: vec![id.to_string()], others }
}

/// The events of `own` rows that none of `others` has, each once: what taking the module out of
/// the semester takes along of what the semester hides and has chosen (B.2).
fn events_alone(own: &[DateRow], others: &[DateRow]) -> Vec<u32> {
    let theirs: BTreeSet<&str> = others.iter().map(|row| row.date.event_id.as_str()).collect();
    let alone: BTreeSet<u32> = own.iter().filter(|row| !theirs.contains(row.date.event_id.as_str())).filter_map(|row| row.date.event_id.parse().ok()).collect();
    alone.into_iter().collect()
}

/// When a module was planned, in seconds since 1970. 0 on the server, which plans nothing.
fn now() -> u64 {
    #[cfg(feature = "csr")]
    {
        (web_sys::js_sys::Date::now() / 1000.0).max(0.0) as u64
    }
    #[cfg(not(feature = "csr"))]
    0
}

#[cfg(test)]
mod tests {
    use crate::i18n::{DE, EN};
    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    #[test]
    fn what_is_stored_is_read_like_anything_from_outside() {
        // Nothing stored, or garbage: an empty plan, never an error.
        assert!(restored(None, None).is_empty());
        let garbage = restored(Some("<script>alert(1)</script>\nm\t2026X\t12104\t0\t\nm\t2026W\t../../etc\t0\t\n\u{0}\u{7f}\n"), Some("../../etc"));
        assert!(garbage.is_empty() && garbage.modules.is_empty() && garbage.placeholders.is_empty() && garbage.program.is_none());
        // A good line among bad ones survives alone.
        let mixed = restored(Some("m\t2026W\t12104\t1790000000\t\nm\tWiSe\t12107\t0\t\ne\t2026W\t0\n"), None);
        assert_eq!(mixed.modules_in(key("2026W")), vec!["12104".to_string()]);
        assert!(!mixed.is_empty());
    }

    /// The Stundenplan's program is the session's (owner, 2026-10-15): what an older build kept in
    /// the plan's text (`g`) is read past, the program of the session taken, and the text keeps
    /// none.
    #[test]
    fn the_program_of_the_stundenplan_is_the_sessions_alone() {
        let text = "g\t079-82-2008\nm\t2026W\t12104\t1\t\n";
        // A new session: „Mein Studiengang" (none of its own), whatever the text named.
        assert_eq!(restored(Some(text), None).program, None);
        let picked = restored(Some(text), Some("048-82-2022"));
        assert_eq!((picked.program.as_deref(), picked.modules_in(key("2026W"))), (Some("048-82-2022"), vec!["12104".to_string()]));
        // The text written back keeps the plan, not the program.
        assert_eq!(stored_text(&picked), "m\t2026W\t12104\t1\t\n");
        assert_eq!(restored(Some(&stored_text(&picked)), Some("048-82-2022")), picked);
        // A program alone is nothing to keep in the plan's text: the key goes.
        assert_eq!(stored_text(&PlanDoc { program: Some("048-82-2022".into()), ..PlanDoc::default() }), "");
    }

    #[test]
    fn an_empty_plan_takes_its_key_out_and_a_plan_without_modules_keeps_what_it_hides() {
        // Nothing at all: the empty text, which `nav::local_set` turns into removing the key.
        assert_eq!(stored_text(&PlanDoc::default()), "");
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key("2026W"), "12104", 1, None));
        doc.set_event(key("2026W"), 149408, true);
        assert!(stored_text(&doc).contains("m\t2026W\t12104") && restored(Some(&stored_text(&doc)), None) == doc);
        // The last module goes; what the semester hides stays, so the key stays too.
        doc.unplan(key("2026W"), "12104", &[]);
        assert!(doc.is_empty() && !stored_text(&doc).is_empty());
        // Showing everything again leaves nothing to keep.
        doc.show_all(key("2026W"));
        assert_eq!(stored_text(&doc), "");
    }

    #[test]
    fn the_rail_counts_each_planned_module_once() {
        let mut doc = PlanDoc::default();
        assert_eq!(planned_modules(&doc), 0);
        assert!(doc.plan(key("2026W"), "12104", 1, None) && doc.plan(key("2026W"), "12107", 1, None));
        // A retake in the next winter is the same module.
        assert!(doc.plan(key("2027W"), "12104", 2, None));
        assert_eq!(planned_modules(&doc), 2);
    }

    fn offer(season: Option<TurnusSeason>) -> Offer {
        Offer { season, parity: None }
    }

    #[test]
    fn einplanen_aims_at_the_semester_that_fits() {
        let now = key("2026W");
        let doc = PlanDoc::default();
        // The page's hint wins, over the module's own data.
        let hint = PlanHint { semester: Some(key("2027S")), fill: None };
        assert_eq!(target_semester(now, Some(now), offer(Some(TurnusSeason::Winter)), Some(&hint), &doc), key("2027S"));
        // The semester the session plans for (P1), where the module is offered then (rule 3).
        assert_eq!(target_semester(now, Some(now), offer(Some(TurnusSeason::Both)), None, &doc), now);
        assert_eq!(target_semester(key("2027S"), Some(now), offer(Some(TurnusSeason::Both)), None, &doc), key("2027S"));
        // Not offered then: the next semester that offers it (rule 4).
        assert_eq!(target_semester(now, Some(key("2026S")), offer(Some(TurnusSeason::Summer)), None, &doc), key("2027S"));
        assert_eq!(target_semester(key("2027S"), None, offer(Some(TurnusSeason::Winter)), None, &doc), key("2027W"));
        assert_eq!(target_semester(now, None, Offer { season: Some(TurnusSeason::Winter), parity: Some(TurnusParity::Odd) }, None, &doc), key("2027W"));
        // No season stated (irregular, unknown): where its events are, when they are later (rule 5).
        assert_eq!(target_semester(now, Some(key("2027S")), offer(Some(TurnusSeason::Irregular)), None, &doc), key("2027S"));
        assert_eq!(target_semester(now, Some(key("2027S")), offer(None), None, &doc), key("2027S"));
        assert_eq!(target_semester(now, Some(key("2026S")), offer(None), None, &doc), now);
        // A season stated decides where the events are not; where they are, they do.
        assert_eq!(target_semester(now, Some(key("2027S")), offer(Some(TurnusSeason::Both)), None, &doc), now);
        assert_eq!(target_semester(now, Some(now), offer(Some(TurnusSeason::Summer)), None, &doc), now);
        // A placeholder's hint without a semester plans into the placeholder's semester.
        let mut with_placeholder = PlanDoc::default();
        with_placeholder.placeholders.push(placeholder(3, "2027W", "Fachübergreifendes Studium"));
        let fill = PlanHint { semester: None, fill: Some(3) };
        assert_eq!(target_semester(now, Some(now), offer(None), Some(&fill), &with_placeholder), key("2027W"));
        // The finder checked another semester („Passt in meinen Stundenplan" off and on again,
        // `fill` kept): the placeholder still decides, so the module lands where it fills it.
        let both = PlanHint { semester: Some(now), fill: Some(3) };
        assert_eq!(target_semester(now, Some(now), offer(None), Some(&both), &with_placeholder), key("2027W"));
        // One the plan no longer has changes nothing: the finder's semester, else the module's.
        let gone = PlanHint { semester: None, fill: Some(9) };
        assert_eq!(target_semester(now, Some(now), offer(None), Some(&gone), &with_placeholder), now);
        let gone_but_checked = PlanHint { semester: Some(key("2027S")), fill: Some(9) };
        assert_eq!(target_semester(now, Some(now), offer(None), Some(&gone_but_checked), &with_placeholder), key("2027S"));
    }

    /// verhalten.html, rule 2: planned from the current semester on, the switch says where and a
    /// press takes it out there; planned only behind the student, a press plans it anew.
    #[test]
    fn a_module_planned_is_pressed_where_it_is_planned() {
        let now = key("2026W");
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key("2027S"), "12330", 1, None));
        let later = aim_of("12330", now, now, Some(now), offer(None), None, &doc);
        assert_eq!((later.target, later.pressed, later.elsewhere.clone()), (key("2027S"), true, vec![]));
        // Behind the student: a retake, planned where the session aims, the past one named.
        let mut past = PlanDoc::default();
        assert!(past.plan(key("2025W"), "12330", 1, None));
        let retake = aim_of("12330", now, now, Some(now), offer(None), None, &past);
        assert_eq!((retake.target, retake.pressed, retake.elsewhere.clone()), (now, false, vec![key("2025W")]));
        // Where the session plans into the past („Weitere Module auch nach WiSe 25/26"): pressed
        // there.
        let chosen = aim_of("12330", now, key("2025W"), None, offer(None), None, &past);
        assert_eq!((chosen.target, chosen.pressed), (key("2025W"), true));
        // A page that means a semester asks about that semester alone.
        let finder = PlanHint { semester: Some(key("2027W")), fill: None };
        let asked = aim_of("12330", now, now, Some(now), offer(None), Some(&finder), &doc);
        assert_eq!((asked.target, asked.pressed, asked.elsewhere.clone()), (key("2027W"), false, vec![key("2027S")]));
    }

    fn placeholder(pid: u32, semester: &str, name: &str) -> folia_plans::studyplan::Placeholder {
        folia_plans::studyplan::Placeholder {
            pid,
            semester: key(semester),
            program_id: "079-82-2008".into(),
            ord: i64::from(pid),
            span: (1, 1),
            credits: Some("6".into()),
            kind: None,
            caption: String::new(),
            name: name.into(),
        }
    }

    #[test]
    fn a_module_address_hints_where_to_plan() {
        assert_eq!(PlanHint::of(&ModuleHint::parse("plan=2027s&fill=p3")), Some(PlanHint { semester: Some(key("2027S")), fill: Some(3) }));
        assert_eq!(PlanHint::of(&ModuleHint::parse("fill=p3")), Some(PlanHint { semester: None, fill: Some(3) }));
        assert_eq!(PlanHint::of(&ModuleHint::parse("plan=nonsense")), None);
        assert_eq!(PlanHint::of(&ModuleHint::parse("")), None);
        // And back into an address: what a preview's „Vollbild" carries to the module's page.
        assert_eq!(PlanHint { semester: Some(key("2027S")), fill: Some(3) }.query(), "?plan=2027S&fill=p3");
        assert_eq!(PlanHint { semester: None, fill: Some(3) }.query(), "?fill=p3");
    }

    #[test]
    fn the_button_says_where_it_plans_and_what_for() {
        let now = key("2026W");
        let mut doc = PlanDoc::default();
        doc.placeholders.push(placeholder(3, "2026W", "Fachübergreifendes Studium"));
        doc.placeholders.push(placeholder(4, "2026W", "wählbar aus dem Wahlpflichtangebot Wirtschaftswissenschaften (Übersicht Anlage a.2) Prü/SL"));
        // An empty plan: the module's own semester, unpressed; the small line says where.
        let plain = aim_of("12330", now, now, Some(now), offer(None), None, &PlanDoc::default());
        assert_eq!((plain.target, plain.pressed, label_text(plain.pressed, &DE), hero_note(&plain, None, &DE)), (now, false, "Einplanen", "WiSe 26/27".to_string()));
        assert_eq!(label_text(true, &DE), "Eingeplant");
        assert_eq!(tooltip_text(&plain, false, &DE), "In WiSe 2026/27 einplanen (P)");
        assert_eq!(tooltip_text(&plain, true, &DE), "Eingeplant in WiSe 2026/27. Noch einmal nimmt das Modul aus dem Plan (P)");
        assert_eq!(semester_line(&plain, None, &DE), "WiSe 2026/27");

        // Planned in a later semester: pressed there, with its Fachsemester where the
        // Studienbeginn is known.
        assert!(doc.plan(key("2027S"), "12330", 1, None));
        let later = aim_of("12330", now, now, Some(now), offer(None), None, &doc);
        assert!(later.pressed);
        assert_eq!(hero_note(&later, Some(key("2025W")), &DE), "SoSe 27 · 4. FS");
        assert_eq!(semester_line(&later, Some(key("2025W")), &DE), "SoSe 2027 · 4. FS");
        assert_eq!(hero_note(&later, None, &DE), "SoSe 27");
        assert_eq!(tooltip_text(&later, true, &DE), "Eingeplant in SoSe 2027. Noch einmal nimmt das Modul aus dem Plan (P)");

        // For a placeholder: pressed only while the module counts for it; the small line names
        // it, and where else the module is planned.
        let hint = PlanHint { semester: None, fill: Some(3) };
        assert!(doc.plan(now, "12330", 1, None));
        let fill = aim_of("12330", now, now, Some(now), offer(None), Some(&hint), &doc);
        assert_eq!((fill.pressed, label_text(fill.pressed, &DE)), (false, "Einplanen"));
        assert_eq!(hero_note(&fill, None, &DE), "WiSe 26/27 · für „Fachübergreifendes Studium“ · geplant: SoSe 27");
        assert_eq!(semester_line(&fill, None, &DE), "WiSe 2026/27 · für „Fachübergreifendes Studium“ · geplant: SoSe 2027");
        assert_eq!(tooltip_text(&fill, false, &DE), "In WiSe 2026/27 einplanen, für „Fachübergreifendes Studium“ · geplant: SoSe 2027 (P)");
        // A long name is cut at a word in the small line; the tooltip keeps it whole.
        let long = aim_of("12330", now, now, Some(now), offer(None), Some(&PlanHint { semester: None, fill: Some(4) }), &doc);
        assert_eq!(hero_note(&long, None, &DE), "WiSe 26/27 · für „wählbar aus dem…“ · geplant: SoSe 27");
        assert!(tooltip_text(&long, false, &DE).contains("Prü/SL“"));
        // A placeholder the plan no longer has asks for nothing.
        let gone = aim_of("12330", now, now, Some(now), offer(None), Some(&PlanHint { semester: None, fill: Some(9) }), &doc);
        assert_eq!((gone.fill.clone(), gone.pressed, label_text(gone.pressed, &DE)), (None, true, "Eingeplant"));

        // The same in English.
        assert_eq!((label_text(false, &EN), label_text(true, &EN)), ("Plan", "Planned"));
        assert_eq!(tooltip_text(&plain, false, &EN), "Plan for Winter 2026/27 (P)");
        assert_eq!(tooltip_text(&plain, true, &EN), "Planned for Winter 2026/27. Once more removes the module from the plan (P)");
        assert_eq!(hero_note(&fill, None, &EN), "WS 26/27 · for \u{201c}Fachübergreifendes Studium\u{201d} · planned: SS 27");
        assert_eq!(hero_note(&later, Some(key("2025W")), &EN), "SS 27 · 4th sem.");
        assert_eq!(tooltip_text(&fill, false, &EN), "Plan for Winter 2026/27, for \u{201c}Fachübergreifendes Studium\u{201d} · planned: Summer 2027 (P)");
    }

    #[test]
    fn a_swiped_row_says_what_the_swipe_does() {
        let now = key("2026W");
        let mut doc = PlanDoc::default();
        doc.placeholders.push(placeholder(3, "2026W", "Fachübergreifendes Studium"));
        // Not planned: „Einplanen" into the semester the switch aims at, then „Eingeplant".
        let plain = aim_of("12330", now, now, Some(now), offer(None), None, &doc);
        assert_eq!(swipe_words(&plain, &DE), ("Einplanen", "WiSe 2026/27".to_string(), "Eingeplant"));
        assert_eq!(swipe_words(&plain, &EN), ("Plan", "Winter 2026/27".to_string(), "Planned"));
        // For the finder's placeholder, which the line names as the switch's does.
        let fill = aim_of("12330", now, now, Some(now), offer(None), Some(&PlanHint { semester: None, fill: Some(3) }), &doc);
        assert_eq!(swipe_words(&fill, &DE).1, "WiSe 2026/27 · für „Fachübergreifendes Studium“");
        // Planned there: the swipe takes it out, and says out of which semester.
        assert!(doc.plan(now, "12330", 1, None));
        let planned = aim_of("12330", now, now, Some(now), offer(None), None, &doc);
        assert!(planned.pressed());
        assert_eq!(swipe_words(&planned, &DE), ("Entfernen", "aus WiSe 2026/27".to_string(), "Entfernt"));
        assert_eq!(swipe_words(&planned, &EN), ("Remove", "from Winter 2026/27".to_string(), "Removed"));
    }

    #[test]
    fn a_click_plans_counts_for_the_placeholder_and_takes_out() {
        let now = key("2026W");
        let mut doc = PlanDoc::default();
        doc.placeholders.push(placeholder(3, "2026W", "Fachübergreifendes Studium"));
        // Unpressed: planned into the semester aimed at.
        let aim = aim_of("12330", now, now, Some(now), offer(None), None, &doc);
        toggle_in(&mut doc, "12330", &aim, false, &[], 7);
        assert!(doc.is_planned(now, "12330") && doc.fillers(3).is_empty());
        // For the placeholder, a module planned there already starts counting for it.
        let hint = PlanHint { semester: None, fill: Some(3) };
        let aim = aim_of("12330", now, now, Some(now), offer(None), Some(&hint), &doc);
        assert!(!aim.pressed);
        toggle_in(&mut doc, "12330", &aim, false, &[], 8);
        assert_eq!(doc.fillers(3).iter().map(|m| (m.module_id.as_str(), m.at)).collect::<Vec<_>>(), [("12330", 7)]);
        assert!(aim_of("12330", now, now, Some(now), offer(None), Some(&hint), &doc).pressed);
        // Without a hint, a click on what is planned elsewhere does not touch what it counts for.
        let aim = Aim { target: now, pressed: false, fill: None, elsewhere: Vec::new() };
        toggle_in(&mut doc, "12330", &aim, false, &[], 9);
        assert_eq!(doc.fillers(3).len(), 1);
        // Pressed: out of the semester, with what the semester hid of its own events.
        doc.set_event(now, 150132, true);
        doc.set_event(now, 149408, true);
        toggle_in(&mut doc, "12330", &aim, true, &[150132], 10);
        assert!(!doc.is_planned(now, "12330"));
        assert_eq!(doc.selection(now, TownChoice::Derive).hidden_events.into_iter().collect::<Vec<_>>(), [149408]);
    }

    fn row(module: &str, event: &str) -> DateRow {
        DateRow {
            module_id: module.into(),
            ord: Some(1),
            cancelled_dates: None,
            date: folia_model::rows_detail::EventDate {
                semester_key: "2026W".into(),
                semester_label: "WiSe 2026/27".into(),
                event_id: event.into(),
                event_number: None,
                event_title: "Übung".into(),
                event_type: None,
                group_name: None,
                weekday: None,
                start_time: None,
                end_time: None,
                rhythm: None,
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
            },
        }
    }

    /// konzept.html, „Mehrere auf einmal": each where it aims, or all into one semester; what is
    /// there already stays and is counted, what is planned ahead moves along, a retake is planned
    /// anew.
    #[test]
    fn several_modules_are_planned_at_once() {
        let now = key("2026W");
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key("2027S"), "12104", 7, Some(3)));
        assert!(doc.plan(key("2025W"), "11103", 5, None));
        assert!(doc.plan(key("2026W"), "13566", 9, None));
        // Each into its own semester: what is planned there already stays.
        let mut each = doc.clone();
        let picks = vec![("13566".to_string(), now), ("14642".to_string(), key("2027S")), ("bad id".to_string(), now)];
        let (into, there) = plan_picks(&mut each, &picks, now, false, 42);
        assert_eq!((into, there), (vec![("14642".to_string(), key("2027S"), None)], 1));
        // All into the summer: the one planned ahead in the winter moves there, the retake from a
        // semester behind is planned anew, and the one in the summer already stays.
        let mut all = doc.clone();
        let summer = key("2027S");
        let picks: Vec<(String, SemesterKey)> = ["13566", "11103", "12104"].iter().map(|id| (id.to_string(), summer)).collect();
        let (into, there) = plan_picks(&mut all, &picks, now, true, 42);
        assert_eq!(into, vec![("13566".to_string(), summer, Some(now)), ("11103".to_string(), summer, None)]);
        assert_eq!(there, 1);
        assert!(!all.is_planned(now, "13566") && all.is_planned(summer, "13566"));
        assert_eq!(all.modules.iter().find(|m| m.semester == summer && m.module_id == "13566").map(|m| m.at), Some(9));
        assert!(all.is_planned(key("2025W"), "11103") && all.is_planned(summer, "11103"));
        let planned = PlannedMany { into, there, before: doc.clone(), after: all.clone() };
        assert_eq!(planned.by_semester(), vec![(summer, 2)]);
    }

    #[test]
    fn taking_a_module_out_keeps_what_another_module_shares() {
        let own = [row("12102", "148008"), row("12102", "148008"), row("12102", "149408"), row("12102", "148455"), row("12102", "no id")];
        let others = [row("11112", "148455")];
        assert_eq!(events_alone(&own, &others), [148008, 149408]);
        assert_eq!(events_alone(&own, &[]), [148008, 148455, 149408]);
    }
}
