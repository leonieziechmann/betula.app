//! The program page: who the program is (the header), and three views of its modules — the
//! study plan of the examination regulations, the tree of its areas, and all its modules.
//!
//! Two things come from the URL as plain values (R1): the view (a path segment) and, where a
//! program has several study plans — most often one per study direction — which of them is shown
//! (`?variant=<n>`). A plan is content, not a preference: its link is shareable and works without
//! JavaScript. How the plan is drawn (matrix or list) is personal instead: it lives in
//! `localStorage` and needs JavaScript (R9, R15).
//!
//! Everything that lists modules uses one table with the same columns, so the three views have
//! the same rhythm: areas and semesters are rows inside it, never boxes beside it. A module
//! clicked in any of them opens in the panel on the right (`?open=<id>`, the idiom of the
//! catalog), an area clicked in „Wahlpflicht & Bereiche" shows what it holds (`?area=<id>`), and a
//! row of the plan that names no module — „Wahlpflichtmodule der Studienrichtung" — shows what the
//! plan states about it and where those modules are to be found (`?req=<n>`); with nothing picked
//! that panel holds the numbers of the view one is looking at.
//!
//! „Vollbild" of the module beside the page shows the module's whole page in place
//! (`&full=1`): the address stays in the programs area, so the tab, the history and „Zurück" do
//! too. On a phone nothing stands beside a page: whatever is picked — a module, an area, a row of
//! the plan — is the page, opened with one tap and one history entry, and „Zurück" leads to what
//! it was picked from.

use std::collections::HashMap;

use catalog::filter::{KindFilter, ProgramRelation, ProgramScope};
use catalog::labels::{Code, ModuleKind, OfferStatus, TurnusSeason};
use catalog::pages::{self, CatalogArea, ProgramData};
use catalog::rows::{Program, ProgramModule};
use catalog::rows_detail::{AreaPlacement, PlanEntry};
use catalog::url::{self, CatalogUrl, ProgramTab, ProgramUrl};
use catalog::CatalogQuery;
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::{use_location, use_params_map};

use crate::data::{use_source, PageStatus};
use crate::format;
use crate::nav;
use crate::pages::catalog::phone_layout;
use crate::pages::module::{ModuleFull, ModulePanel};
use crate::seo::{self, Seo};
use crate::tabs::Area;
use crate::ui::{BackLink, EmptyState, ErrorState, Fact, Frame, Icon, NotFound, OfferBadge, Shortcut};

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

    fn label(self) -> &'static str {
        match self {
            PlanShape::Matrix => "Matrix",
            PlanShape::List => "Liste",
        }
    }

    /// What this browser chose last. The server and a browser without JavaScript draw the
    /// matrix: server HTML is the same for everybody (R9).
    fn remembered() -> Self {
        match nav::local_get(PLAN_SHAPE_KEY).as_deref() {
            Some("list") => PlanShape::List,
            _ => PlanShape::Matrix,
        }
    }
}

#[component]
pub fn ProgramPage() -> impl IntoView {
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
    let full = Memo::new(move |_| here.get().full);
    let area = Memo::new(move |_| here.get().area);
    let req = Memo::new(move |_| here.get().req);
    // What the rows of every table link to; what stands beside the page does not change them.
    let links = Memo::new(move |_| here.get().with_open(None));
    let shape = RwSignal::new(PlanShape::remembered());
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
    let filling = Memo::new(move |_| match (open.get(), full.get(), phone.get()) {
        (Some(id), true, _) | (Some(id), false, true) => Filling::Module(id),
        (Some(_), false, false) | (None, _, false) => Filling::Program,
        (None, _, true) => match (area.get(), req.get()) {
            (Some(id), _) => Filling::Area(id),
            (None, Some(row)) => Filling::Req(row),
            (None, None) => Filling::Program,
        },
    });
    let picked = Signal::derive(move || open.get().is_some() || area.get().is_some() || req.get().is_some());

    move || match (data.get(), tab.get()) {
        (Err(error), _) => {
            status.for_error(&error);
            view! { <div class="page"><ErrorState error/></div> }.into_any()
        }
        (Ok(Some(data)), Some(tab)) => match filling.get() {
            Filling::Module(id) => view! { <ProgramModuleFull id links phone/> }.into_any(),
            Filling::Area(_) | Filling::Req(_) => {
                let name = format!("{} ({})", data.program.name, data.program.degree());
                view! {
                    <Title text=format!("{name}: {} · BTU Cottbus-Senftenberg", if matches!(filling.get_untracked(), Filling::Area(_)) { "Bereich" } else { "Regelstudienplan" })/>
                    <div class="work framed picked-page">
                        <div class="page" id="page-scroll">
                            {picked_panel(&data, tab, variant.get_untracked(), area.get_untracked(), req.get_untracked(), links, true)}
                        </div>
                    </div>
                }
                .into_any()
            }
            Filling::Program => {
                let sidebar = {
                    let data = data.clone();
                    move || view! { <ProgramSidebar data=data.clone() tab shape links area/> }
                };
                let aside = {
                    let data = data.clone();
                    move || view! { <ProgramAside data=data.clone() tab variant open area req links/> }
                };
                view! {
                    <Frame title="Studiengang" sidebar sidebar_first=true aside aside_picked=picked>
                        <ProgramView data tab variant shape links open area req/>
                    </Frame>
                }
                .into_any()
            }
        },
        _ => {
            status.set(404);
            view! { <div class="page"><NotFound title="Studiengang nicht gefunden" hint="Diesen Studiengang oder diese Ansicht gibt es nicht (mehr)."/></div> }.into_any()
        }
    }
}

/// The module in full, inside the program's area: the same page as the module's own, with
/// „Zurück" leading to the program — with the module beside it again on the desktop, without it
/// on a phone (and to the area it was picked from, where it was).
#[component]
fn ProgramModuleFull(id: String, links: Memo<ProgramUrl>, phone: RwSignal<bool>) -> impl IntoView {
    let source = use_source();
    let status = PageStatus::capture();
    let loaded = source.and_then(|source| source.run(|db| pages::module(db, &id)));
    match loaded {
        Err(error) => {
            status.for_error(&error);
            view! { <div class="page"><ErrorState error/></div> }.into_any()
        }
        Ok(None) => {
            status.set(404);
            view! { <div class="page"><NotFound title="Modul nicht gefunden" hint="Dieses Modul steht nicht (mehr) im Modulkatalog der BTU."/></div> }.into_any()
        }
        Ok(Some(data)) => {
            let back = links.with_untracked(|links| if phone.get_untracked() { links.path() } else { links.with_open(Some(&id)).path() });
            view! { <ModuleFull data back_area=Area::Programs back_to=Some(back) noindex=true/> }.into_any()
        }
    }
}

/// What stands beside the page with no module picked — the area, the row of the plan, else the
/// numbers of the view — or, as `page`, what fills the page on a phone.
fn picked_panel(data: &ProgramData, tab: ProgramTab, variant: usize, area: Option<i64>, req: Option<usize>, links: Memo<ProgramUrl>, page: bool) -> AnyView {
    let plans = plan_variants(&data.plan_entries);
    let areas = area_groups(&data.areas);
    if let Some(group) = area.and_then(|id| areas.iter().find(|group| group.id == id)) {
        return view! { <AreaPanel group=group.snapshot() modules=data.curricular.clone() links page/> }.into_any();
    }
    let chosen = plans.get(variant.min(plans.len()).saturating_sub(1));
    let row = req.and_then(|row| chosen.and_then(|plan| plan.entries.get(row - 1).cloned().map(|entry| (entry, plan.label.clone(), plans.len() > 1))));
    match row {
        Some((entry, plan, several)) => {
            let fitting = areas_for_row(&entry, &plan, &areas);
            let known: HashMap<String, ProgramModule> = data.curricular.iter().chain(data.fues.iter()).map(|m| (m.module_id.clone(), m.clone())).collect();
            view! { <PlanRowPanel entry plan=(several).then_some(plan) program=data.program.clone() fitting known links page/> }.into_any()
        }
        None if page => view! {
            <section class="panel detail aside" id="preview">
                <div class="state">
                    <p class="state-title">"Nicht gefunden"</p>
                    <p>"Diesen Bereich oder diese Zeile des Regelstudienplans gibt es nicht (mehr)."</p>
                    <p><a class="button" href=links.get_untracked().with_area(None).path()>"Zum Studiengang"</a></p>
                </div>
            </section>
        }
        .into_any(),
        None => view! { <ProgramNumbers data=data.clone() tab plans variant=Signal::derive(move || variant)/> }.into_any(),
    }
}

