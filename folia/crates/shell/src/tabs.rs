//! The areas of the app behave like the tabs of an app (owner decision 2026-09-20, R19): each
//! remembers where it was left. Going from an open program to the catalog and back to
//! „Studium" returns to that program, not to the overview.
//!
//! „Studium" is the visitor's study first (owner, 2026-10-04: „primär die Studium planen Seite"):
//! its list is „Mein Studium" (`/study`), and the program overview and the programs' pages are what
//! the study is looked up in. Inside the area its tab leads up to „Mein Studium"; „Zurück" on a
//! program's page leads to the list it came from, „Mein Studium" or the overview.
//!
//! - From another area, a tab leads to where its area was left.
//! - In its own area — its list, a module, a program — the tab leads to where the area starts, its
//!   view as it opens without anything remembered (owner, 2026-11-18: „wenn man schon auf einen tab
//!   ist und dann nochmal darauf klickt, dass sich dann die standard Ansicht davon öffnet. Also bei
//!   mein Studium zum Beispiel die mein Studium page. Beim Modulkatalog für Standard Filter von
//!   Studiengang, bei der landing Page etc. Und ganz noch oben gescrollt"): the start page, the
//!   catalog of „Mein Studiengang" (`chrome::tab_home`), „Mein Studium", the Merkliste and the
//!   Stundenplan without a filter, a view or a module beside them; at its top, and a list there
//!   comes back to no row (`Tabs::afresh`, `TabAgain`). Until then the tab led up to the area's
//!   list as it was left, and on the list it was the plain link to the list without filters.
//!
//! The same memory answers two more questions: where „Zurück" on a module's or a program's page
//! leads (the area's list; through the browser history if that is where the visitor came from,
//! so the history does not grow), and which row a list shows when the visitor comes back to it.
//!
//! All of this is personal state: it lives in the browser (`sessionStorage`, so a reload keeps
//! it), never in the URL and never in server HTML (R9, R13). On the server and without
//! JavaScript every tab is the plain link to its area.

use folia_routes::url;
use leptos::prelude::*;

use crate::i18n::use_location;
use folia_design::nav;
use folia_stores::studyplan::Planning;

const STORAGE_KEY: &str = "betula.tabs";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    Home,
    Catalog,
    Programs,
    /// „Merkliste": the visitor's marked modules. It is one page, and a module opened from it is
    /// shown on it (`folia_widgets::local`), beside the list or in full.
    Bookmarks,
    /// „Studienplan": the visitor's plan. One page as well; a module opened from it stands beside
    /// the plan, and fills it after „Vollbild" (`folia_routes::url::PlanAddress`).
    Studyplan,
}

impl Area {
    /// The area shows the modules it lists in place (`folia_widgets::local`): beside its page and in full,
    /// without leaving the area. What is opened there stays the area's: a module's own page
    /// reached from it (a link on the module's page) does not become what the catalog remembers,
    /// and its „Zurück" leads back into the area (`Tabs::came_from`).
    pub fn shows_in_place(self) -> bool {
        matches!(self, Area::Programs | Area::Bookmarks | Area::Studyplan)
    }

    /// The area a tab names (`data-area` of the navigation's links).
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "home" => Area::Home,
            "catalog" => Area::Catalog,
            "programs" => Area::Programs,
            "bookmarks" => Area::Bookmarks,
            "studyplan" => Area::Studyplan,
            _ => return None,
        })
    }

    pub fn of(path: &str) -> Self {
        if path.starts_with(url::PROGRAMS) || path == url::STUDY {
            Area::Programs
        } else if path.starts_with(url::CATALOG) {
            Area::Catalog
        } else if path.starts_with(url::BOOKMARKS) {
            Area::Bookmarks
        } else if path.starts_with(url::STUDYPLAN) {
            Area::Studyplan
        } else {
            Area::Home
        }
    }

    /// Where „Zurück" on a page of the area leads when nothing is remembered: its list, and for
    /// „Studium" the overview of the programs, which every page of a program is one of (and which
    /// the server's pages link: it knows no „Mein Studium").
    pub fn back_root(self) -> &'static str {
        match self {
            Area::Programs => url::PROGRAMS,
            area => area.root(),
        }
    }

    /// The list of the area: where its tab leads when nothing is remembered. „Studium"'s is „Mein
    /// Studium", the app's alone: the server's tab leads to the overview (`chrome::NavItems`). Where
    /// the area starts, but for the catalog, whose first page is the catalog of „Mein Studiengang"
    /// where one is chosen (`chrome::tab_home`).
    pub fn root(self) -> &'static str {
        match self {
            Area::Home => url::HOME,
            Area::Catalog => url::CATALOG,
            Area::Programs => url::STUDY,
            Area::Bookmarks => url::BOOKMARKS,
            Area::Studyplan => url::STUDYPLAN,
        }
    }

    /// The digit of its tab with the navigation's keys (Alt, or Ctrl and Alt, and the digit;
    /// enhance.js): the tabs from the top of the rail down, from the left of a phone's bar.
    pub fn key(self) -> &'static str {
        match self {
            Area::Home => "1",
            Area::Catalog => "2",
            Area::Programs => "3",
            Area::Bookmarks => "4",
            Area::Studyplan => "5",
        }
    }
}

