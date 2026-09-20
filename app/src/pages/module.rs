//! A module in two sizes: the preview panel next to the catalog list (`/catalog?…&open=<id>`)
//! and the module's own page (`/catalog/module/<id>`), which uses the whole screen.

use catalog::labels::{OfferStatus, PrerequisiteKind, Relation, ResolveStatus, TeachingForm, TextItemKind, TurnusSeason};
use catalog::pages::{self, ModuleData};
use catalog::rows::Prerequisite;
use catalog::rows_detail::EventDate;
use catalog::url::{self, ProgramTab};
use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use leptos_router::hooks::use_params_map;

use crate::data::{use_source, PageStatus};
use crate::format;
use crate::ui::{ErrorState, Fact, Icon, KindBadge, NotFound, OfferBadge, Prose, Shortcut};

/// What both the preview panel and the full page show about a module, precomputed once.
#[derive(Clone)]
struct Derived {
    other_title: Option<String>,
    responsible: Vec<String>,
    campus: Vec<&'static str>,
    workload: Vec<String>,
    literature: Vec<String>,
    mandatory: Vec<Prerequisite>,
    recommended: Vec<Prerequisite>,
    description: String,
}

fn derive(data: &ModuleData) -> Derived {
    let m = &data.module;
    Derived {
        other_title: match (&m.title_de, &m.title_en) {
            (Some(de), Some(en)) if de != en => Some(if m.title == *de { en.clone() } else { de.clone() }),
            _ => None,
        },
        responsible: data
            .lecturers
            .iter()
            .filter(|l| l.role.code() == "responsible")
            .map(|l| match &l.title {
                Some(title) => format!("{title} {}", l.name),
                None => l.name.clone(),
            })
            .collect(),
        campus: [
            (m.at_zentralcampus, "Zentralcampus Cottbus"),
            (m.at_sachsendorf, "Cottbus-Sachsendorf"),
            (m.at_senftenberg, "Senftenberg"),
        ]
        .iter()
        .filter(|(at, _)| *at == Some(true))
        .map(|(_, label)| *label)
        .collect(),
        workload: data
            .teaching_forms
            .iter()
            .filter(|f| !f.form.is(TeachingForm::SelfStudy))
            .map(|f| match &f.workload_raw {
                Some(raw) => format!("{} {raw}", f.form_raw),
                None => f.form_raw.clone(),
            })
            .collect(),
        literature: data.text_items.iter().filter(|i| i.kind.is(TextItemKind::Literature)).map(|i| i.text.clone()).collect(),
        mandatory: data.prerequisites.iter().filter(|p| p.kind.is(PrerequisiteKind::Mandatory)).cloned().collect(),
        recommended: data.prerequisites.iter().filter(|p| p.kind.is(PrerequisiteKind::Recommended)).cloned().collect(),
        description: m
            .contents
            .clone()
            .or_else(|| m.learning_outcomes.clone())
            .map(|text| text.chars().take(160).collect::<String>())
            .unwrap_or_else(|| format!("Modul {} der BTU Cottbus-Senftenberg", m.id)),
    }
}

/// The preview next to the catalog list. `close_href` is the same list without the preview.
#[component]
pub fn ModulePanel(data: ModuleData, close_href: String) -> impl IntoView {
    let id = data.module.id.clone();
    view! {
        <section class="panel detail" id="preview" aria-label="Modulvorschau">
            <div class="scroll" data-keep-scroll="detail">
                <header class="hero">
                    <div class="hero-top">
                        <a class="icon-btn back" href=close_href.clone() aria-label="Zurück zur Liste"><Icon name="arrow-left"/></a>
                        <span class="mono">{id.clone()}</span>
                        <a class="ghost" href=url::module_path(&id) data-action="fullscreen" title="Als ganze Seite öffnen (F)"><Icon name="maximize-2"/>"Vollbild"<Shortcut keys="F"/></a>
                        <a class="ghost" href=close_href data-action="close-detail" title="Vorschau schließen (Esc)"><Icon name="x"/>"Schließen"<Shortcut keys="Esc"/></a>
                    </div>
                    <Heading data=data.clone()/>
                </header>
                <div class="dbody">
                    <Side data=data.clone()/>
                    <Main data=data.clone()/>
                    <Source data=data.clone()/>
                </div>
            </div>
        </section>
    }
}

