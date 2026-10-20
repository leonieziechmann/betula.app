//! The note after marking (folia/design/merken/oberflaeche.html, „Merken und der Hinweis danach";
//! owner, 2026-10-19: „Also ich finde die option mit dem Feld auch mit abstand am besten"): a note
//! that waits for interaction (`folia_design::waitnote::WaitNote`), in the frame of the note of
//! „Einplanen" (`plannote`) — a check, what happened (on a wide screen with the module's name),
//! where it went, „Rückgängig" after a line — but without its stepper: collections are no row to
//! step through (the same day: „Weil das was für das eindimensionale semester auswählen gut
//! funktioniert muss nicht gut sein für komplexere Sachen, wie Gruppen").
//!
//! - Marked: the field says where the module went — „Ohne Sammlung", „Für Lena", „Wahlpflicht, Für
//!   Lena +1", with a pin where the default for new marks put it there, „Sammlung" with a plus
//!   where there is none yet. A tap on it, or S while the note stands, opens the menu of its
//!   collections and its note (`collect`); the note waits behind it, and on a phone „Fertig" there
//!   ends it too. Who passes it by has marked, „Ohne Sammlung". „Rückgängig" takes the mark away
//!   with what the default gave it.
//! - Taken away: „Entfernt", and „Rückgängig" brings the mark back with its collections and note.
//! - The Merkliste full: „Merkliste voll · 256 Module", the way to it; nothing was marked.
//! - Others in the same frame (`Notices`): a collection deleted („‚Für Lena‘ gelöscht · 3 bleiben
//!   gemerkt", „Auch 2 entfernen" where modules were in it alone), the Merkliste emptied, a link
//!   or the list copied; of the rows selected on the Merkliste, „3 Module entfernt" and „3
//!   eingeplant · in WiSe 26/27", each with „Rückgängig" for all of them.
//!
//! One note stands at a time: this one takes the place of the note of „Einplanen", and that one
//! takes this one's (`Planning::hush`).

use leptos::ev::KeyboardEvent;
use leptos::prelude::*;

use folia_design::ui::Icon;
use folia_design::waitnote::WaitNote;
use folia_routes::url;
use folia_stores::bookmarks::{Bookmarks, Cleared, Marked, Toggled, MAX_MARKS};
use folia_stores::collections::{Anchor, CollectAsk, Collecting, Collections, Deleted};
use folia_stores::studyplan::{Planning, PlannedMany, Studyplan};

use crate::i18n::{self, Texts};

/// What the note says besides a mark given or taken away (`Bookmarks::last`).
#[derive(Clone, Debug, PartialEq)]
pub enum Notice {
    /// A collection deleted: what „Rückgängig" puts back, the modules that were in it alone (marked,
    /// in no other collection), and those of them taken away with it („Auch 2 entfernen").
    Deleted { deleted: Deleted, alone: Vec<String>, taken: Option<Cleared> },
    /// The Merkliste emptied: what „Rückgängig" puts back.
    Cleared(Cleared),
    /// Copied: a link for another device, or the list as text, of so many modules.
    Copied { link: bool, count: usize },
    /// The rows selected taken away: what „Rückgängig" puts back.
    Removed(Cleared),
    /// The rows selected planned: where they went, and „Rückgängig" for all of them (the plan before
    /// and after it, boxed).
    Planned(Box<PlannedMany>),
}

/// The notices said, the newest with a number of its own, shared through context.
#[derive(Clone, Copy)]
pub struct Notices(RwSignal<Option<(u64, Notice)>>);

impl Notices {
    pub fn provide() -> Self {
        let notices = Notices(RwSignal::new(None));
        provide_context(notices);
        notices
    }

    pub fn expect() -> Option<Self> {
        use_context::<Notices>()
    }

    /// Says it: the note shows it, in the place of what it showed.
    pub fn say(self, notice: Notice) {
        let serial = self.0.with_untracked(|said| said.as_ref().map_or(0, |(serial, _)| serial.wrapping_add(1)));
        self.0.set(Some((serial, notice)));
    }
}

