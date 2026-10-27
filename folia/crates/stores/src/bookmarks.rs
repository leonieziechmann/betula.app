//! „Merken": the modules a visitor wants to come back to, and the button that marks them.
//!
//! Owner decision (2026-09-20): marking exists in the browser app only, and no data of a visitor
//! reaches the server. So the list lives in this browser's `localStorage` and nowhere else:
//!
//! - never in a URL (R13): URLs are requested from the server and end up in its logs. The URL of
//!   the list (`/bookmarks?…`) says how the list is shown, not what is on it;
//! - never in server HTML (R9): the server renders every mark as „not marked" and cannot know
//!   better, the stylesheet keeps the buttons out of sight until the app has taken over (`.app`);
//! - in no request: the app asks the local copy of the catalog about the marked modules.
//!
//! Every button reads its own state through a memo of its own, so marking a module changes that
//! module's buttons and nothing else (R5): not the list around it, not the preview next to it.
//!
//! The key is versioned, and each profile has its own (`profiles::key_of`). What is stored is a
//! line per module, the newest first: `<module id>\t<seconds since 1970 when it was marked>`. It
//! is read like anything that comes from outside: ids that could not be ids are dropped (they end
//! up in links and queries), a module counts once, and the list has an upper limit.

use std::collections::BTreeSet;

use folia_model::ids::is_module_id;
use folia_pages::MAX_BOOKMARKS;
use leptos::prelude::*;

use crate::collections::{clean_name, clean_note, name_key, Anchor, Belongings, CollectAsk, Collecting, Collections, MAX_COLLECTIONS};
use crate::i18n;
use crate::profiles::Kept;
use folia_design::ui::{Icon, Shortcut};

/// The first profile's key; another profile's has its number in it (`profiles::key_of`).
pub(crate) const STORAGE_KEY: &str = "betula.bookmarks.v1";

/// How many modules can be marked (owner, 2026-10-19: „Also 256 Module sind auch genug. Dann gibt
/// es einfach ein Limit, und man bekommt ne Meldung"). It holds when something is added: a list that
/// holds more from before (read up to `MAX_BOOKMARKS`) stays as it is, and nothing is added to it.
pub const MAX_MARKS: usize = 256;

/// What marking or unmarking a module did, for the note that follows it (`MarkNote`) and its
/// „Rückgängig".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Toggled {
    /// Marked, and put into the collections of the default for new marks (none without one).
    Marked { into: BTreeSet<u32> },
    /// Its mark taken away, with what it had beside it: „Rückgängig" puts all of it back.
    Unmarked { mark: Mark, belongings: Belongings },
    /// Not marked: the Merkliste holds `MAX_MARKS` modules already.
    Full,
}

/// What emptying the Merkliste took away (`Bookmarks::clear`), for its „Rückgängig".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cleared {
    pub marks: Vec<Mark>,
    /// The modules' collections and notes, by module.
    pub belongings: Vec<(String, Belongings)>,
}

/// The last mark given or taken away, for the note after it: the module, its title, what
/// happened, and a number of its own, so that the same thing twice is two notes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marked {
    pub id: String,
    pub title: String,
    pub what: Toggled,
    pub serial: u64,
}

/// A marked module and when it was marked (seconds since 1970; 0 if unknown).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    pub id: String,
    pub at: u64,
}

/// The marked modules, the newest first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Marks(Vec<Mark>);

impl Marks {
    fn contains(&self, id: &str) -> bool {
        self.0.iter().any(|mark| mark.id == id)
    }

    /// Marks the module, or takes the mark away again. `true` if it is marked afterwards.
    #[cfg(test)]
    fn toggle(&mut self, id: &str, now: u64) -> bool {
        matches!(self.flip(id, now), Flip::Marked)
    }

    /// Marks the module, or takes its mark away again; refuses a new mark where `MAX_MARKS` are
    /// there.
    fn flip(&mut self, id: &str, now: u64) -> Flip {
        if let Some(at) = self.0.iter().position(|mark| mark.id == id) {
            return Flip::Unmarked(self.0.remove(at));
        }
        if !is_module_id(id) {
            return Flip::Refused;
        }
        if self.0.len() >= MAX_MARKS {
            return Flip::Full;
        }
        self.0.insert(0, Mark { id: id.to_string(), at: now });
        Flip::Marked
    }

    /// Puts a mark back where its time puts it (the „Rückgängig" of taking it away), before the
    /// marks of the same second: it was the newest of them a moment ago.
    fn put_back(&mut self, mark: Mark) {
        if self.contains(&mark.id) || !is_module_id(&mark.id) {
            return;
        }
        let at = self.0.iter().position(|kept| kept.at <= mark.at).unwrap_or(self.0.len());
        self.0.insert(at, mark);
    }

    /// Adds the marks that are not there yet and keeps the newest first.
    fn merge(&mut self, marks: Vec<Mark>) {
        for mark in marks {
            if is_module_id(&mark.id) && !self.contains(&mark.id) {
                self.0.push(mark);
            }
        }
        self.0.sort_by_key(|mark| std::cmp::Reverse(mark.at));
        self.0.truncate(MAX_BOOKMARKS);
    }

    fn stored(&self) -> String {
        self.0.iter().map(|mark| format!("{}\t{}\n", mark.id, mark.at)).collect()
    }

