//! Tabs that stay built (folia/design/tabwechsel/konzept.html, „Tabs, die stehen bleiben"; owner,
//! 2026-10-18: „Es ist nice zu sehen, dass die Seiten gleich da sind. Das muss auf jeden Fall
//! rein"). In the browser every tab has a stage in `main`, and its page stays built there while
//! Betula is open: a change of tab only swaps which stage is in front, without a skeleton and
//! without a frame's delay, and the page comes back as it was left, scrolled as far.
//!
//! - **A place for each stage.** The stage the router shows (`shown`) is where the router is; the
//!   others stay where they were left, the top of their tab's stack (`nav`). The pages read their
//!   stage's place (`folia_design::stage`, `i18n::use_location`), and a change within the tab
//!   (a filter, a module beside the list) changes the page on its stage as it always did.
//! - **In front** (`front`): the stage the router shows, or at once the stage that shows where the
//!   app is going (`Pending::to`): a tab whose stage stands there already comes in the frame of the
//!   tap, and the router follows (`Pending` takes the step at once, `nav` after the history is
//!   written). A stage that does not show it yet comes with the router, as a page did before.
//! - **Behind** is built and not drawn: `content-visibility: hidden` and `inert` (app.css). Nothing
//!   there takes a finger or a reader, and the browser keeps what it worked out. The document's
//!   title and the tags for search engines are those of the page in front.
//! - **The window's scroll on a phone** is the stage's: kept as it goes behind, given back in the
//!   frame it comes to the front. On a wide screen every page scrolls in its own areas, which keep
//!   their place.
//! - **Which stage** (`Key::of`): its tab's; a module's own page opened from a tab that shows its
//!   modules in place stays on that tab's stage (`nav::tab_of_page`, `Tabs`), and the pages of no
//!   tab (the profile, the legal pages, a page that does not exist) have a stage of their own.
//!
//! The server knows nothing of this: it renders the page of its address with `<Routes>`, in a
//! stage in front, so that the browser app's page stands as the server's does at the takeover.

use std::rc::Rc;

use folia_design::stage::{Stage, Titles};
use leptos::context::Provider;
use leptos::oco::Oco;
use leptos::prelude::*;
use leptos_router::location::Location;
use leptos_router::params::ParamsMap;

use crate::pending::Pending;
use crate::tabs::Area;

/// The stages: one for each tab, and one for the pages of none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Home,
    Catalog,
    Programs,
    Bookmarks,
    Studyplan,
    /// The profile, the legal pages, a page that does not exist: the one visited last stays.
    Other,
}

impl Key {
    pub const ALL: [Key; 6] = [Key::Home, Key::Catalog, Key::Programs, Key::Bookmarks, Key::Studyplan, Key::Other];

    /// Its name in the page (`data-stage`), for app.css, enhance.js and the checks.
    pub fn name(self) -> &'static str {
        match self {
            Key::Home => "home",
            Key::Catalog => "catalog",
            Key::Programs => "programs",
            Key::Bookmarks => "bookmarks",
            Key::Studyplan => "studyplan",
            Key::Other => "other",
        }
    }

    fn index(self) -> usize {
        match self {
            Key::Home => 0,
            Key::Catalog => 1,
            Key::Programs => 2,
            Key::Bookmarks => 3,
            Key::Studyplan => 4,
            Key::Other => 5,
        }
    }

    fn area(self) -> Option<Area> {
        match self {
            Key::Home => Some(Area::Home),
            Key::Catalog => Some(Area::Catalog),
            Key::Programs => Some(Area::Programs),
            Key::Bookmarks => Some(Area::Bookmarks),
            Key::Studyplan => Some(Area::Studyplan),
            Key::Other => None,
        }
    }

    fn of_area(area: Area) -> Self {
        match area {
            Area::Home => Key::Home,
            Area::Catalog => Key::Catalog,
            Area::Programs => Key::Programs,
            Area::Bookmarks => Key::Bookmarks,
            Area::Studyplan => Key::Studyplan,
        }
    }

    /// The stage of `path`, come to from the stage `from`: its tab's, but a module's own page opened
    /// from a tab that shows its modules in place stays on that tab's stage, and a page of no tab
    /// is on the stage of its own.
    pub fn of(path: &str, from: Key) -> Self {
        match Page::of(path) {
            Page::Profile | Page::Imprint | Page::Privacy | Page::NotFound => Key::Other,
            Page::Module if from.area().is_some_and(Area::shows_in_place) => from,
            _ => Key::of_area(Area::of(path)),
        }
    }
}

