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
//!
//! What the visitor planned meets the module in the browser app alone (R9): „Einplanen" beside
//! „Merken" and among the sidebar's actions (`studyplan::PlanButton`), and the other modules
//! planned into the semester of its Termine, drawn beside them in its week (`plan_overlay`).

use std::collections::BTreeSet;

use catalog::exam_reading::{self, ExamReading, Reason, Slot};
use catalog::labels::{OfferStatus, PrerequisiteKind, Relation, ResolveStatus, Rhythm, TeachingForm, TextItemKind, TurnusSeason};
use catalog::pages::{self, ModuleData, Overlay};
use catalog::rows::{Module, Prerequisite, Semester};
use catalog::rows_detail::{EventDate, ProgramLink};
use catalog::timetable::day::{clock, minutes, Day};
use catalog::timetable::grid;
use catalog::timetable::ics::TZID;
use catalog::timetable::kind::{class_of, kinds_of, Class, EventKind, KindSet};
use catalog::timetable::rowkey::RowKey;
use catalog::timetable::semester::SemesterKey;
use catalog::url::{self, ModuleHint, ProgramTab};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use crate::i18n::use_location;

use crate::bookmarks::{MarkButton, MarkLook};
use crate::data::{use_source, PageStatus};
use crate::format;
use crate::myprogram::MyProgram;
use crate::seo::{self, Seo};
use crate::studyplan::{PlanButton, PlanHint, PlanLook, Studyplan};
use crate::tabs::{self, Area, Tabs};
use crate::ui::{BackLink, ErrorState, Fact, Frame, Icon, JsOnly, KindBadge, NotFound, OfferBadge, Prose, Shortcut};
use crate::week::{GridSlot, WeekGrid, MIN_HOURS};

/// The browser app (`csr`): only there is a plan to meet.
const APP: bool = cfg!(feature = "csr");

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
        campus: campuses(m),
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
            .map(|text| seo::excerpt(&format!("{} ({}, {}) an der BTU Cottbus-Senftenberg: {text}", m.title, m.id, format::credits(m.credits, crate::i18n::locale())), 300))
            .unwrap_or_else(|| format!("{} (Modul {}, {}) an der BTU Cottbus-Senftenberg: Turnus, Prüfung, Voraussetzungen und Studiengänge.", m.title, m.id, format::credits(m.credits, crate::i18n::locale()))),
    }
}

/// The campuses the module is taught at, as the page names them.
fn campuses(m: &Module) -> Vec<&'static str> {
    [(m.at_zentralcampus, "Zentralcampus Cottbus"), (m.at_sachsendorf, "Cottbus-Sachsendorf"), (m.at_senftenberg, "Senftenberg")]
        .iter()
        .filter(|(at, _)| *at == Some(true))
        .map(|(_, label)| *label)
        .collect()
}

/// The programs whose curricula the module belongs to, as „Studiengänge" lists them.
fn curricula(data: &ModuleData) -> Vec<ProgramLink> {
    data.programs
        .iter()
        .filter(|l| l.resolve_status.is(ResolveStatus::Resolved) && l.relation.as_ref().is_some_and(|r| r.is(Relation::Curricular)))
        .cloned()
        .collect()
}

/// Where the validated plan of a program the module belongs to places it: „1. Semester".
fn plan_semesters(data: &ModuleData, link: &ProgramLink) -> Option<String> {
    format::plan_semesters(&data.plan_semesters(link.program_id.as_deref()?), crate::i18n::locale())
}

/// The module as schema.org knows it (a `Course` of the university) and the way to it. Only what
/// the page shows, and what a search engine answers questions with: when and where the module is
/// taught and examined, what it requires, and in which semester the study plans place it. Its
/// Termine are the course's instance in their semester (`schedule_of`), its exam dates events of
/// the semester they are listed under (`exam_event`). Kept small: the texts only as the short
/// description, no Termin without a time of the week, no exam date the page marks as doubtful.
fn structured(data: &ModuleData) -> Vec<serde_json::Value> {
    let m = &data.module;
    let mut course = serde_json::json!({
        "@type": "Course",
        "@id": seo::absolute(&url::module_path(&m.id)),
        "url": seo::absolute(&url::module_path(&m.id)),
        "name": m.title,
        "courseCode": m.id,
        "provider": seo::university(),
    });
    let languages: Vec<&str> = [(m.teaches_german, "de"), (m.teaches_english, "en")].iter().filter(|(taught, _)| *taught == Some(true)).map(|(_, code)| *code).collect();
    if let Some(course) = course.as_object_mut() {
        if let Some(text) = m.contents.as_ref().or(m.learning_outcomes.as_ref()) {
            course.insert("description".into(), seo::excerpt(text, 500).into());
        }
        if let Some(credits) = m.credits {
            course.insert("numberOfCredits".into(), serde_json::json!({ "@type": "QuantitativeValue", "value": credits, "unitText": "ECTS" }));
        }
        if !languages.is_empty() {
            course.insert("inLanguage".into(), languages.into());
        }
        if let Some(source) = &m.source_url {
            course.insert("sameAs".into(), source.clone().into());
        }
        let required = required_modules(data);
        if !required.is_empty() {
            course.insert("coursePrerequisites".into(), required.into());
        }
        let plans = plan_alignments(data);
        if !plans.is_empty() {
            course.insert("educationalAlignment".into(), plans.into());
        }
        let instances = course_instances(data);
        if !instances.is_empty() {
            course.insert("hasCourseInstance".into(), instances.into());
        }
    }
    vec![
        course,
        seo::breadcrumbs(&[("Betula", url::HOME.to_string()), ("Modulkatalog", url::CATALOG.to_string()), (m.title.as_str(), url::module_path(&m.id))]),
    ]
}

/// The modules the description names as required and the catalog knows, as the page links them.
fn required_modules(data: &ModuleData) -> Vec<serde_json::Value> {
    data.prerequisites
        .iter()
        .filter(|p| p.kind.is(PrerequisiteKind::Mandatory))
        .map(|p| {
            let mut module = serde_json::json!({ "@type": "Course", "@id": seo::absolute(&url::module_path(&p.required_module_id)), "courseCode": p.required_module_id });
            if let (Some(module), Some(title)) = (module.as_object_mut(), &p.required_title) {
                module.insert("name".into(), title.clone().into());
            }
            module
        })
        .collect()
}

/// Where the validated study plans place the module, in the words of schema.org's alignment of a
/// learning resource with a framework: the plan is the framework, its semester the level.
fn plan_alignments(data: &ModuleData) -> Vec<serde_json::Value> {
    let mut seen = BTreeSet::new();
    curricula(data)
        .iter()
        .filter(|link| link.program_id.as_ref().is_some_and(|id| seen.insert(id.clone())))
        .filter_map(|link| {
            let semesters = plan_semesters(data, link)?;
            let slug = link.program_slug.as_deref()?;
            let degree = link.degree_display.clone().or(link.degree_raw.clone()).unwrap_or_default();
            let po = link.po_version.clone().unwrap_or_default();
            Some(serde_json::json!({
                "@type": "AlignmentObject",
                "alignmentType": "educationalLevel",
                "educationalFramework": format!("Regelstudienplan {} ({degree}), PO {po}", link.program_name.clone().unwrap_or_default()),
                "targetName": semesters,
                "targetUrl": seo::absolute(&url::program_path(slug, ProgramTab::Plan)),
            }))
        })
        .collect()
}

