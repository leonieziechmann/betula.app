//! The program page: who the program is (the header), and three views — the study plan of the
//! examination regulations, the tree of its areas, and „Mein Plan", the visitor's plan of the
//! whole study (a placeholder so far, owner 2026-09-25; all modules of the program are the
//! catalog's, `url::program_catalog_path`).
//!
//! Two things come from the URL as plain values (R1): the view (a path segment) and, where a
//! program has several study plans — most often one per study direction — which of them is shown
//! (`?variant=<n>`). A plan is content, not a preference: its link is shareable and works without
//! JavaScript. How the plan is drawn (matrix or list) is personal instead: it lives in
//! `localStorage` and needs JavaScript (R9, R15).
//!
//! Everything that lists modules uses one table with the same columns, so the views have the
//! same rhythm: areas and semesters are rows inside it, never boxes beside it. A module
//! clicked in any of them opens in the panel on the right (`?open=<id>`, the idiom of the
//! catalog), an area clicked in „Wahlpflicht & Bereiche" shows what it holds (`?area=<id>`), and a
//! row of the plan that names no module — „Wahlpflichtmodule der Studienrichtung" — shows what the
//! plan states about it and where those modules are to be found (`?req=<n>`); with nothing picked
//! that panel holds the numbers of the view one is looking at.
//!
//! „Vollbild" of the module beside the page shows the module's whole page in place
//! (`&full=1`, a local view, `crate::local`): the address stays in the programs area, so the tab,
//! the history and „Zurück" do too. On a phone nothing stands beside a page: whatever is picked —
//! a module, an area, a row of the plan — is the page, opened with one tap and one history entry,
//! and „Zurück" leads to what it was picked from.

use std::collections::{BTreeSet, HashMap};

use catalog::filter::ProgramScope;
use catalog::labels::{Code, ModuleKind, OfferStatus, TurnusSeason};
use catalog::pages::{self, CatalogArea, ProgramData};
use catalog::plan;
use catalog::rows::{Program, ProgramModule};
use catalog::rows_detail::{AreaPlacement, Plan, PlanEntry, PlanTotal};
use catalog::url::{self, CatalogUrl, LocalView, PlanView, ProgramTab, ProgramUrl, StudyplanUrl};
use catalog::variants::{self, plan_variants, Choice, PlanVariant};
use catalog::CatalogQuery;
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use crate::data::{use_source, PageStatus};
use crate::format;
use crate::i18n::{self, use_location, Locale, Texts};
use crate::local::{self, ModuleInPlace};
use crate::myprogram::{program_name, MineButton, ProgramPlans};
use crate::nav;
use crate::pages::catalog::phone_layout;
use crate::pages::module::ModulePanel;
use crate::pending::{Change, Pending};
use crate::seo::{self, Seo};
use crate::skeleton::DetailSkeleton;
use crate::tabs::Area;
use crate::ui::{BackLink, EmptyState, ErrorState, Frame, Icon, NotFound, OfferBadge, Shortcut};

/// The browser app (`csr`), or the server rendering the page for crawlers and for browsers
/// without JavaScript.
const APP: bool = cfg!(feature = "csr");

/// What fills the page: the program itself (with what was picked beside it, on the desktop), or
/// what was picked — the module in full, or on a phone also an area or a row of the plan.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Filling {
    Program,
    Module(String),
    Area(i64),
    Req(usize),
}

/// Where this browser remembers how it likes the study plan drawn.
const PLAN_SHAPE_KEY: &str = "betula.plan.shape";

/// The two drawings of one study plan: the matrix of the printed regulations (a row per module,
/// a column per semester) and the same plan as a list, semester after semester.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlanShape {
    Matrix,
    List,
}

impl PlanShape {
    const ALL: [Self; 2] = [Self::Matrix, Self::List];

    fn code(self) -> &'static str {
        match self {
            PlanShape::Matrix => "matrix",
            PlanShape::List => "list",
        }
    }

    fn label(self, t: &Texts) -> &'static str {
        match self {
            PlanShape::Matrix => t.program.matrix,
            PlanShape::List => t.program.list,
        }
    }

    /// What this browser chose last, the matrix where it chose nothing. The server draws the
    /// list, and only the list (owner, 2026-09-21: what a search engine or any other reader of
    /// the HTML gets is a list, the better read; server HTML is the same for everybody, R9).
    fn remembered() -> Self {
        if !APP {
            return PlanShape::List;
        }
        match nav::local_get(PLAN_SHAPE_KEY).as_deref() {
            Some("list") => PlanShape::List,
            _ => PlanShape::Matrix,
        }
    }
}

#[component]
pub fn ProgramPage() -> impl IntoView {
    let t = i18n::t();
    let params = use_params_map();
    let location = use_location();
    let slug = Memo::new(move |_| params.read().get("slug").unwrap_or_default());
    // An unknown tab segment is a wrong address, not the default tab.
    let tab = Memo::new(move |_| match params.read().get("tab") {
        None => Some(ProgramTab::default()),
        Some(segment) => ProgramTab::from_segment(&segment),
    });
    // The parts of the address: the view, the study plan it shows, and what the visitor picked
    // (a module beside the page or filling it, an area, a row of the plan). Each part re-renders
    // only what depends on it.
    // The server's page lays nothing beside itself and fills itself with nothing else: what is
    // picked (`open`, `full`, `area`, `req`) is the app's, so the server renders the program's
    // page as if it were not there, and links pages of their own instead (`module_href`,
    // `area_href`). The app turns such an address into what it names.
    let here = Memo::new(move |_| {
        let mut here = ProgramUrl::parse(&slug.get(), tab.get().unwrap_or_default(), &location.search.get());
        if !APP {
            here = ProgramUrl { open: None, full: false, area: None, req: None, ..here };
        }
        here
    });
    let variant = Memo::new(move |_| here.get().variant);
    let open = Memo::new(move |_| here.get().open);
    let area = Memo::new(move |_| here.get().area);
    let req = Memo::new(move |_| here.get().req);
    // What the rows of every table link to; what stands beside the page does not change them.
    let links = Memo::new(move |_| here.get().with_open(None));
    // The drawing the visitor chose; `drawn` is the one on the screen: on a phone always the
    // list, the matrix has no room there (owner, 2026-09-21), and there is nothing to switch.
    // On a larger screen the list as well wherever the page is too narrow for the matrix of the
    // plan on it (`room`, measured by the plan): rather the list than a matrix that scrolls
    // sideways (owner, 2026-09-21). The choice stays; with room the matrix comes back.
    let shape = RwSignal::new(PlanShape::remembered());
    let room = RwSignal::new(true);
    let source = use_source();
    let status = PageStatus::capture();
    // The program is loaded once per slug; switching tabs only re-renders.
    let data = Memo::new(move |_| {
        let slug = slug.get();
        source.clone().and_then(|source| source.run(|db| pages::program(db, &slug)))
    });

    // On the desktop a module stands beside the page until „Vollbild" lets it fill the page. On
    // a phone nothing stands beside a page: what is picked is the page, and the page is a
    // history entry of its own (one tap, one step back), never a preview and then a page.
    let phone = phone_layout();
    let drawn = Signal::derive(move || if phone.get() || !room.get() { PlanShape::List } else { shape.get() });
    let filling = Memo::new(move |_| match here.with(|here| local::filling(here, phone.get())) {
        Some(id) => Filling::Module(id),
        None if phone.get() => match (area.get(), req.get()) {
            (Some(id), _) => Filling::Area(id),
            (None, Some(row)) => Filling::Req(row),
            (None, None) => Filling::Program,
        },
        None => Filling::Program,
    });
    // A pick on its way beside the page (`pending`, a change of what stands beside it): where it
    // leads, read as the router will read it. The panel follows the click in the next frame —
    // there at once for a pick, gone at once for a close — instead of the frame after.
    let going = Pending::expect();
    let target = Memo::new(move |_| {
        let going = going.filter(|going| going.change() == Some(Change::Aside))?;
        let search = going.search_on(&going.path()?)?;
        Some(ProgramUrl::parse(&slug.get(), tab.get().unwrap_or_default(), &search))
    });
    let picked = Signal::derive(move || match target.get() {
        Some(to) => to.open.is_some() || to.area.is_some() || to.req.is_some(),
        None => open.get().is_some() || area.get().is_some() || req.get().is_some(),
    });

    move || match (data.get(), tab.get()) {
        (Err(error), _) => {
            status.for_error(&error);
            view! { <div class="page"><ErrorState error/></div> }.into_any()
        }
        (Ok(Some(data)), Some(tab)) => match filling.get() {
            // The module in full, inside the program's area: „Zurück" leads to the program — with
            // the module beside it again on the desktop, without it on a phone (and to the area
            // it was picked from, where it was).
            Filling::Module(id) => {
                let back = here.with_untracked(|here| local::back_href(here, phone.get_untracked()));
                view! { <ModuleInPlace id area=Area::Programs back/> }.into_any()
            }
            Filling::Area(_) | Filling::Req(_) => {
                let name = format!("{} ({})", data.program.name, data.program.degree());
                view! {
                    <Title text=format!("{name}: {} · BTU Cottbus-Senftenberg", if matches!(filling.get_untracked(), Filling::Area(_)) { t.program.area } else { t.program.plan })/>
                    <div class="work framed picked-page">
                        <div class="page" id="page-scroll">
                            {picked_panel(&data, variant.get_untracked(), area.get_untracked(), req.get_untracked(), links, true, t)}
                        </div>
                    </div>
                }
                .into_any()
            }
            Filling::Program => {
                let sidebar = {
                    let data = data.clone();
                    move || view! { <ProgramSidebar data=data.clone() tab shape drawn room phone links variant area req/> }
                };
                let aside = {
                    let data = data.clone();
                    move || view! { <ProgramAside data=data.clone() variant open area req target links/> }
                };
                view! {
                    <Frame title=t.program.program sidebar sidebar_first=true aside aside_picked=picked>
                        <ProgramView data tab variant shape=drawn room links open area req/>
                    </Frame>
                }
                .into_any()
            }
        },
        _ => {
            status.set(404);
            view! { <div class="page"><NotFound title=t.program.not_found_title hint=t.program.not_found_hint/></div> }.into_any()
        }
    }
}

