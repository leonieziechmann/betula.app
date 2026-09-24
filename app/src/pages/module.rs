//! A module in two sizes: the preview panel next to the catalog list (`/catalog?…&open=<id>`)
//! and the module's own page (`/catalog/module/<id>`), which uses the whole screen. Both are made
//! of the same parts in the same order (times and key facts, then the description), so a module
//! reads the same wherever it is opened; a wide page puts the two halves side by side.
//!
//! The page has the same frame as the catalog: a sidebar as wide as the filter panel (same
//! handle, same remembered width), so nothing jumps when a module goes from preview to page.
//! The sidebar holds what belongs to the module as a whole: where to go on the page, and what
//! to do with the module.
//!
//! The page is one component (`ModuleFull`) wherever it is shown: at the module's own address,
//! and inside an area that shows its modules in place (`crate::local`: a program's page, the
//! marked modules — `…&open=<id>&full=1`, and on a phone whatever `open` names), where „Vollbild"
//! must neither change the area nor the tab.

use std::collections::BTreeSet;

use catalog::exam_reading::{self, ExamReading, Reason, Slot};
use catalog::labels::{OfferStatus, PrerequisiteKind, Relation, ResolveStatus, Rhythm, TeachingForm, TextItemKind, TurnusSeason};
use catalog::pages::{self, ModuleData};
use catalog::rows::Prerequisite;
use catalog::rows_detail::EventDate;
use catalog::timetable::day::{clock, minutes, Day};
use catalog::timetable::kind::{class_of, kinds_of, Class};
use catalog::url::{self, ProgramTab};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::{use_location, use_params_map};

use crate::bookmarks::{MarkButton, MarkLook};
use crate::data::{use_source, PageStatus};
use crate::format;
use crate::seo::{self, Seo};
use crate::tabs::{self, Area, Tabs};
use crate::ui::{BackLink, ErrorState, Fact, Frame, Icon, JsOnly, KindBadge, NotFound, OfferBadge, Prose, Shortcut};
use crate::week::{GridSlot, WeekGrid};

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
            .map(|text| seo::excerpt(&format!("{} ({}, {}) an der BTU Cottbus-Senftenberg: {text}", m.title, m.id, format::credits(m.credits)), 300))
            .unwrap_or_else(|| format!("{} (Modul {}, {}) an der BTU Cottbus-Senftenberg: Turnus, Prüfung, Voraussetzungen und Studiengänge.", m.title, m.id, format::credits(m.credits))),
    }
}

/// The module as schema.org knows it (a `Course` of the university) and the way to it. Only what
/// the page shows.
fn structured(data: &ModuleData) -> Vec<serde_json::Value> {
    let m = &data.module;
    let mut course = serde_json::json!({
        "@type": "Course",
        "@id": seo::absolute(&url::module_path(&m.id)),
        "url": seo::absolute(&url::module_path(&m.id)),
        "name": m.title,
        "courseCode": m.id,
        "provider": { "@type": "CollegeOrUniversity", "name": seo::UNIVERSITY, "url": seo::UNIVERSITY_URL },
    });
    let languages: Vec<&str> = [(m.teaches_german, "de"), (m.teaches_english, "en")].iter().filter(|(taught, _)| *taught == Some(true)).map(|(_, code)| *code).collect();
    if let Some(course) = course.as_object_mut() {
        if let Some(text) = m.contents.as_ref().or(m.learning_outcomes.as_ref()) {
            course.insert("description".into(), seo::excerpt(text, 500).into());
        }
        if let Some(credits) = m.credits {
            course.insert("numberOfCredits".into(), serde_json::json!({ "@type": "StructuredValue", "value": credits, "unitText": "ECTS" }));
        }
        if !languages.is_empty() {
            course.insert("inLanguage".into(), languages.into());
        }
        if let Some(source) = &m.source_url {
            course.insert("sameAs".into(), source.clone().into());
        }
    }
    vec![
        course,
        seo::breadcrumbs(&[("Betula", url::HOME.to_string()), ("Modulkatalog", url::CATALOG.to_string()), (m.title.as_str(), url::module_path(&m.id))]),
    ]
}

