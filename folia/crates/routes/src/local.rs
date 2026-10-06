//! What an area that shows its modules in place (the browser app's local views: the program page,
//! the marked modules, the Studienplan) shows and where its links lead, from its address alone
//! (`url::LocalView`). The views themselves are the app's (`folia_app::local`).

use crate::url::LocalView;

/// What fills the page of an area that shows its modules in place: the module it has open, where
/// that is shown in full, after „Vollbild" (`full`). `None`: the area's own page, with the module
/// beside it where one is open — on a phone a sheet over it (owner, 2026-10-06: „wenn man wie bei
/// der Übersicht nach bereichen so ein menu bekommt, dass sich dann von unten öffnet").
pub fn filling(url: &impl LocalView) -> Option<String> {
    url.open().filter(|_| url.full()).map(str::to_string)
}

/// Where „Vollbild" of the module beside the page leads: the same page, filled with it.
pub fn full_href(url: &impl LocalView, id: &str) -> String {
    url.with_open(Some(id)).with_full(true).path()
}

/// Where „Zurück" leads from the module that fills the page: the page with the module beside it
/// again (on a phone its sheet over the page).
pub fn back_href(url: &impl LocalView) -> String {
    url.with_full(false).path()
}

#[cfg(test)]
mod tests {
    use crate::url::{BookmarksUrl, ProgramTab, ProgramUrl};

    use super::*;

    #[test]
    fn what_fills_the_page_and_where_it_leads() {
        let beside = BookmarksUrl::parse("sort=title&open=11101");
        let full = BookmarksUrl::parse("sort=title&open=11101&full=1");
        // The module stands beside the list (on a phone a sheet over it) until „Vollbild".
        assert_eq!((filling(&beside), filling(&full)), (None, Some("11101".to_string())));
        assert_eq!(filling(&BookmarksUrl::parse("sort=title")), None);
        assert_eq!(full_href(&beside, "11101"), "/bookmarks?sort=title&open=11101&full=1");
        assert_eq!(full_href(&BookmarksUrl::parse("open=12204"), "11101"), "/bookmarks?open=11101&full=1");
        // „Zurück": the list with the module beside it again.
        assert_eq!(back_href(&full), "/bookmarks?sort=title&open=11101");
        // The same on a program's page, where an area picked before stays.
        let program = ProgramUrl::parse("informatik", ProgramTab::Areas, "area=12&open=11101&full=1");
        assert_eq!(filling(&program).as_deref(), Some("11101"));
        assert_eq!(back_href(&program), "/programs/informatik/areas?area=12&open=11101");
    }
}
