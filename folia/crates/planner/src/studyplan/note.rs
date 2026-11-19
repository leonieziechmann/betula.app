//! The note of the Stundenplan (owner, 2026-11-19: „Such mal noch nach weiteren stellen, wo
//! unnötiger weise was on screen gezeigt wird, obwohl es auch eine wait note getan hätte"): what was
//! just done on the page, in a note that waits for interaction at the window's foot
//! (`folia_design::waitnote::DoneNote`). It says the last change of the plan with „Rückgängig"
//! (`PlanCtx::undo`: a Termin of one's own made, changed or deleted, a module taken into a calendar
//! or out of it, a calendar made or deleted, a share taken in, what a Termin's card did) and what
//! else the page has to say (`PlanCtx::said`: a saved plan loaded or deleted, each with its own
//! „Rückgängig", a new address copied, a link that opens nothing, a profile saved). One note stands
//! at a time, the newer in the
//! place of the older. Until 2026-11-19 most of them stood in the page where they were done: a line
//! under a calendar, in „+ Modul", under the saved plans, in the place of a share's offer.
//!
//! A note whose going loses what it offers for good is `lasting` (`WaitNote`): a Termin of one's
//! own, a calendar or a saved plan deleted, a saved plan loaded over the semester's plan. It stays
//! until it is put away, a newer note takes its place or the page is left.
//!
//! On a phone a sheet of the head (`head::PhoneSheets`) lies over the page, modal, and nothing of
//! the page can stand over it: what is done in a sheet is said at the sheet's foot, by the note of
//! that sheet (`PlanNote` with `shown` while it is open), and the page's note waits behind it until
//! the sheet is closed. A sheet opened or closed is a step on: a note that does not last goes with
//! it (`mod.rs`).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use folia_design::waitnote::{DoneNote, Mark};
use leptos::prelude::*;

use super::add::ADDED;
use super::calendars::{CALENDAR_GONE, CALENDAR_MADE, CALENDAR_REMOVED, CALENDAR_UNREAD};
use super::own::{focus_where_lost, OWN_GONE, OWN_NOTE};
use super::PlanCtx;

/// What the page says besides a change of the plan (`PlanCtx::said`).
#[derive(Clone)]
pub(crate) struct PlanSaid {
    /// Which it is: another one is a new note.
    turn: u64,
    icon: &'static str,
    mark: Mark,
    said: String,
    /// „Rückgängig", where what was done can be taken back (a saved plan loaded or deleted): the
    /// note lasts.
    undo: Option<Arc<dyn Fn() + Send + Sync>>,
}

/// The number of the next `PlanSaid`.
static TURN: AtomicU64 = AtomicU64::new(1);

impl PlanCtx {
    /// Says it in the page's note, in the place of what the note said (a change's note too).
    pub(super) fn say(self, icon: &'static str, mark: Mark, said: impl Into<String>) {
        self.put(PlanSaid { turn: 0, icon, mark, said: said.into(), undo: None });
    }

    /// Says it with „Rückgängig", which does `undo`: a note that lasts.
    pub(super) fn say_undoable(self, icon: &'static str, mark: Mark, said: impl Into<String>, undo: impl Fn() + Send + Sync + 'static) {
        self.put(PlanSaid { turn: 0, icon, mark, said: said.into(), undo: Some(Arc::new(undo)) });
    }

    fn put(self, said: PlanSaid) {
        let turn = TURN.fetch_add(1, Ordering::Relaxed);
        self.undo.set(None);
        self.said.set(Some(PlanSaid { turn, ..said }));
    }

    /// Whether the note that stands lasts until it is put away.
    pub(super) fn note_lasts(self) -> bool {
        if let Some(lasts) = self.undo.with_untracked(|undo| undo.as_ref().map(|(note, _)| looks(note).3)) {
            return lasts;
        }
        self.said.with_untracked(|said| said.as_ref().is_some_and(|said| said.undo.is_some()))
    }
}

/// How a change's note is said: its words without the mark it begins with, its icon, its mark, and
/// whether it lasts. A note without a mark (a share taken in, a calendar made of a link) is said as
/// it is.
fn looks(note: &str) -> (&str, &'static str, Mark, bool) {
    if let Some(said) = note.strip_prefix(OWN_GONE) {
        (said, "trash-2", Mark::Out, true)
    } else if let Some(said) = note.strip_prefix(OWN_NOTE) {
        (said, "check", Mark::Done, false)
    } else if let Some(said) = note.strip_prefix(ADDED) {
        (said, "calendar-plus", Mark::Done, false)
    } else if let Some(rest) = note.strip_prefix(CALENDAR_REMOVED) {
        (rest.split_once(' ').map_or(rest, |(_, said)| said), "calendar-minus", Mark::Out, false)
    } else if let Some(said) = note.strip_prefix(CALENDAR_GONE) {
        (said, "trash-2", Mark::Out, true)
    } else if let Some(said) = note.strip_prefix(CALENDAR_MADE) {
        (said, "calendar-plus", Mark::Done, false)
    } else if let Some(said) = note.strip_prefix(CALENDAR_UNREAD) {
        (said, "triangle-alert", Mark::Warn, false)
    } else {
        (note, "check", Mark::Done, false)
    }
}