/// What the note shows.
#[derive(Clone, Debug, PartialEq)]
enum Shown {
    Mark(Marked),
    Notice(u64, Notice),
}

impl Shown {
    /// Which note it is: another one is a new note.
    fn key(&self) -> (u8, u64) {
        match self {
            Shown::Mark(marked) => (0, marked.serial),
            Shown::Notice(serial, _) => (1, *serial),
        }
    }
}

/// The note, once in the app (`folia_app::App`): nothing while there is none.
#[component]
pub fn MarkNote() -> impl IntoView {
    let Some(bookmarks) = Bookmarks::expect() else { return ().into_any() };
    let notices = Notices::expect();
    let planning = Planning::expect();
    let shown = RwSignal::new(None::<Shown>);
    let show = move |what: Shown| {
        shown.set(Some(what));
        if let Some(planning) = planning {
            planning.hush();
        }
    };
    // A mark given or taken away.
    Effect::new(move |before: Option<Option<u64>>| {
        let last = bookmarks.last();
        let serial = last.as_ref().map(|last| last.serial);
        if let Some(last) = last.filter(|_| serial != before.flatten()) {
            show(Shown::Mark(last));
        }
        serial
    });
    // Another notice.
    if let Some(notices) = notices {
        Effect::new(move |before: Option<Option<u64>>| {
            let said = notices.0.get();
            let serial = said.as_ref().map(|(serial, _)| *serial);
            if let Some((serial, notice)) = said.filter(|_| serial != before.flatten()) {
                show(Shown::Notice(serial, notice));
            }
            serial
        });
    }
    // The note of „Einplanen" takes this one's place.
    if let Some(planning) = planning {
        Effect::new(move |before: Option<Option<u32>>| {
            let turn = planning.note().map(|note| note.turn);
            if turn.is_some() && turn != before.flatten() {
                shown.set(None);
            }
            turn
        });
    }
    // Another profile: what the note was about is gone.
    Effect::new(move |before: Option<u16>| {
        let profile = bookmarks.profile();
        if before.is_some_and(|before| before != profile) {
            shown.set(None);
        }
        profile
    });
    let key = Memo::new(move |_| shown.with(|shown| shown.as_ref().map(Shown::key)));
    (move || {
        let key = key.get()?;
        let what = shown.get_untracked()?;
        let gone = Callback::new(move |()| {
            if shown.with_untracked(|shown| shown.as_ref().map(Shown::key)) == Some(key) {
                shown.set(None);
            }
        });
        Some(view! { <NoteView what bookmarks gone/> })
    })
    .into_any()
}