/// The module's Termine as schema.org's course instances, one per semester the page shows them
/// for: the teaching semester's with a `Schedule` per slot of its week, and the exam dates as
/// its `subEvent`s where they are of the same semester, else as an instance of their own.
fn course_instances(data: &ModuleData) -> Vec<serde_json::Value> {
    let m = &data.module;
    let newest = data.schedule.first().map(|d| (d.semester_key.clone(), d.semester_label.clone()));
    let teaching: Vec<&EventDate> = data.schedule.iter().filter(|d| newest.as_ref().is_some_and(|(key, _)| *key == d.semester_key)).collect();
    let exam_semester = data.exams.first().map(|d| (d.semester_key.clone(), d.semester_label.clone()));
    let exam_row = exam_semester.as_ref().and_then(|(key, _)| data.semesters.iter().find(|s| s.key == *key));
    let mut exams: Vec<(Day, serde_json::Value)> = data
        .exams
        .iter()
        .filter(|d| exam_semester.as_ref().is_some_and(|(key, _)| *key == d.semester_key))
        .filter_map(|d| exam_event(d, &exam_reading::read(d, exam_row)))
        .collect();
    let shared = newest.as_ref().zip(exam_semester.as_ref()).is_some_and(|((teaching, _), (exam, _))| teaching == exam);
    let mut instances = Vec::new();
    if let Some((_, label)) = &newest {
        let own = if shared { std::mem::take(&mut exams) } else { Vec::new() };
        instances.push(course_instance(m, label, &teaching, own));
    }
    if let Some((_, label)) = exam_semester.filter(|_| !exams.is_empty()) {
        instances.push(course_instance(m, &label, &[], exams));
    }
    instances
}

/// One semester of the module: from its first Termin to its last, where it is taught, the slots
/// of its week and its exam dates. An instance of exam dates alone spans them.
fn course_instance(m: &Module, semester: &str, teaching: &[&EventDate], exams: Vec<(Day, serde_json::Value)>) -> serde_json::Value {
    let mut days: Vec<Day> = teaching.iter().flat_map(|d| [d.first_date.as_deref(), d.last_date.as_deref()]).flatten().filter_map(Day::parse).collect();
    if teaching.is_empty() {
        days.extend(exams.iter().map(|(day, _)| *day));
    }
    let mut instance = serde_json::Map::new();
    instance.insert("@type".into(), "CourseInstance".into());
    instance.insert("name".into(), format!("{} ({semester})", m.title).into());
    if let (Some(first), Some(last)) = (days.iter().min(), days.iter().max()) {
        instance.insert("startDate".into(), first.iso().into());
        instance.insert("endDate".into(), last.iso().into());
    }
    let places: Vec<serde_json::Value> = campuses(m).into_iter().map(|name| serde_json::json!({ "@type": "Place", "name": name })).collect();
    if !teaching.is_empty() && !places.is_empty() {
        instance.insert("location".into(), places.into());
    }
    let week = schedule_of(teaching);
    if !week.is_empty() {
        instance.insert("courseSchedule".into(), week.into());
    }
    if !exams.is_empty() {
        instance.insert("subEvent".into(), exams.into_iter().map(|(_, event)| event).collect::<Vec<_>>().into());
    }
    serde_json::Value::Object(instance)
}

/// The week of a semester's Termine as schema.org `Schedule`s: every row with a weekday and a
/// time, from its first date to its last and at its rhythm, rows alike in all but the weekday as
/// one. A row without a time, or „nach Absprache", has no slot (the page says „Zeit offen").
fn schedule_of(teaching: &[&EventDate]) -> Vec<serde_json::Value> {
    /// What the slot is, its times, its dates and how often it repeats.
    type Slot = (String, u16, u16, Option<String>, Option<String>, Option<&'static str>);
    let mut slots: Vec<(Slot, Vec<u8>)> = Vec::new();
    for d in teaching {
        let Some(day) = d.weekday.and_then(|day| u8::try_from(day).ok()).filter(|day| (1..=7).contains(day)) else { continue };
        let (Some(from), Some(to)) = (d.start_time.as_deref().and_then(minutes), d.end_time.as_deref().and_then(minutes)) else { continue };
        if to <= from {
            continue;
        }
        let repeat = match d.rhythm.as_ref().and_then(|rhythm| rhythm.known()) {
            Some(Rhythm::Other) => continue,
            Some(Rhythm::Weekly) => Some("P1W"),
            Some(Rhythm::WeekA | Rhythm::WeekB) => Some("P2W"),
            _ => None,
        };
        let kind = d.event_type.as_deref().map(str::trim).filter(|kind| !kind.is_empty());
        // QIS writes „[unbenannt]" for a group without a name.
        let group = d.group_name.as_deref().map(str::trim).filter(|group| !group.is_empty() && !group.starts_with('['));
        let name = match (kind, group) {
            (Some(kind), Some(group)) => format!("{kind} ({group})"),
            (Some(kind), None) => kind.to_string(),
            (None, Some(group)) => format!("{} ({group})", d.event_title),
            (None, None) => d.event_title.clone(),
        };
        let first = d.first_date.as_deref().and_then(Day::parse).map(Day::iso);
        let last = d.last_date.as_deref().and_then(Day::parse).map(Day::iso).or(first.clone());
        let slot = (name, from, to, first, last, repeat);
        match slots.iter_mut().find(|(known, _)| *known == slot) {
            Some((_, days)) if !days.contains(&day) => days.push(day),
            Some(_) => {}
            None => slots.push((slot, vec![day])),
        }
    }
    slots
        .into_iter()
        .map(|((name, from, to, first, last, repeat), mut days)| {
            days.sort_unstable();
            let mut slot = serde_json::Map::new();
            slot.insert("@type".into(), "Schedule".into());
            slot.insert("name".into(), name.into());
            slot.insert("byDay".into(), days.into_iter().filter_map(seo::day_of_week).collect::<Vec<_>>().into());
            slot.insert("startTime".into(), clock(from).into());
            // 24:00 is no time of the day the slot is on.
            if to < 24 * 60 {
                slot.insert("endTime".into(), clock(to).into());
            }
            if let Some(first) = first {
                slot.insert("startDate".into(), first.into());
            }
            if let Some(last) = last {
                slot.insert("endDate".into(), last.into());
            }
            if let Some(repeat) = repeat {
                slot.insert("repeatFrequency".into(), repeat.into());
            }
            slot.insert("scheduleTimezone".into(), TZID.into());
            serde_json::Value::Object(slot)
        })
        .collect()
}

/// An exam date as a schema.org `EducationEvent`, as the page reads it (`exam_reading`): its times
/// as moments in `Europe/Berlin`, a deadline as its day. What the page marks (QIS's placeholder
/// for a date not fixed yet, a time that is probably an input error) is no date to state. With the
/// day it starts on, which an instance of exam dates alone spans.
fn exam_event(date: &EventDate, reading: &ExamReading) -> Option<(Day, serde_json::Value)> {
    if reading.is_marked() {
        return None;
    }
    let shown = &reading.shown;
    let first = Day::parse(shown.first_date.as_deref()?)?;
    let last = shown.last_date.as_deref().and_then(Day::parse).unwrap_or(first);
    let at = |day: Day, time: Option<&str>| time.and_then(|time| seo::berlin_time(day, time)).unwrap_or_else(|| day.iso());
    let mut event = serde_json::Map::new();
    event.insert("@type".into(), "EducationEvent".into());
    event.insert("name".into(), format!("Prüfung {}", date.event_title).into());
    event.insert("startDate".into(), at(first, shown.start_time.as_deref()).into());
    let end = match (&shown.start_time, &shown.end_time) {
        (Some(_), Some(end)) if seo::berlin_time(last, end).is_some() => Some(at(last, Some(end))),
        _ => (last != first).then(|| last.iso()),
    };
    if let Some(end) = end {
        event.insert("endDate".into(), end.into());
    }
    // The room's whole name, which the page gives the short form of as its tooltip.
    if let Some(room) = date.room.as_deref().map(str::trim).filter(|room| !room.is_empty()).or(date.room_shown()) {
        event.insert("location".into(), serde_json::json!({ "@type": "Place", "name": room }));
    }
    Some((first, serde_json::Value::Object(event)))
}

/// The preview next to a list. `close_href` is the same page without the preview. `docked` gives
/// it the head of a frame's panel (`ui::Frame`); it floats over the page either way.
/// `full_href` is where „Vollbild" leads: the module's own page unless the page beside which the
/// module stands can show it in full itself (a program's page). `hint` says where „Einplanen"
/// plans to when the list beside it was asked for a semester or a placeholder (the finder).
#[component]
pub fn ModulePanel(
    data: ModuleData,
    close_href: String,
    #[prop(optional)] docked: bool,
    #[prop(optional_no_strip)] full_href: Option<String>,
    #[prop(optional, into)] hint: Signal<Option<PlanHint>>,
) -> impl IntoView {
    let id = data.module.id.clone();
    // The module's own page keeps the hint (`?plan=…&fill=…`, as on a phone), so „Einplanen"
    // aims there as it does here. The browser app's alone: the server's pages carry no hint.
    let full_href = {
        let id = id.clone();
        move || match (&full_href, hint.get().filter(|_| APP)) {
            (Some(href), _) => href.clone(),
            (None, Some(hint)) => format!("{}{}", url::module_path(&id), hint.query()),
            (None, None) => url::module_path(&id),
        }
    };
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
                    <Heading data=data.clone() hint/>
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
/// module. It stays in view while the page scrolls, so „Merken" and „Einplanen" are here as well
/// as beside the badges of the heading; here „Einplanen" also names its semester and offers the
/// others.
#[component]
fn Sidebar(data: ModuleData, hint: Signal<Option<PlanHint>>) -> impl IntoView {
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
            {plan_button(&data, hint, PlanLook::Action)}
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
    // Where „Einplanen" plans to when the finder sent the visitor here (`?plan=…&fill=…`, a
    // phone's way from the catalog): the browser app's alone, like the plan (the server keys the
    // page by its path and renders the button as for everybody).
    let location = use_location();
    let hint = Memo::new(move |_| if APP { PlanHint::of(&ModuleHint::parse(&location.search.get())) } else { None });

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
                view! { <ModuleFull data back_area=back.0 back_to=back.1 hint/> }.into_any()
            }
        }
    }
}