/// Views of the program, how the plan is drawn, where its areas are, what is related to it and
/// what can be done with it.
#[component]
fn ProgramSidebar(data: ProgramData, tab: ProgramTab, shape: RwSignal<PlanShape>, links: Memo<ProgramUrl>, area: Memo<Option<i64>>) -> impl IntoView {
    let p = data.program.clone();
    let catalog_link = CatalogUrl {
        query: CatalogQuery { program: Some(ProgramScope { program_slug: p.slug.clone(), ..Default::default() }), ..Default::default() },
        page: 1,
        open: None,
    }
    .path();
    let related = data.counterpart.is_some() || !data.versions.is_empty();
    let areas = area_groups(&data.areas);
    let shapes = (tab == ProgramTab::Plan && !data.plan_entries.is_empty()).then_some(());
    let jumps = (tab == ProgramTab::Areas && !areas.is_empty()).then_some(());
    view! {
        <nav class="toc views" aria-label="Ansichten des Studiengangs">
            <p class="flabel label">"Ansichten"</p>
            {ProgramTab::ALL.iter().map(|t| {
                let active = *t == tab;
                view! { <a data-walk="tab" href=url::program_path(&p.slug, *t) data-noscroll="" aria-current=active.then_some("page")>{t.label()}</a> }
            }).collect_view()}
        </nav>
        // How the plan is drawn is a personal setting: it is kept in this browser and needs
        // JavaScript, so the switch is not there without it (R15).
        {shapes.map(|_| view! {
            <div class="fgroup js-only">
                <p class="flabel label">"Darstellung"</p>
                <div class="seg" role="radiogroup" aria-label="Darstellung des Regelstudienplans">
                    {PlanShape::ALL.iter().map(|option| {
                        let option = *option;
                        view! {
                            <button
                                type="button"
                                role="radio"
                                aria-checked=move || if shape.get() == option { "true" } else { "false" }
                                on:click=move |_| {
                                    shape.set(option);
                                    nav::local_set(PLAN_SHAPE_KEY, option.code());
                                }
                            >{option.label()}</button>
                        }
                    }).collect_view()}
                </div>
            </div>
        })}
        {jumps.map(|_| view! {
            <nav class="toc jumps" aria-label="Bereiche des Studiengangs">
                <p class="flabel label">"Bereiche"</p>
                {areas.iter().map(|group| {
                    let id = group.id;
                    view! {
                        <a
                            href=move || area_href(&links.get(), id)
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
                <p class="flabel label">"Verwandt"</p>
                {data.counterpart.as_ref().map(|c| view! {
                    <a class="action" href=url::program_path(&c.slug, ProgramTab::Plan)>
                        <Icon name="graduation-cap"/>
                        <span>{format!("Passender {}", c.level.label())}<small>{c.name.clone()}" · PO "{c.po_version.clone()}</small></span>
                    </a>
                })}
                {data.versions.iter().map(|v| view! {
                    <a class="action" href=url::program_path(&v.slug, tab)>
                        <Icon name="file-check-2"/>
                        <span>"PO "{v.po_version.clone()}{v.is_latest_po.then_some(" (aktuell)")}</span>
                    </a>
                }).collect_view()}
            </div>
        })}
        <div class="fgroup actions">
            <p class="flabel label">"Aktionen"</p>
            <a class="action" href=catalog_link><Icon name="sliders-horizontal"/>"Module im Katalog filtern"</a>
            <span class="action soon" title="In Arbeit"><Icon name="star"/>"Als meinen Studiengang setzen"<em>"bald"</em></span>
            {(!data.documents.is_empty()).then(|| view! { <a class="action" href="#dokumente" data-action="jump"><Icon name="file-check-2"/>"Ordnungen & Dokumente"</a> })}
            <a class="action" href=p.source_url.clone() rel="noopener"><Icon name="arrow-up-right"/>"Im Verzeichnis der BTU"</a>
        </div>
    }
}

#[component]
fn ProgramView(
    data: ProgramData,
    tab: ProgramTab,
    variant: Memo<usize>,
    shape: RwSignal<PlanShape>,
    links: Memo<ProgramUrl>,
    open: Memo<Option<String>>,
    area: Memo<Option<i64>>,
    req: Memo<Option<usize>>,
) -> impl IntoView {
    let p = data.program.clone();
    let plans = plan_variants(&data.plan_entries);
    let description = format!(
        "{} ({}, PO {}) an der BTU Cottbus-Senftenberg: {} Module, Regelstudienplan, Wahlpflichtbereiche und Ordnungen.",
        p.name,
        p.degree(),
        p.po_version,
        p.curricular_modules
    );

    // Each view of the program is a page of its own; `/programs/<slug>` is the plan.
    let view_name = match tab {
        ProgramTab::Plan => "Regelstudienplan",
        ProgramTab::Areas => "Wahlpflicht und Bereiche",
        ProgramTab::Modules => "Alle Module",
    };
    let name = format!("{} ({})", p.name, p.degree());
    let trail = vec![seo::breadcrumbs(&[
        ("Betula", url::HOME.to_string()),
        ("Studiengänge", url::PROGRAMS.to_string()),
        (name.as_str(), url::program_path(&p.slug, ProgramTab::Plan)),
    ])];
    // What the catalog knows about a module of this program, for the tables of every view.
    let known: HashMap<String, ProgramModule> =
        data.curricular.iter().chain(data.fues.iter()).map(|m| (m.module_id.clone(), m.clone())).collect();

    view! {
        <Title text=format!("{name}: {view_name} · BTU Cottbus-Senftenberg")/>
        // Older examination regulations stay reachable but are not what a search should find.
        // A chosen study plan is a facet of the same page, so the address stays the plain one.
        <Seo title=format!("{name}: {view_name}") description=description path=url::program_path(&p.slug, tab) card=crate::seo::program_card(&p.slug) noindex=!p.is_latest_po data=trail/>
        <article class="page-inner" data-walk="program-page" data-walk-id=p.slug.clone()>
            <ProgramHead program=p.clone() plans=plans.clone()/>

            {match tab {
                ProgramTab::Plan => {
                    let missing = p
                        .plan_status
                        .as_ref()
                        .map(|status| status.label().to_string())
                        .unwrap_or_else(|| "Für diesen Studiengang liegt kein geprüfter Regelstudienplan vor".to_string());
                    let validated = data.plan.as_ref().and_then(|plan| plan.validated_at.clone());
                    view! { <PlanTab plans=plans.clone() validated missing variant shape links open req/> }.into_any()
                }
                ProgramTab::Areas => view! { <AreasTab areas=data.areas.clone() known=known.clone() links open area/> }.into_any(),
                ProgramTab::Modules => view! { <ModulesTab curricular=data.curricular.clone() fues=data.fues.clone() links open/> }.into_any(),
            }}

            {(!data.documents.is_empty()).then(|| view! {
                <section class="panel" id="dokumente">
                    <header class="block-head"><h2>"Ordnungen & Dokumente"</h2></header>
                    <ul class="doclist">
                        {data.documents.iter().map(|d| view! {
                            <li>
                                <a class="docrow" href=d.url.clone() rel="noopener">
                                    <Icon name="file-check-2"/>
                                    <span class="docname">{d.title.clone()}</span>
                                    <small>{d.doc_type.label().to_string()}</small>
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

// ---------- the panel on the right ----------

/// What stands beside the page: the module the visitor picked, or — with none picked — the
/// numbers of the view they are looking at. A module uses the same panel as in the catalog, so it
/// reads the same wherever it is opened.
#[component]
fn ProgramAside(
    data: ProgramData,
    tab: ProgramTab,
    variant: Memo<usize>,
    open: Memo<Option<String>>,
    area: Memo<Option<i64>>,
    req: Memo<Option<usize>>,
    links: Memo<ProgramUrl>,
) -> impl IntoView {
    let source = use_source();
    let module = Memo::new(move |_| match open.get() {
        None => Ok(None),
        Some(id) => source.clone().and_then(|source| source.run(|db| pages::module(db, &id))).map(Some),
    });
    move || match module.get() {
        Ok(Some(Some(module))) => {
            let full_href = links.get().with_open(Some(&module.module.id)).with_full(true).path();
            view! { <ModulePanel data=module close_href=links.get().path() docked=true full_href=Some(full_href)/> }.into_any()
        }
        Ok(Some(None)) | Err(_) => view! {
            <section class="panel detail aside" id="preview" aria-label="Modulvorschau">
                <div class="state">
                    <p class="state-title">"Modul nicht gefunden"</p>
                    <p>"Dieses Modul steht nicht (mehr) im Modulkatalog der BTU."</p>
                    <p><a class="button" href=links.get().path()>"Vorschau schließen"</a></p>
                </div>
            </section>
        }
        .into_any(),
        // No module picked: the area or the row of the plan one clicked, else this view's numbers.
        Ok(None) => picked_panel(&data, tab, variant.get(), area.get(), req.get(), links, false),
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
    links: Memo<ProgramUrl>,
    /// The panel is the page (a phone): „Zurück" instead of „Schließen".
    #[prop(optional)] page: bool,
) -> impl IntoView {
    let close = links.get_untracked().with_req(None).path();
    let semester = match semester_span(&entry) {
        Some((from, to)) if from == to => Some(format!("{from}. Semester")),
        Some((from, to)) => Some(format!("{from}.–{to}. Semester")),
        None => entry.semester_span.clone().map(|span| format!("Semester {span}")),
    };
    let credits = plan_credits(&entry).map(|credits| format!("{credits} LP"));
    let fues = entry.kind.as_ref().is_some_and(|kind| kind.is(ModuleKind::Fues));
    // A row the plan states as Pflicht (or as the thesis, or as the internship) means one module,
    // not a choice: the catalog simply does not know it under this name. Only a choice gets areas.
    let one_module = entry
        .kind
        .as_ref()
        .is_some_and(|kind| kind.is(ModuleKind::Compulsory) || kind.is(ModuleKind::Thesis) || kind.is(ModuleKind::Internship));
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
    // Where to look further: the name in the whole catalog for a single module, else the modules
    // that can be chosen — the catalog scoped to this program.
    let catalog = CatalogUrl {
        query: if one_module {
            CatalogQuery { text: entry.module_name.clone(), ..Default::default() }
        } else {
            CatalogQuery {
                program: Some(ProgramScope {
                    program_slug: program.slug.clone(),
                    relation: if fues { ProgramRelation::Fues } else { ProgramRelation::Curricular },
                    kinds: if fues { Vec::new() } else { vec![KindFilter::Stated(ModuleKind::Elective)] },
                    ..Default::default()
                }),
                ..Default::default()
            }
        },
        page: 1,
        open: None,
    }
    .path();

    view! {
        <section class="panel detail aside" id="preview" aria-label="Zeile des Regelstudienplans">
            <div class="scroll">
                <header class="hero">
                    <div class="hero-top">
                        {page.then(|| view! { <BackLink area=Area::Programs to=Some(close.clone())/> })}
                        <span class="mono">"Regelstudienplan"</span>
                        {(!page).then(|| view! {
                            <a class="ghost" href=close.clone() data-action="close-detail" title="Schließen (Esc)">
                                <Icon name="x"/>"Schließen"<Shortcut keys="Esc"/>
                            </a>
                        })}
                    </div>
                    <h2>{entry.module_name.clone()}</h2>
                    {plan.map(|plan| view! { <p class="en">{plan}</p> })}
                    <p class="badges">
                        {credits.clone().map(|credits| view! { <span class="badge strong num">{credits}</span> })}
                        {entry.kind.clone().map(|kind| view! { <span class="badge">{kind.label().to_string()}</span> })}
                        {semester.clone().map(|semester| view! { <span class="badge">{semester}</span> })}
                    </p>
                </header>
                <div class="dbody">
                    <div class="section">
                        <p class="label">"Was der Plan sagt"</p>
                        <dl class="facts">
                            <Fact icon="award" label="Leistungspunkte" value=credits/>
                            <Fact icon="calendar-range" label="Semester" value=semester/>
                            <Fact icon="star" label="Art" value=entry.kind.as_ref().map(|kind| kind.label().to_string()).or_else(|| entry.kind_raw.clone())/>
                            <Fact wide=true icon="sliders-horizontal" label="Bereich im Plan" value=area/>
                        </dl>
                    </div>
                    <p class="note">
                        <Icon name="info"/>
                        <span>
                            {if one_module {
                                "Der Plan nennt hier ein einzelnes Modul, das der Modulkatalog unter diesem Namen nicht führt — es kann anders heißen oder nicht mehr angeboten werden.".to_string()
                            } else if fues {
                                "Der Plan verlangt hier ein Modul aus dem Fachübergreifenden Studium. Welche Module dafür angerechnet werden können, steht in der FÜS-Liste dieses Studiengangs.".to_string()
                            } else if ambiguous {
                                "Der Plan nennt für diese Zeile kein einzelnes Modul und verweist auf keinen Bereich. Dem Namen nach passen die Bereiche unten gleich gut dazu.".to_string()
                            } else if has_fitting {
                                "Der Plan nennt für diese Zeile kein einzelnes Modul und verweist auf keinen Bereich. Der Bereich unten passt dem Namen nach dazu.".to_string()
                            } else {
                                "Der Plan nennt für diese Zeile kein einzelnes Modul. Welche Module dafür in Frage kommen, steht in den Bereichen dieses Studiengangs.".to_string()
                            }}
                        </span>
                    </p>
                    // The area the name points at, with its modules; the others as links.
                    {first.map(|area| {
                        let count = area.modules.len();
                        let id = area.id;
                        let label = area.label.clone();
                        let path = area.path.clone();
                        view! {
                            <div class="section">
                                <p class="label">"Vermutlich " {label.clone()}<span>{count}" Module"</span></p>
                                <p class="hint">"Aus dem Namen der Planzeile abgeleitet: der Plan selbst nennt keinen Bereich. "{path}</p>
                                <div class="linklist">{area_module_links(&area.modules, &known, links)}</div>
                                <a class="pre more-area" href=move || area_href(&links.get(), id) data-walk="area" data-noscroll="">
                                    <b>"Diesen Bereich ganz ansehen"</b>
                                    <small>{label}</small>
                                    <Icon name="chevron-right"/>
                                </a>
                            </div>
                        }
                    })}
                    {(!others.is_empty()).then(|| view! {
                        <div class="section">
                            <p class="label">{if ambiguous { "Passende Bereiche" } else { "Kommt auch in Frage" }}<span>{others.len()}</span></p>
                            <div class="linklist">
                                {others.clone().into_iter().map(|area| {
                                    let id = area.id;
                                    view! {
                                        <a class="pre" href=move || area_href(&links.get(), id) data-walk="area" data-noscroll="">
                                            <b>{area.label.clone()}</b>
                                            <small>{area.modules.len()}" Module"</small>
                                            <Icon name="chevron-right"/>
                                        </a>
                                    }
                                }).collect_view()}
                            </div>
                        </div>
                    })}
                    <div class="section">
                        <p class="label">"Weiter"</p>
                        <div class="linklist">
                            <a class="pre" href=url::program_path(&program.slug, if fues || one_module { ProgramTab::Modules } else { ProgramTab::Areas })>
                                <b>{if fues { "FÜS-Liste des Studiengangs" } else if one_module { "Alle Module des Studiengangs" } else { "Wahlpflicht & Bereiche" }}</b>
                                <small>{if fues { format!("{} Module", program.fues_modules) } else if one_module { format!("{} Module", program.curricular_modules) } else { "alle Bereiche".to_string() }}</small>
                                <Icon name="chevron-right"/>
                            </a>
                            <a class="pre" href=catalog>
                                <b>{if fues { "FÜS-Module im Katalog" } else if one_module { "Diesen Namen im Katalog suchen" } else { "Wahlpflichtmodule im Katalog" }}</b>
                                <small>{if one_module { entry.module_name.clone() } else { "mit allen Filtern".to_string() }}</small>
                                <Icon name="chevron-right"/>
                            </a>
                        </div>
                    </div>
                </div>
            </div>
        </section>
    }
}

/// The modules of an area as links that open them beside the page.
fn area_module_links(modules: &[AreaPlacement], known: &HashMap<String, ProgramModule>, links: Memo<ProgramUrl>) -> AnyView {
    modules
        .iter()
        .map(|placement| {
            let id = placement.module_id.clone();
            let kind = placement.kind.clone().or_else(|| known.get(&placement.module_id).and_then(|m| m.kind.clone()));
            view! {
                <a class="pre" href=move || module_href(&links.get(), &id) data-walk="module" data-noscroll="">
                    <span class="mono">{placement.module_id.clone()}</span>
                    <b>{placement.module_title.clone()}</b>
                    <small>
                        {kind.map(|kind| format!("{} · ", kind.label())).unwrap_or_default()}
                        {format::credits(placement.module_credits)}
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
fn areas_for_row(entry: &PlanEntry, plan: &str, areas: &[AreaGroup]) -> Vec<AreaGroup> {
    let known: Vec<CatalogArea> = areas.iter().map(AreaGroup::as_catalog_area).collect();
    let found = catalog::plan::areas_for_row(entry, plan, &known);
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

/// What an area of the program holds: its numbers, the areas under it, and its modules. Opened by
/// clicking the area in „Wahlpflicht & Bereiche"; a module picked from here keeps it, so closing
/// the module comes back to this list.
#[component]
fn AreaPanel(
    group: AreaGroup,
    modules: Vec<ProgramModule>,
    links: Memo<ProgramUrl>,
    /// The panel is the page (a phone): „Zurück" instead of „Schließen".
    #[prop(optional)] page: bool,
) -> impl IntoView {
    let known: HashMap<String, ProgramModule> = modules.into_iter().map(|m| (m.module_id.clone(), m)).collect();
    let close = links.get_untracked().with_area(None).path();
    let count = group.modules.len();
    let sum: f64 = group.modules.iter().filter_map(|placement| placement.module_credits).sum();
    let kind_of = |placement: &AreaPlacement| placement.kind.clone().or_else(|| known.get(&placement.module_id).and_then(|m| m.kind.clone()));
    let kinds = kind_counts(group.modules.iter().map(&kind_of));
    let children = group.children.clone();
    let path = group.path.clone();

    view! {
        <section class="panel detail aside" id="preview" aria-label="Bereich">
            <div class="scroll">
                <header class="hero">
                    <div class="hero-top">
                        {page.then(|| view! { <BackLink area=Area::Programs to=Some(close.clone())/> })}
                        <span class="mono">"Bereich"</span>
                        {(!page).then(|| view! {
                            <a class="ghost" href=close.clone() data-action="close-detail" title="Schließen (Esc)">
                                <Icon name="x"/>"Schließen"<Shortcut keys="Esc"/>
                            </a>
                        })}
                    </div>
                    <h2>{group.label.clone()}</h2>
                    {(!path.is_empty()).then(|| view! { <p class="en">{path.clone()}</p> })}
                    <p class="badges">
                        <span class="badge strong num">{count}" Module"</span>
                        {(sum > 0.0).then(|| view! { <span class="badge num">{format::number(sum)}" LP"</span> })}
                    </p>
                </header>
                <div class="dbody">
                    <div class="section">
                        <p class="label">"Auf einen Blick"</p>
                        <dl class="facts">
                            {kinds.into_iter().map(|(icon, label, count)| view! { <Fact icon=icon label=label value=Some(count.to_string())/> }).collect_view()}
                            <Fact icon="award" label="Leistungspunkte" value=(sum > 0.0).then(|| format!("{} LP", format::number(sum)))/>
                        </dl>
                    </div>
                    {(!children.is_empty()).then(|| view! {
                        <div class="section">
                            <p class="label">"Bereiche darin"<span>{children.len()}</span></p>
                            <div class="linklist">
                                {children.into_iter().map(|(id, label, modules)| view! {
                                    <a class="pre" href=move || area_href(&links.get(), id) data-noscroll="">
                                        <b>{label}</b>
                                        <small>{modules}" Module"</small>
                                        <Icon name="chevron-right"/>
                                    </a>
                                }).collect_view()}
                            </div>
                        </div>
                    })}
                    <div class="section">
                        <p class="label">"Module"<span>{count}</span></p>
                        <div class="linklist">{area_module_links(&group.modules, &known, links)}</div>
                    </div>
                </div>
            </div>
        </section>
    }
}

/// The numbers of the view one is looking at: what the plan of this study direction adds up to,
/// or what the program has in areas and modules. Only what the data states, never a guess.
#[component]
fn ProgramNumbers(data: ProgramData, tab: ProgramTab, plans: Vec<PlanVariant>, #[prop(into)] variant: Signal<usize>) -> impl IntoView {
    let p = data.program.clone();
    let areas = area_groups(&data.areas).len();
    let kinds = kind_counts(data.curricular.iter().map(|m| m.kind.clone()));

    move || {
        let plan = plans.get(variant.get().min(plans.len()).saturating_sub(1)).cloned();
        let heading = match tab {
            ProgramTab::Plan => "Regelstudienplan",
            ProgramTab::Areas => "Wahlpflicht & Bereiche",
            ProgramTab::Modules => "Alle Module",
        };
        let several = plans.len() > 1;
        view! {
            <section class="panel detail aside" id="preview" aria-label="Überblick">
                <div class="scroll">
                    <header class="hero">
                        <h2>{heading}</h2>
                        {(tab == ProgramTab::Plan).then(|| plan.as_ref().map(|plan| view! {
                            <p class="en" title=plan.full.clone()>{if several { plan.label.clone() } else { "Aus der Prüfungs- und Studienordnung".to_string() }}</p>
                        }))}
                    </header>
                    <div class="dbody">
                        <div class="section">
                            <p class="label">"Auf einen Blick"</p>
                            <dl class="facts">
                                {match (tab, plan.clone()) {
                                    (ProgramTab::Plan, Some(plan)) => view! {
                                        <Fact icon="calendar-range" label="Fachsemester" value=Some(format!("{}", plan.semesters))/>
                                        <Fact icon="award" label="Leistungspunkte" value=Some(format!("{} LP", format::number(plan.credits)))/>
                                        <Fact icon="layout-list" label="Zeilen im Plan" value=Some(format!("{}", plan.entries.len()))/>
                                        <Fact icon="file-check-2" label="Im Katalog verlinkt" value=Some(format!("{}", plan.entries.iter().filter(|entry| entry.module_id.is_some()).count()))/>
                                    }.into_any(),
                                    _ => view! {
                                        <Fact icon="layout-list" label="Module im Curriculum" value=Some(format!("{}", p.curricular_modules))/>
                                        <Fact icon="sliders-horizontal" label="Bereiche" value=Some(format!("{areas}"))/>
                                        {kinds.clone().into_iter().map(|(icon, label, count)| view! { <Fact icon=icon label=label value=Some(count.to_string())/> }).collect_view()}
                                        <Fact icon="file-check-2" label="FÜS-Module" value=Some(format!("{}", p.fues_modules))/>
                                    }.into_any(),
                                }}
                            </dl>
                        </div>
                        {(tab == ProgramTab::Plan).then(|| plan.as_ref().map(|plan| {
                            let per = credits_per_semester(plan);
                            let most = per.iter().map(|(_, credits)| *credits).fold(0.0f64, f64::max);
                            (!per.is_empty() && most > 0.0).then(|| view! {
                                <div class="section">
                                    <p class="label">"LP je Semester"</p>
                                    <ul class="bars">
                                        {per.into_iter().map(|(semester, credits)| view! {
                                            <li>
                                                <span class="bar-n">{semester}"."</span>
                                                <span class="bar" style=format!("--at:{:.1}%", credits / most * 100.0)></span>
                                                <span class="bar-v num">{(credits > 0.0).then(|| format::number(credits))}</span>
                                            </li>
                                        }).collect_view()}
                                    </ul>
                                    <p class="hint">"Nur Module, die der Plan einem einzelnen Semester zuordnet."</p>
                                </div>
                            })
                        }))}
                        <p class="pick"><Icon name="info"/><span>"Ein Modul anklicken, um es hier zu lesen."</span></p>
                    </div>
                </div>
            </section>
        }
    }
}

/// What the plan puts into each of its semesters. A module over several semesters belongs to no
/// single one, so it is in none of these sums (the plan does not say how it splits).
fn credits_per_semester(plan: &PlanVariant) -> Vec<(i64, f64)> {
    let mut per: Vec<(i64, f64)> = (1..=plan.semesters.max(1)).map(|semester| (semester, 0.0)).collect();
    for entry in &plan.entries {
        if let Some((from, to)) = semester_span(entry) {
            if from == to {
                if let Some((_, credits)) = per.iter_mut().find(|(semester, _)| *semester == from) {
                    *credits += entry.credits.or(entry.min_credits).unwrap_or(0.0);
                }
            }
        }
    }
    per
}

/// The head of the page: where the visitor is, what the program is called, what it is in numbers
/// and under which regulations. The numbers stand on the title's line, right of it.
#[component]
fn ProgramHead(program: Program, plans: Vec<PlanVariant>) -> impl IntoView {
    let level = url::LevelGroup::of(&program.degree_level);
    let level_link = url::ProgramsUrl { levels: vec![level], ..Default::default() }.path();
    // Length and size of the studies are what the validated plan says, not what a source states.
    let semesters = span_of(plans.iter().map(|plan| plan.semesters as f64));
    let credits = span_of(plans.iter().map(|plan| plan.credits));
    view! {
        <header class="panel prog-head">
            <div class="hero-top">
                <BackLink area=Area::Programs/>
                <nav class="crumbs" aria-label="Pfad">
                    <a href=url::PROGRAMS>"Studiengänge"</a>
                    <a href=level_link>{level.label()}</a>
                </nav>
            </div>
            <div class="prog-title">
                <h1>{program.name.clone()}</h1>
                <p class="prog-facts">
                    {semesters.map(|text| view! { <span class="pfact" title="Fachsemester laut Regelstudienplan"><b>{text}</b>" Semester"</span> })}
                    {credits.map(|text| view! { <span class="pfact" title="Leistungspunkte laut Regelstudienplan"><b>{text}</b>" LP"</span> })}
                    <span class="pfact"><b>{program.curricular_modules}</b>" Module"</span>
                    <span class="pfact"><b>{program.fues_modules}</b>" FÜS-Module"</span>
                </p>
            </div>
            <p class="prog-meta">
                <span class="degree">{program.degree().to_string()}</span>
                <span>"Prüfungsordnung "{program.po_version.clone()}</span>
                {program.study_variant.as_ref().map(|v| view! { <span>{format::variant_short(v)}</span> })}
                {if program.is_latest_po {
                    view! { <span class="current">"aktuell"</span> }.into_any()
                } else {
                    view! { <span class="flag">"ältere Prüfungsordnung"</span> }.into_any()
                }}
            </p>
        </header>
    }
}

// ---------- the study plan ----------

/// One study plan of a program. Most programs have exactly one; where the regulations print one
/// plan per study direction, each is its own plan with its own semesters and its own sum.
#[derive(Clone, Debug)]
struct PlanVariant {
    /// What the chips say: the name of the plan without its boilerplate.
    label: String,
    /// The name as the regulations print it (the title of the chip).
    full: String,
    /// The last semester the plan names.
    semesters: i64,
    /// What the whole plan adds up to.
    credits: f64,
    entries: Vec<PlanEntry>,
}

/// Splits the rows of the plan into the plans they were printed as. The order is the order of the
/// document; rows without a name of their own form one unnamed plan.
fn plan_variants(entries: &[PlanEntry]) -> Vec<PlanVariant> {
    let mut plans: Vec<PlanVariant> = Vec::new();
    for entry in entries {
        let full = entry.specialization.clone().unwrap_or_default();
        let plan = match plans.iter_mut().find(|plan| plan.full == full) {
            Some(plan) => plan,
            None => {
                plans.push(PlanVariant { label: String::new(), full: full.clone(), semesters: 0, credits: 0.0, entries: Vec::new() });
                match plans.last_mut() {
                    Some(plan) => plan,
                    None => continue,
                }
            }
        };
        if let Some((_, to)) = semester_span(entry) {
            plan.semesters = plan.semesters.max(to);
        }
        plan.credits += entry.credits.or(entry.min_credits).unwrap_or(0.0);
        plan.entries.push(entry.clone());
    }
    plans.truncate(url::MAX_PLAN_VARIANTS);
    // The chips say what tells the plans apart, which only all of them together can say.
    let labels = tell_plans_apart(&plans.iter().map(|plan| plan.full.clone()).collect::<Vec<_>>());
    for (plan, label) in plans.iter_mut().zip(labels) {
        plan.label = label;
    }
    plans
}

/// The name of a plan without the words every plan of this program carries anyway. The captions
/// of the regulations differ in one place only — „Regelstudienplan der Studienrichtungen **MIT
/// und EET** im grundständigen Studium" — so a chip says „MIT und EET" and keeps the whole
/// caption as its title. Nothing is invented: where the names do not differ, the name stays.
fn tell_plans_apart(names: &[String]) -> Vec<String> {
    let stripped: Vec<&str> = names.iter().map(|name| strip_plan_boilerplate(name)).collect();
    let words: Vec<Vec<&str>> = stripped.iter().map(|name| name.split_whitespace().collect()).collect();
    let shortest = words.iter().map(Vec::len).min().unwrap_or(0);
    if names.len() < 2 || shortest == 0 {
        return stripped.iter().map(|name| clip_plan_name(name)).collect();
    }
    let same_at = |i: usize| {
        let first = words.first().and_then(|first| first.get(i));
        words.iter().all(|name| name.get(i) == first)
    };
    fn from_end<'a>(name: &[&'a str], i: usize) -> Option<&'a str> {
        name.len().checked_sub(i + 1).and_then(|at| name.get(at)).copied()
    }
    let same_from_end = |i: usize| {
        let first = words.first().and_then(|first| from_end(first, i));
        words.iter().all(|name| from_end(name, i) == first)
    };
    let lead = (0..shortest).take_while(|i| same_at(*i)).count();
    // Words at the end are only boilerplate when there are several of them („im grundstaendigen
    // Studium"); a single one usually belongs to the name („Konstruktiver *Ingenieurbau*").
    let tail = match (0..shortest.saturating_sub(lead)).take_while(|i| same_from_end(*i)).count() {
        1 => 0,
        several => several,
    };
    words
        .iter()
        .zip(stripped.iter())
        .map(|(name, whole)| {
            let rest: Vec<&str> = name.iter().skip(lead).take(name.len().saturating_sub(lead + tail)).copied().collect();
            match rest.is_empty() {
                true => clip_plan_name(whole),
                false => clip_plan_name(rest.join(" ").trim_matches(|c: char| c == '–' || c == '-' || c == ',' || c == ';' || c.is_whitespace())),
            }
        })
        .collect()
}

/// The caption of a plan without the words every such caption starts with.
fn strip_plan_boilerplate(name: &str) -> &str {
    let name = name.trim();
    for prefix in [
        "Regelstudienplan der Studienrichtungen ",
        "Regelstudienplan der Studienrichtung ",
        "Regelstudienplan für das ",
        "Regelstudienplans für das ",
        "Regelstudienplan ",
        "Studienplan · ",
        "Studienplan ",
        "Studienrichtung ",
    ] {
        if let Some(rest) = name.strip_prefix(prefix) {
            return rest.trim();
        }
    }
    name
}

/// Long captions are cut at a word, never in the middle of one; the chip shortens what is still
/// too wide for it, and the whole caption is its title.
fn clip_plan_name(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        return "Regelstudienplan".to_string();
    }
    if name.chars().count() <= 72 {
        return name.to_string();
    }
    let cut = name.char_indices().nth(72).map(|(at, _)| at).unwrap_or(name.len());
    let head = name.get(..cut).unwrap_or(name);
    let head = head.rsplit_once(' ').map(|(start, _)| start).unwrap_or(head);
    format!("{}…", head.trim_end_matches([',', ';', '(']).trim())
}

/// Which semesters a row of the plan belongs to: one, a span, or none at all.
fn semester_span(entry: &PlanEntry) -> Option<(i64, i64)> {
    catalog::plan::semester_span(entry)
}

/// „6" or „3–4": one number where all plans agree, the range where they do not. `None` where no
/// plan says anything.
fn span_of(values: impl Iterator<Item = f64>) -> Option<String> {
    let values: Vec<f64> = values.filter(|value| *value > 0.0).collect();
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    match values.is_empty() {
        true => None,
        false if low == high => Some(format::number(low)),
        false => Some(format!("{}–{}", format::number(low), format::number(high))),
    }
}

/// What a row of the plan says about its credits: a number, a range, or nothing.
fn plan_credits(entry: &PlanEntry) -> Option<String> {
    match (entry.credits, entry.min_credits, entry.max_credits) {
        (Some(credits), _, _) => Some(format::number(credits)),
        (None, Some(min), Some(max)) if min != max => Some(format!("{}–{}", format::number(min), format::number(max))),
        (None, Some(value), _) | (None, None, Some(value)) => Some(format::number(value)),
        _ => None,
    }
}

#[component]
fn PlanTab(
    plans: Vec<PlanVariant>,
    validated: Option<String>,
    /// What to say when there is no validated plan.
    missing: String,
    variant: Memo<usize>,
    shape: RwSignal<PlanShape>,
    links: Memo<ProgramUrl>,
    open: Memo<Option<String>>,
    req: Memo<Option<usize>>,
) -> impl IntoView {
    if plans.is_empty() {
        return view! {
            <EmptyState title=missing hint="Die Module findest du unter „Wahlpflicht & Bereiche“ und „Alle Module“. Fachsemester werden nur angezeigt, wenn ein Regelstudienplan sie nennt."/>
        }
        .into_any();
    }
    let chips = plans.clone();
    let body = plans.clone();
    view! {
        <section class="panel plan-block">
            <header class="block-head">
                <h2>"Regelstudienplan"</h2>
                <p>
                    "Aus der Prüfungs- und Studienordnung übernommen und geprüft"
                    {validated.as_deref().map(|at| format!(" am {}", format::date(at)))}"."
                </p>
            </header>
            {move || {
                let plans = chips.clone();
                let here = links.get();
                let now = variant.get();
                (plans.len() > 1).then(|| view! {
                    <div class="variants">
                        <p class="flabel label">"Studienrichtung"</p>
                        <p class="chip-links">
                            {plans.iter().enumerate().map(|(i, plan)| {
                                let number = i + 1;
                                let chosen = number == now.min(plans.len());
                                view! {
                                    <a
                                        class="chip"
                                        data-walk="plan-variant"
                                        href=here.with_variant(number).path()
                                        title=plan.full.clone()
                                        data-noscroll=""
                                        aria-current=chosen.then_some("true")
                                        data-state=if chosen { "with" } else { "off" }
                                    >
                                        <span class="chip-label">{plan.label.clone()}</span>
                                        <span class="chip-count num">{format::number(plan.credits)}" LP"</span>
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

/// The plan as its regulations print it: a row per module, a column per semester, the credits in
/// the cell. Modules over several semesters span their columns.
#[component]
fn PlanMatrix(plan: PlanVariant, links: Memo<ProgramUrl>, open: Memo<Option<String>>, req: Memo<Option<usize>>) -> impl IntoView {
    let last = plan.semesters.max(1);
    let columns = (1..=last).collect::<Vec<i64>>();
    let width = usize::try_from(last).unwrap_or(1) + 2;
    // Only what the plan puts into one semester can be added up in that semester's column.
    let mut sums = vec![0.0f64; columns.len()];
    let mut spread = false;
    for entry in &plan.entries {
        match semester_span(entry) {
            Some((from, to)) if from == to => {
                if let Some(sum) = usize::try_from(from - 1).ok().and_then(|i| sums.get_mut(i)) {
                    *sum += entry.credits.or(entry.min_credits).unwrap_or(0.0);
                }
            }
            _ => spread = true,
        }
    }
    let differs = plan.entries.iter().any(|entry| entry.credits_differ_from_catalog);

    view! {
        <div class="table-scroll">
            <table class="ptable matrix">
                // Fixed columns: every semester is exactly as wide as every other one.
                <colgroup>
                    <col/>
                    <col class="c-kind-col"/>
                    <col class="c-sem-col" span=last/>
                </colgroup>
                <thead>
                    <tr>
                        <th scope="col" rowspan="2" class="c-name">"Modul"</th>
                        <th scope="col" rowspan="2" class="c-kind">"Art"</th>
                        <th scope="colgroup" colspan=last class="c-group">"Leistungspunkte im Semester"</th>
                    </tr>
                    <tr>
                        {columns.iter().map(|n| view! { <th scope="col" class="c-sem num">{*n}</th> }).collect_view()}
                    </tr>
                </thead>
                {section_groups(&plan.entries).into_iter().map(|(section, entries)| view! {
                    <tbody>
                        {section.map(|name| view! { <tr class="group"><th colspan=width scope="rowgroup">{name}</th></tr> })}
                        {entries.into_iter().map(|(row, entry)| {
                            let span = semester_span(&entry);
                            let credits = plan_credits(&entry);
                            let row_id = entry.module_id.clone();
                            view! {
                                <tr class:open=move || is_open(&row_id, open) || req.get() == Some(row)>
                                    <th scope="row" class="c-name">{plan_module(&entry, row, links, open)}</th>
                                    <td class="c-kind">{kind_cell(entry.kind.clone())}</td>
                                    {columns.iter().filter_map(|n| match span {
                                        Some((from, to)) if *n == from => {
                                            let class = if to > from { "lp num spans" } else { "lp num" };
                                            Some(view! {
                                                <td class=class colspan=to - from + 1 class:differs=entry.credits_differ_from_catalog
                                                    title=entry.credits_differ_from_catalog.then(|| "Die LP dieses Plans weichen vom Modulkatalog ab".to_string())>
                                                    {credits.clone().unwrap_or_else(|| "·".to_string())}
                                                </td>
                                            }.into_any())
                                        }
                                        Some((from, to)) if *n > from && *n <= to => None,
                                        Some(_) => Some(view! { <td class="empty"></td> }.into_any()),
                                        // The plan names no semester for this module.
                                        None if *n == 1 => Some(view! {
                                            <td class="lp num loose" colspan=last title="Der Plan ordnet dieses Modul keinem Semester zu">
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
                        <th scope="row" class="c-name">"Summe"</th>
                        <td class="c-kind"></td>
                        {sums.iter().map(|sum| view! { <td class="lp num">{(*sum > 0.0).then(|| format::number(*sum))}</td> }).collect_view()}
                    </tr>
                </tfoot>
            </table>
        </div>
        {(spread || differs).then(|| view! {
            <p class="hint footnote">
                {spread.then_some("Module über mehrere Semester stehen über ihrem Zeitraum und sind in keiner Semestersumme enthalten. ")}
                {differs.then_some("Markierte LP weichen vom Modulkatalog ab.")}
            </p>
        })}
    }
}

/// The same plan as a list: semester after semester, a row per module.
#[component]
fn PlanList(plan: PlanVariant, links: Memo<ProgramUrl>, open: Memo<Option<String>>, req: Memo<Option<usize>>) -> impl IntoView {
    let with_area = plan.entries.iter().any(|entry| entry.study_section.is_some() || entry.subject_area.is_some());
    let head_cols = if with_area { 3 } else { 2 };
    // Semesters the plan names, in order, with the rows they hold.
    let mut groups: Vec<((i64, i64), String, Vec<NumberedRow>)> = Vec::new();
    for (row, entry) in plan.entries.iter().cloned().enumerate().map(|(i, entry)| (i + 1, entry)) {
        let span = semester_span(&entry);
        let label = match (span, &entry.semester_span) {
            (Some((from, to)), _) if from == to => format!("{from}. Semester"),
            (Some((from, to)), _) => format!("{from}.–{to}. Semester"),
            (None, Some(span)) => format!("Semester {span}"),
            (None, None) => "Ohne Semesterangabe im Plan".to_string(),
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
                        <th scope="col" class="c-name">"Modul"</th>
                        <th scope="col" class="c-kind">"Art"</th>
                        {with_area.then(|| view! { <th scope="col" class="c-area">"Bereich"</th> })}
                        <th scope="col" class="c-lp num">"LP"</th>
                    </tr>
                </thead>
                {groups.into_iter().map(|(_, label, entries)| {
                    let sum: f64 = entries.iter().map(|(_, entry)| entry.credits.or(entry.min_credits).unwrap_or(0.0)).sum();
                    let count = entries.len();
                    view! {
                        <tbody>
                            <tr class="group">
                                <th colspan=head_cols scope="rowgroup">
                                    <span class="ginner">
                                        <span class="gname">{label}</span>
                                        <span class="gcount">{count}" Module"</span>
                                    </span>
                                </th>
                                <td class="c-lp num">{(sum > 0.0).then(|| format::number(sum))}</td>
                            </tr>
                            {entries.into_iter().map(|(row, entry)| {
                                let row_id = entry.module_id.clone();
                                view! {
                                    <tr class:open=move || is_open(&row_id, open) || req.get() == Some(row)>
                                        <th scope="row" class="c-name">{plan_module(&entry, row, links, open)}</th>
                                        <td class="c-kind">{kind_cell(entry.kind.clone())}</td>
                                        {with_area.then(|| view! { <td class="c-area">{entry.study_section.clone().or(entry.subject_area.clone())}</td> })}
                                        <td class="c-lp num" class:differs=entry.credits_differ_from_catalog>
                                            {plan_credits(&entry)}
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
fn plan_module(entry: &PlanEntry, row: usize, links: Memo<ProgramUrl>, open: Memo<Option<String>>) -> AnyView {
    match &entry.module_id {
        Some(id) => module_link(id, &entry.module_name, links, open),
        // A row that names no module: in the app it opens beside the page (`?req=<n>`); the
        // server's page has no page for it, so there it is what the plan says, as text.
        None if !APP => view! { <span class="unstated">{entry.module_name.clone()}</span> }.into_any(),
        None => {
            let name = entry.module_name.clone();
            view! {
                <a class="unstated" href=move || links.get().with_req(Some(row)).path() data-walk="plan-row" data-noscroll="" title="Was der Plan zu dieser Zeile sagt">
                    {name}
                </a>
            }
            .into_any()
        }
    }
}

/// Where a module of the program leads: in the app beside the page (`?open=<id>`, as in the
/// catalog, with „Vollbild" there); on the server's page to the module's own page.
fn module_href(links: &ProgramUrl, id: &str) -> String {
    if APP {
        links.with_open(Some(id)).path()
    } else {
        url::module_path(id)
    }
}

/// Where an area of the program leads: in the app beside the page (`?area=<id>`); on the
/// server's page to the catalog narrowed down to the area, the page that lists what it holds.
fn area_href(links: &ProgramUrl, id: i64) -> String {
    if APP {
        links.with_area(Some(id)).path()
    } else {
        CatalogUrl {
            query: CatalogQuery { program: Some(ProgramScope { program_slug: links.slug.clone(), area: Some(id), ..Default::default() }), ..Default::default() },
            page: 1,
            open: None,
        }
        .path()
    }
}

/// A module of the program, opening beside the page (`?open=<id>`, as in the catalog).
fn module_link(id: &str, title: &str, links: Memo<ProgramUrl>, open: Memo<Option<String>>) -> AnyView {
    let id = id.to_string();
    let href = {
        let id = id.clone();
        move || module_href(&links.get(), &id)
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
fn section_groups(entries: &[PlanEntry]) -> Vec<RowGroup> {
    let numbered: Vec<NumberedRow> = entries.iter().cloned().enumerate().map(|(i, entry)| (i + 1, entry)).collect();
    let named = entries.iter().any(|entry| entry.study_section.is_some() || entry.subject_area.is_some());
    if !named {
        return vec![(None, numbered)];
    }
    let mut groups: Vec<RowGroup> = Vec::new();
    for (row, entry) in numbered {
        let name = entry.study_section.clone().or_else(|| entry.subject_area.clone()).unwrap_or_else(|| "Ohne Bereich im Plan".to_string());
        match groups.iter_mut().find(|(existing, _)| existing.as_deref() == Some(name.as_str())) {
            Some((_, rows)) => rows.push((row, entry)),
            None => groups.push((Some(name), vec![(row, entry)])),
        }
    }
    groups
}

// ---------- areas and modules: one table ----------

/// How a list of modules splits by what the program states them as: every kind that occurs
/// („Pflicht", „Wahlpflicht", „Abschlussarbeit", „Praktikum", „FÜS") and, where no source says,
/// „Art nicht angegeben" (R12). Nothing is counted into a kind it was not stated as.
fn kind_counts(kinds: impl Iterator<Item = Option<Code<ModuleKind>>>) -> Vec<(&'static str, String, usize)> {
    let mut counts: Vec<(&'static str, String, usize)> = Vec::new();
    let mut unstated = 0usize;
    for kind in kinds {
        let Some(kind) = kind else {
            unstated += 1;
            continue;
        };
        let label = kind.label().to_string();
        match counts.iter_mut().find(|(_, existing, _)| *existing == label) {
            Some((_, _, count)) => *count += 1,
            None => counts.push((kind_icon(&kind), label, 1)),
        }
    }
    if unstated > 0 {
        counts.push(("info", "Art nicht angegeben".to_string(), unstated));
    }
    counts
}

fn kind_icon(kind: &Code<ModuleKind>) -> &'static str {
    match kind.known() {
        Some(ModuleKind::Compulsory) => "graduation-cap",
        Some(ModuleKind::Elective) => "star",
        Some(ModuleKind::Thesis) => "award",
        Some(ModuleKind::Internship) => "layout-list",
        Some(ModuleKind::Fues) => "arrow-up-right",
        None => "info",
    }
}

/// „Pflicht", „Wahlpflicht" … as a table cell; „–" where no source states the kind (the full
/// wording stays in the title, so nothing is invented and nothing is claimed).
fn kind_cell(kind: Option<Code<ModuleKind>>) -> AnyView {
    match kind {
        Some(kind) => view! { <span class=format!("kind k-{}", kind.code())><i></i>{kind.label().to_string()}</span> }.into_any(),
        None => view! { <span class="unknown" title="Art nicht angegeben">"–"</span> }.into_any(),
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
    /// Shown only where the area is not already the group of the row.
    area: Option<String>,
}

fn module_head(with_area: bool) -> AnyView {
    view! {
        <thead>
            <tr>
                <th scope="col" class="c-id">"Nr."</th>
                <th scope="col" class="c-name">"Modul"</th>
                {with_area.then(|| view! { <th scope="col" class="c-area">"Bereich"</th> })}
                <th scope="col" class="c-kind">"Art"</th>
                <th scope="col" class="c-lp num">"LP"</th>
                <th scope="col" class="c-turnus">"Turnus"</th>
                <th scope="col" class="c-sem num">"Sem."</th>
            </tr>
        </thead>
    }
    .into_any()
}

fn module_row(row: ModuleRow, with_area: bool, links: Memo<ProgramUrl>, open: Memo<Option<String>>) -> AnyView {
    let id = row.id.clone();
    view! {
        <tr class:open=move || is_open(&Some(id.clone()), open)>
            <td class="c-id">{row.id.clone()}</td>
            <th scope="row" class="c-name">
                {module_link(&row.id, &row.title, links, open)}
                <OfferBadge status=row.offer/>
            </th>
            {with_area.then(|| {
                // The area of a module, as short as it stays unique to the eye; the whole path
                // is in the title, so nothing is lost.
                let full = row.area.clone();
                let short = row.area.as_ref().map(|area| area.rsplit(" / ").next().unwrap_or(area).to_string());
                view! { <td class="c-area" title=full><span>{short}</span></td> }
            })}
            <td class="c-kind">{kind_cell(row.kind)}</td>
            <td class="c-lp num">{row.credits.map(format::number)}</td>
            <td class="c-turnus">{row.turnus.as_ref().map(|season| format::turnus(Some(season), None))}</td>
            <td class="c-sem num" class:unknown=row.semester.is_none()>
                {row.semester.map(|n| format!("{n}.")).unwrap_or_else(|| "–".to_string())}
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
    /// The label of the area directly above, from the tree itself.
    parent_label: Option<String>,
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
        Self { id: 0, label: String::new(), path: String::new(), parent: None, parent_label: None, depth: 0, modules: Vec::new(), children: Vec::new() }
    }

    fn is_ambiguous(&self) -> bool {
        self.id == 0
    }

    /// The area as the catalog crate knows it (for the derivation shared with the catalog).
    fn as_catalog_area(&self) -> CatalogArea {
        CatalogArea { id: self.id, label: self.label.clone(), path: self.path.clone(), depth: self.depth, modules: self.modules.len(), choice: pages::is_choice(&self.modules), parent: self.parent_label.clone() }
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
                parent_label: self.parent_label.clone(),
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
                parent_label: placement.parent_label.clone(),
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
    let groups = area_groups(&areas);
    if groups.is_empty() {
        return view! {
            <EmptyState title="Keine Bereiche bekannt" hint="Das Vorlesungsverzeichnis gliedert diesen Studiengang nicht in Bereiche."/>
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
                <h2>"Wahlpflicht & Bereiche "<span class="tab-count">{groups.len()}</span></h2>
                <p>"Wie das Vorlesungsverzeichnis diesen Studiengang gliedert. Ein Modul kann in mehreren Bereichen stehen."</p>
            </header>
            <div class="table-scroll">
                <table class="ptable modules areas">
                    {module_head(false)}
                    {groups.into_iter().map(|group| {
                        let sum: f64 = group.modules.iter().filter_map(|m| m.module_credits).sum();
                        let count = group.modules.len();
                        let known = &known;
                        view! {
                            <tbody>
                                <tr class=format!("group depth-{}", group.depth.clamp(1, 4)) id=format!("area-{}", group.id) class:open=move || area.get() == Some(group.id)>
                                    <th colspan="6" scope="rowgroup">
                                        // The area itself is a link: it shows beside the page what it holds.
                                        <a class="ginner" data-walk="area" data-noscroll="" href=move || area_href(&links.get(), group.id) aria-current=move || (area.get() == Some(group.id)).then_some("true")>
                                            <span class="gname">{group.label}</span>
                                            {group.parent.map(|parent| view! { <span class="gpath">{parent}</span> })}
                                            <span class="gcount">{count}" Module"{(sum > 0.0).then(|| format!(" · {} LP", format::number(sum)))}</span>
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
                                        area: None,
                                        id: placement.module_id,
                                        title: placement.module_title,
                                    }, false, links, open)
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

#[component]
fn ModulesTab(curricular: Vec<ProgramModule>, fues: Vec<ProgramModule>, links: Memo<ProgramUrl>, open: Memo<Option<String>>) -> impl IntoView {
    let table = |modules: Vec<ProgramModule>, with_area: bool| view! {
        <div class="table-scroll">
            <table class="ptable modules">
                {module_head(with_area)}
                <tbody>
                    {modules.into_iter().map(|m| module_row(ModuleRow {
                        id: m.module_id,
                        title: m.module_title,
                        kind: m.kind,
                        credits: m.module_credits,
                        turnus: m.turnus_season,
                        semester: m.plan_semester,
                        offer: m.offer_status,
                        area: m.area,
                    }, with_area, links, open)).collect_view()}
                </tbody>
            </table>
        </div>
    };
    let fues_count = fues.len();
    let relation_hint = ProgramRelation::Fues.code();

    view! {
        <section class="panel">
            <header class="block-head">
                <h2>"Curriculum "<span class="tab-count">{curricular.len()}</span></h2>
                <p>"Alle Module des Studiengangs. Das Fachsemester steht dort, wo der Regelstudienplan es nennt."</p>
            </header>
            {table(curricular, true)}
        </section>
        <section class="panel" id=relation_hint>
            <header class="block-head">
                <h2>"Fachübergreifendes Studium (FÜS) "<span class="tab-count">{fues_count}</span></h2>
                <p>"Module, die in diesem Studiengang als FÜS angerechnet werden können. Sie gehören nicht zum Curriculum."</p>
            </header>
            {if fues_count == 0 {
                view! { <p class="hint">"Für diesen Studiengang ist keine FÜS-Liste bekannt."</p> }.into_any()
            } else {
                view! {
                    <details class="fues">
                        <summary><span>{fues_count}" FÜS-Module anzeigen"</span></summary>
                        {table(fues, false)}
                    </details>
                }
                .into_any()
            }}
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chips_say_what_tells_the_plans_apart() {
        let names = |list: &[&str]| list.iter().map(|name| name.to_string()).collect::<Vec<_>>();
        assert_eq!(
            tell_plans_apart(&names(&[
                "Regelstudienplan der Studienrichtungen MIT und EET im grundständigen Studium",
                "Regelstudienplan der Studienrichtungen PA und IoT im grundständigen Studium",
            ])),
            vec!["MIT und EET", "PA und IoT"],
        );
        assert_eq!(
            tell_plans_apart(&names(&[
                "Regelstudienplan Bachelor of Science – grundlagenorientiert (180 LP) Studienrichtung Konstruktiver Ingenieurbau",
                "Regelstudienplan Bachelor of Science – grundlagenorientiert (180 LP) Studienrichtung Allgemeiner Ingenieurbau",
                "Regelstudienplan Bachelor of Science – praxisorientiert (240 LP) Studienrichtung Konstruktiver Ingenieurbau",
            ])),
            vec![
                "grundlagenorientiert (180 LP) Studienrichtung Konstruktiver Ingenieurbau",
                "grundlagenorientiert (180 LP) Studienrichtung Allgemeiner Ingenieurbau",
                "praxisorientiert (240 LP) Studienrichtung Konstruktiver Ingenieurbau",
            ],
        );
        // One plan keeps its name, and a plan without one is still called what it is.
        assert_eq!(tell_plans_apart(&names(&["Regelstudienplan"])), vec!["Regelstudienplan"]);
        assert_eq!(tell_plans_apart(&names(&[""])), vec!["Regelstudienplan"]);
        // Names that do not differ are not cut down to nothing.
        assert_eq!(tell_plans_apart(&names(&["Studienplan · Seite 5", "Studienplan · Seite 5"])), vec!["Seite 5", "Seite 5"]);
        // A plan that differs in one word keeps the word, not only what is around it.
        assert_eq!(
            tell_plans_apart(&names(&["Regelstudienplan Studienrichtung Konstruktiver Ingenieurbau", "Regelstudienplan Studienrichtung Allgemeiner Ingenieurbau"])),
            vec!["Konstruktiver Ingenieurbau", "Allgemeiner Ingenieurbau"],
        );
    }

    #[test]
    fn the_areas_a_row_points_at_are_the_page_s_groups_with_the_mark_of_a_tie() {
        let area = |id: i64, label: &str, modules: usize| AreaGroup {
            id,
            label: label.to_string(),
            path: format!("Grundstudium / {label}"),
            parent: Some("Grundstudium".to_string()),
            parent_label: Some("Grundstudium".to_string()),
            depth: 2,
            modules: Vec::new(),
            children: Vec::new(),
        }
        .with_modules(modules);
        let areas = vec![area(1, "Informatik (MIT)", 1), area(2, "Informatik (EET)", 1), area(5, "Studienrichtungsspezifische Vertiefungsmodule (MIT)", 23)];
        let row = |name: &str| PlanEntry {
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
        };
        let plan = "Regelstudienplan der Studienrichtungen MIT und EET im grundständigen Studium";
        let fitting = areas_for_row(&row("Wahlpflichtmodul aus der Informatik"), plan, &areas);
        assert!(fitting.first().is_some_and(AreaGroup::is_ambiguous), "two directions fit, so none is shown as the one");
        assert_eq!(fitting.iter().filter(|area| !area.is_ambiguous()).map(|area| area.id).collect::<Vec<_>>(), vec![1, 2]);
        let fitting = areas_for_row(&row("Wahlpflichtmodule der Studienrichtung"), plan, &areas);
        assert_eq!(fitting.first().map(|area| area.id), Some(5));
        assert!(areas_for_row(&row("Bachelor-Arbeit"), plan, &areas).is_empty());
    }

    #[test]
    fn a_row_of_the_plan_knows_its_semesters() {
        let entry = |semester, start, end| PlanEntry {
            module_id: None,
            module_name: "M".to_string(),
            semester,
            start_semester: start,
            end_semester: end,
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
        };
        assert_eq!(semester_span(&entry(Some(3), Some(1), Some(2))), Some((3, 3)));
        assert_eq!(semester_span(&entry(None, Some(3), Some(2))), Some((2, 3)));
        assert_eq!(semester_span(&entry(None, None, Some(4))), Some((4, 4)));
        assert_eq!(semester_span(&entry(None, None, None)), None);
        assert_eq!(span_of([6.0, 6.0].into_iter()).as_deref(), Some("6"));
        assert_eq!(span_of([4.0, 6.0, 0.0].into_iter()).as_deref(), Some("4–6"));
        assert_eq!(span_of([0.0].into_iter()), None);
    }
}
