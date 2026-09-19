//! The module page: everything the module description states, its schedule and exams
//! per semester, and the programs it belongs to.

use catalog::labels::{OfferStatus, PrerequisiteKind, Relation, ResolveStatus, TextItemKind, TurnusSeason};
use catalog::pages::{self, ModuleData};
use catalog::rows_detail::EventDate;
use catalog::url::{self, ProgramTab};
use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use leptos_router::hooks::use_params_map;

use crate::data::{use_source, PageStatus};
use crate::format;
use crate::ui::{ErrorState, Fact, KindBadge, NotFound, OfferBadge, Prose};

#[component]
pub fn ModulePage() -> impl IntoView {
    let params = use_params_map();
    let id = Memo::new(move |_| params.read().get("id").unwrap_or_default());
    let source = use_source();
    let status = PageStatus::capture();
    // Queries are synchronous on both sides (rusqlite here, sql.js in the browser), so a page
    // is a plain function of its route parameters: no resources, nothing to serialize.
    move || {
        let id = id.get();
        match source.clone().and_then(|source| source.run(|db| pages::module(db, &id))) {
            Err(error) => {
                status.for_error(&error);
                view! { <ErrorState error/> }.into_any()
            }
            Ok(None) => {
                status.set(404);
                view! { <NotFound title="Modul nicht gefunden" hint="Dieses Modul steht nicht (mehr) im Modulkatalog der BTU."/> }.into_any()
            }
            Ok(Some(data)) => view! { <ModuleView data/> }.into_any(),
        }
    }
}

