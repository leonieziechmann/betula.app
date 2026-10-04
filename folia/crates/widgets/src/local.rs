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
//! 2. Its page asks `folia_routes::local::filling` what fills it: the module (`ModuleInPlace`,
//!    with `back_href` for „Zurück"), or its own content, with the module beside it where one
//!    is open (`ModulePanel`, with `full_href` for „Vollbild").
//! 3. What it lists leads to the module beside the page, on a phone as well (a `Row` with
//!    `in_place`), not to the module's own page.
//! 4. `pending::change` knows its steps (`local_change` there), so that a click answers at once.
//! 5. Its `tabs::Area` says `shows_in_place`: a module's own page reached from there (a link on
//!    the module's page) is none of the catalog's business either, and leads back.
//!
//! The browser app only: the server's pages lay nothing beside themselves and fill themselves with
//! nothing else (docs/folia/frontend.md), so they leave `open` and `full` out before they get here.

use folia_pages::ask::ModuleAsk;
use leptos::prelude::*;

use folia_data::{use_data, PageStatus};
use crate::i18n;
use crate::module::ModuleFull;
use folia_shell::tabs::Area;
use folia_shell::frame::{ErrorState, Plain};
use folia_design::ui::NotFound;

/// The module filling the page of `area`: the module's whole page (`ModuleFull`, the same page as
/// at its own address), in the place of the area's page. „Zurück" leads to `back` (`back_href`),
/// a path of the app without the language's prefix: the page writes it as a link of its language.
/// A view of the module's own page is not one for search engines: `noindex`, and its address for
/// them stays the module's own.
#[component]
pub fn ModuleInPlace(id: String, area: Area, back: String) -> impl IntoView {
    let t = i18n::t();
    let status = PageStatus::capture();
    let data = use_data();
    // Follows its answer: from the data worker it comes a moment later.
    move || match data.clone().and_then(|data| data.now(&ModuleAsk { id: id.clone() })) {
        Err(error) => {
            status.for_error(&error);
            view! { <Plain><ErrorState error/></Plain> }.into_any()
        }
        Ok(None) => {
            status.set(404);
            view! { <Plain><NotFound title=t.module.not_found hint=t.module.not_found_hint/></Plain> }.into_any()
        }
        Ok(Some(data)) => view! { <ModuleFull data back_area=area back_to=Some(back.clone()) noindex=true/> }.into_any(),
    }
}
