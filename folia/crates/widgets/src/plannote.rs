//! The note of what „Einplanen" did (owner, 2026-10-18, concept E of
//! folia/design/einplanen/oberflaeche.html: „Also ich liebe Option E, das ist mit Abstand die
//! eleganteste"), a note that waits for interaction (`folia_design::waitnote::WaitNote`; the same
//! day: „Bau diese notification mal bitte als wait for interaction notification"): it stands until
//! the student does something else, and goes then. In it a check, the semester between ‹ and › with
//! its Fachsemester and what is unusual there („kein Angebot", „liegt zurück"; where there is room
//! also what happened, „eingeplant", „verschoben"), and „Rückgängig". The arrows move the module at
//! once, into the past as well, to the first Fachsemester (← and → do too while the note stands,
//! its own keys); once it was moved, a second row offers „Weitere Module auch nach SoSe 27" with a
//! switch, and while the session plans into a semester chosen so it stands in every note, to switch
//! it off where it is seen (the same day: „die Option mit, die nächsten Module auch in das Semester
//! planen ist genial. Aber das Design von der Box muss vielleicht nochmal etwas angepasst werden").
//! A module taken out says so, with „Rückgängig", and one marked as passed in the switch's menu
//! (`planmenu`) or no longer.
//!
//! The semester and its arrows are one control (the same day, of a phone: „Erstmal müssen die Links
//! und rechts Buttons virtuell über den komplette text da gehen, also wenn man auf der rechten Seite
//! drückt, dann nach rechts und wenn man auf der linken Seite drückt nach links. Dazu muss auch
//! swiping unterstützt werden"): a tap on its left half is ‹, on its right half ›, and a finger
//! swiped sideways over it moves the module a semester the way a page turns (to the left the next
//! one; the note's `Sideways`). What it says fits between the arrows (the same day: „Wichtig finde
//! ich, dass das eingeplante Semester (also fs. 12) und die Warnung da steht. Der Rest kann optional
//! ausgeblendet werden"): the semester, its Fachsemester and the warning; what happened only where
//! there is room.
//!
//! On a wide screen the module is named in the bar (the same day: „bei der pc Version kann die
//! notification ruhig über die ganze Breite rechts von der sidebar gehen und dann vllt etwas weniger
//! hoch sein"); on a phone the switch is a row over the semester, so that what is under the finger
//! stays (app.css „The note of „Einplanen"", `.plan-toast`).

use folia_calendar::semester::{fachsemester, SemesterKey};
use leptos::ev::{KeyboardEvent, MouseEvent};
use leptos::prelude::*;

use folia_design::ui::Icon;
use folia_design::waitnote::{Sideways, WaitNote};
use folia_stores::myprogram::MyProgram;
use folia_stores::studyplan::{NoteKind, Planning, Pressed, Studyplan};

use crate::i18n::{self, Texts};

/// The note, once in the app (`folia_app::App`): nothing while there is none.
#[component]
pub fn PlanNote() -> impl IntoView {
    let (Some(planning), Some(plan)) = (Planning::expect(), Studyplan::expect()) else { return ().into_any() };
    let mine = MyProgram::expect();
    // A new press makes a new note; what changes on one changes inside it, so the focus stays.
    let turn = Memo::new(move |_| planning.note().map(|note| note.turn));
    (move || turn.get().map(|_| view! { <NoteView planning plan mine/> })).into_any()
}

