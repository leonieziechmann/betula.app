//! The Studienplan in this browser: its store, and where „Einplanen" plans a module to.
//!
//! What a visitor plans is theirs alone (R20, owner decision 2026-09-24): the plan lives in this
//! browser's `localStorage` (`betula.studyplan.v1`, the text of `catalog::studyplan::PlanDoc`) and
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
//! „Einplanen" (`PlanButton`) plans a module from its preview and its page: into the semester
//! `target_semester` aims at, and for the placeholder the finder was asked for (`PlanHint`). A row
//! of the catalog swiped to the right on a phone presses it too (`crate::swipe`, `press`).

use std::collections::BTreeSet;

use catalog::labels::{Code, TurnusSeason};
use catalog::queries;
use catalog::rows_detail::DateRow;
use catalog::studyplan::{PlanDoc, SavedPlans};
use catalog::timetable::select::{Selection, TownChoice};
use catalog::timetable::semester::{fachsemester, SemesterKey};
use catalog::url::{self, LocalView, ModuleHint, StudyplanUrl};
use leptos::prelude::*;

use crate::data::{use_source, Source};
use crate::i18n::{self, Texts};
use crate::myprogram::MyProgram;
use crate::nav;
use crate::ui::{Icon, Shortcut};

const STORAGE_KEY: &str = "betula.studyplan.v1";

/// The browser app (`csr`): only there is a plan, and only there does a hint aim the button.
const APP: bool = cfg!(feature = "csr");

/// The visitor's Studienplan, shared through context. Empty on the server, always.
#[derive(Clone, Copy)]
pub struct Studyplan(RwSignal<PlanDoc>);

impl Studyplan {
    /// Reads what this browser has stored and provides the plan. Another tab of the same browser
    /// may change it; this one follows.
    pub fn provide() -> Self {
        let plan = Studyplan(RwSignal::new(load()));
        provide_context(plan);
        // Effects run in the browser only.
        Effect::new(move |_| {
            let handle = window_event_listener_untyped("storage", move |_| {
                let stored = load();
                if plan.0.with_untracked(|doc| *doc != stored) {
                    plan.0.set(stored);
                }
            });
            on_cleanup(move || handle.remove());
        });
        plan
    }

