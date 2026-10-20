//! The menu of a module's collections and note (folia/design/merken/oberflaeche.html, „Ein Menü für
//! Sammlung und Notiz"): one menu wherever it opens — the field of the note after marking
//! (`marknote`), the arrow of a module's switch, S and N at a row or at the open module
//! (`enhance.js`, `betula:collect`). The store says which module it is for
//! (`folia_stores::collections::Collecting`); this host shows it.
//!
//! - At its top the field, which filters the collections as it is typed into and makes one where
//!   none has the name (owner, 2026-10-19: „mach einfach gleich den Fokus in das Suchfeld"; no
//!   digits beside the rows: „Da ist einfach eine Zahl hinter einer anderen zahl"). ↑ ↓ go through
//!   the rows, Enter switches the one the keys are on or makes the collection. Names are there once:
//!   a name typed that a collection has is that collection, never a second „anlegen".
//! - The collections with a check where the module is in one, and how many marked modules each
//!   holds; in their own order, which does not change under the pointer.
//! - „Standard für neue Merker" once the module is in a collection: what is marked from now on goes
//!   into the same collections, until it is switched off (oberflaeche.html, question B). Switched
//!   on here, it follows what is chosen here. A module in none says which default there is, with
//!   „Aufheben".
//! - The note, in Markdown, a field that grows (`folia_model::text`, „ohne Editor mit Knöpfen").
//!
//! On a wide screen the menu stands at what opened it, as the menus of „Mein Studium" do, and each
//! choice holds at once; Esc or a click beside it closes it, the focus goes back. On a phone it is a
//! sheet from below and a draft (owner, the same day: „Wenn man das fenster so zu macht, dann werden
//! die changes nicht gespeichert"): „Fertig" at its foot, as wide as the sheet, keeps what was chosen
//! and written and ends the note it came from („dann ist der Prozess done und die Notification kann
//! auch weg"); swiped down, tapped beside it or Back lets all of it go, a collection made in it
//! too. The field there gets the focus only when tapped, so that the keyboard does not cover the
//! collections at once; with the keyboard up the sheet takes what it leaves (`folia_design::
//! keyboard`).
//!
//! What goes into a collection, or gets a note, is marked (konzept.html: „In eine Sammlung tun
//! merkt ein Modul"), without the default for new marks: what is chosen here is what it gets.
//!
//! For the rows selected on the Merkliste (`CollectAsk::more`) the same menu holds their
//! collections alone (oberflaeche.html, „Drei ausgewählte"): a check where all of them are in, a
//! dash where some are, and a tap on the dash puts all of them in; no note, no default.

use std::collections::BTreeSet;

use leptos::prelude::*;

use folia_design::nav::{room, Room};
use folia_design::ui::Icon;
use folia_stores::bookmarks::Bookmarks;
use folia_stores::collections::{Anchor, CollectAsk, Collecting, Collections, Refused, Shelf};

use crate::i18n;
use crate::list::phone_layout;

/// How wide the menu is on a wide screen, and how far it stays from the window's edges.
const WIDTH: f64 = 300.0;
const EDGE: f64 = 8.0;
/// Below what opened it while there is this much room there (px), else where there is more.
const ROOM_BELOW: f64 = 340.0;

/// Where the menu stands on a wide screen: its left edge, and its top (under what opened it) or its
/// bottom (over it, from the window's bottom), and how tall it may be.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Placed {
    left: f64,
    top: Option<f64>,
    bottom: Option<f64>,
    most: f64,
}

/// Under what opened it, its right edge at that one's, while there is room below; over it where
/// there is more room above (the note after marking, at the window's bottom). Never past the
/// window's edges, nor under the rail. Without an anchor in the middle, near the top.
fn place(at: Option<Anchor>, room: Room) -> Placed {
    let Room { left: rail, width, height } = room;
    let least = rail + EDGE;
    let Some(at) = at else {
        return Placed { left: ((width - WIDTH) / 2.0).max(least), top: Some((height * 0.12).round()), bottom: None, most: (height * 0.76).round() };
    };
    let left = (at.right - WIDTH).clamp(least, (width - WIDTH - EDGE).max(least));
    let below = height - at.bottom - 4.0 - EDGE;
    let above = at.top - 4.0 - EDGE;
    if below >= ROOM_BELOW || below >= above {
        Placed { left, top: Some(at.bottom + 4.0), bottom: None, most: below.max(160.0) }
    } else {
        Placed { left, top: None, bottom: Some(height - at.top + 4.0), most: above.max(160.0) }
    }
}

