//! The areas of the app behave like the tabs of an app (owner decision 2026-09-20, R19): each
//! remembers where it was left. Going from an open program to the catalog and back to
//! „Studium" returns to that program, not to the overview.
//!
//! - From another area, a tab leads to where its area was left.
//! - On a page inside the area (a module, a program), the area's own tab leads up to the area's
//!   list as it was left (filters, position).
//! - On the list itself, the tab is the plain link to the list without filters.
//!
//! The same memory answers two more questions: where „Zurück" on a module's or a program's page
//! leads (the area's list; through the browser history if that is where the visitor came from,
//! so the history does not grow), and which row a list shows when the visitor comes back to it.
//!
//! All of this is personal state: it lives in the browser (`sessionStorage`, so a reload keeps
//! it), never in the URL and never in server HTML (R9, R13). On the server and without
//! JavaScript every tab is the plain link to its area.

use catalog::url;
use leptos::prelude::*;
use leptos_router::hooks::use_location;

use crate::nav;

const STORAGE_KEY: &str = "betula.tabs";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    Home,
    Catalog,
    Programs,
    /// „Merkliste": the visitor's marked modules. It is one page, and a module opened from it is
    /// shown on it (`crate::local`), beside the list or in full.
    Bookmarks,
    /// „Studienplan": the visitor's plan. One page as well; a module opened from it stands beside
    /// the plan, and fills it after „Vollbild" (`crate::studyplan::PlanAddress`).
    Studyplan,
}

impl Area {
    /// The area shows the modules it lists in place (`crate::local`): beside its page and in full,
    /// without leaving the area. What is opened there stays the area's: a module's own page
    /// reached from it (a link on the module's page) does not become what the catalog remembers,
    /// and its „Zurück" leads back into the area (`Tabs::came_from`).
    pub fn shows_in_place(self) -> bool {
        matches!(self, Area::Programs | Area::Bookmarks | Area::Studyplan)
    }