    pub fn expect() -> Option<Self> {
        use_context::<Studyplan>()
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
    /// changed; an empty plan takes the key out of the storage (`nav::local_set`).
    pub fn update<R>(self, f: impl FnOnce(&mut PlanDoc) -> R) -> R {
        let mut doc = self.0.try_with_untracked(Clone::clone).unwrap_or_default();
        let result = f(&mut doc);
        if self.0.try_with_untracked(|now| *now != doc).unwrap_or(false) {
            nav::local_set(STORAGE_KEY, &stored_text(&doc));
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
}

const SAVED_KEY: &str = "betula.studyplan.saved.v1";

/// The plans saved in this browser („Plan speichern", `catalog::studyplan::SavedPlans`): kept like
/// the plan (R20: in `localStorage` alone, never in an address or a request, read like anything
/// from outside). Only the Stundenplan's sidebar reads them, so it makes the store itself; another
/// tab's change follows.
#[derive(Clone, Copy)]
pub struct Saved(RwSignal<SavedPlans>);

impl Saved {
    /// Reads what this browser has saved; follows another tab from then on.
    pub fn open() -> Self {
        let saved = Saved(RwSignal::new(load_saved()));
        Effect::new(move |_| {
            let handle = window_event_listener_untyped("storage", move |_| {
                let stored = load_saved();
                if saved.0.with_untracked(|plans| *plans != stored) {
                    saved.0.set(stored);
                }
            });
            on_cleanup(move || handle.remove());
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
            nav::local_set(SAVED_KEY, &plans.stored());
            self.0.set(plans);
        }
        result
    }
}

fn load_saved() -> SavedPlans {
    nav::local_get(SAVED_KEY).as_deref().map(SavedPlans::restored).unwrap_or_default()
}

/// What this browser has stored. Nothing on the server, and nothing in a browser that refuses
/// storage (a plan then lasts as long as the page).
fn load() -> PlanDoc {
    restored(nav::local_get(STORAGE_KEY).as_deref())
}

/// The plan of a stored text, read like anything from outside: garbage is dropped line by line,
/// never an error, and no text reads as more than the caps allow.
fn restored(stored: Option<&str>) -> PlanDoc {
    stored.map(PlanDoc::restored).unwrap_or_default()
}

/// How many modules a plan holds, each once however many semesters it is planned into (a retake):
/// the number the rail's „Plan" shows.
fn planned_modules(doc: &PlanDoc) -> usize {
    doc.modules.iter().map(|planned| planned.module_id.as_str()).collect::<BTreeSet<_>>().len()
}

/// The text a plan is stored as. Empty only when there is nothing at all to keep, and then the key
/// goes (`nav::local_set`): a plan without modules may still hold hidden Termine, a subscription's
/// code or a newer build's lines, which must survive (`PlanDoc::is_empty` is not the test).
fn stored_text(doc: &PlanDoc) -> String {
    doc.stored()
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

/// The semester „Einplanen" plans a module into (A.9), the first that applies: the page's hint
/// (the semester of the placeholder it fills while the plan still has it, else the semester the
/// finder checked); the semester of the module's newest events, when that is not over; the next
/// semester whose half of the year the module is offered in; the current one.
///
/// The placeholder wins over the finder's semester: a module planned for a placeholder fills it
/// (`fills=<pid>`, A.7), which means something only in the placeholder's own semester. The
/// finder's switch checks that semester too, also when it is turned off and on again while `fill`
/// stays in the address; the two differ only in an address written by hand.
pub fn target_semester(current: SemesterKey, newest: Option<SemesterKey>, turnus: Option<TurnusSeason>, hint: Option<&PlanHint>, doc: &PlanDoc) -> SemesterKey {
    if let Some(hint) = hint {
        let filled = hint.fill.and_then(|pid| doc.placeholders.iter().find(|p| p.pid == pid)).map(|p| p.semester);
        if let Some(semester) = filled.or(hint.semester) {
            return semester;
        }
    }
    if let Some(newest) = newest.filter(|newest| *newest >= current) {
        return newest;
    }
    let winter = match turnus {
        Some(TurnusSeason::Winter) => Some(true),
        Some(TurnusSeason::Summer) => Some(false),
        _ => None,
    };
    match winter {
        Some(winter) if winter != current.winter => current.plus(1).unwrap_or(current),
        _ => current,
    }
}

/// Where a plan button stands decides what it looks like.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanLook {
    /// In the line of a module's badges beside „Merken", in the preview and on the module's page:
    /// the switch with its label and its shortcut (R10), and in the app a small line of what the
    /// plan adds (`hero_note`).
    Hero,
    /// Among the actions of a module page's sidebar: the semester under the label, and „Anderes
    /// Semester" for the other semesters.
    Action,
}

/// „Einplanen": plans the module into the semester `target_semester` aims at — for the hint's
/// placeholder, when the finder was asked for one — and a second click takes it out again. Same
/// behaviour in every look; `P` presses the one of what the visitor is at (`enhance.js`,
/// `data-action="plan"`).
///
/// Part of server HTML like `MarkButton`: unpressed and aimed as for an empty plan, the same for
/// everybody (R9), kept in its place but not shown until the app runs (`.plan-toggle`), so nothing
/// moves at the takeover (R15). A click flips the button at once and writes the plan after the
/// next frame (R21): what follows from the plan (the week beside the module, the finder, the
/// lists) is computed after the button has answered.
///
/// `current` is the snapshot's current semester, `newest` the semester of the module's newest
/// teaching events (`target_semester`).
#[component]
pub fn PlanButton(
    #[prop(into)] id: String,
    #[prop(into)] title: String,
    turnus: Option<Code<TurnusSeason>>,
    current: SemesterKey,
    newest: Option<SemesterKey>,
    #[prop(optional, into)] hint: Signal<Option<PlanHint>>,
    look: PlanLook,
) -> impl IntoView {
    let t = i18n::t();
    let plan = Studyplan::expect().filter(|_| APP);
    let source = use_source().ok();
    let turnus = turnus.and_then(|turnus| turnus.known());
    // One memo per button (R5), from the plan and the hint alone.
    let aim = {
        let id = id.clone();
        Memo::new(move |_| match plan {
            Some(plan) => {
                let hint = hint.get();
                plan.with(|doc| aim_of(&id, current, newest, turnus, hint.as_ref(), doc))
            }
            None => aim_of(&id, current, newest, turnus, None, &PlanDoc::default()),
        })
    };
    // What the last click said, until the plan it writes after the next frame has it.
    let said = RwSignal::new(None::<bool>);
    let pressed = Memo::new(move |_| said.get().unwrap_or_else(|| aim.with(|aim| aim.pressed)));
    let toggle = {
        let id = id.clone();
        move |_: leptos::ev::MouseEvent| {
            let Some(plan) = plan else { return };
            let (was, aim) = (pressed.get_untracked(), aim.get_untracked());
            said.set(Some(!was));
            press(plan, source.clone(), id.clone(), aim, was, move || {
                said.try_set(None);
            });
        }
    };
    // The label reads `pressed` alone; the other texts read the click's word and the aim, never
    // `pressed` with the aim it comes from (R16).
    let label = move || label_text(pressed.get(), t);
    let tip = move || {
        let said = said.get();
        aim.with(|aim| tooltip_text(aim, said.unwrap_or(aim.pressed), t))
    };
    let pressed_attr = move || if pressed.get() { "true" } else { "false" };
    let busy = move || said.get().map(|_| "true");
    let icon = move || match pressed.get() {
        true => view! { <Icon name="calendar-check-2"/> }.into_any(),
        false => view! { <Icon name="calendar-plus"/> }.into_any(),
    };
    match look {
        PlanLook::Hero => {
            let note = move || aim.with(|aim| hero_note(aim, t)).map(|note| view! { <small>{note}</small> });
            view! {
                <button class="plan-toggle mark-switch hit" type="button" data-action="plan" on:click=toggle aria-pressed=pressed_attr aria-busy=busy title=tip>
                    {icon}<span>{label}</span>{note}<Shortcut keys="P"/>
                </button>
            }
            .into_any()
        }
        PlanLook::Action => {
            let line = move || aim.with(|aim| semester_line(aim, t));
            view! {
                <button class="plan-toggle action" type="button" data-action="plan" on:click=toggle aria-pressed=pressed_attr aria-busy=busy title=tip>
                    {icon}<span>{label}<small>{line}</small></span>
                </button>
                <OtherSemesters id title current/>
            }
            .into_any()
        }
    }
}

/// „Anderes Semester" in the sidebar of a module's page: the semesters the plan holds and the
/// current one with the four after it (`menu_semesters`), each a switch that plans the module
/// there or takes it out. The list is the browser app's; the button that opens it is part of
/// server HTML like the one above it, so the actions under it do not move at the takeover.
#[component]
fn OtherSemesters(id: String, title: String, current: SemesterKey) -> impl IntoView {
    let t = i18n::t();
    let plan = Studyplan::expect().filter(|_| APP);
    let mine = MyProgram::expect();
    let open = RwSignal::new(false);
    let entries = Memo::new(move |_| {
        let start = mine.and_then(MyProgram::start);
        let semesters = match plan {
            Some(plan) => plan.with(|doc| menu_semesters(current, doc)),
            None => menu_semesters(current, &PlanDoc::default()),
        };
        semesters.into_iter().map(|semester| (semester, semester_entry(semester, start, t))).collect::<Vec<_>>()
    });
    let list = move || {
        let id = id.clone();
        open.get().then(|| {
            view! {
                <div class="sp-sub" role="group" aria-label=(t.planner.semesters_of)(&title)>
                    <For each=move || entries.get() key=|entry| entry.clone() let:entry>
                        <SemesterSwitch id=id.clone() semester=entry.0 label=entry.1/>
                    </For>
                </div>
            }
        })
    };
    view! {
        <button class="plan-toggle action" type="button" aria-expanded=move || if open.get() { "true" } else { "false" } on:click=move |_| open.update(|open| *open = !*open)>
            <Icon name="calendar-range"/><span>{t.planner.other_semester}</span>
        </button>
        {list}
    }
}

/// One semester of „Anderes Semester": the module planned there or not, flipped at once and
/// written after the next frame (R21).
#[component]
fn SemesterSwitch(id: String, semester: SemesterKey, label: String) -> impl IntoView {
    let t = i18n::t();
    let plan = Studyplan::expect().filter(|_| APP);
    let source = use_source().ok();
    let planned = {
        let id = id.clone();
        Memo::new(move |_| plan.is_some_and(|plan| plan.is_planned(semester, &id)))
    };
    let said = RwSignal::new(None::<bool>);
    let pressed = Memo::new(move |_| said.get().unwrap_or_else(|| planned.get()));
    let toggle = move |_: leptos::ev::MouseEvent| {
        let Some(plan) = plan else { return };
        let was = pressed.get_untracked();
        said.set(Some(!was));
        let (id, source) = (id.clone(), source.clone());
        nav::after_paint(move || {
            if was {
                let events = plan.with_untracked(|doc| only_its_events(source.as_ref(), doc, semester, &id));
                plan.update(|doc| doc.unplan(semester, &id, &events));
            } else {
                plan.update(|doc| {
                    doc.plan(semester, &id, now(), None);
                });
            }
            said.try_set(None);
        });
    };
    let icon = move || match pressed.get() {
        true => view! { <Icon name="calendar-check-2"/> }.into_any(),
        false => view! { <Icon name="calendar-plus"/> }.into_any(),
    };
    // Where it is planned, said in words at the end of the line (the pill of `.action em`).
    let mark = move || pressed.get().then(|| view! { <em>{t.planner.planned_mark}</em> });
    view! {
        <button class="action" type="button" on:click=toggle aria-pressed=move || if pressed.get() { "true" } else { "false" } aria-busy=move || said.get().map(|_| "true")>
            {icon}<span>{label}</span>{mark}
        </button>
    }
}

/// What a plan button aims at: from the plan, the page's hint and the module's semesters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Aim {
    /// The semester a click plans into (`target_semester`).
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
    pub(crate) fn pressed(&self) -> bool {
        self.pressed
    }
}

/// What „Einplanen" aims at for a module now, asked once and not through a memo: for what presses
/// it without being its switch, a row swiped to the right (`crate::swipe`), which asks when the
/// finger starts. `current` and `newest` as for `PlanButton`.
pub(crate) fn aim_now(plan: Studyplan, id: &str, current: SemesterKey, newest: Option<SemesterKey>, turnus: Option<TurnusSeason>, hint: Option<&PlanHint>) -> Aim {
    plan.with_untracked(|doc| aim_of(id, current, newest, turnus, hint, doc))
}

fn aim_of(id: &str, current: SemesterKey, newest: Option<SemesterKey>, turnus: Option<TurnusSeason>, hint: Option<&PlanHint>, doc: &PlanDoc) -> Aim {
    let target = target_semester(current, newest, turnus, hint, doc);
    // A placeholder the plan no longer has asks for nothing, as in `target_semester`.
    let fill = hint.and_then(|hint| hint.fill).and_then(|pid| doc.placeholders.iter().find(|p| p.pid == pid)).map(|p| (p.pid, p.name.clone()));
    let pressed = match &fill {
        Some((pid, _)) => doc.modules.iter().any(|m| m.semester == target && m.module_id == id && m.fills == Some(*pid)),
        None => doc.is_planned(target, id),
    };
    let elsewhere = doc.planned_in(id).into_iter().filter(|semester| *semester != target).collect();
    Aim { target, pressed, fill, elsewhere }
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
/// server gave it (R15), a click only swaps the word. What a button says beyond it — the
/// placeholder it plans for, the other semesters the module is planned in — is its small line
/// (`hero_note`, `semester_line`), which gives way first where space is short.
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

/// The small line of the switch beside „Merken" („für „Anwendungsfach“ · geplant: SoSe 27"):
/// `None` for an empty plan without a hint, as on the server.
fn hero_note(aim: &Aim, t: &'static Texts) -> Option<String> {
    let parts: Vec<String> = [fill_text(aim, t), elsewhere_text(aim, |key| key.short(t.locale), t)].into_iter().flatten().collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// The line under the sidebar's label: the semester a click plans into, the placeholder it plans
/// for, and where else the module is planned.
fn semester_line(aim: &Aim, t: &'static Texts) -> String {
    [Some(aim.target.label(t.locale)), fill_text(aim, t), elsewhere_text(aim, |key| key.label(t.locale), t)].into_iter().flatten().collect::<Vec<_>>().join(" · ")
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

/// What a row swiped to the right says (`crate::swipe`), as (word, line, word once done): the
/// switch's „Einplanen" with the semester it plans into and the placeholder it plans for, or
/// „Entfernen" with the semester it takes the module out of — what the swipe does, where the
/// switch's label says what the module is.
pub(crate) fn swipe_words(aim: &Aim, t: &'static Texts) -> (&'static str, String, &'static str) {
    let semester = aim.target.label(t.locale);
    if aim.pressed {
        return (t.planner.remove, (t.planner.out_of)(&semester), t.planner.removed);
    }
    let line = [Some(semester), fill_text(aim, t)].into_iter().flatten().collect::<Vec<_>>().join(" · ");
    (t.planner.plan, line, t.planner.planned)
}

/// „Einplanen" pressed, by its switch or by a swiped row: `aim` as it was when pressed, and
/// whether the module was planned there (`was`). The plan is written after the next frame (R21:
/// what was pressed has answered by then), with what taking the module out takes along of the
/// semester's own choices; `then` runs once the plan has it.
pub(crate) fn press(plan: Studyplan, source: Option<Source>, id: String, aim: Aim, was: bool, then: impl FnOnce() + 'static) {
    nav::after_paint(move || {
        let events = if was { plan.with_untracked(|doc| only_its_events(source.as_ref(), doc, aim.target, &id)) } else { Vec::new() };
        plan.update(|doc| toggle_in(doc, &id, &aim, was, &events, now()));
        then();
    });
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

/// The events of a module that no other module planned into `semester` links: what taking it out
/// of the semester takes along of what the semester hides and has chosen (B.2). Asked of the local
/// catalog after the click has answered, with the questions the week beside the module asks, so
/// the answers are the visit's. Nothing when the catalog cannot say, which keeps those lines (the
/// safe direction).
fn only_its_events(source: Option<&Source>, doc: &PlanDoc, semester: SemesterKey, id: &str) -> Vec<u32> {
    let Some(source) = source else { return Vec::new() };
    let key = semester.key();
    let own = [id.to_string()];
    let others: Vec<String> = doc.modules_in(semester).into_iter().filter(|other| other != id).collect();
    source
        .run(|db| {
            let mine = [queries::modules_schedule(db, &own, &key)?, queries::modules_exams(db, &own, &key)?].concat();
            let theirs = [queries::modules_schedule(db, &others, &key)?, queries::modules_exams(db, &others, &key)?].concat();
            Ok(events_alone(&mine, &theirs))
        })
        .unwrap_or_default()
}

/// The events of `own` rows that none of `others` has, each once.
fn events_alone(own: &[DateRow], others: &[DateRow]) -> Vec<u32> {
    let theirs: BTreeSet<&str> = others.iter().map(|row| row.date.event_id.as_str()).collect();
    let alone: BTreeSet<u32> = own.iter().filter(|row| !theirs.contains(row.date.event_id.as_str())).filter_map(|row| row.date.event_id.parse().ok()).collect();
    alone.into_iter().collect()
}

/// The semesters „Anderes Semester" offers: those the plan holds, and the current one with the
/// four after it, in order.
pub fn menu_semesters(current: SemesterKey, doc: &PlanDoc) -> Vec<SemesterKey> {
    let mut all: BTreeSet<SemesterKey> = doc.semesters().into_iter().collect();
    all.extend((0..=4).filter_map(|n| current.plus(n)));
    all.into_iter().collect()
}

/// „3. FS · SoSe 2027" with a known Studienbeginn (numbers first), else „SoSe 2027".
fn semester_entry(semester: SemesterKey, start: Option<SemesterKey>, t: &'static Texts) -> String {
    match start.and_then(|start| fachsemester(semester, start)) {
        Some(fs) => (t.planner.semester_of_study)(fs, &semester.label(t.locale)),
        None => semester.label(t.locale),
    }
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

/// The Studienplan's address as a local view (`url::LocalView`, `crate::local`), so that `open`
/// and `full` mean what they mean on every page that has them: `open=<id>` is the module beside
/// the plan, and `full=1` lets it fill the plan's place with the module's whole page, inside the
/// Studienplan's area (tab, history and „Zurück" stay the plan's, the catalog's tab never hears of
/// it). What stands beside the plan is the plan's own panel of the module (its Termine and what is
/// chosen of them), not the module's preview, so on a phone `open` alone shows that panel as the
/// page and only `full` fills it with the module: the plan asks `local::filling` as a desktop
/// does. `StudyplanUrl` says the rest; `full` is written last.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanAddress {
    pub url: StudyplanUrl,
    /// The module of `open` fills the page. Nothing without `open`.
    pub full: bool,
}

impl PlanAddress {
    /// Tolerant like every address: `full` only as `full=1` and only with a module in `open`.
    pub fn parse(raw_query: &str) -> Self {
        let url = StudyplanUrl::parse(raw_query);
        let (_, full) = url::local_from_pairs(&url::parse_pairs(raw_query));
        Self { full: full && url.open.is_some(), url }
    }

    pub fn path(&self) -> String {
        let path = self.url.path();
        match (self.full, &self.url.open) {
            (true, Some(_)) => format!("{path}&full=1"),
            _ => path,
        }
    }
}

impl LocalView for PlanAddress {
    fn open(&self) -> Option<&str> {
        self.url.open.as_deref()
    }

    fn full(&self) -> bool {
        self.full
    }

    /// The Termin the panel pointed at stays while the module does.
    fn with_module(&self, open: Option<&str>, full: bool) -> Self {
        let url = if open == self.url.open.as_deref() { self.url.clone() } else { self.url.with_open(open, None) };
        Self { full: full && url.open.is_some(), url }
    }

    fn path(&self) -> String {
        PlanAddress::path(self)
    }
}

#[cfg(test)]
mod tests {
    use catalog::url::PlanView;

    use super::*;
    use crate::i18n::{DE, EN};

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    #[test]
    fn what_is_stored_is_read_like_anything_from_outside() {
        // Nothing stored, or garbage: an empty plan, never an error.
        assert!(restored(None).is_empty());
        let garbage = restored(Some("<script>alert(1)</script>\nm\t2026X\t12104\t0\t\nm\t2026W\t../../etc\t0\t\n\u{0}\u{7f}\n"));
        assert!(garbage.is_empty() && garbage.modules.is_empty() && garbage.placeholders.is_empty());
        // A good line among bad ones survives alone.
        let mixed = restored(Some("m\t2026W\t12104\t1790000000\t\nm\tWiSe\t12107\t0\t\ne\t2026W\t0\n"));
        assert_eq!(mixed.modules_in(key("2026W")), vec!["12104".to_string()]);
        assert!(!mixed.is_empty());
    }

    #[test]
    fn an_empty_plan_takes_its_key_out_and_a_plan_without_modules_keeps_what_it_hides() {
        // Nothing at all: the empty text, which `nav::local_set` turns into removing the key.
        assert_eq!(stored_text(&PlanDoc::default()), "");
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key("2026W"), "12104", 1, None));
        doc.set_event(key("2026W"), 149408, true);
        assert!(stored_text(&doc).contains("m\t2026W\t12104") && restored(Some(&stored_text(&doc))) == doc);
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

    #[test]
    fn einplanen_aims_at_the_semester_that_fits() {
        let now = key("2026W");
        let doc = PlanDoc::default();
        // The page's hint wins, over the module's own data.
        let hint = PlanHint { semester: Some(key("2027S")), fill: None };
        assert_eq!(target_semester(now, Some(now), Some(TurnusSeason::Winter), Some(&hint), &doc), key("2027S"));
        // Events of the current semester or a later one: that semester.
        assert_eq!(target_semester(now, Some(now), Some(TurnusSeason::Summer), None, &doc), now);
        assert_eq!(target_semester(now, Some(key("2027S")), None, None, &doc), key("2027S"));
        // Only past events: the next semester of its half of the year.
        assert_eq!(target_semester(now, Some(key("2026S")), Some(TurnusSeason::Summer), None, &doc), key("2027S"));
        assert_eq!(target_semester(now, Some(key("2026S")), Some(TurnusSeason::Winter), None, &doc), now);
        // Every semester, irregular or unknown: the current one.
        assert_eq!(target_semester(now, Some(key("2026S")), Some(TurnusSeason::Both), None, &doc), now);
        assert_eq!(target_semester(now, None, Some(TurnusSeason::Irregular), None, &doc), now);
        assert_eq!(target_semester(now, None, None, None, &doc), now);
        // No data, a winter module, in a winter: the current winter.
        assert_eq!(target_semester(now, None, Some(TurnusSeason::Winter), None, &doc), now);
        // A summer: the winter to come.
        assert_eq!(target_semester(key("2027S"), None, Some(TurnusSeason::Winter), None, &doc), key("2027W"));
        // A placeholder's hint without a semester plans into the placeholder's semester.
        let mut with_placeholder = PlanDoc::default();
        with_placeholder.placeholders.push(catalog::studyplan::Placeholder {
            pid: 3,
            semester: key("2027W"),
            program_id: "079-82-2008".into(),
            ord: 17,
            span: (3, 3),
            credits: Some("6".into()),
            kind: None,
            caption: String::new(),
            name: "Fachübergreifendes Studium".into(),
        });
        let fill = PlanHint { semester: None, fill: Some(3) };
        assert_eq!(target_semester(now, Some(now), None, Some(&fill), &with_placeholder), key("2027W"));
        // The finder checked another semester („Passt in meinen Stundenplan" off and on again,
        // `fill` kept): the placeholder still decides, so the module lands where it fills it.
        let both = PlanHint { semester: Some(now), fill: Some(3) };
        assert_eq!(target_semester(now, Some(now), None, Some(&both), &with_placeholder), key("2027W"));
        // One the plan no longer has changes nothing: the finder's semester, else the module's.
        let gone = PlanHint { semester: None, fill: Some(9) };
        assert_eq!(target_semester(now, Some(now), None, Some(&gone), &with_placeholder), now);
        let gone_but_checked = PlanHint { semester: Some(key("2027S")), fill: Some(9) };
        assert_eq!(target_semester(now, Some(now), None, Some(&gone_but_checked), &with_placeholder), key("2027S"));
    }

    #[test]
    fn the_plan_is_a_local_view() {
        let beside = PlanAddress::parse("sem=2026W&open=12104&row=148369-aaf38");
        assert!(!beside.full && beside.url.row.as_deref() == Some("148369-aaf38"));
        // „Vollbild": the same page filled with the module; the Termin pointed at stays.
        let full = beside.with_full(true);
        assert_eq!(full.path(), "/studyplan?sem=2026W&open=12104&row=148369-aaf38&full=1");
        assert_eq!(PlanAddress::parse("sem=2026W&open=12104&row=148369-aaf38&full=1"), full);
        assert_eq!(full.with_full(false), beside);
        // Another module: its own Termine, no row of the one before.
        assert_eq!(beside.with_open(Some("12107")).path(), "/studyplan?sem=2026W&open=12107");
        assert_eq!(beside.with_open(None).path(), "/studyplan?sem=2026W");
        // `full` is nothing without a module, nor anything but `full=1`.
        assert_eq!(PlanAddress::parse("view=dates&full=1"), PlanAddress { url: StudyplanUrl { view: PlanView::Dates, ..Default::default() }, full: false });
        assert!(!PlanAddress::parse("open=12104&full=yes").full);
        assert!(!PlanAddress::parse("open=<x>&full=1").full);
        // Only „Vollbild" fills the plan's page, on a phone as well: `open` alone is the panel.
        assert_eq!(crate::local::filling(&full, false).as_deref(), Some("12104"));
        assert_eq!(crate::local::filling(&beside, false), None);
        assert_eq!(crate::local::back_href(&full, false), "/studyplan?sem=2026W&open=12104&row=148369-aaf38");
    }

    fn placeholder(pid: u32, semester: &str, name: &str) -> catalog::studyplan::Placeholder {
        catalog::studyplan::Placeholder {
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
        // An empty plan: the module's own semester, unpressed, nothing more to say; what the
        // server renders.
        let plain = aim_of("12330", now, Some(now), None, None, &PlanDoc::default());
        assert_eq!((plain.target, plain.pressed, label_text(plain.pressed, &DE), hero_note(&plain, &DE)), (now, false, "Einplanen", None));
        assert_eq!(label_text(true, &DE), "Eingeplant");
        assert_eq!(tooltip_text(&plain, false, &DE), "In WiSe 2026/27 einplanen (P)");
        assert_eq!(tooltip_text(&plain, true, &DE), "Eingeplant in WiSe 2026/27. Noch einmal nimmt das Modul aus dem Plan (P)");
        assert_eq!(semester_line(&plain, &DE), "WiSe 2026/27");

        // Planned in another semester: said beside the semester it aims at, and under the label
        // of the switch in short.
        assert!(doc.plan(key("2027S"), "12330", 1, None));
        let elsewhere = aim_of("12330", now, Some(now), None, None, &doc);
        assert!(!elsewhere.pressed);
        assert_eq!(semester_line(&elsewhere, &DE), "WiSe 2026/27 · geplant: SoSe 2027");
        assert_eq!(hero_note(&elsewhere, &DE).as_deref(), Some("geplant: SoSe 27"));
        assert_eq!(tooltip_text(&elsewhere, false, &DE), "In WiSe 2026/27 einplanen · geplant: SoSe 2027 (P)");

        // For a placeholder: pressed only while the module counts for it; the label stays, the
        // small line names it.
        let hint = PlanHint { semester: None, fill: Some(3) };
        assert!(doc.plan(now, "12330", 1, None));
        let fill = aim_of("12330", now, Some(now), None, Some(&hint), &doc);
        assert_eq!((fill.pressed, label_text(fill.pressed, &DE)), (false, "Einplanen"));
        assert_eq!(hero_note(&fill, &DE).as_deref(), Some("für „Fachübergreifendes Studium“ · geplant: SoSe 27"));
        assert_eq!(semester_line(&fill, &DE), "WiSe 2026/27 · für „Fachübergreifendes Studium“ · geplant: SoSe 2027");
        assert_eq!(tooltip_text(&fill, false, &DE), "In WiSe 2026/27 einplanen, für „Fachübergreifendes Studium“ · geplant: SoSe 2027 (P)");
        // A long name is cut at a word in the small line; the tooltip keeps it whole.
        let long = aim_of("12330", now, Some(now), None, Some(&PlanHint { semester: None, fill: Some(4) }), &doc);
        assert_eq!(hero_note(&long, &DE).as_deref(), Some("für „wählbar aus dem…“ · geplant: SoSe 27"));
        assert!(tooltip_text(&long, false, &DE).contains("Prü/SL“"));
        // A placeholder the plan no longer has asks for nothing.
        let gone = aim_of("12330", now, Some(now), None, Some(&PlanHint { semester: None, fill: Some(9) }), &doc);
        assert_eq!((gone.fill.clone(), gone.pressed, label_text(gone.pressed, &DE)), (None, true, "Eingeplant"));

        // The same in English.
        assert_eq!((label_text(false, &EN), label_text(true, &EN)), ("Plan", "Planned"));
        assert_eq!(tooltip_text(&plain, false, &EN), "Plan for Winter 2026/27 (P)");
        assert_eq!(tooltip_text(&plain, true, &EN), "Planned for Winter 2026/27. Once more removes the module from the plan (P)");
        assert_eq!(hero_note(&fill, &EN).as_deref(), Some("for \u{201c}Fachübergreifendes Studium\u{201d} · planned: SS 27"));
        assert_eq!(semester_line(&elsewhere, &EN), "Winter 2026/27 · planned: Summer 2027");
        assert_eq!(tooltip_text(&fill, false, &EN), "Plan for Winter 2026/27, for \u{201c}Fachübergreifendes Studium\u{201d} · planned: Summer 2027 (P)");
    }

    #[test]
    fn a_swiped_row_says_what_the_swipe_does() {
        let now = key("2026W");
        let mut doc = PlanDoc::default();
        doc.placeholders.push(placeholder(3, "2026W", "Fachübergreifendes Studium"));
        // Not planned: „Einplanen" into the semester the switch aims at, then „Eingeplant".
        let plain = aim_of("12330", now, Some(now), None, None, &doc);
        assert_eq!(swipe_words(&plain, &DE), ("Einplanen", "WiSe 2026/27".to_string(), "Eingeplant"));
        assert_eq!(swipe_words(&plain, &EN), ("Plan", "Winter 2026/27".to_string(), "Planned"));
        // For the finder's placeholder, which the line names as the switch's does.
        let fill = aim_of("12330", now, Some(now), None, Some(&PlanHint { semester: None, fill: Some(3) }), &doc);
        assert_eq!(swipe_words(&fill, &DE).1, "WiSe 2026/27 · für „Fachübergreifendes Studium“");
        // Planned there: the swipe takes it out, and says out of which semester.
        assert!(doc.plan(now, "12330", 1, None));
        let planned = aim_of("12330", now, Some(now), None, None, &doc);
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
        let aim = aim_of("12330", now, Some(now), None, None, &doc);
        toggle_in(&mut doc, "12330", &aim, false, &[], 7);
        assert!(doc.is_planned(now, "12330") && doc.fillers(3).is_empty());
        // For the placeholder, a module planned there already starts counting for it.
        let hint = PlanHint { semester: None, fill: Some(3) };
        let aim = aim_of("12330", now, Some(now), None, Some(&hint), &doc);
        assert!(!aim.pressed);
        toggle_in(&mut doc, "12330", &aim, false, &[], 8);
        assert_eq!(doc.fillers(3).iter().map(|m| (m.module_id.as_str(), m.at)).collect::<Vec<_>>(), [("12330", 7)]);
        assert!(aim_of("12330", now, Some(now), None, Some(&hint), &doc).pressed);
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
            date: catalog::rows_detail::EventDate {
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
            },
        }
    }

    #[test]
    fn taking_a_module_out_keeps_what_another_module_shares() {
        let own = [row("12102", "148008"), row("12102", "148008"), row("12102", "149408"), row("12102", "148455"), row("12102", "no id")];
        let others = [row("11112", "148455")];
        assert_eq!(events_alone(&own, &others), [148008, 149408]);
        assert_eq!(events_alone(&own, &[]), [148008, 148455, 149408]);
    }

    #[test]
    fn another_semester_is_one_the_plan_holds_or_one_of_the_next() {
        let now = key("2026W");
        let mut doc = PlanDoc::default();
        assert_eq!(menu_semesters(now, &doc), ["2026W", "2027S", "2027W", "2028S", "2028W"].map(key));
        // A retake planned long before, and a semester further on, stand in order.
        assert!(doc.plan(key("2025W"), "12104", 1, None) && doc.plan(key("2029W"), "12104", 1, None));
        assert_eq!(menu_semesters(now, &doc), ["2025W", "2026W", "2027S", "2027W", "2028S", "2028W", "2029W"].map(key));
        // The Fachsemester first, when the Studienbeginn is known.
        assert_eq!(semester_entry(key("2027W"), Some(key("2026W")), &DE), "3. FS · WiSe 2027/28");
        assert_eq!(semester_entry(key("2025W"), Some(key("2026W")), &DE), "WiSe 2025/26");
        assert_eq!(semester_entry(key("2027S"), None, &DE), "SoSe 2027");
        assert_eq!(semester_entry(key("2027W"), Some(key("2026W")), &EN), "3rd sem. · Winter 2027/28");
        assert_eq!(semester_entry(key("2027S"), None, &EN), "Summer 2027");
    }
}
