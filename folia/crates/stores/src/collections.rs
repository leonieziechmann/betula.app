//! „Sammlungen" and notes: what only the visitor knows of the modules they marked, beside the marks
//! themselves (folia/design/merken/konzept.html, „Sammlungen und Notizen"). A module can be in any
//! number of collections, or in none („Ohne Sammlung", which is no collection of its own); each
//! has one note, in Markdown as it was typed. Like the marks, all of it lives in this browser
//! alone, under a key of its own in each profile (`STORAGE_KEY`, `profiles::KEYS`): never in a URL,
//! never in server HTML, in no request (R9, R13, R20).
//!
//! - Every name is there once (konzept.html: „Jeden Namen gibt es nur einmal"): case and the
//!   spaces at its ends do not count, „für lena" is „Für Lena". A name typed that is there already
//!   is that collection, never a second one; renaming refuses it the same way.
//! - The collections stand in the order they were made in until one is moved. The ones used last
//!   come first where a note after marking offers them (`recent`).
//! - Deleting a collection takes the collection away and leaves its modules marked; what was
//!   taken is handed back, for „Rückgängig".
//! - A default for new marks (`standard`, „Standard für neue Merker") puts what is marked from then on into the same
//!   collections, until it is switched off (oberflaeche.html, question B: „Bis man ihn ausschaltet").
//!
//! The text is read like anything from outside: a line that does not check out is dropped alone,
//! a name or a note longer than its cap is cut, and every number has an upper limit.

use std::collections::{BTreeMap, BTreeSet};

use folia_model::ids::is_module_id;
use folia_pages::MAX_BOOKMARKS;
use leptos::prelude::*;

use crate::profiles::Kept;

/// The first profile's key; another profile's has its number in it (`profiles::key_of`).
pub(crate) const STORAGE_KEY: &str = "betula.collections.v1";

/// How many collections a profile has at most: far more than anybody makes.
pub const MAX_COLLECTIONS: usize = 64;
/// How long a name may be, in characters.
pub const MAX_NAME: usize = 60;
/// How long a note may be, in characters: pages of text, a short page in storage.
pub const MAX_NOTE: usize = 4000;
/// How much text the store reads at most (the caps above, with room).
const MAX_STORED: usize = 2 * 1024 * 1024;

/// A collection: its number, which stays while it lives (a view's address names it by it,
/// `/bookmarks?collection=3`), and its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collection {
    pub id: u32,
    pub name: String,
}

/// Why a name was not taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Nothing but spaces.
    Empty,
    /// A collection of this name is there already (the number is that one's).
    Taken(u32),
    /// `MAX_COLLECTIONS` are there.
    Full,
}

/// A collection taken away, with where it stood and what was in it: what „Rückgängig" puts back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deleted {
    pub collection: Collection,
    pub at: usize,
    /// The modules that were in it.
    pub modules: Vec<String>,
    /// It was (part of) the default for new marks.
    pub was_default: bool,
}

/// What a module had beside its mark when the mark was taken away: what „Rückgängig" brings back
/// with the mark.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Belongings {
    pub collections: BTreeSet<u32>,
    pub note: Option<String>,
}

/// Everything the visitor keeps of their marked modules besides the marks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shelf {
    /// In their order.
    collections: Vec<Collection>,
    /// Which collections each module is in; a module in none has no entry.
    members: BTreeMap<String, BTreeSet<u32>>,
    /// The note of each module that has one.
    notes: BTreeMap<String, String>,
    /// The collections new marks go into, until switched off; `None` where there is no default.
    standard: Option<BTreeSet<u32>>,
    /// The collections used last, the newest first.
    recent: Vec<u32>,
}

/// A name as the visitor means it: without the spaces at its ends, inner runs of white space one
/// space, no control characters, at most `MAX_NAME` characters.
pub fn clean_name(name: &str) -> String {
    let words: Vec<&str> = name.split(|c: char| c.is_whitespace() || c.is_control()).filter(|word| !word.is_empty()).collect();
    words.join(" ").chars().take(MAX_NAME).collect()
}

/// What two names that are the same name share: the clean name in small letters.
fn name_key(name: &str) -> String {
    clean_name(name).to_lowercase()
}