/// The pages of the app, by the routes of `folia_app::App` (the server's `<Routes>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Home,
    Catalog,
    /// `/catalog/module/:id`
    Module,
    Programs,
    /// `/programs/:slug`
    Program,
    /// `/programs/:slug/:tab`
    ProgramTab,
    Bookmarks,
    Studyplan,
    Study,
    Profile,
    Imprint,
    Privacy,
    NotFound,
}

impl Page {
    /// The page `path` shows, as the routes match it.
    pub fn of(path: &str) -> Self {
        let segments: Vec<&str> = path.split('/').filter(|segment| !segment.is_empty()).collect();
        match segments.as_slice() {
            [] => Page::Home,
            ["catalog"] => Page::Catalog,
            ["catalog", "module", _] => Page::Module,
            ["programs"] => Page::Programs,
            ["programs", _] => Page::Program,
            ["programs", _, _] => Page::ProgramTab,
            ["bookmarks"] => Page::Bookmarks,
            ["studyplan"] => Page::Studyplan,
            ["study"] => Page::Study,
            ["profile"] => Page::Profile,
            ["impressum"] => Page::Imprint,
            ["datenschutz"] => Page::Privacy,
            _ => Page::NotFound,
        }
    }

    /// The params its route takes from `path`, as the router has them (the segments as written).
    pub fn params(self, path: &str) -> ParamsMap {
        let segments: Vec<&str> = path.split('/').filter(|segment| !segment.is_empty()).collect();
        let mut params = ParamsMap::new();
        match (self, segments.as_slice()) {
            (Page::Module, [_, _, id]) => params.insert("id", (*id).to_string()),
            (Page::Program, [_, slug]) => params.insert("slug", (*slug).to_string()),
            (Page::ProgramTab, [_, slug, tab]) => {
                params.insert("slug", (*slug).to_string());
                params.insert("tab", (*tab).to_string());
            }
            _ => {}
        }
        params
    }
}

/// Where a stage is: path (without the language's prefix), query (without `?`) and its pairs, and
/// the fragment, as the router has them.
#[derive(Clone, Debug, PartialEq)]
struct Place {
    path: String,
    search: String,
    query: ParamsMap,
    hash: String,
}

impl Place {
    fn of(pathname: Memo<String>, search: Memo<String>, query: Memo<ParamsMap>, hash: Memo<String>) -> Self {
        Place { path: pathname.get(), search: search.get(), query: query.get(), hash: hash.get() }
    }

    /// Whether this is the place of the address `to` (path, query and fragment as a link writes
    /// them): the same path and query.
    fn is(&self, to: &str) -> bool {
        let without_fragment = to.split('#').next().unwrap_or(to);
        let (path, search) = without_fragment.split_once('?').unwrap_or((without_fragment, ""));
        self.path == path && self.search == search
    }
}

/// The page on each stage: every route of the app as the stages show it.
pub type Pages = fn(Page) -> AnyView;

