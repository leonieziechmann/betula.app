//! Where „Einplanen" plans in this tab's session, what it says it did, and the question for a
//! study not set up (owner, 2026-10-18, concept E of folia/design/einplanen/oberflaeche.html after
//! the behaviour of verhalten.html: „Also ich liebe Option E, das ist mit Abstand die
//! eleganteste").
//!
//! - The semester of the session (P1, P2): the first time a session needs it, until 15 November the
//!   winter that runs and until 15 May the summer, after that the next semester (`planning_semester`).
//!   It stays until the tab closes (owner, 2026-10-18: „Wichtig ist aber, dass sich das nicht
//!   verändert in der aktuellen Session, das währe sehr disorientierend"), in the tab's
//!   `sessionStorage`; never before the snapshot's current semester.
//! - „Weitere Module auch nach …" (P3, `choose`): the semester the student chose for the next
//!   modules, for the session, from the note's second row; a past one, too.
//! - The note (`Note`): what a click, P or a swipe did, over the page at the bottom of the window
//!   (`folia_widgets::plannote`): the semester between ‹ and ›, which move the module at once
//!   (`step`), back into the past as well, to the first Fachsemester; and „Rückgängig" (`undo`).
//!   A module taken out says so, with „Rückgängig".
//! - The menu at the arrow beside „Eingeplant" (`MenuAsk`, `folia_widgets::planmenu`; owner,
//!   2026-10-20: „Mach das mal bei planen auch so. Dass man da das semester ändern kann aber zum
//!   beispiel auch das modul als bestanden markieren kann"): what is planned changes there without
//!   taking it out and planning it anew — moved to another semester (`move_planned`), marked as
//!   passed or no longer (`set_passed`), taken out (`remove`); each says so in the note, with
//!   „Rückgängig".
//! - The question for the study (`SetupAsk`): where no Studienbeginn is stored and the session has
//!   not put the question aside („Ohne Studium einplanen"), „Einplanen" opens a dialog that sets the
//!   study up and plans the module in one (`folia_widgets::plansetup`; owner, 2026-10-18: „so dass
//!   man im best case nur auf ok drücken muss, dann das Studium eingerichtet ist und dass Modul im
//!   richtigen Semester geplant ist").
//!
//! A change of the plan is written after the next frame (R21), the note is there at once.

use folia_calendar::day::Day;
use folia_calendar::semester::SemesterKey;
use folia_data::DataClient;
use folia_plans::study::Offer;
use folia_plans::studyplan::PlanDoc;
use leptos::prelude::*;

use super::{events_alone, move_in, now, only_its_events_ask, pass_in, toggle_in, Aim, PlanHint, Studyplan};
use crate::collections::Anchor;
use crate::myprogram::MyProgram;
use folia_design::nav;

/// The semester the session plans for (P1), kept in the tab's `sessionStorage` from its first use.
const SESSION_KEY: &str = "betula.plan.semester";
/// „Weitere Module auch nach …" (P3), for the session.
const CHOSEN_KEY: &str = "betula.plan.chosen";
/// The question for the study answered with „Ohne Studium einplanen", for the session.
const ASKED_KEY: &str = "betula.plan.asked";

/// How far ‹ goes back where the Studienbeginn is not known: six semesters before the current one.
const BACK_UNKNOWN: i32 = 6;

/// The module „Einplanen" was pressed for, as its switch or its row knows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Pressed {
    pub id: String,
    pub title: String,
    /// The snapshot's current semester: what lies before it is behind the student.
    pub current: SemesterKey,
    pub offer: Offer,
    /// The semester of the module's newest events: where a click aims without a semester chosen
    /// for the next modules (`target_semester`), which the dialog goes back to when that is cleared.
    pub newest: Option<SemesterKey>,
    /// The page's hint: a page that means a semester (the finder's, a placeholder's).
    pub hint: Option<PlanHint>,
}

/// What a note says was done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteKind {
    /// Planned into `at`; `stepped` once moved with ‹ ›.
    Planned { at: SemesterKey, stepped: bool },
    /// Taken out of `from`.
    Removed { from: SemesterKey },
    /// Marked as passed in `at`, or no longer (`passed`), from the switch's menu.
    Passed { at: SemesterKey, passed: bool },
}

/// The note of what „Einplanen" did last.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    /// Which press it answers: the plan it writes is this note's only while it is.
    pub turn: u32,
    /// Counts every change of the note (a step): what the page does right after one belongs to it
    /// (`folia_design::waitnote::WaitNote`, `changed`).
    pub shown: u32,
    pub what: Pressed,
    pub kind: NoteKind,
}