/// Path and query as one string, the way links are written.
pub fn location_of(path: &str, search: &str) -> String {
    match search.trim_start_matches('?') {
        "" => path.to_string(),
        search => format!("{path}?{search}"),
    }
}

fn path_of(location: &str) -> &str {
    location.split(['?', '#']).next().unwrap_or(location)
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Memory {
    /// The location the app is at, and the one before it.
    current: String,
    previous: String,
    /// Where each area was left, and where its list was left.
    catalog: Option<String>,
    catalog_list: Option<String>,
    programs: Option<String>,
    /// The list a program's page was last reached from: „Mein Studium" or the overview, as left.
    programs_list: Option<String>,
    /// The marked modules as they were left: their order, the module open beside them.
    bookmarks: Option<String>,
    /// The Studienplan as it was left: its semester and view, the module beside it.
    studyplan: Option<String>,
    /// „Mein Studium" as it was left (the module beside it): where the way back of the program
    /// overview leads up to (`Tabs::study`).
    study: Option<String>,
    /// The catalog lent to the workbench of „Mein Studium", with what its tab held before (`catalog`,
    /// `catalog_list`; owner, 2026-11-18: „wenn man von der Werkbank in den Katalog geht, dann
    /// überschreibt das die filter auch wenn man auf abbrechen oder fertig drückt. Die vorherigen
    /// Filter müssen danach wieder hergestellt werden"). While the workbench is open the catalog is
    /// its — „Im Katalog suchen" with the subgroup's filters and the semester's half of the year, the
    /// bar at its head —, and its pages are remembered as any, so that a module's „Zurück" leads to
    /// that list; the first step out of the catalog („Abbrechen", „Fertig", „Zurück zu WiSe
    /// 2026/27", another tab) gives back what the tab held. Only this session's: where it held
    /// nothing yet, nothing (the catalog of „Mein Studiengang" then, as on the first entry).
    lent: Option<(Option<String>, Option<String>)>,
    /// Where the tab of the area the app is in, tapped again, leads (`Tabs::afresh`), until the
    /// app's next address: the area's page as it starts, which comes from no page before it.
    again: Option<String>,
}

/// The line before what the catalog held when it was lent (`Memory::lent`), as this tab's session
/// keeps it.
const LENT: &str = "workbench";

/// A path of this site, else nothing: what is kept ends up in links.
fn site_path(line: &str) -> Option<String> {
    (line.starts_with('/') && !line.starts_with("//")).then(|| line.to_string())
}

impl Memory {
    /// The app is at `location`, `bench` whether the workbench of „Mein Studium" is open. True where
    /// it is where the tab tapped again led (`Tabs::afresh`): that page comes from no page before
    /// it, so that its list comes back to no row (`Tabs::before`).
    fn visit(&mut self, location: String, bench: bool) -> bool {
        if location == self.current {
            return false;
        }
        let afresh = self.again.take().is_some_and(|again| again == location);
        self.step(location, bench);
        if afresh {
            self.previous.clear();
        }
        afresh
    }

    fn step(&mut self, location: String, bench: bool) {
        let path = path_of(&location).to_string();
        // The catalog lent to the workbench from its first page while the workbench is open, until
        // the first step out of the catalog; a page of a while on top of it (a Termin, the profile:
        // `nav::passing`) is still in it.
        if Area::of(&path) == Area::Catalog {
            if bench && self.lent.is_none() {
                self.lent = Some((self.catalog.clone(), self.catalog_list.clone()));
            }
        } else if !crate::nav::passing(&path) {
            if let Some((catalog, list)) = self.lent.take() {
                (self.catalog, self.catalog_list) = (catalog, list);
            }
        }
        // A module's page opened out of an area that shows its modules in place (a program, the
        // marked modules: a link on the module shown there) belongs to that area, not to the
        // catalog: the catalog's tab keeps leading to the list as it was left, and that list does
        // not reveal a module the visitor never picked there (owner, 2026-09-20).
        if path.starts_with("/catalog/module/") && Area::of(path_of(&self.current)).shows_in_place() {
            self.previous = std::mem::replace(&mut self.current, location);
            return;
        }
        let (last, list) = match Area::of(&path) {
            Area::Catalog => (&mut self.catalog, &mut self.catalog_list),
            Area::Programs => (&mut self.programs, &mut self.programs_list),
            Area::Bookmarks => {
                self.bookmarks = Some(location.clone());
                self.previous = std::mem::replace(&mut self.current, location);
                return;
            }
            Area::Studyplan => {
                self.studyplan = Some(location.clone());
                self.previous = std::mem::replace(&mut self.current, location);
                return;
            }
            Area::Home => {
                self.previous = std::mem::replace(&mut self.current, location);
                return;
            }
        };
        if path == Area::of(&path).root() || path == url::PROGRAMS {
            *list = Some(location.clone());
        }
        if path == url::STUDY {
            self.study = Some(location.clone());
        }
        *last = Some(location.clone());
        self.previous = std::mem::replace(&mut self.current, location);
    }

    fn stored(&self) -> String {
        let mut lines = [&self.catalog, &self.catalog_list, &self.programs, &self.programs_list, &self.bookmarks, &self.studyplan, &self.study].map(|entry| entry.clone().unwrap_or_default()).to_vec();
        // The catalog lent to the workbench, so that a reload meanwhile still gives it back.
        if let Some((catalog, list)) = &self.lent {
            lines.extend([LENT.to_string(), catalog.clone().unwrap_or_default(), list.clone().unwrap_or_default()]);
        }
        lines.join("\n")
    }

    fn restored(stored: &str) -> Self {
        // Only paths of this site: what is stored ends up in links. What an older version stored
        // (fewer lines) reads as far as it goes.
        let mut lines = stored.lines();
        let mut next = || lines.next().and_then(site_path);
        let memory = Self { catalog: next(), catalog_list: next(), programs: next(), programs_list: next(), bookmarks: next(), studyplan: next(), study: next(), ..Default::default() };
        let lent = (lines.next() == Some(LENT)).then(|| (lines.next().and_then(site_path), lines.next().and_then(site_path)));
        Self { lent, ..memory }
    }
}

/// The memory of the tabs, shared through context.
#[derive(Clone, Copy)]
pub struct Tabs(RwSignal<Memory>);

impl Tabs {
    /// Creates the memory (from what this browser session remembers) and provides it.
    pub fn provide() -> Self {
        let tabs = Tabs(RwSignal::new(nav::session_get(STORAGE_KEY).map(|stored| Memory::restored(&stored)).unwrap_or_default()));
        provide_context(tabs);
        tabs
    }

    pub fn expect() -> Option<Self> {
        use_context::<Tabs>()
    }

    /// Follows the router. Call it once, inside the router.
    pub fn follow(self) {
        let location = use_location();
        // The workbench of „Mein Studium" open, which the catalog is lent to (`Memory::lent`).
        let planning = Planning::expect();
        Effect::new(move |_| {
            let now = location_of(&location.pathname.get(), &location.search.get());
            let bench = planning.is_some_and(|planning| planning.bench_untracked().is_some());
            let afresh = self.0.try_update(|memory| memory.visit(now, bench)).unwrap_or(false);
            nav::session_set(STORAGE_KEY, &self.0.with_untracked(Memory::stored));
            // Where the tab tapped again led: the page starts at its top, once it is built.
            if afresh {
                to_top(false);
            }
        });
    }

    /// Where the tab of `area` leads, seen from `path`.
    pub fn href(self, area: Area, path: &str) -> String {
        self.href_with_root(area, path, area.root())
    }

    /// Where the tab of `area` leads, seen from `path`, where the area has a first page of its own
    /// for this visitor (the catalog of „Mein Studiengang", A.10): from another area where the
    /// area was left, `root` only where nothing of it is remembered, its first entry of the
    /// session, so that a filter the visitor took away is not added back by a change of tabs. In
    /// its own area, on its list and on a page inside it, `root`: the tab tapped again opens the
    /// area as it starts (owner, 2026-11-18: „dass sich dann die standard Ansicht davon öffnet").
    pub fn href_with_root(self, area: Area, path: &str, root: &str) -> String {
        if Area::of(path) == area {
            return root.to_string();
        }
        self.0.with(|memory| {
            let last = match area {
                Area::Catalog => &memory.catalog,
                Area::Programs => &memory.programs,
                Area::Bookmarks => &memory.bookmarks,
                Area::Studyplan => &memory.studyplan,
                Area::Home => &None,
            };
            last.clone().unwrap_or_else(|| root.to_string())
        })
    }

    /// The tab of the area the app is in was tapped again and leads to `to`, where the area starts
    /// (`href_with_root`): that page comes from no page before it, so that its list comes back to
    /// no row it would show again (`before`), and starts at its top once the app is there
    /// (`follow`). Until the app's next address.
    pub fn afresh(self, to: &str) {
        self.0.update_untracked(|memory| memory.again = Some(to.to_string()));
    }

    /// The list of `area` as it was left: where „Zurück" on a page of the area leads. A program's
    /// page goes back to the list it was reached from, „Mein Studium" or the overview, and to the
    /// overview where nothing is remembered (`Area::back_root`).
    pub fn list(self, area: Area) -> String {
        self.0.with_untracked(|memory| match area {
            Area::Catalog => memory.catalog_list.clone(),
            Area::Programs => memory.programs_list.clone(),
            Area::Bookmarks => memory.bookmarks.clone(),
            Area::Studyplan => memory.studyplan.clone(),
            Area::Home => None,
        })
        .unwrap_or_else(|| area.back_root().to_string())
    }

    /// „Mein Studium" as it was left: where the way back of the program overview leads
    /// (`chrome::Up::Study`).
    pub fn study(self) -> String {
        self.0.with_untracked(|memory| memory.study.clone()).unwrap_or_else(|| url::STUDY.to_string())
    }

    /// The catalog's list as it was left, if it was: what the catalog's tab held opens with its
    /// filters (`chrome::NavItems`), where the tab itself may lead to a module's page.
    pub fn catalog_list(self) -> Option<String> {
        self.0.with(|memory| memory.catalog_list.clone())
    }

    /// The page of `area` the visitor was on last, whatever it was. Unlike `list` this is the
    /// page itself (a program with a module open beside it, a module of the catalog).
    pub fn left(self, area: Area) -> Option<String> {
        self.0.with_untracked(|memory| match area {
            Area::Catalog => memory.catalog.clone(),
            Area::Programs => memory.programs.clone(),
            Area::Bookmarks => memory.bookmarks.clone(),
            Area::Studyplan => memory.studyplan.clone(),
            Area::Home => None,
        })
    }

    /// Where the visitor was before `now`. Pages ask this while they are built, which may be
    /// before or after the memory has heard of `now`; the answer is the same either way. Nothing
    /// where the tab tapped again led to `now` (`afresh`).
    pub fn before(self, now: &str) -> String {
        self.0.with_untracked(|memory| {
            if memory.again.as_deref() == Some(now) {
                String::new()
            } else if memory.current == now {
                memory.previous.clone()
            } else {
                memory.current.clone()
            }
        })
    }

    /// The area the visitor was in before `now`. A module's page asks this: opened out of an area
    /// that shows its modules in place (`Area::shows_in_place`), it leads back into that area, not
    /// to the catalog's list.
    pub fn came_from(self, now: &str) -> Area {
        Area::of(path_of(&self.before(now)))
    }
}

/// A tab tapped while its area is the one the app is in (owner, 2026-10-06, of „Studium": „Wenn
/// man den anklickt muss es sofort wieder an die standard position gehen"; 2026-11-18, of every
/// tab: „dass sich dann die standard Ansicht davon öffnet … Und ganz noch oben gescrollt"): the
/// tab leads to where the area starts (`Tabs::href_with_root`), and the area's page goes back to
/// where it starts (`count`: „Mein Studium" its semester, its row and its workbench), also where
/// the tab leads to the address the app is at and the router has nothing to do. The page comes to
/// its top (`to_top`).
#[derive(Clone, Copy)]
pub struct TabAgain {
    counts: RwSignal<[u32; 5]>,
    /// How many pages of each area, built, take themselves to their top (`takes_its_top`).
    own: StoredValue<[u32; 5]>,
}

impl TabAgain {
    /// Creates the count and provides it: call it once, in `App`.
    pub fn provide() -> Self {
        let again = TabAgain::new();
        provide_context(again);
        again
    }

    fn new() -> Self {
        TabAgain { counts: RwSignal::new([0; 5]), own: StoredValue::new([0; 5]) }
    }

    pub fn expect() -> Option<Self> {
        use_context::<Self>()
    }

    fn slot(area: Area) -> usize {
        match area {
            Area::Home => 0,
            Area::Catalog => 1,
            Area::Programs => 2,
            Area::Bookmarks => 3,
            Area::Studyplan => 4,
        }
    }

    /// The tab of `area` was tapped while its area is the one the app is in.
    pub fn tapped(self, area: Area) {
        self.counts.update(|counts| {
            if let Some(count) = counts.get_mut(Self::slot(area)) {
                *count = count.wrapping_add(1);
            }
        });
    }

    /// How often the tab of `area` was tapped while its area was the one the app is in. Tracked.
    pub fn count(self, area: Area) -> u32 {
        self.counts.with(|counts| counts.get(Self::slot(area)).copied().unwrap_or(0))
    }

    /// The page of `area` being built takes itself to its top where its tab is tapped again while
    /// the app is there already (`count`): the row of „Mein Studium" on a phone, whose overview
    /// comes in from its top as the row turns to it, and whose window goes up at the overview. The
    /// window does not glide besides (`to_top`). For as long as the page is built.
    pub fn takes_its_top(self, area: Area) {
        let slot = Self::slot(area);
        self.own.update_value(|own| {
            if let Some(pages) = own.get_mut(slot) {
                *pages += 1;
            }
        });
        on_cleanup(move || {
            self.own.try_update_value(|own| {
                if let Some(pages) = own.get_mut(slot) {
                    *pages = pages.saturating_sub(1);
                }
            });
        });
    }

    /// Whether a page of `area` built takes itself to its top (`takes_its_top`).
    pub(crate) fn keeps_its_top(self, area: Area) -> bool {
        self.own.with_value(|own| own.get(Self::slot(area)).is_some_and(|pages| *pages > 0))
    }
}

/// The page in front to its top (owner, 2026-11-18: „Und ganz noch oben gescrollt"): the window on a
/// phone, the page's own scroll area on a wide screen, as „Nach oben" takes it there (enhance.js,
/// `betula:tab-again`). `glide`: from where it is, as „Nach oben" glides, where the tab tapped again
/// leaves the page what it is; else at once, in the next frame, where the tab led to where the area
/// starts and the page is built there (`Tabs::follow`).
pub(crate) fn to_top(glide: bool) {
    #[cfg(feature = "csr")]
    {
        use leptos::wasm_bindgen::JsValue;
        use leptos::web_sys;
        let send = move || {
            let Some(window) = web_sys::window() else { return };
            let detail = web_sys::js_sys::Object::new();
            let _ = web_sys::js_sys::Reflect::set(&detail, &JsValue::from_str("glide"), &JsValue::from_bool(glide));
            let init = web_sys::CustomEventInit::new();
            init.set_detail(&detail);
            if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("betula:tab-again", &init) {
                let _ = window.dispatch_event(&event);
            }
        };
        if glide {
            send();
        } else {
            request_animation_frame(send);
        }
    }
    #[cfg(not(feature = "csr"))]
    let _ = glide;
}

/// Whether the catalog is lent to the workbench of „Mein Studium" (`Memory::lent`), as this tab's
/// session keeps it: a phone's change of tabs out of it leaves the catalog's stack as it was before
/// (`nav`), as the memory gives back its pages.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
pub(crate) fn catalog_lent() -> bool {
    nav::session_get(STORAGE_KEY).is_some_and(|stored| Memory::restored(&stored).lent.is_some())
}

/// The last segment of `location` if it is a page below `prefix`: the module or program the
/// visitor comes back from.
pub fn page_below(location: &str, prefix: &str) -> Option<String> {
    let rest = path_of(location).strip_prefix(prefix)?.strip_prefix('/')?;
    let segment = rest.split('/').next().filter(|segment| !segment.is_empty())?;
    Some(url::decode(segment))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tab names its area as the navigation's links do (`data-area`), and a tab tapped again
    /// counts for its own area alone.
    #[test]
    fn a_tab_names_its_area() {
        for area in [Area::Home, Area::Catalog, Area::Programs, Area::Bookmarks, Area::Studyplan] {
            let name = match area {
                Area::Home => "home",
                Area::Catalog => "catalog",
                Area::Programs => "programs",
                Area::Bookmarks => "bookmarks",
                Area::Studyplan => "studyplan",
            };
            assert_eq!(Area::from_name(name), Some(area));
        }
        assert_eq!(Area::from_name("study"), None);
        // The count's signal needs an owner, as on a page: where the server is built in the same
        // run (a plain `cargo test`), `leptos_axum` turns on Leptos's sandboxed arenas, which keep
        // a signal in the arena of its owner and have none outside one.
        let owner = Owner::new();
        owner.set();
        let again = TabAgain::new();
        again.tapped(Area::Programs);
        again.tapped(Area::Programs);
        assert_eq!((again.count(Area::Programs), again.count(Area::Catalog)), (2, 0));
        // A page that takes itself to its top does so while it is built.
        let page = Owner::new();
        page.with(|| again.takes_its_top(Area::Programs));
        assert!(again.keeps_its_top(Area::Programs) && !again.keeps_its_top(Area::Catalog));
        page.cleanup();
        assert!(!again.keeps_its_top(Area::Programs));
    }

    #[test]
    fn tabs_remember_where_their_area_was_left() {
        let mut memory = Memory::default();
        for location in ["/catalog?turnus=winter", "/catalog?turnus=winter&page=3", "/catalog/module/11112", "/programs?level=master", "/programs/master-informatik-2008/plan"] {
            memory.visit(location.to_string(), false);
        }
        assert_eq!(memory.catalog.as_deref(), Some("/catalog/module/11112"));
        assert_eq!(memory.catalog_list.as_deref(), Some("/catalog?turnus=winter&page=3"));
        assert_eq!(memory.programs.as_deref(), Some("/programs/master-informatik-2008/plan"));
        assert_eq!(memory.programs_list.as_deref(), Some("/programs?level=master"));
        assert_eq!((memory.previous.as_str(), memory.current.as_str()), ("/programs?level=master", "/programs/master-informatik-2008/plan"));

        // A module opened out of a program leaves the catalog's memory alone.
        let mut from_program = Memory::default();
        for location in ["/catalog?turnus=winter", "/programs/master-informatik-2008/plan?open=11112", "/catalog/module/11112"] {
            from_program.visit(location.to_string(), false);
        }
        assert_eq!(from_program.catalog.as_deref(), Some("/catalog?turnus=winter"));
        assert_eq!(from_program.programs.as_deref(), Some("/programs/master-informatik-2008/plan?open=11112"));
        assert_eq!(from_program.previous.as_str(), "/programs/master-informatik-2008/plan?open=11112");

        // What a reload keeps: the areas, not the step before.
        let restored = Memory::restored(&memory.stored());
        assert_eq!((restored.catalog.clone(), restored.programs_list.clone()), (memory.catalog.clone(), memory.programs_list.clone()));
        assert_eq!(Memory::restored("javascript:alert(1)\n//evil.example/x\n/programs\n"), Memory { programs: Some("/programs".into()), ..Default::default() });
    }

    #[test]
    fn the_marked_modules_are_an_area_of_their_own() {
        assert_eq!(Area::of("/bookmarks"), Area::Bookmarks);
        let mut memory = Memory::default();
        for location in ["/catalog?turnus=winter", "/bookmarks?sort=ects&open=11112", "/bookmarks?sort=ects&open=11112&full=1"] {
            memory.visit(location.to_string(), false);
        }
        // „Vollbild" stays among the marked modules: their tab remembers it, the catalog's does not
        // hear of it.
        assert_eq!(memory.bookmarks.as_deref(), Some("/bookmarks?sort=ects&open=11112&full=1"));
        assert_eq!((memory.catalog.as_deref(), memory.catalog_list.as_deref()), (Some("/catalog?turnus=winter"), Some("/catalog?turnus=winter")));
        // A module's own page reached from there (a successor named on the page) is theirs as well:
        // its „Zurück" leads back to them, and the catalog's tab still leads to its list.
        memory.visit("/catalog/module/11113".to_string(), false);
        assert_eq!((Area::of(path_of(&memory.previous)), memory.catalog.as_deref()), (Area::Bookmarks, Some("/catalog?turnus=winter")));
        assert!(Area::Bookmarks.shows_in_place() && Area::Programs.shows_in_place() && !Area::Catalog.shows_in_place() && !Area::Home.shows_in_place());
        // A reload keeps it, and what an older version stored (four lines) still reads.
        assert_eq!(Memory::restored(&memory.stored()).bookmarks, memory.bookmarks);
        assert_eq!(Memory::restored("/catalog\n/catalog\n\n\n"), Memory { catalog: Some("/catalog".into()), catalog_list: Some("/catalog".into()), ..Default::default() });
    }

    #[test]
    fn the_studyplan_is_an_area_of_its_own() {
        // The tabs' signal needs an owner, as on a page: with `ssr` Leptos keeps a signal in the
        // arena of its owner and has none outside one.
        let owner = Owner::new();
        owner.set();
        assert_eq!((Area::of("/studyplan"), Area::Studyplan.root()), (Area::Studyplan, "/studyplan"));
        assert!(Area::Studyplan.shows_in_place());
        let mut memory = Memory::default();
        for location in ["/catalog?turnus=winter", "/studyplan?sem=2026W&view=dates", "/studyplan?sem=2026W&view=dates&open=12104&full=1"] {
            memory.visit(location.to_string(), false);
        }
        // Its tab remembers the plan as it was left, the module filling it included; the catalog's
        // does not hear of the module.
        assert_eq!(memory.studyplan.as_deref(), Some("/studyplan?sem=2026W&view=dates&open=12104&full=1"));
        assert_eq!(memory.catalog.as_deref(), Some("/catalog?turnus=winter"));
        // A module's own page reached from there belongs to the plan: its „Zurück" leads back to it.
        memory.visit("/catalog/module/12107".to_string(), false);
        assert_eq!((Area::of(path_of(&memory.previous)), memory.catalog.as_deref()), (Area::Studyplan, Some("/catalog?turnus=winter")));

        // The tab: from elsewhere to where the plan was left; on the plan itself where it starts.
        let tabs = Tabs(RwSignal::new(memory.clone()));
        assert_eq!(tabs.href(Area::Studyplan, "/catalog"), "/studyplan?sem=2026W&view=dates&open=12104&full=1");
        assert_eq!(tabs.href(Area::Studyplan, "/studyplan"), "/studyplan");
        assert_eq!(tabs.list(Area::Studyplan), "/studyplan?sem=2026W&view=dates&open=12104&full=1");
        assert_eq!(Tabs(RwSignal::new(Memory::default())).href(Area::Studyplan, "/catalog"), "/studyplan");

        // A reload keeps it as the sixth line, and what the version before stored (five lines) reads.
        let stored = memory.stored();
        assert_eq!(stored.split('\n').count(), 7, "the sixth of seven, „Mein Studium“ after it");
        assert_eq!(Memory::restored(&stored).studyplan, memory.studyplan);
        let five = "/catalog\n/catalog\n/programs\n/programs\n/bookmarks?sort=ects";
        assert_eq!(
            Memory::restored(five),
            Memory { catalog: Some("/catalog".into()), catalog_list: Some("/catalog".into()), programs: Some("/programs".into()), programs_list: Some("/programs".into()), bookmarks: Some("/bookmarks?sort=ects".into()), ..Default::default() }
        );
        assert_eq!(Memory::restored("\n\n\n\n\n//evil.example/studyplan").studyplan, None);
    }

    #[test]
    fn studium_leads_to_mein_studium() {
        // The tabs' signal needs an owner, as on a page.
        let owner = Owner::new();
        owner.set();
        assert_eq!((Area::of("/study"), Area::Programs.root(), Area::Programs.back_root()), (Area::Programs, "/study", "/programs"));
        assert!(Area::Programs.shows_in_place());
        let fresh = Tabs(RwSignal::new(Memory::default()));
        // Nothing remembered: „Mein Studium" from everywhere, and a program's „Zurück" the overview.
        assert_eq!(fresh.href(Area::Programs, "/catalog"), "/study");
        assert_eq!(fresh.href(Area::Programs, "/programs/bachelor-informatik-2008/plan"), "/study");
        assert_eq!(fresh.list(Area::Programs), "/programs");

        let mut memory = Memory::default();
        for location in ["/study?open=12104", "/programs/bachelor-informatik-2008/plan", "/catalog?turnus=winter"] {
            memory.visit(location.to_string(), false);
        }
        let tabs = Tabs(RwSignal::new(memory.clone()));
        // From another area: where the area was left; inside it „Mein Studium" as it starts (owner,
        // 2026-11-18: „bei mein Studium zum Beispiel die mein Studium page"), not as it was left.
        assert_eq!(tabs.href(Area::Programs, "/catalog"), "/programs/bachelor-informatik-2008/plan");
        assert_eq!(tabs.href(Area::Programs, "/programs/bachelor-informatik-2008/areas"), "/study");
        assert_eq!(tabs.href(Area::Programs, "/programs"), "/study");
        assert_eq!(tabs.href(Area::Programs, "/study"), "/study");
        // The way back of the program overview: „Mein Studium" as it was left.
        assert_eq!(tabs.study(), "/study?open=12104");
        // „Zurück" on the program's page: the list it was reached from.
        assert_eq!(tabs.list(Area::Programs), "/study?open=12104");
        memory.visit("/programs?level=master".to_string(), false);
        memory.visit("/programs/master-informatik-2008/plan".to_string(), false);
        assert_eq!(Tabs(RwSignal::new(memory.clone())).list(Area::Programs), "/programs?level=master");
        // A module's own page reached from „Mein Studium" is the area's, as from a program.
        memory.visit("/study".to_string(), false);
        memory.visit("/catalog/module/12104".to_string(), false);
        assert_eq!((Area::of(path_of(&memory.previous)), memory.catalog.as_deref()), (Area::Programs, Some("/catalog?turnus=winter")));
        // A reload keeps it as the seventh line.
        assert_eq!(Memory::restored(&memory.stored()).study.as_deref(), Some("/study"));
    }

    #[test]
    fn the_catalog_of_mein_studiengang_is_only_the_first_entry() {
        // The tabs' signal needs an owner, as on a page: with `ssr` Leptos keeps a signal in the
        // arena of its owner and has none outside one.
        let owner = Owner::new();
        owner.set();
        let mine = "/catalog?program=bachelor-informatik-2008";
        // Nothing of the catalog remembered in this session: the tab leads to the program's
        // catalog, from another area and from a module's page reached some other way.
        let fresh = Tabs(RwSignal::new(Memory::default()));
        assert_eq!(fresh.href_with_root(Area::Catalog, "/", mine), mine);
        assert_eq!(fresh.href_with_root(Area::Catalog, "/catalog/module/12104", mine), mine);
        // On the catalog's list itself the tab tapped again leads there too, where the catalog
        // starts (owner, 2026-11-18: „Beim Modulkatalog für Standard Filter von Studiengang"); until
        // then a reset to the whole catalog.
        assert_eq!(fresh.href_with_root(Area::Catalog, "/catalog", mine), mine);

        // Once the catalog was visited, what it was left at wins from another area: the student who
        // took the program's tag away is not led back to it by a change of tabs. In the catalog its
        // tab is the catalog as it starts.
        let mut memory = Memory::default();
        for location in [mine, "/catalog", "/programs"] {
            memory.visit(location.to_string(), false);
        }
        let visited = Tabs(RwSignal::new(memory));
        assert_eq!(visited.href_with_root(Area::Catalog, "/programs", mine), "/catalog");
        assert_eq!(visited.href_with_root(Area::Catalog, "/catalog/module/12104", mine), mine);
        assert_eq!(visited.href_with_root(Area::Catalog, "/catalog", mine), mine);
        // `href` is the same with the area's own root.
        assert_eq!(visited.href(Area::Catalog, "/programs"), visited.href_with_root(Area::Catalog, "/programs", "/catalog"));
        assert_eq!(fresh.href(Area::Catalog, "/"), "/catalog");
    }

    #[test]
    fn the_workbench_gives_the_catalog_back() {
        // The tabs' signal needs an owner, as on a page.
        let owner = Owner::new();
        owner.set();
        let mine = "/catalog?program=bachelor-informatik-2008";
        let bench = "/catalog?program=bachelor-informatik-2008&turnus=winter&fill=p3";
        // The student's own filters, then the workbench's „Im Katalog suchen", a module beside its
        // list and its own page, whose „Zurück" leads to the workbench's list.
        let mut memory = Memory::default();
        memory.visit("/catalog?turnus=summer&lang=en".to_string(), false);
        memory.visit("/catalog?turnus=summer&lang=en&open=11112".to_string(), false);
        for location in ["/study?plan=1", bench, &format!("{bench}&open=12160"), "/catalog/module/12160"] {
            memory.visit(location.to_string(), true);
        }
        let beside = format!("{bench}&open=12160");
        assert_eq!(Tabs(RwSignal::new(memory.clone())).list(Area::Catalog), beside);
        // A reload meanwhile keeps the loan.
        let reloaded = Memory::restored(&memory.stored());
        assert_eq!((reloaded.catalog_list.as_deref(), reloaded.lent.clone()), (Some(beside.as_str()), memory.lent.clone()));
        // „Fertig": the tab leads to the catalog as the student left it, from everywhere.
        memory.visit(format!("{bench}&open=12160"), true);
        memory.visit("/study?plan=1".to_string(), true);
        assert_eq!(memory.lent, None);
        let own = "/catalog?turnus=summer&lang=en&open=11112";
        assert_eq!((memory.catalog.as_deref(), memory.catalog_list.as_deref()), (Some(own), Some(own)));
        let tabs = Tabs(RwSignal::new(memory.clone()));
        assert_eq!((tabs.href_with_root(Area::Catalog, "/study", mine), tabs.list(Area::Catalog)), (own.to_string(), own.to_string()));
        assert_eq!(Memory::restored(&memory.stored()), Memory { current: String::new(), previous: String::new(), ..memory.clone() });
        assert_eq!(memory.stored().split('\n').count(), 7, "nothing lent, nothing more kept");

        // Only this session's: a catalog not visited before the workbench is not visited after it,
        // and the tab's first entry is the catalog of „Mein Studiengang" again.
        let mut fresh = Memory::default();
        for location in ["/study", bench, "/catalog?turnus=winter"] {
            fresh.visit(location.to_string(), true);
        }
        // Back to „Mein Studium" with the browser's Back, a Termin's page of a while on the way.
        fresh.visit("/event/148370-eb017".to_string(), true);
        assert!(fresh.lent.is_some(), "a page of a while is still the catalog's");
        fresh.visit("/study".to_string(), true);
        assert_eq!((fresh.catalog.clone(), fresh.catalog_list.clone(), fresh.lent.clone()), (None, None, None));
        assert_eq!(Tabs(RwSignal::new(fresh)).href_with_root(Area::Catalog, "/study", mine), mine);

        // Without the workbench the catalog is the tab's as always; a module's page opened from
        // „Mein Studium" lends nothing it would change.
        let mut plain = Memory::default();
        for location in ["/catalog?turnus=summer", "/study", "/catalog?turnus=winter", "/study"] {
            plain.visit(location.to_string(), false);
        }
        assert_eq!(plain.catalog_list.as_deref(), Some("/catalog?turnus=winter"));
        plain.visit("/catalog/module/12104".to_string(), true);
        plain.visit("/study".to_string(), true);
        assert_eq!((plain.catalog_list.as_deref(), plain.lent.clone()), (Some("/catalog?turnus=winter"), None));
        // What a session kept is read back only as paths of this site.
        assert_eq!(Memory::restored("\n\n\n\n\n\n\nworkbench\n//evil.example\n/catalog").lent, Some((None, Some("/catalog".into()))));
        assert_eq!(Memory::restored("\n\n\n\n\n\n\nsomething\n/catalog\n/catalog").lent, None);
    }

    #[test]
    fn a_tab_tapped_again_starts_its_area_afresh() {
        // The tabs' signal needs an owner, as on a page.
        let owner = Owner::new();
        owner.set();
        let mine = "/catalog?program=bachelor-informatik-2008";
        let mut memory = Memory::default();
        for location in ["/catalog?turnus=winter", "/catalog?turnus=winter&open=11112", "/catalog/module/11112"] {
            assert!(!memory.visit(location.to_string(), false), "no tab tapped again");
        }
        let tabs = Tabs(RwSignal::new(memory));
        // The list a module's page goes back to shows that module's row again (`page_below`)…
        assert_eq!(tabs.before(mine), "/catalog/module/11112");
        // …but not where the tab tapped again leads: asked while that page is built, before the
        // memory has heard of it, and after.
        tabs.afresh(mine);
        assert_eq!(tabs.before(mine), "");
        assert!(tabs.0.try_update(|memory| memory.visit(mine.to_string(), false)).unwrap_or(false), "the page the tab led to");
        assert_eq!(tabs.before(mine), "");
        // It is remembered as any page of the area: the catalog's tab leads there from elsewhere,
        // „Zurück" of a module's page to it.
        assert_eq!((tabs.left(Area::Catalog).as_deref(), tabs.list(Area::Catalog)), (Some(mine), mine.to_string()));
        // Only until the next address: one elsewhere ends it, and the page after it comes from
        // the page before as always.
        tabs.afresh("/study");
        let open = format!("{mine}&open=12104");
        assert!(!tabs.0.try_update(|memory| memory.visit(open.clone(), false)).unwrap_or(true));
        assert_eq!((tabs.before(&open), tabs.before("/study")), (mine.to_string(), open.clone()));
        // A visit to the address the app is at is no visit.
        tabs.afresh(&open);
        assert!(!tabs.0.try_update(|memory| memory.visit(open.clone(), false)).unwrap_or(true));
        // What a reload keeps knows nothing of it.
        assert_eq!(tabs.0.with_untracked(|memory| Memory::restored(&memory.stored()).again), None);
    }

    #[test]
    fn pages_below_a_list() {
        assert_eq!(page_below("/catalog/module/11112", "/catalog/module").as_deref(), Some("11112"));
        assert_eq!(page_below("/programs/bachelor-informatik-2008/plan?x=1", "/programs").as_deref(), Some("bachelor-informatik-2008"));
        assert_eq!(page_below("/programs?level=master", "/programs"), None);
        assert_eq!(page_below("/catalog?open=1", "/programs"), None);
    }
}