impl Placed {
    fn style(self) -> String {
        let vertical = match (self.top, self.bottom) {
            (Some(top), _) => format!("top:{top:.0}px;"),
            (None, Some(bottom)) => format!("bottom:{bottom:.0}px;"),
            (None, None) => String::new(),
        };
        format!("left:{:.0}px;{vertical}max-height:{:.0}px", self.left, self.most)
    }
}

/// What the menu changes: on a wide screen the shelf itself, each choice at once; on a phone a
/// draft of it, until „Fertig".
#[derive(Clone, Copy)]
enum Work {
    Live(Collections),
    Draft(RwSignal<Shelf>),
}

impl Work {
    /// Reads it. Tracked.
    fn with<R>(self, f: impl FnOnce(&Shelf) -> R) -> R {
        match self {
            Work::Live(collections) => collections.with(f),
            Work::Draft(draft) => draft.with(f),
        }
    }

    /// The same without subscribing: what the menu starts from, and what a click decides.
    fn with_untracked<R>(self, f: impl FnOnce(&Shelf) -> R) -> R {
        match self {
            Work::Live(collections) => collections.with_untracked(f),
            Work::Draft(draft) => draft.with_untracked(f),
        }
    }

    fn change(self, f: impl FnOnce(&mut Shelf)) {
        match self {
            Work::Live(collections) => collections.update(f),
            Work::Draft(draft) => draft.update(f),
        }
    }

    fn create(self, name: &str) -> Result<u32, Refused> {
        match self {
            Work::Live(collections) => collections.update(|shelf| shelf.create(name)),
            Work::Draft(draft) => draft.try_update(|shelf| shelf.create(name)).unwrap_or(Err(Refused::Full)),
        }
    }
}

/// The menu open, over the page: a wide screen's at what opened it, a phone's as a sheet. Once in
/// the app (`folia_app::App`), inside the router.
#[component]
pub fn CollectHost() -> impl IntoView {
    let Some(collecting) = Collecting::expect() else { return ().into_any() };
    let phone = phone_layout();
    // S and N at a row or at the open module (`enhance.js`): the module, its title, where it is.
    Effect::new(move |_| {
        let handle = window_event_listener_untyped("betula:collect", move |ev| {
            if let Some(ask) = ask_of(&ev) {
                collecting.open(ask);
            }
        });
        on_cleanup(move || handle.remove());
    });
    // A new menu is drawn anew; one asked again for the same module stays as it is.
    let asked = Memo::new(move |_| collecting.asked());
    (move || {
        let ask = asked.get()?;
        Some(if phone.get_untracked() { view! { <CollectSheet ask collecting/> }.into_any() } else { view! { <CollectPop ask collecting/> }.into_any() })
    })
    .into_any()
}

/// The menu asked for by `betula:collect` (`enhance.js`): `detail` has the module's `id` and
/// `title`, `note` for N, and the box of what it is for.
fn ask_of(ev: &leptos::web_sys::Event) -> Option<CollectAsk> {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let detail = ev.dyn_ref::<web_sys::CustomEvent>()?.detail();
        let get = |key: &str| web_sys::js_sys::Reflect::get(&detail, &wasm_bindgen::JsValue::from_str(key)).ok();
        let id = get("id")?.as_string()?;
        if !folia_model::ids::is_module_id(&id) {
            return None;
        }
        let title = get("title").and_then(|title| title.as_string()).unwrap_or_else(|| id.clone());
        let number = |key: &str| get(key).and_then(|value| value.as_f64());
        let at = match (number("left"), number("top"), number("right"), number("bottom")) {
            (Some(left), Some(top), Some(right), Some(bottom)) => Some(Anchor { left, top, right, bottom }),
            _ => None,
        };
        let note = get("note").is_some_and(|note| note.as_bool() == Some(true));
        Some(CollectAsk { id, title, more: Vec::new(), at, note, from_note: None })
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = ev;
        None
    }
}

/// What had the focus when the menu opened: it gets it back when the menu closes, if it is still
/// on the page.
#[derive(Clone, Copy)]
pub(crate) struct FocusBack(StoredValue<Option<send_wrapper::SendWrapper<leptos::web_sys::HtmlElement>>>);

impl FocusBack {
    pub(crate) fn take() -> Self {
        #[cfg(feature = "csr")]
        {
            use wasm_bindgen::JsCast;
            let active = web_sys::window().and_then(|window| window.document()).and_then(|document| document.active_element()).and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok());
            FocusBack(StoredValue::new(active.map(send_wrapper::SendWrapper::new)))
        }
        #[cfg(not(feature = "csr"))]
        FocusBack(StoredValue::new(None))
    }

    pub(crate) fn give(self) {
        #[cfg(feature = "csr")]
        if let Some(element) = self.0.try_update_value(Option::take).flatten() {
            if element.is_connected() {
                let options = web_sys::FocusOptions::new();
                options.set_prevent_scroll(true);
                let _ = element.focus_with_options(&options);
            }
        }
        #[cfg(not(feature = "csr"))]
        let _ = self.0;
    }
}