/// What „Rückgängig" puts back: the plan before the press, while the plan is what the note's last
/// write made of it.
#[derive(Clone, Debug, PartialEq)]
struct Undo {
    turn: u32,
    before: PlanDoc,
    after: PlanDoc,
}

/// The menu of a module planned, asked for at the arrow beside its switch, or with ⇧P
/// (`folia_widgets::planmenu`).
#[derive(Clone, Debug, PartialEq)]
pub struct MenuAsk {
    pub what: Pressed,
    /// The semester the switch is pressed for: where the module is planned.
    pub at: SemesterKey,
    /// The module is passed there.
    pub passed: bool,
    /// The box of the switch and its arrow: the menu stands under it.
    pub anchor: Option<Anchor>,
}

/// „Einplanen in …" asked for (folia/design/tasten/konzept.html, „Knopf mit Pfeil"; owner,
/// 2026-10-20: „Ja bitte setze deine Vorschläge um"): the same arrow, or ⇧P, at a module not planned.
/// The semesters it can go to, each with whether the module is offered then; `folia_widgets::
/// planinto` shows it, `Planning::place` does what is chosen.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanAsk {
    pub what: Pressed,
    /// Where the switch aims as the arrow is clicked.
    pub aim: Aim,
    /// The box of the switch and its arrow, where a wide screen puts the menu.
    pub at: Option<Anchor>,
}

/// „Einplanen" pressed where no study is set up: the dialog that sets it up and plans the module.
#[derive(Clone, Debug, PartialEq)]
pub struct SetupAsk {
    pub turn: u32,
    pub what: Pressed,
    /// Where the click aimed: where the dialog starts the module.
    pub target: SemesterKey,
    /// The placeholder the click planned for (the finder's): kept where the module goes there.
    pub fill: Option<u32>,
}

/// What „Einplanen" keeps for the tab's session, shared through context; provided once in the app,
/// after the Studienplan and „Mein Studiengang".
#[derive(Clone, Copy)]
pub struct Planning {
    mine: Option<MyProgram>,
    session: Option<SemesterKey>,
    chosen: RwSignal<Option<SemesterKey>>,
    asked: RwSignal<bool>,
    note: RwSignal<Option<Note>>,
    setup: RwSignal<Option<SetupAsk>>,
    menu: RwSignal<Option<MenuAsk>>,
    /// „Einplanen in …" asked for (`PlanAsk`).
    into: RwSignal<Option<PlanAsk>>,
    undo: StoredValue<Option<Undo>>,
    /// The press „Rückgängig" took back before its plan was written: that write is left out.
    undone: StoredValue<u32>,
    turns: StoredValue<u32>,
}

impl Planning {
    pub fn provide() -> Self {
        let planning = Planning {
            mine: MyProgram::expect(),
            session: session_semester(),
            chosen: RwSignal::new(nav::session_get(CHOSEN_KEY).as_deref().and_then(SemesterKey::parse)),
            asked: RwSignal::new(nav::session_get(ASKED_KEY).is_some()),
            note: RwSignal::new(None),
            setup: RwSignal::new(None),
            menu: RwSignal::new(None),
            into: RwSignal::new(None),
            undo: StoredValue::new(None),
            undone: StoredValue::new(0),
            turns: StoredValue::new(0),
        };
        provide_context(planning);
        // Another profile has another plan: what the note and the dialog were about is gone.
        if let Some(plan) = Studyplan::expect() {
            Effect::new(move |before: Option<u16>| {
                let profile = plan.profile();
                if before.is_some_and(|before| before != profile) {
                    planning.note.set(None);
                    planning.setup.set(None);
                    planning.menu.set(None);
                    planning.into.set(None);
                    planning.undo.set_value(None);
                }
                profile
            });
        }
        planning
    }

    pub fn expect() -> Option<Self> {
        use_context::<Planning>()
    }

    /// Where a click plans where the page means no semester (P1–P3): the semester chosen for the
    /// next modules, else the session's, never before `current`, the snapshot's current semester;
    /// and whether it was chosen, which the module's events then do not move (`target_semester`).
    /// Tracked: a memo per switch reads it.
    pub fn base(self, current: SemesterKey) -> (SemesterKey, bool) {
        let chosen = self.chosen.get();
        (base_of(chosen, self.session, current), chosen.is_some())
    }