/// One note.
#[component]
fn NoteView(what: Shown, bookmarks: Bookmarks, gone: Callback<()>) -> impl IntoView {
    let t = i18n::t();
    let m = &t.marks;
    // „Rückgängig" done, or what the note offered: it goes.
    let done = RwSignal::new(false);
    match what {
        Shown::Mark(marked) => match marked.what.clone() {
            Toggled::Marked { into } => view! { <MarkedNote marked into bookmarks gone/> }.into_any(),
            Toggled::Unmarked { .. } => {
                let title = marked.title.clone();
                let undo = move || bookmarks.undo(&marked);
                view! {
                    <WaitNote on_gone=gone away=Signal::derive(move || done.get()) class="mark-note">
                        <div class="mark-toast">
                            <span class="plan-toast-mark out" aria-hidden="true"><Icon name="bookmark-minus"/></span>
                            <span class="mark-toast-said">
                                <span class="mark-toast-name">{title}</span>
                                <span class="mark-toast-what wide">" "{m.removed_after}</span>
                                <span class="mark-toast-what narrow">{m.removed}</span>
                            </span>
                            <span class="mark-toast-gap"></span>
                            {end_of(t, done, undo)}
                        </div>
                    </WaitNote>
                }
                .into_any()
            }
            Toggled::Full => view! {
                <WaitNote on_gone=gone class="mark-note">
                    <div class="mark-toast">
                        <span class="plan-toast-mark warn" aria-hidden="true"><Icon name="triangle-alert"/></span>
                        <span class="mark-toast-said"><b>{m.full}</b><i>" · "{(m.full_count)(MAX_MARKS)}</i></span>
                        <span class="mark-toast-gap"></span>
                        <a class="mark-toast-act" href=t.path(url::BOOKMARKS)>{m.to_list}</a>
                    </div>
                </WaitNote>
            }
            .into_any(),
        },
        Shown::Notice(_, Notice::Deleted { deleted, alone, taken }) => {
            let notices = Notices::expect();
            let collections = Collections::expect();
            let name = deleted.collection.name.clone();
            let kept = deleted.modules.len().saturating_sub(taken.as_ref().map_or(0, |taken| taken.marks.len()));
            let said = match &taken {
                Some(taken) => format!("{} · {}", (m.deleted)(&name), (m.n_removed)(taken.marks.len())),
                None => format!("{} · {}", (m.deleted)(&name), (m.stay_marked)(kept)),
            };
            let also = (!alone.is_empty() && taken.is_none()).then(|| {
                let (deleted, alone) = (deleted.clone(), alone.clone());
                let count = alone.len();
                view! {
                    <button class="mark-toast-act" type="button" on:click=move |_| {
                        let taken = bookmarks.take_away(&alone);
                        if let Some(notices) = notices {
                            notices.say(Notice::Deleted { deleted: deleted.clone(), alone: Vec::new(), taken: Some(taken) });
                        }
                    }>{(m.also_remove)(count)}</button>
                }
            });
            let undo = move || {
                // The marks first: a collection keeps marked modules alone.
                if let Some(taken) = taken.clone() {
                    bookmarks.restore(taken);
                }
                if let Some(collections) = collections {
                    collections.update(|shelf| shelf.undelete(deleted.clone()));
                }
            };
            view! {
                <WaitNote on_gone=gone away=Signal::derive(move || done.get()) class="mark-note">
                    <div class="mark-toast">
                        <span class="plan-toast-mark out" aria-hidden="true"><Icon name="trash-2"/></span>
                        <span class="mark-toast-said">{said}</span>
                        <span class="mark-toast-gap"></span>
                        {also}
                        {end_of(t, done, undo)}
                    </div>
                </WaitNote>
            }
            .into_any()
        }
        Shown::Notice(_, Notice::Cleared(cleared)) => {
            let undo = move || bookmarks.restore(cleared.clone());
            view! {
                <WaitNote on_gone=gone away=Signal::derive(move || done.get()) class="mark-note">
                    <div class="mark-toast">
                        <span class="plan-toast-mark out" aria-hidden="true"><Icon name="trash-2"/></span>
                        <span class="mark-toast-said">{m.cleared}</span>
                        <span class="mark-toast-gap"></span>
                        {end_of(t, done, undo)}
                    </div>
                </WaitNote>
            }
            .into_any()
        }
        Shown::Notice(_, Notice::Removed(cleared)) => {
            let said = (m.removed_many)(cleared.marks.len());
            let undo = move || bookmarks.restore(cleared.clone());
            view! {
                <WaitNote on_gone=gone away=Signal::derive(move || done.get()) class="mark-note">
                    <div class="mark-toast">
                        <span class="plan-toast-mark out" aria-hidden="true"><Icon name="bookmark-minus"/></span>
                        <span class="mark-toast-said">{said}</span>
                        <span class="mark-toast-gap"></span>
                        {end_of(t, done, undo)}
                    </div>
                </WaitNote>
            }
            .into_any()
        }
        Shown::Notice(_, Notice::Planned(planned)) => {
            let said = planned_words(&planned, t);
            let plan = Studyplan::expect();
            let undo = (!planned.into.is_empty()).then(|| {
                let undo = move || {
                    if let Some(plan) = plan {
                        plan.undo_many((*planned).clone());
                    }
                };
                end_of(t, done, undo)
            });
            view! {
                <WaitNote on_gone=gone away=Signal::derive(move || done.get()) class="mark-note">
                    <div class="mark-toast">
                        <span class="plan-toast-mark" aria-hidden="true"><Icon name="calendar-check-2"/></span>
                        <span class="mark-toast-said">{said}</span>
                        <span class="mark-toast-gap"></span>
                        {undo}
                    </div>
                </WaitNote>
            }
            .into_any()
        }
        Shown::Notice(_, Notice::Copied { link, count }) => view! {
            <WaitNote on_gone=gone class="mark-note">
                <div class="mark-toast">
                    <span class="plan-toast-mark" aria-hidden="true"><Icon name=if link { "link" } else { "copy" }/></span>
                    <span class="mark-toast-said"><b>{if link { m.copied_link } else { m.copied_text }}</b><i>" · "{(m.copied_what)(count)}</i></span>
                    <span class="mark-toast-gap"></span>
                </div>
            </WaitNote>
        }
        .into_any(),
    }
}