/// On a wide screen: at what opened it, over a shade that closes it; each choice holds at once.
#[component]
fn CollectPop(ask: CollectAsk, collecting: Collecting) -> impl IntoView {
    let t = i18n::t();
    let Some(collections) = Collections::expect() else { return ().into_any() };
    let back = FocusBack::take();
    let placed = place(ask.at, room());
    let close = Callback::new(move |()| {
        collecting.close();
        back.give();
    });
    folia_shell::nav::layer_while(move || collecting.asked().is_some(), move || collecting.close(), folia_shell::nav::Kind::Over);
    let keys = move |ev: leptos::ev::KeyboardEvent| {
        if ev.key() == "Escape" {
            ev.prevent_default();
            ev.stop_propagation();
            close.run(());
        }
    };
    let label = format!("{} · {}", if ask.many() { t.marks.collection } else { t.marks.menu_title }, ask.title);
    view! {
        <div class="collect-shade" on:mousedown=|ev: leptos::ev::MouseEvent| ev.prevent_default() on:click=move |_| close.run(()) on:contextmenu=move |ev: leptos::ev::MouseEvent| { ev.prevent_default(); close.run(()); }></div>
        <div class="collect-pop" role="dialog" aria-label=label style=placed.style() on:keydown=keys>
            <p class="collect-head">{ask.title.clone()}</p>
            <CollectBody ask work=Work::Live(collections) phone=false close/>
        </div>
    }
    .into_any()
}

/// On a phone: a sheet from below, a draft until „Fertig".
#[component]
fn CollectSheet(ask: CollectAsk, collecting: Collecting) -> impl IntoView {
    let t = i18n::t();
    let Some(collections) = Collections::expect() else { return ().into_any() };
    let bookmarks = Bookmarks::expect();
    let node = NodeRef::<leptos::html::Dialog>::new();
    // What the shelf was when the sheet opened: what the draft adds to it is what „Fertig" keeps.
    let before = collections.with_untracked(Clone::clone);
    let draft = RwSignal::new(before.clone());
    node.on_load(move |dialog| {
        if !dialog.open() {
            let _ = dialog.show_modal();
            // The sheet has the focus, not its field (a browser gives a dialog's first field the
            // focus as it opens), so that the screen's keyboard does not come at once.
            let _ = dialog.focus();
        }
    });
    folia_shell::nav::layer_while(move || collecting.asked().is_some(), move || collecting.close(), folia_shell::nav::Kind::Dialog);
    // The dialog closes as a dialog does, so that it leaves the top layer before it leaves the
    // page; its `close` lets the menu go.
    let shut = move || match node.get_untracked() {
        Some(dialog) if dialog.open() => dialog.close(),
        _ => collecting.close(),
    };
    on_cleanup(move || {
        if let Some(dialog) = node.try_get_untracked().flatten() {
            if dialog.open() {
                dialog.close();
            }
        }
    });
    let close = Callback::new(move |()| shut());
    let (modules, title, many) = (ask.modules(), ask.title.clone(), ask.many());
    let from_note = ask.from_note;
    let done = move |_| {
        let now = draft.get_untracked();
        // What a module gets makes it marked; where the Merkliste is full the note says so, and
        // it gets nothing.
        let marked: Vec<String> = modules.iter().filter(|id| !now.keeps(id) || bookmarks.is_some_and(|bookmarks| bookmarks.mark_quiet(id, &title))).cloned().collect();
        if !marked.is_empty() {
            collections.update(|shelf| adopt(shelf, &before, &now, &marked, !many));
        }
        if let Some(serial) = from_note {
            collecting.end_note(serial);
        }
        shut();
    };
    let on_backdrop = move |ev: leptos::ev::MouseEvent| {
        let on_backdrop = ev.target().and_then(|target| {
            node.get_untracked().map(|dialog| {
                let dialog: &leptos::web_sys::EventTarget = dialog.as_ref();
                dialog == &target
            })
        });
        if on_backdrop == Some(true) {
            shut();
        }
    };
    view! {
        // `autofocus` on the dialog itself: it takes the focus as it opens, not its field, so that the
        // screen's keyboard does not cover the collections at once (oberflaeche.html: „Das Feld
        // bekommt den Fokus erst beim Antippen").
        <dialog class="st-dialog collect-sheet" data-sheet="" autofocus=true tabindex="-1" node_ref=node aria-label=format!("{} · {}", if ask.many() { t.marks.collection } else { t.marks.menu_title }, ask.title) on:close=move |_| collecting.close() on:click=on_backdrop>
            <div class="st-dlg">
                <header class="st-dlg-head collect-sheet-head">
                    <b>{ask.title.clone()}</b>
                </header>
                <div class="st-dlg-body collect-sheet-body">
                    <CollectBody ask=ask.clone() work=Work::Draft(draft) phone=true close/>
                </div>
                <footer class="st-dlg-foot collect-sheet-foot">
                    <button class="btn primary" type="button" id="collect-done" on:click=done>{t.marks.done}</button>
                </footer>
            </div>
        </dialog>
    }
    .into_any()
}