/// What the note shows.
#[derive(Clone)]
enum Now {
    Change(String),
    Said(PlanSaid),
}

/// The note, while `shown`: the page's while no sheet of a phone lies over it, a sheet's while it is
/// open. The keyboard comes to a lasting note's „Rückgängig" where what was done took it (the bin, the
/// card's „Löschen", a form or a menu that closed), never to the page's body.
#[component]
pub(super) fn PlanNote(ctx: PlanCtx, #[prop(into)] shown: Signal<bool>) -> impl IntoView {
    // „Rückgängig" of a change: the change taken back, not what was changed meanwhile elsewhere
    // (`PlanUndo`). The note goes down and away; then the keyboard goes back where a deletion took it
    // (the slot of the Termin of one's own), else to the week.
    let restore = Callback::new(move |()| {
        let (Some(plan), Some((note, before))) = (ctx.plan, ctx.undo.get_untracked()) else { return };
        plan.update_after_paint(move |doc| before.restore(doc));
        let back = if note.starts_with(OWN_GONE) { ctx.back_to.get_value() } else { None };
        set_timeout(
            move || {
                let back = back.as_deref().into_iter().chain([".sp-week .week a.slot"]).collect::<Vec<_>>();
                focus_where_lost(&back);
            },
            std::time::Duration::from_millis(260),
        );
    });
    move || {
        if !shown.get() {
            return None;
        }
        // A change's note is the newer where both are there for a moment (`say` takes it away).
        let now = match ctx.undo.with(|undo| undo.as_ref().map(|(note, _)| note.clone())) {
            Some(note) => Now::Change(note),
            None => Now::Said(ctx.said.get()?),
        };
        let (icon, mark, said, undo, lasting) = match &now {
            Now::Change(note) => {
                let (said, icon, mark, lasting) = looks(note);
                (icon, mark, said.to_string(), Some(restore), lasting)
            }
            Now::Said(said) => {
                let undo = said.undo.clone().map(|undo| Callback::new(move |()| undo()));
                (said.icon, said.mark, said.said.clone(), undo, said.undo.is_some())
            }
        };
        // The keyboard was on what a deletion took: it comes to „Rückgängig" — once what went has
        // gone from the page, and once a card, form or menu has closed.
        if lasting {
            folia_design::nav::after_paint(|| request_animation_frame(|| focus_where_lost(&[&format!("#{UNDO_ID}")])));
            set_timeout(|| focus_where_lost(&[&format!("#{UNDO_ID}")]), std::time::Duration::from_millis(350));
        }
        // A note that leaves lets go of itself alone, not of a newer one that took its place (the
        // next card's, within the 200 ms the old one takes to go).
        let gone = Callback::new(move |()| match &now {
            Now::Change(mine) => {
                if ctx.undo.with_untracked(|undo| undo.as_ref().is_some_and(|(note, _)| note == mine)) {
                    ctx.undo.set(None);
                }
            }
            Now::Said(mine) => {
                if ctx.said.with_untracked(|said| said.as_ref().is_some_and(|said| said.turn == mine.turn)) {
                    ctx.said.set(None);
                }
            }
        });
        Some(view! { <DoneNote on_gone=gone icon mark said undo undo_id=UNDO_ID lasting class="plan-note"/> })
    }
}

/// The id of the note's „Rückgängig".
pub(super) const UNDO_ID: &str = "plan-note-undo";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_is_said_without_its_mark_and_a_deletion_lasts() {
        assert_eq!(looks(&format!("{OWN_GONE}„Lerngruppe“ gelöscht")), ("„Lerngruppe“ gelöscht", "trash-2", Mark::Out, true));
        assert_eq!(looks(&format!("{OWN_NOTE}VL EvS angelegt")), ("VL EvS angelegt", "check", Mark::Done, false));
        assert_eq!(looks(&format!("{CALENDAR_REMOVED}3 „EiP“ aus „Lena“ genommen")), ("„EiP“ aus „Lena“ genommen", "calendar-minus", Mark::Out, false));
        assert_eq!(looks(&format!("{CALENDAR_GONE}„Lena“ gelöscht")).3, true);
        assert_eq!(looks(&format!("{ADDED}„EiP“ in „WG“ aufgenommen")).3, false);
        assert_eq!(looks("Aus dem Link übernommen: 3 Module"), ("Aus dem Link übernommen: 3 Module", "check", Mark::Done, false));
    }
}
