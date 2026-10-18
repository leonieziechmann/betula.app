//! The note of what „Einplanen" did (owner, 2026-10-18, concept E of
//! folia/design/einplanen/oberflaeche.html: „Also ich liebe Option E, das ist mit Abstand die
//! eleganteste"), over the page at the bottom of the window (`folia_stores::studyplan::Planning`): a
//! check, the semester between ‹ and › with what happened under it („eingeplant · 3. FS",
//! „verschoben", „liegt zurück", „kein Angebot"), and „Rückgängig". The arrows move the module at
//! once, into the past as well, to the first Fachsemester (← and → do too while the note stands);
//! once it was moved, a second row offers „Weitere Module auch nach SoSe 27" with a switch, and while
//! the session plans into a semester chosen so it stands in every note, to switch it off where it
//! is seen (owner, the same day: „die Option mit, die nächsten Module auch in das Semester planen ist
//! genial. Aber das Design von der Box muss vielleicht nochmal etwas angepasst werden"). A module
//! taken out says so, with „Rückgängig".
//!
//! It stands 4.5 s, 7 s once moved, and as long as the pointer or the keyboard is on it; a newer
//! one takes its place. On a wide screen one bar across the page right of the rail, one line high,
//! the module named in it (owner, 2026-10-18: „bei der pc Version kann die notification ruhig über
//! die ganze Breite rechts von der sidebar gehen und dann vllt etwas weniger hoch sein"); on a phone
//! a card above the bottom bar, the switch a row over the arrows, so that what is under the finger
//! stays. Dark in both themes (owner, the same day: „dark modus muss natürlich auch dafür
//! funktionieren"; app.css „The note of „Einplanen"", `.plan-toast`).

use std::time::Duration;

use folia_calendar::semester::{fachsemester, SemesterKey};
use leptos::prelude::*;

use folia_design::ui::Icon;
use folia_stores::myprogram::MyProgram;
use folia_stores::studyplan::{NoteKind, Planning, Pressed, Studyplan};

use crate::i18n::{self, Texts};

/// How long the note stands, and once the module was moved.
const STANDS: Duration = Duration::from_millis(4500);
const STANDS_MOVED: Duration = Duration::from_millis(7000);

/// The note, once in the app (`folia_app::App`): nothing while there is none.
#[component]
pub fn PlanNote() -> impl IntoView {
    let (Some(planning), Some(plan)) = (Planning::expect(), Studyplan::expect()) else { return ().into_any() };
    let mine = MyProgram::expect();
    // The pointer or the keyboard on the note: it waits.
    let held = RwSignal::new(false);
    // Its while, from each change of it, unless it is held: a newer change leaves the older timer
    // without effect (`shown`).
    Effect::new(move |_| {
        let Some((shown, moved)) = planning.note().map(|note| (note.shown, matches!(note.kind, NoteKind::Planned { stepped: true, .. }))) else { return };
        if held.get() {
            return;
        }
        set_timeout(move || planning.dismiss(Some(shown)), if moved { STANDS_MOVED } else { STANDS });
    });
    #[cfg(feature = "csr")]
    {
        let keys = window_event_listener(leptos::ev::keydown, move |ev| arrows(&ev, planning, plan));
        on_cleanup(move || keys.remove());
    }
    // A new press makes a new note; what changes on one changes inside it, so the focus stays.
    let turn = Memo::new(move |_| planning.note().map(|note| note.turn));
    (move || turn.get().map(|_| view! { <NoteView planning plan mine held/> })).into_any()
}

/// ← and → while the note of a module planned stands: the module a semester earlier or later, as
/// its arrows. Not where the keys mean something else: in a field, a picker, a menu, a slider, at a
/// panel's handle, in a dialog, or where the page took the key already.
#[cfg(feature = "csr")]
fn arrows(ev: &leptos::web_sys::KeyboardEvent, planning: Planning, plan: Studyplan) {
    use leptos::wasm_bindgen::JsCast;
    if ev.default_prevented() || ev.ctrl_key() || ev.meta_key() || ev.alt_key() || ev.shift_key() {
        return;
    }
    let by = match ev.key().as_str() {
        "ArrowLeft" => -1,
        "ArrowRight" => 1,
        _ => return,
    };
    if !planning.note_untracked().is_some_and(|note| matches!(note.kind, NoteKind::Planned { .. })) {
        return;
    }
    let target = ev.target().and_then(|target| target.dyn_into::<leptos::web_sys::Element>().ok());
    let elsewhere = "input, textarea, select, [contenteditable], [role=slider], [role=tab], [role=menu], [role=menuitem], [role=listbox], [role=option], .combo, [data-action^='resize-'], dialog";
    if target.is_some_and(|target| target.closest(elsewhere).ok().flatten().is_some()) {
        return;
    }
    ev.prevent_default();
    planning.step(plan, by);
}