/// What the phone's draft adds to the shelf for the modules `ids` (`before`: the shelf when the
/// sheet opened): the collections made in it (by name: one made meanwhile elsewhere of the same
/// name is that one), the modules' collections; for one module (`whole`) its note, and the default
/// where it changed.
fn adopt(shelf: &mut Shelf, before: &Shelf, draft: &Shelf, ids: &[String], whole: bool) {
    let mut mapped: Vec<(u32, u32)> = Vec::new();
    for collection in draft.collections() {
        let same = before.collection(collection.id).is_some_and(|was| was.name == collection.name) && shelf.collection(collection.id).is_some();
        let now = if same {
            Some(collection.id)
        } else {
            match shelf.create(&collection.name) {
                Ok(made) => Some(made),
                Err(Refused::Taken(there)) => Some(there),
                Err(Refused::Empty | Refused::Full) => None,
            }
        };
        if let Some(now) = now {
            mapped.push((collection.id, now));
        }
    }
    let map = |ids: &mut dyn Iterator<Item = u32>| -> BTreeSet<u32> { ids.filter_map(|draft_id| mapped.iter().find(|(from, _)| *from == draft_id).map(|(_, to)| *to)).collect() };
    for id in ids {
        let into = map(&mut draft.of(id).into_iter());
        shelf.set_in(id, &into);
        if whole {
            shelf.set_note(id, draft.note(id).unwrap_or_default());
        }
    }
    if whole && draft.standard() != before.standard() {
        let standard = draft.standard().map(|ids| map(&mut ids.iter().copied()));
        shelf.set_standard(standard.as_ref());
    }
}

/// How much of what the menu is for a collection holds: all of it (a check), some of the modules
/// (a dash), none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Held {
    All,
    Some,
    None,
}

impl Held {
    fn of(shelf: &Shelf, collection: u32, modules: &[String]) -> Self {
        let inside = modules.iter().filter(|module| shelf.holds(collection, module)).count();
        match inside {
            0 => Held::None,
            n if n == modules.len() => Held::All,
            _ => Held::Some,
        }
    }

    /// `aria-checked`.
    fn aria(self) -> &'static str {
        match self {
            Held::All => "true",
            Held::Some => "mixed",
            Held::None => "false",
        }
    }
}

/// A row of the menu the keys can be on: a collection, or „anlegen".
#[derive(Clone, Debug, PartialEq)]
enum Line {
    Collection { id: u32, name: String, count: usize, held: Held },
    Create(String),
}

/// What the menu shows below its field for what is typed: the collections whose names have it, and
/// „‚…‘ anlegen" where no collection has that name. Each says how many marked modules it holds
/// (the menu's own counted, marked or not) and how much of the menu's it holds.
fn lines_of(shelf: &Shelf, modules: &[String], typed: &str, marked: impl Fn(&str) -> bool) -> Vec<Line> {
    let typed = folia_stores::collections::clean_name(typed);
    let mut lines: Vec<Line> = shelf
        .collections()
        .iter()
        .filter(|collection| typed.is_empty() || folia_search::matches(&collection.name, &typed))
        .map(|collection| Line::Collection {
            id: collection.id,
            name: collection.name.clone(),
            count: shelf.count_in(collection.id, |other| modules.iter().any(|module| module == other) || marked(other)),
            held: Held::of(shelf, collection.id, modules),
        })
        .collect();
    if !typed.is_empty() && shelf.find(&typed).is_none() {
        lines.push(Line::Create(typed));
    }
    lines
}

/// Puts the modules into a collection (`into`), or takes them out of it.
fn put_in(shelf: &mut Shelf, modules: &[String], collection: u32, into: bool) {
    for module in modules {
        let mut ids: BTreeSet<u32> = shelf.of(module).into_iter().collect();
        if into {
            ids.insert(collection);
        } else {
            ids.remove(&collection);
        }
        shelf.set_in(module, &ids);
    }
}