/// One note: what it says changes in place (the arrows, the switch), so that the focus and the
/// pointer stay where they are.
#[component]
fn NoteView(planning: Planning, plan: Studyplan, mine: Option<MyProgram>) -> impl IntoView {
    let t = i18n::t();
    let Some(first) = planning.note_untracked() else { return ().into_any() };
    // This note goes, not a newer one that took its place.
    let turn = first.turn;
    let gone = Callback::new(move |()| planning.dismiss(turn));
    // Each change of the note by the student's hand (a step): what follows it belongs to it.
    let changed = Signal::derive(move || planning.note().map_or(0, |note| note.shown));
    // „Rückgängig" at the end, after a line.
    let end = view! {
        <div class="plan-toast-end">
            <span class="plan-toast-sep" aria-hidden="true"></span>
            <button class="plan-toast-undo" type="button" on:click=move |_| planning.undo(plan)>{t.common.undo}</button>
        </div>
    };
    match first.kind {
        // Marked as passed in the switch's menu, or no longer: said as „entfernt" is.
        NoteKind::Passed { passed, .. } => view! {
            <WaitNote on_gone=gone changed class="planning-note">
                <div class="plan-toast out">
                    <div class="plan-toast-main">
                        <span class="plan-toast-mark" class:out=!passed aria-hidden="true"><Icon name=if passed { "circle-check-big" } else { "rotate-ccw" }/></span>
                        <span class="plan-toast-said">
                            <span class="plan-toast-said-name">{first.what.title.clone()}</span>
                            <span class="plan-toast-said-what">" "{if passed { t.planner.note_passed } else { t.planner.note_unpassed }}</span>
                        </span>
                    </div>
                    {end}
                </div>
            </WaitNote>
        }
        .into_any(),
        NoteKind::Removed { .. } => view! {
            <WaitNote on_gone=gone changed class="planning-note">
                <div class="plan-toast out">
                    <div class="plan-toast-main">
                        <span class="plan-toast-mark out" aria-hidden="true"><Icon name="calendar-minus"/></span>
                        // The module's name gives way where the note is narrow, „entfernt" stays.
                        <span class="plan-toast-said">
                            <span class="plan-toast-said-name">{first.what.title.clone()}</span>
                            <span class="plan-toast-said-what">" "{t.planner.note_removed}</span>
                        </span>
                    </div>
                    {end}
                </div>
            </WaitNote>
        }
        .into_any(),
        NoteKind::Planned { at, .. } => {
            let what = first.what;
            // Where the module is now, and whether it was moved: the note's alone.
            let now = Memo::new(move |_| match planning.note().map(|note| note.kind) {
                Some(NoteKind::Planned { at, stepped }) => (at, stepped),
                _ => (at, false),
            });
            let start = Memo::new(move |_| mine.and_then(MyProgram::start));
            let chosen = Memo::new(move |_| planning.chosen());
            let floor = planning.floor(what.current);
            // The way the semester came from at the last step: its words come in from that side.
            let came = RwSignal::new(0_i32);
            let can = move |by: i32| by > 0 || now.get_untracked().0 > floor;
            let go = move |by: i32| {
                if can(by) {
                    came.set(by);
                    planning.step(plan, by);
                }
            };
            // A finger swiped over the semester: its words follow it (`slide`).
            let slide = RwSignal::new(0.0_f64);
            let sideways = Sideways { zone: ".plan-toast-stepper", slide, step: Callback::new(go), can: Callback::new(can) };
            // A tap: the arrow's own way (a key on it as well), else the half of the semester.
            let tap = move |ev: MouseEvent| {
                if let Some(by) = way_of_tap(&ev) {
                    go(by);
                }
            };
            let on_key = Callback::new(move |ev: KeyboardEvent| arrows(&ev, go));
            let words = {
                let what = what.clone();
                move || {
                    let (at, stepped) = now.get();
                    let from = match came.get_untracked() {
                        by if by > 0 => "right",
                        by if by < 0 => "left",
                        _ => "",
                    };
                    let line = line_of(at, stepped, &what, start.get(), t);
                    view! {
                        <span class="plan-toast-now" data-from=from style=move || (slide.get() != 0.0).then(|| format!("transform:translateX({:.1}px);opacity:{:.2}", slide.get(), (1.0 - slide.get().abs() / 140.0).max(0.35)))>
                            <b>{at.short(t.locale)}</b>
                            <small>{line}</small>
                        </span>
                    }
                }
            };
            let keep = move || {
                let (at, stepped) = now.get();
                let on = chosen.get() == Some(at);
                (stepped || chosen.get().is_some()).then(|| view! {
                    <button class="plan-toast-keep" type="button" role="switch" aria-checked=if on { "true" } else { "false" } title=t.planner.keep_hint on:click=move |_| planning.keep(at)>
                        <Icon name="layers"/>
                        <span>{t.planner.keep_for}" "<b>{at.short(t.locale)}</b></span>
                        <span class="plan-toast-switch" aria-hidden="true"></span>
                    </button>
                })
            };
            let title = what.title.clone();
            view! {
                <WaitNote on_gone=gone changed on_key sideways=Some(sideways) class="planning-note">
                    <div class="plan-toast">
                        <div class="plan-toast-main">
                            <span class="plan-toast-mark" aria-hidden="true"><Icon name="check"/></span>
                            // The module, where the bar has room for it (a wide screen).
                            <span class="plan-toast-title">{title}</span>
                            <div class="plan-toast-stepper" role="group" aria-label=(t.planner.semester_of)(&what.title) on:click=tap>
                                <button class="plan-toast-step" type="button" data-by="-1" aria-label=t.planner.earlier title=format!("{} (←)", t.planner.earlier) disabled=move || now.get().0 <= floor>
                                    <Icon name="chevron-left"/>
                                </button>
                                <span class="plan-toast-at">{words}</span>
                                <button class="plan-toast-step" type="button" data-by="1" aria-label=t.planner.later title=format!("{} (→)", t.planner.later)>
                                    <Icon name="chevron-right"/>
                                </button>
                            </div>
                        </div>
                        {keep}
                        {end}
                    </div>
                </WaitNote>
            }
            .into_any()
        }
    }
}