/// The stages in `main`, in the browser app: one for each tab and one for the pages of none, each
/// with the page of its place (`pages`), the one in front drawn.
#[component]
pub fn Stages(pages: Pages) -> impl IntoView {
    let router = folia_design::i18n::router_location();
    let (pathname, search, query, hash, state) = (router.pathname, router.search, router.query, router.hash, router.state);
    let pending = Pending::expect();
    // Where the router is, and which stage shows it.
    let here = Memo::new(move |_| Place::of(pathname, search, query, hash));
    let shown = Memo::new(move |before: Option<&Key>| Key::of(&pathname.get(), before.copied().unwrap_or(Key::Home)));
    let places: [RwSignal<Option<Place>>; 6] = std::array::from_fn(|_| RwSignal::new(None));
    let place = move |key: Key| places.get(key.index()).copied();
    // The page the app starts with is there with the first frame.
    if let Some(first) = place(shown.get_untracked()) {
        first.set(Some(here.get_untracked()));
    }
    // The stage the router shows is where the router is.
    Effect::new(move |_| {
        let (key, now) = (shown.get(), here.get());
        if let Some(slot) = place(key) {
            if slot.with_untracked(|was| was.as_ref() != Some(&now)) {
                slot.set(Some(now));
            }
        }
    });
    // Another stage than the router's that shows `to` already: where a change only swaps.
    let standing = move |to: &str| -> Option<Key> {
        let path = to.split(['?', '#']).next().unwrap_or(to);
        let from = shown.get_untracked();
        let key = Key::of(path, from);
        (key != from && place(key).is_some_and(|slot| slot.with_untracked(|place| place.as_ref().is_some_and(|place| place.is(to))))).then_some(key)
    };
    if let Some(pending) = pending {
        pending.bind_stages(Rc::new(move |to: &str| standing(to).is_some()));
    }
    // In front: where the app is going, if a stage shows it already; else the router's stage.
    let front = Memo::new(move |_| {
        let shown = shown.get();
        let going = pending.and_then(|pending| pending.to());
        // Tracked: a place the app is going to may come to stand on its stage meanwhile.
        for key in Key::ALL {
            if let Some(slot) = place(key) {
                slot.track();
            }
        }
        going.and_then(|to| standing(&to)).unwrap_or(shown)
    });
    let nodes: [NodeRef<leptos::html::Div>; 6] = std::array::from_fn(|_| NodeRef::new());
    // The document's title is the one of the page in front (`folia_design::stage::Title`).
    let titles: [Titles; 6] = std::array::from_fn(|_| Titles::default());
    let title = move || titles.get(front.get().index()).map_or(Oco::Borrowed(""), |titles| titles.shown());
    let first = front.get_untracked();
    // How far the window was scrolled on each stage (a phone), as it scrolls: by the time another
    // stage comes to the front, the router may have taken the window to the top of the new page.
    let scrolled = StoredValue::new([None::<f64>; 6]);
    #[cfg(feature = "csr")]
    Effect::new(move |_| {
        let handle = window_event_listener(leptos::ev::scroll, move |_| {
            if !folia_design::nav::is_phone() {
                return;
            }
            let down = leptos::web_sys::window().and_then(|window| window.scroll_y().ok());
            let key = front.get_untracked();
            scrolled.update_value(|scrolled| {
                if let Some(slot) = scrolled.get_mut(key.index()) {
                    *slot = down;
                }
            });
        });
        on_cleanup(move || handle.remove());
    });
    // Where each stage was when it went behind: one that comes back there is scrolled as it was
    // left (at the top if it never scrolled); one at a new place is where the router puts it.
    let left = StoredValue::new(std::array::from_fn::<Option<Place>, 6, _>(|_| None));
    Effect::new(move |before: Option<Key>| {
        let now = front.get();
        let before = before.unwrap_or(first);
        let mut restore = None;
        if before != now {
            // Where `now` stands: the router's place if the router shows it, else its own.
            let at = if now == shown.get_untracked() { Some(here.get_untracked()) } else { place(now).and_then(|slot| slot.get_untracked()) };
            let back = at.is_some() && left.with_value(|left| left.get(now.index()).is_some_and(|was| *was == at));
            scrolled.update_value(|scrolled| {
                if let Some(down) = scrolled.get_mut(now.index()) {
                    if back {
                        restore = Some(down.unwrap_or(0.0));
                    } else {
                        *down = None;
                    }
                }
            });
            let was = place(before).and_then(|slot| slot.get_untracked());
            left.update_value(|left| {
                if let Some(entry) = left.get_mut(before.index()) {
                    *entry = was;
                }
            });
        }
        #[cfg(feature = "csr")]
        browser::swap(before, now, &nodes, restore);
        #[cfg(not(feature = "csr"))]
        let _ = restore;
        now
    });
    let stages = Key::ALL
        .into_iter()
        .filter_map(|key| {
            let slot = place(key)?;
            let node = nodes.get(key.index()).copied()?;
            let titles = titles.get(key.index()).copied()?;
            let location = Location {
                pathname: Memo::new(move |_| slot.with(|place| place.as_ref().map(|place| place.path.clone()).unwrap_or_default())),
                search: Memo::new(move |_| slot.with(|place| place.as_ref().map(|place| place.search.clone()).unwrap_or_default())),
                query: Memo::new(move |_| slot.with(|place| place.as_ref().map(|place| place.query.clone()).unwrap_or_default())),
                hash: Memo::new(move |_| slot.with(|place| place.as_ref().map(|place| place.hash.clone()).unwrap_or_default())),
                state,
            };
            let params = Memo::new(move |_| slot.with(|place| place.as_ref().map(|place| Page::of(&place.path).params(&place.path)).unwrap_or_default()));
            let which = Memo::new(move |_| slot.with(|place| place.as_ref().map(|place| Page::of(&place.path))));
            let in_front = Memo::new(move |_| front.get() == key);
            let stage = Stage { location, params, front: in_front.into(), node, titles };
            Some(view! { <OneStage key stage which pages front=key == first/> })
        })
        .collect_view();
    view! {
        <leptos_meta::Title text=title/>
        {stages}
    }
}