/// A note as it is kept: at most `MAX_NOTE` characters, its line breaks as `\n`; empty for one of
/// nothing but white space.
fn clean_note(note: &str) -> String {
    let note = note.replace("\r\n", "\n").replace('\r', "\n");
    if note.trim().is_empty() {
        return String::new();
    }
    note.chars().filter(|c| *c == '\n' || *c == '\t' || !c.is_control()).take(MAX_NOTE).collect()
}

impl Shelf {
    pub fn collections(&self) -> &[Collection] {
        &self.collections
    }

    pub fn collection(&self, id: u32) -> Option<&Collection> {
        self.collections.iter().find(|collection| collection.id == id)
    }

    /// The collection of this name, as `name_key` compares names.
    pub fn find(&self, name: &str) -> Option<u32> {
        let key = name_key(name);
        self.collections.iter().find(|collection| name_key(&collection.name) == key).map(|collection| collection.id)
    }

    /// A new collection at the end, empty; its number. One of a name that is there is refused with
    /// that one's number.
    pub fn create(&mut self, name: &str) -> Result<u32, Refused> {
        let name = clean_name(name);
        if name.is_empty() {
            return Err(Refused::Empty);
        }
        if let Some(id) = self.find(&name) {
            return Err(Refused::Taken(id));
        }
        if self.collections.len() >= MAX_COLLECTIONS {
            return Err(Refused::Full);
        }
        // A number no collection had in this text: a view's address of a deleted one names none.
        let id = self.collections.iter().map(|collection| collection.id).chain(self.recent.iter().copied()).max().unwrap_or(0).saturating_add(1);
        self.collections.push(Collection { id, name });
        Ok(id)
    }

    /// Renames a collection; the same name in other letters is fine, another collection's is not.
    pub fn rename(&mut self, id: u32, name: &str) -> Result<(), Refused> {
        let name = clean_name(name);
        if name.is_empty() {
            return Err(Refused::Empty);
        }
        match self.find(&name) {
            Some(other) if other != id => return Err(Refused::Taken(other)),
            _ => {}
        }
        if let Some(collection) = self.collections.iter_mut().find(|collection| collection.id == id) {
            collection.name = name;
        }
        Ok(())
    }

    /// Takes a collection away; its modules stay marked. What it was, for `undelete`.
    pub fn delete(&mut self, id: u32) -> Option<Deleted> {
        let at = self.collections.iter().position(|collection| collection.id == id)?;
        let collection = self.collections.remove(at);
        let mut modules = Vec::new();
        self.members.retain(|module, ids| {
            if ids.remove(&id) {
                modules.push(module.clone());
            }
            !ids.is_empty()
        });
        let was_default = self.standard.as_mut().is_some_and(|standard| standard.remove(&id));
        if self.standard.as_ref().is_some_and(BTreeSet::is_empty) {
            self.standard = None;
        }
        self.recent.retain(|used| *used != id);
        Some(Deleted { collection, at, modules, was_default })
    }

    /// Puts a deleted collection back where it stood, with its modules; a name made meanwhile
    /// keeps the collection that has it, and the one put back is not there again.
    pub fn undelete(&mut self, deleted: Deleted) -> bool {
        if self.find(&deleted.collection.name).is_some() || self.collection(deleted.collection.id).is_some() || self.collections.len() >= MAX_COLLECTIONS {
            return false;
        }
        let id = deleted.collection.id;
        let at = deleted.at.min(self.collections.len());
        self.collections.insert(at, deleted.collection);
        for module in deleted.modules {
            self.members.entry(module).or_default().insert(id);
        }
        if deleted.was_default {
            self.standard.get_or_insert_with(BTreeSet::new).insert(id);
        }
        true
    }

    /// Moves a collection one place up or down.
    pub fn shift(&mut self, id: u32, up: bool) {
        let Some(at) = self.collections.iter().position(|collection| collection.id == id) else { return };
        let to = if up { at.checked_sub(1) } else { Some(at + 1).filter(|to| *to < self.collections.len()) };
        if let Some(to) = to {
            self.collections.swap(at, to);
        }
    }

    /// The collections a module is in, in their order.
    pub fn of(&self, module: &str) -> Vec<u32> {
        let Some(ids) = self.members.get(module) else { return Vec::new() };
        self.collections.iter().map(|collection| collection.id).filter(|id| ids.contains(id)).collect()
    }