/// The field, the collections, the default and the note: the same on a wide screen and in the
/// phone's sheet. For several modules the collections alone.
#[component]
fn CollectBody(ask: CollectAsk, work: Work, phone: bool, close: Callback<()>) -> impl IntoView {
    let t = i18n::t();
    let m = &t.marks;
    let bookmarks = Bookmarks::expect();
    // The modules it is for: one, or the rows selected (`many`: their collections alone).
    let modules = StoredValue::new(ask.modules());
    let many = ask.many();
    let CollectAsk { id, title, note: note_first, .. } = ask;
    let typed = RwSignal::new(String::new());
    // The row the keys are on: with something typed the first, else none until an arrow.
    let active = RwSignal::new(None::<usize>);
    let refused = RwSignal::new(false);
    let lines = Memo::new(move |_| {
        let typed = typed.get();
        modules.with_value(|modules| work.with(|shelf| lines_of(shelf, modules, &typed, |other| bookmarks.is_some_and(|bookmarks| bookmarks.is_marked(other)))))
    });
    Effect::new(move |_| {
        let typed = typed.get();
        active.set((!typed.trim().is_empty()).then_some(0));
        refused.set(false);
    });

    // On a wide screen what the menu does holds at once, and marks the modules first (whether one
    // of them is marked then; where the Merkliste is full the note says so); a phone's draft marks
    // them with „Fertig".
    let marked_first = {
        let title = title.clone();
        move || match work {
            // Each of them, not only up to the first marked.
            Work::Live(_) => modules.with_value(|modules| modules.iter().filter(|module| bookmarks.is_some_and(|bookmarks| bookmarks.mark_quiet(module, &title))).count() > 0),
            Work::Draft(_) => true,
        }
    };
    // „Standard für neue Merker" switched on in this menu follows what is chosen in it.
    let module_set = {
        let id = id.clone();
        move || work.with(|shelf| shelf.of(&id).into_iter().collect::<BTreeSet<u32>>())
    };
    let module_set_now = {
        let id = id.clone();
        move || work.with_untracked(|shelf| shelf.of(&id).into_iter().collect::<BTreeSet<u32>>())
    };
    let following = RwSignal::new(!many && {
        let now = module_set_now.clone();
        work.with_untracked(|shelf| shelf.standard().cloned()).is_some_and(|standard| !standard.is_empty() && standard == now())
    });
    let follow = {
        let module_set = module_set_now.clone();
        move || {
            if following.get_untracked() {
                let now = module_set();
                if now.is_empty() {
                    following.set(false);
                }
                work.change(|shelf| shelf.set_standard(Some(&now)));
            }
        }
    };
    // A collection switched: out of it where all of the modules are in it, else all of them into
    // it (a tap on the dash takes in the rest).
    let toggle = {
        let (marked_first, follow) = (marked_first.clone(), follow.clone());
        move |collection: u32| {
            if !marked_first() {
                return;
            }
            work.change(|shelf| modules.with_value(|modules| put_in(shelf, modules, collection, Held::of(shelf, collection, modules) != Held::All)));
            follow();
        }
    };
    let create = {
        let (marked_first, follow) = (marked_first.clone(), follow.clone());
        move |name: String| {
            if !marked_first() {
                return;
            }
            let made = match work.create(&name) {
                Ok(made) | Err(Refused::Taken(made)) => made,
                Err(Refused::Full) => {
                    refused.set(true);
                    return;
                }
                Err(Refused::Empty) => return,
            };
            work.change(|shelf| modules.with_value(|modules| put_in(shelf, modules, made, true)));
            follow();
            typed.set(String::new());
        }
    };
    let choose = {
        let (toggle, create) = (toggle.clone(), create.clone());
        move |line: Line| match line {
            Line::Collection { id, .. } => toggle(id),
            Line::Create(name) => create(name),
        }
    };

    let field = NodeRef::<leptos::html::Input>::new();
    let note_field = NodeRef::<leptos::html::Textarea>::new();
    // The focus: in the field on a wide screen (owner: „mach einfach gleich den Fokus in das
    // Suchfeld"), in the note for N; on a phone only for N, so the keyboard does not come at once.
    #[cfg(feature = "csr")]
    request_animation_frame(move || {
        let options = web_sys::FocusOptions::new();
        options.set_prevent_scroll(true);
        if note_first {
            if let Some(note) = note_field.get_untracked() {
                let _ = note.focus_with_options(&options);
                let end = u32::try_from(note.value().encode_utf16().count()).unwrap_or(u32::MAX);
                let _ = note.set_selection_range(end, end);
            }
        } else if !phone {
            if let Some(field) = field.get_untracked() {
                let _ = field.focus_with_options(&options);
            }
        }
    });
    #[cfg(not(feature = "csr"))]
    let _ = note_first;

    let field_keys = {
        let choose = choose.clone();
        move |ev: leptos::ev::KeyboardEvent| {
            let count = lines.with_untracked(Vec::len);
            match ev.key().as_str() {
                "ArrowDown" if count > 0 => {
                    ev.prevent_default();
                    active.set(Some(active.get_untracked().map_or(0, |at| (at + 1) % count)));
                }
                "ArrowUp" if count > 0 => {
                    ev.prevent_default();
                    active.set(Some(active.get_untracked().map_or(count - 1, |at| if at == 0 { count - 1 } else { at - 1 })));
                }
                "Enter" => {
                    ev.prevent_default();
                    let line = active.get_untracked().and_then(|at| lines.with_untracked(|lines| lines.get(at).cloned()));
                    if let Some(line) = line {
                        choose(line);
                        typed.set(String::new());
                    }
                    // A phone's keyboard „Fertig": the keyboard goes.
                    if phone {
                        if let Some(field) = field.get_untracked() {
                            let _ = field.blur();
                        }
                    }
                }
                _ => {}
            }
        }
    };

    // The default for new marks.
    let standard_row = {
        let module_set = module_set.clone();
        move || {
            let now = module_set();
            let standard = work.with(|shelf| shelf.standard().cloned());
            let names = move |ids: &BTreeSet<u32>| work.with(|shelf| shelf.collections().iter().filter(|collection| ids.contains(&collection.id)).map(|collection| collection.name.clone()).collect::<Vec<_>>().join(", "));
            if !now.is_empty() {
                let on = standard.as_ref() == Some(&now);
                let before = standard.filter(|standard| *standard != now).map(|standard| (m.standard_before)(&names(&standard)));
                let flip = move |_| {
                    if on {
                        following.set(false);
                        work.change(|shelf| shelf.set_standard(None));
                    } else {
                        following.set(true);
                        let now = now.clone();
                        work.change(|shelf| shelf.set_standard(Some(&now)));
                    }
                };
                Some(view! {
                    <div class="collect-sep"></div>
                    <button class="collect-default" type="button" role="switch" aria-checked=if on { "true" } else { "false" } on:click=flip>
                        <span class="collect-default-label"><b>{m.standard}</b>{before.map(|before| view! { <small>{before}</small> })}</span>
                        <span class="collect-switch" aria-hidden="true"></span>
                    </button>
                }.into_any())
            } else {
                standard.map(|standard| {
                    let names = names(&standard);
                    view! {
                        <div class="collect-sep"></div>
                        <div class="collect-default">
                            <span class="collect-default-label"><b>{m.standard}</b><small>{names}</small></span>
                            <button class="mini" type="button" on:click=move |_| work.change(|shelf| shelf.set_standard(None))>{m.standard_clear}</button>
                        </div>
                    }
                    .into_any()
                })
            }
        }
    };

    // The note: as it is when the menu opens, then as it is typed.
    let note_text = {
        let id = id.clone();
        work.with_untracked(|shelf| shelf.note(&id).map(str::to_string)).unwrap_or_default()
    };
    let write_note = {
        let (id, marked_first) = (id.clone(), marked_first.clone());
        move |ev: leptos::ev::Event| {
            let text = event_target_value(&ev);
            if !text.trim().is_empty() && !marked_first() {
                return;
            }
            work.change(|shelf| shelf.set_note(&id, &text));
        }
    };
    let note_keys = move |ev: leptos::ev::KeyboardEvent| {
        if ev.key() == "Escape" && !phone {
            ev.prevent_default();
            ev.stop_propagation();
            close.run(());
        }
    };

    let line_view = move |at: usize, line: Line| {
        let choose = choose.clone();
        let is_active = move || !phone && active.get() == Some(at);
        match line.clone() {
            Line::Collection { name, count, held, .. } => view! {
                <button class="collect-item" class:on=held == Held::All class:part=held == Held::Some class:act=is_active type="button" role="menuitemcheckbox" aria-checked=held.aria() on:click=move |_| choose(line.clone()) on:pointermove=move |_| if !phone { active.set(Some(at)) }>
                    <span class="collect-check">{match held {
                        Held::All => Some(view! { <Icon name="check"/> }.into_any()),
                        Held::Some => Some(view! { <Icon name="minus"/> }.into_any()),
                        Held::None => None,
                    }}</span>
                    <span class="collect-name">{name}</span>
                    <span class="collect-count num">{count}</span>
                </button>
            }
            .into_any(),
            Line::Create(name) => view! {
                <button class="collect-item create" class:act=is_active type="button" role="menuitem" on:click=move |_| choose(line.clone()) on:pointermove=move |_| if !phone { active.set(Some(at)) }>
                    <span class="collect-check"><Icon name="plus"/></span>
                    <span class="collect-name">{(m.create)(&name)}</span>
                    {move || is_active().then(|| view! { <kbd>"Enter"</kbd> })}
                </button>
            }
            .into_any(),
        }
    };

    view! {
        <div class="collect" class:phone=phone>
            <label class="collect-field">
                <Icon name="search"/>
                <input
                    node_ref=field
                    type="text"
                    placeholder=m.search_or_create
                    aria-label=m.search_or_create
                    autocomplete="off"
                    enterkeyhint="done"
                    maxlength="60"
                    prop:value=move || typed.get()
                    on:input=move |ev| typed.set(event_target_value(&ev))
                    on:keydown=field_keys
                />
            </label>
            <div class="collect-items" role="menu" aria-label=m.collect>
                {move || {
                    let lines = lines.get();
                    if lines.is_empty() {
                        return view! { <p class="collect-empty">{m.no_collection_yet}</p> }.into_any();
                    }
                    lines.into_iter().enumerate().map(|(at, line)| line_view(at, line)).collect_view().into_any()
                }}
                {move || refused.get().then(|| view! { <p class="collect-empty warn">{m.collections_full}</p> })}
            </div>
            {(!many).then(|| view! {
                {standard_row}
                <div class="collect-sep"></div>
                <p class="collect-label label">{m.note}</p>
                <textarea class="collect-note" node_ref=note_field rows="2" placeholder=m.note_placeholder aria-label=m.note_label maxlength="4000" prop:value=note_text on:input=write_note on:keydown=note_keys></textarea>
            })}
        </div>
    }
}