    pub fn base_untracked(self, current: SemesterKey) -> (SemesterKey, bool) {
        let chosen = self.chosen.get_untracked();
        (base_of(chosen, self.session, current), chosen.is_some())
    }

    /// The semester the session plans for (P1): the snapshot's current one where the browser has
    /// no clock (the server).
    pub fn session(self, current: SemesterKey) -> SemesterKey {
        self.session.map_or(current, |session| session.max(current))
    }

    /// „Weitere Module auch nach …": the semester chosen. Tracked.
    pub fn chosen(self) -> Option<SemesterKey> {
        self.chosen.get()
    }

    /// The note now. Tracked.
    pub fn note(self) -> Option<Note> {
        self.note.get()
    }

    /// The same without subscribing: for what a key decides.
    pub fn note_untracked(self) -> Option<Note> {
        self.note.get_untracked()
    }

    /// The dialog asked for. Tracked.
    pub fn setup(self) -> Option<SetupAsk> {
        self.setup.get()
    }

    /// „Einplanen" pressed, by its switch, its key or a row swiped (E): taken out where it is
    /// planned, else planned where it aims, and the note says so; where no study is set up and the
    /// session has not put the question aside, the dialog asks first. `true` where the plan
    /// changes (the switch flips at once, and `then` runs once the plan has it), `false` where the
    /// dialog opened instead.
    pub fn press(self, plan: Studyplan, source: Option<DataClient>, what: Pressed, aim: Aim, then: impl FnOnce() + 'static) -> bool {
        if !aim.pressed && self.asks_first() {
            let turn = self.next_turn();
            self.note.set(None);
            let fill = aim.fill.as_ref().map(|(pid, _)| *pid);
            self.setup.set(Some(SetupAsk { turn, what, target: aim.target, fill }));
            return false;
        }
        self.write(plan, source, what, aim, then);
        true
    }

    /// „Einplanen in …", for the module of `ask` (another one open goes).
    pub fn open_into(self, ask: PlanAsk) {
        self.into.set(Some(ask));
    }

    /// „Einplanen in …" asked for. Tracked.
    pub fn into_menu(self) -> Option<PlanAsk> {
        self.into.get()
    }

    pub fn close_into(self) {
        if self.into.with_untracked(Option::is_some) {
            self.into.set(None);
        }
    }

    /// A semester chosen in „Einplanen in …" (`PlanAsk`): the module planned into `to`, and the note
    /// says so as after a click — with ‹ ›, and „Rückgängig" (owner, 2026-10-20: „Ja bitte setze
    /// deine Vorschläge um"). Where no study is set up, the dialog asks first, the module at `to`;
    /// a semester the module is planned in already changes nothing. `true` where the plan changes.
    pub fn place(self, plan: Studyplan, what: Pressed, to: SemesterKey) -> bool {
        self.close_into();
        if plan.with_untracked(|doc| doc.is_planned(to, &what.id)) {
            return false;
        }
        if self.asks_first() {
            let turn = self.next_turn();
            self.note.set(None);
            self.setup.set(Some(SetupAsk { turn, what, target: to, fill: None }));
            return false;
        }
        let turn = self.next_turn();
        self.note.set(Some(Note { turn, shown: turn, what: what.clone(), kind: NoteKind::Planned { at: to, stepped: false } }));
        self.undo.set_value(None);
        let id = what.id;
        nav::after_paint(move || {
            if self.undone.try_get_value() == Some(turn) {
                return;
            }
            let (before, after) = plan.update(|doc| {
                let before = doc.clone();
                doc.plan(to, &id, now(), None);
                (before, doc.clone())
            });
            if self.note.with_untracked(|note| note.as_ref().is_some_and(|note| note.turn == turn)) {
                self.undo.set_value(Some(Undo { turn, before, after }));
            }
        });
        true
    }

    /// Whether the study is to be asked for before a module is planned: no Studienbeginn is stored,
    /// and the session has not put the question aside.
    pub fn asks_first(self) -> bool {
        !self.asked.get_untracked() && self.mine.is_some_and(|mine| mine.start_untracked().is_none())
    }