/// The module's whole page: the frame with the module's sidebar, the module on the rest of the
/// screen. One component wherever the page is shown; `back_area` and `back_to` say where
/// „Zurück" leads (the list of the area, or the page `back_to` names). `noindex` marks the page
/// as a view of another one (a module shown in full inside a program): search engines follow
/// it, its address for them stays the module's own. `hint` aims „Einplanen" (`ModulePanel`).
#[component]
pub fn ModuleFull(
    data: ModuleData,
    back_area: Area,
    #[prop(optional_no_strip)] back_to: Option<String>,
    #[prop(optional)] noindex: bool,
    #[prop(optional, into)] hint: Signal<Option<PlanHint>>,
) -> impl IntoView {
    let derived = derive(&data);
    view! {
        // The name first (what people search for), then number and university.
        <Title text=format!("{} ({}) · Modul der BTU Cottbus-Senftenberg", data.module.title, data.module.id)/>
        <Frame
            title="Modul"
            head={ let id = data.module.id.clone(); move || view! { <span class="mono">{id.clone()}</span> } }
            sidebar={ let data = data.clone(); move || view! { <Sidebar data=data.clone() hint/> } }
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
                        <Heading data=data.clone() hint/>
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

/// The snapshot's current semester, and the semester of the module's newest teaching Termine (the
/// week's, `Schedule`): what „Einplanen" aims with (`studyplan::target_semester`). Exams do not
/// count: a module taught in summer holds retakes in the winter too, and its only rows of a winter
/// would plan it into a semester it is not taught in, where its turnus says the next summer.
fn semesters_of(semesters: &[Semester], schedule: &[EventDate]) -> (Option<SemesterKey>, Option<SemesterKey>) {
    let current = semesters.iter().find(|s| s.is_current).and_then(|s| SemesterKey::parse(&s.key));
    let newest = schedule.iter().filter_map(|d| SemesterKey::parse(&d.semester_key)).max();
    (current, newest)
}

/// „Einplanen" for the module. A snapshot always names its current semester; should one not, the
/// module's newest one stands in, and without either there is nothing to plan into.
fn plan_button(data: &ModuleData, hint: Signal<Option<PlanHint>>, look: PlanLook) -> Option<impl IntoView> {
    let (current, newest) = semesters_of(&data.semesters, &data.schedule);
    let m = &data.module;
    current.or(newest).map(|current| {
        view! { <PlanButton id=m.id.clone() title=m.title.clone() turnus=m.turnus_season.clone() current newest hint look/> }
    })
}

#[component]
fn Heading(data: ModuleData, hint: Signal<Option<PlanHint>>) -> impl IntoView {
    let m = data.module.clone();
    let derived = derive(&data);
    view! {
        <h2>{m.title.clone()}</h2>
        {derived.other_title.map(|title| view! { <p class="en">{title}</p> })}
        // „Merken" stands in the line of the credits, at its right end (owner, 2026-09-20), in the
        // preview and on the module's page alike, and „Einplanen" before it. Where the line has no
        // room for the two side by side they stand one over the other at its end, and the badges
        // wrap in the rest of it (owner, 2026-09-26): the two never leave the line. In the order
        // they are seen, for the Tab key. Marking and planning belong to the browser app: the
        // switches are part of server HTML so that nothing moves at the takeover, and the
        // stylesheet shows them once the app runs (R9, R15).
        <div class="hero-line">
            <p class="badges">
                <span class="badge strong num">{format::credits(m.credits, crate::i18n::locale())}</span>
                <span class="badge">{format::turnus(m.turnus_season.as_ref(), m.turnus_parity.as_ref(), crate::i18n::locale())}</span>
                {format::languages(m.teaches_german, m.teaches_english).map(|l| view! { <span class="badge">{l}</span> })}
                {m.is_fues.then(|| view! { <span class="badge">"FÜS"</span> })}
                {(!m.offer_status.is(OfferStatus::Active)).then(|| view! { <span class="badge warn">{m.offer_status.label(crate::i18n::locale()).to_string()}</span> })}
            </p>
            <div class="switches">
                {plan_button(&data, hint, PlanLook::Hero)}
                <MarkButton id=m.id.clone() title=m.title.clone() look=MarkLook::Hero/>
            </div>
        </div>
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
            <h3 class="label">"Auf einen Blick"</h3>
            <dl class="facts">
                <Fact icon="file-check-2" label="Prüfung" value=m.exam_form.as_ref().map(|form| format::exam_short(form, crate::i18n::locale())).or(m.exam_form_raw.clone())/>
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
                <h3 class="label">"Voraussetzungen"</h3>
                <div class="linklist">
                    {prerequisite_links(derived.mandatory.clone(), "zwingend")}
                    {prerequisite_links(derived.recommended.clone(), "empfohlen")}
                </div>
                {m.prerequisites_mandatory.clone().map(|text| view! { <details class="more"><summary>"Zwingend, im Wortlaut"</summary><Prose text/></details> })}
                {m.prerequisites_recommended.clone().map(|text| view! { <details class="more"><summary>"Empfohlen, im Wortlaut"</summary><Prose text/></details> })}
            </div>
        })}
        {m.contents.clone().map(|text| view! { <div class="section" id="inhalte"><h3 class="label">"Inhalte"</h3><Prose text/></div> })}
        {m.learning_outcomes.clone().map(|text| view! { <div class="section" id="lernziele"><h3 class="label">"Lernziele"</h3><Prose text/></div> })}
        {m.exam_details.clone().map(|text| view! { <div class="section" id="pruefungsleistung"><h3 class="label">"Prüfungsleistung"</h3><Prose text/></div> })}
        {(!literature.is_empty()).then(|| view! {
            <div class="section" id="literatur">
                <details class="more"><summary>"Literatur ("{literature.len()}")"</summary>
                    <ul class="list-plain">{literature.into_iter().map(|l| view! { <li>{l}</li> }).collect_view()}</ul>
                </details>
            </div>
        })}
        {m.remarks.clone().map(|text| view! { <div class="section" id="bemerkungen"><h3 class="label">"Bemerkungen"</h3><Prose text/></div> })}
    }
}