    /// Whether a module is in a collection.
    pub fn holds(&self, id: u32, module: &str) -> bool {
        self.members.get(module).is_some_and(|ids| ids.contains(&id))
    }

    /// Puts a module into exactly these collections (those that exist).
    pub fn set_in(&mut self, module: &str, ids: &BTreeSet<u32>) {
        if !is_module_id(module) {
            return;
        }
        let ids: BTreeSet<u32> = ids.iter().copied().filter(|id| self.collection(*id).is_some()).collect();
        let added: Vec<u32> = ids.iter().copied().filter(|id| !self.holds(*id, module)).collect();
        if ids.is_empty() {
            self.members.remove(module);
        } else if self.members.len() < MAX_BOOKMARKS || self.members.contains_key(module) {
            self.members.insert(module.to_string(), ids);
        }
        for id in added {
            self.used(id);
        }
    }

    /// Puts a module into a collection, or takes it out. `true` if it is in it afterwards.
    pub fn toggle_in(&mut self, module: &str, id: u32) -> bool {
        let mut ids: BTreeSet<u32> = self.members.get(module).cloned().unwrap_or_default();
        let inside = !ids.remove(&id);
        if inside {
            ids.insert(id);
        }
        self.set_in(module, &ids);
        self.holds(id, module)
    }

    /// A collection was chosen for a module: it is the first the note after marking offers.
    fn used(&mut self, id: u32) {
        self.recent.retain(|used| *used != id);
        self.recent.insert(0, id);
        self.recent.truncate(MAX_COLLECTIONS);
    }

    /// The collections in the order the note after marking offers them: the ones used last first.
    pub fn by_use(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.recent.iter().copied().filter(|id| self.collection(*id).is_some()).collect();
        ids.extend(self.collections.iter().map(|collection| collection.id).filter(|id| !self.recent.contains(id)));
        ids
    }

    /// How many of the modules `marked` says are marked a collection holds.
    pub fn count_in(&self, id: u32, marked: impl Fn(&str) -> bool) -> usize {
        self.members.iter().filter(|(module, ids)| ids.contains(&id) && marked(module)).count()
    }

    /// How many modules have a note.
    pub fn notes_count(&self) -> usize {
        self.notes.len()
    }

    pub fn note(&self, module: &str) -> Option<&str> {
        self.notes.get(module).map(String::as_str)
    }

    /// What the visitor keeps of a module, as a search reads it: its note as plain text and the
    /// names of its collections; nothing where it has neither.
    pub fn kept_text(&self, module: &str) -> Option<String> {
        let names: Vec<&str> = self.of(module).into_iter().filter_map(|collection| self.collection(collection)).map(|collection| collection.name.as_str()).collect();
        let note = self.note(module).map(folia_model::text::note_plain);
        (!names.is_empty() || note.is_some()).then(|| [note.unwrap_or_default(), names.join(" ")].join(" "))
    }

    /// The names of the collections a module is in, in their order, „Wahlpflicht, Für Lena".
    pub fn names_of(&self, module: &str) -> String {
        self.collections.iter().filter(|collection| self.holds(collection.id, module)).map(|collection| collection.name.as_str()).collect::<Vec<_>>().join(", ")
    }

    /// Keeps a note; one of nothing but white space takes it away.
    pub fn set_note(&mut self, module: &str, note: &str) {
        if !is_module_id(module) {
            return;
        }
        let note = clean_note(note);
        if note.is_empty() {
            self.notes.remove(module);
        } else if self.notes.len() < MAX_BOOKMARKS || self.notes.contains_key(module) {
            self.notes.insert(module.to_string(), note);
        }
    }

    /// The default for new marks, where there is one.
    pub fn standard(&self) -> Option<&BTreeSet<u32>> {
        self.standard.as_ref()
    }

    /// Sets the default for new marks (of the collections that exist); `None` or none switches it
    /// off.
    pub fn set_standard(&mut self, ids: Option<&BTreeSet<u32>>) {
        let ids: Option<BTreeSet<u32>> = ids.map(|ids| ids.iter().copied().filter(|id| self.collection(*id).is_some()).collect());
        self.standard = ids.filter(|ids| !ids.is_empty());
    }