    /// The dialog's answer: the module planned into `at` (with the study set up first, or without
    /// it, `aside`, which the session asks no more), and the note says so.
    pub fn plan_from_setup(self, plan: Studyplan, source: Option<DataClient>, ask: &SetupAsk, at: SemesterKey, aside: bool) {
        if aside {
            self.asked.set(true);
            nav::session_set(ASKED_KEY, "1");
        }
        self.setup.set(None);
        let fill = ask.fill.filter(|_| at == ask.target).map(|pid| (pid, String::new()));
        let aim = Aim { target: at, pressed: false, fill, elsewhere: Vec::new(), passed: None };
        self.write(plan, source, ask.what.clone(), aim, || {});
    }

    /// The dialog closed without an answer: the next „Einplanen" asks again.
    pub fn close_setup(self) {
        self.setup.set(None);
    }

    fn write(self, plan: Studyplan, source: Option<DataClient>, what: Pressed, aim: Aim, then: impl FnOnce() + 'static) {
        let turn = self.next_turn();
        let was = aim.pressed;
        let kind = if was { NoteKind::Removed { from: aim.target } } else { NoteKind::Planned { at: aim.target, stepped: false } };
        self.note.set(Some(Note { turn, shown: turn, what: what.clone(), kind }));
        self.undo.set_value(None);
        nav::after_paint(move || {
            let ask = (source.is_some() && was).then(|| plan.with_untracked(|doc| only_its_events_ask(doc, aim.target, &what.id)));
            let apply = move |events: Vec<u32>| {
                if self.undone.try_get_value() != Some(turn) {
                    let (before, after) = plan.update(|doc| {
                        let before = doc.clone();
                        toggle_in(doc, &what.id, &aim, was, &events, now());
                        (before, doc.clone())
                    });
                    if self.note.with_untracked(|note| note.as_ref().is_some_and(|note| note.turn == turn)) {
                        self.undo.set_value(Some(Undo { turn, before, after }));
                    }
                }
                then();
            };
            // Planned in: nothing to ask. Taken out: which of its events no other module has,
            // asked first (its rows and theirs), and nothing where the catalog cannot say.
            let (Some(source), Some(ask)) = (source, ask) else {
                apply(Vec::new());
                return;
            };
            leptos::task::spawn_local(async move {
                let events = source.ask(ask).await.map(|(mine, theirs)| events_alone(&mine, &theirs)).unwrap_or_default();
                apply(events);
            });
        });
    }

    /// „Rückgängig": the plan as it was before the press, while it is what the note made of it;
    /// changed besides (another tab), what the note did undone alone. Before its write, the write
    /// is left out.
    pub fn undo(self, plan: Studyplan) {
        let Some(note) = self.note.get_untracked() else { return };
        self.note.set(None);
        self.undone.set_value(note.turn);
        let undo = self.undo.get_value().filter(|undo| undo.turn == note.turn);
        self.undo.set_value(None);
        // Nothing written yet: nothing to take back.
        let Some(undo) = undo else { return };
        plan.update_after_paint(move |doc| {
            if *doc == undo.after {
                *doc = undo.before;
                return;
            }
            let id = note.what.id.as_str();
            match note.kind {
                NoteKind::Planned { at, .. } => {
                    if !undo.before.is_planned(at, id) {
                        doc.unplan(at, id, &[]);
                    }
                    // Moved with the menu: back where it was, with what was passed of it there.
                    for was in undo.before.modules.iter().filter(|m| m.module_id == id && m.semester != at) {
                        if !doc.is_planned(was.semester, id) {
                            doc.plan(was.semester, id, was.at, was.fills);
                        }
                    }
                    if let Some(passed) = undo.before.passed_in(id).filter(|_| doc.passed_in(id).is_none() || doc.passed_in(id) == Some(at)) {
                        doc.pass(passed, id);
                    }
                }
                NoteKind::Removed { from } => {
                    let was = undo.before.modules.iter().find(|m| m.semester == from && m.module_id == id).map(|m| (m.at, m.fills));
                    let (at, fills) = was.unwrap_or((now(), None));
                    doc.plan(from, id, at, fills);
                    if undo.before.passed_in(id) == Some(from) {
                        doc.pass(from, id);
                    }
                }
                NoteKind::Passed { .. } => match undo.before.passed_in(id) {
                    Some(was) => {
                        doc.pass(was, id);
                    }
                    None => doc.unpass(id),
                },
            }
        });
    }

    /// The menu of a module planned, at its switch's arrow. Tracked.
    pub fn menu(self) -> Option<MenuAsk> {
        self.menu.get()
    }

    pub fn open_menu(self, ask: MenuAsk) {
        self.menu.set(Some(ask));
    }