/// What stands beside the page with no module picked — the area or the row of the plan one
/// clicked — or, as `page`, what fills the page on a phone. With nothing picked nothing stands
/// beside the page (`ui::Frame`).
fn picked_panel(data: &ProgramData, variant: usize, area: Option<i64>, req: Option<usize>, links: Memo<ProgramUrl>, page: bool, t: &'static Texts) -> AnyView {
    let plans = plan_variants(&data.plan_entries, &data.plan_totals, t.locale);
    let areas = area_groups(&data.areas);
    let known = pages::catalog_areas(&data.areas, &data.area_tree);
    let chosen = plans.get(variant.min(plans.len()).saturating_sub(1));
    let row = req.and_then(|row| chosen.and_then(|plan| plan.entries.get(row.checked_sub(1)?).cloned().map(|entry| (row, entry, plan))));
    if let Some(group) = area.and_then(|id| areas.iter().find(|group| group.id == id)) {
        let here = links.get_untracked();
        // Opened out of a row of the plan, the area says how one got there and closing it
        // returns to the row; else it is where the tree puts it, and closing it leaves the page.
        let trail = row.as_ref().map(|(row, entry, plan)| row_trail(*row, entry, plan, group, &known, &areas, &here, t)).unwrap_or_default();
        let close = match &row {
            Some((row, ..)) => here.with_req(Some(*row)).path(),
            None => here.with_area(None).path(),
        };
        let catalog = catalog_for(data, variant, Some(group.id), req, t);
        return view! { <AreaPanel group=group.snapshot() modules=data.curricular.clone() trail catalog close links page/> }.into_any();
    }
    match row {
        Some((_, entry, chosen)) => {
            let fitting = areas_for_row(&entry, chosen, &known, &areas);
            let plan = (plans.len() > 1).then(|| chosen.label.clone());
            let known: HashMap<String, ProgramModule> = data.curricular.iter().chain(data.fues.iter()).map(|m| (m.module_id.clone(), m.clone())).collect();
            let catalog = catalog_for(data, variant, None, req, t);
            let choice = chosen.choice_for(&entry);
            view! { <PlanRowPanel entry plan program=data.program.clone() fitting known catalog choice links page/> }.into_any()
        }
        None => view! {
            <section class="panel detail aside" id="preview">
                <div class="state">
                    <p class="state-title">{t.program.picked_not_found_title}</p>
                    <p>{t.program.picked_not_found_hint}</p>
                    <p><a class="button" href=t.path(&links.get_untracked().with_area(None).path()) data-action="close-detail">{t.program.to_program}</a></p>
                </div>
            </section>
        }
        .into_any(),
    }
}

/// How one got to an area opened out of a row of the plan: the row, then the areas from the one
/// its name points at down to this one, without it — „Anwendungsfach / Mathematik" is the row and
/// the area. Each step leads back to where it was: the row to its panel, an area to its own with
/// the row kept. The paths are the app's, without the language's prefix.
#[allow(clippy::too_many_arguments)]
fn row_trail(row: usize, entry: &PlanEntry, plan: &PlanVariant, group: &AreaGroup, known: &[CatalogArea], areas: &[AreaGroup], here: &ProgramUrl, t: &Texts) -> Vec<(String, String)> {
    let mut steps = vec![(entry.module_name.clone(), here.with_req(Some(row)).path())];
    let under = |above: &str, path: &str| path.starts_with(&format!("{above} / "));
    let fitting = areas_for_row(entry, plan, known, areas);
    // The area the row points at that holds this one (itself, or one above it in the tree).
    if let Some(top) = fitting.iter().filter(|fit| fit.id != 0).find(|fit| fit.path == group.path || under(&fit.path, &group.path)) {
        let mut between: Vec<&AreaGroup> =
            areas.iter().filter(|g| g.id != group.id && (g.path == top.path || under(&top.path, &g.path)) && under(&g.path, &group.path)).collect();
        between.sort_by_key(|g| g.path.len());
        steps.extend(between.into_iter().map(|g| (g.label.clone(), here.with_area_keeping_req(g.id).path())));
    }
    // A row named like the area it leads to („Proseminar oder Praktikum") would read twice: it is
    // named by the plan it is a row of instead, and still leads back to itself.
    let next = steps.get(1).map_or(group.label.as_str(), |(label, _)| label.as_str());
    let twice = entry.module_name.trim().eq_ignore_ascii_case(next.trim());
    if let Some((label, _)) = steps.first_mut().filter(|_| twice) {
        *label = t.program.plan.to_string();
    }
    steps
}

/// Views of the program, how the plan is drawn, where its areas are, what is related to it and
/// what can be done with it.
#[component]
fn ProgramSidebar(
    data: ProgramData,
    tab: ProgramTab,
    shape: RwSignal<PlanShape>,
    /// The drawing on the screen, and whether the page has room for the matrix.
    drawn: Signal<PlanShape>,
    room: RwSignal<bool>,
    /// The phone's layout: the plan is a list there, without a switch.
    phone: RwSignal<bool>,
    links: Memo<ProgramUrl>,
    variant: Memo<usize>,
    area: Memo<Option<i64>>,
    req: Memo<Option<usize>>,
) -> impl IntoView {
    let t = i18n::t();
    let p = data.program.clone();
    // The catalog with what is picked on the page: first in the sidebar (owner, 2026-09-21).
    let catalog = {
        let data = data.clone();
        Memo::new(move |_| catalog_for(&data, variant.get(), area.get(), req.get(), t))
    };
    let related = data.counterpart.is_some() || !data.versions.is_empty();
    let areas = area_groups(&data.areas);
    // „Mein Studiengang" and the Studienplan take the plan shown: its Studienrichtung is what the
    // store keeps (a page that fills a core plan's row: the core, with the page as its direction),
    // and „In den Stundenplan" takes it over (A.10). Only the plan's tab shows one.
    let plans = plan_variants(&data.plan_entries, &data.plan_totals, crate::i18n::locale());
    let count = plans.len();
    let mine_plans = ProgramPlans::new(&plans, variants::supplements(&plans));
    let shown = Signal::derive(move || (tab == ProgramTab::Plan && count > 0).then(|| variant.get().saturating_sub(1)));
    let import = {
        let slug = p.slug.clone();
        move || t.path(&StudyplanUrl { view: PlanView::Overview, import: Some(slug.clone()), variant: variant.get().clamp(1, count.max(1)), ..Default::default() }.path())
    };
    // The view the app is going to is the current one at once (`pending`); its page follows.
    let going = Pending::expect();
    let slug = p.slug.clone();
    let shown_tab = Memo::new(move |_| {
        let path = going.and_then(|going| going.path());
        path.and_then(|path| ProgramTab::ALL.iter().copied().find(|v| path == url::program_path(&slug, *v) || (*v == ProgramTab::default() && path == url::program_path(&slug, *v).trim_end_matches(v.segment()).trim_end_matches('/')))).unwrap_or(tab)
    });
    let shapes = (tab == ProgramTab::Plan && !data.plan_entries.is_empty()).then_some(());
    let jumps = (tab == ProgramTab::Areas && !areas.is_empty()).then_some(());
    view! {
        <div class="fgroup actions catalog-jump">
            <a class="action" data-walk="catalog" href=move || catalog.with(|(href, _)| t.path(href))>
                <Icon name="layout-list"/>
                <span>{t.program.in_catalog}<small>{move || catalog.with(|(_, what)| what.clone())}</small></span>
                <Icon name="chevron-right"/>
            </a>
        </div>
        <nav class="toc views" aria-label=t.program.views_label>
            <p class="flabel label">{t.program.views}</p>
            {ProgramTab::ALL.iter().map(|view| {
                let view = *view;
                view! { <a data-walk="tab" href=t.path(&url::program_path(&p.slug, view)) data-noscroll="" aria-current=move || (shown_tab.get() == view).then_some("page")>{view.label(t.locale)}</a> }
            }).collect_view()}
        </nav>
        // How the plan is drawn is a personal setting: it is kept in this browser and needs
        // JavaScript, so the switch is not there without it (R15), nor on a phone, where the
        // plan is always the list. Where the page is too narrow for the matrix, the switch shows
        // the list that is drawn and says why the matrix is not there; the choice stays. The
        // server does not know the phone: its switch is hidden there by the stylesheet
        // (`.plan-shapes`), so the page does not move up when the app takes over.
        {move || shapes.filter(|_| !phone.get()).map(|_| view! {
            <div class="fgroup js-only plan-shapes">
                <p class="flabel label">{t.program.shape}</p>
                <div class="seg" role="radiogroup" aria-label=t.program.shape_label>
                    {PlanShape::ALL.iter().map(|option| {
                        let option = *option;
                        let blocked = move || option == PlanShape::Matrix && !room.get();
                        view! {
                            <button
                                type="button"
                                role="radio"
                                aria-checked=move || if drawn.get() == option { "true" } else { "false" }
                                aria-disabled=move || blocked().then_some("true")
                                on:click=move |_| {
                                    if blocked() {
                                        return;
                                    }
                                    shape.set(option);
                                    nav::local_set(PLAN_SHAPE_KEY, option.code());
                                }
                            >{option.label(t)}</button>
                        }
                    }).collect_view()}
                </div>
                {move || (!room.get()).then(|| view! {
                    <p class="hint">{t.program.too_narrow}</p>
                })}
            </div>
        })}
        {jumps.map(|_| view! {
            <nav class="toc jumps" aria-label=t.program.areas_label>
                <p class="flabel label">{t.program.areas}</p>
                {areas.iter().map(|group| {
                    let id = group.id;
                    view! {
                        <a
                            href=move || t.path(&area_href(&links.get(), id))
                            data-walk="area"
                            data-noscroll=""
                            class=format!("depth-{}", group.depth.clamp(1, 4))
                            title=group.label.clone()
                            aria-current=move || (area.get() == Some(id)).then_some("true")
                        >
                            <span class="toc-label">{group.label.clone()}</span>
                            <span class="num">{group.modules.len()}</span>
                        </a>
                    }
                }).collect_view()}
            </nav>
        })}
        {related.then(|| view! {
            <div class="fgroup actions">
                <p class="flabel label">{t.program.related}</p>
                {data.counterpart.as_ref().map(|c| view! {
                    <a class="action" href=t.path(&url::program_path(&c.slug, ProgramTab::Plan))>
                        <Icon name="graduation-cap"/>
                        <span>{(t.program.counterpart)(c.level.label(t.locale))}<small>{c.name.clone()}" · PO "{c.po_version.clone()}</small></span>
                    </a>
                })}
                {data.versions.iter().map(|v| view! {
                    <a class="action" href=t.path(&url::program_path(&v.slug, tab))>
                        <Icon name="file-check-2"/>
                        <span>"PO "{v.po_version.clone()}{v.is_latest_po.then_some(t.program.current_po)}</span>
                    </a>
                }).collect_view()}
            </div>
        })}
        <div class="fgroup actions">
            <p class="flabel label">{t.program.actions}</p>
            // Both are part of server HTML, invisible until the app runs and gone without it
            // (`.mine-toggle`), so the actions under them do not move at the takeover (R15).
            <MineButton program_id=p.id.clone() name=program_name(&p) plans=mine_plans shown/>
            {(count > 0).then(|| view! {
                <a class="action mine-toggle" href=import><Icon name="calendar-plus"/><span>{t.program.to_studyplan}</span></a>
            })}
            {(!data.documents.is_empty()).then(|| view! { <a class="action" href="#dokumente" data-action="jump"><Icon name="file-check-2"/>{t.program.documents}</a> })}
            <a class="action" href=p.source_url.clone() rel="noopener"><Icon name="arrow-up-right"/>{t.program.at_btu}</a>
        </div>
    }
}