#[component]
fn ModuleView(data: ModuleData) -> impl IntoView {
    let m = data.module.clone();
    let description = m
        .contents
        .clone()
        .or_else(|| m.learning_outcomes.clone())
        .map(|text| text.chars().take(160).collect::<String>())
        .unwrap_or_else(|| format!("Modul {} der BTU Cottbus-Senftenberg", m.id));
    let other_title = match (&m.title_de, &m.title_en) {
        (Some(de), Some(en)) if de != en => Some(if m.title == *de { en.clone() } else { de.clone() }),
        _ => None,
    };
    let responsible: Vec<String> = data
        .lecturers
        .iter()
        .filter(|l| l.role.code() == "responsible")
        .map(|l| match &l.title {
            Some(title) => format!("{title} {}", l.name),
            None => l.name.clone(),
        })
        .collect();
    let campus: Vec<&str> = [
        (m.at_zentralcampus, "Zentralcampus Cottbus"),
        (m.at_sachsendorf, "Cottbus-Sachsendorf"),
        (m.at_senftenberg, "Senftenberg"),
    ]
    .iter()
    .filter(|(at, _)| *at == Some(true))
    .map(|(_, label)| *label)
    .collect();
    let literature: Vec<String> = data.text_items.iter().filter(|i| i.kind.is(TextItemKind::Literature)).map(|i| i.text.clone()).collect();
    let courses: Vec<String> = data.text_items.iter().filter(|i| i.kind.is(TextItemKind::Course)).map(|i| i.text.clone()).collect();
    let mandatory: Vec<_> = data.prerequisites.iter().filter(|p| p.kind.is(PrerequisiteKind::Mandatory)).cloned().collect();
    let recommended: Vec<_> = data.prerequisites.iter().filter(|p| p.kind.is(PrerequisiteKind::Recommended)).cloned().collect();

    let prerequisite_list = |title: &'static str, text: Option<String>, linked: Vec<catalog::rows::Prerequisite>| {
        (text.is_some() || !linked.is_empty()).then(|| view! {
            <section class="block">
                <h3>{title}</h3>
                {text.map(|text| view! { <Prose text/> })}
                {(!linked.is_empty()).then(|| view! {
                    <ul class="linklist">
                        {linked.into_iter().map(|p| view! {
                            <li>
                                <a href=url::module_path(&p.required_module_id)>
                                    {p.required_module_id.clone()}" "{p.required_title.clone().unwrap_or_default()}
                                </a>
                                {p.required_offer_status.map(|status| view! { <OfferBadge status/> })}
                            </li>
                        }).collect_view()}
                    </ul>
                })}
            </section>
        })
    };

    view! {
        <Title text=format!("{} {}", m.id, m.title)/>
        <Meta name="description" content=description/>
        <article class="detail">
            <header class="detail-header">
                <p class="eyebrow"><a href=url::CATALOG>"Modulkatalog"</a>" / Modul "{m.id.clone()}</p>
                <h1>{m.title.clone()}</h1>
                {other_title.map(|title| view! { <p class="subtitle">{title}</p> })}
                <p class="badges">
                    <span class="badge badge-strong">{format::credits(m.credits)}</span>
                    <span class="badge">{format::turnus(m.turnus_season.as_ref(), m.turnus_parity.as_ref())}</span>
                    {format::languages(m.teaches_german, m.teaches_english).map(|l| view! { <span class="badge">{l}</span> })}
                    <OfferBadge status=m.offer_status.clone()/>
                    {m.is_fues.then(|| view! { <span class="badge badge-fues">"FÜS"</span> })}
                </p>
            </header>

            {(!data.successors.is_empty()).then(|| view! {
                <aside class="notice">
                    {if m.offer_status.is(OfferStatus::Active) { "Dieses Modul hat ein Nachfolgemodul: " } else { "Dieses Modul wird abgelöst durch: " }}
                    {data.successors.iter().map(|s| view! {
                        <a href=url::module_path(&s.successor_id)>{s.successor_id.clone()}" "{s.successor_title.clone().unwrap_or_default()}</a>" "
                    }).collect_view()}
                </aside>
            })}

            <section class="block">
                <h2>"Auf einen Blick"</h2>
                <dl class="facts">
                    <Fact label="Leistungspunkte" value=m.credits.map(|c| format::credits(Some(c)))/>
                    <Fact label="Turnus" value=m.turnus_raw.clone()/>
                    <Fact label="Dauer" value=m.duration_raw.clone()/>
                    <Fact label="Sprache" value=m.language_raw.clone()/>
                    <Fact label="Prüfungsform" value=m.exam_form.as_ref().map(|f| f.label().to_string()).or(m.exam_form_raw.clone())/>
                    <Fact label="Benotung" value=m.is_graded.map(|g| if g { "benotet".to_string() } else { "unbenotet".to_string() }).or(m.grading_raw.clone())/>
                    <Fact label="Teilnehmerbegrenzung" value=match (m.is_limited, m.participant_limit) {
                        (Some(true), Some(n)) => Some(format!("max. {n} Teilnehmende")),
                        (Some(true), None) => m.limitation_raw.clone().or(Some("begrenzt".to_string())),
                        (Some(false), _) => Some("keine".to_string()),
                        (None, _) => None,
                    }/>
                    <Fact label="Fachgebiet" value=m.department.clone()/>
                    <Fact label="Modulverantwortung" value=(!responsible.is_empty()).then(|| responsible.join("; "))/>
                    <Fact label="Standort" value=(!campus.is_empty()).then(|| campus.join(", "))/>
                </dl>
            </section>

            <Schedule data=data.clone()/>

            {m.learning_outcomes.clone().map(|text| view! { <section class="block"><h2>"Lernziele"</h2><Prose text/></section> })}
            {m.contents.clone().map(|text| view! { <section class="block"><h2>"Inhalte"</h2><Prose text/></section> })}

            {(m.prerequisites_mandatory.is_some() || m.prerequisites_recommended.is_some() || !data.prerequisites.is_empty()).then(|| view! {
                <section class="block">
                    <h2>"Voraussetzungen"</h2>
                    {prerequisite_list("Zwingend", m.prerequisites_mandatory.clone(), mandatory)}
                    {prerequisite_list("Empfohlen", m.prerequisites_recommended.clone(), recommended)}
                </section>
            })}

            <section class="block">
                <h2>"Lehre & Prüfung"</h2>
                {(!data.teaching_forms.is_empty()).then(|| view! {
                    <table class="plain">
                        <thead><tr><th scope="col">"Lehrform"</th><th scope="col">"Umfang"</th></tr></thead>
                        <tbody>
                            {data.teaching_forms.iter().map(|f| view! {
                                <tr><td>{f.form_raw.clone()}</td><td>{f.workload_raw.clone().unwrap_or_else(|| "–".to_string())}</td></tr>
                            }).collect_view()}
                        </tbody>
                    </table>
                })}
                {m.exam_details.clone().map(|text| view! { <h3>"Prüfungsleistung"</h3><Prose text/> })}
                {(!courses.is_empty()).then(|| view! {
                    <h3>"Lehrveranstaltungen laut Modulbeschreibung"</h3>
                    <ul>{courses.into_iter().map(|c| view! { <li>{c}</li> }).collect_view()}</ul>
                })}
            </section>

            {(!literature.is_empty()).then(|| view! {
                <section class="block">
                    <h2>"Literatur"</h2>
                    <ul class="literature">{literature.into_iter().map(|l| view! { <li>{l}</li> }).collect_view()}</ul>
                </section>
            })}
            {m.remarks.clone().map(|text| view! { <section class="block"><h2>"Bemerkungen"</h2><Prose text/></section> })}

            <Programs data=data.clone()/>

            <footer class="detail-footer">
                {m.source_url.clone().map(|href| view! { <a href=href rel="noopener">"Modulbeschreibung bei der BTU öffnen"</a> })}
                {m.fetched_at.as_deref().map(|at| format!(" · abgerufen am {}", format::date(at)))}
            </footer>
        </article>
    }
}