    fn restored(stored: &str) -> Self {
        let mut marks: Vec<Mark> = Vec::new();
        for line in stored.lines() {
            let (id, at) = line.split_once('\t').unwrap_or((line, ""));
            let id = id.trim();
            if is_module_id(id) && !marks.iter().any(|mark| mark.id == id) {
                marks.push(Mark { id: id.to_string(), at: at.trim().parse().unwrap_or(0) });
            }
            if marks.len() == MAX_BOOKMARKS {
                break;
            }
        }
        Self(marks)
    }
}

/// What `Marks::flip` did.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Flip {
    Marked,
    Unmarked(Mark),
    Full,
    /// No id: nothing.
    Refused,
}

/// The visitor's marked modules (the active profile's), shared through context. Empty on the
/// server, always. The last mark given or taken away is kept beside them for the note after it
/// (`last`), and what the marks taken away in this tab had (`Forgotten`), so that marking one again
/// brings it back as it was.
#[derive(Clone, Copy)]
pub struct Bookmarks(RwSignal<Marks>, Kept, RwSignal<Option<Marked>>, StoredValue<Vec<Forgotten>>);

/// A mark taken away in this tab, with its time and what it had (its collections, its note): marked
/// again, the module comes back as it was (oberflaeche.html, „Die Zeile": a tap on the bookmark of a
/// row dimmed on the Merkliste „holt es zurück"; a slip of M on a module's page loses nothing).
/// Kept for the tab's life alone, by profile, never stored.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Forgotten {
    profile: u16,
    mark: Mark,
    belongings: Belongings,
}

impl Bookmarks {
    /// Reads what this browser has stored for the active profile and provides the list. Another
    /// tab of the same browser may change it, or another profile becomes the active one; this one
    /// follows.
    pub fn provide() -> Self {
        let kept = Kept::of(STORAGE_KEY);
        let bookmarks = Bookmarks(RwSignal::new(load(kept)), kept, RwSignal::new(None), StoredValue::new(Vec::new()));
        provide_context(bookmarks);
        kept.follow(move || {
            let stored = load(kept);
            if bookmarks.0.with_untracked(|marks| *marks != stored) {
                bookmarks.0.set(stored);
            }
        });
        bookmarks
    }

    pub fn expect() -> Option<Self> {
        use_context::<Bookmarks>()
    }

    /// The profile whose marks these are. Tracked: a page that lists what was marked when it was
    /// opened starts again from another profile's marks.
    pub fn profile(self) -> u16 {
        self.1.profile()
    }

    /// Whether the module is marked. Tracked: read it in a memo per button, not in a list.
    pub fn is_marked(self, id: &str) -> bool {
        self.0.with(|marks| marks.contains(id))
    }

    /// The same without subscribing: for a handler that asks once (a swiped row, `folia_widgets::swipe`).
    pub fn is_marked_untracked(self, id: &str) -> bool {
        self.0.with_untracked(|marks| marks.contains(id))
    }

    /// Marks the module or takes its mark away, as `flip`. `true` if it is marked afterwards.
    pub fn toggle(self, id: &str) -> bool {
        matches!(self.flip(id, ""), Some(Toggled::Marked { .. }))
    }

    /// Marks the module or takes its mark away, with its collections and its note: a new mark goes
    /// into the collections of the default for new marks, a mark taken away takes the module out
    /// of its collections and its note with it (`collections::Shelf::forget`). What it did is the
    /// last change (`last`), for the note after it; `title` is the module's, for that note. `None`
    /// for what is no module id, which is never marked.
    pub fn flip(self, id: &str, title: &str) -> Option<Toggled> {
        let shelf = Collections::expect();
        // Taken away in this tab before: back as it was, at its time, without the default.
        if let Some(forgotten) = self.recall(id) {
            let Forgotten { mark, belongings, .. } = forgotten;
            self.0.update(|marks| marks.put_back(mark));
            if let Some(shelf) = shelf {
                shelf.update(|shelf| shelf.bring_back(id, &belongings));
            }
            self.save();
            return Some(self.noted(id, title, Toggled::Marked { into: BTreeSet::new() }));
        }
        let mut flipped = Flip::Refused;
        self.0.update(|marks| flipped = marks.flip(id, now()));
        let what = match flipped {
            Flip::Refused => return None,
            Flip::Full => Toggled::Full,
            Flip::Marked => Toggled::Marked { into: shelf.map(|shelf| shelf.update(|shelf| shelf.marked(id))).unwrap_or_default() },
            Flip::Unmarked(mark) => {
                let belongings = shelf.map(|shelf| shelf.update(|shelf| shelf.forget(id))).unwrap_or_default();
                self.remember(mark.clone(), belongings.clone());
                Toggled::Unmarked { mark, belongings }
            }
        };
        if what != Toggled::Full {
            self.save();
        }
        Some(self.noted(id, title, what))
    }