    /// A module was marked: it goes into the default's collections, if there is one. What it went
    /// into.
    pub fn marked(&mut self, module: &str) -> BTreeSet<u32> {
        let Some(default) = self.standard.clone() else { return BTreeSet::new() };
        let mut ids: BTreeSet<u32> = self.members.get(module).cloned().unwrap_or_default();
        ids.extend(default.iter().copied());
        self.set_in(module, &ids);
        default
    }

    /// Its mark was taken away: the module leaves its collections and its note goes. What it had,
    /// for `bring_back`.
    pub fn forget(&mut self, module: &str) -> Belongings {
        Belongings { collections: self.members.remove(module).unwrap_or_default(), note: self.notes.remove(module) }
    }

    /// „Rückgängig" of `forget`: what the module had, where its collections are still there.
    pub fn bring_back(&mut self, module: &str, belongings: &Belongings) {
        let mut ids: BTreeSet<u32> = self.members.get(module).cloned().unwrap_or_default();
        ids.extend(belongings.collections.iter().copied());
        self.set_in(module, &ids);
        if let Some(note) = &belongings.note {
            if self.note(module).is_none() {
                self.set_note(module, note);
            }
        }
    }

    /// Keeps the collections and notes of the modules that are marked alone (konzept.html:
    /// „Sammlungen und Notizen gelten nur für gemerkte Module"): what an older tab unmarked, which
    /// knows nothing of them, goes with the next change.
    pub fn keep_marked(&mut self, marked: impl Fn(&str) -> bool) {
        self.members.retain(|module, _| marked(module));
        self.notes.retain(|module, _| marked(module));
    }

    /// Whether a module has anything here: a collection or a note.
    pub fn keeps(&self, module: &str) -> bool {
        self.members.contains_key(module) || self.notes.contains_key(module)
    }

    /// Everything of these modules goes (the Merkliste emptied); the collections stay. What went,
    /// by module, for `bring_back`.
    pub fn forget_all(&mut self) -> Vec<(String, Belongings)> {
        let modules: BTreeSet<String> = self.members.keys().chain(self.notes.keys()).cloned().collect();
        modules.into_iter().map(|module| {
            let belongings = self.forget(&module);
            (module, belongings)
        }).collect()
    }

    /// The text `restored` reads back as this shelf. The note's backslashes, line breaks and tabs
    /// are written `\\`, `\n` and `\t`, so that a note never breaks its line.
    pub fn stored(&self) -> String {
        let mut out = String::new();
        let list = |ids: &mut dyn Iterator<Item = u32>| ids.map(|id| id.to_string()).collect::<Vec<_>>().join(",");
        for collection in &self.collections {
            out.push_str(&format!("c\t{}\t{}\n", collection.id, collection.name));
        }
        if let Some(default) = &self.standard {
            out.push_str(&format!("d\t{}\n", list(&mut default.iter().copied())));
        }
        if !self.recent.is_empty() {
            out.push_str(&format!("r\t{}\n", list(&mut self.recent.iter().copied())));
        }
        for (module, ids) in &self.members {
            out.push_str(&format!("m\t{module}\t{}\n", list(&mut ids.iter().copied())));
        }
        for (module, note) in &self.notes {
            let escaped = note.replace('\\', "\\\\").replace('\n', "\\n").replace('\t', "\\t");
            out.push_str(&format!("n\t{module}\t{escaped}\n"));
        }
        out
    }