#[component]
fn ProgramView(
    data: ProgramData,
    tab: ProgramTab,
    variant: Memo<usize>,
    shape: Signal<PlanShape>,
    room: RwSignal<bool>,
    links: Memo<ProgramUrl>,
    open: Memo<Option<String>>,
    area: Memo<Option<i64>>,
    req: Memo<Option<usize>>,
) -> impl IntoView {
    let t = i18n::t();
    let p = data.program.clone();
    let plans = plan_variants(&data.plan_entries, &data.plan_totals, t.locale);

    // Each view of the program is a page of its own; `/programs/<slug>` is the plan. So is the
    // plan of each study direction (`?variant=<n>`, the first at the plain address): a search for
    // the plan of one direction finds its page, and the modules of every direction are linked
    // from a page that is listed. A number past the last plan shows the last, and names its page.
    let chosen = variant.get_untracked().clamp(1, plans.len().max(1));
    let direction = plans.get(chosen - 1).filter(|_| tab == ProgramTab::Plan && plans.len() > 1).map(|plan| plan.label.clone());
    let address = match tab {
        ProgramTab::Plan => ProgramUrl::new(&p.slug, tab).with_variant(chosen).path(),
        _ => url::program_path(&p.slug, tab),
    };
    let view_name = match (tab, &direction) {
        (ProgramTab::Plan, Some(direction)) => (t.program.plan_of)(direction),
        (ProgramTab::Plan, None) => t.program.plan.to_string(),
        (ProgramTab::Areas, _) => t.program.areas_title.to_string(),
        (ProgramTab::MyPlan, _) => ProgramTab::MyPlan.label(t.locale).to_string(),
    };
    let description = match &direction {
        Some(direction) => (t.program.seo_description_track)(&p.name, p.degree(), &p.po_version, direction, p.curricular_modules),
        None => (t.program.seo_description)(&p.name, p.degree(), &p.po_version, p.curricular_modules),
    };
    let name = format!("{} ({})", p.name, p.degree());
    let trail = vec![
        structured(&p, &plans, tab, variant.get_untracked()),
        // The paths of the app: `breadcrumbs` writes them in the page's language.
        seo::breadcrumbs(&[
            ("Betula", url::HOME.to_string()),
            (t.app.programs, url::PROGRAMS.to_string()),
            (name.as_str(), url::program_path(&p.slug, ProgramTab::Plan)),
        ]),
    ];
    // What the catalog knows about a module of this program, for the tables of every view.
    let known: HashMap<String, ProgramModule> =
        data.curricular.iter().chain(data.fues.iter()).map(|m| (m.module_id.clone(), m.clone())).collect();

    view! {
        <Title text=format!("{name}: {view_name} · BTU Cottbus-Senftenberg")/>
        // Older examination regulations stay reachable but are not what a search should find,
        // nor is „Mein Plan".
        <Seo title=format!("{name}: {view_name}") description=description path=address card=crate::seo::program_card(&p.slug) noindex=!p.is_latest_po || !tab.indexed() data=trail/>
        <article class="page-inner" data-walk="program-page" data-walk-id=p.slug.clone()>
            <ProgramHead program=p.clone() plans=plans.clone()/>

            {match tab {
                ProgramTab::Plan => {
                    let missing = p
                        .plan_status
                        .as_ref()
                        .map(|status| status.label(t.locale).to_string())
                        .unwrap_or_else(|| t.program.no_plan.to_string());
                    let validated = data.plan.as_ref().and_then(|plan| plan.validated_at.clone());
                    let source = data.plan.as_ref().and_then(|plan| plan_source(plan, t));
                    view! { <PlanTab plans=plans.clone() validated source missing variant shape room links open req/> }.into_any()
                }
                ProgramTab::Areas => view! { <AreasTab areas=data.areas.clone() known=known.clone() links open area/> }.into_any(),
                ProgramTab::MyPlan => view! { <MyPlanTab slug=p.slug.clone()/> }.into_any(),
            }}

            {(!data.documents.is_empty()).then(|| view! {
                <section class="panel" id="dokumente">
                    <header class="block-head"><h2>{t.program.documents}</h2></header>
                    <ul class="doclist">
                        {data.documents.iter().map(|d| view! {
                            <li>
                                <a class="docrow" href=d.url.clone() rel="noopener">
                                    <Icon name="file-check-2"/>
                                    <span class="docname">{d.title.clone()}</span>
                                    <small>{d.doc_type.label(t.locale).to_string()}</small>
                                    <Icon name="arrow-up-right" class="go"/>
                                </a>
                            </li>
                        }).collect_view()}
                    </ul>
                </section>
            })}
        </article>
    }
}

/// The program as schema.org knows it: a study program of the university, with what its head says
/// in numbers (the degree, and the semesters and credits its validated plans agree on) and, on the
/// view of the plan, the modules of the plan shown (`hasCourse`, each as the address of its page,
/// where its `Course` is). One address for both views: the plan's.
fn structured(p: &Program, plans: &[PlanVariant], tab: ProgramTab, variant: usize) -> serde_json::Value {
    let address = seo::absolute(&url::program_path(&p.slug, ProgramTab::Plan));
    let mut program = serde_json::json!({
        "@type": "EducationalOccupationalProgram",
        "@id": address,
        "url": address,
        "name": format!("{} ({})", p.name, p.degree()),
        "educationalCredentialAwarded": p.degree(),
        "provider": seo::university(),
    });
    let Some(fields) = program.as_object_mut() else { return program };
    let semesters: Vec<i64> = plans.iter().map(|plan| plan.semesters).collect();
    if let Some(&length) = semesters.first().filter(|first| **first > 0 && semesters.iter().all(|n| n == *first)) {
        fields.insert("timeToComplete".into(), duration(length).into());
    }
    // A plan whose regulation prints a span of credits has no one number.
    let credits: Vec<f64> = plans.iter().filter(|plan| plan.credits_max - plan.credits <= 0.01).map(|plan| plan.credits).collect();
    if let Some(&total) = credits.first().filter(|first| **first > 0.0 && credits.len() == plans.len() && credits.iter().all(|c| (c - **first).abs() < 0.01)) {
        fields.insert("numberOfCredits".into(), serde_json::json!({ "@type": "QuantitativeValue", "value": total, "unitText": "ECTS" }));
    }
    let shown = plans.get(variant.min(plans.len()).saturating_sub(1)).filter(|_| tab == ProgramTab::Plan);
    if let Some(plan) = shown {
        let mut seen = BTreeSet::new();
        let courses: Vec<serde_json::Value> = plan
            .entries
            .iter()
            .filter_map(|entry| Some((entry.module_id.as_deref()?, entry.module_name.as_str())))
            .filter(|(id, _)| seen.insert(*id))
            .map(|(id, name)| serde_json::json!({ "@type": "Course", "@id": seo::absolute(&url::module_path(id)), "name": name }))
            .collect();
        if !courses.is_empty() {
            fields.insert("hasCourse".into(), courses.into());
        }
    }
    program
}

/// Semesters as an ISO 8601 duration: 6 → `P3Y`, 7 → `P3Y6M`, 1 → `P6M`.
fn duration(semesters: i64) -> String {
    match (semesters / 2, semesters % 2 == 1) {
        (0, _) => "P6M".to_string(),
        (years, false) => format!("P{years}Y"),
        (years, true) => format!("P{years}Y6M"),
    }
}

// ---------- the panel on the right ----------

/// What stands beside the page while the visitor picked something: the module, else the area or
/// the row of the plan. A module uses the same panel as in the catalog, so it reads the same
/// wherever it is opened.
#[component]
fn ProgramAside(
    data: ProgramData,
    variant: Memo<usize>,
    open: Memo<Option<String>>,
    area: Memo<Option<i64>>,
    req: Memo<Option<usize>>,
    /// Where a pick on its way leads (`pending`), before the router has it.
    target: Memo<Option<ProgramUrl>>,
    links: Memo<ProgramUrl>,
) -> impl IntoView {
    let t = i18n::t();
    let source = use_source();
    let module = Memo::new(move |_| match open.get() {
        None => Ok(None),
        Some(id) => source.clone().and_then(|source| source.run(|db| pages::module(db, &id))).map(Some),
    });
    // What was picked beside the page, before it is there (`pending`): its skeleton, where the
    // wait is seen or where nothing stood beside the page yet — without sliding in again where a
    // panel stood already.
    let going = Pending::expect();
    let picks = |to: &ProgramUrl| to.open.is_some() || to.area.is_some() || to.req.is_some();
    move || {
        let there = open.get().is_some() || area.get().is_some() || req.get().is_some();
        let coming = target.with(|to| to.as_ref().is_some_and(picks));
        if coming && (!there || going.is_some_and(|going| going.waits(Change::Aside))) {
            return view! { <DetailSkeleton aside=true calm=there/> }.into_any();
        }
        match module.get() {
            Ok(Some(Some(module))) => {
                let full_href = local::full_href(&links.get(), &module.module.id);
                view! { <ModulePanel data=module close_href=links.get().path() docked=true full_href=Some(full_href)/> }.into_any()
            }
            Ok(Some(None)) | Err(_) => view! {
                <section class="panel detail aside" id="preview" aria-label=t.program.preview>
                    <div class="state">
                        <p class="state-title">{t.program.module_not_found_title}</p>
                        <p>{t.program.module_not_found_hint}</p>
                        <p><a class="button" href=t.path(&links.get().path())>{t.program.close_preview}</a></p>
                    </div>
                </section>
            }
            .into_any(),
            // No module picked: the area or the row of the plan one clicked.
            Ok(None) => picked_panel(&data, variant.get(), area.get(), req.get(), links, false, t),
        }
    }
}