    /// Marks a module that is not marked, as it is: no default, no note after it (a collection
    /// chosen for it, or a note written, `folia_widgets::collect`). Where the Merkliste is full the
    /// note says so. Whether it is marked afterwards.
    pub fn mark_quiet(self, id: &str, title: &str) -> bool {
        if self.is_marked_untracked(id) {
            return true;
        }
        if let Some(Forgotten { mark, .. }) = self.recall(id) {
            self.0.update(|marks| marks.put_back(mark));
            self.save();
            return true;
        }
        let mut flipped = Flip::Refused;
        self.0.update(|marks| flipped = marks.flip(id, now()));
        match flipped {
            Flip::Marked => {
                self.save();
                true
            }
            Flip::Full => {
                self.noted(id, title, Toggled::Full);
                false
            }
            Flip::Unmarked(mark) => {
                // Marked meanwhile (another tab): as it was.
                self.0.update(|marks| marks.put_back(mark));
                true
            }
            Flip::Refused => false,
        }
    }

    /// Keeps what a mark taken away had, for marking it again (`Forgotten`): the newest of each
    /// module, at most as many as can be marked.
    fn remember(self, mark: Mark, belongings: Belongings) {
        let profile = self.1.profile_untracked();
        self.3.update_value(|forgotten| {
            forgotten.retain(|kept| kept.mark.id != mark.id);
            forgotten.insert(0, Forgotten { profile, mark, belongings });
            forgotten.truncate(MAX_MARKS);
        });
    }

    /// What a module had when its mark was taken away in this tab, in this profile, if it is not
    /// marked now and there is room for it; taken out of what is kept.
    fn recall(self, id: &str) -> Option<Forgotten> {
        if self.is_marked_untracked(id) || self.room_untracked() == 0 {
            return None;
        }
        let profile = self.1.profile_untracked();
        let mut found = None;
        self.3.update_value(|forgotten| {
            if let Some(at) = forgotten.iter().position(|kept| kept.profile == profile && kept.mark.id == id) {
                found = Some(forgotten.remove(at));
            }
        });
        found
    }

    /// Records a change as the last one, for the note after it, and hands it back.
    fn noted(self, id: &str, title: &str, what: Toggled) -> Toggled {
        let serial = self.2.with_untracked(|last| last.as_ref().map_or(0, |last| last.serial.wrapping_add(1)));
        self.2.set(Some(Marked { id: id.to_string(), title: title.to_string(), what: what.clone(), serial }));
        what
    }

    /// „Rückgängig" of a change (`flip`): a new mark taken away again, with the collections the
    /// default put it into; a mark taken away put back where its time puts it, with its
    /// collections and its note.
    pub fn undo(self, change: &Marked) {
        match &change.what {
            Toggled::Marked { .. } => {
                let mut taken = None;
                self.0.update(|marks| {
                    if let Some(at) = marks.0.iter().position(|mark| mark.id == change.id) {
                        taken = Some(marks.0.remove(at));
                    }
                });
                let belongings = Collections::expect().map(|shelf| shelf.update(|shelf| shelf.forget(&change.id))).unwrap_or_default();
                if let Some(mark) = taken {
                    self.remember(mark, belongings);
                }
            }
            Toggled::Unmarked { mark, belongings } => {
                self.0.update(|marks| marks.put_back(mark.clone()));
                if let Some(shelf) = Collections::expect() {
                    shelf.update(|shelf| shelf.bring_back(&change.id, belongings));
                }
            }
            Toggled::Full => return,
        }
        self.save();
    }

    /// The last mark given or taken away (`flip`), for the note after it. Tracked.
    pub fn last(self) -> Option<Marked> {
        self.2.get()
    }

    /// How many more modules can be marked.
    pub fn room(self) -> usize {
        MAX_MARKS.saturating_sub(self.count())
    }

    fn room_untracked(self) -> usize {
        MAX_MARKS.saturating_sub(self.0.with_untracked(|marks| marks.0.len()))
    }

    /// Takes every mark away, with the modules' collections and notes (the collections stay), and
    /// hands back what was marked and what they had, for `restore`.
    pub fn clear(self) -> Cleared {
        let marks = self.marks_untracked();
        self.0.set(Marks::default());
        self.save();
        let belongings = Collections::expect().map(|shelf| shelf.update(|shelf| shelf.forget_all())).unwrap_or_default();
        Cleared { marks, belongings }
    }

    /// Takes these marks away, with the modules' collections and notes, and hands back what they
    /// were, for `restore` (a collection deleted „Auch 2 entfernen", `folia_widgets::marknote`; the
    /// rows selected on the Merkliste). Each is kept as a mark taken away by its switch is: marked
    /// again in this tab, a module gets back what it had.
    pub fn take_away(self, ids: &[String]) -> Cleared {
        let mut marks = Vec::new();
        self.0.update(|current| {
            current.0.retain(|mark| {
                let taken = ids.contains(&mark.id);
                if taken {
                    marks.push(mark.clone());
                }
                !taken
            });
        });
        self.save();
        let belongings: Vec<(String, Belongings)> = Collections::expect().map(|shelf| shelf.update(|shelf| marks.iter().map(|mark| (mark.id.clone(), shelf.forget(&mark.id))).collect())).unwrap_or_default();
        for mark in marks.iter().rev() {
            let had = belongings.iter().find(|(id, _)| *id == mark.id).map(|(_, had)| had.clone()).unwrap_or_default();
            self.remember(mark.clone(), had);
        }
        Cleared { marks, belongings }
    }

    /// Puts marks back (the „Rückgängig" of `clear`), with their collections and notes; what has
    /// been marked since stays.
    pub fn restore(self, cleared: Cleared) {
        self.0.update(|current| current.merge(cleared.marks));
        self.save();
        if let Some(shelf) = Collections::expect() {
            shelf.update(|shelf| {
                for (module, belongings) in &cleared.belongings {
                    shelf.bring_back(module, belongings);
                }
            });
        }
    }