/// The note of a module marked: its field says where it went, and opens the menu of its
/// collections and its note.
#[component]
fn MarkedNote(marked: Marked, into: std::collections::BTreeSet<u32>, bookmarks: Bookmarks, gone: Callback<()>) -> impl IntoView {
    let t = i18n::t();
    let m = &t.marks;
    let collecting = Collecting::expect();
    let collections = Collections::expect();
    let serial = marked.serial;
    let done = RwSignal::new(false);
    // What the field says: the module's collections now, as the row names them.
    let field = {
        let id = marked.id.clone();
        Memo::new(move |_| {
            collections.map(|collections| {
                collections.with(|shelf| {
                    let ids = shelf.of(&id);
                    let names: Vec<String> = ids.iter().filter_map(|c| shelf.collection(*c)).map(|c| c.name.clone()).collect();
                    let by_default = !into.is_empty() && ids.iter().copied().collect::<std::collections::BTreeSet<u32>>() == into;
                    (names, by_default, !shelf.collections().is_empty())
                })
            })
            .unwrap_or_default()
        })
    };
    let button = NodeRef::<leptos::html::Button>::new();
    let open = {
        let (id, title) = (marked.id.clone(), marked.title.clone());
        move || {
            let Some(collecting) = collecting else { return };
            collecting.open(CollectAsk { id: id.clone(), title: title.clone(), more: Vec::new(), at: rect_of(button), note: false, from_note: Some(serial) });
        }
    };
    // ⇧M while the note stands: its menu (the note's own key, as ← and → are the note of
    // „Einplanen"'s; the field is the arrow of what M did), not where a field or a menu has the keys.
    let on_key = {
        let open = open.clone();
        Callback::new(move |ev: KeyboardEvent| {
            if !is_shift_m(&ev) {
                return false;
            }
            ev.prevent_default();
            open();
            true
        })
    };
    let held = Signal::derive(move || collecting.is_some_and(|collecting| collecting.from_note(serial)));
    let away = Signal::derive(move || done.get() || collecting.is_some_and(|collecting| collecting.ended() == Some(serial)));
    let label = move || {
        let (names, by_default, any) = field.get();
        let text = match names.as_slice() {
            [] if !any => m.collection.to_string(),
            [] => m.without_collection.to_string(),
            [one] => one.clone(),
            [first, second] => format!("{first}, {second}"),
            [first, second, rest @ ..] => format!("{first}, {second} +{}", rest.len()),
        };
        let icon = if !any { "plus" } else if by_default && !names.is_empty() { "pin" } else { "tag" };
        let aria = if by_default && !names.is_empty() { (m.field_by_default)(&text) } else { (m.field_label)(&text) };
        (text, icon, aria, names.is_empty())
    };
    let title = marked.title.clone();
    view! {
        <WaitNote on_gone=gone on_key held away class="mark-note">
            <div class="mark-toast">
                <span class="plan-toast-mark" aria-hidden="true"><Icon name="check"/></span>
                <span class="mark-toast-said"><span class="mark-toast-name wide">{title}</span><span class="mark-toast-what narrow">{m.saved}</span></span>
                <span class="mark-toast-gap"></span>
                {move || {
                    let (text, icon, aria, empty) = label();
                    let open = open.clone();
                    view! {
                        <button class="mark-toast-field" class:empty=empty type="button" node_ref=button data-action="collect" aria-haspopup="dialog" aria-label=aria title=m.collect_hint on:click=move |_| open()>
                            <Icon name=icon/><span>{text}</span><Icon name="chevron-down"/>
                        </button>
                    }
                }}
                <div class="plan-toast-end">
                    <span class="plan-toast-sep" aria-hidden="true"></span>
                    <button class="plan-toast-undo" type="button" on:click=move |_| {
                        bookmarks.undo(&marked);
                        done.set(true);
                    }>{t.common.undo}</button>
                </div>
            </div>
        </WaitNote>
    }
}

