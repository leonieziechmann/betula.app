//! The program page: header, documents, and the tabs Regelstudienplan / Bereiche / Alle Module.
//! The tab is part of the URL, so it arrives as a plain value from the router.

use catalog::filter::{ProgramRelation, ProgramScope};
use catalog::pages::{self, ProgramData};
use catalog::rows::ProgramModule;
use catalog::rows_detail::{AreaPlacement, PlanEntry};
use catalog::url::{self, CatalogUrl, ProgramTab};
use catalog::CatalogQuery;
use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use leptos_router::hooks::use_params_map;

use crate::data::{use_source, PageStatus};
use crate::format;
use crate::ui::{EmptyState, ErrorState, KindBadge, NotFound, OfferBadge};

#[component]
pub fn ProgramPage() -> impl IntoView {
    let params = use_params_map();
    let slug = Memo::new(move |_| params.read().get("slug").unwrap_or_default());
    // An unknown tab segment is a wrong address, not the default tab.
    let tab = Memo::new(move |_| match params.read().get("tab") {
        None => Some(ProgramTab::default()),
        Some(segment) => ProgramTab::from_segment(&segment),
    });
    let source = use_source();
    let status = PageStatus::capture();
    // The program is loaded once per slug; switching tabs only re-renders.
    let data = Memo::new(move |_| {
        let slug = slug.get();
        source.clone().and_then(|source| source.run(|db| pages::program(db, &slug)))
    });

    move || {
        let inner = match (data.get(), tab.get()) {
            (Err(error), _) => {
                status.for_error(&error);
                view! { <ErrorState error/> }.into_any()
            }
            (Ok(Some(data)), Some(tab)) => view! { <ProgramView data tab/> }.into_any(),
            _ => {
                status.set(404);
                view! { <NotFound title="Studiengang nicht gefunden" hint="Diesen Studiengang oder diese Ansicht gibt es nicht (mehr)."/> }.into_any()
            }
        };
        view! { <div class="page">{inner}</div> }
    }
}

#[component]
fn ProgramView(data: ProgramData, tab: ProgramTab) -> impl IntoView {
    let p = data.program.clone();
    let catalog_link = CatalogUrl {
        query: CatalogQuery {
            program: Some(ProgramScope { program_slug: p.slug.clone(), ..Default::default() }),
            ..Default::default()
        },
        page: 1,
        open: None,
    }
    .path();
    let description = format!(
        "{} ({}, PO {}) an der BTU Cottbus-Senftenberg: {} Module, Regelstudienplan, Wahlpflichtbereiche und Ordnungen.",
        p.name,
        p.degree(),
        p.po_version,
        p.curricular_modules
    );

    view! {
        <Title text=format!("{} · {}", p.name, p.degree())/>
        <Meta name="description" content=description/>
        <article class="page-inner" data-walk="program-page" data-walk-id=p.slug.clone()>
            <header class="panel page-head">
                <p class="eyebrow"><a href=url::PROGRAMS>"Studiengänge"</a>" / "{p.degree_level.label().to_string()}</p>
                <h1>{p.name.clone()}</h1>
                <p class="badges">
                    <span class="badge badge-strong">{p.degree().to_string()}</span>
                    <span class="badge">"PO "{p.po_version.clone()}</span>
                    {p.study_variant.as_ref().map(|v| view! { <span class="badge">{v.label().to_string()}</span> })}
                    {(!p.is_latest_po).then(|| view! { <span class="badge badge-phase_out">"ältere Prüfungsordnung"</span> })}
                </p>
                <dl class="facts">
                    <div class="fact"><dt>"Module im Curriculum"</dt><dd>{p.curricular_modules}</dd></div>
                    <div class="fact"><dt>"Anrechenbare FÜS-Module"</dt><dd>{p.fues_modules}</dd></div>
                    {data.counterpart.as_ref().map(|c| view! {
                        <div class="fact">
                            <dt>{format!("Passender {}", c.level.label())}</dt>
                            <dd><a href=url::program_path(&c.slug, ProgramTab::Plan)>{c.name.clone()}" (PO "{c.po_version.clone()}")"</a></dd>
                        </div>
                    })}
                    {(!data.versions.is_empty()).then(|| view! {
                        <div class="fact">
                            <dt>"Andere Prüfungsordnungen"</dt>
                            <dd class="inline-links">
                                {data.versions.iter().map(|v| view! {
                                    <a href=url::program_path(&v.slug, tab)>"PO "{v.po_version.clone()}{v.is_latest_po.then_some(" (aktuell)")}</a>
                                }).collect_view()}
                            </dd>
                        </div>
                    })}
                </dl>
                <p class="actions">
                    <a class="button" href=catalog_link>"Module dieses Studiengangs filtern"</a>
                    <a class="button button-quiet" href=p.source_url.clone() rel="noopener">"Im Vorlesungsverzeichnis der BTU"</a>
                </p>
            </header>

            <nav class="panel tabs" aria-label="Ansichten des Studiengangs">
                {ProgramTab::ALL.iter().map(|t| {
                    let active = *t == tab;
                    view! {
                        <a class="tab" class:active=active data-walk="tab" href=url::program_path(&p.slug, *t) aria-current=active.then_some("page")>
                            {t.label()}
                        </a>
                    }
                }).collect_view()}
            </nav>

            {match tab {
                ProgramTab::Plan => view! { <PlanTab data=data.clone()/> }.into_any(),
                ProgramTab::Areas => view! { <AreasTab areas=data.areas.clone()/> }.into_any(),
                ProgramTab::Modules => view! { <ModulesTab curricular=data.curricular.clone() fues=data.fues.clone()/> }.into_any(),
            }}

            {(!data.documents.is_empty()).then(|| view! {
                <section class="panel block">
                    <h2>"Ordnungen & Dokumente"</h2>
                    <ul class="list-plain">
                        {data.documents.iter().map(|d| view! {
                            <li><a href=d.url.clone() rel="noopener">{d.title.clone()}</a>" "<span class="badge">{d.doc_type.label().to_string()}</span></li>
                        }).collect_view()}
                    </ul>
                </section>
            })}
        </article>
    }
}