/// What the plan states about a row that names no module of the catalog — most of them are
/// requirements („Wahlpflichtmodule der Studienrichtung", „Wahlpflichtmodul aus der Informatik").
/// No source says which modules satisfy such a row, so nothing is claimed: the panel shows what
/// the plan itself states and where the modules that can be chosen are listed (R12).
#[component]
fn PlanRowPanel(
    entry: PlanEntry,
    /// The study direction this row belongs to, where the program has more than one plan.
    plan: Option<String>,
    program: Program,
    /// The areas the name of this row points at (derived), most-fitting first.
    fitting: Vec<AreaGroup>,
    /// What the catalog knows about the modules of this program.
    known: HashMap<String, ProgramModule>,
    /// The catalog narrowed down to what the row means, and what it lists (`catalog_for`).
    catalog: (String, String),
    /// Where the row's credits come from, when the regulation ties it to other rows.
    choice: Option<Choice>,
    links: Memo<ProgramUrl>,
    /// The panel is the page (a phone): „Zurück" instead of „Schließen".
    #[prop(optional)] page: bool,
) -> impl IntoView {
    let t = i18n::t();
    let close = links.get_untracked().with_req(None).path();
    let semester = match plan::semester_span(&entry) {
        Some(span) => format::plan_semesters(&[span], t.locale),
        None => entry.semester_span.clone().map(|span| (t.program.semester_as_written)(&span)),
    };
    let credits = plan_credits(&entry, t.locale).map(|credits| (t.format.credits)(&credits));
    // The FÜS by its kind or by its name („Fachübergreifendes Studium", „Modul aus dem FÜS-Katalog").
    let fues = catalog::plan::is_fues(&entry);
    // A row the plan states as Pflicht (or as the thesis, or as the internship) means one module,
    // not a choice: the catalog simply does not know it under this name. Only a choice gets areas.
    let one_module = catalog::plan::is_single_module(&entry);
    let area = entry.study_section.clone().or_else(|| entry.subject_area.clone());
    let mut fitting = if fues || one_module { Vec::new() } else { fitting };
    // The first is the area to show with its modules — unless several fit equally well.
    let ambiguous = fitting.first().is_some_and(AreaGroup::is_ambiguous);
    if ambiguous {
        fitting.remove(0);
    }
    let first = (!ambiguous && !fitting.is_empty()).then(|| fitting.remove(0));
    let others = fitting;
    let has_fitting = first.is_some() || !others.is_empty();
    // Where to look further: the catalog with what the row means (`catalog_for`: its areas, the
    // FÜS list, the name of a single module, else the program's electives). It is what the row
    // asks one to do, so it comes first; what can be chosen follows under it.
    let (catalog, catalog_what) = catalog;
    let (action_icon, action) = if fues {
        ("sliders-horizontal", t.program.fues_in_catalog)
    } else if one_module {
        ("search", t.program.search_catalog)
    } else if has_fitting {
        ("sliders-horizontal", t.program.fitting_in_catalog)
    } else {
        ("sliders-horizontal", t.program.electives_in_catalog)
    };
    // The head states the row once: its study direction and the area the plan prints it under
    // on one line, credits, kind and semesters as badges. An area the row's name already says
    // („Komplex Praktische Informatik" under „Komplex Praktische Informatik") is not repeated.
    let named = entry.module_name.to_lowercase();
    let area = area.filter(|area| !named.contains(&area.trim().to_lowercase()));
    let context = [plan, area].into_iter().flatten().collect::<Vec<_>>().join(" · ");
    let kind = entry.kind.as_ref().map(|kind| kind.label(t.locale).to_string()).or_else(|| entry.kind_raw.clone());
    let note_class = if one_module { "note" } else { "note quiet" };

    view! {
        <section class="panel detail aside" id="preview" aria-label=t.program.row>
            <div class="scroll">
                <header class="hero">
                    <div class="hero-top">
                        {page.then(|| view! { <BackLink area=Area::Programs to=Some(close.clone())/> })}
                        <span class="mono">{t.program.plan}</span>
                        {(!page).then(|| view! {
                            <a class="ghost" href=t.path(&close) data-action="close-detail" title=t.program.close_title>
                                <Icon name="x"/>{t.common.close}<Shortcut keys="Esc"/>
                            </a>
                        })}
                    </div>
                    <h2>{entry.module_name.clone()}</h2>
                    {(!context.is_empty()).then(|| view! { <p class="en">{context}</p> })}
                    <p class="badges">
                        {credits.map(|credits| view! { <span class="badge strong num">{credits}</span> })}
                        {kind.map(|kind| view! { <span class="badge">{kind}</span> })}
                        {semester.map(|semester| view! { <span class="badge">{semester}</span> })}
                    </p>
                </header>
                <div class="dbody">
                    <a class="btn primary row-action" href=t.path(&catalog) title=catalog_what data-walk="catalog">
                        <Icon name=action_icon/>{action}
                    </a>
                    // What can be chosen: the area the name points at with its modules, the
                    // others as links.
                    {first.map(|area| {
                        let count = area.modules.len();
                        let id = area.id;
                        let label = area.label.clone();
                        let path = area.path.clone();
                        view! {
                            <div class="section">
                                <p class="label">{t.program.probably}{label.clone()}<span>{format::modules(count as i64, t.locale)}</span></p>
                                {(!path.is_empty()).then(|| view! { <p class="hint">{path}</p> })}
                                <div class="linklist">{area_module_links(&area.modules, &known, links, t)}</div>
                                <a class="pre more-area" href=move || t.path(&area_within_href(&links.get(), id)) data-walk="area" data-noscroll="">
                                    <b>{t.program.whole_area}</b>
                                    <small>{label}</small>
                                    <Icon name="chevron-right"/>
                                </a>
                            </div>
                        }
                    })}
                    {(!others.is_empty()).then(|| view! {
                        <div class="section">
                            <p class="label">{if ambiguous { t.program.fitting_areas } else { t.program.also_possible }}<span>{others.len()}</span></p>
                            <div class="linklist">
                                {others.clone().into_iter().map(|area| {
                                    let id = area.id;
                                    view! {
                                        <a class="pre" href=move || t.path(&area_within_href(&links.get(), id)) data-walk="area" data-noscroll="">
                                            <b>{area.label.clone()}</b>
                                            <small>{format::modules(area.modules.len() as i64, t.locale)}</small>
                                            <Icon name="chevron-right"/>
                                        </a>
                                    }
                                }).collect_view()}
                            </div>
                        </div>
                    })}
                    // A choice the name points at no area of: the areas of the program are where
                    // its modules are listed.
                    {(!fues && !one_module && !has_fitting).then(|| view! {
                        <div class="section">
                            <p class="label">{t.program.program_areas}</p>
                            <div class="linklist">
                                <a class="pre" href=t.path(&url::program_path(&program.slug, ProgramTab::Areas))>
                                    <b>{ProgramTab::Areas.label(t.locale)}</b>
                                    <small>{t.program.all_areas}</small>
                                    <Icon name="chevron-right"/>
                                </a>
                            </div>
                        </div>
                    })}
                    // A row that prints a range says nothing on its own: what pins it down is
                    // the line the regulation prints over it and its neighbours.
                    {choice.map(|choice| {
                        // The regulation may print a span here as well, and then that span is
                        // everything it says: „28–32 LP", not a number we picked out of it.
                        let together = printed_credits(&choice.total, t.locale);
                        let least = format::number(choice.total.min_credits, t.locale);
                        let most = format::number(choice.total.max_credits, t.locale);
                        let label = choice.total.label.clone();
                        let span = choice_span(&choice.total, t);
                        let others = choice.with.len();
                        view! {
                            <div class="section">
                                <p class="label">{t.program.together}<span>{(t.format.credits)(&together)}</span></p>
                                <p class="hint">{(t.program.choice_hint)(others, &label, &together, &span, &least, &most)}</p>
                                <div class="linklist">
                                    {choice.with.into_iter().map(|(row, name)| view! {
                                        <a class="pre" href=t.path(&links.get_untracked().with_req(Some(row)).path()) data-noscroll="">
                                            <b>{name}</b>
                                            <Icon name="chevron-right"/>
                                        </a>
                                    }).collect_view()}
                                </div>
                            </div>
                        }
                    })}
                    // Said once, under what it explains: no source names the modules of such a
                    // row (R12). Only a module the catalog does not know is worth a warning.
                    <p class=note_class>
                        <Icon name="info"/>
                        <span>
                            {if one_module {
                                t.program.note_one_module
                            } else if fues {
                                t.program.note_fues
                            } else if ambiguous {
                                t.program.note_ambiguous
                            } else if has_fitting {
                                t.program.note_fitting
                            } else {
                                t.program.note_none
                            }}
                        </span>
                    </p>
                </div>
            </div>
        </section>
    }
}

/// The modules of an area as links that open them beside the page.
fn area_module_links(modules: &[AreaPlacement], known: &HashMap<String, ProgramModule>, links: Memo<ProgramUrl>, t: &'static Texts) -> AnyView {
    modules
        .iter()
        .map(|placement| {
            let id = placement.module_id.clone();
            let kind = placement.kind.clone().or_else(|| known.get(&placement.module_id).and_then(|m| m.kind.clone()));
            view! {
                <a class="pre" href=move || t.path(&module_href(&links.get(), &id)) data-walk="module" data-noscroll="">
                    <span class="mono">{placement.module_id.clone()}</span>
                    <b>{placement.module_title.clone()}</b>
                    <small>
                        {kind.map(|kind| format!("{} · ", kind.label(t.locale))).unwrap_or_default()}
                        {format::credits(placement.module_credits, t.locale)}
                    </small>
                    <Icon name="chevron-right"/>
                </a>
            }
        })
        .collect_view()
        .into_any()
}

/// Which areas of the program a row of the plan is about, most fitting first: what the name of
/// the row and the areas have in common, within the study direction the plan is printed for
/// (`catalog::plan::areas_for_row`; the catalog's semester lists rest on the same derivation).
/// Derived, never stated — the panel says so, and where two areas fit equally well it names both
/// instead of picking one (R12): the mark `AreaGroup::ambiguous` then stands in front of them.
/// `known` are the program's areas as the catalog crate knows them (`pages::catalog_areas`, with
/// the nodes above each), `areas` the page's groups of the same areas.
fn areas_for_row(entry: &PlanEntry, plan: &PlanVariant, known: &[CatalogArea], areas: &[AreaGroup]) -> Vec<AreaGroup> {
    let found = catalog::plan::areas_for_row(entry, &plan.full, known, &plan.entries);
    let group_of = |area: &CatalogArea| areas.iter().find(|group| group.id == area.id).cloned();
    let mut fitting: Vec<AreaGroup> = found.areas.iter().filter_map(group_of).collect();
    if found.ambiguous() {
        fitting.insert(0, AreaGroup::ambiguous());
    } else {
        // The one, and after it what also comes into question.
        fitting.extend(found.others.iter().filter_map(group_of));
    }
    fitting
}

/// What an area of the program holds, in the shape of the panel of a row of the plan: the way
/// into the catalog first, then what can be chosen — the areas under it and its modules (owner,
/// 2026-09-23: one kind of panel, whatever was clicked). Opened out of a row of the plan it says
/// how one got there („Anwendungsfach / Mathematik") and closing it returns to the row; opened in
/// „Wahlpflicht & Bereiche" it says where the tree puts it. A module picked from here keeps it, so
/// closing the module comes back to this list.
#[component]
fn AreaPanel(
    group: AreaGroup,
    modules: Vec<ProgramModule>,
    /// The steps before this area where it was opened out of a row of the plan (`row_trail`),
    /// each with where it leads back to.
    trail: Vec<(String, String)>,
    /// The catalog narrowed down to this area, and what it lists (`catalog_for`).
    catalog: (String, String),
    /// Where closing leads: the row the area was opened from, else the page.
    close: String,
    links: Memo<ProgramUrl>,
    /// The panel is the page (a phone): „Zurück" instead of „Schließen".
    #[prop(optional)] page: bool,
) -> impl IntoView {
    let t = i18n::t();
    let known: HashMap<String, ProgramModule> = modules.into_iter().map(|m| (m.module_id.clone(), m)).collect();
    let count = group.modules.len();
    let sum: f64 = group.modules.iter().filter_map(|placement| placement.module_credits).sum();
    let children = group.children.clone();
    let path = group.path.clone();
    let (catalog, catalog_what) = catalog;

    view! {
        <section class="panel detail aside" id="preview" aria-label=t.program.area>
            <div class="scroll">
                <header class="hero">
                    <div class="hero-top">
                        {page.then(|| view! { <BackLink area=Area::Programs to=Some(close.clone())/> })}
                        <span class="mono">{t.program.area}</span>
                        {(!page).then(|| view! {
                            <a class="ghost" href=t.path(&close) data-action="close-detail" title=t.program.close_title>
                                <Icon name="x"/>{t.common.close}<Shortcut keys="Esc"/>
                            </a>
                        })}
                    </div>
                    <h2>{group.label.clone()}</h2>
                    {if trail.is_empty() {
                        (!path.is_empty() && path != group.label).then(|| view! { <p class="en">{path}</p> }).into_any()
                    } else {
                        view! {
                            <p class="en trail">
                                {trail.into_iter().map(|(label, href)| view! { <a href=t.path(&href) data-noscroll="">{label}</a><span class="sep">" / "</span> }).collect_view()}
                                <span aria-current="page">{group.label.clone()}</span>
                            </p>
                        }
                        .into_any()
                    }}
                    <p class="badges">
                        <span class="badge strong num">{format::modules(count as i64, t.locale)}</span>
                        {(sum > 0.0).then(|| view! { <span class="badge num">{(t.format.credits)(&format::number(sum, t.locale))}</span> })}
                    </p>
                </header>
                <div class="dbody">
                    <a class="btn primary row-action" href=t.path(&catalog) title=catalog_what data-walk="catalog">
                        <Icon name="sliders-horizontal"/>{t.program.fitting_in_catalog}
                    </a>
                    {(!children.is_empty()).then(|| view! {
                        <div class="section">
                            <p class="label">{t.program.areas_within}<span>{children.len()}</span></p>
                            <div class="linklist">
                                {children.into_iter().map(|(id, label, modules)| view! {
                                    <a class="pre" href=move || t.path(&area_within_href(&links.get(), id)) data-walk="area" data-noscroll="">
                                        <b>{label}</b>
                                        <small>{format::modules(modules as i64, t.locale)}</small>
                                        <Icon name="chevron-right"/>
                                    </a>
                                }).collect_view()}
                            </div>
                        </div>
                    })}
                    <div class="section">
                        <p class="label">{t.program.modules_heading}<span>{count}</span></p>
                        <div class="linklist">{area_module_links(&group.modules, &known, links, t)}</div>
                    </div>
                </div>
            </div>
        </section>
    }
}