    pub fn of(path: &str) -> Self {
        if path.starts_with(url::PROGRAMS) {
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

    /// The list of the area: where its tab leads when nothing is remembered.
    pub fn root(self) -> &'static str {
        match self {
            Area::Home => url::HOME,
            Area::Catalog => url::CATALOG,
            Area::Programs => url::PROGRAMS,
            Area::Bookmarks => url::BOOKMARKS,
            Area::Studyplan => url::STUDYPLAN,
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
    programs_list: Option<String>,
    /// The marked modules as they were left: their order, the module open beside them.
    bookmarks: Option<String>,
    /// The Studienplan as it was left: its semester and view, the module beside it.
    studyplan: Option<String>,
}

impl Memory {
    fn visit(&mut self, location: String) {
        if location == self.current {
            return;
        }
        let path = path_of(&location).to_string();
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
        if path == Area::of(&path).root() {
            *list = Some(location.clone());
        }
        *last = Some(location.clone());
        self.previous = std::mem::replace(&mut self.current, location);
    }

    fn stored(&self) -> String {
        [&self.catalog, &self.catalog_list, &self.programs, &self.programs_list, &self.bookmarks, &self.studyplan].map(|entry| entry.clone().unwrap_or_default()).join("\n")
    }

    fn restored(stored: &str) -> Self {
        // Only paths of this site: what is stored ends up in links. What an older version stored
        // (fewer lines) reads as far as it goes.
        let mut lines = stored.lines().map(|line| Some(line.to_string()).filter(|line| line.starts_with('/') && !line.starts_with("//")));
        let mut next = || lines.next().flatten();
        Self { catalog: next(), catalog_list: next(), programs: next(), programs_list: next(), bookmarks: next(), studyplan: next(), ..Default::default() }
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
        Effect::new(move |_| {
            let now = location_of(&location.pathname.get(), &location.search.get());
            self.0.update(|memory| memory.visit(now));
            nav::session_set(STORAGE_KEY, &self.0.with_untracked(Memory::stored));
        });
    }

    /// Where the tab of `area` leads, seen from `path`.
    pub fn href(self, area: Area, path: &str) -> String {
        self.href_with_root(area, path, area.root())
    }

    /// Where the tab of `area` leads, seen from `path`, where the area has a first page of its own
    /// for this visitor (the catalog of „Mein Studiengang", A.10): `root` stands in only where
    /// nothing of the area is remembered, its first entry of the session. On the area's list the
    /// tab stays the plain link to it, a reset, and a page the visitor left the area at always
    /// wins, so a filter the visitor took away is never added back.
    pub fn href_with_root(self, area: Area, path: &str, root: &str) -> String {
        self.0.with(|memory| {
            let (last, list) = match area {
                Area::Catalog => (&memory.catalog, &memory.catalog_list),
                Area::Programs => (&memory.programs, &memory.programs_list),
                Area::Bookmarks => (&memory.bookmarks, &memory.bookmarks),
                Area::Studyplan => (&memory.studyplan, &memory.studyplan),
                Area::Home => (&None, &None),
            };
            if Area::of(path) == area && path == area.root() {
                return area.root().to_string();
            }
            let remembered = if Area::of(path) != area { last } else { list };
            remembered.clone().unwrap_or_else(|| root.to_string())
        })
    }

    /// The list of `area` as it was left: where „Zurück" on a page of the area leads.
    pub fn list(self, area: Area) -> String {
        self.0.with_untracked(|memory| match area {
            Area::Catalog => memory.catalog_list.clone(),
            Area::Programs => memory.programs_list.clone(),
            Area::Bookmarks => memory.bookmarks.clone(),
            Area::Studyplan => memory.studyplan.clone(),
            Area::Home => None,
        })
        .unwrap_or_else(|| area.root().to_string())
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
    /// before or after the memory has heard of `now`; the answer is the same either way.
    pub fn before(self, now: &str) -> String {
        self.0.with_untracked(|memory| if memory.current == now { memory.previous.clone() } else { memory.current.clone() })
    }

    /// The area the visitor was in before `now`. A module's page asks this: opened out of an area
    /// that shows its modules in place (`Area::shows_in_place`), it leads back into that area, not
    /// to the catalog's list.
    pub fn came_from(self, now: &str) -> Area {
        Area::of(path_of(&self.before(now)))
    }
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

    #[test]
    fn tabs_remember_where_their_area_was_left() {
        let mut memory = Memory::default();
        for location in ["/catalog?turnus=winter", "/catalog?turnus=winter&page=3", "/catalog/module/11112", "/programs?level=master", "/programs/master-informatik-2008/plan"] {
            memory.visit(location.to_string());
        }
        assert_eq!(memory.catalog.as_deref(), Some("/catalog/module/11112"));
        assert_eq!(memory.catalog_list.as_deref(), Some("/catalog?turnus=winter&page=3"));
        assert_eq!(memory.programs.as_deref(), Some("/programs/master-informatik-2008/plan"));
        assert_eq!(memory.programs_list.as_deref(), Some("/programs?level=master"));
        assert_eq!((memory.previous.as_str(), memory.current.as_str()), ("/programs?level=master", "/programs/master-informatik-2008/plan"));

        // A module opened out of a program leaves the catalog's memory alone.
        let mut from_program = Memory::default();
        for location in ["/catalog?turnus=winter", "/programs/master-informatik-2008/plan?open=11112", "/catalog/module/11112"] {
            from_program.visit(location.to_string());
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
            memory.visit(location.to_string());
        }
        // „Vollbild" stays among the marked modules: their tab remembers it, the catalog's does not
        // hear of it.
        assert_eq!(memory.bookmarks.as_deref(), Some("/bookmarks?sort=ects&open=11112&full=1"));
        assert_eq!((memory.catalog.as_deref(), memory.catalog_list.as_deref()), (Some("/catalog?turnus=winter"), Some("/catalog?turnus=winter")));
        // A module's own page reached from there (a successor named on the page) is theirs as well:
        // its „Zurück" leads back to them, and the catalog's tab still leads to its list.
        memory.visit("/catalog/module/11113".to_string());
        assert_eq!((Area::of(path_of(&memory.previous)), memory.catalog.as_deref()), (Area::Bookmarks, Some("/catalog?turnus=winter")));
        assert!(Area::Bookmarks.shows_in_place() && Area::Programs.shows_in_place() && !Area::Catalog.shows_in_place() && !Area::Home.shows_in_place());
        // A reload keeps it, and what an older version stored (four lines) still reads.
        assert_eq!(Memory::restored(&memory.stored()).bookmarks, memory.bookmarks);
        assert_eq!(Memory::restored("/catalog\n/catalog\n\n\n"), Memory { catalog: Some("/catalog".into()), catalog_list: Some("/catalog".into()), ..Default::default() });
    }

    #[test]
    fn the_studyplan_is_an_area_of_its_own() {
        assert_eq!((Area::of("/studyplan"), Area::Studyplan.root()), (Area::Studyplan, "/studyplan"));
        assert!(Area::Studyplan.shows_in_place());
        let mut memory = Memory::default();
        for location in ["/catalog?turnus=winter", "/studyplan?sem=2026W&view=dates", "/studyplan?sem=2026W&view=dates&open=12104&full=1"] {
            memory.visit(location.to_string());
        }
        // Its tab remembers the plan as it was left, the module filling it included; the catalog's
        // does not hear of the module.
        assert_eq!(memory.studyplan.as_deref(), Some("/studyplan?sem=2026W&view=dates&open=12104&full=1"));
        assert_eq!(memory.catalog.as_deref(), Some("/catalog?turnus=winter"));
        // A module's own page reached from there belongs to the plan: its „Zurück" leads back to it.
        memory.visit("/catalog/module/12107".to_string());
        assert_eq!((Area::of(path_of(&memory.previous)), memory.catalog.as_deref()), (Area::Studyplan, Some("/catalog?turnus=winter")));

        // The tab: from elsewhere to where the plan was left; on the plan itself the plain link.
        let tabs = Tabs(RwSignal::new(memory.clone()));
        assert_eq!(tabs.href(Area::Studyplan, "/catalog"), "/studyplan?sem=2026W&view=dates&open=12104&full=1");
        assert_eq!(tabs.href(Area::Studyplan, "/studyplan"), "/studyplan");
        assert_eq!(tabs.list(Area::Studyplan), "/studyplan?sem=2026W&view=dates&open=12104&full=1");
        assert_eq!(Tabs(RwSignal::new(Memory::default())).href(Area::Studyplan, "/catalog"), "/studyplan");

        // A reload keeps it as the sixth line, and what the version before stored (five lines) reads.
        let stored = memory.stored();
        assert_eq!(stored.lines().count(), 6);
        assert_eq!(Memory::restored(&stored).studyplan, memory.studyplan);
        let five = "/catalog\n/catalog\n/programs\n/programs\n/bookmarks?sort=ects";
        assert_eq!(
            Memory::restored(five),
            Memory { catalog: Some("/catalog".into()), catalog_list: Some("/catalog".into()), programs: Some("/programs".into()), programs_list: Some("/programs".into()), bookmarks: Some("/bookmarks?sort=ects".into()), ..Default::default() }
        );
        assert_eq!(Memory::restored("\n\n\n\n\n//evil.example/studyplan").studyplan, None);
    }

    #[test]
    fn the_catalog_of_mein_studiengang_is_only_the_first_entry() {
        let mine = "/catalog?program=bachelor-informatik-2008";
        // Nothing of the catalog remembered in this session: the tab leads to the program's
        // catalog, from another area and from a module's page reached some other way.
        let fresh = Tabs(RwSignal::new(Memory::default()));
        assert_eq!(fresh.href_with_root(Area::Catalog, "/", mine), mine);
        assert_eq!(fresh.href_with_root(Area::Catalog, "/catalog/module/12104", mine), mine);
        // On the catalog's list itself the tab is a reset to the whole catalog, as without it.
        assert_eq!(fresh.href_with_root(Area::Catalog, "/catalog", mine), "/catalog");

        // Once the catalog was visited, what it was left at wins: the student who took the
        // program's tag away is not led back to it.
        let mut memory = Memory::default();
        for location in [mine, "/catalog", "/programs"] {
            memory.visit(location.to_string());
        }
        let visited = Tabs(RwSignal::new(memory));
        assert_eq!(visited.href_with_root(Area::Catalog, "/programs", mine), "/catalog");
        assert_eq!(visited.href_with_root(Area::Catalog, "/catalog/module/12104", mine), "/catalog");
        // `href` is the same with the area's own root.
        assert_eq!(visited.href(Area::Catalog, "/programs"), visited.href_with_root(Area::Catalog, "/programs", "/catalog"));
        assert_eq!(fresh.href(Area::Catalog, "/"), "/catalog");
    }

    #[test]
    fn pages_below_a_list() {
        assert_eq!(page_below("/catalog/module/11112", "/catalog/module").as_deref(), Some("11112"));
        assert_eq!(page_below("/programs/bachelor-informatik-2008/plan?x=1", "/programs").as_deref(), Some("bachelor-informatik-2008"));
        assert_eq!(page_below("/programs?level=master", "/programs"), None);
        assert_eq!(page_below("/catalog?open=1", "/programs"), None);
    }
}