/// The validated study plan, semester by semester. Only what the plan states is shown.
#[component]
fn PlanTab(data: ProgramData) -> impl IntoView {
    if data.plan.is_none() {
        let reason = data
            .program
            .plan_status
            .as_ref()
            .map(|status| status.label().to_string())
            .unwrap_or_else(|| "Für diesen Studiengang liegt kein geprüfter Regelstudienplan vor".to_string());
        return view! {
            <EmptyState title=reason hint="Die Module findest du unter „Wahlpflicht & Bereiche“ und „Alle Module“. Fachsemester werden nur angezeigt, wenn ein Regelstudienplan sie nennt."/>
        }
        .into_any();
    }

    // Group by the semester the plan names: a single one, a span, or none.
    let mut groups: Vec<(String, Vec<PlanEntry>)> = Vec::new();
    for entry in data.plan_entries.iter().cloned() {
        let label = match (entry.semester, entry.start_semester, entry.end_semester, &entry.semester_span) {
            (Some(n), _, _, _) => format!("{n}. Semester"),
            (None, Some(a), Some(b), _) if a != b => format!("{a}.–{b}. Semester"),
            (None, _, _, Some(span)) => format!("Semester {span}"),
            _ => "Ohne Semesterangabe im Plan".to_string(),
        };
        match groups.iter_mut().find(|(existing, _)| *existing == label) {
            Some((_, entries)) => entries.push(entry),
            None => groups.push((label, vec![entry])),
        }
    }

    view! {
        <section class="panel block">
            <h2>"Regelstudienplan"</h2>
            <p class="hint">
                "Aus der Prüfungs- und Studienordnung übernommen und geprüft"
                {data.plan.as_ref().and_then(|plan| plan.validated_at.as_deref().map(|at| format!(" am {}", format::date(at))))}"."
            </p>
            <div class="plan">
                {groups.into_iter().map(|(label, entries)| {
                    let sum: f64 = entries.iter().filter_map(|e| e.credits).sum();
                    view! {
                        <section class="plan-semester">
                            <h3>{label}" "{(sum > 0.0).then(|| view! { <span class="tab-count">{format::number(sum)}" LP"</span> })}</h3>
                            <ul class="plan-entries">
                                {entries.into_iter().map(|e| {
                                    let credits = match (e.credits, e.min_credits, e.max_credits) {
                                        (Some(c), _, _) => Some(format!("{} LP", format::number(c))),
                                        (None, Some(min), Some(max)) => Some(format!("{}–{} LP", format::number(min), format::number(max))),
                                        _ => None,
                                    };
                                    view! {
                                        <li class="plan-entry">
                                            {match &e.module_id {
                                                Some(id) => view! { <a href=url::module_path(id)>{e.module_name.clone()}</a> }.into_any(),
                                                None => view! { <span>{e.module_name.clone()}</span> }.into_any(),
                                            }}
                                            <span class="plan-entry-meta">
                                                {credits.map(|c| view! { <span class="badge">{c}</span> })}
                                                {e.kind.clone().map(|kind| view! { <KindBadge kind=Some(kind)/> })}
                                                {e.credits_differ_from_catalog.then(|| view! { <span class="badge badge-unknown">"LP weichen vom Modulkatalog ab"</span> })}
                                            </span>
                                            {e.study_section.clone().or(e.subject_area.clone()).map(|area| view! { <span class="subtitle">{area}</span> })}
                                        </li>
                                    }
                                }).collect_view()}
                            </ul>
                        </section>
                    }
                }).collect_view()}
            </div>
        </section>
    }
    .into_any()
}

