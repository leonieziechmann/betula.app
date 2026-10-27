//! The module's own page (`/catalog/module/<id>`): the catalog's route, with the module's view
//! (`folia_widgets::module::ModuleFull`) and the way back to where the visitor came from. In the
//! browser app a line of its Termine opens the Termin beside the page (`?termin=<key>&on=<day>`,
//! `ModuleTermin`), on a phone a sheet over it.


use folia_calendar::day::Day;
use folia_calendar::rowkey::RowKey;
use folia_pages::ask::ModuleAsk;
use folia_routes::url::{self, ModuleHint};
use leptos::prelude::*;
use folia_design::i18n::use_params_map;

use folia_data::{use_data, PageStatus};
use crate::i18n::{self, use_location};
use crate::lookup::look_up;
use folia_stores::studyplan::PlanHint;
use folia_shell::pending::same_page;
use folia_shell::skeleton::DetailSkeleton;
use folia_shell::tabs::{self, Area, Tabs};
use folia_shell::frame::{ErrorState, Plain};
use folia_design::ui::{Icon, NotFound, Shortcut};
use folia_widgets::module::{ModuleFull, TerminBeside};
use folia_widgets::termin::{ticking_clock, TerminLinks, TerminPlace, TerminView};

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
    // A Termin of the module beside its page (`?termin=<key>&on=<day>`; folia/design/termin: „über
    // /catalog/module/11112?termin=… zum Nachschlagen"; owner, 2026-10-27): the app's alone, as
    // everything beside a page (the server's lines lead to the Termin's own page, R9). Its
    // addresses keep the page's hint.
    let kept = Memo::new(move |_| ModuleHint::parse(&location.search.get()));
    let termin = Memo::new(move |_| if APP { location.search.with(|search| url::termin_of(search)) } else { None });
    if APP {
        provide_context(TerminBeside(Callback::new(move |key: RowKey| id.with_untracked(|id| kept.with_untracked(|kept| t.path(&url::module_termin_path(id, kept, Some((key, None)))))))));
    }
    let picked = Signal::derive(move || termin.with(Option::is_some));
    let aside = ViewFn::from(move || view! { <ModuleTermin id kept asked=termin/> });

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
                view! { <ModuleFull data back_area=back.0 back_to=back.1 hint aside=aside.clone() aside_picked=picked/> }.into_any()
            }
        }
    }
}

/// A Termin of the module beside its page (`?termin=<key>&on=<day>`): the view to look it up
/// (`lookup`), measured against the visitor's plan as on its own page, on a phone a sheet over the
/// page. Its dates and groups open beside the page as well, „Zum Modul" closes it (the module is
/// the page), „Ganze Seite" (V) is the Termin's own page, „Schließen" (Esc) the page without it,
/// back a step where that was the step before.
#[component]
fn ModuleTermin(id: Memo<String>, kept: Memo<ModuleHint>, asked: Memo<Option<(RowKey, Option<Day>)>>) -> impl IntoView {
    let t = i18n::t();
    let location = use_location();
    let found = look_up(asked);
    let tabs = Tabs::expect();
    let at = move |termin: Option<(RowKey, Option<Day>)>| id.with(|id| kept.with(|kept| url::module_termin_path(id, kept, termin)));
    let close = Signal::derive(move || t.path(&at(None)));
    let back = Signal::derive(move || {
        let now = tabs::location_of(&location.pathname.get(), &location.search.get());
        tabs.is_some_and(|tabs| same_page(&tabs.before(&now), &at(None))).then_some("history")
    });
    let termin = Callback::new(move |shown: (RowKey, Option<Day>)| t.path(&at(Some(shown))));
    let module = Callback::new(move |(id, _): (String, Option<RowKey>)| t.path(&url::module_path(&id)));
    let full = Callback::new(move |(key, on): (RowKey, Option<Day>)| t.path(&url::event_path(key, on)));
    let links = TerminLinks { termin, module, close, back, adjust: Some(found.adjust()), full: Some(full) };
    let ctx = found.ctx(links, TerminPlace::Beside, ticking_clock());
    move || match found.phase.get() {
        // Its answer on its way: the panel as a module's is while it comes.
        None => view! { <DetailSkeleton aside=true module=true/> }.into_any(),
        Some(Ok(true)) => view! { <TerminView ctx/> }.into_any(),
        // An address of no Termin (an old one, one typed): says so, and closes as the view does.
        Some(_) => view! {
            <section class="panel detail aside is-module is-termin" id="preview" aria-label=t.termin.panel>
                <div class="scroll">
                    <header class="hero tm-head">
                        <div class="hero-top tm-bar">
                            <span class="tm-kind">{t.termin.not_found}</span>
                            <a class="ghost tm-close" href=move || close.get() rel="nofollow" data-action="close-detail" data-back=move || back.get() data-noscroll="" title=t.termin.close_title><Icon name="x"/>{t.common.close}<Shortcut keys="Esc"/></a>
                        </div>
                    </header>
                    <div class="dbody tm"><p class="hint">{t.termin.not_found_hint}</p></div>
                </div>
            </section>
        }
        .into_any(),
    }
}
