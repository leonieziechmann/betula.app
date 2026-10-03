//! Local views (owner, 2026-09-24: „so, dass man das in jedem Tab ganz einfach implementieren kann
//! als lokale Ansicht"): an area that lists modules shows a module where it was opened — beside
//! its page, and after „Vollbild" as the module's whole page in the page's place — without leaving
//! the area. The address stays the area's (`url::LocalView`: `open=<id>`, `&full=1`), so the tab
//! stays the current one and remembers the module, the history is the area's, „Zurück" leads to
//! the page the module was opened on, and the catalog's tab is not touched. On a phone nothing
//! stands beside a page: whatever is opened is the page, with one tap and one history entry.
//!
//! The program page and the marked modules are such areas; the semester plan will be one. The
//! catalog is not: its „Vollbild" is the module's own page, which is the catalog's area anyway.
//! What an area needs:
//!
//! 1. Its address implements `url::LocalView`, reading and writing the two parameters with
//!    `url::local_from_pairs` and `url::local_pairs`.
//! 2. Its page asks `filling` what fills it: the module (`ModuleInPlace`, with `back_href` for
//!    „Zurück"), or its own content, with the module beside it where one is open
//!    (`ModulePanel`, with `full_href` for „Vollbild").
//! 3. What it lists leads to the module beside the page, on a phone as well (a `Row` with
//!    `in_place`), not to the module's own page.
//! 4. `pending::change` knows its steps (`local_change` there), so that a click answers at once.
//! 5. Its `tabs::Area` says `shows_in_place`: a module's own page reached from there (a link on
//!    the module's page) is none of the catalog's business either, and leads back.
//!
//! The browser app only: the server's pages lay nothing beside themselves and fill themselves with
//! nothing else (docs/folia/frontend.md), so they leave `open` and `full` out before they get here.

use catalog::pages;
use catalog::url::LocalView;
use leptos::prelude::*;

use crate::data::{use_source, PageStatus};
use crate::i18n;
use crate::pages::module::ModuleFull;
use crate::tabs::Area;
use crate::ui::{ErrorState, NotFound, Plain};

/// What fills the page of an area that shows its modules in place: the module it has open, where
/// that is shown in full — after „Vollbild" (`full`), and on a phone always. `None`: the area's own
/// page, with the module beside it where one is open.
pub fn filling(url: &impl LocalView, phone: bool) -> Option<String> {
    url.open().filter(|_| url.full() || phone).map(str::to_string)
}

/// Where „Vollbild" of the module beside the page leads: the same page, filled with it.
pub fn full_href(url: &impl LocalView, id: &str) -> String {
    url.with_open(Some(id)).with_full(true).path()
}

/// Where „Zurück" leads from the module that fills the page: the page with the module beside it
/// again, or on a phone, where the module was the page, the page without it.
pub fn back_href(url: &impl LocalView, phone: bool) -> String {
    if phone {
        url.with_open(None).path()
    } else {
        url.with_full(false).path()
    }
}

/// The module filling the page of `area`: the module's whole page (`ModuleFull`, the same page as
/// at its own address), in the place of the area's page. „Zurück" leads to `back` (`back_href`),
/// a path of the app without the language's prefix: the page writes it as a link of its language.
/// A view of the module's own page is not one for search engines: `noindex`, and its address for
/// them stays the module's own.
#[component]
pub fn ModuleInPlace(id: String, area: Area, back: String) -> impl IntoView {
    let t = i18n::t();
    let status = PageStatus::capture();
    match use_source().and_then(|source| source.run(|db| pages::module(db, &id))) {
        Err(error) => {
            status.for_error(&error);
            view! { <Plain><ErrorState error/></Plain> }.into_any()
        }
        Ok(None) => {
            status.set(404);
            view! { <Plain><NotFound title=t.module.not_found hint=t.module.not_found_hint/></Plain> }.into_any()
        }
        Ok(Some(data)) => view! { <ModuleFull data back_area=area back_to=Some(back) noindex=true/> }.into_any(),
    }
}

#[cfg(test)]
mod tests {
    use catalog::url::{BookmarksUrl, ProgramTab, ProgramUrl};

    use super::*;

    #[test]
    fn what_fills_the_page_and_where_it_leads() {
        let beside = BookmarksUrl::parse("sort=title&open=11101");
        let full = BookmarksUrl::parse("sort=title&open=11101&full=1");
        // On the desktop the module stands beside the list until „Vollbild"; on a phone it is the page.
        assert_eq!((filling(&beside, false), filling(&full, false)), (None, Some("11101".to_string())));
        assert_eq!((filling(&beside, true), filling(&BookmarksUrl::parse("sort=title"), true)), (Some("11101".to_string()), None));
        assert_eq!(full_href(&beside, "11101"), "/bookmarks?sort=title&open=11101&full=1");
        assert_eq!(full_href(&BookmarksUrl::parse("open=12204"), "11101"), "/bookmarks?open=11101&full=1");
        // „Zurück": the list with the module beside it again; on a phone the list without it.
        assert_eq!((back_href(&full, false), back_href(&full, true)), ("/bookmarks?sort=title&open=11101".to_string(), "/bookmarks?sort=title".to_string()));
        // The same on a program's page, where an area picked before stays.
        let program = ProgramUrl::parse("informatik", ProgramTab::Areas, "area=12&open=11101&full=1");
        assert_eq!(filling(&program, false).as_deref(), Some("11101"));
        assert_eq!(back_href(&program, false), "/programs/informatik/areas?area=12&open=11101");
        assert_eq!(back_href(&program, true), "/programs/informatik/areas?area=12");
    }
}