    /// The shelf a stored text holds, read like anything from outside. Never fails: what does not
    /// check out is left out, line by line; the collections come first, so a later line only names
    /// those that are there.
    pub fn restored(text: &str) -> Self {
        let text = text.get(..MAX_STORED).unwrap_or(text);
        let mut shelf = Shelf::default();
        let numbers = |field: &str| -> BTreeSet<u32> { field.split(',').filter_map(|n| n.trim().parse::<u32>().ok()).collect() };
        let lines: Vec<Vec<&str>> = text.split('\n').map(|line| line.trim_end_matches('\r').splitn(3, '\t').collect()).collect();
        for fields in &lines {
            if let ["c", id, name] = fields.as_slice() {
                let (Ok(id), name) = (id.trim().parse::<u32>(), clean_name(name)) else { continue };
                if id > 0 && !name.is_empty() && shelf.collection(id).is_none() && shelf.find(&name).is_none() && shelf.collections.len() < MAX_COLLECTIONS {
                    shelf.collections.push(Collection { id, name });
                }
            }
        }
        for fields in &lines {
            match fields.as_slice() {
                ["d", ids] if shelf.standard.is_none() => {
                    let ids = numbers(ids);
                    shelf.set_standard(Some(&ids));
                }
                ["r", ids] if shelf.recent.is_empty() => {
                    let mut seen = BTreeSet::new();
                    shelf.recent = ids.split(',').filter_map(|n| n.trim().parse::<u32>().ok()).filter(|id| shelf.collection(*id).is_some() && seen.insert(*id)).collect();
                }
                ["m", module, ids] if is_module_id(module.trim()) && !shelf.members.contains_key(module.trim()) => {
                    let ids: BTreeSet<u32> = numbers(ids).into_iter().filter(|id| shelf.collection(*id).is_some()).collect();
                    if !ids.is_empty() && shelf.members.len() < MAX_BOOKMARKS {
                        shelf.members.insert(module.trim().to_string(), ids);
                    }
                }
                ["n", module, note] if is_module_id(module.trim()) && !shelf.notes.contains_key(module.trim()) => {
                    let mut text = String::new();
                    let mut chars = note.chars();
                    while let Some(c) = chars.next() {
                        match (c, c == '\\') {
                            (_, true) => match chars.next() {
                                Some('n') => text.push('\n'),
                                Some('t') => text.push('\t'),
                                Some(other) => text.push(other),
                                None => {}
                            },
                            (c, false) => text.push(c),
                        }
                    }
                    if shelf.notes.len() < MAX_BOOKMARKS {
                        shelf.set_note(module.trim(), &text);
                    }
                }
                _ => {}
            }
        }
        shelf
    }
}

/// The visitor's collections and notes (the active profile's), shared through context. Empty on
/// the server, always.
#[derive(Clone, Copy)]
pub struct Collections(RwSignal<Shelf>, Kept);

impl Collections {
    /// Reads what this browser has stored for the active profile and provides it. Another tab of
    /// the same browser may change it, or another profile becomes the active one; this one follows.
    pub fn provide() -> Self {
        let kept = Kept::of(STORAGE_KEY);
        let collections = Collections(RwSignal::new(load(kept)), kept);
        provide_context(collections);
        kept.follow(move || {
            let stored = load(kept);
            if collections.0.with_untracked(|shelf| *shelf != stored) {
                collections.0.set(stored);
            }
        });
        collections
    }

    pub fn expect() -> Option<Self> {
        use_context::<Collections>()
    }

    /// Reads the shelf. Tracked: a memo per part that shows it, as with the marks (R5).
    pub fn with<R>(self, f: impl FnOnce(&Shelf) -> R) -> R {
        self.0.with(f)
    }

    pub fn with_untracked<R>(self, f: impl FnOnce(&Shelf) -> R) -> R {
        self.0.with_untracked(f)
    }

    /// Changes the shelf and keeps it. What depends on it hears of it only when something changed;
    /// an empty shelf takes the key out of the storage (`nav::local_set`). What belongs to a module
    /// that is no longer marked goes with it (`Shelf::keep_marked`): mark a module before putting
    /// it into a collection.
    pub fn update<R>(self, f: impl FnOnce(&mut Shelf) -> R) -> R {
        let mut shelf = self.0.try_with_untracked(Clone::clone).unwrap_or_default();
        let result = f(&mut shelf);
        if let Some(bookmarks) = crate::bookmarks::Bookmarks::expect() {
            shelf.keep_marked(|module| bookmarks.is_marked_untracked(module));
        }
        if self.0.try_with_untracked(|now| *now != shelf).unwrap_or(false) {
            self.1.write(&shelf.stored());
            self.0.set(shelf);
        }
        result
    }
}

/// A box on the screen, in CSS pixels: what opened the menu of a module's collections and note.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Anchor {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