/// The last semesters of some plans share one column („5.–6."), and the regulation sums them
/// together. Those semesters have no figure of their own; this is what they share. A span whose
/// semesters the plan also sums one by one is not one of them — the finer statement says more.
fn shared_semester_totals(plan: &PlanVariant) -> Vec<(i64, i64, f64)> {
    plan.totals
        .iter()
        .filter(|total| total.is_whole_plan() && total.end_semester > total.start_semester)
        .filter(|total| (total.start_semester..=total.end_semester).all(|semester| plan.stated_for(semester, semester).is_none()))
        .map(|total| (total.start_semester, total.end_semester, total.credits))
        .collect()
}

/// The head of the page: where the visitor is, what the program is called, what it is in numbers
/// and under which regulations. The numbers stand on the title's line, right of it.
#[component]
fn ProgramHead(program: Program, plans: Vec<PlanVariant>) -> impl IntoView {
    let t = i18n::t();
    let level = url::LevelGroup::of(&program.degree_level);
    let level_link = url::ProgramsUrl { levels: vec![level], ..Default::default() }.path();
    // Length and size of the studies are what the validated plan says, not what a source states.
    let semesters = span_of(plans.iter().map(|plan| plan.semesters as f64), t.locale);
    let credits = span_of(plans.iter().flat_map(|plan| [plan.credits, plan.credits_max]), t.locale);
    view! {
        <header class="panel prog-head">
            <div class="hero-top">
                <BackLink area=Area::Programs/>
                <nav class="crumbs" aria-label=t.program.crumbs>
                    <a href=t.path(url::PROGRAMS)>{t.app.programs}</a>
                    <a href=t.path(&level_link)>{level.label(t.locale)}</a>
                </nav>
            </div>
            <div class="prog-title">
                <h1>{program.name.clone()}</h1>
                <p class="prog-facts">
                    {semesters.map(|text| view! { <span class="pfact" title=t.program.semesters_title><b>{text.clone()}</b>{(t.program.semesters_after)(&text)}</span> })}
                    {credits.map(|text| view! { <span class="pfact" title=t.program.credits_title><b>{text}</b>" "{t.common.credits_unit}</span> })}
                    <span class="pfact"><b>{program.curricular_modules}</b>{(t.program.modules_after)(program.curricular_modules)}</span>
                    <span class="pfact"><b>{program.fues_modules}</b>{(t.program.fues_modules_after)(program.fues_modules)}</span>
                </p>
            </div>
            <p class="prog-meta">
                <span class="degree">{program.degree().to_string()}</span>
                <span>{t.program.regulations}{program.po_version.clone()}</span>
                {program.study_variant.as_ref().map(|v| view! { <span>{format::variant_short(v, t.locale)}</span> })}
                {if program.is_latest_po {
                    view! { <span class="current">{t.program.current}</span> }.into_any()
                } else {
                    view! { <span class="flag">{t.program.older}</span> }.into_any()
                }}
            </p>
        </header>
    }
}

// ---------- the study plan ----------

/// What a plan comes to, as it is written: „180" or, where the regulation prints spans,
/// „116–126".
fn credits_label(plan: &PlanVariant, locale: Locale) -> String {
    match plan.credits_max - plan.credits > 0.01 {
        true => format!("{}–{}", format::number(plan.credits, locale), format::number(plan.credits_max, locale)),
        false => format::number(plan.credits, locale),
    }
}

/// „6" or „3–4": one number where all plans agree, the range where they do not. `None` where no
/// plan says anything.
fn span_of(values: impl Iterator<Item = f64>, locale: Locale) -> Option<String> {
    let values: Vec<f64> = values.filter(|value| *value > 0.0).collect();
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    match values.is_empty() {
        true => None,
        false if low == high => Some(format::number(low, locale)),
        false => Some(format!("{}–{}", format::number(low, locale), format::number(high, locale))),
    }
}

/// What a row of the plan says about its credits: a number, a range, or nothing.
fn plan_credits(entry: &PlanEntry, locale: Locale) -> Option<String> {
    match (entry.credits, entry.min_credits, entry.max_credits) {
        (Some(credits), _, _) => Some(format::number(credits, locale)),
        (None, Some(min), Some(max)) if min != max => Some(format!("{}–{}", format::number(min, locale), format::number(max, locale))),
        (None, Some(value), _) | (None, None, Some(value)) => Some(format::number(value, locale)),
        _ => None,
    }
}

/// How the plan's place in the regulation reads: „Seite 9" or „Seite 9–11", with the heading it
/// stands under where the document prints one. A plan whose pages were not recorded says nothing
/// rather than guessing.
fn plan_source(plan: &Plan, t: &Texts) -> Option<(String, Option<String>)> {
    let pages = plan.source_pages.as_deref()?.trim();
    if pages.is_empty() {
        return None;
    }
    let word = if pages.contains(['\u{2013}', ',']) { t.program.pages } else { t.program.page };
    Some((format!("{word} {pages}"), plan.source_label.as_deref().and_then(plan_heading)))
}

/// The heading worth repeating. A plan read from a table headed only „Regelstudienplan" is already
/// under that word on this page, so naming it again says nothing; a heading that tells one branch
/// of a Lesefassung from another does. A heading the PDF cut mid-parenthesis is closed, because it
/// is printed as a quotation and an open bracket would swallow the sentence.
fn plan_heading(label: &str) -> Option<String> {
    let label = label.trim();
    let plain = label.rsplit(" · ").next().unwrap_or(label).trim();
    if plain.eq_ignore_ascii_case("Regelstudienplan") || plain.eq_ignore_ascii_case("Studienplan") {
        return None;
    }
    let mut text = label.to_string();
    let open = text.matches('(').count();
    let close = text.matches(')').count();
    if open > close {
        text.push_str(&")".repeat(open - close));
    }
    Some(text)
}

#[component]
fn PlanTab(
    plans: Vec<PlanVariant>,
    validated: Option<String>,
    /// Where the plan stands in the regulation: the page a reader turns to, and the heading it
    /// stands under where the document prints one.
    source: Option<(String, Option<String>)>,
    /// What to say when there is no validated plan.
    missing: String,
    variant: Memo<usize>,
    shape: Signal<PlanShape>,
    /// Told whether the matrix of the chosen plan fits the page without scrolling sideways.
    room: RwSignal<bool>,
    links: Memo<ProgramUrl>,
    open: Memo<Option<String>>,
    req: Memo<Option<usize>>,
) -> impl IntoView {
    let t = i18n::t();
    if plans.is_empty() {
        return view! {
            <EmptyState title=missing hint=t.program.no_plan_hint/>
        }
        .into_any();
    }
    // The panel is as wide as the page and the table fills it. It is measured whenever its width
    // changes (the window, or the sidebar or the panel on the right is dragged), and whether the
    // matrix of the chosen plan fits is told to the page, which draws the list where it does not.
    // Only a change is told, so a resize does not draw the plan again, and nothing is told before
    // the panel is measured.
    let width = RwSignal::new(None::<f64>);
    Effect::new(move |_| {
        let watch = nav::watch_size(PLAN_BLOCK_ID, move || width.set(nav::width_of(PLAN_BLOCK_ID)));
        on_cleanup(move || drop(watch));
    });
    let semesters: Vec<i64> = plans.iter().map(|plan| plan.semesters).collect();
    Effect::new(move |_| {
        let Some(width) = width.get() else { return };
        let chosen = variant.get().min(semesters.len()).saturating_sub(1);
        let needs = semesters.get(chosen).map_or(0.0, |semesters| matrix_min_width(*semesters));
        // A pixel of slack: the page's width is not always a whole number.
        let fits = width + 1.0 >= needs;
        if room.get_untracked() != fits {
            room.set(fits);
        }
    });
    let chips = plans.clone();
    let body = plans.clone();
    view! {
        <section class="panel plan-block" id=PLAN_BLOCK_ID>
            <header class="block-head">
                <h2>{t.program.plan}</h2>
                <p>
                    {(t.program.taken_from)(validated.as_deref().map(|at| format::date(at, t.locale)).as_deref())}
                    {source.as_ref().map(|(pages, label)| (t.program.found_at)(pages, label.as_deref()))}
                </p>
            </header>
            {move || {
                let plans = chips.clone();
                let here = links.get();
                let now = variant.get();
                (plans.len() > 1).then(|| view! {
                    <div class="variants">
                        <p class="flabel label">{t.program.track}</p>
                        <p class="chip-links">
                            {plans.iter().enumerate().map(|(i, plan)| {
                                let number = i + 1;
                                let chosen = number == now.min(plans.len());
                                view! {
                                    <a
                                        class="chip"
                                        data-walk="plan-variant"
                                        href=t.path(&here.with_variant(number).path())
                                        title=plan.full.clone()
                                        data-noscroll=""
                                        aria-current=chosen.then_some("true")
                                        data-state=if chosen { "with" } else { "off" }
                                    >
                                        <span class="chip-label">{plan.label.clone()}</span>
                                        <span class="chip-count num">{(t.format.credits)(&credits_label(plan, t.locale))}</span>
                                    </a>
                                }
                            }).collect_view()}
                        </p>
                    </div>
                })
            }}
            {move || {
                let plans = body.clone();
                let chosen = variant.get().min(plans.len()).saturating_sub(1);
                match plans.get(chosen).cloned() {
                    None => ().into_any(),
                    Some(plan) => match shape.get() {
                        PlanShape::Matrix => view! { <PlanMatrix plan links open req/> }.into_any(),
                        PlanShape::List => view! { <PlanList plan links open req/> }.into_any(),
                    },
                }
            }}
        </section>
    }
    .into_any()
}

/// The panel of the study plan, measured for the room the matrix needs.
const PLAN_BLOCK_ID: &str = "plan";

/// How wide the matrix of a plan must at least be to be drawn without scrolling sideways: the
/// names at their minimum and every semester, without „Art", which gives way first. The numbers
/// of `.matrix` in app.css (`--name-min`, `--sem`).
fn matrix_min_width(semesters: i64) -> f64 {
    let semesters = semesters.max(1) as f64;
    200.0 + semesters * (144.0 / semesters).max(48.0)
}