/// The module's own page (`/catalog/module/<id>`): the whole screen, two columns.
#[component]
pub fn ModulePage() -> impl IntoView {
    let params = use_params_map();
    let id = Memo::new(move |_| params.read().get("id").unwrap_or_default());
    let source = use_source();
    let status = PageStatus::capture();

    move || {
        let id = id.get();
        let inner = match source.clone().and_then(|source| source.run(|db| pages::module(db, &id))) {
            Err(error) => {
                status.for_error(&error);
                view! { <ErrorState error/> }.into_any()
            }
            Ok(None) => {
                status.set(404);
                view! { <NotFound title="Modul nicht gefunden" hint="Dieses Modul steht nicht (mehr) im Modulkatalog der BTU."/> }.into_any()
            }
            Ok(Some(data)) => {
                let derived = derive(&data);
                view! {
                    <Title text=format!("{} {}", data.module.id, data.module.title)/>
                    <Meta name="description" content=derived.description/>
                    <article class="module-page">
                        <header class="panel hero">
                            <div class="hero-top">
                                <a class="ghost" href=url::CATALOG data-action="back" title="Zurück (Esc)"><Icon name="arrow-left"/>"Zurück"<Shortcut keys="Esc"/></a>
                                <span class="mono">{data.module.id.clone()}</span>
                            </div>
                            <Heading data=data.clone()/>
                        </header>
                        <div class="module-grid">
                            <div class="panel dbody"><Main data=data.clone()/><Source data=data.clone()/></div>
                            <aside class="panel dbody"><Side data=data.clone()/></aside>
                        </div>
                    </article>
                }
                .into_any()
            }
        };
        view! { <div class="page">{inner}</div> }
    }
}

#[component]
fn Heading(data: ModuleData) -> impl IntoView {
    let m = data.module.clone();
    let derived = derive(&data);
    view! {
        <h2>{m.title.clone()}</h2>
        {derived.other_title.map(|title| view! { <p class="en">{title}</p> })}
        <p class="badges">
            <span class="badge strong num">{format::credits(m.credits)}</span>
            <span class="badge">{format::turnus(m.turnus_season.as_ref(), m.turnus_parity.as_ref())}</span>
            {format::languages(m.teaches_german, m.teaches_english).map(|l| view! { <span class="badge">{l}</span> })}
            {m.is_fues.then(|| view! { <span class="badge">"FÜS"</span> })}
            {(!m.offer_status.is(OfferStatus::Active)).then(|| view! { <span class="badge warn">{m.offer_status.label().to_string()}</span> })}
        </p>
    }
}

/// Schedule, key facts and programs: the right column of the page, the top of the preview.
#[component]
fn Side(data: ModuleData) -> impl IntoView {
    let m = data.module.clone();
    let derived = derive(&data);
    view! {
        {(!data.successors.is_empty()).then(|| view! {
            <p class="note">
                <Icon name="info"/>
                <span>
                    {if m.offer_status.is(OfferStatus::Active) { "Nachfolgemodul: " } else { "Wird abgelöst durch: " }}
                    {data.successors.iter().map(|s| view! {
                        <a href=url::module_path(&s.successor_id)>{s.successor_id.clone()}" "{s.successor_title.clone().unwrap_or_default()}</a>" "
                    }).collect_view()}
                </span>
            </p>
        })}
        <Schedule data=data.clone()/>
        <div class="section">
            <p class="label">"Auf einen Blick"</p>
            <dl class="facts">
                <Fact icon="file-check-2" label="Prüfung" value=m.exam_form.as_ref().map(format::exam_short).or(m.exam_form_raw.clone())/>
                <Fact icon="award" label="Benotung" value=m.is_graded.map(|g| if g { "benotet".to_string() } else { "unbenotet".to_string() }).or(m.grading_raw.clone())/>
                <Fact icon="clock-3" label="Dauer" value=m.duration_raw.clone()/>
                <Fact icon="users-round" label="Plätze" value=match (m.is_limited, m.participant_limit) {
                    (Some(true), Some(n)) => Some(format!("max. {n}")),
                    (Some(true), None) => m.limitation_raw.clone().or(Some("begrenzt".to_string())),
                    (Some(false), _) => Some("unbegrenzt".to_string()),
                    (None, _) => None,
                }/>
                <Fact icon="languages" label="Sprache" value=m.language_raw.clone()/>
                <Fact icon="map-pin" label="Standort" value=(!derived.campus.is_empty()).then(|| derived.campus.join(", "))/>
                <Fact wide=true icon="user-round" label="Verantwortlich" value=(!derived.responsible.is_empty()).then(|| derived.responsible.join("; "))/>
                <Fact wide=true icon="layout-list" label="Lehrformen" value=(!derived.workload.is_empty()).then(|| derived.workload.join(" · "))/>
            </dl>
        </div>
        <Programs data=data.clone()/>
    }
}