/// One note: what it says changes in place (the arrows, the switch), so that the focus and the
/// pointer stay where they are.
#[component]
fn NoteView(planning: Planning, plan: Studyplan, mine: Option<MyProgram>, held: RwSignal<bool>) -> impl IntoView {
    let t = i18n::t();
    let Some(first) = planning.note_untracked() else { return ().into_any() };
    let hold = move || {
        if !held.get_untracked() {
            held.set(true);
            planning.renew();
        }
    };
    let free = move || held.set(false);
    // The keyboard leaving the note for something outside it.
    let focus_out = move |ev: leptos::ev::FocusEvent| {
        if !focus_stays(&ev) {
            free();
        }
    };
    let undo = move |_| {
        held.set(false);
        planning.undo(plan);
    };
    // „Rückgängig" at the end, after a line.
    let end = view! {
        <div class="plan-toast-end">
            <span class="plan-toast-sep" aria-hidden="true"></span>
            <button class="plan-toast-undo" type="button" on:click=undo>{t.common.undo}</button>
        </div>
    };
    let (body, removed) = match first.kind {
        NoteKind::Removed { .. } => (
            view! {
                <div class="plan-toast-main">
                    <span class="plan-toast-mark out" aria-hidden="true"><Icon name="calendar-minus"/></span>
                    <span class="plan-toast-said">{(t.planner.note_removed)(&first.what.title)}</span>
                </div>
                {end}
            }
            .into_any(),
            true,
        ),
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
            let label = move || now.get().0.short(t.locale);
            let line = {
                let what = what.clone();
                move || {
                    let (at, stepped) = now.get();
                    line_of(at, stepped, &what, start.get(), t)
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
            (
                view! {
                    <div class="plan-toast-main">
                        <span class="plan-toast-mark" aria-hidden="true"><Icon name="check"/></span>
                        // The module, where the bar has room for it (a wide screen).
                        <span class="plan-toast-title">{title}</span>
                        <button class="plan-toast-step" type="button" aria-label=t.planner.earlier title=format!("{} (←)", t.planner.earlier) disabled=move || now.get().0 <= floor on:click=move |_| planning.step(plan, -1)>
                            <Icon name="chevron-left"/>
                        </button>
                        <span class="plan-toast-at"><b>{label}</b><small>{line}</small></span>
                        <button class="plan-toast-step" type="button" aria-label=t.planner.later title=format!("{} (→)", t.planner.later) on:click=move |_| planning.step(plan, 1)>
                            <Icon name="chevron-right"/>
                        </button>
                    </div>
                    {keep}
                    {end}
                }
                .into_any(),
                false,
            )
        }
    };
    view! {
        <div
            class="plan-toast"
            class:out=removed
            role="status"
            on:pointerenter=move |_| hold()
            on:pointerleave=move |_| free()
            on:focusin=move |_| hold()
            on:focusout=focus_out
        >
            {body}
        </div>
    }
    .into_any()
}

/// Whether the focus that leaves an element of the note goes to another of it.
fn focus_stays(ev: &leptos::ev::FocusEvent) -> bool {
    use leptos::wasm_bindgen::JsCast;
    let note = ev.current_target().and_then(|note| note.dyn_into::<leptos::web_sys::Node>().ok());
    let next = ev.related_target().and_then(|next| next.dyn_into::<leptos::web_sys::Node>().ok());
    note.zip(next).is_some_and(|(note, next)| note.contains(Some(&next)))
}

/// What the line under the semester says: what happened there („eingeplant", „verschoben", „liegt
/// zurück" in a semester behind the student), its Fachsemester where the Studienbeginn is known,
/// and „kein Angebot" where the module's turnus leaves it out.
fn line_of(at: SemesterKey, stepped: bool, what: &Pressed, start: Option<SemesterKey>, t: &'static Texts) -> AnyView {
    let p = &t.planner;
    let past = at < what.current;
    let fs = start.and_then(|start| fachsemester(at, start)).map(|fs| format!(" · {}", (p.fs_short)(fs)));
    let offered = (!past && !what.offer.offered(at)).then(|| format!(" · {}", p.note_not_offered));
    let said = if past {
        view! { <em class="past">{p.note_behind}</em> }.into_any()
    } else {
        view! { <span>{if stepped { p.note_moved } else { p.note_planned }}</span> }.into_any()
    };
    view! { {said}{fs}{offered} }.into_any()
}