/// The plan as its regulations print it: a row per module, a column per semester, the credits in
/// the cell. Modules over several semesters span their columns.
#[component]
fn PlanMatrix(plan: PlanVariant, links: Memo<ProgramUrl>, open: Memo<Option<String>>, req: Memo<Option<usize>>) -> impl IntoView {
    let t = i18n::t();
    let last = plan.semesters.max(1);
    let columns = (1..=last).collect::<Vec<i64>>();
    let width = usize::try_from(last).unwrap_or(1) + 2;
    // The foot of the table is the regulation's own line of sums where it prints one, column for
    // column and, over semesters it sums together, as one cell across them. Without such a line
    // only what the plan puts into a single semester can be added up in that semester's column.
    let shared = shared_semester_totals(&plan);
    let mut foot: Vec<(i64, Option<f64>)> = Vec::new();
    let mut n = 1;
    while n <= last {
        match shared.iter().find(|(from, to, _)| n >= *from && n <= *to) {
            Some((from, to, together)) => {
                foot.push((to - from + 1, Some(*together)));
                n = to + 1;
            }
            None => {
                foot.push((1, plan.semester_credits(n)));
                n += 1;
            }
        }
    }
    // A module over several semesters is in no single semester's sum — unless the regulation's
    // own line covers exactly those semesters.
    let spread = plan.entries.iter().any(|entry| match plan::semester_span(entry) {
        Some((from, to)) => from != to && !shared.iter().any(|(a, b, _)| from >= *a && to <= *b),
        None => true,
    });
    let stated = plan.stated;
    let differs = plan.entries.iter().any(|entry| entry.credits_differ_from_catalog);

    view! {
        <div class="table-scroll">
            // Fixed columns: every semester is exactly as wide as every other one. How many there
            // are decides how wide the table must at least be (app.css).
            <table class="ptable matrix" style=format!("--sems: {last}")>
                <colgroup>
                    <col/>
                    <col class="c-kind-col"/>
                    <col class="c-sem-col" span=last/>
                </colgroup>
                <thead>
                    <tr>
                        <th scope="col" rowspan="2" class="c-name">{t.program.module}</th>
                        <th scope="col" rowspan="2" class="c-kind">{t.program.kind}</th>
                        <th scope="colgroup" colspan=last class="c-group">{t.program.credits_per_semester}</th>
                    </tr>
                    <tr>
                        {columns.iter().map(|n| view! { <th scope="col" class="c-sem num">{*n}</th> }).collect_view()}
                    </tr>
                </thead>
                {section_groups(&plan.entries, t).into_iter().map(|(section, entries)| view! {
                    <tbody>
                        {section.map(|name| view! { <tr class="group"><th colspan=width scope="rowgroup"><span class="ginner"><span class="gname">{name}</span></span></th></tr> })}
                        {entries.into_iter().map(|(row, entry)| {
                            let span = plan::semester_span(&entry);
                            let credits = plan_credits(&entry, t.locale);
                            let row_id = entry.module_id.clone();
                            view! {
                                <tr class:open=move || is_open(&row_id, open) || req.get() == Some(row)>
                                    <th scope="row" class="c-name">{plan_module(&entry, row, links, open, t)}</th>
                                    <td class="c-kind">{kind_cell(entry.kind.clone(), t)}</td>
                                    {columns.iter().filter_map(|n| match span {
                                        Some((from, to)) if *n == from => {
                                            let class = if to > from { "lp num spans" } else { "lp num" };
                                            Some(view! {
                                                <td class=class colspan=to - from + 1 class:differs=entry.credits_differ_from_catalog
                                                    title=entry.credits_differ_from_catalog.then_some(t.program.credits_differ)>
                                                    {credits.clone().unwrap_or_else(|| "·".to_string())}
                                                </td>
                                            }.into_any())
                                        }
                                        Some((from, to)) if *n > from && *n <= to => None,
                                        Some(_) => Some(view! { <td class="empty"></td> }.into_any()),
                                        // The plan names no semester for this module.
                                        None if *n == 1 => Some(view! {
                                            <td class="lp num loose" colspan=last title=t.program.no_semester_title>
                                                {credits.clone().unwrap_or_else(|| "·".to_string())}
                                            </td>
                                        }.into_any()),
                                        None => None,
                                    }).collect_view()}
                                </tr>
                            }
                        }).collect_view()}
                    </tbody>
                }).collect_view()}
                <tfoot>
                    <tr>
                        <th scope="row" class="c-name">{t.program.total}</th>
                        <td class="c-kind"></td>
                        {foot.into_iter().map(|(over, sum)| {
                            let spans = over > 1;
                            view! { <td class="lp num" class:spans=spans colspan=over>{sum.map(|value| format::number(value, t.locale))}</td> }
                        }).collect_view()}
                    </tr>
                </tfoot>
            </table>
        </div>
        {(spread || differs || stated).then(|| view! {
            <p class="hint footnote">
                {stated.then_some(t.program.stated_note)}
                {spread.then_some(t.program.spread_note)}
                {differs.then_some(t.program.differs_note)}
            </p>
        })}
    }
}

/// The same plan as a list: semester after semester, a row per module.
#[component]
fn PlanList(plan: PlanVariant, links: Memo<ProgramUrl>, open: Memo<Option<String>>, req: Memo<Option<usize>>) -> impl IntoView {
    let t = i18n::t();
    let with_area = plan.entries.iter().any(|entry| entry.study_section.is_some() || entry.subject_area.is_some());
    let head_cols = if with_area { 3 } else { 2 };
    // Semesters the plan names, in order, with the rows they hold.
    let mut groups: Vec<((i64, i64), String, Vec<NumberedRow>)> = Vec::new();
    for (row, entry) in plan.entries.iter().cloned().enumerate().map(|(i, entry)| (i + 1, entry)) {
        let span = plan::semester_span(&entry);
        let label = match (span, &entry.semester_span) {
            (Some(span), _) => format::plan_semesters(&[span], t.locale).unwrap_or_default(),
            (None, Some(span)) => (t.program.semester_as_written)(span),
            (None, None) => t.program.no_semester.to_string(),
        };
        let key = span.unwrap_or((i64::MAX, i64::MAX));
        match groups.iter_mut().find(|(existing, ..)| *existing == key) {
            Some((.., entries)) => entries.push((row, entry)),
            None => groups.push((key, label, vec![(row, entry)])),
        }
    }
    groups.sort_by_key(|(key, ..)| *key);

    view! {
        <div class="table-scroll">
            <table class="ptable planlist">
                <colgroup>
                    <col/>
                    <col class="c-kind-col"/>
                    {with_area.then(|| view! { <col class="c-area-col"/> })}
                    <col class="c-lp-col"/>
                </colgroup>
                <thead>
                    <tr>
                        <th scope="col" class="c-name">{t.program.module}</th>
                        <th scope="col" class="c-kind">{t.program.kind}</th>
                        {with_area.then(|| view! { <th scope="col" class="c-area">{t.program.area}</th> })}
                        <th scope="col" class="c-lp num">{t.common.credits_unit}</th>
                    </tr>
                </thead>
                {groups.into_iter().map(|(key, label, entries)| {
                    // What the regulation states for these semesters, else what the rows give —
                    // which for rows with a credit span is only their lower bound.
                    let sum = plan.stated_for(key.0, key.1).map(|(low, _)| low).unwrap_or_else(|| {
                        entries.iter().map(|(_, entry)| entry.credits.or(entry.min_credits).unwrap_or(0.0)).sum()
                    });
                    let count = entries.len();
                    view! {
                        <tbody>
                            <tr class="group">
                                <th colspan=head_cols scope="rowgroup">
                                    <span class="ginner">
                                        <span class="gname">{label}</span>
                                        <span class="gcount">{(t.program.modules)(count)}</span>
                                    </span>
                                </th>
                                <td class="c-lp num">{(sum > 0.0).then(|| format::number(sum, t.locale))}</td>
                            </tr>
                            {entries.into_iter().map(|(row, entry)| {
                                let row_id = entry.module_id.clone();
                                view! {
                                    <tr class:open=move || is_open(&row_id, open) || req.get() == Some(row)>
                                        <th scope="row" class="c-name">{plan_module(&entry, row, links, open, t)}</th>
                                        <td class="c-kind">{kind_cell(entry.kind.clone(), t)}</td>
                                        {with_area.then(|| view! { <td class="c-area">{entry.study_section.clone().or(entry.subject_area.clone())}</td> })}
                                        <td class="c-lp num" class:differs=entry.credits_differ_from_catalog>
                                            {plan_credits(&entry, t.locale)}
                                        </td>
                                    </tr>
                                }
                            }).collect_view()}
                        </tbody>
                    }
                }).collect_view()}
            </table>
        </div>
    }
}

/// A row of the plan: a link to its module where the row is linked to the catalog, and a link to
/// what the plan says about it („Wahlpflichtmodule der Studienrichtung" names no module) where it
/// is not. Every row of the plan can be picked; what it opens differs.
fn plan_module(entry: &PlanEntry, row: usize, links: Memo<ProgramUrl>, open: Memo<Option<String>>, t: &'static Texts) -> AnyView {
    match &entry.module_id {
        Some(id) => module_link(id, &entry.module_name, links, open, t),
        // A row that names no module: in the app it opens beside the page (`?req=<n>`); the
        // server's page has no page for it, so there it is what the plan says, as text.
        None if !APP => view! { <span class="unstated">{entry.module_name.clone()}</span> }.into_any(),
        None => {
            let name = entry.module_name.clone();
            view! {
                <a class="unstated" href=move || t.path(&links.get().with_req(Some(row)).path()) data-walk="plan-row" data-noscroll="" title=t.program.row_title>
                    {name}
                </a>
            }
            .into_any()
        }
    }
}

/// Where a module of the program leads: in the app beside the page (`?open=<id>`, as in the
/// catalog, with „Vollbild" there); on the server's page to the module's own page. The app's path,
/// without the language's prefix.
fn module_href(links: &ProgramUrl, id: &str) -> String {
    if APP {
        links.with_open(Some(id)).path()
    } else {
        url::module_path(id)
    }
}

/// The catalog narrowed down to what is picked on the program's page, and what it lists (the
/// second line of the link): an area shown beside the page is that area; a row of the plan is
/// what `variants::row_query` makes of it — the areas its name means (the derivation of the row's
/// panel, all of them where it means several), the FÜS list for a FÜS row, the name for a single
/// module, else the program's electives; nothing picked is the program. The server's page picks
/// nothing. The path is the app's, without the language's prefix.
fn catalog_for(data: &ProgramData, variant: usize, area: Option<i64>, req: Option<usize>, t: &Texts) -> (String, String) {
    let base = ProgramScope { program_slug: data.program.slug.clone(), ..Default::default() };
    let path = |query: CatalogQuery| CatalogUrl { query, page: 1, open: None, fill: None }.path();
    let scoped = |scope: ProgramScope| path(CatalogQuery { program: Some(scope), ..Default::default() });
    let known = pages::catalog_areas(&data.areas, &data.area_tree);
    if let Some(id) = area {
        let name = known.iter().find(|area| area.id == id).map(|area| area.name().to_string()).unwrap_or_else(|| t.program.area.to_string());
        return (scoped(ProgramScope { areas: vec![id], ..base }), name);
    }
    let plans = plan_variants(&data.plan_entries, &data.plan_totals, crate::i18n::locale());
    let chosen = plans.get(variant.min(plans.len()).saturating_sub(1));
    let row = req.and_then(|row| chosen.and_then(|plan| plan.entries.get(row.checked_sub(1)?).map(|entry| (entry, plan))));
    let Some((entry, plan)) = row else {
        return (scoped(base), (t.programs.modules)(data.program.curricular_modules));
    };
    let (query, what) = variants::row_query(&data.program.slug, plan, entry, &known, crate::i18n::locale());
    (path(query), what)
}