#[component]
fn Source(data: ModuleData) -> impl IntoView {
    let m = data.module;
    view! {
        <p class="source">
            <Icon name="shield-check"/>
            "Quelle: Modulbeschreibung der BTU"
            {m.fetched_at.as_deref().map(|at| format!(" · abgerufen {}", format::date(at, crate::i18n::locale())))}
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

    // The Studienplan beside the week (A.9): nothing on the server, so its HTML is the module's
    // alone; in the app the other modules planned into the semester of these Termine.
    let overlay = plan_overlay(&m.id, newest_key.as_deref().and_then(SemesterKey::parse), current.and_then(|c| SemesterKey::parse(&c.key)));
    let own = own_groups(&teaching);
    let slots = Memo::new(move |_| overlay.with(|overlay| with_overlay(&own, overlay)));
    let week_line = move || {
        overlay.with(|overlay| overlay.line.clone()).map(|(warn, text)| match warn {
            true => view! { <p class="note"><Icon name="triangle-alert"/><span>{text}</span></p> }.into_any(),
            false => view! { <p class="hint">{text}</p> }.into_any(),
        })
    };
    // The exam line speaks of the plan's semester: under these exams only when they are of it.
    let exams_of_plan = exam_semester.as_ref().map(|(key, _)| key.clone()) == newest_key;
    // A warning where no other Termin avoids it; else quiet, since it says which one passes (A.5).
    let exam_line = move || {
        exams_of_plan
            .then(|| overlay.with(|overlay| overlay.exam_line.clone()))
            .flatten()
            .map(|(warn, text)| match warn {
                true => view! { <p class="note"><Icon name="triangle-alert"/><span>{text}</span></p> }.into_any(),
                false => view! { <p class="note quiet"><span>{text}</span></p> }.into_any(),
            })
    };

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
                let when = if reading.is_some() { shown_when(&slot) } else { format::time_slot(d.weekday, d.start_time.as_deref(), d.end_time.as_deref(), crate::i18n::locale()) };
                let open = if reading.as_ref().is_some_and(|r| r.has(Reason::PlaceholderTime)) && slot.first_date.is_none() { "Termin offen" } else { "Zeit offen" };
                // What the BTU wrote where the row shows something else without marking it (a deadline).
                let stated = reading.as_ref().filter(|r| r.shown != r.stated && !r.is_marked()).map(|r| format!("In QIS: {}", stated_slot(r)));
                let marker = reading.as_ref().filter(|r| r.is_marked()).map(marker_text);
                let rhythm = d.rhythm.as_ref().map(|r| r.label(crate::i18n::locale()).to_string()).or(d.rhythm_raw.clone());
                let head: Vec<String> = [d.event_type.clone(), d.group_name.clone(), rhythm].into_iter().flatten().collect();
                let tail: Vec<String> = [d.room_shown().map(str::to_string), d.instructor.clone(), d.comment.clone()].into_iter().flatten().collect();
                let days = slot.first_date.clone().map(|first| (first, slot.last_date.clone().filter(|last| Some(last) != slot.first_date.as_ref())));
                let details = !head.is_empty() || days.is_some() || !tail.is_empty();
                // The short form stands in the line; QIS's whole name is the tooltip (owner, 2026-09-25).
                let room_long = d.room_short.is_some().then(|| d.room.clone()).flatten();
                let body = view! {
                    <span class="when" title=stated>{when.unwrap_or_else(|| open.to_string())}</span>
                    <b>{d.event_title.clone()}</b>
                    {(details || marker.is_none()).then(|| view! { <small title=room_long>{detail_line(head, days, tail)}</small> })}
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
            <h3 class="label">"Termine"{newest.as_ref().map(|(_, label)| view! { <span>{label.clone()}</span> })}</h3>
            <WeekGrid slots/>
            {week_line}
            {gap_note.map(|note| view! { <p class="note"><Icon name="info"/><span>{note}</span></p> })}
            {no_schedule_note.map(|note| view! { <p class="hint">{note}</p> })}
            {(!teaching.is_empty()).then(|| view! { <div class="evlist">{event_list(teaching.into_iter().map(|d| (d, None)).collect())}</div> })}
        </div>
        {exam_semester.map(|(_, label)| view! {
            <div class="section" id="pruefungstermine">
                <h3 class="label">"Prüfungstermine"<span>{label}</span></h3>
                <div class="evlist">{event_list(exams)}</div>
                {exam_line}
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
#[cfg(test)]
fn own_slots(teaching: &[EventDate]) -> Vec<GridSlot> {
    own_groups(teaching).into_iter().map(|(slot, _)| slot).collect()
}

/// `own_slots`, each with the keys of the rows it stands for (`RowKey`): what the Studienplan
/// names when one of them clashes with it.
fn own_groups(teaching: &[EventDate]) -> Vec<(GridSlot, Vec<RowKey>)> {
    /// Once, weekday, from, to, event, group.
    type Key<'a> = (bool, u8, u16, u16, &'a str, Option<&'a str>);
    /// A slot's key, its first row, the days of its single dates, and the keys of its rows.
    type Group<'a> = (Key<'a>, &'a EventDate, BTreeSet<Option<&'a str>>, Vec<RowKey>);
    let mut groups: Vec<Group<'_>> = Vec::new();
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
        let row = RowKey::of(date);
        match groups.iter_mut().find(|(known, ..)| *known == key) {
            Some((_, _, dates, rows)) => {
                dates.insert(date.first_date.as_deref());
                rows.extend(row);
            }
            None => groups.push((key, date, BTreeSet::from([date.first_date.as_deref()]), row.into_iter().collect())),
        }
    }
    groups
        .into_iter()
        .map(|((once, day, from, to, ..), date, dates, rows)| (own_slot(once, day, from, to, date, &dates), rows))
        .collect()
}

/// The slot of one group of rows (`own_groups`): `date` is its first row, `dates` the days of its
/// single dates.
fn own_slot(once: bool, day: u8, from: u16, to: u16, date: &EventDate, dates: &BTreeSet<Option<&str>>) -> GridSlot {
    // The kinds in their few letters („VL", „Prak", „VL/Ü"), as in the Studienplan's week: what
    // QIS calls the event fits a narrow day only in part, and one grid said „Vorlesung", „VL" and
    // „Laborausbi…" side by side (review 2026-09-25). QIS's word leads the tooltip, and stays the
    // label of a type no kind is known of. The module's name is the page's own and stands in no
    // slot.
    let word = date.event_type.as_deref().map(str::trim).filter(|word| !word.is_empty());
    let kinds = kinds_of(word);
    let label = match kinds == KindSet::default().with(EventKind::Other) {
        true => word.unwrap_or("Termin").to_string(),
        false => kinds.iter().map(|kind| kind.short(crate::i18n::locale())).collect::<Vec<_>>().join("/"),
    };
    let what = match word {
        Some(word) => format!("{word} · {}", date.event_title),
        None => date.event_title.clone(),
    };
    if !once {
        let lecture = class_of(kinds) == Class::Lecture;
        return GridSlot { day, from, to, label, small: clock(from), title: what, class: if lecture { "" } else { "other" }, ..GridSlot::default() };
    }
    let days: Vec<Day> = dates.iter().filter_map(|date| date.and_then(Day::parse)).collect();
    let small = match (dates.len(), days.as_slice()) {
        (1, [only]) => format!("1 Termin · {}", only.short()),
        (1, _) => "1 Termin".to_string(),
        (n, _) => format!("{n} Termine"),
    };
    // The tooltip names the dates the small line only counts.
    let title = if days.is_empty() {
        what
    } else {
        format!("{what} · {}", days.iter().map(|day| day.german()).collect::<Vec<_>>().join(", "))
    };
    GridSlot { day, from, to, label, small, title, class: "once", ..GridSlot::default() }
}

/// Whether a module's week shows the Studienplan beside it (A.9): its Termine are of the current
/// semester or a later one — a past semester is not compared with a plan — and the plan holds
/// other modules in that semester.
fn overlay_wanted(newest: Option<SemesterKey>, current: Option<SemesterKey>, others: &[String]) -> bool {
    matches!((newest, current), (Some(newest), Some(current)) if newest >= current) && !others.is_empty()
}

/// The Studienplan beside a module's week (A.9), in the browser app: the other modules planned
/// into `newest`, the semester of the module's Termine, as `pages::overlay` sees them against the
/// module. Empty on the server, without a plan in that semester, and when the catalog cannot
/// answer (the overlay only adds to the page).
///
/// Two memos, so that a change of the plan elsewhere (another semester, a module's placeholder)
/// asks the catalog nothing: `asked` reads the stores alone (R16) and keeps what the overlay is
/// made of; the overlay reads `asked` alone. Its questions are the same for every module shown
/// beside the same plan, so only the module's own three are new (`pages::overlay`).
fn plan_overlay(id: &str, newest: Option<SemesterKey>, current: Option<SemesterKey>) -> Memo<Overlay> {
    let plan = Studyplan::expect().filter(|_| APP);
    let mine = MyProgram::expect();
    let source = use_source().ok();
    let asked = {
        let id = id.to_string();
        Memo::new(move |_| {
            let (plan, key) = (plan?, newest?);
            let town = mine.map(MyProgram::town).unwrap_or_default();
            let (others, selection, program) = plan.with(|doc| (doc.modules_in(key).into_iter().filter(|other| *other != id).collect::<Vec<_>>(), doc.selection(key, town), doc.program.clone()));
            let program = program.or_else(|| mine.and_then(|mine| mine.with(|mine| mine.program.clone())));
            overlay_wanted(Some(key), current, &others).then_some((key, others, selection, program))
        })
    };
    let id = id.to_string();
    Memo::new(move |_| {
        let (Some((key, others, selection, program)), Some(source)) = (asked.get(), source.as_ref()) else {
            return Overlay::default();
        };
        source
            .run(|db| {
                let plan = pages::studyplan_in(db, key, &others, program.as_deref(), crate::i18n::locale())?;
                pages::overlay(db, &plan, &id, &selection)
            })
            .unwrap_or_default()
    })
}

/// The module's own slots with the plan beside them (A.9): its slots in a hard clash marked, and
/// the other planned modules' Termine as quiet `.planned` slots cut to the days and hours the
/// module's own week spans, so that the grid never grows when the plan arrives. Planned Termine
/// at the same time are one slot that names each module once, by its abbreviation
/// (`StudyplanData::slot_names`, done in `pages::overlay`): the plan is the context here, and where it
/// meets the module's own slots it takes a slim lane (`crate::week`). The names are the slot's
/// label, not its small line, which phones hide: an outline without a name says nothing.
fn with_overlay(own: &[(GridSlot, Vec<RowKey>)], overlay: &Overlay) -> Vec<GridSlot> {
    let mut slots: Vec<GridSlot> = own.iter().map(|(slot, rows)| GridSlot { clash: rows.iter().any(|row| overlay.clashing.contains(row)), ..slot.clone() }).collect();
    if slots.is_empty() {
        return slots;
    }
    let span = grid::span(&slots.iter().map(GridSlot::placed).collect::<Vec<_>>(), MIN_HOURS.saturating_mul(60));
    /// Day, from and to as drawn; the modules' titles; each title with its whole time (the tooltip).
    type Time<'a> = ((u8, u16, u16), Vec<&'a str>, Vec<String>);
    let mut times: Vec<Time<'_>> = Vec::new();
    for planned in &overlay.planned {
        let (from, to) = (planned.from.max(span.first), planned.to.min(span.last));
        if !(1..=span.days).contains(&planned.day) || to <= from {
            continue;
        }
        let when = format::time_slot(Some(i64::from(planned.day)), Some(&clock(planned.from)), Some(&clock(planned.to)), crate::i18n::locale()).unwrap_or_default();
        let line = format!("{} · {when}", planned.short);
        let at = (planned.day, from, to);
        match times.iter_mut().find(|(known, ..)| *known == at) {
            Some((_, names, lines)) => {
                if !names.contains(&planned.short.as_str()) {
                    names.push(&planned.short);
                }
                if !lines.contains(&line) {
                    lines.push(line);
                }
            }
            None => times.push((at, vec![&planned.short], vec![line])),
        }
    }
    slots.extend(
        times.into_iter().map(|((day, from, to), names, lines)| GridSlot { day, from, to, label: names.join(" · "), title: lines.join("\n"), class: "planned", ..GridSlot::default() }),
    );
    slots
}

/// „Mo 09:15–10:45", and „Mo bis 24:00" for a deadline (an end without a start).
fn shown_when(slot: &Slot) -> Option<String> {
    match (&slot.start_time, &slot.end_time) {
        (None, Some(end)) => Some(match format::time_slot(slot.weekday, None, None, crate::i18n::locale()) {
            Some(day) => format!("{day} bis {end}"),
            None => format!("bis {end}"),
        }),
        (start, end) => format::time_slot(slot.weekday, start.as_deref(), end.as_deref(), crate::i18n::locale()),
    }
}

/// The line under a Termin: type, group and rhythm, its days, then room, lecturers and comment,
/// joined by „ · ". The days are `<time>` elements: „15.02.2027" to a reader, 2027-02-15 to a
/// machine (a search engine reading the page).
fn detail_line(head: Vec<String>, days: Option<(String, Option<String>)>, tail: Vec<String>) -> impl IntoView {
    let mut lead = head.join(" · ");
    let mut rest = tail.join(" · ");
    if !lead.is_empty() && (days.is_some() || !rest.is_empty()) {
        lead.push_str(" · ");
    }
    if days.is_some() && !rest.is_empty() {
        rest.insert_str(0, " · ");
    }
    // An empty text would still be a text node, which the server writes as a space.
    view! {
        {(!lead.is_empty()).then_some(lead)}
        {days.map(|(first, last)| view! { {date_of(&first)}{last.map(|last| view! { " – "{date_of(&last)} })} })}
        {(!rest.is_empty()).then_some(rest)}
    }
}

/// A day as the page writes it, „15.02.2027", with its date for machines; anything that is no
/// `YYYY-MM-DD` as it is.
fn date_of(iso: &str) -> AnyView {
    match Day::parse(iso) {
        Some(day) => view! { <time datetime=day.iso()>{day.german()}</time> }.into_any(),
        None => format::date(iso, crate::i18n::locale()).into_any(),
    }
}

/// „27.12.2015", „08.02.2027 – 19.02.2027"
fn date_range(slot: &Slot) -> Option<String> {
    match (&slot.first_date, &slot.last_date) {
        (Some(first), Some(last)) if first != last => Some(format!("{} – {}", format::date(first, crate::i18n::locale()), format::date(last, crate::i18n::locale()))),
        (Some(first), _) => Some(format::date(first, crate::i18n::locale())),
        _ => None,
    }
}

/// What the BTU wrote, in the words of the row: „So 01:00–02:30 · 27.12.2015". The dates only
/// where the row does not show them already.
fn stated_slot(reading: &ExamReading) -> String {
    let (stated, shown) = (&reading.stated, &reading.shown);
    let dates = (date_range(stated) != date_range(shown)).then(|| date_range(stated)).flatten();
    [format::time_slot(stated.weekday, stated.start_time.as_deref(), stated.end_time.as_deref(), crate::i18n::locale()), dates].into_iter().flatten().collect::<Vec<_>>().join(" · ")
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
    // Each with the semester its validated plan places the module in, where it has one.
    let curricular: Vec<(ProgramLink, Option<String>)> = curricula(&data).into_iter().map(|link| {
        let semesters = plan_semesters(&data, &link);
        (link, semesters)
    }).collect();
    let fues = resolved.iter().filter(|l| l.relation.as_ref().is_some_and(|r| r.is(Relation::Fues))).count();
    let unresolved = data.programs.iter().filter(|l| l.resolve_status.is(ResolveStatus::Unresolved)).count();

    (!data.programs.is_empty()).then(|| view! {
        <div class="section" id="studiengaenge">
            <h3 class="label">"Studiengänge"<span>{curricular.len()}" Curricula"</span></h3>
            {curricular.is_empty().then(|| view! { <p class="hint">"Das Modul gehört zu keinem Curriculum eines Studiengangs im Katalog."</p> })}
            <div class="linklist">
                {curricular.into_iter().map(|(link, semesters)| {
                    let slug = link.program_slug.clone().unwrap_or_default();
                    view! {
                        <a class="pre" href=url::program_path(&slug, ProgramTab::Plan)>
                            <b>
                                {link.program_name.clone().unwrap_or_default()}" · "
                                {link.degree_display.clone().or(link.degree_raw.clone()).unwrap_or_default()}
                                <span class="subtitle">"PO "{link.po_version.clone().unwrap_or_default()}{semesters.map(|semesters| format!(" · {semesters}"))}{link.area.clone().map(|area| format!(" · {area}"))}</span>
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
            room_short: None,
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
                (2, 705, 900, "VL", "1 Termin · 23.02.", "once"),
                (4, 690, 780, "VL", "11:30", ""),
                (1, 825, 915, "Ü", "13:45", "other"),
                (2, 825, 915, "Ü", "13:45", "other"),
            ]
        );
        // QIS's word leads the tooltip.
        assert_eq!(slots.first().map(|s| s.title.as_str()), Some("Vorlesung · Allgemeine Betriebswirtschaftslehre II · 23.02.2027"));
        assert_eq!(slots.get(2).map(|s| s.title.as_str()), Some("Übung · Allgemeine Betriebswirtschaftslehre II"));
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
        let shown: Vec<(u8, &str, &str, &str, &str)> = slots.iter().map(|s| (s.day, s.label.as_str(), s.small.as_str(), s.class, s.title.as_str())).collect();
        let abwl = "Allgemeine Betriebswirtschaftslehre II";
        assert_eq!(
            shown,
            [
                (1, "Ü", "15:30", "other", format!("Übung · {abwl}").as_str()),
                (3, "VL/Ü", "3 Termine", "once", format!("Vorlesung/Übung · {abwl} · 04.11.2026, 11.11.2026, 18.11.2026").as_str()),
                (5, "Sem", "1 Termin · 16.10.", "once", format!("Seminar · {abwl} · 16.10.2026").as_str()),
                (7, "Proj", "22:00", "other", format!("Projekt · {abwl}").as_str()),
            ]
        );
        assert_eq!(slots.get(3).map(|s| s.to), Some(1440));
        // A „Vorlesung/Übung" is a lecture.
        let weekly = teaching("148370", "Vorlesung/Übung", "[unbenannt]", 3, ("09:15", "10:45"), "weekly", "2026-10-14", "2027-01-27");
        assert_eq!(own_slots(&[weekly]).first().map(|s| s.class), Some(""));
        // A type no kind is known of keeps its word, and a row without one says „Termin".
        let odd = teaching("148374", "Blockwoche", "D", 4, ("08:00", "16:00"), "weekly", "2026-10-15", "2027-01-28");
        let bare = EventDate { event_id: "148375".into(), event_type: None, ..odd.clone() };
        assert_eq!(own_slots(&[odd, bare]).iter().map(|s| (s.label.as_str(), s.title.as_str())).collect::<Vec<_>>(), [("Blockwoche", format!("Blockwoche · {abwl}").as_str()), ("Termin", abwl)]);
    }

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    #[test]
    fn the_plan_stands_beside_a_week_of_now_or_later() {
        let others = vec!["12104".to_string()];
        assert!(overlay_wanted(Some(key("2026W")), Some(key("2026W")), &others));
        assert!(overlay_wanted(Some(key("2027S")), Some(key("2026W")), &others));
        // A module whose newest Termine are past is not compared with a plan.
        assert!(!overlay_wanted(Some(key("2026S")), Some(key("2026W")), &others));
        // Nothing else planned in the semester: no overlay, and no line.
        assert!(!overlay_wanted(Some(key("2026W")), Some(key("2026W")), &[]));
        // No Termine, or no current semester: nothing to compare.
        assert!(!overlay_wanted(None, Some(key("2026W")), &others));
        assert!(!overlay_wanted(Some(key("2026W")), None, &others));
    }

    fn semester(text: &str, is_current: bool) -> Semester {
        let at = key(text);
        Semester {
            key: at.key(),
            season: catalog::labels::Code::parse(if at.winter { "winter" } else { "summer" }),
            year: i64::from(at.year),
            label: at.label(crate::i18n::locale()),
            starts_on: String::new(),
            ends_on: String::new(),
            is_current,
            teaching_events: 1,
            exam_events: 1,
        }
    }

    #[test]
    fn einplanen_aims_with_the_teaching_not_with_retakes() {
        use catalog::studyplan::PlanDoc;
        let semesters = [semester("2026S", false), semester("2026W", true), semester("2027S", false)];
        let aim = |schedule: &[EventDate]| {
            let (current, newest) = semesters_of(&semesters, schedule);
            let current = current.unwrap();
            crate::studyplan::target_semester(current, newest, Some(TurnusSeason::Summer), None, &PlanDoc::default())
        };
        // Analysis II (11104), taught in summer: no teaching rows, its only row of WiSe 2026/27 an
        // exam (a retake). The exam is no reason to plan it into the winter: the next summer.
        assert_eq!(semesters_of(&semesters, &[]), (Some(key("2026W")), None));
        assert_eq!(aim(&[]), key("2027S"));
        // Taught last summer: the next summer too.
        let taught = EventDate { semester_key: "2026S".into(), ..teaching("149001", "Vorlesung", "[unbenannt]", 2, ("09:15", "10:45"), "weekly", "2026-04-14", "2026-07-14") };
        assert_eq!(semesters_of(&semesters, std::slice::from_ref(&taught)).1, Some(key("2026S")));
        assert_eq!(aim(&[taught]), key("2027S"));
        // Taught this winter after all: this winter, whatever the turnus says.
        let now = teaching("149002", "Vorlesung", "[unbenannt]", 2, ("09:15", "10:45"), "weekly", "2026-10-13", "2027-01-26");
        assert_eq!(aim(&[now]), key("2026W"));
    }

    #[test]
    fn the_plan_beside_a_week_keeps_the_week_s_frame() {
        use catalog::pages::OverlaySlot;
        // Analysis I's Übung on Tuesday 09:15 and its lecture on Thursday 11:30.
        let rows = [
            teaching("150132", "Übung", "1-Gruppe", 2, ("09:15", "10:45"), "weekly", "2026-10-13", "2027-01-26"),
            teaching("148663", "Vorlesung", "[unbenannt]", 4, ("11:30", "13:00"), "weekly", "2026-10-15", "2027-01-28"),
        ];
        let own = own_groups(&rows);
        let clashing = own.first().and_then(|(_, rows)| rows.first().copied());
        let named = |name: &str, day, from: &str, to: &str| OverlaySlot { module: "12102".into(), short: name.into(), day, from: minutes(from).unwrap(), to: minutes(to).unwrap() };
        let planned = |day, from: &str, to: &str| named("Programmierpraktikum", day, from, to);
        let overlay = Overlay {
            planned: vec![
                // Inside the frame, beside the Übung it clashes with; two modules at one time are
                // one slot.
                planned(2, "09:15", "10:45"),
                named("Elektrische und elektronische Grundlagen der Informatik", 2, "09:15", "10:45"),
                // Reaching past the frame: cut to it (08:00–13:00).
                planned(4, "12:30", "14:00"),
                planned(1, "07:30", "09:00"),
                // Wholly outside it, or on a Saturday the week does not have: left out.
                planned(3, "16:00", "17:30"),
                planned(6, "09:00", "12:00"),
            ],
            clashing: clashing.into_iter().collect(),
            ..Overlay::default()
        };
        let slots = with_overlay(&own, &overlay);
        let shown: Vec<(u8, String, String, &str, bool)> = slots.iter().map(|s| (s.day, clock(s.from), clock(s.to), s.class, s.clash)).collect();
        assert_eq!(
            shown,
            [
                (2, "09:15".into(), "10:45".into(), "other", true),
                (4, "11:30".into(), "13:00".into(), "", false),
                (2, "09:15".into(), "10:45".into(), "planned", false),
                // Half an hour after the cut: the grid gives it one line (`brief`).
                (4, "12:30".into(), "13:00".into(), "planned", false),
                (1, "08:00".into(), "09:00".into(), "planned", false),
            ]
        );
        assert!(slots.get(3).is_some_and(|s| s.classes() == "slot planned brief"));
        // On the Tuesday the planned slot takes the planned part beside the Übung's own.
        let (_, styles) = crate::week::geometry(&slots, MIN_HOURS).unwrap();
        assert!(styles[0].ends_with(";--mine:1;--theirs:1") && styles[2].ends_with(";--mine:1;--theirs:1;--beside:1"), "{styles:?}");
        // The grid's frame is the module's own, with the plan beside it or not.
        let own_slots: Vec<GridSlot> = own.iter().map(|(slot, _)| slot.clone()).collect();
        let frame = |slots: &[GridSlot]| crate::week::geometry(slots, MIN_HOURS).map(|(week, _)| week);
        assert_eq!(frame(&slots), frame(&own_slots));
        // A planned slot names its modules in its label (phones hide the small line), and their
        // times in the tooltip.
        assert_eq!(
            slots.get(2).map(|s| (s.label.as_str(), s.small.as_str(), s.title.as_str())),
            Some((
                "Programmierpraktikum · Elektrische und elektronische Grundlagen der Informatik",
                "",
                "Programmierpraktikum · Di 09:15–10:45\nElektrische und elektronische Grundlagen der Informatik · Di 09:15–10:45"
            ))
        );
        assert_eq!(slots.get(3).map(|s| s.title.as_str()), Some("Programmierpraktikum · Do 12:30–14:00"));
        // Without a plan the week is the module's alone.
        assert_eq!(with_overlay(&own, &Overlay::default()), own_slots);
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

    /// Analysis I as the page shows it, with these Termine and exam dates: taught in the
    /// Informatik B.Sc., whose plan places it in the first semester, and requiring module 11000.
    fn analysis(schedule: Vec<EventDate>, exams: Vec<EventDate>) -> ModuleData {
        use catalog::labels::Code;
        let module = Module {
            id: "11101".into(),
            title: "Analysis I".into(),
            title_de: Some("Analysis I".into()),
            title_en: None,
            credits: Some(8.0),
            language_raw: Some("Deutsch".into()),
            teaches_german: Some(true),
            teaches_english: Some(false),
            duration_raw: None,
            duration_semesters: None,
            turnus_raw: None,
            turnus_season: Some(Code::parse("winter")),
            turnus_parity: None,
            offer_status: Code::parse("active"),
            limitation_raw: None,
            is_limited: None,
            participant_limit: None,
            exam_form: None,
            exam_form_raw: None,
            exam_details: None,
            grading_raw: None,
            is_graded: None,
            is_fues: false,
            department: None,
            learning_outcomes: None,
            contents: Some("Folgen und Reihen, Stetigkeit, Differentialrechnung.".into()),
            prerequisites_recommended: None,
            prerequisites_mandatory: None,
            remarks: None,
            source_url: Some("https://www.b-tu.de/modul/11101".into()),
            fetched_at: None,
            at_zentralcampus: Some(true),
            at_sachsendorf: None,
            at_senftenberg: None,
        };
        let informatik = ProgramLink {
            degree_raw: Some("Bachelor (universitär)".into()),
            program_raw: Some("Informatik".into()),
            po_raw: Some("2008".into()),
            resolve_status: Code::parse("resolved"),
            program_id: Some("079-82-2008".into()),
            program_slug: Some("bachelor-informatik-2008".into()),
            program_name: Some("Informatik".into()),
            degree_display: Some("B.Sc.".into()),
            po_version: Some("2008".into()),
            is_latest_po: Some(true),
            relation: Some(Code::parse("curricular")),
            kind: Some(Code::parse("compulsory")),
            kind_source: None,
            area: None,
        };
        let semester = |key: &str, label: &str, starts: &str, ends: &str| Semester {
            key: key.into(),
            season: Code::parse(if key.ends_with('W') { "winter" } else { "summer" }),
            year: 2026,
            label: label.into(),
            starts_on: starts.into(),
            ends_on: ends.into(),
            is_current: key == "2026W",
            teaching_events: 1,
            exam_events: 1,
        };
        ModuleData {
            module,
            lecturers: Vec::new(),
            teaching_forms: Vec::new(),
            text_items: Vec::new(),
            prerequisites: vec![Prerequisite {
                module_id: "11101".into(),
                required_module_id: "11000".into(),
                kind: Code::parse("mandatory"),
                required_title: Some("Brückenkurs Mathematik".into()),
                required_offer_status: None,
            }],
            successors: Vec::new(),
            schedule,
            exams,
            programs: vec![informatik],
            plan_places: vec![catalog::rows_detail::PlanPlace { program_id: "079-82-2008".into(), semester: Some(1), start_semester: Some(1), end_semester: Some(1) }],
            semesters: vec![semester("2026S", "SoSe 2026", "2026-04-01", "2026-09-30"), semester("2026W", "WiSe 2026/27", "2026-10-01", "2027-03-31")],
        }
    }

    #[test]
    fn structured_data_states_the_termine_the_exams_and_the_plan_s_semester() {
        let schedule = vec![
            // Monday and Wednesday at the same time over the same weeks: one slot on two days.
            teaching("148663", "Vorlesung", "[unbenannt]", 1, ("09:15", "10:45"), "weekly", "2026-10-12", "2027-01-25"),
            teaching("148663", "Vorlesung", "[unbenannt]", 3, ("09:15", "10:45"), "weekly", "2026-10-12", "2027-01-25"),
            teaching("150132", "Übung", "1-Gruppe", 2, ("13:45", "15:15"), "week_a", "2026-10-13", "2027-01-26"),
            teaching("148663", "Vorlesung", "[unbenannt]", 2, ("11:45", "15:00"), "single", "2027-02-23", "2027-02-23"),
            // No time of the week: the page says „Zeit offen", the data says nothing.
            teaching("150133", "Tutorium", "C", 2, ("10:00", ""), "weekly", "2026-10-13", "2027-01-26"),
            teaching("150134", "Konsultation", "D", 5, ("10:00", "12:00"), "other", "2026-10-16", "2027-01-29"),
        ];
        let exams = vec![
            EventDate { room: Some("Audimax 1".into()), ..exam(Some(1), Some("09:00"), Some("11:00"), Some("2027-02-15")) },
            // QIS's placeholder for a date not fixed yet, and a deadline.
            exam(Some(7), Some("01:00"), Some("02:30"), Some("2015-12-27")),
            exam(Some(7), Some("23:45"), Some("24:00"), Some("2027-03-21")),
        ];
        let graph = structured(&analysis(schedule, exams));
        let course = &graph[0];
        assert_eq!(course["educationalAlignment"][0]["targetName"], "1. Semester");
        assert_eq!(course["educationalAlignment"][0]["educationalFramework"], "Regelstudienplan Informatik (B.Sc.), PO 2008");
        assert_eq!(course["educationalAlignment"][0]["targetUrl"], "https://betula.app/programs/bachelor-informatik-2008/plan");
        assert_eq!(course["coursePrerequisites"][0]["@id"], "https://betula.app/catalog/module/11000");

        let instances = course["hasCourseInstance"].as_array().unwrap();
        assert_eq!(instances.len(), 1, "exams of the teaching semester belong to its instance: {instances:?}");
        let winter = &instances[0];
        assert_eq!((&winter["name"], &winter["startDate"], &winter["endDate"]), (&"Analysis I (WiSe 2026/27)".into(), &"2026-10-12".into(), &"2027-02-23".into()));
        assert_eq!(winter["location"][0]["name"], "Zentralcampus Cottbus");
        let week: Vec<String> = winter["courseSchedule"]
            .as_array()
            .unwrap()
            .iter()
            .map(|slot| {
                let days: Vec<&str> = slot["byDay"].as_array().unwrap().iter().filter_map(|day| day.as_str()?.strip_prefix("https://schema.org/")).collect();
                let field = |name: &str| slot[name].as_str().unwrap_or("-").to_string();
                format!("{} {} {}–{} {}…{} {}", field("name"), days.join("+"), field("startTime"), field("endTime"), field("startDate"), field("endDate"), field("repeatFrequency"))
            })
            .collect();
        assert_eq!(
            week,
            [
                "Vorlesung Monday+Wednesday 09:15–10:45 2026-10-12…2027-01-25 P1W",
                "Übung (1-Gruppe) Tuesday 13:45–15:15 2026-10-13…2027-01-26 P2W",
                "Vorlesung Tuesday 11:45–15:00 2027-02-23…2027-02-23 -",
            ]
        );
        assert!(winter["courseSchedule"].as_array().unwrap().iter().all(|slot| slot["scheduleTimezone"] == "Europe/Berlin"));
        let exams: Vec<String> = winter["subEvent"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| format!("{} {} {} {}", event["name"], event["startDate"], event["endDate"], event["location"]["name"]))
            .collect();
        assert_eq!(
            exams,
            [
                "\"Prüfung Analysis I\" \"2027-02-15T09:00:00+01:00\" \"2027-02-15T11:00:00+01:00\" \"Audimax 1\"",
                "\"Prüfung Analysis I\" \"2027-03-21\" null null",
            ]
        );

        // What it costs a page: the whole block, with the breadcrumbs.
        let json = seo::json_ld(&serde_json::json!({ "@context": "https://schema.org", "@graph": graph }));
        assert!(json.len() < 3_000, "{} bytes: {json}", json.len());
    }

    #[test]
    fn exams_of_another_semester_are_an_instance_of_their_own() {
        // Taught in the summer, retaken in the winter.
        let summer = EventDate { semester_key: "2026S".into(), semester_label: "SoSe 2026".into(), ..teaching("149001", "Vorlesung", "[unbenannt]", 2, ("09:15", "10:45"), "weekly", "2026-04-14", "2026-07-14") };
        let retake = exam(Some(3), Some("13:00"), Some("15:00"), Some("2026-10-07"));
        let graph = structured(&analysis(vec![summer], vec![retake.clone()]));
        let instances = graph[0]["hasCourseInstance"].as_array().unwrap();
        let shown: Vec<(&str, Option<&str>, usize)> = instances
            .iter()
            .map(|i| (i["name"].as_str().unwrap(), i["startDate"].as_str(), i["subEvent"].as_array().map_or(0, Vec::len)))
            .collect();
        assert_eq!(shown, [("Analysis I (SoSe 2026)", Some("2026-04-14"), 0), ("Analysis I (WiSe 2026/27)", Some("2026-10-07"), 1)]);
        assert_eq!(instances[1]["subEvent"][0]["startDate"], "2026-10-07T13:00:00+02:00");
        assert!(instances[1].get("courseSchedule").is_none() && instances[1].get("location").is_none());

        // Neither Termine nor exam dates the page could state: no instance at all.
        let placeholder = exam(Some(7), Some("01:00"), Some("02:30"), Some("2015-12-27"));
        assert!(structured(&analysis(Vec::new(), vec![placeholder]))[0].get("hasCourseInstance").is_none());
    }
}