/// The note of a marked module, under its switches (oberflaeche.html, „Am Modul"): whole, its
/// Markdown drawn (`NoteProse`: links open in a new tab, a picture only as a link, HTML as text).
/// The pencil, or a click beside a link, edits it in the menu with the focus in the note, as N does.
/// Nothing without a note; nothing on the server, which knows no notes (R9).
#[component]
pub fn ModuleNote(#[prop(into)] id: String, #[prop(into)] title: String) -> impl IntoView {
    let t = i18n::t();
    let collections = Collections::expect();
    let note = {
        let id = id.clone();
        Memo::new(move |_| collections.and_then(|collections| collections.with(|shelf| shelf.note(&id).map(str::to_string))))
    };
    let edit = move |ev: leptos::ev::MouseEvent| {
        // A link in the note is the link's.
        if folia_design::nav::link_under(ev.target()).is_some() {
            return;
        }
        if let Some(collecting) = Collecting::expect() {
            collecting.open(CollectAsk { id: id.clone(), title: title.clone(), more: Vec::new(), at: box_of(&ev), note: true, from_note: None });
        }
    };
    move || {
        let edit = edit.clone();
        note.get().map(|text| {
            view! {
                <div class="module-note" on:click=edit>
                    <button class="module-note-edit" type="button" data-action="note" aria-label=t.marks.edit_note title=t.marks.edit_note><Icon name="pencil"/></button>
                    <folia_design::ui::NoteProse text/>
                </div>
            }
        })
    }
}