/// The module tree of the program: one section per area, chips to jump to an area.
#[component]
fn AreasTab(areas: Vec<AreaPlacement>) -> impl IntoView {
    if areas.is_empty() {
        return view! {
            <EmptyState title="Keine Bereiche bekannt" hint="Das Vorlesungsverzeichnis gliedert diesen Studiengang nicht in Bereiche."/>
        }
        .into_any();
    }
    let mut groups: Vec<(i64, String, String, i64, Vec<AreaPlacement>)> = Vec::new();
    for placement in areas {
        match groups.iter_mut().find(|(id, ..)| *id == placement.area_id) {
            Some((.., modules)) => modules.push(placement),
            None => groups.push((placement.area_id, placement.area_label.clone(), placement.area.clone(), placement.depth, vec![placement])),
        }
    }

    view! {
        <section class="panel block">
            <h2>"Wahlpflicht & Bereiche"</h2>
            <p class="chip-links">
                {groups.iter().map(|(id, label, _, _, modules)| view! {
                    <a class="chip" data-walk="area-chip" href=format!("#area-{id}")>{label.clone()}" "<span class="tab-count">{modules.len()}</span></a>
                }).collect_view()}
            </p>
            {groups.into_iter().map(|(id, label, path, depth, modules)| view! {
                <section class=format!("area area-depth-{}", depth.clamp(1, 4)) id=format!("area-{id}")>
                    <h3>{label}</h3>
                    <p class="subtitle">{path}</p>
                    <ul class="list-plain">
                        {modules.into_iter().map(|m| view! {
                            <li>
                                <a href=url::module_path(&m.module_id)>{m.module_id.clone()}" "{m.module_title.clone()}</a>
                                " "<span class="badge">{format::credits(m.module_credits)}</span>
                                " "<KindBadge kind=m.kind.clone()/>
                            </li>
                        }).collect_view()}
                    </ul>
                </section>
            }).collect_view()}
        </section>
    }
    .into_any()
}

#[component]
fn ModulesTab(curricular: Vec<ProgramModule>, fues: Vec<ProgramModule>) -> impl IntoView {
    let table = |modules: Vec<ProgramModule>, with_kind: bool| view! {
        <div class="table-scroll">
            <table class="modules">
                <thead>
                    <tr>
                        <th scope="col">"Nr."</th>
                        <th scope="col">"Modul"</th>
                        <th scope="col">"LP"</th>
                        <th scope="col">"Turnus"</th>
                        {with_kind.then(|| view! { <th scope="col">"Art"</th><th scope="col">"Semester"</th> })}
                    </tr>
                </thead>
                <tbody>
                    {modules.into_iter().map(|m| view! {
                        <tr>
                            <td class="cell-id">{m.module_id.clone()}</td>
                            <td class="cell-title">
                                <a href=url::module_path(&m.module_id)>{m.module_title.clone()}</a>
                                <span class="badges"><OfferBadge status=m.offer_status.clone()/></span>
                                {m.area.clone().map(|area| view! { <span class="subtitle">{area}</span> })}
                            </td>
                            <td class="cell-number">{m.module_credits.map(format::number)}</td>
                            <td>{format::turnus(m.turnus_season.as_ref(), None)}</td>
                            {with_kind.then(|| view! {
                                <td><KindBadge kind=m.kind.clone()/></td>
                                <td class="cell-number" class:unknown=m.plan_semester.is_none()>
                                    {m.plan_semester.map(|n| format!("{n}.")).unwrap_or_else(|| "–".to_string())}
                                </td>
                            })}
                        </tr>
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    };
    let fues_count = fues.len();
    let relation_hint = ProgramRelation::Fues.code();

    view! {
        <section class="panel block">
            <h2>"Curriculum "<span class="tab-count">{curricular.len()}</span></h2>
            {table(curricular, true)}
        </section>
        <section class="panel block" id=relation_hint>
            <h2>"Fachübergreifendes Studium (FÜS) "<span class="tab-count">{fues_count}</span></h2>
            <p class="hint">"Module, die in diesem Studiengang als FÜS angerechnet werden können. Sie gehören nicht zum Curriculum."</p>
            {if fues_count == 0 {
                view! { <p class="hint">"Für diesen Studiengang ist keine FÜS-Liste bekannt."</p> }.into_any()
            } else {
                view! { <details><summary>{fues_count}" FÜS-Module anzeigen"</summary>{table(fues, false)}</details> }.into_any()
            }}
        </section>
    }
}