    /// Marks these now, in the order given (a list brought over from another device), as many as
    /// there is room for (`MAX_MARKS`). What is marked already keeps its place. How many were
    /// marked.
    pub fn add_all(self, ids: &[String]) -> usize {
        let now = now();
        let mut added = 0;
        self.0.update(|current| {
            let room = MAX_MARKS.saturating_sub(current.0.len());
            let new: Vec<Mark> = ids.iter().filter(|id| is_module_id(id) && !current.contains(id)).take(room).map(|id| Mark { id: id.clone(), at: now }).collect();
            added = new.len();
            current.0.splice(0..0, new);
        });
        self.save();
        added
    }

    pub fn count(self) -> usize {
        self.0.with(|marks| marks.0.len())
    }

    /// The same without subscribing: for what is built once.
    pub fn count_untracked(self) -> usize {
        self.0.with_untracked(|marks| marks.0.len())
    }

    /// The marked modules, the newest first.
    pub fn marks(self) -> Vec<Mark> {
        self.0.with(|marks| marks.0.clone())
    }

    /// The same without subscribing: for a page that decides itself when its list changes.
    pub fn marks_untracked(self) -> Vec<Mark> {
        self.0.with_untracked(|marks| marks.0.clone())
    }

    fn save(self) {
        self.1.write(&self.0.with_untracked(Marks::stored));
    }
}

/// What this browser has stored. Nothing on the server, and nothing in a browser that refuses
/// storage (marks then last as long as the page).
fn load(kept: Kept) -> Marks {
    kept.read().map(|stored| Marks::restored(&stored)).unwrap_or_default()
}

/// A stored list as the store reads it and writes it back (a profile brought from a file,
/// `profiles::Backup`).
pub(crate) fn checked(stored: &str) -> String {
    Marks::restored(stored).stored()
}

/// How many modules a stored list marks.
pub(crate) fn count(stored: &str) -> usize {
    Marks::restored(stored).0.len()
}

pub(crate) fn now() -> u64 {
    #[cfg(feature = "csr")]
    {
        (web_sys::js_sys::Date::now() / 1000.0).max(0.0) as u64
    }
    #[cfg(not(feature = "csr"))]
    0
}

/// How a list travels to another device without a server in between: in the fragment of a link
/// to the marked modules (`/bookmarks#m=<code>`). A browser never sends the fragment of an address
/// anywhere (not with the request, not as a referrer), so the list reaches neither the server nor
/// its logs; the page reads it, asks the visitor, and takes it out of the address.
///
/// The code (`pack`) holds the ids as a set of numbers, in ascending order, each as its distance
/// from the one before, and ends in two characters that check the rest: a link cut short or a
/// character typed wrong is noticed instead of bringing other modules. The order of the list does
/// not travel (owner, 2026-09-23: it does not matter), which is what makes the code short: 20 marked
/// modules take about 40 characters instead of 120. It begins with the number of its layout
/// (`VERSION`), so that a later layout can be read beside it.
const TRANSFER: &str = "m=";
/// What a code of the list is for. It is part of what the check characters check, so a code of
/// another kind is never taken for a list.
const KIND: &str = "bookmarks";
/// The layout of `Transfer` that codes are written in, four bits of the code
/// (`folia_pack::to_versioned_code`; owner, 2026-09-25). A change beyond adding a field at the end takes
/// the next one.
const VERSION: u8 = 1;

/// The marked modules as a code carries them. A module id is five digits, so it travels as a
/// number; an id that is none travels as it is. Since Folia 1.39.0 a code also carries collections
/// and notes (folia/design/merken/konzept.html, „Weitergeben"): a collection's link its name, the
/// link for another device every collection, and the notes where they go along. They are fields
/// at the end, empty in a code of only modules, so a code of before reads as it did and one of
/// only modules is what it was (`folia_pack`'s rule for lasting codes).
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Transfer {
    /// The ids that are numbers.
    #[serde(with = "folia_pack::set")]
    numbers: Vec<u64>,
    /// Every other id (the catalog has none). Empty, this costs nothing: it ends the code.
    others: Vec<String>,
    /// The collections: each its name and its modules, by their places in the list a reader
    /// gets (`Transfer::ids`).
    collections: Vec<Carried>,
    /// The notes: the place of a module and its note.
    notes: Vec<(u64, String)>,
}

/// A collection as a code carries it.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Carried {
    name: String,
    #[serde(with = "folia_pack::set")]
    places: Vec<u64>,
}

/// What a link of the Merkliste brings: modules; a collection's link its name, the link for another
/// device every collection, each with its modules; and the notes that go along, by module.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Brought {
    pub ids: Vec<String>,
    pub collections: Vec<(String, Vec<String>)>,
    pub notes: Vec<(String, String)>,
}

impl Brought {
    /// Only modules.
    pub fn modules(ids: &[String]) -> Self {
        Brought { ids: ids.to_vec(), ..Brought::default() }
    }
}