/// The menu of a module's collections and note asked for (oberflaeche.html, „Ein Menü für
/// Sammlung und Notiz"): one menu wherever it opens — the note after marking, the switch of a
/// module, the keys S and N at a row or at the open module; and for the rows selected on the
/// Merkliste (`more`), with their collections alone. `folia_widgets::collect` shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct CollectAsk {
    pub id: String,
    /// The module's title: the head of the menu; for several „3 Module".
    pub title: String,
    /// The other modules it is for besides `id` (the rows selected on the Merkliste): a check where
    /// all of them are in a collection, a dash where some are; no note, no default.
    pub more: Vec<String>,
    /// What opened it, where a wide screen puts the menu; `None`: in the middle, near the top.
    pub at: Option<Anchor>,
    /// The note has the focus, not the field (N, a click on the note at the module).
    pub note: bool,
    /// Opened from the note after marking (`turn`: that note's mark): on a phone „Fertig" ends the
    /// note as well, and the note waits while the menu is open.
    pub from_note: Option<u64>,
}

impl CollectAsk {
    /// The modules it is for: `id` and the others.
    pub fn modules(&self) -> Vec<String> {
        std::iter::once(self.id.clone()).chain(self.more.iter().filter(|other| **other != self.id).cloned()).collect()
    }

    /// Whether it is for several modules.
    pub fn many(&self) -> bool {
        self.more.iter().any(|other| *other != self.id)
    }
}

/// The menu open, shared through context: one at a time, over the page.
#[derive(Clone, Copy)]
pub struct Collecting {
    open: RwSignal<Option<CollectAsk>>,
    /// The notes after marking that „Fertig" ended (their marks' serials), the newest last.
    ended: RwSignal<Option<u64>>,
}

impl Collecting {
    pub fn provide() -> Self {
        let collecting = Collecting { open: RwSignal::new(None), ended: RwSignal::new(None) };
        provide_context(collecting);
        collecting
    }

    pub fn expect() -> Option<Self> {
        use_context::<Collecting>()
    }

    /// Opens the menu (another one open goes).
    pub fn open(self, ask: CollectAsk) {
        self.open.set(Some(ask));
    }

    pub fn close(self) {
        if self.open.with_untracked(Option::is_some) {
            self.open.set(None);
        }
    }

    /// The menu asked for. Tracked.
    pub fn asked(self) -> Option<CollectAsk> {
        self.open.get()
    }

    pub fn asked_untracked(self) -> Option<CollectAsk> {
        self.open.get_untracked()
    }

    /// Whether the menu is open from the note of the mark `serial`. Tracked.
    pub fn from_note(self, serial: u64) -> bool {
        self.open.with(|open| open.as_ref().is_some_and(|ask| ask.from_note == Some(serial)))
    }

    /// „Fertig" on a phone: the note the menu came from is done too.
    pub fn end_note(self, serial: u64) {
        self.ended.set(Some(serial));
    }

    /// The note „Fertig" ended last. Tracked.
    pub fn ended(self) -> Option<u64> {
        self.ended.get()
    }
}

/// What this browser has stored. Nothing on the server, and nothing in a browser that refuses
/// storage (the collections then last as long as the page).
fn load(kept: Kept) -> Shelf {
    kept.read().map(|stored| Shelf::restored(&stored)).unwrap_or_default()
}

/// A stored text as the store reads it and writes it back (a profile brought from a file,
/// `profiles::Backup`).
pub(crate) fn checked(stored: &str) -> String {
    Shelf::restored(stored).stored()
}