/// Prerequisites and the texts of the module description: the main column of the page.
#[component]
fn Main(data: ModuleData) -> impl IntoView {
    let m = data.module.clone();
    let derived = derive(&data);
    let has_prerequisites = m.prerequisites_mandatory.is_some() || m.prerequisites_recommended.is_some() || !data.prerequisites.is_empty();
    let literature = derived.literature.clone();
    let prerequisite_links = |linked: Vec<Prerequisite>, kind: &'static str| {
        linked
            .into_iter()
            .map(|p| view! {
                <a class="pre" href=url::module_path(&p.required_module_id)>
                    <span class="mono">{p.required_module_id.clone()}</span>
                    <b>{p.required_title.clone().unwrap_or_default()}</b>
                    {p.required_offer_status.map(|status| view! { <OfferBadge status/> })}
                    <small>{kind}</small>
                    <Icon name="chevron-right"/>
                </a>
            })
            .collect_view()
    };
    view! {
        {has_prerequisites.then(|| view! {
            <div class="section">
                <p class="label">"Voraussetzungen"</p>
                <div class="linklist">
                    {prerequisite_links(derived.mandatory.clone(), "zwingend")}
                    {prerequisite_links(derived.recommended.clone(), "empfohlen")}
                </div>
                {m.prerequisites_mandatory.clone().map(|text| view! { <details class="more"><summary>"Zwingend, im Wortlaut"</summary><Prose text/></details> })}
                {m.prerequisites_recommended.clone().map(|text| view! { <details class="more"><summary>"Empfohlen, im Wortlaut"</summary><Prose text/></details> })}
            </div>
        })}
        {m.contents.clone().map(|text| view! { <div class="section"><p class="label">"Inhalte"</p><Prose text/></div> })}
        {m.learning_outcomes.clone().map(|text| view! { <div class="section"><p class="label">"Lernziele"</p><Prose text/></div> })}
        {m.exam_details.clone().map(|text| view! { <div class="section"><p class="label">"Prüfungsleistung"</p><Prose text/></div> })}
        {(!literature.is_empty()).then(|| view! {
            <div class="section">
                <details class="more"><summary>"Literatur ("{literature.len()}")"</summary>
                    <ul class="list-plain">{literature.into_iter().map(|l| view! { <li>{l}</li> }).collect_view()}</ul>
                </details>
            </div>
        })}
        {m.remarks.clone().map(|text| view! { <div class="section"><p class="label">"Bemerkungen"</p><Prose text/></div> })}
    }
}

#[component]
fn Source(data: ModuleData) -> impl IntoView {
    let m = data.module;
    view! {
        <p class="source">
            <Icon name="shield-check"/>
            "Quelle: Modulbeschreibung der BTU"
            {m.fetched_at.as_deref().map(|at| format!(" · abgerufen {}", format::date(at)))}
            {m.source_url.clone().map(|href| view! { <a href=href rel="noopener">"Original"<Icon name="arrow-up-right"/></a> })}
        </p>
    }
}

