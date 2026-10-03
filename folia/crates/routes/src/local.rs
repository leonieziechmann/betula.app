//! What an area that shows its modules in place (the browser app's local views: the program page,
//! the marked modules, the Studienplan) shows and where its links lead, from its address alone
//! (`url::LocalView`). The views themselves are the app's (`folia_app::local`).

use crate::url::LocalView;

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

#[cfg(test)]
mod tests {
    use crate::url::{BookmarksUrl, ProgramTab, ProgramUrl};

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