/// How many collections and notes a stored text keeps.
pub(crate) fn count(stored: &str) -> (usize, usize) {
    let shelf = Shelf::restored(stored);
    (shelf.collections.len(), shelf.notes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[u32]) -> BTreeSet<u32> {
        list.iter().copied().collect()
    }

    #[test]
    fn every_name_is_there_once() {
        let mut shelf = Shelf::default();
        let lena = shelf.create("  Für   Lena ").unwrap();
        assert_eq!(shelf.collection(lena).map(|c| c.name.as_str()), Some("Für Lena"));
        // Case and the spaces at its ends do not count: the same name is that collection.
        assert_eq!(shelf.create("für lena"), Err(Refused::Taken(lena)));
        assert_eq!(shelf.find("FÜR LENA "), Some(lena));
        assert_eq!(shelf.create(" \t "), Err(Refused::Empty));
        let witzig = shelf.create("Witzig").unwrap();
        assert_ne!(witzig, lena);
        // Renaming: the same name in other letters is fine, another collection's is not.
        assert_eq!(shelf.rename(lena, "für Lena"), Ok(()));
        assert_eq!(shelf.rename(witzig, "Für lena"), Err(Refused::Taken(lena)));
        assert_eq!(shelf.rename(witzig, ""), Err(Refused::Empty));
        // A name is cut at its cap; control characters are spaces.
        let long = shelf.create(&format!("a\u{7}b {}", "x".repeat(100))).unwrap();
        assert_eq!(shelf.collection(long).unwrap().name.chars().count(), MAX_NAME);
        assert!(shelf.collection(long).unwrap().name.starts_with("a b x"));
        // At most MAX_COLLECTIONS.
        for n in shelf.collections().len()..MAX_COLLECTIONS {
            shelf.create(&format!("Nr. {n}")).unwrap();
        }
        assert_eq!(shelf.create("Noch eine"), Err(Refused::Full));
    }

    #[test]
    fn modules_in_collections_and_their_notes() {
        let mut shelf = Shelf::default();
        let (lena, witzig) = (shelf.create("Für Lena").unwrap(), shelf.create("Witzig").unwrap());
        assert!(shelf.toggle_in("14169", witzig));
        assert!(shelf.toggle_in("14169", lena));
        assert_eq!(shelf.of("14169"), [lena, witzig], "in the collections' order");
        assert!(!shelf.toggle_in("14169", lena));
        assert_eq!(shelf.of("14169"), [witzig]);
        // A collection that is not there, or an id that is none, changes nothing.
        shelf.set_in("14169", &ids(&[witzig, 99]));
        assert_eq!(shelf.of("14169"), [witzig]);
        shelf.set_in("1 OR 1=1", &ids(&[witzig]));
        assert!(shelf.of("1 OR 1=1").is_empty());
        // The one chosen last comes first.
        assert_eq!(shelf.by_use(), [lena, witzig]);
        shelf.toggle_in("12177", witzig);
        assert_eq!(shelf.by_use(), [witzig, lena]);
        assert_eq!(shelf.count_in(witzig, |module| module == "12177"), 1);
        assert_eq!(shelf.count_in(witzig, |_| false), 0, "only what is marked counts");

        shelf.set_note("14169", "  **Lena** fragen\r\n- Plätze begrenzt\r\n");
        assert_eq!(shelf.note("14169"), Some("  **Lena** fragen\n- Plätze begrenzt\n"));
        shelf.set_note("14169", " \n ");
        assert_eq!(shelf.note("14169"), None, "a note of nothing but white space goes");
        shelf.set_note("12177", &"é".repeat(MAX_NOTE + 10));
        assert_eq!(shelf.note("12177").map(|note| note.chars().count()), Some(MAX_NOTE));

        // Its mark taken away: the module leaves its collections, its note goes; „Rückgängig"
        // brings both back.
        shelf.set_note("14169", "Lena fragen");
        let gone = shelf.forget("14169");
        assert_eq!((shelf.of("14169"), shelf.note("14169")), (vec![], None));
        shelf.bring_back("14169", &gone);
        assert_eq!((shelf.of("14169"), shelf.note("14169")), (vec![witzig], Some("Lena fragen")));
        assert!(shelf.keeps("14169") && shelf.keeps("12177") && !shelf.keeps("11103"));

        // What an older tab unmarked goes with the next change: only marked modules keep anything.
        shelf.keep_marked(|module| module == "12177");
        assert_eq!((shelf.of("14169"), shelf.note("14169"), shelf.of("12177")), (vec![], None, vec![witzig]));
    }

    #[test]
    fn what_a_search_reads_and_the_names_of_a_module() {
        let mut shelf = Shelf::default();
        let (lena, witzig) = (shelf.create("Für Lena").unwrap(), shelf.create("Witzig").unwrap());
        shelf.toggle_in("14169", witzig);
        shelf.toggle_in("14169", lena);
        shelf.set_note("14169", "**Lena** fragen");
        // The note as plain text, then the collections; in the collections' order, not the clicks'.
        assert_eq!(shelf.kept_text("14169").as_deref(), Some("Lena fragen Für Lena Witzig"));
        assert_eq!(shelf.names_of("14169"), "Für Lena, Witzig");
        shelf.set_note("12177", "nur eine Notiz");
        assert_eq!((shelf.kept_text("12177").as_deref(), shelf.names_of("12177")), (Some("nur eine Notiz "), String::new()));
        assert_eq!((shelf.kept_text("11103"), shelf.names_of("11103")), (None, String::new()));
    }

    #[test]
    fn the_default_for_new_marks() {
        let mut shelf = Shelf::default();
        let (lena, witzig) = (shelf.create("Für Lena").unwrap(), shelf.create("Witzig").unwrap());
        assert!(shelf.marked("11103").is_empty(), "no default: nothing");
        shelf.set_standard(Some(&ids(&[lena])));
        assert_eq!(shelf.marked("11103"), ids(&[lena]));
        assert_eq!(shelf.of("11103"), [lena]);
        shelf.toggle_in("12104", witzig);
        assert_eq!(shelf.marked("12104"), ids(&[lena]));
        assert_eq!(shelf.of("12104"), [lena, witzig], "what it was in stays");
        // Its collection deleted, the default goes with it; „Rückgängig" brings both back.
        let deleted = shelf.delete(lena).unwrap();
        assert_eq!((shelf.standard(), deleted.modules.len(), deleted.was_default), (None, 2, true));
        assert_eq!(shelf.of("11103"), Vec::<u32>::new());
        assert!(shelf.undelete(deleted.clone()));
        assert_eq!((shelf.standard(), shelf.of("11103"), shelf.collections()[0].id), (Some(&ids(&[lena])), vec![lena], lena));
        // Not twice, and not over a collection of the same name made meanwhile.
        assert!(!shelf.undelete(deleted.clone()));
        let again = shelf.delete(lena).unwrap();
        shelf.create("für lena").unwrap();
        assert!(!shelf.undelete(again));
        shelf.set_standard(Some(&ids(&[])));
        assert_eq!(shelf.standard(), None);
    }

    #[test]
    fn moving_and_numbering() {
        let mut shelf = Shelf::default();
        let (a, b, c) = (shelf.create("A").unwrap(), shelf.create("B").unwrap(), shelf.create("C").unwrap());
        shelf.shift(c, true);
        assert_eq!(shelf.collections().iter().map(|c| c.id).collect::<Vec<_>>(), [a, c, b]);
        shelf.shift(a, true);
        shelf.shift(b, false);
        assert_eq!(shelf.collections().iter().map(|c| c.id).collect::<Vec<_>>(), [a, c, b], "at the ends nothing moves");
        // A new collection takes a number none had.
        shelf.delete(b);
        assert!(shelf.create("D").unwrap() > b);
    }

    #[test]
    fn what_is_stored_is_read_like_anything_from_outside() {
        let mut shelf = Shelf::default();
        let (lena, witzig) = (shelf.create("Für Lena").unwrap(), shelf.create("Witzig").unwrap());
        shelf.toggle_in("14169", witzig);
        shelf.toggle_in("14169", lena);
        shelf.set_note("14169", "Lena fragen\n\tPlätze: \\begrenzt\\");
        shelf.set_standard(Some(&ids(&[lena])));
        let text = shelf.stored();
        assert_eq!(text, "c\t1\tFür Lena\nc\t2\tWitzig\nd\t1\nr\t1,2\nm\t14169\t1,2\nn\t14169\tLena fragen\\n\\tPlätze: \\\\begrenzt\\\\\n");
        assert_eq!(Shelf::restored(&text), shelf);
        assert_eq!(Shelf::restored(""), Shelf::default());
        // Lines that do not check out are dropped alone: a name twice, a collection that is not
        // there, an id that is none, a number that is none, the first of two notes counts.
        let odd = "c\t1\tFür Lena\nc\t2\tfür lena\nc\tx\tWitzig\nc\t0\tNull\nc\t3\t  \nm\t14169\t1,9\nm\t../etc\t1\nm\t12177\t9\nn\t14169\terste\nn\t14169\tzweite\nd\t9\nq\twas\n";
        let read = Shelf::restored(odd);
        assert_eq!(read.collections(), [Collection { id: 1, name: "Für Lena".to_string() }]);
        assert_eq!((read.of("14169"), read.of("12177"), read.note("14169"), read.standard()), (vec![1], vec![], Some("erste"), None));
        assert_eq!(checked(odd), read.stored());
        assert_eq!(count(&text), (2, 1));
    }
}