/// What the note says of modules planned at once: „3 eingeplant · in WiSe 26/27", „3 eingeplant ·
/// 2 in WiSe 26/27, 1 in SoSe 27", what was planned there already („1 war schon geplant"), or that
/// all of them were.
fn planned_words(planned: &PlannedMany, t: &'static Texts) -> String {
    let m = &t.marks;
    let places = planned.by_semester();
    let mut said = match places.as_slice() {
        [] => return m.all_planned.to_string(),
        [(semester, _)] => format!("{} · {}", (m.n_planned)(planned.into.len()), (m.planned_in)(&semester.short(t.locale))),
        many => format!("{} · {}", (m.n_planned)(planned.into.len()), many.iter().map(|(semester, n)| (m.n_in)(*n, &semester.short(t.locale))).collect::<Vec<_>>().join(", ")),
    };
    if planned.there > 0 {
        said.push_str(" · ");
        said.push_str(&(m.planned_before)(planned.there));
    }
    said
}

/// ⇧M (Shift, not the letter's case: Caps Lock's M is M), not where a field, a menu or a dialog has
/// the keys.
fn is_shift_m(ev: &KeyboardEvent) -> bool {
    use leptos::wasm_bindgen::JsCast;
    if ev.default_prevented() || ev.ctrl_key() || ev.meta_key() || ev.alt_key() || !ev.shift_key() || !matches!(ev.key().as_str(), "m" | "M") {
        return false;
    }
    let target = ev.target().and_then(|target| target.dyn_into::<leptos::web_sys::Element>().ok());
    let elsewhere = "input, textarea, select, [contenteditable], [role=menu], [role=menuitem], [role=listbox], [role=option], [role=dialog], .combo, dialog";
    !target.is_some_and(|target| target.closest(elsewhere).ok().flatten().is_some())
}

/// The box of the field, where the menu stands on a wide screen.
fn rect_of(button: NodeRef<leptos::html::Button>) -> Option<Anchor> {
    #[cfg(feature = "csr")]
    {
        let rect = button.get_untracked()?.get_bounding_client_rect();
        Some(Anchor { left: rect.left(), top: rect.top(), right: rect.right(), bottom: rect.bottom() })
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = button;
        None
    }
}

/// „Rückgängig" at the end of a note, after a line: it does `undo`, and the note goes.
fn end_of(t: &'static Texts, done: RwSignal<bool>, undo: impl Fn() + 'static) -> impl IntoView {
    view! {
        <div class="plan-toast-end">
            <span class="plan-toast-sep" aria-hidden="true"></span>
            <button class="plan-toast-undo" type="button" on:click=move |_| {
                undo();
                done.set(true);
            }>{t.common.undo}</button>
        </div>
    }
}