/// The box of the element an event was heard on.
fn box_of(ev: &leptos::ev::MouseEvent) -> Option<Anchor> {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let rect = ev.current_target()?.dyn_into::<web_sys::Element>().ok()?.get_bounding_client_rect();
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

    const VIEW: Room = Room::window(1440.0, 900.0);

    fn at(left: f64, top: f64, right: f64, bottom: f64) -> Option<Anchor> {
        Some(Anchor { left, top, right, bottom })
    }

    #[test]
    fn the_menu_stands_where_the_window_has_room() {
        // Under a switch, its right edge at the switch's.
        let under = place(at(900.0, 200.0, 1100.0, 232.0), VIEW);
        assert_eq!((under.left, under.top, under.bottom), (800.0, Some(236.0), None));
        // Over the note at the window's bottom.
        let over = place(at(700.0, 840.0, 900.0, 880.0), VIEW);
        assert_eq!((over.left, over.top, over.bottom, over.most), (600.0, None, Some(64.0), 828.0));
        // Never past the left edge, nor under the rail.
        assert_eq!(place(at(0.0, 100.0, 120.0, 130.0), VIEW).left, EDGE);
        assert_eq!(place(at(60.0, 100.0, 280.0, 130.0), Room { left: 52.0, ..VIEW }).left, 60.0);
        // Without an anchor in the middle.
        assert_eq!(place(None, VIEW).left, 570.0);
    }

    #[test]
    fn what_is_typed_finds_a_collection_or_makes_one() {
        let mut shelf = Shelf::default();
        let lena = shelf.create("Für Lena").unwrap();
        let wahl = shelf.create("Wahlpflicht").unwrap();
        shelf.toggle_in("11103", lena);
        shelf.toggle_in("12204", lena);
        let names = |lines: &[Line]| lines.iter().map(|line| match line { Line::Collection { name, .. } => name.clone(), Line::Create(name) => format!("+{name}") }).collect::<Vec<_>>();
        let one = ["11103".to_string()];
        // Nothing typed: every collection, with the module's check and how many marked ones it has.
        let all = lines_of(&shelf, &one, "", |module| module == "12204");
        assert_eq!(names(&all), ["Für Lena", "Wahlpflicht"]);
        assert_eq!(all.first(), Some(&Line::Collection { id: lena, name: "Für Lena".into(), count: 2, held: Held::All }));
        // „lena" finds „Für Lena", and offers to make „lena" after it.
        assert_eq!(names(&lines_of(&shelf, &one, "lena", |_| true)), ["Für Lena", "+lena"]);
        // A name there is that collection, never a second one.
        assert_eq!(names(&lines_of(&shelf, &one, " für lena ", |_| true)), ["Für Lena"]);
        assert_eq!(names(&lines_of(&shelf, &one, "mathe", |_| true)), ["+mathe"]);
        let _ = wahl;
    }

    /// oberflaeche.html, „Drei ausgewählte": a check where all of them are in, a dash where some
    /// are; a tap on the dash takes in the rest, one on the check takes all of them out.
    #[test]
    fn several_modules_see_where_all_or_some_of_them_are() {
        let mut shelf = Shelf::default();
        let lena = shelf.create("Für Lena").unwrap();
        let wahl = shelf.create("Wahlpflicht").unwrap();
        let witzig = shelf.create("Witzig").unwrap();
        for module in ["11103", "12204", "13849"] {
            shelf.toggle_in(module, wahl);
        }
        shelf.toggle_in("11103", lena);
        let three: Vec<String> = ["11103", "12204", "13849"].iter().map(|id| id.to_string()).collect();
        let held = |shelf: &Shelf| lines_of(shelf, &three, "", |_| false).into_iter().filter_map(|line| match line { Line::Collection { held, .. } => Some(held), Line::Create(_) => None }).collect::<Vec<_>>();
        assert_eq!(held(&shelf), [Held::Some, Held::All, Held::None]);
        // The dash: all of them in.
        let into = Held::of(&shelf, lena, &three) != Held::All;
        put_in(&mut shelf, &three, lena, into);
        assert_eq!(held(&shelf), [Held::All, Held::All, Held::None]);
        // The check: all of them out; the others' collections stay.
        let into = Held::of(&shelf, wahl, &three) != Held::All;
        put_in(&mut shelf, &three, wahl, into);
        assert_eq!(held(&shelf), [Held::All, Held::None, Held::None]);
        assert_eq!(shelf.of("12204"), [lena]);
        let _ = witzig;
    }

    #[test]
    fn the_phone_s_draft_adds_what_was_chosen_in_it() {
        let mut before = Shelf::default();
        let lena = before.create("Für Lena").unwrap();
        let mut draft = before.clone();
        let mathe = draft.create("Mathe").unwrap();
        draft.set_in("11103", &[lena, mathe].into_iter().collect());
        draft.set_note("11103", "Lena fragen");
        draft.set_standard(Some(&[mathe].into_iter().collect()));
        // Meanwhile another tab made „Witzig" (and took the number the draft gave „Mathe").
        let mut shelf = before.clone();
        let witzig = shelf.create("Witzig").unwrap();
        assert_eq!(witzig, mathe);
        adopt(&mut shelf, &before, &draft, &["11103".to_string()], true);
        let made = shelf.find("Mathe").unwrap();
        assert_ne!(made, witzig);
        assert_eq!(shelf.of("11103"), [lena, made]);
        assert_eq!(shelf.note("11103"), Some("Lena fragen"));
        assert_eq!(shelf.standard(), Some(&[made].into_iter().collect()));
        assert!(!shelf.holds(witzig, "11103"));
    }
}
