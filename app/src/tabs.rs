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
}

impl Area {
    pub fn of(path: &str) -> Self {
        if path.starts_with(url::PROGRAMS) {
            Area::Programs
        } else if path.starts_with(url::CATALOG) {
            Area::Catalog
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
}

impl Memory {
    fn visit(&mut self, location: String) {
        if location == self.current {
            return;
        }
        let path = path_of(&location).to_string();
        let (last, list) = match Area::of(&path) {
            Area::Catalog => (&mut self.catalog, &mut self.catalog_list),
            Area::Programs => (&mut self.programs, &mut self.programs_list),
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
        [&self.catalog, &self.catalog_list, &self.programs, &self.programs_list].map(|entry| entry.clone().unwrap_or_default()).join("\n")
    }

    fn restored(stored: &str) -> Self {
        // Only paths of this site: what is stored ends up in links.
        let mut lines = stored.lines().map(|line| Some(line.to_string()).filter(|line| line.starts_with('/') && !line.starts_with("//")));
        let mut next = || lines.next().flatten();
        Self { catalog: next(), catalog_list: next(), programs: next(), programs_list: next(), ..Default::default() }
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
        self.0.with(|memory| {
            let (last, list) = match area {
                Area::Catalog => (&memory.catalog, &memory.catalog_list),
                Area::Programs => (&memory.programs, &memory.programs_list),
                Area::Home => (&None, &None),
            };
            let remembered = if Area::of(path) != area {
                last
            } else if path != area.root() {
                list
            } else {
                &None
            };
            remembered.clone().unwrap_or_else(|| area.root().to_string())
        })
    }

    /// The list of `area` as it was left: where „Zurück" on a page of the area leads.
    pub fn list(self, area: Area) -> String {
        self.0.with_untracked(|memory| match area {
            Area::Catalog => memory.catalog_list.clone(),
            Area::Programs => memory.programs_list.clone(),
            Area::Home => None,
        })
        .unwrap_or_else(|| area.root().to_string())
    }

    /// Where the visitor was before `now`. Pages ask this while they are built, which may be
    /// before or after the memory has heard of `now`; the answer is the same either way.
    pub fn before(self, now: &str) -> String {
        self.0.with_untracked(|memory| if memory.current == now { memory.previous.clone() } else { memory.current.clone() })
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
        for location in ["/catalog?turnus=winter", "/catalog?turnus=winter&page=3", "/programs?level=master", "/programs/master-informatik-2008/plan", "/catalog/module/11112"] {
            memory.visit(location.to_string());
        }
        assert_eq!(memory.catalog.as_deref(), Some("/catalog/module/11112"));
        assert_eq!(memory.catalog_list.as_deref(), Some("/catalog?turnus=winter&page=3"));
        assert_eq!(memory.programs.as_deref(), Some("/programs/master-informatik-2008/plan"));
        assert_eq!(memory.programs_list.as_deref(), Some("/programs?level=master"));
        assert_eq!((memory.previous.as_str(), memory.current.as_str()), ("/programs/master-informatik-2008/plan", "/catalog/module/11112"));

        // What a reload keeps: the areas, not the step before.
        let restored = Memory::restored(&memory.stored());
        assert_eq!((restored.catalog.clone(), restored.programs_list.clone()), (memory.catalog.clone(), memory.programs_list.clone()));
        assert_eq!(Memory::restored("javascript:alert(1)\n//evil.example/x\n/programs\n"), Memory { programs: Some("/programs".into()), ..Default::default() });
    }

    #[test]
    fn pages_below_a_list() {
        assert_eq!(page_below("/catalog/module/11112", "/catalog/module").as_deref(), Some("11112"));
        assert_eq!(page_below("/programs/bachelor-informatik-2008/plan?x=1", "/programs").as_deref(), Some("bachelor-informatik-2008"));
        assert_eq!(page_below("/programs?level=master", "/programs"), None);
        assert_eq!(page_below("/catalog?open=1", "/programs"), None);
    }
}