/// The preview next to a list. `close_href` is the same page without the preview. `docked` gives
/// it the head of a frame's panel (`ui::Frame`); it floats over the page either way.
/// `full_href` is where „Vollbild" leads: the module's own page unless the page beside which the
/// module stands can show it in full itself (a program's page).
#[component]
pub fn ModulePanel(data: ModuleData, close_href: String, #[prop(optional)] docked: bool, #[prop(optional_no_strip)] full_href: Option<String>) -> impl IntoView {
    let id = data.module.id.clone();
    let full_href = full_href.unwrap_or_else(|| url::module_path(&id));
    view! {
        <section class="panel detail" class:aside=docked id="preview" aria-label="Modulvorschau">
            <div class="scroll" data-keep-scroll="detail">
                <header class="hero">
                    <div class="hero-top">
                        <a class="icon-btn back" href=close_href.clone() aria-label="Vorschau schließen"><Icon name="arrow-left"/></a>
                        <span class="mono">{id.clone()}</span>
                        <a class="ghost" href=full_href data-action="fullscreen" title="Als ganze Seite öffnen (F)"><Icon name="maximize-2"/>"Vollbild"<Shortcut keys="F"/></a>
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

/// The sections a module has, in the order of the page: (anchor, heading).
fn sections(data: &ModuleData) -> Vec<(&'static str, &'static str)> {
    let m = &data.module;
    let literature = data.text_items.iter().any(|i| i.kind.is(TextItemKind::Literature));
    let prerequisites = m.prerequisites_mandatory.is_some() || m.prerequisites_recommended.is_some() || !data.prerequisites.is_empty();
    [
        (true, "termine", "Termine"),
        (!data.exams.is_empty(), "pruefungstermine", "Prüfungstermine"),
        (true, "blick", "Auf einen Blick"),
        (!data.programs.is_empty(), "studiengaenge", "Studiengänge"),
        (prerequisites, "voraussetzungen", "Voraussetzungen"),
        (m.contents.is_some(), "inhalte", "Inhalte"),
        (m.learning_outcomes.is_some(), "lernziele", "Lernziele"),
        (m.exam_details.is_some(), "pruefungsleistung", "Prüfungsleistung"),
        (literature, "literatur", "Literatur"),
        (m.remarks.is_some(), "bemerkungen", "Bemerkungen"),
    ]
    .into_iter()
    .filter(|(present, ..)| *present)
    .map(|(_, anchor, heading)| (anchor, heading))
    .collect()
}

/// The sidebar of the module's page: the sections of the page, and what can be done with the
/// module. It stays in view while the page scrolls, so „Merken" is here as well as beside the
/// badges of the heading. The semester plan is announced here, where it will live.
#[component]
fn Sidebar(data: ModuleData) -> impl IntoView {
    let source_url = data.module.source_url.clone();
    let (id, title) = (data.module.id.clone(), data.module.title.clone());
    view! {
        <nav class="toc jumps" aria-label="Auf dieser Seite">
            <p class="flabel label">"Auf dieser Seite"</p>
            {sections(&data).into_iter().map(|(anchor, heading)| view! {
                <a href=format!("#{anchor}") data-action="jump">{heading}</a>
            }).collect_view()}
        </nav>
        <div class="fgroup actions">
            <p class="flabel label">"Aktionen"</p>
            <MarkButton id title look=MarkLook::Action/>
            <span class="action soon" title="Geplant"><Icon name="calendar-range"/>"Ins Semester einplanen"<em>"bald"</em></span>
            <JsOnly><a class="action" href="#" data-action="copy-link"><Icon name="share-2"/><span>"Link kopieren"</span></a></JsOnly>
            {source_url.map(|href| view! { <a class="action" href=href rel="noopener"><Icon name="arrow-up-right"/>"Original bei der BTU"</a> })}
        </div>
    }
}

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
    let params = use_params_map();
    let id = Memo::new(move |_| params.read().get("id").unwrap_or_default());
    let source = use_source();
    let status = PageStatus::capture();

    move || {
        let id = id.get();
        match source.clone().and_then(|source| source.run(|db| pages::module(db, &id))) {
            Err(error) => {
                status.for_error(&error);
                view! { <div class="page"><ErrorState error/></div> }.into_any()
            }
            Ok(None) => {
                status.set(404);
                view! { <div class="page"><NotFound title="Modul nicht gefunden" hint="Dieses Modul steht nicht (mehr) im Modulkatalog der BTU."/></div> }.into_any()
            }
            Ok(Some(data)) => {
                // „Zurück" leads where the visitor came from: the program whose page had this
                // module open beside it (its „Vollbild"), else the catalog's list as it was left.
                let back = back_to(&id);
                view! { <ModuleFull data back_area=back.0 back_to=back.1/> }.into_any()
            }
        }
    }
}

/// The module's whole page: the frame with the module's sidebar, the module on the rest of the
/// screen. One component wherever the page is shown; `back_area` and `back_to` say where
/// „Zurück" leads (the list of the area, or the page `back_to` names). `noindex` marks the page
/// as a view of another one (a module shown in full inside a program): search engines follow
/// it, its address for them stays the module's own.
#[component]
pub fn ModuleFull(data: ModuleData, back_area: Area, #[prop(optional_no_strip)] back_to: Option<String>, #[prop(optional)] noindex: bool) -> impl IntoView {
    let derived = derive(&data);
    view! {
        // The name first (what people search for), then number and university.
        <Title text=format!("{} ({}) · Modul der BTU Cottbus-Senftenberg", data.module.title, data.module.id)/>
        <Frame
            title="Modul"
            head={ let id = data.module.id.clone(); move || view! { <span class="mono">{id.clone()}</span> } }
            sidebar={ let data = data.clone(); move || view! { <Sidebar data=data.clone()/> } }
        >
                <Seo
                    title=format!("{} ({})", data.module.title, data.module.id)
                    description=derived.description
                    path=url::module_path(&data.module.id)
                    card=crate::seo::module_card(&data.module.id)
                    data=structured(&data)
                    noindex=noindex
                />
                <article class="module-page">
                    <header class="panel hero">
                        <div class="hero-top">
                            <BackLink area=back_area to=back_to/>
                            <span class="mono">{data.module.id.clone()}</span>
                        </div>
                        <Heading data=data.clone()/>
                    </header>
                    // Same parts, same order as the preview; side by side where there is room.
                    <div class="module-grid">
                        <aside class="panel dbody"><Side data=data.clone()/></aside>
                        <div class="panel dbody"><Main data=data.clone()/><Source data=data.clone()/></div>
                    </div>
                </article>
        </Frame>
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
            // „Merken" stands in the line of the credits, at its right end (owner, 2026-09-20), in
            // the preview and on the module's page alike. Marking belongs to the browser app: the
            // switch is part of server HTML so that nothing moves at the takeover, and the
            // stylesheet shows it once the app runs (R9, R15).
            <MarkButton id=m.id.clone() title=m.title.clone() look=MarkLook::Hero/>
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
        <div class="section" id="blick">
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
            <div class="section" id="voraussetzungen">
                <p class="label">"Voraussetzungen"</p>
                <div class="linklist">
                    {prerequisite_links(derived.mandatory.clone(), "zwingend")}
                    {prerequisite_links(derived.recommended.clone(), "empfohlen")}
                </div>
                {m.prerequisites_mandatory.clone().map(|text| view! { <details class="more"><summary>"Zwingend, im Wortlaut"</summary><Prose text/></details> })}
                {m.prerequisites_recommended.clone().map(|text| view! { <details class="more"><summary>"Empfohlen, im Wortlaut"</summary><Prose text/></details> })}
            </div>
        })}
        {m.contents.clone().map(|text| view! { <div class="section" id="inhalte"><p class="label">"Inhalte"</p><Prose text/></div> })}
        {m.learning_outcomes.clone().map(|text| view! { <div class="section" id="lernziele"><p class="label">"Lernziele"</p><Prose text/></div> })}
        {m.exam_details.clone().map(|text| view! { <div class="section" id="pruefungsleistung"><p class="label">"Prüfungsleistung"</p><Prose text/></div> })}
        {(!literature.is_empty()).then(|| view! {
            <div class="section" id="literatur">
                <details class="more"><summary>"Literatur ("{literature.len()}")"</summary>
                    <ul class="list-plain">{literature.into_iter().map(|l| view! { <li>{l}</li> }).collect_view()}</ul>
                </details>
            </div>
        })}
        {m.remarks.clone().map(|text| view! { <div class="section" id="bemerkungen"><p class="label">"Bemerkungen"</p><Prose text/></div> })}
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

    let slots = own_slots(&teaching);

    // An exam date is shown as read (`catalog::exam_reading`): the BTU's placeholder is no time,
    // a deadline reads „bis 24:00", and the original stays in the row.
    let exam_semester_row = exam_semester.as_ref().and_then(|(key, _)| data.semesters.iter().find(|s| s.key == *key));
    let exams: Vec<(EventDate, Option<ExamReading>)> = exams
        .into_iter()
        .map(|d| {
            let reading = exam_reading::read(&d, exam_semester_row);
            (d, Some(reading))
        })
        .collect();
    let exam_note = exam_semester.as_ref().and_then(|(_, label)| exam_note(exams.iter().filter_map(|(_, r)| r.as_ref()), label));

    let event_list = |dates: Vec<(EventDate, Option<ExamReading>)>| {
        dates
            .into_iter()
            .map(|(d, reading)| {
                let slot = reading.as_ref().map_or_else(|| Slot::of(&d), |r| r.shown.clone());
                let when = if reading.is_some() { shown_when(&slot) } else { format::time_slot(d.weekday, d.start_time.as_deref(), d.end_time.as_deref()) };
                let open = if reading.as_ref().is_some_and(|r| r.has(Reason::PlaceholderTime)) && slot.first_date.is_none() { "Termin offen" } else { "Zeit offen" };
                // What the BTU wrote where the row shows something else without marking it (a deadline).
                let stated = reading.as_ref().filter(|r| r.shown != r.stated && !r.is_marked()).map(|r| format!("In QIS: {}", stated_slot(r)));
                let marker = reading.as_ref().filter(|r| r.is_marked()).map(marker_text);
                let rhythm = d.rhythm.as_ref().map(|r| r.label().to_string()).or(d.rhythm_raw.clone());
                let details: Vec<String> = [d.event_type.clone(), d.group_name.clone(), rhythm, date_range(&slot), d.room.clone(), d.instructor.clone(), d.comment.clone()]
                    .into_iter()
                    .flatten()
                    .collect();
                let body = view! {
                    <span class="when" title=stated>{when.unwrap_or_else(|| open.to_string())}</span>
                    <b>{d.event_title.clone()}</b>
                    {(!details.is_empty() || marker.is_none()).then(|| view! { <small>{details.join(" · ")}</small> })}
                    {marker.map(|text| view! { <small class="odd"><Icon name="info"/><span>{text}</span></small> })}
                };
                match d.source_url.clone() {
                    Some(href) => view! { <a class="ev" href=href rel="noopener">{body}</a> }.into_any(),
                    None => view! { <div class="ev">{body}</div> }.into_any(),
                }
            })
            .collect_view()
    };

    view! {
        <div class="section" id="termine">
            <p class="label">"Termine"{newest.as_ref().map(|(_, label)| view! { <span>{label.clone()}</span> })}</p>
            <WeekGrid slots/>
            {gap_note.map(|note| view! { <p class="note"><Icon name="info"/><span>{note}</span></p> })}
            {no_schedule_note.map(|note| view! { <p class="hint">{note}</p> })}
            {(!teaching.is_empty()).then(|| view! { <div class="evlist">{event_list(teaching.into_iter().map(|d| (d, None)).collect())}</div> })}
        </div>
        {exam_semester.map(|(_, label)| view! {
            <div class="section" id="pruefungstermine">
                <p class="label">"Prüfungstermine"<span>{label}</span></p>
                <div class="evlist">{event_list(exams)}</div>
                {exam_note.map(|note| view! { <p class="note"><Icon name="info"/><span>{note}</span></p> })}
            </div>
        })}
    }
}