/// Where an area leads out of a panel beside the page: to its own panel with the row of the plan
/// kept, so that panel says how one got there (`ProgramUrl::with_area_keeping_req`). Panels are
/// the app's; the server's page links the catalog instead (`area_href`).
fn area_within_href(links: &ProgramUrl, id: i64) -> String {
    if APP {
        links.with_area_keeping_req(id).path()
    } else {
        area_href(links, id)
    }
}

/// Where an area of the program leads: in the app beside the page (`?area=<id>`); on the
/// server's page to the catalog narrowed down to the area, the page that lists what it holds.
fn area_href(links: &ProgramUrl, id: i64) -> String {
    if APP {
        links.with_area(Some(id)).path()
    } else {
        CatalogUrl {
            query: CatalogQuery { program: Some(ProgramScope { program_slug: links.slug.clone(), areas: vec![id], ..Default::default() }), ..Default::default() },
            page: 1,
            open: None,
            fill: None,
        }
        .path()
    }
}

/// A module of the program, opening beside the page (`?open=<id>`, as in the catalog).
fn module_link(id: &str, title: &str, links: Memo<ProgramUrl>, open: Memo<Option<String>>, t: &'static Texts) -> AnyView {
    let id = id.to_string();
    let href = {
        let id = id.clone();
        move || t.path(&module_href(&links.get(), &id))
    };
    view! {
        <a href=href data-walk="module" data-id=id.clone() data-noscroll="" aria-current=move || is_open(&Some(id.clone()), open).then_some("true")>
            {title.to_string()}
        </a>
    }
    .into_any()
}

/// Whether this module is the one shown beside the page.
fn is_open(id: &Option<String>, open: Memo<Option<String>>) -> bool {
    match id {
        Some(id) => open.read().as_deref() == Some(id.as_str()),
        None => false,
    }
}

/// A row of the plan with its place in it (1-based, what `?req=<n>` names).
type NumberedRow = (usize, PlanEntry);
/// Rows of the plan under the heading they belong to.
type RowGroup = (Option<String>, Vec<NumberedRow>);

/// The rows of a plan in the sections it names, in the order of the document, each with its place
/// in the plan. `None` where the plan states no sections: then the table has no group rows at all.
fn section_groups(entries: &[PlanEntry], t: &Texts) -> Vec<RowGroup> {
    let numbered: Vec<NumberedRow> = entries.iter().cloned().enumerate().map(|(i, entry)| (i + 1, entry)).collect();
    let named = entries.iter().any(|entry| entry.study_section.is_some() || entry.subject_area.is_some());
    if !named {
        return vec![(None, numbered)];
    }
    let mut groups: Vec<RowGroup> = Vec::new();
    for (row, entry) in numbered {
        let name = entry.study_section.clone().or_else(|| entry.subject_area.clone()).unwrap_or_else(|| t.program.no_area.to_string());
        match groups.iter_mut().find(|(existing, _)| existing.as_deref() == Some(name.as_str())) {
            Some((_, rows)) => rows.push((row, entry)),
            None => groups.push((Some(name), vec![(row, entry)])),
        }
    }
    groups
}

// ---------- areas and modules: one table ----------

/// „Pflicht", „Wahlpflicht" … as a table cell; „–" where no source states the kind (the full
/// wording stays in the title, so nothing is invented and nothing is claimed).
fn kind_cell(kind: Option<Code<ModuleKind>>, t: &Texts) -> AnyView {
    match kind {
        Some(kind) => view! { <span class=format!("kind k-{}", kind.code())><i></i>{kind.label(t.locale).to_string()}</span> }.into_any(),
        None => view! { <span class="unknown" title=t.ui.kind_unknown>"–"</span> }.into_any(),
    }
}

/// One line of the module tables: the same columns wherever the modules of a program are listed.
struct ModuleRow {
    id: String,
    title: String,
    kind: Option<Code<ModuleKind>>,
    credits: Option<f64>,
    turnus: Option<Code<TurnusSeason>>,
    semester: Option<i64>,
    offer: Code<OfferStatus>,
}

fn module_head(t: &Texts) -> AnyView {
    view! {
        <thead>
            <tr>
                <th scope="col" class="c-id">{t.program.number}</th>
                <th scope="col" class="c-name">{t.program.module}</th>
                <th scope="col" class="c-kind">{t.program.kind}</th>
                <th scope="col" class="c-lp num">{t.common.credits_unit}</th>
                <th scope="col" class="c-turnus">{t.program.turnus}</th>
                <th scope="col" class="c-sem num">{t.program.semester_short}</th>
            </tr>
        </thead>
    }
    .into_any()
}

fn module_row(row: ModuleRow, links: Memo<ProgramUrl>, open: Memo<Option<String>>, t: &'static Texts) -> AnyView {
    let id = row.id.clone();
    view! {
        <tr class:open=move || is_open(&Some(id.clone()), open)>
            <td class="c-id">{row.id.clone()}</td>
            <th scope="row" class="c-name">
                {module_link(&row.id, &row.title, links, open, t)}
                <OfferBadge status=row.offer/>
            </th>
            <td class="c-kind">{kind_cell(row.kind, t)}</td>
            <td class="c-lp num">{row.credits.map(|value| format::number(value, t.locale))}</td>
            <td class="c-turnus">{row.turnus.as_ref().map(|season| format::turnus(Some(season), None, t.locale))}</td>
            <td class="c-sem num" class:unknown=row.semester.is_none()>
                {row.semester.map(t.format.semester_one).unwrap_or_else(|| "–".to_string())}
            </td>
        </tr>
    }
    .into_any()
}

/// An area of the program with the modules placed in it.
#[derive(Clone)]
struct AreaGroup {
    id: i64,
    label: String,
    /// The whole path of the area („Gesamtkonto Bachelor / Entwerfen").
    path: String,
    /// Where the area sits in the tree (its path without the label).
    parent: Option<String>,
    depth: i64,
    modules: Vec<AreaPlacement>,
    /// The areas one level below this one: (id, label, how many modules).
    children: Vec<(i64, String, usize)>,
}

impl AreaGroup {
    /// The group as the panel beside the page needs it (its modules and its children).
    fn snapshot(&self) -> Self {
        self.clone()
    }

    /// Not an area: the mark that several fit a row of the plan equally well, so none is shown
    /// as the one. `areas_for_row` puts it in front of them.
    fn ambiguous() -> Self {
        Self { id: 0, label: String::new(), path: String::new(), parent: None, depth: 0, modules: Vec::new(), children: Vec::new() }
    }

    fn is_ambiguous(&self) -> bool {
        self.id == 0
    }

    #[cfg(test)]
    fn with_modules(mut self, count: usize) -> Self {
        self.modules = (0..count)
            .map(|i| AreaPlacement {
                module_id: format!("{}{i}", self.id),
                module_title: String::new(),
                module_credits: None,
                area_id: self.id,
                area: self.path.clone(),
                area_label: self.label.clone(),
                depth: self.depth,
                area_ord: 0,
                kind: None,
                kind_basis: None,
                module_kind: None,
            })
            .collect();
        self
    }
}

fn area_groups(areas: &[AreaPlacement]) -> Vec<AreaGroup> {
    let mut groups: Vec<AreaGroup> = Vec::new();
    for placement in areas.iter().cloned() {
        match groups.iter_mut().find(|group| group.id == placement.area_id) {
            Some(group) => group.modules.push(placement),
            None => groups.push(AreaGroup {
                id: placement.area_id,
                label: placement.area_label.clone(),
                path: placement.area.clone(),
                // The path without the label — cut off as a whole: the label itself may read
                // „Maschinenbau / Elektrotechnik".
                parent: placement.area.strip_suffix(placement.area_label.as_str()).and_then(|above| above.strip_suffix(" / ")).filter(|above| !above.is_empty()).map(str::to_string),
                depth: placement.depth,
                modules: vec![placement],
                children: Vec::new(),
            }),
        }
    }
    // What lies under an area: the areas whose path is this one plus a step.
    let below: Vec<(String, i64, String, usize)> =
        groups.iter().filter_map(|group| group.parent.clone().map(|parent| (parent, group.id, group.label.clone(), group.modules.len()))).collect();
    for group in groups.iter_mut() {
        group.children = below.iter().filter(|(parent, ..)| *parent == group.path).map(|(_, id, label, count)| (*id, label.clone(), *count)).collect();
    }
    groups
}

/// The module tree of the program: one group of rows per area, all in one table, so every area
/// reads on the same lines.
#[component]
fn AreasTab(
    areas: Vec<AreaPlacement>,
    known: HashMap<String, ProgramModule>,
    links: Memo<ProgramUrl>,
    open: Memo<Option<String>>,
    area: Memo<Option<i64>>,
) -> impl IntoView {
    let t = i18n::t();
    let groups = area_groups(&areas);
    if groups.is_empty() {
        return view! {
            <EmptyState title=t.program.no_areas_title hint=t.program.no_areas_hint/>
        }
        .into_any();
    }

    // Picked from the sidebar, from a link or from an address: the area moves into view.
    Effect::new(move |_| {
        if let Some(id) = area.get() {
            nav::reveal_selector(&format!("#area-{id}"));
        }
    });

    view! {
        <section class="panel">
            <header class="block-head">
                <h2>{ProgramTab::Areas.label(t.locale)}" "<span class="tab-count">{groups.len()}</span></h2>
                <p>{t.program.areas_intro}</p>
            </header>
            <div class="table-scroll">
                <table class="ptable modules areas">
                    {module_head(t)}
                    {groups.into_iter().map(|group| {
                        let sum: f64 = group.modules.iter().filter_map(|m| m.module_credits).sum();
                        let count = group.modules.len();
                        let known = &known;
                        view! {
                            <tbody>
                                <tr class=format!("group depth-{}", group.depth.clamp(1, 4)) id=format!("area-{}", group.id) class:open=move || area.get() == Some(group.id)>
                                    <th colspan="6" scope="rowgroup">
                                        // The area itself is a link: it shows beside the page what it holds.
                                        <a class="ginner" data-walk="area" data-noscroll="" href=move || t.path(&area_href(&links.get(), group.id)) aria-current=move || (area.get() == Some(group.id)).then_some("true")>
                                            <span class="gname">{group.label}</span>
                                            {group.parent.map(|parent| view! { <span class="gpath">{parent}</span> })}
                                            <span class="gcount">{(t.program.modules)(count)}{(sum > 0.0).then(|| format!(" · {}", (t.format.credits)(&format::number(sum, t.locale))))}</span>
                                        </a>
                                    </th>
                                </tr>
                                {group.modules.into_iter().map(|placement| {
                                    let catalog = known.get(&placement.module_id);
                                    module_row(ModuleRow {
                                        kind: placement.kind.clone().or_else(|| catalog.and_then(|m| m.kind.clone())),
                                        credits: placement.module_credits,
                                        turnus: catalog.and_then(|m| m.turnus_season.clone()),
                                        semester: catalog.and_then(|m| m.plan_semester),
                                        offer: catalog.map(|m| m.offer_status.clone()).unwrap_or(Code::Known(OfferStatus::Active)),
                                        id: placement.module_id,
                                        title: placement.module_title,
                                    }, links, open, t)
                                }).collect_view()}
                            </tbody>
                        }
                    }).collect_view()}
                </table>
            </div>
        </section>
    }
    .into_any()
}