impl Transfer {
    fn of(brought: &Brought) -> Self {
        let mut transfer = Transfer::default();
        for id in &brought.ids {
            match id.parse::<u64>().ok().filter(|number| number.to_string() == *id) {
                Some(number) => transfer.numbers.push(number),
                None => transfer.others.push(id.clone()),
            }
        }
        // The places as a reader sees them (`ids`): the numbers in ascending order, each once,
        // then the others.
        let mut numbers = transfer.numbers.clone();
        numbers.sort_unstable();
        numbers.dedup();
        let order: Vec<String> = numbers.into_iter().map(|number| number.to_string()).chain(transfer.others.iter().cloned()).collect();
        let place = |id: &str| order.iter().position(|known| known == id).and_then(|at| u64::try_from(at).ok());
        transfer.collections = brought.collections.iter().map(|(name, ids)| Carried { name: name.clone(), places: ids.iter().filter_map(|id| place(id)).collect() }).collect();
        transfer.notes = brought.notes.iter().filter_map(|(id, note)| Some((place(id)?, note.clone()))).collect();
        transfer
    }

    /// What it brings: the ids (the numbers in ascending order, then the others), and the
    /// collections and notes by them.
    fn brought(self) -> Brought {
        let ids: Vec<String> = self.numbers.into_iter().map(|number| number.to_string()).chain(self.others).collect();
        let at = |place: u64| usize::try_from(place).ok().and_then(|place| ids.get(place)).cloned();
        let collections = self.collections.into_iter().map(|carried| (carried.name, carried.places.into_iter().filter_map(at).collect())).collect();
        let notes = self.notes.into_iter().filter_map(|(place, note)| Some((at(place)?, note))).collect();
        Brought { ids, collections, notes }
    }
}

/// The fragment (without `#`) that carries what `brought` names; `None` for more than a code may
/// hold (thousands of ids that are no module numbers, which the catalog does not have).
pub fn share_fragment(brought: &Brought) -> Option<String> {
    folia_pack::to_versioned_code(KIND, VERSION, &Transfer::of(brought)).ok().map(|code| format!("{TRANSFER}{code}"))
}

/// The fragment that carries these modules alone.
pub fn transfer_fragment(ids: &[String]) -> Option<String> {
    share_fragment(&Brought::modules(ids))
}

/// A fragment that carries a list, but not one that can be read: cut short, or a character of it
/// typed wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BrokenLink;

/// What a fragment brings, checked like anything from outside: module ids that are ids, each once,
/// as many as a browser keeps; collections by a clean name, each once (`collections::name_key`),
/// as many as there may be, with modules of the list; notes of modules of the list, clean, one
/// each. Nothing for any other fragment.
pub fn brought_from_fragment(fragment: &str) -> Result<Brought, BrokenLink> {
    let Some(code) = fragment.trim_start_matches('#').strip_prefix(TRANSFER) else {
        return Ok(Brought::default());
    };
    let raw = folia_pack::from_versioned_code::<Transfer>(KIND, VERSION, code.trim()).map_err(|_| BrokenLink)?.brought();
    let mut ids: Vec<String> = Vec::new();
    for id in raw.ids.into_iter().filter(|id| is_module_id(id)) {
        if !ids.contains(&id) {
            ids.push(id);
        }
        if ids.len() == MAX_BOOKMARKS {
            break;
        }
    }
    let mut collections: Vec<(String, Vec<String>)> = Vec::new();
    for (name, members) in raw.collections {
        let name = clean_name(&name);
        if name.is_empty() {
            continue;
        }
        let members: Vec<String> = members.into_iter().filter(|id| ids.contains(id)).collect();
        match collections.iter().position(|(known, _)| name_key(known) == name_key(&name)) {
            Some(at) => {
                if let Some((_, known)) = collections.get_mut(at) {
                    for id in members {
                        if !known.contains(&id) {
                            known.push(id);
                        }
                    }
                }
            }
            None if collections.len() < MAX_COLLECTIONS => collections.push((name, members)),
            None => {}
        }
    }
    let mut notes: Vec<(String, String)> = Vec::new();
    for (id, note) in raw.notes {
        let note = clean_note(&note);
        if ids.contains(&id) && !note.is_empty() && !notes.iter().any(|(known, _)| *known == id) {
            notes.push((id, note));
        }
    }
    Ok(Brought { ids, collections, notes })
}

/// The modules a fragment carries, checked like anything from outside. None for any other fragment.
pub fn ids_from_fragment(fragment: &str) -> Result<Vec<String>, BrokenLink> {
    brought_from_fragment(fragment).map(|brought| brought.ids)
}

/// Where a mark button stands decides what it looks like.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkLook {
    /// At the end of a row of a list: the icon alone.
    Row,
    /// In the line of a module's badges, at its right end, in the preview and on the module's
    /// page: the switch with its label and its shortcut (R10).
    Hero,
    /// Among the actions of a sidebar.
    Action,
}