/// Teaching events and exams of the newest semester that has any; older semesters are not kept.
#[component]
fn Schedule(data: ModuleData) -> impl IntoView {
    let m = &data.module;
    let newest = data.schedule.first().map(|d| (d.semester_key.clone(), d.semester_label.clone()));
    let current = data.semesters.iter().find(|s| s.is_current);
    let upcoming = data.semesters.iter().find(|s| current.is_some_and(|c| s.key > c.key));

    // The gap before the BTU publishes the next semester (owner decision Q8).
    let gap_note = match (&newest, upcoming) {
        (Some((key, label)), Some(next)) if *key < next.key => Some(format!(
            "Termine aus dem {label}. Für das {} hat die BTU noch keine Termine zu diesem Modul veröffentlicht.",
            next.label
        )),
        _ => None,
    };
    let no_schedule_note = newest.is_none().then(|| match m.turnus_season.as_ref().and_then(|s| s.known()) {
        Some(TurnusSeason::Winter) => "Noch keine Termine veröffentlicht. Laut Modulbeschreibung wird das Modul im Wintersemester angeboten.",
        Some(TurnusSeason::Summer) => "Noch keine Termine veröffentlicht. Laut Modulbeschreibung wird das Modul im Sommersemester angeboten.",
        Some(TurnusSeason::Both) => "Noch keine Termine veröffentlicht. Laut Modulbeschreibung wird das Modul jedes Semester angeboten.",
        _ => "Zu diesem Modul sind keine Termine veröffentlicht.",
    });

    let table = |dates: Vec<EventDate>, with_type: bool| view! {
        <div class="table-scroll">
            <table class="plain">
                <thead>
                    <tr>
                        <th scope="col">"Veranstaltung"</th>
                        <th scope="col">"Zeit"</th>
                        <th scope="col">"Rhythmus / Datum"</th>
                        <th scope="col">"Raum"</th>
                        <th scope="col">"Lehrende"</th>
                    </tr>
                </thead>
                <tbody>
                    {dates.into_iter().map(|d| {
                        let when = match (&d.first_date, &d.last_date) {
                            (Some(first), Some(last)) if first != last => Some(format!("{} – {}", format::date(first), format::date(last))),
                            (Some(first), _) => Some(format::date(first)),
                            _ => None,
                        };
                        let rhythm = d.rhythm.as_ref().map(|r| r.label().to_string()).or(d.rhythm_raw.clone());
                        let detail = [rhythm, when].into_iter().flatten().collect::<Vec<_>>().join(", ");
                        view! {
                            <tr>
                                <td>
                                    {match &d.source_url {
                                        Some(href) => view! { <a href=href.clone() rel="noopener">{d.event_title.clone()}</a> }.into_any(),
                                        None => view! { <span>{d.event_title.clone()}</span> }.into_any(),
                                    }}
                                    {with_type.then(|| d.event_type.clone().map(|t| view! { <span class="subtitle">{t}{d.group_name.clone().map(|g| format!(" · {g}"))}</span> }))}
                                    {d.comment.clone().map(|c| view! { <span class="subtitle">{c}</span> })}
                                </td>
                                <td>{format::time_slot(d.weekday, d.start_time.as_deref(), d.end_time.as_deref()).unwrap_or_else(|| "–".to_string())}</td>
                                <td>{if detail.is_empty() { "–".to_string() } else { detail }}</td>
                                <td>{d.room.clone().unwrap_or_else(|| "–".to_string())}</td>
                                <td>{d.instructor.clone().unwrap_or_else(|| "–".to_string())}</td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    };

    let newest_key = newest.as_ref().map(|(key, _)| key.clone());
    let teaching: Vec<EventDate> = data.schedule.iter().filter(|d| Some(&d.semester_key) == newest_key.as_ref()).cloned().collect();
    let exam_semester = data.exams.first().map(|d| (d.semester_key.clone(), d.semester_label.clone()));
    let exams: Vec<EventDate> = data.exams.iter().filter(|d| exam_semester.as_ref().is_some_and(|(key, _)| *key == d.semester_key)).cloned().collect();

    view! {
        <section class="block">
            <h2>"Termine"{newest.as_ref().map(|(_, label)| format!(" · {label}"))}</h2>
            {gap_note.map(|note| view! { <p class="hint">{note}</p> })}
            {no_schedule_note.map(|note| view! { <p class="hint">{note}</p> })}
            {(!teaching.is_empty()).then(|| table(teaching, true))}
            {exam_semester.map(|(_, label)| view! {
                <h3>"Prüfungstermine · "{label}</h3>
                {table(exams, false)}
            })}
        </section>
    }
}

/// The programs the module belongs to: curricula first, then the programs that accept it as FÜS.
#[component]
fn Programs(data: ModuleData) -> impl IntoView {
    let resolved: Vec<_> = data.programs.iter().filter(|l| l.resolve_status.is(ResolveStatus::Resolved)).cloned().collect();
    let curricular: Vec<_> = resolved.iter().filter(|l| l.relation.as_ref().is_some_and(|r| r.is(Relation::Curricular))).cloned().collect();
    let fues = resolved.iter().filter(|l| l.relation.as_ref().is_some_and(|r| r.is(Relation::Fues))).count();
    let unresolved = data.programs.iter().filter(|l| l.resolve_status.is(ResolveStatus::Unresolved)).count();

    (!data.programs.is_empty()).then(|| view! {
        <section class="block">
            <h2>"Studiengänge"</h2>
            {if curricular.is_empty() {
                view! { <p class="hint">"Das Modul gehört zu keinem Curriculum eines Studiengangs im Katalog."</p> }.into_any()
            } else {
                view! {
                    <ul class="linklist">
                        {curricular.into_iter().map(|link| {
                            let slug = link.program_slug.clone().unwrap_or_default();
                            view! {
                                <li>
                                    <a href=url::program_path(&slug, ProgramTab::Modules)>
                                        {link.program_name.clone().unwrap_or_default()}" · "
                                        {link.degree_display.clone().or(link.degree_raw.clone()).unwrap_or_default()}
                                        " · PO "{link.po_version.clone().unwrap_or_default()}
                                    </a>
                                    " "<KindBadge kind=link.kind.clone()/>
                                    {link.area.clone().map(|area| view! { <span class="subtitle">{area}</span> })}
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                }.into_any()
            }}
            {(fues > 0).then(|| view! {
                <p class="hint">"Außerdem als fachübergreifendes Studium (FÜS) anrechenbar in "{fues}" Studiengängen."</p>
            })}
            {(unresolved > 0).then(|| view! {
                <p class="hint">{unresolved}" weitere Nennungen in der Modulbeschreibung gehören zu Studiengängen, die nicht im Katalog stehen."</p>
            })}
        </section>
    })
}
