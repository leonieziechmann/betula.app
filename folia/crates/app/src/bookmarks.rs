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
//! The key is versioned. What is stored is a line per module, the newest first:
//! `<module id>\t<seconds since 1970 when it was marked>`. It is read like anything that comes
//! from outside: ids that could not be ids are dropped (they end up in links and queries), a
//! module counts once, and the list has an upper limit.

use folia_model::ids::is_module_id;
use folia_pages::MAX_BOOKMARKS;
use leptos::prelude::*;

use crate::i18n;
use folia_design::nav;
use folia_design::ui::{Icon, Shortcut};

const STORAGE_KEY: &str = "betula.bookmarks.v1";

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
    fn toggle(&mut self, id: &str, now: u64) -> bool {
        if self.contains(id) {
            self.0.retain(|mark| mark.id != id);
            return false;
        }
        if !is_module_id(id) {
            return false;
        }
        self.0.insert(0, Mark { id: id.to_string(), at: now });
        self.0.truncate(MAX_BOOKMARKS);
        true
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

/// The visitor's marked modules, shared through context. Empty on the server, always.
#[derive(Clone, Copy)]
pub struct Bookmarks(RwSignal<Marks>);

impl Bookmarks {
    /// Reads what this browser has stored and provides the list. Another tab of the same browser
    /// may change it; this one follows.
    pub fn provide() -> Self {
        let bookmarks = Bookmarks(RwSignal::new(load()));
        provide_context(bookmarks);
        // Effects run in the browser only.
        Effect::new(move |_| {
            let handle = window_event_listener_untyped("storage", move |_| {
                let stored = load();
                if bookmarks.0.with_untracked(|marks| *marks != stored) {
                    bookmarks.0.set(stored);
                }
            });
            on_cleanup(move || handle.remove());
        });
        bookmarks
    }

    pub fn expect() -> Option<Self> {
        use_context::<Bookmarks>()
    }

    /// Whether the module is marked. Tracked: read it in a memo per button, not in a list.
    pub fn is_marked(self, id: &str) -> bool {
        self.0.with(|marks| marks.contains(id))
    }

    /// The same without subscribing: for a handler that asks once (a swiped row, `crate::swipe`).
    pub fn is_marked_untracked(self, id: &str) -> bool {
        self.0.with_untracked(|marks| marks.contains(id))
    }

    pub fn toggle(self, id: &str) -> bool {
        let mut marked = false;
        self.0.update(|marks| marked = marks.toggle(id, now()));
        self.save();
        marked
    }

    /// Takes every mark away and hands back what was marked, for `restore`.
    pub fn clear(self) -> Vec<Mark> {
        let cleared = self.marks_untracked();
        self.0.set(Marks::default());
        self.save();
        cleared
    }

    /// Puts marks back (the „Rückgängig" of `clear`); what has been marked since stays.
    pub fn restore(self, marks: Vec<Mark>) {
        self.0.update(|current| current.merge(marks));
        self.save();
    }

    /// Marks all of these now, in the order given (a list brought over from another device).
    /// What is marked already keeps its place.
    pub fn add_all(self, ids: &[String]) {
        let now = now();
        self.0.update(|current| {
            let new: Vec<Mark> = ids.iter().filter(|id| is_module_id(id) && !current.contains(id)).map(|id| Mark { id: id.clone(), at: now }).collect();
            current.0.splice(0..0, new);
            current.0.truncate(MAX_BOOKMARKS);
        });
        self.save();
    }

    pub fn count(self) -> usize {
        self.0.with(|marks| marks.0.len())
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
        nav::local_set(STORAGE_KEY, &self.0.with_untracked(Marks::stored));
    }
}

/// What this browser has stored. Nothing on the server, and nothing in a browser that refuses
/// storage (marks then last as long as the page).
fn load() -> Marks {
    nav::local_get(STORAGE_KEY).map(|stored| Marks::restored(&stored)).unwrap_or_default()
}

fn now() -> u64 {
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
/// number; an id that is none travels as it is.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Transfer {
    /// The ids that are numbers.
    #[serde(with = "folia_pack::set")]
    numbers: Vec<u64>,
    /// Every other id (the catalog has none). Empty, this costs nothing: it ends the code.
    others: Vec<String>,
}

impl Transfer {
    fn of(ids: &[String]) -> Self {
        let mut transfer = Transfer::default();
        for id in ids {
            match id.parse::<u64>().ok().filter(|number| number.to_string() == *id) {
                Some(number) => transfer.numbers.push(number),
                None => transfer.others.push(id.clone()),
            }
        }
        transfer
    }

    /// The ids: the numbers in ascending order, then the others.
    fn ids(self) -> Vec<String> {
        self.numbers.into_iter().map(|number| number.to_string()).chain(self.others).collect()
    }
}

/// The fragment (without `#`) that carries these modules; `None` for a list longer than a code
/// may be (thousands of ids that are no module numbers, which the catalog does not have).
pub fn transfer_fragment(ids: &[String]) -> Option<String> {
    folia_pack::to_versioned_code(KIND, VERSION, &Transfer::of(ids)).ok().map(|code| format!("{TRANSFER}{code}"))
}

/// A fragment that carries a list, but not one that can be read: cut short, or a character of it
/// typed wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BrokenLink;

/// The modules a fragment carries, checked like anything from outside. None for any other fragment.
pub fn ids_from_fragment(fragment: &str) -> Result<Vec<String>, BrokenLink> {
    let Some(code) = fragment.trim_start_matches('#').strip_prefix(TRANSFER) else {
        return Ok(Vec::new());
    };
    let listed = folia_pack::from_versioned_code::<Transfer>(KIND, VERSION, code.trim()).map_err(|_| BrokenLink)?.ids();
    let mut ids: Vec<String> = Vec::new();
    for id in listed.into_iter().filter(|id| is_module_id(id)) {
        if !ids.contains(&id) {
            ids.push(id);
        }
        if ids.len() == MAX_BOOKMARKS {
            break;
        }
    }
    Ok(ids)
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
/// It is part of server HTML (unpressed, like for everybody) only where leaving it out would make
/// the page jump when the app takes over; the stylesheet shows it once the app runs.
#[component]
pub fn MarkButton(#[prop(into)] id: String, #[prop(into)] title: String, look: MarkLook) -> impl IntoView {
    let t = i18n::t();
    let bookmarks = Bookmarks::expect();
    let marked = {
        let id = id.clone();
        Memo::new(move |_| bookmarks.is_some_and(|bookmarks| bookmarks.is_marked(&id)))
    };
    let toggle = move |_: leptos::ev::MouseEvent| {
        if let Some(bookmarks) = bookmarks {
            bookmarks.toggle(&id);
        }
    };
    let pressed = move || if marked.get() { "true" } else { "false" };
    let hint = move || if marked.get() { t.marks.saved_hint } else { t.marks.save_hint };
    let label = move || if marked.get() { t.marks.saved } else { t.marks.save };
    match look {
        // In a list the row is the stop of the Tab key, as before there were marks; the keyboard
        // marks with M on the row (the head of the list says so).
        MarkLook::Row => view! {
            <button class="mark-toggle icon-btn" type="button" tabindex="-1" data-action="mark" on:click=toggle aria-pressed=pressed aria-label=(t.marks.save_title)(&title) title=hint>
                <Icon name="bookmark"/>
            </button>
        }
        .into_any(),
        MarkLook::Hero => view! {
            <button class="mark-toggle mark-switch hit" type="button" data-action="mark" on:click=toggle aria-pressed=pressed title=hint>
                <Icon name="bookmark"/><span>{label}</span><Shortcut keys="M"/>
            </button>
        }
        .into_any(),
        MarkLook::Action => view! {
            <button class="mark-toggle action" type="button" data-action="mark" on:click=toggle aria-pressed=pressed title=hint>
                <Icon name="bookmark"/><span>{label}</span>
            </button>
        }
        .into_any(),
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
        let later = format!("m={}", folia_pack::to_versioned_code(KIND, VERSION + 1, &Transfer::of(&owned(&["12204", "11101"]))).unwrap());
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