/// The switch that marks a module. Same behaviour in every look; `M` presses the one that
/// belongs to what the visitor is looking at (`enhance.js`, `data-action="mark"`).
///
/// Beside it at a module and at a row its arrow (folia/design/tasten/konzept.html, „Knopf mit
/// Pfeil"; owner, 2026-10-20: „Ja bitte setze deine Vorschläge um"): „Sammlung und Notiz"
/// (`collections::Collecting`), which ⇧M opens as well (`data-action="collect"`); a collection chosen
/// there marks the module. At a row it shows while the row is pointed at or has the focus.
///
/// At a module (`Hero`) it says more (folia/design/merken/oberflaeche.html, „Am Modul"): marked, it
/// carries the module's collections small, as „Einplanen" carries its semester; not marked, where a
/// default for new marks is set, where the mark will go („Merken · in Für Lena").
///
/// It is part of server HTML (unpressed, like for everybody) only where leaving it out would make
/// the page jump when the app takes over; the stylesheet shows it once the app runs.
#[component]
pub fn MarkButton(
    #[prop(into)] id: String,
    #[prop(into)] title: String,
    look: MarkLook,
    /// At a row of a list box (`folia_widgets::list::Row`'s `position`): the buttons are the
    /// mouse's, which a screen reader leaves out (a list box holds options alone); the keyboard has
    /// M and Shift + M at the row.
    #[prop(optional)]
    quiet: bool,
) -> impl IntoView {
    let t = i18n::t();
    let bookmarks = Bookmarks::expect();
    let marked = {
        let id = id.clone();
        Memo::new(move |_| bookmarks.is_some_and(|bookmarks| bookmarks.is_marked(&id)))
    };
    let toggle = {
        let (id, title) = (id.clone(), title.clone());
        move |_: leptos::ev::MouseEvent| {
            if let Some(bookmarks) = bookmarks {
                bookmarks.flip(&id, &title);
            }
        }
    };
    let pressed = move || if marked.get() { "true" } else { "false" };
    let hint = move || if marked.get() { t.marks.saved_hint } else { t.marks.save_hint };
    let label = move || if marked.get() { t.marks.saved } else { t.marks.save };
    // As a command of „Befehle" (folia_shell::commands): what a click does now, with its keys.
    let command = move || if marked.get() { t.marks.unsave } else { t.marks.save };
    match look {
        // In a list the row is the stop of the Tab key, as before there were marks; the keyboard
        // marks with M on the row (the head of the list says so), and ⇧M is the arrow.
        MarkLook::Row => {
            let collect = {
                let (id, title) = (id.clone(), title.clone());
                move |ev: leptos::ev::MouseEvent| {
                    if let Some(collecting) = Collecting::expect() {
                        collecting.open(CollectAsk { id: id.clone(), title: title.clone(), more: Vec::new(), at: pair_of(&ev), note: false, from_note: None });
                    }
                }
            };
            view! {
                <button class="mark-toggle icon-btn" type="button" tabindex="-1" aria-hidden=quiet.then_some("true") data-action="mark" data-id=id.clone() data-title=title.clone() on:click=toggle aria-pressed=pressed aria-label=(t.marks.save_title)(&title) title=hint data-command=command data-command-keys="M">
                    <Icon name="bookmark"/>
                </button>
                <button class="row-more icon-btn" type="button" tabindex="-1" aria-hidden=quiet.then_some("true") data-action="collect" data-id=id aria-haspopup="dialog" aria-label=(t.marks.collect_title)(&title) title=t.marks.collect_hint on:click=collect data-command=t.marks.collect data-command-keys="Shift+M" data-command-icon="layers">
                    <Icon name="chevron-down"/>
                </button>
            }
            .into_any()
        }
        MarkLook::Hero => {
            let collections = Collections::expect();
            // Under „Gemerkt" the module's collections; under „Merken" where the default puts it.
            let small = {
                let id = id.clone();
                Memo::new(move |_| {
                    let marked = marked.get();
                    collections?.with(|shelf| {
                        // Two by name, then how many more, as the note's field and the rows say it.
                        let names = |ids: &mut dyn Iterator<Item = u32>| {
                            let names: Vec<String> = ids.filter_map(|c| shelf.collection(c)).map(|c| c.name.clone()).collect();
                            match names.as_slice() {
                                [first, second, rest @ ..] if !rest.is_empty() => format!("{first}, {second} +{}", rest.len()),
                                _ => names.join(", "),
                            }
                        };
                        let said = if marked { names(&mut shelf.of(&id).into_iter()) } else { shelf.standard().map(|ids| names(&mut ids.iter().copied())).filter(|names| !names.is_empty()).map(|names| (t.marks.mark_into)(&names)).unwrap_or_default() };
                        (!said.is_empty()).then_some(said)
                    })
                })
            };
            let collect = {
                let (id, title) = (id.clone(), title.clone());
                move |ev: leptos::ev::MouseEvent| {
                    if let Some(collecting) = Collecting::expect() {
                        collecting.open(CollectAsk { id: id.clone(), title: title.clone(), more: Vec::new(), at: pair_of(&ev), note: false, from_note: None });
                    }
                }
            };
            // The arrow is there marked or not, in server HTML as well, so nothing moves when the
            // app takes over (R15).
            view! {
                <span class="mark-pair split">
                    // `with-note` while it says where it marks (app.css), as the plan's switch.
                    <button class="mark-toggle mark-switch hit" class:with-note=move || small.with(Option::is_some) type="button" data-action="mark" data-id=id data-title=title on:click=toggle aria-pressed=pressed title=hint data-command=command data-command-keys="M">
                        <Icon name="bookmark"/><span>{label}</span>{move || small.get().map(|small| view! { <small>{small}</small> })}<Shortcut keys="M"/>
                    </button>
                    <button class="mark-more hit" type="button" data-action="collect" aria-label=t.marks.collect title=t.marks.collect_hint aria-haspopup="dialog" on:click=collect data-command=t.marks.collect data-command-keys="Shift+M" data-command-icon="layers">
                        <Icon name="chevron-down"/>
                    </button>
                </span>
            }
            .into_any()
        }
        MarkLook::Action => view! {
            <button class="mark-toggle action" type="button" data-action="mark" data-id=id data-title=title on:click=toggle aria-pressed=pressed title=hint data-command=command data-command-keys="M">
                <Icon name="bookmark"/><span>{label}</span>
            </button>
        }
        .into_any(),
    }
}