    pub fn close_menu(self) {
        if self.menu.with_untracked(Option::is_some) {
            self.menu.set(None);
        }
    }

    /// „Verschieben nach" in the switch's menu: the module planned in `from` goes to `to` at once,
    /// with what was passed of it there, and the note says where, with ‹ › and „Rückgängig", as
    /// after a press.
    pub fn move_planned(self, plan: Studyplan, what: Pressed, from: SemesterKey, to: SemesterKey) {
        if from == to {
            return;
        }
        let turn = self.next_turn();
        let id = what.id.clone();
        self.note.set(Some(Note { turn, shown: turn, what, kind: NoteKind::Planned { at: to, stepped: true } }));
        self.undo.set_value(None);
        self.change(plan, turn, move |doc| {
            move_in(doc, &id, from, to);
        });
    }

    /// „Als bestanden markieren" in the switch's menu, or „Bestanden zurücknehmen" (`passed`): in
    /// the semester the module is planned in, and the note says so, with „Rückgängig".
    pub fn set_passed(self, plan: Studyplan, what: Pressed, at: SemesterKey, passed: bool) {
        let turn = self.next_turn();
        let id = what.id.clone();
        self.note.set(Some(Note { turn, shown: turn, what, kind: NoteKind::Passed { at, passed } }));
        self.undo.set_value(None);
        self.change(plan, turn, move |doc| {
            pass_in(doc, &id, at, passed);
        });
    }

    /// „Entfernen" in the switch's menu: what a press of the switch does (`press`), without asking
    /// for the study.
    pub fn remove(self, plan: Studyplan, source: Option<DataClient>, what: Pressed, at: SemesterKey) {
        let aim = Aim { target: at, pressed: true, fill: None, elsewhere: Vec::new(), passed: None };
        self.write(plan, source, what, aim, || {});
    }

    /// Writes a change of the note `turn` after the next frame, and keeps the plan before and after
    /// it for „Rückgängig" while the note is that one; a change „Rückgängig" took back first is left
    /// out.
    fn change(self, plan: Studyplan, turn: u32, f: impl FnOnce(&mut PlanDoc) + 'static) {
        let (undo, undone, note) = (self.undo, self.undone, self.note);
        plan.update_after_paint(move |doc| {
            if undone.try_get_value() == Some(turn) {
                return;
            }
            let before = doc.clone();
            f(doc);
            if note.try_with_untracked(|note| note.as_ref().is_some_and(|note| note.turn == turn)) == Some(true) {
                undo.set_value(Some(Undo { turn, before, after: doc.clone() }));
            }
        });
    }

    /// ‹ or › in the note: the module planned moves a semester, at once; back to the first
    /// Fachsemester (`floor`), into the past as well.
    pub fn step(self, plan: Studyplan, by: i32) {
        let Some(note) = self.note.get_untracked() else { return };
        let NoteKind::Planned { at, .. } = note.kind else { return };
        let Some(to) = at.plus(by).filter(|to| *to >= self.floor(note.what.current)) else { return };
        let (id, turn) = (note.what.id.clone(), note.turn);
        let shown = self.next_turn();
        self.note.set(Some(Note { kind: NoteKind::Planned { at: to, stepped: true }, shown, ..note }));
        let undo = self.undo;
        plan.update_after_paint(move |doc| {
            move_in(doc, &id, at, to);
            undo.update_value(|undo| {
                if let Some(undo) = undo.as_mut().filter(|undo| undo.turn == turn) {
                    undo.after = doc.clone();
                }
            });
        });
    }

    /// The earliest semester ‹ reaches: the first Fachsemester, six back where the Studienbeginn is
    /// not known.
    pub fn floor(self, current: SemesterKey) -> SemesterKey {
        let start = self.mine.and_then(MyProgram::start_untracked).filter(|start| *start <= current);
        start.unwrap_or_else(|| current.plus(-BACK_UNKNOWN).unwrap_or(current))
    }

    /// The second row of the note: „Weitere Module auch nach `at`" on, or off where it was on.
    pub fn keep(self, at: SemesterKey) {
        let next = if self.chosen.get_untracked() == Some(at) { None } else { Some(at) };
        self.choose(next);
    }

    /// The semester the session plans the next modules into (P3), or the session's again.
    pub fn choose(self, semester: Option<SemesterKey>) {
        self.chosen.set(semester);
        nav::session_set(CHOSEN_KEY, &semester.map(SemesterKey::key).unwrap_or_default());
    }

