//! The module's own page (`/catalog/module/<id>`): the catalog's route, with the module's view
//! (`folia_widgets::module::ModuleFull`) and the way back to where the visitor came from.


use folia_pages::ask::ModuleAsk;
use folia_routes::url::{self, ModuleHint};
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use folia_data::{use_data, PageStatus};
use crate::i18n::{self, use_location};
use folia_stores::studyplan::PlanHint;
use folia_shell::tabs::{self, Area, Tabs};
use folia_shell::frame::{ErrorState, Plain};
use folia_design::ui::NotFound;
use folia_widgets::module::ModuleFull;

/// The browser app (`csr`): only there is a plan to meet.
const APP: bool = cfg!(feature = "csr");

/// Where „Zurück" leads from a module's page, as an area and (where one page answers it) that
/// page: the page it was opened on in an area that shows its modules in place (a program, the
/// marked modules: a link on the module shown there), else the catalog's list as it was left.
/// The step the visitor took decides — except where this page is what the catalog was left at:
/// the visitor came back to it (the catalog's tab, Back), and „Zurück" leads up to the catalog's
/// list, not across to where the visitor was in between. A reload forgets the step, and then the
/// memory of the programs answers, because a program page names the module it has open
/// (`open=<id>`).
fn back_to(id: &str) -> (Area, Option<String>) {
    let location = use_location();
    let Some(tabs) = Tabs::expect() else { return (Area::Catalog, None) };
    let now = tabs::location_of(&location.pathname.get_untracked(), &location.search.get_untracked());
    let before = tabs.before(&now);
    if tabs.left(Area::Catalog).as_deref() == Some(now.as_str()) {
        return (Area::Catalog, None);
    }
    let came_from = tabs.came_from(&now);
    if came_from.shows_in_place() {
        return (came_from, Some(before));
    }
    match tabs.left(Area::Programs).filter(|left| before.is_empty() && shows_module(left, id)) {
        Some(program) => (Area::Programs, Some(program)),
        None => (Area::Catalog, None),
    }
}

/// Does this address name a program's page with this module open beside it?
fn shows_module(location: &str, id: &str) -> bool {
    let query = location.split_once('?').map(|(_, query)| query).unwrap_or_default();
    tabs::page_below(location, url::PROGRAMS).is_some()
        && url::parse_pairs(query).iter().any(|(key, value)| key == "open" && value == id)
}

/// The module's own page (`/catalog/module/<id>`): sidebar, and the module on the rest of the screen.
#[component]
pub fn ModulePage() -> impl IntoView {
    let t = i18n::t();
    let params = use_params_map();
    let id = Memo::new(move |_| params.read().get("id").unwrap_or_default());
    let source = use_data();
    let status = PageStatus::capture();
    // Where „Einplanen" plans to when the finder sent the visitor here (`?plan=…&fill=…`, a
    // phone's way from the catalog): the browser app's alone, like the plan (the server keys the
    // page by its path and renders the button as for everybody).
    let location = use_location();
    let hint = Memo::new(move |_| if APP { PlanHint::of(&ModuleHint::parse(&location.search.get())) } else { None });

    move || {
        let id = id.get();
        match source.clone().and_then(|source| source.now(&ModuleAsk { id: id.clone() })) {
            Err(error) => {
                status.for_error(&error);
                view! { <Plain><ErrorState error/></Plain> }.into_any()
            }
            Ok(None) => {
                status.set(404);
                view! { <Plain><NotFound title=t.module.not_found hint=t.module.not_found_hint/></Plain> }.into_any()
            }
            Ok(Some(data)) => {
                // „Zurück" leads where the visitor came from: the program whose page had this
                // module open beside it (its „Vollbild"), else the catalog's list as it was left.
                let back = back_to(&id);
                view! { <ModuleFull data back_area=back.0 back_to=back.1 hint/> }.into_any()
            }
        }
    }
}