/// The box of the pair of a module's switch and its arrow, whose arrow was clicked (at a row, the
/// row's): the menu stands under it. „Einplanen"'s arrow as well (`studyplan::PlanButton`).
pub(crate) fn pair_of(ev: &leptos::ev::MouseEvent) -> Option<Anchor> {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let arrow = ev.current_target()?.dyn_into::<web_sys::Element>().ok()?;
        let pair = arrow.parent_element().unwrap_or(arrow);
        let rect = pair.get_bounding_client_rect();
        Some(Anchor { left: rect.left(), top: rect.top(), right: rect.right(), bottom: rect.bottom() })
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = ev;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marking_puts_the_newest_first_and_a_second_time_takes_it_away() {
        let mut marks = Marks::default();
        assert!(marks.toggle("11101", 10));
        assert!(marks.toggle("12204", 20));
        assert_eq!(marks.0.iter().map(|mark| mark.id.as_str()).collect::<Vec<_>>(), ["12204", "11101"]);
        assert!(!marks.toggle("11101", 30), "the second time takes the mark away");
        assert!(marks.contains("12204") && !marks.contains("11101"));
        assert!(!marks.toggle("no id", 40) && marks.0.len() == 1, "what is no id is never stored");

        // Putting back what was cleared keeps what has been marked since, the newest first.
        let cleared = vec![Mark { id: "12204".into(), at: 20 }, Mark { id: "11101".into(), at: 10 }];
        let mut since = Marks(vec![Mark { id: "13001".into(), at: 50 }, Mark { id: "12204".into(), at: 45 }]);
        since.merge(cleared);
        assert_eq!(since.0.iter().map(|mark| (mark.id.as_str(), mark.at)).collect::<Vec<_>>(), [("13001", 50), ("12204", 45), ("11101", 10)]);
    }

    #[test]
    fn at_most_256_and_a_mark_taken_away_comes_back_in_its_place() {
        let mut marks = Marks::default();
        for n in 0..MAX_MARKS {
            assert_eq!(marks.flip(&(10_000 + n).to_string(), n as u64), Flip::Marked);
        }
        // The 257th is refused, and nothing falls out.
        assert_eq!(marks.flip("99999", 999), Flip::Full);
        assert_eq!(marks.0.len(), MAX_MARKS);
        // Taking one away makes room; what is no id is never marked.
        let Flip::Unmarked(gone) = marks.flip("10100", 1000) else { panic!("10100 was marked") };
        assert_eq!(gone, Mark { id: "10100".into(), at: 100 });
        assert_eq!(marks.flip("no id", 1001), Flip::Refused);
        // „Rückgängig": back where its time puts it, between the newer and the older marks.
        marks.put_back(gone.clone());
        let at = marks.0.iter().position(|mark| mark.id == "10100").unwrap();
        assert_eq!((marks.0[at - 1].at, marks.0[at + 1].at), (101, 99));
        marks.put_back(gone);
        assert_eq!(marks.0.len(), MAX_MARKS, "not twice");
        // A list that holds more from before is read as it is.
        let long: String = (0..300).map(|n| format!("{}\t{n}\n", 10_000 + n)).collect();
        assert_eq!(Marks::restored(&long).0.len(), 300);
    }

    fn owned(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| id.to_string()).collect()
    }

    #[test]
    fn a_list_travels_in_the_fragment_of_a_link() {
        let ids = owned(&["12204", "11101", "13849"]);
        // Links with this code are out there: a change of the code or of `Transfer` breaks them.
        assert_eq!(transfer_fragment(&ids).as_deref(), Some("m=PZsLKbA_QdJty"));
        let there_and_back = |ids: &[String]| ids_from_fragment(&format!("#{}", transfer_fragment(ids).unwrap()));
        // The same modules, not the same order: the numbers ascending, then ids that are none.
        assert_eq!(there_and_back(&ids), Ok(owned(&["11101", "12204", "13849"])));
        let mixed = owned(&["FUES-7", "41000", "11101", "0123", "99999999", "12204", "Z", "11101"]);
        assert_eq!(there_and_back(&mixed), Ok(owned(&["11101", "12204", "41000", "99999999", "FUES-7", "0123", "Z"])));
        // Whatever else a fragment may be.
        assert_eq!(ids_from_fragment("#termine"), Ok(Vec::new()));
        assert_eq!(ids_from_fragment(""), Ok(Vec::new()));
        // What a code brings is checked like anything from outside: ids, each once, at most so many.
        let odd = owned(&["12204", "12204", "<script>", "../etc", "", "13001"]);
        assert_eq!(there_and_back(&odd), Ok(owned(&["12204", "13001"])));
        let many: Vec<String> = (0..MAX_BOOKMARKS + 50).map(|n| (10_000 + n).to_string()).collect();
        assert_eq!(there_and_back(&many), Ok(many[..MAX_BOOKMARKS].to_vec()));
        // Longer than a code may be: no link.
        let long: Vec<String> = (0..MAX_BOOKMARKS).map(|n| format!("x{n:031}")).collect();
        assert_eq!(transfer_fragment(&long), None);
    }

    #[test]
    fn a_link_brings_collections_and_notes() {
        let brought = Brought {
            ids: owned(&["12204", "11101", "13849", "FUES-7"]),
            collections: vec![("Für Lena".into(), owned(&["13849", "12204"])), ("Witzig".into(), owned(&["FUES-7"]))],
            notes: vec![("11101".into(), "**früh** anmelden\n- Lena fragen".into())],
        };
        let there_and_back = |brought: &Brought| brought_from_fragment(&format!("#{}", share_fragment(brought).unwrap())).unwrap();
        // In the order a reader gets the modules: the numbers ascending, then the others.
        assert_eq!(
            there_and_back(&brought),
            Brought {
                ids: owned(&["11101", "12204", "13849", "FUES-7"]),
                collections: vec![("Für Lena".into(), owned(&["12204", "13849"])), ("Witzig".into(), owned(&["FUES-7"]))],
                notes: vec![("11101".into(), "**früh** anmelden\n- Lena fragen".into())],
            }
        );
        // A code of only modules is what it was, and one of before reads as it did.
        assert_eq!(share_fragment(&Brought::modules(&owned(&["12204", "11101", "13849"]))).as_deref(), Some("m=PZsLKbA_QdJty"));
        assert_eq!(brought_from_fragment("#m=PZsLKbA_QdJty").map(|brought| brought.collections.len() + brought.notes.len()), Ok(0));
        // Checked like anything from outside: a name clean and there once (case does not count),
        // none without one; modules of the list; a note clean, of the list, once.
        let odd = Brought {
            ids: owned(&["12204", "11101"]),
            collections: vec![("  Für   Lena ".into(), owned(&["12204", "99999"])), ("für lena".into(), owned(&["11101"])), ("   ".into(), owned(&["12204"]))],
            notes: vec![("12204".into(), "  \n ".into()), ("11101".into(), "eins".into()), ("11101".into(), "zwei".into()), ("13849".into(), "nicht dabei".into())],
        };
        let back = there_and_back(&odd);
        assert_eq!(back.collections, vec![("Für Lena".to_string(), owned(&["12204", "11101"]))]);
        assert_eq!(back.notes, vec![("11101".to_string(), "eins".to_string())]);
    }

    #[test]
    fn a_link_that_lost_or_changed_a_character_brings_nothing() {
        let fragment = transfer_fragment(&owned(&["12204", "11101", "13849", "12205"])).unwrap();
        let typed_wrong: String = fragment.char_indices().map(|(at, c)| if at == 6 { if c == 'x' { 'y' } else { 'x' } } else { c }).collect();
        let swapped: String = {
            let mut chars: Vec<char> = fragment.chars().collect();
            chars.swap(4, 5);
            chars.into_iter().collect()
        };
        assert_ne!(swapped, fragment);
        // A code of a later layout than this build reads.
        let later = format!("m={}", folia_pack::to_versioned_code(KIND, VERSION + 1, &Transfer::of(&Brought::modules(&owned(&["12204", "11101"])))).unwrap());
        for broken in [typed_wrong, swapped, fragment[..fragment.len() - 1].to_string(), format!("{fragment}A"), "m=".to_string(), "m=12204".to_string(), later] {
            assert_eq!(ids_from_fragment(&broken), Err(BrokenLink), "{broken}");
        }
    }

    #[test]
    fn a_marked_list_takes_a_few_characters_a_module() {
        // 20 modules as somebody marks them: from three departments, several from one in a row.
        let ids = owned(&[
            "12204", "12205", "12311", "11101", "11103", "11205", "13849", "13850", "12402", "12450", "11304", "13901", "13902", "13911", "12207", "12230", "11110", "11115",
            "13003", "12601",
        ]);
        let fragment = transfer_fragment(&ids).unwrap();
        let listed = ids.join(",");
        assert!(fragment.len() <= 40 && fragment.len() * 3 < listed.len(), "{} characters instead of {}", fragment.len(), listed.len());
    }

    #[test]
    fn what_is_stored_is_read_like_anything_from_outside() {
        let mut marks = Marks::default();
        marks.toggle("11101", 1_758_391_200);
        marks.toggle("FUES-7", 1_758_391_260);
        assert_eq!(marks.stored(), "FUES-7\t1758391260\n11101\t1758391200\n");
        assert_eq!(Marks::restored(&marks.stored()), marks);

        // Broken lines, ids that cannot be ids, a module twice, a time that is none.
        let restored = Marks::restored("12204\tyesterday\n\n<script>\t5\n../../etc\n12204\t9\n 13001 \t7\r\n");
        assert_eq!(restored, Marks(vec![Mark { id: "12204".into(), at: 0 }, Mark { id: "13001".into(), at: 7 }]));
        assert_eq!(Marks::restored(""), Marks::default());

        let many: String = (0..MAX_BOOKMARKS + 50).map(|n| format!("{n}\t1\n")).collect();
        assert_eq!(Marks::restored(&many).0.len(), MAX_BOOKMARKS);
    }
}