    /// The note of the press `turn` goes: the student moved on, or put it away
    /// (`folia_widgets::plannote`). A newer note stays.
    pub fn dismiss(self, turn: u32) {
        if self.note.with_untracked(|note| note.as_ref().is_some_and(|note| note.turn == turn)) {
            self.note.set(None);
        }
    }

    /// Another note takes the place of this one's, whatever it says (the note after marking,
    /// `folia_widgets::marknote`): one note stands at a time.
    pub fn hush(self) {
        if self.note.with_untracked(Option::is_some) {
            self.note.set(None);
        }
    }

    fn next_turn(self) -> u32 {
        let turn = self.turns.get_value().wrapping_add(1);
        self.turns.set_value(turn);
        turn
    }
}

fn base_of(chosen: Option<SemesterKey>, session: Option<SemesterKey>, current: SemesterKey) -> SemesterKey {
    chosen.unwrap_or_else(|| session.map_or(current, |session| session.max(current)))
}

/// The semester the session plans for, as the tab keeps it; the first time, from the day.
fn session_semester() -> Option<SemesterKey> {
    if let Some(kept) = nav::session_get(SESSION_KEY).as_deref().and_then(SemesterKey::parse) {
        return Some(kept);
    }
    let semester = planning_semester(today()?)?;
    nav::session_set(SESSION_KEY, &semester.key());
    Some(semester)
}

/// P1: the semester a session that begins on `day` plans for: the one that runs (April to
/// September the summer, October to March the winter) until 15 May or 15 November, the next one
/// after that — who plans in the middle of a semester plans the next (owner, 2026-10-18: „wenn man
/// mitten im Semester plant, dann ist das wahrscheinlich für das nächste").
pub fn planning_semester(day: Day) -> Option<SemesterKey> {
    let (year, month, date) = day.ymd();
    let year = u16::try_from(year).ok()?;
    let running = match month {
        4..=9 => SemesterKey::new(year, false),
        10..=12 => SemesterKey::new(year, true),
        _ => SemesterKey::new(year.checked_sub(1)?, true),
    }?;
    let past_its_middle = match month {
        5 | 11 => date > 15,
        6..=9 | 12 | 1..=3 => true,
        _ => false,
    };
    if past_its_middle {
        running.plus(1)
    } else {
        Some(running)
    }
}

/// The browser's date. `None` on the server, which plans nothing and must not read a clock.
fn today() -> Option<Day> {
    #[cfg(feature = "csr")]
    {
        let now = web_sys::js_sys::Date::new_0();
        Day::from_ymd(i32::try_from(now.get_full_year()).ok()?, now.get_month() + 1, now.get_date())
    }
    #[cfg(not(feature = "csr"))]
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    fn day(y: i32, m: u32, d: u32) -> Day {
        Day::from_ymd(y, m, d).unwrap()
    }

    /// verhalten.html, P1: until 15 November the winter that runs, until 15 May the summer, then
    /// the next.
    #[test]
    fn a_session_plans_for_the_running_semester_until_its_middle() {
        assert_eq!(planning_semester(day(2026, 10, 1)), Some(key("2026W")));
        assert_eq!(planning_semester(day(2026, 10, 20)), Some(key("2026W")));
        assert_eq!(planning_semester(day(2026, 11, 15)), Some(key("2026W")));
        assert_eq!(planning_semester(day(2026, 11, 16)), Some(key("2027S")));
        assert_eq!(planning_semester(day(2027, 2, 9)), Some(key("2027S")));
        assert_eq!(planning_semester(day(2027, 4, 12)), Some(key("2027S")));
        assert_eq!(planning_semester(day(2027, 5, 15)), Some(key("2027S")));
        assert_eq!(planning_semester(day(2027, 5, 16)), Some(key("2027W")));
        assert_eq!(planning_semester(day(2027, 9, 30)), Some(key("2027W")));
    }

    #[test]
    fn the_chosen_semester_wins_and_the_sessions_is_never_behind() {
        let now = key("2026W");
        assert_eq!(base_of(None, None, now), now);
        assert_eq!(base_of(None, Some(key("2027S")), now), key("2027S"));
        // A session kept since a semester that is over: the current one.
        assert_eq!(base_of(None, Some(key("2026S")), now), now);
        // Chosen: also one behind the student (owner, 2026-10-18: „Betula sollte bei sowas nicht
        // nein sagen").
        assert_eq!(base_of(Some(key("2025W")), Some(key("2027S")), now), key("2025W"));
    }
}