/// ← and → while the note of a module planned stands: the module a semester earlier or later, as
/// its arrows; the note's own keys (`WaitNote`'s `on_key`), `true` where it took one. Not where the
/// keys mean something else — in a field, a picker, a menu, a slider, at a panel's handle, in a
/// dialog, or where the page took the key already —, which is the student moving on.
fn arrows(ev: &KeyboardEvent, go: impl Fn(i32)) -> bool {
    use leptos::wasm_bindgen::JsCast;
    if ev.default_prevented() || ev.ctrl_key() || ev.meta_key() || ev.alt_key() || ev.shift_key() {
        return false;
    }
    let by = match ev.key().as_str() {
        "ArrowLeft" => -1,
        "ArrowRight" => 1,
        _ => return false,
    };
    let target = ev.target().and_then(|target| target.dyn_into::<leptos::web_sys::Element>().ok());
    let elsewhere = "input, textarea, select, [contenteditable], [role=slider], [role=tab], [role=menu], [role=menuitem], [role=listbox], [role=option], .combo, [data-action^='resize-'], dialog";
    if target.is_some_and(|target| target.closest(elsewhere).ok().flatten().is_some()) {
        return false;
    }
    ev.prevent_default();
    go(by);
    true
}

/// The way a click on the semester moves it: an arrow's own (`data-by`, also a key on the arrow),
/// else -1 on the left half of the semester, 1 on the right.
fn way_of_tap(ev: &MouseEvent) -> Option<i32> {
    use leptos::wasm_bindgen::JsCast;
    let target = ev.target().and_then(|target| target.dyn_into::<leptos::web_sys::Element>().ok());
    if let Some(by) = target.and_then(|target| target.closest("[data-by]").ok().flatten()).and_then(|arrow| arrow.get_attribute("data-by")) {
        return by.parse().ok();
    }
    #[cfg(feature = "csr")]
    {
        let area = ev.current_target().and_then(|area| area.dyn_into::<leptos::web_sys::Element>().ok())?;
        let rect = area.get_bounding_client_rect();
        Some(if f64::from(ev.client_x()) < rect.left() + rect.width() / 2.0 { -1 } else { 1 })
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// What the line under the semester says, most needed last to go: what happened there
/// („eingeplant", „verschoben"; only where there is room, app.css), its Fachsemester where the
/// Studienbeginn is known, and the warning — „liegt zurück" in a semester behind the student, „kein
/// Angebot" where the module's turnus leaves it out.
fn line_of(at: SemesterKey, stepped: bool, what: &Pressed, start: Option<SemesterKey>, t: &'static Texts) -> AnyView {
    let p = &t.planner;
    let past = at < what.current;
    let said = (!past).then_some(if stepped { p.note_moved } else { p.note_planned });
    let fs = start.and_then(|start| fachsemester(at, start)).map(|fs| (p.fs_short)(fs));
    let warning = if past {
        Some(p.note_behind)
    } else if !what.offer.offered(at) {
        Some(p.note_not_offered)
    } else {
        None
    };
    let parts: Vec<(&'static str, String)> = [said.map(|said| ("plan-toast-what", said.to_string())), fs.map(|fs| ("plan-toast-fs", fs)), warning.map(|warning| ("plan-toast-warn", warning.to_string()))].into_iter().flatten().collect();
    let last = parts.len().saturating_sub(1);
    parts
        .into_iter()
        .enumerate()
        .map(|(i, (class, text))| {
            view! {
                <span class=class>{text}</span>
                {(i < last).then(|| view! { <span class="plan-toast-dot" aria-hidden="true">" · "</span> })}
            }
        })
        .collect_view()
        .into_any()
}
