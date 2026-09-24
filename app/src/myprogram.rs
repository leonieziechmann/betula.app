//! „Mein Studiengang" in this browser: the visitor's program, study direction, Studienbeginn and
//! town, and what the local catalog knows of that program.
//!
//! Owner clarification (2026-09-24): „Mein Studiengang" is the app's alone and kept in
//! `localStorage` (`betula.myprogram.v1`, the text of `catalog::studyplan::MineDoc`), like the
//! Merkliste: empty on the server (R9), in no request, and never dragged along in addresses. The
//! store is where every page reads it (the Studienplan's Fachsemester and import, the finder,
//! the Standort); the program's slug stands only in a catalog address that is filtered by it.

use catalog::pages::{self, MyProgramInfo};
use catalog::studyplan::MineDoc;
use catalog::timetable::select::TownChoice;
use catalog::timetable::semester::SemesterKey;
use catalog::url;
use leptos::prelude::*;

use crate::data::use_source;
use crate::nav;

const STORAGE_KEY: &str = "betula.myprogram.v1";

/// The browser app (`csr`): only there is anything stored.
const APP: bool = cfg!(feature = "csr");

/// „Mein Studiengang", shared through context. Empty on the server, always.
#[derive(Clone, Copy)]
pub struct MyProgram(RwSignal<MineDoc>);

impl MyProgram {
    /// Reads what this browser has stored and provides it; another tab of the same browser may
    /// change it, and this one follows.
    pub fn provide() -> Self {
        let mine = MyProgram(RwSignal::new(load()));
        provide_context(mine);
        // Effects run in the browser only.
        Effect::new(move |_| {
            let handle = window_event_listener_untyped("storage", move |_| {
                let stored = load();
                if mine.0.with_untracked(|doc| *doc != stored) {
                    mine.0.set(stored);
                }
            });
            on_cleanup(move || handle.remove());
        });
        mine
    }

    pub fn expect() -> Option<Self> {
        use_context::<MyProgram>()
    }

    /// All of it. Tracked: read it in a memo that keeps only what it needs.
    pub fn get(self) -> MineDoc {
        self.0.get()
    }

    /// Reads it without copying. Tracked.
    pub fn with<R>(self, f: impl FnOnce(&MineDoc) -> R) -> R {
        self.0.with(f)
    }

    /// Whether this program (`program.id`) is the visitor's. Tracked: a memo per button (R5).
    pub fn is(self, program_id: &str) -> bool {
        self.0.with(|doc| doc.program.as_deref() == Some(program_id))
    }

    /// The Standort the visitor chose; `Derive` when none. Tracked.
    pub fn town(self) -> TownChoice {
        self.0.with(|doc| doc.town)
    }

    /// The Studienbeginn, when one is stored. Tracked.
    pub fn start(self) -> Option<SemesterKey> {
        self.0.with(|doc| doc.start)
    }

    /// The program (its id, never the slug), its display name as it reads now (said once the id
    /// is gone from the snapshot), the caption of the chosen plan (`""`: the only or the unnamed
    /// one) and the page that fills a core plan's direction row. An id that cannot be one is not
    /// taken; Studienbeginn and Standort stay.
    pub fn set_program(self, id: &str, name: &str, caption: &str, direction: Option<&str>) {
        if !url::is_program_id(id) {
            return;
        }
        self.change(|doc| {
            doc.program = Some(id.to_string());
            doc.name = Some(name.to_string()).filter(|name| !name.trim().is_empty());
            doc.caption = Some(caption.to_string());
            doc.direction = direction.map(str::to_string);
        });
    }

    /// No program any more: its name, plan and direction go with it; Studienbeginn and Standort
    /// stay, since they are the student's whatever the program.
    pub fn clear_program(self) {
        self.change(MineDoc::clear_program);
    }

    pub fn set_start(self, start: Option<SemesterKey>) {
        self.change(|doc| doc.start = start);
    }

    pub fn set_town(self, town: TownChoice) {
        self.change(|doc| doc.town = town);
    }

    /// Changes and stores it, telling what depends on it only when something changed. Nothing left
    /// takes the key out (`nav::local_set`).
    fn change(self, f: impl FnOnce(&mut MineDoc)) {
        let mut doc = self.0.try_with_untracked(Clone::clone).unwrap_or_default();
        f(&mut doc);
        if self.0.try_with_untracked(|now| *now != doc).unwrap_or(false) {
            nav::local_set(STORAGE_KEY, &doc.stored());
            self.0.set(doc);
        }
    }
}

/// What this browser has stored; nothing on the server and in a browser that refuses storage.
fn load() -> MineDoc {
    restored(nav::local_get(STORAGE_KEY).as_deref())
}

/// „Mein Studiengang" of a stored text, read like anything from outside: never an error.
fn restored(stored: Option<&str>) -> MineDoc {
    stored.map(MineDoc::restored).unwrap_or_default()
}

/// The stored program as the local catalog has it (`pages::my_program`): the program itself, or,
/// when its PO is gone from the snapshot, the newest of its family (`exact` false: then no page
/// sets a default from it, A.10). `None` without a program, and on the server, which never knows
/// one (R9). One answer for the whole app, from the visit's cached `programs`.
#[derive(Clone, Copy)]
pub struct MineResolved(pub Memo<Option<MyProgramInfo>>);

impl MineResolved {
    /// Call it after `MyProgram::provide`.
    pub fn provide() -> Self {
        let mine = MyProgram::expect();
        let source = use_source().ok();
        // The program alone: a changed Studienbeginn or Standort asks the catalog nothing.
        let program = Memo::new(move |_| if APP { mine.and_then(|mine| mine.with(|doc| doc.program.clone())) } else { None });
        let resolved = MineResolved(Memo::new(move |_| {
            let id = program.get()?;
            source.as_ref()?.run(|db| pages::my_program(db, &id)).ok().flatten()
        }));
        provide_context(resolved);
        resolved
    }

    pub fn expect() -> Option<Self> {
        use_context::<MineResolved>()
    }
}

#[cfg(test)]
mod tests {
    use catalog::timetable::select::Town;

    use super::*;

    #[test]
    fn what_is_stored_is_read_like_anything_from_outside() {
        assert_eq!(restored(None), MineDoc::default());
        // Garbage: nothing of it is taken for a program, a start or a town.
        let garbage = restored(Some("program\t../../etc\nstart\t2026X\ntown\tberlin\n\u{0}\n"));
        assert_eq!((garbage.program, garbage.start, garbage.town), (None, None, TownChoice::Derive));
        let good = restored(Some("program\t079-82-2008\nstart\t2026W\ntown\tsenftenberg\n"));
        assert_eq!((good.program.as_deref(), good.start, good.town), (Some("079-82-2008"), SemesterKey::parse("2026W"), TownChoice::Only(Town::Senftenberg)));
    }

    #[test]
    fn nothing_left_takes_the_key_out() {
        // What `nav::local_set` is handed: empty means the key goes.
        assert_eq!(MineDoc::default().stored(), "");
        let mut doc = restored(Some("program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2026W\n"));
        doc.clear_program();
        // The Studienbeginn is the student's whatever the program: it stays, and so does the key.
        assert_eq!(doc.stored(), "start\t2026W\n");
        doc.start = None;
        assert_eq!(doc.stored(), "");
    }
}