/// Teaching events of the newest semester that has any, as a week grid plus a list; exams below.
#[component]
fn Schedule(data: ModuleData) -> impl IntoView {
    let m = &data.module;
    let newest = data.schedule.first().map(|d| (d.semester_key.clone(), d.semester_label.clone()));
    let current = data.semesters.iter().find(|s| s.is_current);
    let upcoming = data.semesters.iter().find(|s| current.is_some_and(|c| s.key > c.key));

    // The gap before the BTU publishes the next semester (owner decision Q8).
    let gap_note = match (&newest, upcoming) {
        (Some((key, label)), Some(next)) if *key < next.key => {
            Some(format!("Termine aus dem {label}. Für das {} hat die BTU noch keine Termine zu diesem Modul veröffentlicht.", next.label))
        }
        _ => None,
    };
    let no_schedule_note = newest.is_none().then(|| match m.turnus_season.as_ref().and_then(|s| s.known()) {
        Some(TurnusSeason::Winter) => "Noch keine Termine veröffentlicht. Laut Modulbeschreibung wird das Modul im Wintersemester angeboten.",
        Some(TurnusSeason::Summer) => "Noch keine Termine veröffentlicht. Laut Modulbeschreibung wird das Modul im Sommersemester angeboten.",
        Some(TurnusSeason::Both) => "Noch keine Termine veröffentlicht. Laut Modulbeschreibung wird das Modul jedes Semester angeboten.",
        _ => "Zu diesem Modul sind keine Termine veröffentlicht.",
    });

    let newest_key = newest.as_ref().map(|(key, _)| key.clone());
    let teaching: Vec<EventDate> = data.schedule.iter().filter(|d| Some(&d.semester_key) == newest_key.as_ref()).cloned().collect();
    let exam_semester = data.exams.first().map(|d| (d.semester_key.clone(), d.semester_label.clone()));
    let exams: Vec<EventDate> = data.exams.iter().filter(|d| exam_semester.as_ref().is_some_and(|(key, _)| *key == d.semester_key)).cloned().collect();

    // Week grid: only dates with a weekday and both times, Monday to Friday (Saturday if used).
    let slots: Vec<(i64, f64, f64, EventDate)> = teaching
        .iter()
        .filter_map(|d| {
            let from = format::half_hours(d.start_time.as_deref()?)?;
            let to = format::half_hours(d.end_time.as_deref()?)?;
            let day = d.weekday.filter(|day| (1..=6).contains(day))?;
            (to > from).then(|| (day, from, to, d.clone()))
        })
        .collect();
    let days: i64 = if slots.iter().any(|(day, ..)| *day == 6) { 6 } else { 5 };
    let first = slots.iter().map(|(_, from, ..)| (from / 2.0).floor() * 2.0).fold(f64::INFINITY, f64::min).min(16.0);
    let last = slots.iter().map(|(_, _, to, _)| (to / 2.0).ceil() * 2.0).fold(0.0, f64::max).max(first + 8.0);
    let span = last - first;
    let day_names = ["Mo", "Di", "Mi", "Do", "Fr", "Sa"];

    let event_list = |dates: Vec<EventDate>| {
        dates
            .into_iter()
            .map(|d| {
                let when = format::time_slot(d.weekday, d.start_time.as_deref(), d.end_time.as_deref());
                let dates = match (&d.first_date, &d.last_date) {
                    (Some(first), Some(last)) if first != last => Some(format!("{} – {}", format::date(first), format::date(last))),
                    (Some(first), _) => Some(format::date(first)),
                    _ => None,
                };
                let rhythm = d.rhythm.as_ref().map(|r| r.label().to_string()).or(d.rhythm_raw.clone());
                let details: Vec<String> = [d.event_type.clone(), d.group_name.clone(), rhythm, dates, d.room.clone(), d.instructor.clone(), d.comment.clone()]
                    .into_iter()
                    .flatten()
                    .collect();
                let body = view! {
                    <span class="when">{when.unwrap_or_else(|| "Zeit offen".to_string())}</span>
                    <b>{d.event_title.clone()}</b>
                    <small>{details.join(" · ")}</small>
                };
                match d.source_url.clone() {
                    Some(href) => view! { <a class="ev" href=href rel="noopener">{body}</a> }.into_any(),
                    None => view! { <div class="ev">{body}</div> }.into_any(),
                }
            })
            .collect_view()
    };

    view! {
        <div class="section">
            <p class="label">"Termine"{newest.as_ref().map(|(_, label)| view! { <span>{label.clone()}</span> })}</p>
            {(!slots.is_empty()).then(|| view! {
                <div class="week" style=format!("--days:{days};--first:{first};--span:{span}")>
                    <span></span>
                    {day_names.iter().take(days as usize).map(|name| view! { <span class="d">{*name}</span> }).collect_view()}
                    <div class="hours">
                        {(0..(span as i64 / 2)).map(|i| view! { <span>{(first as i64 / 2) + i}</span> }).collect_view()}
                    </div>
                    {(1..=days).map(|day| view! {
                        <div class="col">
                            {slots.iter().filter(|(d, ..)| *d == day).map(|(_, from, to, date)| {
                                let lecture = date.event_type.as_deref().is_some_and(|t| t.to_lowercase().contains("vorlesung"));
                                view! {
                                    <div class="slot" class:other=!lecture style=format!("--from:{from};--to:{to}") title=date.event_title.clone()>
                                        {date.event_type.clone().unwrap_or_else(|| "Termin".to_string())}
                                        <small>{date.start_time.clone()}</small>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    }).collect_view()}
                </div>
            })}
            {gap_note.map(|note| view! { <p class="note"><Icon name="info"/><span>{note}</span></p> })}
            {no_schedule_note.map(|note| view! { <p class="hint">{note}</p> })}
            {(!teaching.is_empty()).then(|| view! { <div class="evlist">{event_list(teaching)}</div> })}
        </div>
        {exam_semester.map(|(_, label)| view! {
            <div class="section">
                <p class="label">"Prüfungstermine"<span>{label}</span></p>
                <div class="evlist">{event_list(exams)}</div>
            </div>
        })}
    }
}

/// The programs the module belongs to: curricula first, then how many accept it as FÜS.
#[component]
fn Programs(data: ModuleData) -> impl IntoView {
    let resolved: Vec<_> = data.programs.iter().filter(|l| l.resolve_status.is(ResolveStatus::Resolved)).cloned().collect();
    let curricular: Vec<_> = resolved.iter().filter(|l| l.relation.as_ref().is_some_and(|r| r.is(Relation::Curricular))).cloned().collect();
    let fues = resolved.iter().filter(|l| l.relation.as_ref().is_some_and(|r| r.is(Relation::Fues))).count();
    let unresolved = data.programs.iter().filter(|l| l.resolve_status.is(ResolveStatus::Unresolved)).count();

    (!data.programs.is_empty()).then(|| view! {
        <div class="section">
            <p class="label">"Studiengänge"<span>{curricular.len()}" Curricula"</span></p>
            {curricular.is_empty().then(|| view! { <p class="hint">"Das Modul gehört zu keinem Curriculum eines Studiengangs im Katalog."</p> })}
            <div class="linklist">
                {curricular.into_iter().map(|link| {
                    let slug = link.program_slug.clone().unwrap_or_default();
                    view! {
                        <a class="pre" href=url::program_path(&slug, ProgramTab::Modules)>
                            <b>
                                {link.program_name.clone().unwrap_or_default()}" · "
                                {link.degree_display.clone().or(link.degree_raw.clone()).unwrap_or_default()}
                                <span class="subtitle">"PO "{link.po_version.clone().unwrap_or_default()}{link.area.clone().map(|area| format!(" · {area}"))}</span>
                            </b>
                            <small><KindBadge kind=link.kind.clone()/></small>
                            <Icon name="chevron-right"/>
                        </a>
                    }
                }).collect_view()}
            </div>
            {(fues > 0).then(|| view! { <p class="hint">"Außerdem als fachübergreifendes Studium (FÜS) anrechenbar in "{fues}" Studiengängen."</p> })}
            {(unresolved > 0).then(|| view! { <p class="hint">{unresolved}" weitere Nennungen gehören zu Studiengängen, die nicht im Katalog stehen."</p> })}
        </div>
    })
}