/// „Mein Plan": the whole study, semester by semester — to come (owner, 2026-09-25). Until then a
/// placeholder that leads on: to the Stundenplan of one semester, and to the modules of the
/// program in the catalog (where „Alle Module" went).
#[component]
fn MyPlanTab(slug: String) -> impl IntoView {
    let t = i18n::t();
    view! {
        <section class="panel my-plan">
            <header class="block-head">
                <h2>{ProgramTab::MyPlan.label(t.locale)}</h2>
                <p>{t.program.my_plan_hint}</p>
            </header>
            <div class="my-plan-links">
                <a class="action" href=t.path(url::STUDYPLAN)><Icon name="calendar-plus"/><span>{t.program.to_timetable}</span><Icon name="chevron-right"/></a>
                <a class="action" href=t.path(&url::program_catalog_path(&slug, None))><Icon name="layout-list"/><span>{t.program.all_modules}</span><Icon name="chevron-right"/></a>
            </div>
        </section>
    }
}

/// What a sum prints: a number, or the span the regulation prints in its place.
fn printed_credits(total: &PlanTotal, locale: Locale) -> String {
    match total.is_span() {
        true => format!("{}–{}", format::number(total.credits, locale), format::number(total.credits_max, locale)),
        false => format::number(total.credits, locale),
    }
}

/// The semesters a printed sum covers, as a sentence names them.
fn choice_span(total: &PlanTotal, t: &Texts) -> String {
    if total.start_semester == total.end_semester {
        (t.program.choice_one)(total.start_semester)
    } else {
        (t.program.choice_many)(total.start_semester, total.end_semester)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_areas_a_row_points_at_are_the_page_s_groups_with_the_mark_of_a_tie() {
        let area = |id: i64, label: &str, modules: usize| AreaGroup {
            id,
            label: label.to_string(),
            path: format!("Grundstudium / {label}"),
            parent: Some("Grundstudium".to_string()),
            depth: 2,
            modules: Vec::new(),
            children: Vec::new(),
        }
        .with_modules(modules);
        let areas = vec![area(1, "Informatik (MIT)", 1), area(2, "Informatik (EET)", 1), area(5, "Studienrichtungsspezifische Vertiefungsmodule (MIT)", 23)];
        let known: Vec<CatalogArea> = areas.iter().map(|group| CatalogArea::new(group.id, &group.label, &["Grundstudium"], group.modules.len(), true)).collect();
        let row = |name: &str| PlanEntry {
            ord: 1,
            module_id: None,
            module_name: name.to_string(),
            semester: Some(2),
            start_semester: None,
            end_semester: None,
            semester_span: None,
            credits: Some(6.0),
            min_credits: None,
            max_credits: None,
            kind: None,
            kind_raw: None,
            study_section: None,
            subject_area: None,
            specialization: None,
            catalog_title: None,
            credits_differ_from_catalog: false,
            source_page: None,
        };
        let full = "Regelstudienplan der Studienrichtungen MIT und EET im grundständigen Studium";
        let plan = PlanVariant {
            label: "MIT und EET".to_string(),
            full: full.to_string(),
            semesters: 6,
            credits: 180.0,
            credits_max: 180.0,
            stated: false,
            entries: Vec::new(),
            totals: Vec::new(),
        };
        let fitting = areas_for_row(&row("Wahlpflichtmodul aus der Informatik"), &plan, &known, &areas);
        assert!(fitting.first().is_some_and(AreaGroup::is_ambiguous), "two directions fit, so none is shown as the one");
        assert_eq!(fitting.iter().filter(|area| !area.is_ambiguous()).map(|area| area.id).collect::<Vec<_>>(), vec![1, 2]);
        let fitting = areas_for_row(&row("Wahlpflichtmodule der Studienrichtung"), &plan, &known, &areas);
        assert_eq!(fitting.first().map(|area| area.id), Some(5));
        assert!(areas_for_row(&row("Bachelor-Arbeit"), &plan, &known, &areas).is_empty());
    }

    // Informatik B.Sc. 2008: three rows of „10–24 LP" in the last two semesters, which the plan
    // prints as one merged column; the regulation sums the two semesters together (60). What the
    // whole plan comes to is `variants::plan_variants`' and tested there; the matrix's footer is
    // the page's.
    #[test]
    fn semesters_the_plan_sums_together_share_one_figure() {
        let row = |ord: i64, name: &str, semester: Option<i64>, span: Option<(i64, i64)>, credits: Option<f64>, range: Option<(f64, f64)>| PlanEntry {
            ord,
            module_id: None,
            module_name: name.to_string(),
            semester,
            start_semester: span.map(|(from, _)| from),
            end_semester: span.map(|(_, to)| to),
            semester_span: span.map(|(from, to)| format!("{from}-{to}")),
            credits,
            min_credits: range.map(|(min, _)| min),
            max_credits: range.map(|(_, max)| max),
            kind: None,
            kind_raw: None,
            study_section: None,
            subject_area: None,
            specialization: None,
            catalog_title: None,
            credits_differ_from_catalog: false,
            source_page: None,
        };
        let total = |ord: i64, label: &str, scope: &str, from: i64, to: i64, credits: f64, min: f64, max: f64, entries: Vec<i64>| PlanTotal {
            ord,
            label: label.to_string(),
            scope: Code::parse(scope),
            specialization: None,
            start_semester: from,
            end_semester: to,
            credits,
            credits_max: credits,
            min_credits: min,
            max_credits: max,
            is_choice: max - min > 0.01,
            entry_count: entries.len() as i64,
            entries,
        };
        let entries = vec![
            row(1, "Entwicklung von Softwaresystemen", Some(1), None, Some(8.0), None),
            row(2, "Komplex Grundlagen der Informatik", None, Some((5, 6)), None, Some((10.0, 24.0))),
            row(3, "Komplex Praktische Informatik", None, Some((5, 6)), None, Some((10.0, 24.0))),
            row(4, "Komplex Angewandte und Technische Informatik", None, Some((5, 6)), None, Some((10.0, 24.0))),
            row(5, "Bachelor-Arbeit", None, Some((5, 6)), Some(12.0), None),
        ];
        let totals = vec![
            total(1, "Summe Komplexe des Fachstudiums", "section", 5, 6, 44.0, 30.0, 72.0, vec![2, 3, 4]),
            total(2, "Summe Studium", "plan", 1, 1, 8.0, 8.0, 8.0, vec![1]),
            total(3, "Summe Studium", "plan", 5, 6, 56.0, 42.0, 84.0, vec![2, 3, 4, 5]),
        ];
        // The semesters the plan sums together have one figure between them, not none each.
        let plans = plan_variants(&entries, &totals, catalog::Locale::De);
        assert_eq!(shared_semester_totals(&plans[0]), vec![(5, 6, 56.0)]);
        assert_eq!(credits_label(&plans[0], Locale::De), "64");

        // Where the plan also sums those semesters one by one, the finer statement says more.
        let mut finer = totals.clone();
        finer.push(total(4, "Summe Studium", "plan", 5, 5, 26.0, 20.0, 60.0, vec![2, 3, 4]));
        finer.push(total(5, "Summe Studium", "plan", 6, 6, 30.0, 12.0, 52.0, vec![5]));
        let plans = plan_variants(&entries, &finer, catalog::Locale::De);
        assert!(shared_semester_totals(&plans[0]).is_empty(), "the span stands in for semesters that have no figure");

        // A plan whose regulation prints a span for a semester comes to a span.
        let spans = PlanVariant { credits: 116.0, credits_max: 126.0, ..plans[0].clone() };
        assert_eq!(credits_label(&spans, Locale::De), "116–126");
        let half = PlanVariant { credits: 7.5, credits_max: 7.5, ..spans };
        assert_eq!((credits_label(&half, Locale::De), credits_label(&half, Locale::En)), ("7,5".to_string(), "7.5".to_string()));
    }

    #[test]
    fn the_head_says_one_number_where_all_plans_agree() {
        assert_eq!(span_of([6.0, 6.0].into_iter(), Locale::De).as_deref(), Some("6"));
        assert_eq!(span_of([4.0, 6.0, 0.0].into_iter(), Locale::De).as_deref(), Some("4–6"));
        assert_eq!(span_of([0.0].into_iter(), Locale::De), None);
        assert_eq!(span_of([172.5, 180.0].into_iter(), Locale::En).as_deref(), Some("172.5–180"));
    }

    /// The sentences of the page say the same in each language, with the words of the language.
    #[test]
    fn the_page_says_its_sentences_in_each_language() {
        let (de, en) = (i18n::texts(Locale::De), i18n::texts(Locale::En));
        let total = PlanTotal {
            ord: 1,
            label: "Summe".to_string(),
            scope: Code::parse("plan"),
            specialization: None,
            start_semester: 5,
            end_semester: 6,
            credits: 28.0,
            credits_max: 32.0,
            min_credits: 10.0,
            max_credits: 24.0,
            is_choice: true,
            entry_count: 2,
            entries: vec![1, 2],
        };
        assert_eq!((choice_span(&total, de), choice_span(&total, en)), ("den Semestern 5 bis 6".to_string(), "semesters 5 to 6".to_string()));
        assert_eq!((printed_credits(&total, Locale::De), printed_credits(&total, Locale::En)), ("28–32".to_string(), "28–32".to_string()));
        assert_eq!(
            (de.program.choice_hint)(1, "Summe", "28–32", "den Semestern 5 bis 6", "10", "24"),
            "Diese Zeile nennt eine Spanne, keine feste Zahl. Die Prüfungsordnung weist sie mit einer weiteren Zeile zusammen aus — „Summe“: 28–32 LP in den Semestern 5 bis 6. Einzeln lassen diese Zeilen 10 bis 24 LP zu; wie die 28–32 LP auf sie aufgeteilt werden, ist die Wahl der Studierenden."
        );
        assert!((en.program.choice_hint)(2, "Total", "60", "semesters 5 to 6", "10", "24").contains("together with 2 more rows — \"Total\": 60 CP in semesters 5 to 6."));
        assert_eq!(((de.program.taken_from)(Some("19.09.2026")), (en.program.taken_from)(None)), ("Aus der Prüfungs- und Studienordnung übernommen und geprüft am 19.09.2026.".to_string(), "Taken from the examination and study regulations and checked.".to_string()));
        assert_eq!(((de.program.found_at)("Seite 9", Some("A")), (en.program.found_at)("pages 9–11", None)), (" Dort auf Seite 9, unter „A“.".to_string(), " There on pages 9–11.".to_string()));
    }
    /// A heading is only worth repeating when it says which plan was read. „Regelstudienplan" is
    /// the word the page is already under, and a heading the PDF cut mid-parenthesis is closed so
    /// the quotation does not swallow the rest of the sentence.
    #[test]
    fn the_heading_of_a_plan_is_named_only_where_it_tells_plans_apart() {
        assert_eq!(plan_heading("Regelstudienplan"), None);
        assert_eq!(plan_heading("  Studienplan "), None);
        assert_eq!(plan_heading("Dual · Regelstudienplan"), None);
        assert_eq!(
            plan_heading("Dual ausbildungsintegrierend · Regelstudienplan B.Sc. (180 LP"),
            Some("Dual ausbildungsintegrierend · Regelstudienplan B.Sc. (180 LP)".to_string())
        );
        assert_eq!(
            plan_heading("Regelstudienplan – praxisorientiert (240 LP)"),
            Some("Regelstudienplan – praxisorientiert (240 LP)".to_string())
        );
    }
}