/// A stage and the page of its place: the page is built anew when another page of the app comes to
/// the stage, not when its query changes (the router's routes did the same). The page has the
/// stage in an owner of its own (`Provider`): a component has none, and six stages given beside
/// each other would leave every page the last one.
#[component]
fn OneStage(key: Key, stage: Stage, which: Memo<Option<Page>>, pages: Pages, front: bool) -> impl IntoView {
    let node = stage.node;
    view! {
        <Provider value=stage>
            <div class=if front { "page-stage front" } else { "page-stage" } data-stage=key.name() inert=(!front).then_some("") node_ref=node>
                {move || which.get().map(|page| untrack(|| pages(page)))}
            </div>
        </Provider>
    }
}

#[cfg(feature = "csr")]
mod browser {
    use leptos::prelude::*;
    use leptos::wasm_bindgen::JsCast;
    use leptos::web_sys;

    use super::Key;

    /// The stage `now` comes to the front, `before` goes behind: inert and not drawn, and on a phone
    /// the window scrolled to `restore` (where `now` was left, if it comes back there), in this
    /// frame.
    pub(super) fn swap(before: Key, now: Key, nodes: &[NodeRef<leptos::html::Div>; 6], restore: Option<f64>) {
        let element = |key: Key| nodes.get(key.index()).and_then(|node| node.get_untracked()).map(|node| node.unchecked_into::<web_sys::Element>());
        let window = web_sys::window();
        if before != now {
            let phone = folia_design::nav::is_phone();
            if let Some(stage) = element(before) {
                // The page that goes behind is left, as a new address left it: a dialog open on it
                // goes with it (its `close` tells the page).
                if let Ok(open) = stage.query_selector_all("dialog[open]") {
                    for dialog in (0..open.length()).filter_map(|i| open.item(i)?.dyn_into::<web_sys::HtmlDialogElement>().ok()) {
                        dialog.close();
                    }
                }
                let _ = stage.class_list().remove_1("front");
                let _ = stage.set_attribute("inert", "");
            }
            if let Some(stage) = element(now) {
                let _ = stage.remove_attribute("inert");
                let _ = stage.class_list().add_1("front");
            }
            if let (true, Some(window), Some(top)) = (phone, window.as_ref(), restore) {
                let options = web_sys::ScrollToOptions::new();
                options.set_top(top);
                options.set_behavior(web_sys::ScrollBehavior::Instant);
                window.scroll_to_with_scroll_to_options(&options);
            }
        }
        folia_design::stage::set_front(element(now));
        // enhance.js follows the page in front: its scroll area, „Nach oben", a sheet left open.
        if before != now {
            if let (Some(window), Ok(event)) = (window, web_sys::Event::new("betula:stage")) {
                let _ = window.dispatch_event(&event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every route of the app's is a page here, and what is none is the page that does not exist.
    #[test]
    fn the_page_of_a_path() {
        assert_eq!(Page::of("/"), Page::Home);
        assert_eq!(Page::of("/catalog"), Page::Catalog);
        assert_eq!(Page::of("/catalog/"), Page::Catalog);
        assert_eq!(Page::of("/catalog/module/11103"), Page::Module);
        assert_eq!(Page::of("/catalog/module"), Page::NotFound);
        assert_eq!(Page::of("/programs"), Page::Programs);
        assert_eq!(Page::of("/programs/bachelor-informatik-2008"), Page::Program);
        assert_eq!(Page::of("/programs/bachelor-informatik-2008/areas"), Page::ProgramTab);
        assert_eq!(Page::of("/programs/a/b/c"), Page::NotFound);
        assert_eq!((Page::of("/bookmarks"), Page::of("/studyplan"), Page::of("/study")), (Page::Bookmarks, Page::Studyplan, Page::Study));
        assert_eq!((Page::of("/profile"), Page::of("/impressum"), Page::of("/datenschutz")), (Page::Profile, Page::Imprint, Page::Privacy));
        assert_eq!(Page::of("/nichts"), Page::NotFound);
        assert_eq!(Page::Module.params("/catalog/module/11103").get("id").as_deref(), Some("11103"));
        let program = Page::ProgramTab.params("/programs/bachelor-informatik-2008/areas");
        assert_eq!((program.get("slug").as_deref(), program.get("tab").as_deref()), (Some("bachelor-informatik-2008"), Some("areas")));
        assert_eq!(Page::Program.params("/programs/x").get("tab"), None);
    }

    /// A page is on its tab's stage; a module's own page stays on a tab that shows its modules in
    /// place; the pages of no tab have a stage of their own.
    #[test]
    fn the_stage_of_a_page() {
        assert_eq!(Key::of("/", Key::Catalog), Key::Home);
        assert_eq!(Key::of("/catalog", Key::Home), Key::Catalog);
        assert_eq!(Key::of("/catalog/module/11103", Key::Catalog), Key::Catalog);
        assert_eq!(Key::of("/catalog/module/11103", Key::Home), Key::Catalog);
        assert_eq!(Key::of("/catalog/module/11103", Key::Bookmarks), Key::Bookmarks);
        assert_eq!(Key::of("/catalog/module/11103", Key::Programs), Key::Programs);
        assert_eq!(Key::of("/catalog/module/11103", Key::Studyplan), Key::Studyplan);
        assert_eq!(Key::of("/catalog/module/11103", Key::Other), Key::Catalog);
        assert_eq!((Key::of("/study", Key::Home), Key::of("/programs/x/plan", Key::Catalog)), (Key::Programs, Key::Programs));
        assert_eq!((Key::of("/profile", Key::Catalog), Key::of("/impressum", Key::Home), Key::of("/nichts", Key::Home)), (Key::Other, Key::Other, Key::Other));
    }

    #[test]
    fn a_place_and_an_address() {
        let place = Place { path: "/catalog".into(), search: "turnus=winter".into(), query: ParamsMap::new(), hash: String::new() };
        assert!(place.is("/catalog?turnus=winter") && place.is("/catalog?turnus=winter#x"));
        assert!(!place.is("/catalog") && !place.is("/catalog?turnus=sommer") && !place.is("/bookmarks?turnus=winter"));
    }
}