/// The week grid's slots of a module's teaching rows (one semester's): every row with a weekday
/// and both times, 24:00 included. Rows of one event and group at the same weekday and times are
/// one slot (the same time in two rooms, or a date range QIS splits in two). A date that happens
/// once (a single date, or a range of one day) is not drawn as a weekly slot: the single dates of
/// such a key are one `.once` slot that counts them („3 Termine", „1 Termin · 23.02.").
fn own_slots(teaching: &[EventDate]) -> Vec<GridSlot> {
    /// Once, weekday, from, to, event, group.
    type Key<'a> = (bool, u8, u16, u16, &'a str, Option<&'a str>);
    let mut groups: Vec<(Key<'_>, &EventDate, BTreeSet<Option<&str>>)> = Vec::new();
    for date in teaching {
        let day = date.weekday.and_then(|day| u8::try_from(day).ok()).filter(|day| (1..=7).contains(day));
        let from = date.start_time.as_deref().and_then(minutes);
        let to = date.end_time.as_deref().and_then(minutes);
        let (Some(day), Some(from), Some(to)) = (day, from, to) else { continue };
        if to <= from {
            continue;
        }
        let once = date.rhythm.as_ref().is_some_and(|rhythm| rhythm.is(Rhythm::Single)) || (date.first_date.is_some() && date.first_date == date.last_date);
        let key = (once, day, from, to, date.event_id.as_str(), date.group_name.as_deref());
        match groups.iter_mut().find(|(known, ..)| *known == key) {
            Some((_, _, dates)) => {
                dates.insert(date.first_date.as_deref());
            }
            None => groups.push((key, date, BTreeSet::from([date.first_date.as_deref()]))),
        }
    }
    groups
        .into_iter()
        .map(|((once, day, from, to, ..), date, dates)| {
            let label = date.event_type.clone().unwrap_or_else(|| "Termin".to_string());
            if !once {
                let lecture = class_of(kinds_of(date.event_type.as_deref())) == Class::Lecture;
                return GridSlot { day, from, to, label, small: clock(from), title: date.event_title.clone(), class: if lecture { "" } else { "other" }, ..GridSlot::default() };
            }
            let days: Vec<Day> = dates.iter().filter_map(|date| date.and_then(Day::parse)).collect();
            let small = match (dates.len(), days.as_slice()) {
                (1, [only]) => format!("1 Termin · {}", only.short()),
                (1, _) => "1 Termin".to_string(),
                (n, _) => format!("{n} Termine"),
            };
            // The tooltip names the dates the small line only counts.
            let title = if days.is_empty() {
                date.event_title.clone()
            } else {
                format!("{} · {}", date.event_title, days.iter().map(|day| day.german()).collect::<Vec<_>>().join(", "))
            };
            GridSlot { day, from, to, label, small, title, class: "once", ..GridSlot::default() }
        })
        .collect()
}

/// „Mo 09:15–10:45", and „Mo bis 24:00" for a deadline (an end without a start).
fn shown_when(slot: &Slot) -> Option<String> {
    match (&slot.start_time, &slot.end_time) {
        (None, Some(end)) => Some(match format::time_slot(slot.weekday, None, None) {
            Some(day) => format!("{day} bis {end}"),
            None => format!("bis {end}"),
        }),
        (start, end) => format::time_slot(slot.weekday, start.as_deref(), end.as_deref()),
    }
}

/// „27.12.2015", „08.02.2027 – 19.02.2027"
fn date_range(slot: &Slot) -> Option<String> {
    match (&slot.first_date, &slot.last_date) {
        (Some(first), Some(last)) if first != last => Some(format!("{} – {}", format::date(first), format::date(last))),
        (Some(first), _) => Some(format::date(first)),
        _ => None,
    }
}

/// What the BTU wrote, in the words of the row: „So 01:00–02:30 · 27.12.2015". The dates only
/// where the row does not show them already.
fn stated_slot(reading: &ExamReading) -> String {
    let (stated, shown) = (&reading.stated, &reading.shown);
    let dates = (date_range(stated) != date_range(shown)).then(|| date_range(stated)).flatten();
    [format::time_slot(stated.weekday, stated.start_time.as_deref(), stated.end_time.as_deref()), dates].into_iter().flatten().collect::<Vec<_>>().join(" · ")
}

/// The line under a marked exam date: the original where the row shows something else, and what
/// is odd about what it shows as stated.
fn marker_text(reading: &ExamReading) -> String {
    let odd: Vec<&str> = [
        (Reason::UnusualTime, "Uhrzeit ungewöhnlich"),
        (Reason::EndsBeforeStart, "Ende vor Beginn"),
        (Reason::DateOutsideSemester, "Datum außerhalb des Semesters"),
    ]
    .into_iter()
    .filter(|(reason, _)| reading.has(*reason))
    .map(|(_, text)| text)
    .collect();
    match (reading.shown != reading.stated, odd.is_empty()) {
        (true, true) => format!("In QIS: {}", stated_slot(reading)),
        (true, false) => format!("In QIS: {} · {}", stated_slot(reading), odd.join(" · ")),
        (false, _) => format!("{}, so steht es in QIS", odd.join(" · ")),
    }
}

/// The note under the exam dates when any is marked: what the marks mean, once per section.
fn exam_note<'a>(readings: impl Iterator<Item = &'a ExamReading> + Clone, semester: &str) -> Option<String> {
    let any = |reason: Reason| readings.clone().any(|r| r.has(reason));
    let mut parts: Vec<String> = Vec::new();
    if any(Reason::PlaceholderTime) {
        parts.push(if any(Reason::PlaceholderDate) {
            format!(
                "01:00–02:30 ist in QIS ein Platzhalter für eine Prüfung ohne festen Termin (oft „nach Vereinbarung“), und das Datum dazu passt nicht ins {semester}. Betula zeigt beides nicht; was die BTU angibt, steht in der Zeile."
            )
        } else {
            "01:00–02:30 ist in QIS ein Platzhalter für eine Prüfung ohne feste Uhrzeit (oft „nach Vereinbarung“). Betula zeigt ihn nicht als Uhrzeit; was die BTU angibt, steht in der Zeile.".to_string()
        });
    }
    let (from, to) = exam_reading::DAY;
    let hours = format!("{:02}:{:02}–{:02}:{:02}", from / 60, from % 60, to / 60, to % 60);
    let odd: Vec<String> = [
        (Reason::UnusualTime, format!("eine Uhrzeit außerhalb von {hours}")),
        (Reason::EndsBeforeStart, "ein Ende vor dem Beginn".to_string()),
        (Reason::DateOutsideSemester, format!("ein Datum weit außerhalb des {semester}")),
    ]
    .into_iter()
    .filter(|(reason, _)| any(*reason))
    .map(|(_, text)| text)
    .collect();
    if let Some((first, rest)) = odd.split_first() {
        let mut list = first.clone();
        for (i, item) in rest.iter().enumerate() {
            list.push_str(if i + 1 == rest.len() { " oder " } else { ", " });
            list.push_str(item);
        }
        let mut letters = list.chars();
        let list: String = letters.next().map(|first| first.to_uppercase().chain(letters).collect()).unwrap_or_default();
        parts.push(format!("{list} steht so in QIS, ist für eine Prüfung aber ungewöhnlich, vermutlich ein Eingabefehler."));
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// The programs the module belongs to: curricula first, then how many accept it as FÜS.
#[component]
fn Programs(data: ModuleData) -> impl IntoView {
    let resolved: Vec<_> = data.programs.iter().filter(|l| l.resolve_status.is(ResolveStatus::Resolved)).cloned().collect();
    let curricular: Vec<_> = resolved.iter().filter(|l| l.relation.as_ref().is_some_and(|r| r.is(Relation::Curricular))).cloned().collect();
    let fues = resolved.iter().filter(|l| l.relation.as_ref().is_some_and(|r| r.is(Relation::Fues))).count();
    let unresolved = data.programs.iter().filter(|l| l.resolve_status.is(ResolveStatus::Unresolved)).count();

    (!data.programs.is_empty()).then(|| view! {
        <div class="section" id="studiengaenge">
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

#[cfg(test)]
mod tests {
    use super::*;

    fn exam(weekday: Option<i64>, start: Option<&str>, end: Option<&str>, day: Option<&str>) -> EventDate {
        EventDate {
            semester_key: "2026W".into(),
            semester_label: "WiSe 2026/27".into(),
            event_id: "148663".into(),
            event_number: None,
            event_title: "Analysis I".into(),
            event_type: None,
            group_name: None,
            weekday,
            start_time: start.map(Into::into),
            end_time: end.map(Into::into),
            rhythm: None,
            rhythm_raw: None,
            first_date: day.map(Into::into),
            last_date: day.map(Into::into),
            room: None,
            campus: None,
            instructor: None,
            comment: None,
            source_url: None,
        }
    }

    /// A teaching row: event, type, group, weekday, times, rhythm and dates.
    #[allow(clippy::too_many_arguments)]
    fn teaching(event: &str, kind: &str, group: &str, weekday: i64, times: (&str, &str), rhythm: &str, first: &str, last: &str) -> EventDate {
        EventDate {
            event_id: event.into(),
            event_title: "Allgemeine Betriebswirtschaftslehre II".into(),
            event_type: Some(kind.into()),
            group_name: Some(group.into()),
            rhythm: Some(catalog::labels::Code::parse(rhythm)),
            last_date: Some(last.into()),
            ..exam(Some(weekday), Some(times.0), Some(times.1), Some(first))
        }
    }

    #[test]
    fn a_single_date_is_a_slot_apart_from_the_weekly_ones() {
        // Module 12229 in WiSe 2026/27: the Tuesday single date of the Vorlesung covered the
        // Übung of the 2-Gruppe while both were drawn as weekly slots.
        let rows = [
            teaching("148019", "Vorlesung", "[unbenannt]", 2, ("11:45", "15:00"), "single", "2027-02-23", "2027-02-23"),
            teaching("148019", "Vorlesung", "[unbenannt]", 4, ("11:30", "13:00"), "weekly", "2026-10-08", "2027-01-28"),
            teaching("148130", "Übung", "1-Gruppe", 1, ("13:45", "15:15"), "weekly", "2026-10-12", "2027-01-25"),
            teaching("148130", "Übung", "2-Gruppe", 2, ("13:45", "15:15"), "weekly", "2026-10-13", "2027-01-26"),
        ];
        let slots = own_slots(&rows);
        let shown: Vec<(u8, u16, u16, &str, &str, &str)> = slots.iter().map(|s| (s.day, s.from, s.to, s.label.as_str(), s.small.as_str(), s.class)).collect();
        assert_eq!(
            shown,
            [
                (2, 705, 900, "Vorlesung", "1 Termin · 23.02.", "once"),
                (4, 690, 780, "Vorlesung", "11:30", ""),
                (1, 825, 915, "Übung", "13:45", "other"),
                (2, 825, 915, "Übung", "13:45", "other"),
            ]
        );
        assert_eq!(slots.first().map(|s| s.title.as_str()), Some("Allgemeine Betriebswirtschaftslehre II · 23.02.2027"));
        // The two Tuesday slots stand side by side.
        let (_, styles) = crate::week::geometry(&slots, crate::week::MIN_HOURS).unwrap();
        assert!(styles[0].ends_with("--lane:0;--lanes:2") && styles[3].ends_with("--lane:1;--lanes:2"), "{styles:?}");
        assert!(styles[1].ends_with("--lane:0;--lanes:1") && styles[2].ends_with("--lane:0;--lanes:1"), "{styles:?}");
    }

    #[test]
    fn rows_of_one_slot_are_one() {
        let rows = [
            // The same weekly time in two rooms, and a range QIS splits in two: one slot.
            teaching("148369", "Übung", "1-Gruppe", 1, ("15:30", "17:00"), "weekly", "2026-10-12", "2026-11-23"),
            teaching("148369", "Übung", "1-Gruppe", 1, ("15:30", "17:00"), "weekly", "2026-12-07", "2027-01-25"),
            // Three single dates at one time: one slot that counts them.
            teaching("148370", "Vorlesung/Übung", "[unbenannt]", 3, ("09:15", "10:45"), "single", "2026-11-04", "2026-11-04"),
            teaching("148370", "Vorlesung/Übung", "[unbenannt]", 3, ("09:15", "10:45"), "single", "2026-11-18", "2026-11-18"),
            teaching("148370", "Vorlesung/Übung", "[unbenannt]", 3, ("09:15", "10:45"), "single", "2026-11-11", "2026-11-11"),
            // A weekly row whose range is one day happens once.
            teaching("148371", "Seminar", "A", 5, ("08:00", "09:30"), "weekly", "2026-10-16", "2026-10-16"),
            // A Sunday slot until midnight is drawn; a row without an end is not.
            teaching("148372", "Projekt", "B", 7, ("22:00", "24:00"), "weekly", "2026-10-11", "2027-01-31"),
            teaching("148373", "Tutorium", "C", 2, ("10:00", ""), "weekly", "2026-10-13", "2027-01-26"),
        ];
        let slots = own_slots(&rows);
        let shown: Vec<(u8, &str, &str, &str)> = slots.iter().map(|s| (s.day, s.small.as_str(), s.class, s.title.as_str())).collect();
        assert_eq!(
            shown,
            [
                (1, "15:30", "other", "Allgemeine Betriebswirtschaftslehre II"),
                (3, "3 Termine", "once", "Allgemeine Betriebswirtschaftslehre II · 04.11.2026, 11.11.2026, 18.11.2026"),
                (5, "1 Termin · 16.10.", "once", "Allgemeine Betriebswirtschaftslehre II · 16.10.2026"),
                (7, "22:00", "other", "Allgemeine Betriebswirtschaftslehre II"),
            ]
        );
        assert_eq!(slots.get(3).map(|s| s.to), Some(1440));
        // A „Vorlesung/Übung" is a lecture.
        let weekly = teaching("148370", "Vorlesung/Übung", "[unbenannt]", 3, ("09:15", "10:45"), "weekly", "2026-10-14", "2027-01-27");
        assert_eq!(own_slots(&[weekly]).first().map(|s| s.class), Some(""));
    }

    fn read(date: &EventDate) -> ExamReading {
        let winter = catalog::rows::Semester {
            key: "2026W".into(),
            season: catalog::labels::Code::parse("winter"),
            year: 2026,
            label: "WiSe 2026/27".into(),
            starts_on: "2026-10-01".into(),
            ends_on: "2027-03-31".into(),
            is_current: false,
            teaching_events: 0,
            exam_events: 0,
        };
        exam_reading::read(date, Some(&winter))
    }

    #[test]
    fn an_exam_date_names_what_the_btu_wrote() {
        // Analysis I: the placeholder, dated 27.12.2015.
        let placeholder = read(&exam(Some(7), Some("01:00"), Some("02:30"), Some("2015-12-27")));
        assert_eq!(shown_when(&placeholder.shown), None);
        assert_eq!(marker_text(&placeholder), "In QIS: So 01:00–02:30 · 27.12.2015");
        let note = exam_note([&placeholder].into_iter(), "WiSe 2026/27").unwrap_or_default();
        assert!(note.starts_with("01:00–02:30 ist in QIS ein Platzhalter für eine Prüfung ohne festen Termin") && note.contains("nicht ins WiSe 2026/27"), "{note}");

        // A block of oral exams: its days stay in the row, so the line names the time only.
        let block = EventDate { last_date: Some("2027-02-19".into()), ..exam(None, Some("01:00"), Some("02:30"), Some("2027-02-08")) };
        assert_eq!(marker_text(&read(&block)), "In QIS: 01:00–02:30");
        assert!(exam_note([&read(&block)].into_iter(), "WiSe 2026/27").unwrap_or_default().contains("ohne feste Uhrzeit"));

        // A deadline: read, not marked, and no note.
        let deadline = read(&exam(Some(7), Some("23:45"), Some("24:00"), Some("2027-03-21")));
        assert_eq!(shown_when(&deadline.shown).as_deref(), Some("So bis 24:00"));
        assert_eq!(stated_slot(&deadline), "So 23:45–24:00");
        assert_eq!(exam_note([&deadline].into_iter(), "WiSe 2026/27"), None);

        // Only marked: the row shows the source, the line says what is odd.
        let night = read(&exam(Some(2), Some("03:00"), Some("02:00"), Some("2015-02-10")));
        assert_eq!(shown_when(&night.shown).as_deref(), Some("Di 03:00–02:00"));
        assert_eq!(marker_text(&night), "Uhrzeit ungewöhnlich · Ende vor Beginn · Datum außerhalb des Semesters, so steht es in QIS");
        assert_eq!(
            exam_note([&night].into_iter(), "WiSe 2026/27").as_deref(),
            Some("Eine Uhrzeit außerhalb von 06:00–22:00, ein Ende vor dem Beginn oder ein Datum weit außerhalb des WiSe 2026/27 steht so in QIS, ist für eine Prüfung aber ungewöhnlich, vermutlich ein Eingabefehler."),
        );
        let plain = read(&exam(Some(2), Some("09:00"), Some("11:00"), Some("2027-02-09")));
        assert_eq!(exam_note([&plain].into_iter(), "WiSe 2026/27"), None);
    }
}
