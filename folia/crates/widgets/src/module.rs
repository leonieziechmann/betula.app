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

use folia_calendar::day::{clock, minutes, Day};
use folia_calendar::kind::{class_of, kinds_of, Class, EventKind, KindSet};
use folia_calendar::rowkey::RowKey;
use folia_calendar::semester::SemesterKey;
use folia_model::labels::{
    Campus, Labelled, OfferStatus, PrerequisiteKind, Relation, ResolveStatus, Rhythm, TeachingForm, TextItemKind,
    TurnusSeason,
};
use folia_model::rows::{Module, Prerequisite, Semester};
use folia_model::rows_detail::{EventDate, ProgramLink, Series};
use folia_pages::ask::OverlayAsk;
use folia_pages::{ModuleData, Overlay};
use folia_routes::url::{self, ProgramTab};
use folia_timetable::exam_reading::{self, ExamReading, Reason, Slot};
use folia_timetable::grid;
use folia_timetable::ics::TZID;
use leptos::prelude::*;
use folia_design::stage::Title;

use folia_stores::bookmarks::{MarkButton, MarkLook};
use folia_data::use_data;
use folia_design::format;
use crate::i18n::{self, use_location, Locale, Texts};
use folia_stores::myprogram::MyProgram;
use folia_shell::seo::{self, Seo};
use folia_stores::studyplan::{PlanButton, PlanHint, PlanLook, Studyplan};
use folia_shell::pending::same_page;
use folia_shell::tabs::{self, Area, Tabs};
use folia_shell::frame::{BackLink, Frame};
use folia_design::ui::{Fact, Icon, JsOnly, KindBadge, OfferBadge, Prose, Shortcut};
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

fn derive(data: &ModuleData, t: &'static Texts) -> Derived {
    let m = &data.module;
    let credits = format::credits(m.credits, t.locale);
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
        campus: campuses(m, t.locale),
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
            .map(|text| seo::excerpt(&(t.module.description)(&m.title, &m.id, &credits, &folia_model::text::plain(&text)), 300))
            .unwrap_or_else(|| (t.module.description_bare)(&m.title, &m.id, &credits)),
    }
}

/// The campuses the module is taught at, as the page names them.
fn campuses(m: &Module, locale: Locale) -> Vec<&'static str> {
    [(m.at_zentralcampus, Campus::Zentralcampus), (m.at_sachsendorf, Campus::Sachsendorf), (m.at_senftenberg, Campus::Senftenberg)]
        .iter()
        .filter(|(at, _)| *at == Some(true))
        .map(|(_, campus)| campus.label(locale))
        .collect()
}

/// A semester as the page names it in its language („WiSe 2026/27", "Winter 2026/27"), from its
/// key; the snapshot's label where the key is none.
fn semester_name(key: &str, label: &str, locale: Locale) -> String {
    SemesterKey::parse(key).map_or_else(|| label.to_string(), |key| key.label(locale))
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
fn plan_semesters(data: &ModuleData, link: &ProgramLink, locale: Locale) -> Option<String> {
    format::plan_semesters(&data.plan_semesters(link.program_id.as_deref()?), locale)
}

/// The module as schema.org knows it (a `Course` of the university) and the way to it. Only what
/// the page shows, and what a search engine answers questions with: when and where the module is
/// taught and examined, what it requires, and in which semester the study plans place it. Its
/// Termine are the course's instance in their semester (`schedule_of`), its exam dates events of
/// the semester they are listed under (`exam_event`). Kept small: the texts only as the short
/// description, no Termin without a time of the week, no exam date the page marks as doubtful.
/// In the page's language: what the app words itself (the semesters, the plan, the breadcrumbs).
fn structured(data: &ModuleData, t: &'static Texts) -> Vec<serde_json::Value> {
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
            course.insert("description".into(), seo::excerpt(&folia_model::text::plain(text), 500).into());
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
        let plans = plan_alignments(data, t);
        if !plans.is_empty() {
            course.insert("educationalAlignment".into(), plans.into());
        }
        let instances = course_instances(data, t);
        if !instances.is_empty() {
            course.insert("hasCourseInstance".into(), instances.into());
        }
    }
    vec![
        course,
        seo::breadcrumbs(&[("Betula", url::HOME.to_string()), (t.module.breadcrumb_catalog, url::CATALOG.to_string()), (m.title.as_str(), url::module_path(&m.id))]),
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
fn plan_alignments(data: &ModuleData, t: &'static Texts) -> Vec<serde_json::Value> {
    let mut seen = BTreeSet::new();
    curricula(data)
        .iter()
        .filter(|link| link.program_id.as_ref().is_some_and(|id| seen.insert(id.clone())))
        .filter_map(|link| {
            let semesters = plan_semesters(data, link, t.locale)?;
            let slug = link.program_slug.as_deref()?;
            let degree = link.degree_display.clone().or(link.degree_raw.clone()).unwrap_or_default();
            let po = link.po_version.clone().unwrap_or_default();
            Some(serde_json::json!({
                "@type": "AlignmentObject",
                "alignmentType": "educationalLevel",
                "educationalFramework": (t.module.plan_framework)(link.program_name.as_deref().unwrap_or_default(), &degree, &po),
                "targetName": semesters,
                "targetUrl": seo::absolute(&url::program_path(slug, ProgramTab::Plan)),
            }))
        })
        .collect()
}

/// The module's Termine as schema.org's course instances, one per semester the page shows them
/// for: the teaching semester's with a `Schedule` per slot of its week, and the exam dates as
/// its `subEvent`s where they are of the same semester, else as an instance of their own.
fn course_instances(data: &ModuleData, t: &'static Texts) -> Vec<serde_json::Value> {
    let m = &data.module;
    let newest = data.schedule.first().map(|d| (d.semester_key.clone(), semester_name(&d.semester_key, &d.semester_label, t.locale)));
    let teaching: Vec<&EventDate> = data.schedule.iter().filter(|d| newest.as_ref().is_some_and(|(key, _)| *key == d.semester_key)).collect();
    let exam_semester = data.exams.first().map(|d| (d.semester_key.clone(), semester_name(&d.semester_key, &d.semester_label, t.locale)));
    let exam_row = exam_semester.as_ref().and_then(|(key, _)| data.semesters.iter().find(|s| s.key == *key));
    let mut exams: Vec<(Day, serde_json::Value)> = data
        .exams
        .iter()
        .filter(|d| exam_semester.as_ref().is_some_and(|(key, _)| *key == d.semester_key))
        .filter_map(|d| exam_event(d, &exam_reading::read(d, exam_row), t))
        .collect();
    let shared = newest.as_ref().zip(exam_semester.as_ref()).is_some_and(|((teaching, _), (exam, _))| teaching == exam);
    let mut instances = Vec::new();
    if let Some((_, label)) = &newest {
        let own = if shared { std::mem::take(&mut exams) } else { Vec::new() };
        instances.push(course_instance(m, label, &teaching, own, t.locale));
    }
    if let Some((_, label)) = exam_semester.filter(|_| !exams.is_empty()) {
        instances.push(course_instance(m, &label, &[], exams, t.locale));
    }
    instances
}

/// One semester of the module: from its first Termin to its last, where it is taught, the slots
/// of its week and its exam dates. An instance of exam dates alone spans them.
fn course_instance(m: &Module, semester: &str, teaching: &[&EventDate], exams: Vec<(Day, serde_json::Value)>, locale: Locale) -> serde_json::Value {
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
    let places: Vec<serde_json::Value> = campuses(m, locale).into_iter().map(|name| serde_json::json!({ "@type": "Place", "name": name })).collect();
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
/// one. A row without a time, or „nach Absprache", has no slot (the page says „Zeit offen"). The
/// dates of a series of single dates at one time (`termine`) are one slot in the series' rhythm,
/// with the days it leaves out.
fn schedule_of(teaching: &[&EventDate]) -> Vec<serde_json::Value> {
    /// What the slot is, its times, its dates, how often it repeats and the days it leaves out.
    type Slot = (String, u16, u16, Option<String>, Option<String>, Option<&'static str>, Vec<String>);
    let mut slots: Vec<(Slot, Vec<u8>)> = Vec::new();
    // A series of single dates is one slot in its rhythm; every other row its own, rows alike
    // made one below.
    let entries = termine(teaching.iter().copied()).into_iter().flat_map(|rows| {
        let days: BTreeSet<&str> = rows.iter().filter_map(|row| row.first_date.as_deref()).collect();
        match rows.first().and_then(|d| d.series).filter(|_| days.len() > 1) {
            Some(_) => vec![rows],
            None => rows.into_iter().map(|row| vec![row]).collect(),
        }
    });
    for rows in entries {
        let Some(d) = rows.first() else { continue };
        let Some(day) = d.weekday.and_then(|day| u8::try_from(day).ok()).filter(|day| (1..=7).contains(day)) else { continue };
        let (Some(from), Some(to)) = (d.start_time.as_deref().and_then(minutes), d.end_time.as_deref().and_then(minutes)) else { continue };
        if to <= from {
            continue;
        }
        let days: BTreeSet<Day> = rows.iter().filter_map(|row| row.first_date.as_deref().and_then(Day::parse)).collect();
        let series = d.series.filter(|_| days.len() > 1);
        let repeat = match (series, d.rhythm.as_ref().and_then(|rhythm| rhythm.known())) {
            (Some(series), _) => Some(match series.weeks {
                2 => "P2W",
                3 => "P3W",
                4 => "P4W",
                _ => "P1W",
            }),
            (None, Some(Rhythm::Other)) => continue,
            (None, Some(Rhythm::Weekly)) => Some("P1W"),
            (None, Some(Rhythm::WeekA | Rhythm::WeekB)) => Some("P2W"),
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
        let (first, last, except) = match (series, days.first(), days.last()) {
            (Some(series), Some(first), Some(last)) => {
                let step = 7 * i32::from(series.weeks);
                let made = std::iter::successors(Some(*first), |day| Some(day.plus(step))).take_while(|day| day <= last);
                (Some(first.iso()), Some(last.iso()), made.filter(|day| !days.contains(day)).map(|day| day.iso()).collect())
            }
            _ => {
                let first = d.first_date.as_deref().and_then(Day::parse).map(Day::iso);
                let last = d.last_date.as_deref().and_then(Day::parse).map(Day::iso).or(first.clone());
                (first, last, Vec::new())
            }
        };
        let slot = (name, from, to, first, last, repeat, except);
        match slots.iter_mut().find(|(known, _)| *known == slot) {
            Some((_, days)) if !days.contains(&day) => days.push(day),
            Some(_) => {}
            None => slots.push((slot, vec![day])),
        }
    }
    slots
        .into_iter()
        .map(|((name, from, to, first, last, repeat, except), mut days)| {
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
            if !except.is_empty() {
                slot.insert("exceptDate".into(), except.into());
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
fn exam_event(date: &EventDate, reading: &ExamReading, t: &'static Texts) -> Option<(Day, serde_json::Value)> {
    if reading.is_marked() {
        return None;
    }
    let shown = &reading.shown;
    let first = Day::parse(shown.first_date.as_deref()?)?;
    let last = shown.last_date.as_deref().and_then(Day::parse).unwrap_or(first);
    let at = |day: Day, time: Option<&str>| time.and_then(|time| seo::berlin_time(day, time)).unwrap_or_else(|| day.iso());
    let mut event = serde_json::Map::new();
    event.insert("@type".into(), "EducationEvent".into());
    event.insert("name".into(), (t.module.exam_event)(&date.event_title).into());
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
/// module stands can show it in full itself (a program's page). Both are paths of the app,
/// without the language's prefix: the panel writes them as links of the page's language. `hint`
/// says where „Einplanen" plans to when the list beside it was asked for a semester or a
/// placeholder (the finder).
#[component]
pub fn ModulePanel(
    data: ModuleData,
    close_href: String,
    #[prop(optional)] docked: bool,
    #[prop(optional_no_strip)] full_href: Option<String>,
    #[prop(optional, into)] hint: Signal<Option<PlanHint>>,
) -> impl IntoView {
    let t = i18n::t();
    let id = data.module.id.clone();
    // The module's own page keeps the hint (`?plan=…&fill=…`, as on a phone), so „Einplanen"
    // aims there as it does here. The browser app's alone: the server's pages carry no hint.
    let full_href = {
        let id = id.clone();
        move || match (&full_href, hint.get().filter(|_| APP)) {
            (Some(href), _) => t.path(href),
            (None, Some(hint)) => t.path(&format!("{}{}", url::module_path(&id), hint.query())),
            (None, None) => t.path(&url::module_path(&id)),
        }
    };
    // Opened from the page it closes to (the step before in the history): closing it on a phone,
    // where it is a sheet over that page, goes back that step, as Back does (enhance.js), and the
    // history does not grow.
    let back = {
        let (location, tabs, page) = (use_location(), Tabs::expect(), close_href.clone());
        move || {
            let now = tabs::location_of(&location.pathname.get(), &location.search.get());
            tabs.is_some_and(|tabs| same_page(&tabs.before(&now), &page)).then_some("history")
        }
    };
    let close_href = t.path(&close_href);
    view! {
        <section class="panel detail is-module" class:aside=docked id="preview" aria-label=t.module.preview>
            <div class="scroll" data-keep-scroll="detail">
                <header class="hero">
                    <div class="hero-top">
                        // Closing it leaves the page where it is (on a phone the window is what scrolls).
                        <a class="icon-btn back" href=close_href.clone() data-noscroll="" aria-label=t.module.close_preview><Icon name="arrow-left"/></a>
                        <span class="mono">{id.clone()}</span>
                        // Not on a phone, where the panel is a sheet that goes all up (app.css).
                        <a class="ghost full-view" href=full_href data-action="fullscreen" title=t.module.full_view_title><Icon name="maximize-2"/>{t.common.full_view}<Shortcut keys="F"/></a>
                        <a class="ghost" href=close_href data-action="close-detail" data-back=back data-noscroll="" title=t.module.close_preview_title><Icon name="x"/>{t.common.close}<Shortcut keys="Esc"/></a>
                    </div>
                    <Heading data=data.clone() hint/>
                </header>
                <div class="dbody">
                    <Side data=data.clone()/>
                    <Main data=data.clone()/>
                    <DataClient data=data.clone()/>
                </div>
            </div>
        </section>
    }
}

/// The sections a module has, in the order of the page: (anchor, heading). The anchors are the
/// same in every language, so that a link to a section holds in each.
fn sections(data: &ModuleData, t: &'static Texts) -> Vec<(&'static str, &'static str)> {
    let m = &data.module;
    let literature = data.text_items.iter().any(|i| i.kind.is(TextItemKind::Literature));
    let prerequisites = m.prerequisites_mandatory.is_some() || m.prerequisites_recommended.is_some() || !data.prerequisites.is_empty();
    let words = &t.module;
    [
        (true, "termine", words.dates),
        (!data.exams.is_empty(), "pruefungstermine", words.exam_dates),
        (true, "blick", words.at_a_glance),
        (prerequisites, "voraussetzungen", words.prerequisites),
        (m.contents.is_some(), "inhalte", words.contents),
        (m.learning_outcomes.is_some(), "lernziele", words.learning_outcomes),
        (m.exam_details.is_some(), "pruefungsleistung", words.assessment),
        (literature, "literatur", words.literature),
        (m.remarks.is_some(), "bemerkungen", words.remarks),
        (!data.programs.is_empty(), "studiengaenge", words.programs),
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
    let t = i18n::t();
    let source_url = data.module.source_url.clone();
    let (id, title) = (data.module.id.clone(), data.module.title.clone());
    view! {
        <nav class="toc jumps" aria-label=t.module.on_this_page>
            <p class="flabel label">{t.module.on_this_page}</p>
            {sections(&data, t).into_iter().map(|(anchor, heading)| view! {
                <a href=format!("#{anchor}") data-action="jump">{heading}</a>
            }).collect_view()}
        </nav>
        <div class="fgroup actions">
            <p class="flabel label">{t.module.actions}</p>
            <MarkButton id title look=MarkLook::Action/>
            {plan_button(&data, hint, PlanLook::Action)}
            <JsOnly><a class="action" href="#" data-action="copy-link"><Icon name="share-2"/><span>{t.module.copy_link}</span></a></JsOnly>
            {source_url.map(|href| view! { <a class="action" href=href rel="noopener"><Icon name="arrow-up-right"/>{t.module.original_at_btu}</a> })}
        </div>
    }
}

/// The module's whole page: the frame with the module's sidebar, the module on the rest of the
/// screen. One component wherever the page is shown; `back_area` and `back_to` say where
/// „Zurück" leads (the list of the area, or the page `back_to` names: a path of the app, which
/// `BackLink` writes as a link of the page's language). `noindex` marks the page
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
    let t = i18n::t();
    let derived = derive(&data, t);
    view! {
        // The name first (what people search for), then number and university.
        <Title text=(t.module.title)(&data.module.title, &data.module.id)/>
        <Frame
            title=t.module.sidebar_title
            head={ let id = data.module.id.clone(); move || view! { <span class="mono">{id.clone()}</span> } }
            sidebar={ let data = data.clone(); move || view! { <Sidebar data=data.clone() hint/> } }
        >
                <Seo
                    title=format!("{} ({})", data.module.title, data.module.id)
                    description=derived.description
                    path=url::module_path(&data.module.id)
                    card=folia_shell::seo::module_card(&data.module.id)
                    data=structured(&data, t)
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
                        <div class="panel dbody"><Main data=data.clone()/><DataClient data=data.clone()/></div>
                    </div>
                </article>
        </Frame>
    }
}

/// The snapshot's current semester, and the semester of the module's newest teaching Termine (the
/// week's, `Schedule`): what „Einplanen" aims with (`studyplan::target_semester`). Exams do not
/// count: a module taught in summer holds retakes in the winter too, and its only rows of a winter
/// would plan it into a semester it is not taught in, where its turnus says the next summer. A row
/// swiped to the right aims with the same (`crate::swipe`).
pub(crate) fn semesters_of(semesters: &[Semester], schedule: &[EventDate]) -> (Option<SemesterKey>, Option<SemesterKey>) {
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
        view! { <PlanButton id=m.id.clone() title=m.title.clone() turnus=m.turnus_season.clone() parity=m.turnus_parity.clone() current newest hint look/> }
    })
}

#[component]
fn Heading(data: ModuleData, hint: Signal<Option<PlanHint>>) -> impl IntoView {
    let t = i18n::t();
    let m = data.module.clone();
    let derived = derive(&data, t);
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
                <span class="badge strong num">{format::credits(m.credits, t.locale)}</span>
                <span class="badge">{format::turnus(m.turnus_season.as_ref(), m.turnus_parity.as_ref(), t.locale)}</span>
                {format::languages(m.teaches_german, m.teaches_english).map(|l| view! { <span class="badge">{l}</span> })}
                // „FÜS" is the programme's name in every language.
                {m.is_fues.then(|| view! { <span class="badge">"FÜS"</span> })}
                {(!m.offer_status.is(OfferStatus::Active)).then(|| view! { <span class="badge warn">{m.offer_status.label(t.locale).to_string()}</span> })}
            </p>
            <div class="switches">
                {plan_button(&data, hint, PlanLook::Hero)}
                <MarkButton id=m.id.clone() title=m.title.clone() look=MarkLook::Hero/>
            </div>
        </div>
    }
}

/// The longest value of a fact of „Auf einen Blick" that stands beside another, some two lines of
/// it on a phone: a longer one takes the line of the facts to itself. One whose longest word does
/// not fit beside another takes it as well, by the stylesheet (app.css `.facts`).
const FACT_BESIDE: usize = 24;

/// Whether a fact takes the line of the facts to itself: its value is longer than `FACT_BESIDE`.
fn fact_wide(value: Option<&str>) -> bool {
    value.is_some_and(|value| value.chars().count() > FACT_BESIDE)
}

/// Schedule and key facts: the right column of the page, the top of the preview.
#[component]
fn Side(data: ModuleData) -> impl IntoView {
    let t = i18n::t();
    let m = data.module.clone();
    let derived = derive(&data, t);
    let words = &t.module;
    // Two facts side by side where their values are short (owner, 2026-10-17: „Da könnten einige
    // auch nebeneinander passen. Setz das mal bitte um, so dass möglichst effizient vertical space
    // frei gegeben wird"); the persons and the teaching forms always have a line of their own.
    let exam = m.exam_form.as_ref().map(|form| format::exam_short(form, t.locale)).or(m.exam_form_raw.clone());
    let grading = m.is_graded.map(|g| if g { words.graded.to_string() } else { words.ungraded.to_string() }).or(m.grading_raw.clone());
    let duration = m.duration_raw.clone();
    let places = match (m.is_limited, m.participant_limit) {
        (Some(true), Some(n)) => Some((words.places_max)(n)),
        (Some(true), None) => m.limitation_raw.clone().or(Some(words.limited.to_string())),
        (Some(false), _) => Some(words.unlimited.to_string()),
        (None, _) => None,
    };
    let language = m.language_raw.clone();
    let location = (!derived.campus.is_empty()).then(|| derived.campus.join(", "));
    view! {
        {(!data.successors.is_empty()).then(|| view! {
            <p class="note">
                <Icon name="info"/>
                <span>
                    {if m.offer_status.is(OfferStatus::Active) { words.successor } else { words.replaced_by }}
                    {data.successors.iter().map(|s| view! {
                        <a href=t.path(&url::module_path(&s.successor_id))>{s.successor_id.clone()}" "{s.successor_title.clone().unwrap_or_default()}</a>" "
                    }).collect_view()}
                </span>
            </p>
        })}
        <Schedule data=data.clone()/>
        <div class="section" id="blick">
            <h3 class="label">{words.at_a_glance}</h3>
            <dl class="facts">
                <Fact icon="file-check-2" label=words.exam wide=fact_wide(exam.as_deref()) value=exam/>
                <Fact icon="award" label=words.grading wide=fact_wide(grading.as_deref()) value=grading/>
                <Fact icon="clock-3" label=words.duration wide=fact_wide(duration.as_deref()) value=duration/>
                <Fact icon="users-round" label=words.places wide=fact_wide(places.as_deref()) value=places/>
                <Fact icon="languages" label=words.language wide=fact_wide(language.as_deref()) value=language/>
                <Fact icon="map-pin" label=words.location wide=fact_wide(location.as_deref()) value=location/>
                <Fact wide=true icon="user-round" label=words.responsible value=(!derived.responsible.is_empty()).then(|| derived.responsible.join("; "))/>
                <Fact wide=true icon="layout-list" label=words.teaching_forms value=(!derived.workload.is_empty()).then(|| derived.workload.join(" · "))/>
            </dl>
        </div>
    }
}

/// Prerequisites and the texts of the module description, and last the programs: the main column of
/// the page. The programs stand at the end of the module (owner, 2026-10-17: „die Studiengänge
/// können ganz an den Schluss das interessiert einen wahrscheinlich eh nicht").
#[component]
fn Main(data: ModuleData) -> impl IntoView {
    let t = i18n::t();
    let words = &t.module;
    let m = data.module.clone();
    let derived = derive(&data, t);
    let has_prerequisites = m.prerequisites_mandatory.is_some() || m.prerequisites_recommended.is_some() || !data.prerequisites.is_empty();
    let literature = derived.literature.clone();
    // The texts are in the language of the module's page, whatever the language of Betula's.
    let lang = {
        let lang = m.page_lang.clone();
        move || lang.clone()
    };
    let prerequisite_links = |linked: Vec<Prerequisite>, kind: &'static str| {
        linked
            .into_iter()
            .map(|p| view! {
                <a class="pre" href=t.path(&url::module_path(&p.required_module_id))>
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
                <h3 class="label">{words.prerequisites}</h3>
                <div class="linklist">
                    {prerequisite_links(derived.mandatory.clone(), words.mandatory)}
                    {prerequisite_links(derived.recommended.clone(), words.recommended)}
                </div>
                {m.prerequisites_mandatory.clone().map(|text| view! { <details class="more"><summary>{words.mandatory_verbatim}</summary><Prose text lang=lang()/></details> })}
                {m.prerequisites_recommended.clone().map(|text| view! { <details class="more"><summary>{words.recommended_verbatim}</summary><Prose text lang=lang()/></details> })}
            </div>
        })}
        {m.contents.clone().map(|text| view! { <div class="section" id="inhalte"><h3 class="label">{words.contents}</h3><Prose text lang=lang()/></div> })}
        {m.learning_outcomes.clone().map(|text| view! { <div class="section" id="lernziele"><h3 class="label">{words.learning_outcomes}</h3><Prose text lang=lang()/></div> })}
        {m.exam_details.clone().map(|text| view! { <div class="section" id="pruefungsleistung"><h3 class="label">{words.assessment}</h3><Prose text lang=lang()/></div> })}
        {(!literature.is_empty()).then(|| view! {
            <div class="section" id="literatur">
                <details class="more"><summary>{(words.literature_count)(literature.len())}</summary>
                    <ul class="list-plain">{literature.into_iter().map(|l| view! { <li>{l}</li> }).collect_view()}</ul>
                </details>
            </div>
        })}
        {m.remarks.clone().map(|text| view! { <div class="section" id="bemerkungen"><h3 class="label">{words.remarks}</h3><Prose text lang=lang()/></div> })}
        <Programs data=data.clone()/>
    }
}

#[component]
fn DataClient(data: ModuleData) -> impl IntoView {
    let t = i18n::t();
    let m = data.module;
    view! {
        <p class="source">
            <Icon name="shield-check"/>
            {t.module.source}
            {m.fetched_at.as_deref().map(|at| (t.module.fetched)(&format::date(at, t.locale)))}
            {m.source_url.clone().map(|href| view! { <a href=href rel="noopener">{t.module.original}<Icon name="arrow-up-right"/></a> })}
        </p>
    }
}

/// Teaching events of the newest semester that has any, as a week grid plus a list; exams below.
#[component]
fn Schedule(data: ModuleData) -> impl IntoView {
    let t = i18n::t();
    let words = &t.module;
    let m = &data.module;
    // The semesters as the page's language names them, not as the snapshot does.
    let newest = data.schedule.first().map(|d| (d.semester_key.clone(), semester_name(&d.semester_key, &d.semester_label, t.locale)));
    let current = data.semesters.iter().find(|s| s.is_current);
    let upcoming = data.semesters.iter().find(|s| current.is_some_and(|c| s.key > c.key));

    // The gap before the BTU publishes the next semester (owner decision Q8).
    let gap_note = match (&newest, upcoming) {
        (Some((key, label)), Some(next)) if *key < next.key => Some((words.dates_from)(label, &semester_name(&next.key, &next.label, t.locale))),
        _ => None,
    };
    let no_schedule_note = newest.is_none().then(|| match m.turnus_season.as_ref().and_then(|s| s.known()) {
        Some(TurnusSeason::Winter) => words.no_dates_winter,
        Some(TurnusSeason::Summer) => words.no_dates_summer,
        Some(TurnusSeason::Both) => words.no_dates_every,
        _ => words.no_dates,
    });

    let newest_key = newest.as_ref().map(|(key, _)| key.clone());
    let teaching: Vec<EventDate> = data.schedule.iter().filter(|d| Some(&d.semester_key) == newest_key.as_ref()).cloned().collect();
    let exam_semester = data.exams.first().map(|d| (d.semester_key.clone(), semester_name(&d.semester_key, &d.semester_label, t.locale)));
    let exams: Vec<EventDate> = data.exams.iter().filter(|d| exam_semester.as_ref().is_some_and(|(key, _)| *key == d.semester_key)).cloned().collect();

    // The Studienplan beside the week (A.9): nothing on the server, so its HTML is the module's
    // alone; in the app the other modules planned into the semester of these Termine.
    let overlay = plan_overlay(&m.id, newest_key.as_deref().and_then(SemesterKey::parse), current.and_then(|c| SemesterKey::parse(&c.key)));
    let own = own_groups(&teaching, t);
    // Under a week the list is folded (owner, 2026-10-17: „die Termine können ruhig ausklappbar
    // sein, so dass nicht so viel vertical space verschwendet wird"): a slot unfolds it and marks
    // its lines (enhance.js „termin"). Without a week the list is all there is, and stays open.
    let folded = !own.is_empty();
    let slots = Memo::new(move |_| overlay.with(|overlay| with_overlay(&own, overlay, t)));
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

    // An exam date is shown as read (`folia_timetable::exam_reading`): the BTU's placeholder is no time,
    // a deadline reads „bis 24:00", and the original stays in the row.
    let exam_semester_row = exam_semester.as_ref().and_then(|(key, _)| data.semesters.iter().find(|s| s.key == *key));
    let exams: Vec<(EventDate, Option<ExamReading>)> = exams
        .into_iter()
        .map(|d| {
            let reading = exam_reading::read(&d, exam_semester_row);
            (d, Some(reading))
        })
        .collect();
    let exam_note = exam_semester.as_ref().and_then(|(_, label)| exam_note(exams.iter().filter_map(|(_, r)| r.as_ref()), label, t));

    // A line per Termin (`termine`, `termin_text`), with the id its slots lead to; an exam date as
    // read (`exam_reading`).
    let event_list = |dates: Vec<(Option<String>, Vec<EventDate>, Option<ExamReading>)>| {
        dates
            .into_iter()
            .filter_map(|(id, rows, reading)| {
                let d = rows.first()?.clone();
                let mut text = termin_text(&rows, t)?;
                let (mut open, mut stated, mut marker) = (words.time_open, None, None);
                if let Some(r) = reading.as_ref() {
                    let slot = &r.shown;
                    text.when = shown_when(slot, t);
                    text.dates = match &slot.first_date {
                        Some(first) => Dates::Range(first.clone(), slot.last_date.clone().filter(|last| last != first)),
                        None => Dates::None,
                    };
                    if r.has(Reason::PlaceholderTime) && slot.first_date.is_none() {
                        open = words.date_open;
                    }
                    // What the BTU wrote where the row shows something else without marking it (a deadline).
                    stated = (r.shown != r.stated && !r.is_marked()).then(|| (words.in_qis)(&stated_slot(r, t)));
                    marker = r.is_marked().then(|| marker_text(r, t));
                }
                let details = !text.head.is_empty() || text.dates != Dates::None || !text.tail.is_empty();
                // The short form stands in the line; QIS's whole name is the tooltip (owner, 2026-09-25).
                let room_long = joined(rows.iter().filter(|row| row.room_short.is_some()).map(|row| row.room.as_deref()), " / ");
                let TerminText { when, head, dates, tail, listed } = text;
                let body = view! {
                    <span class="when" title=stated>{when.unwrap_or_else(|| open.to_string())}</span>
                    <b>{d.event_title.clone()}</b>
                    {(details || marker.is_none()).then(|| view! { <small title=room_long>{detail_line(head, dates, tail, t.locale)}</small> })}
                    {marker.map(|text| view! { <small class="odd"><Icon name="info"/><span>{text}</span></small> })}
                };
                Some(match d.source_url.clone() {
                    Some(href) => view! { <a class="ev" id=id href=href rel="noopener" title=listed>{body}</a> }.into_any(),
                    None => view! { <div class="ev" id=id title=listed>{body}</div> }.into_any(),
                })
            })
            .collect_view()
    };

    view! {
        <div class="section" id="termine">
            <h3 class="label">{words.dates}{newest.as_ref().map(|(_, label)| view! { <span>{label.clone()}</span> })}</h3>
            <WeekGrid slots/>
            {week_line}
            {gap_note.map(|note| view! { <p class="note"><Icon name="info"/><span>{note}</span></p> })}
            {no_schedule_note.map(|note| view! { <p class="hint">{note}</p> })}
            {(!teaching.is_empty()).then(|| {
                let lines: Vec<(Option<String>, Vec<EventDate>, Option<ExamReading>)> =
                    termine(&teaching).into_iter().enumerate().map(|(n, rows)| (Some(termin_id(n)), rows.into_iter().cloned().collect(), None)).collect();
                let count = lines.len();
                let list = view! { <div class="evlist">{event_list(lines)}</div> };
                match folded {
                    true => view! {
                        <details class="evfold">
                            <summary data-action="fold">{words.dates_list}<span class="num">{format!("({count})")}</span></summary>
                            {list}
                        </details>
                    }
                    .into_any(),
                    false => list.into_any(),
                }
            })}
        </div>
        // The exam dates stay open (owner, 2026-10-17: „Prüfungen einfach ausgeklappt lassen").
        {exam_semester.map(|(_, label)| view! {
            <div class="section" id="pruefungstermine">
                <h3 class="label">{words.exam_dates}<span>{label}</span></h3>
                <div class="evlist">{event_list(exams.into_iter().map(|(d, reading)| (None, vec![d], reading)).collect())}</div>
                {exam_line}
                {exam_note.map(|note| view! { <p class="note"><Icon name="info"/><span>{note}</span></p> })}
            </div>
        })}
    }
}

/// What makes rows one line of the Termine list (`termine`).
#[derive(Clone, Copy, PartialEq)]
enum TerminKey<'a> {
    /// The dates of a series of single dates at one time: event, series, start, end.
    Series(&'a str, Series, Option<&'a str>, Option<&'a str>),
    /// The single dates of one slot in no series, the same date in several rooms among them:
    /// event, group, weekday, start, end, and whether QIS gives a date.
    Once(&'a str, Option<&'a str>, Option<i64>, Option<&'a str>, Option<&'a str>, bool),
    /// The recurring rows of one slot: event, group, weekday, start, end, rhythm, and whether QIS
    /// gives them a range (a row without one is a line of its own, as before). A Termin in several
    /// rooms, and one QIS splits into several ranges.
    Slot(&'a str, Option<&'a str>, Option<i64>, Option<&'a str>, Option<&'a str>, Option<&'a str>, bool),
}

/// The Termine of a module's teaching rows, each the rows of one line of the list (owner,
/// 2026-10-13/14: „sehr unübersichtlich, wenn da Einzeltermine sind", „bündel auch die geteilten
/// Zeiträume"): the dates of a series of single dates at one time, one Termin in the series'
/// rhythm; the other single dates of one slot (event, group, weekday and time), the same date in
/// several rooms among them; the recurring rows of one slot and rhythm, a Termin in several rooms
/// or in several ranges. A date of a series at another time is a line of its own, and so are the
/// single dates of a slot beside its weekly rows, as in the week. Each line stands where its first
/// row does.
fn termine<'a>(teaching: impl IntoIterator<Item = &'a EventDate>) -> Vec<Vec<&'a EventDate>> {
    let mut lines: Vec<(TerminKey<'a>, Vec<&'a EventDate>)> = Vec::new();
    for date in teaching {
        let key = termin_key(date);
        match lines.iter_mut().find(|(known, _)| *known == key) {
            Some((_, rows)) => rows.push(date),
            None => lines.push((key, vec![date])),
        }
    }
    lines.into_iter().map(|(_, rows)| rows).collect()
}

/// The id of the `n`th line of the Termine list (from 0), which its slots of the week lead to:
/// „termin-1". The same in every language, as the sections' anchors are.
fn termin_id(n: usize) -> String {
    format!("termin-{}", n + 1)
}

/// Per row, the line of the Termine list it stands on (`termine`, which makes the lines in the
/// order their first rows come).
fn line_of_rows(teaching: &[EventDate]) -> Vec<usize> {
    let mut keys: Vec<TerminKey<'_>> = Vec::new();
    teaching
        .iter()
        .map(|date| {
            let key = termin_key(date);
            keys.iter().position(|known| *known == key).unwrap_or_else(|| {
                keys.push(key);
                keys.len() - 1
            })
        })
        .collect()
}

/// The line of the Termine list a row belongs to (`termine`).
fn termin_key(date: &EventDate) -> TerminKey<'_> {
    let (event, from, to, dated) = (date.event_id.as_str(), date.start_time.as_deref(), date.end_time.as_deref(), span_of(date).is_some());
    match (date.series, once(date)) {
        (Some(series), _) => TerminKey::Series(event, series, from, to),
        (None, true) => TerminKey::Once(event, date.group_name.as_deref(), date.weekday, from, to, dated),
        (None, false) => TerminKey::Slot(event, date.group_name.as_deref(), date.weekday, from, to, date.rhythm.as_ref().map(|rhythm| rhythm.code()), dated),
    }
}

/// A row's days, its first to its last, one day for a single date; `None` for a row without a date.
fn span_of(row: &EventDate) -> Option<(Day, Day)> {
    let first = row.first_date.as_deref().and_then(Day::parse)?;
    let last = row.last_date.as_deref().and_then(Day::parse).filter(|last| *last >= first).unwrap_or(first);
    Some((first, last))
}

/// „06.10." for a day, „09.10.–23.10." for a range.
fn span_text((first, last): (Day, Day), locale: Locale) -> String {
    match first == last {
        true => first.day_month(locale),
        false => format!("{}–{}", first.day_month(locale), last.day_month(locale)),
    }
}

/// A date that happens once: a single date, or a range of one day.
fn once(date: &EventDate) -> bool {
    date.rhythm.as_ref().is_some_and(|rhythm| rhythm.is(Rhythm::Single)) || (date.first_date.is_some() && date.first_date == date.last_date)
}

/// A line of the Termine list in words: when, and the line under it.
#[derive(Debug, PartialEq)]
struct TerminText {
    /// Weekday and time, from the earliest start of the rows to their latest end.
    when: Option<String>,
    /// Type, group, rhythm and how many: „Seminar · 1-Gruppe · wöchentlich · 6 Termine".
    head: Vec<String>,
    dates: Dates,
    /// Rooms, persons and comments (`said_on`).
    tail: Vec<String>,
    /// The tooltip of a series: its dates as QIS lists them.
    listed: Option<String>,
}

/// The days of a line: none, a range (its first and its last day, ISO), or its days and ranges one
/// by one (`span_of`).
#[derive(Debug, PartialEq)]
enum Dates {
    None,
    Range(String, Option<String>),
    List(Vec<(Day, Day)>),
}

/// The words of one line of the Termine list (`termine`): a series of single dates in its rhythm,
/// how many and from when to when, its dates in the tooltip; the single dates of one slot counted
/// and named day by day („6 Einzeltermine · 06.10., 20.10., …"); a recurring Termin in its ranges
/// („wöchentlich · 09.10.–23.10., 06.11.–27.11."), with all its rooms. QIS's „[unbenannt]" names
/// no group, so the line names none (owner, 2026-10-14: it read as a person QIS does not know).
fn termin_text(rows: &[EventDate], t: &'static Texts) -> Option<TerminText> {
    let d = rows.first()?;
    let days: Vec<Day> = rows.iter().filter_map(|row| row.first_date.as_deref().and_then(Day::parse)).collect::<BTreeSet<_>>().into_iter().collect();
    let spans: Vec<(Day, Day)> = rows.iter().filter_map(span_of).collect::<BTreeSet<_>>().into_iter().collect();
    let several = days.len() > 1;
    let series = d.series.filter(|_| several);
    let singles = several && series.is_none() && once(d);
    let feed = folia_timetable::i18n::texts(t.locale);
    let rhythm = match series {
        Some(series) => Some(feed.series_rhythm(series.weeks).to_string()),
        None if singles => None,
        None => d.rhythm.as_ref().map(|r| r.label(t.locale).to_string()).or(d.rhythm_raw.clone()),
    };
    let count = match series {
        Some(_) => Some((t.module.dates_count)(days.len())),
        None => singles.then(|| (t.module.single_dates)(days.len())),
    };
    let group = d.group_name.as_deref().map(str::trim).filter(|group| !group.is_empty() && !group.starts_with('[')).map(str::to_string);
    let head = [d.event_type.clone(), group, rhythm, count].into_iter().flatten().collect();
    let dates = match (singles || (series.is_none() && spans.len() > 1), rows.iter().filter_map(|row| row.first_date.as_deref()).min()) {
        (true, _) => Dates::List(spans),
        (false, Some(first)) => {
            let last = rows.iter().filter_map(|row| row.last_date.as_deref()).max().filter(|last| *last != first);
            Dates::Range(first.to_string(), last.map(str::to_string))
        }
        (false, None) => Dates::None,
    };
    let tail = [
        said_on(rows, EventDate::room_shown, " / ", t),
        said_on(rows, |row| row.instructor.as_deref(), "; ", t),
        said_on(rows, |row| row.comment.as_deref(), "; ", t),
    ]
    .into_iter()
    .flatten()
    .collect();
    let listed = series.map(|_| format!("{}: {}", feed.feed_singles, days.iter().map(|day| day.date(t.locale)).collect::<Vec<_>>().join(", ")));
    let from = rows.iter().filter_map(|row| row.start_time.as_deref()).min();
    let to = rows.iter().filter_map(|row| row.end_time.as_deref()).max();
    Some(TerminText { when: format::time_slot(d.weekday, from, to, t.locale), head, dates, tail, listed })
}

/// What the rows of a line say (`value`), each once in their order and joined by `by`; a value
/// not every date or range of the line has names them, or, where most have it, those it leaves out
/// („ZB/VI.03 (16.10., 27.11.)", „SFB/1.308 (außer 26.01.) / SFB/1.309 (26.01.)", „SFB/1.211
/// (09.10.–23.10.)"), so that the line says of no date what QIS says of another. `None` where none
/// says anything.
fn said_on<'a>(rows: &'a [EventDate], value: impl Fn(&'a EventDate) -> Option<&'a str>, by: &str, t: &'static Texts) -> Option<String> {
    let all: BTreeSet<(Day, Day)> = rows.iter().filter_map(span_of).collect();
    let mut values: Vec<(&str, BTreeSet<(Day, Day)>)> = Vec::new();
    for row in rows {
        let Some(said) = value(row).map(str::trim).filter(|said| !said.is_empty()) else { continue };
        match values.iter_mut().find(|(known, _)| *known == said) {
            Some((_, spans)) => spans.extend(span_of(row)),
            None => values.push((said, span_of(row).into_iter().collect())),
        }
    }
    let named = |spans: &mut dyn Iterator<Item = &(Day, Day)>| spans.map(|span| span_text(*span, t.locale)).collect::<Vec<_>>().join(", ");
    let parts: Vec<String> = values
        .into_iter()
        .map(|(said, days)| match days == all || days.is_empty() {
            true => said.to_string(),
            false if 2 * days.len() > all.len() => format!("{said} ({})", (t.module.except_on)(&named(&mut all.difference(&days)))),
            false => format!("{said} ({})", named(&mut days.iter())),
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(by))
}

/// What rows say, each once in their order and joined by `by` („HG 0.20 / HG 0.18"); `None` where
/// none says anything.
fn joined<'a>(values: impl Iterator<Item = Option<&'a str>>, by: &str) -> Option<String> {
    let mut seen: Vec<&str> = Vec::new();
    for value in values.flatten().map(str::trim).filter(|value| !value.is_empty()) {
        if !seen.contains(&value) {
            seen.push(value);
        }
    }
    (!seen.is_empty()).then(|| seen.join(by))
}

/// The week grid's slots of a module's teaching rows (one semester's): every row with a weekday
/// and both times, 24:00 included. Rows of one event and group at the same weekday and times are
/// one slot (the same time in two rooms, or a date range QIS splits in two). A date that happens
/// once (a single date, or a range of one day) is not drawn as a weekly slot: the single dates of
/// such a key are one `.once` slot that counts them („3 Termine", „1 Termin · 23.02."). The dates
/// of a series of single dates at one time (`EventDate::series`) are the one Termin they are, a
/// slot like a weekly one whose tooltip names their rhythm and days; a date of it alone at its
/// time is a single date.
#[cfg(test)]
fn own_slots(teaching: &[EventDate], t: &'static Texts) -> Vec<GridSlot> {
    own_groups(teaching, t).into_iter().map(|(slot, _)| slot).collect()
}

/// `own_slots`, each with the keys of the rows it stands for (`RowKey`): what the Studienplan
/// names when one of them clashes with it. Each slot leads to the lines of the Termine list its rows
/// stand on (`GridSlot::lines`, `termin_id`).
fn own_groups(teaching: &[EventDate], t: &'static Texts) -> Vec<(GridSlot, Vec<RowKey>)> {
    /// Once, the series, weekday, from, to, event, group.
    type Key<'a> = (bool, Option<Series>, u8, u16, u16, &'a str, Option<&'a str>);
    /// A slot's key, its first row, the days of its single dates, the keys of its rows, and the
    /// lines of the list they stand on.
    type Group<'a> = (Key<'a>, &'a EventDate, BTreeSet<Option<&'a str>>, Vec<RowKey>, BTreeSet<usize>);
    let lines = line_of_rows(teaching);
    let mut groups: Vec<Group<'_>> = Vec::new();
    for (date, line) in teaching.iter().zip(lines) {
        let day = date.weekday.and_then(|day| u8::try_from(day).ok()).filter(|day| (1..=7).contains(day));
        let from = date.start_time.as_deref().and_then(minutes);
        let to = date.end_time.as_deref().and_then(minutes);
        let (Some(day), Some(from), Some(to)) = (day, from, to) else { continue };
        if to <= from {
            continue;
        }
        let once = date.series.is_none() && once(date);
        let key = (once, date.series, day, from, to, date.event_id.as_str(), date.group_name.as_deref());
        let row = RowKey::of(date);
        match groups.iter_mut().find(|(known, ..)| *known == key) {
            Some((_, _, dates, rows, on)) => {
                dates.insert(date.first_date.as_deref());
                rows.extend(row);
                on.insert(line);
            }
            None => groups.push((key, date, BTreeSet::from([date.first_date.as_deref()]), row.into_iter().collect(), BTreeSet::from([line]))),
        }
    }
    groups
        .into_iter()
        .map(|((once, series, day, from, to, ..), date, dates, rows, on)| {
            // A date of a series alone at its time is the single date it is.
            let once = once || (series.is_some() && dates.len() == 1);
            let slot = own_slot(once, series.filter(|_| !once), day, from, to, date, &dates, t);
            (GridSlot { lines: on.into_iter().map(termin_id).collect(), ..slot }, rows)
        })
        .collect()
}

/// The slot of one group of rows (`own_groups`): `date` is its first row, `dates` the days of its
/// single dates, or of the series of single dates (`series`) it is.
#[allow(clippy::too_many_arguments)]
fn own_slot(once: bool, series: Option<Series>, day: u8, from: u16, to: u16, date: &EventDate, dates: &BTreeSet<Option<&str>>, t: &'static Texts) -> GridSlot {
    // The kinds in their few letters („VL", „Prak", „VL/Ü"), as in the Studienplan's week: what
    // QIS calls the event fits a narrow day only in part, and one grid said „Vorlesung", „VL" and
    // „Laborausbi…" side by side (review 2026-09-25). QIS's word leads the tooltip, and stays the
    // label of a type no kind is known of. The module's name is the page's own and stands in no
    // slot.
    let word = date.event_type.as_deref().map(str::trim).filter(|word| !word.is_empty());
    let kinds = kinds_of(word);
    let label = match kinds == KindSet::default().with(EventKind::Other) {
        true => word.unwrap_or(t.module.slot_date).to_string(),
        false => kinds.iter().map(|kind| kind.short(t.locale)).collect::<Vec<_>>().join("/"),
    };
    let what = match word {
        Some(word) => format!("{word} · {}", date.event_title),
        None => date.event_title.clone(),
    };
    let days: Vec<Day> = dates.iter().filter_map(|date| date.and_then(Day::parse)).collect();
    let listed = || days.iter().map(|day| day.date(t.locale)).collect::<Vec<_>>().join(", ");
    if !once {
        let lecture = class_of(kinds) == Class::Lecture;
        // A series names its rhythm and the days it stands for.
        let title = match series {
            Some(series) => format!("{what} · {} · {}", folia_timetable::i18n::texts(t.locale).series_rhythm(series.weeks), listed()),
            None => what,
        };
        return GridSlot { day, from, to, label, small: clock(from), title, class: if lecture { "" } else { "other" }, ..GridSlot::default() };
    }
    let small = match (dates.len(), days.as_slice()) {
        (1, [only]) => (t.module.one_date_on)(&only.day_month(t.locale)),
        (n, _) => (t.module.dates_count)(n),
    };
    // The tooltip names the dates the small line only counts.
    let title = if days.is_empty() { what } else { format!("{what} · {}", listed()) };
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
    let source = use_data().ok();
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
        source.now(&OverlayAsk { key, others, program, module_id: id.clone(), selection, locale: crate::i18n::locale() }).unwrap_or_default()
    })
}

/// The module's own slots with the plan beside them (A.9): its slots in a hard clash marked, and
/// the other planned modules' Termine as quiet `.planned` slots cut to the days and hours the
/// module's own week spans, so that the grid never grows when the plan arrives. Planned Termine
/// at the same time are one slot that names each module once, by its abbreviation
/// (`StudyplanData::slot_names`, done in `pages::overlay`): the plan is the context here, and where it
/// meets the module's own slots it takes a slim lane (`crate::week`). The names are the slot's
/// label, not its small line, which phones hide: an outline without a name says nothing.
fn with_overlay(own: &[(GridSlot, Vec<RowKey>)], overlay: &Overlay, t: &'static Texts) -> Vec<GridSlot> {
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
        let when = format::time_slot(Some(i64::from(planned.day)), Some(&clock(planned.from)), Some(&clock(planned.to)), t.locale).unwrap_or_default();
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
fn shown_when(slot: &Slot, t: &'static Texts) -> Option<String> {
    match (&slot.start_time, &slot.end_time) {
        (None, Some(end)) => Some(match format::time_slot(slot.weekday, None, None, t.locale) {
            Some(day) => (t.module.day_until)(&day, end),
            None => (t.module.until)(end),
        }),
        (start, end) => format::time_slot(slot.weekday, start.as_deref(), end.as_deref(), t.locale),
    }
}

/// The line under a Termin: type, group and rhythm, its days, then room, lecturers and comment,
/// joined by „ · ". The days are `<time>` elements: „15.02.2027" to a reader (in the page's
/// language), 2027-02-15 to a machine (a search engine reading the page); single dates and ranges
/// one by one read „06.10., 20.10." and „09.10.–23.10., 06.11.–27.11."
fn detail_line(head: Vec<String>, dates: Dates, tail: Vec<String>, locale: Locale) -> impl IntoView {
    let mut lead = head.join(" · ");
    let mut rest = tail.join(" · ");
    let dated = dates != Dates::None;
    if !lead.is_empty() && (dated || !rest.is_empty()) {
        lead.push_str(" · ");
    }
    if dated && !rest.is_empty() {
        rest.insert_str(0, " · ");
    }
    let days = match dates {
        Dates::None => None,
        Dates::Range(first, last) => Some(view! { {date_of(&first, locale)}{last.map(|last| view! { " – "{date_of(&last, locale)} })} }.into_any()),
        Dates::List(spans) => Some(
            spans
                .into_iter()
                .enumerate()
                .map(|(n, (first, last))| {
                    let till = (last != first).then(|| view! { "–"<time datetime=last.iso()>{last.day_month(locale)}</time> });
                    view! { {(n > 0).then_some(", ")}<time datetime=first.iso()>{first.day_month(locale)}</time>{till} }
                })
                .collect_view()
                .into_any(),
        ),
    };
    // An empty text would still be a text node, which the server writes as a space.
    view! {
        {(!lead.is_empty()).then_some(lead)}
        {days}
        {(!rest.is_empty()).then_some(rest)}
    }
}

/// A day as the page writes it, „15.02.2027", "15 Feb 2027", with its date for machines;
/// anything that is no `YYYY-MM-DD` as it is.
fn date_of(iso: &str, locale: Locale) -> AnyView {
    match Day::parse(iso) {
        Some(day) => view! { <time datetime=day.iso()>{day.date(locale)}</time> }.into_any(),
        None => format::date(iso, locale).into_any(),
    }
}

/// „27.12.2015", „08.02.2027 – 19.02.2027"
fn date_range(slot: &Slot, locale: Locale) -> Option<String> {
    match (&slot.first_date, &slot.last_date) {
        (Some(first), Some(last)) if first != last => Some(format!("{} – {}", format::date(first, locale), format::date(last, locale))),
        (Some(first), _) => Some(format::date(first, locale)),
        _ => None,
    }
}

/// What the BTU wrote, in the words of the row: „So 01:00–02:30 · 27.12.2015". The dates only
/// where the row does not show them already.
fn stated_slot(reading: &ExamReading, t: &'static Texts) -> String {
    let (stated, shown) = (&reading.stated, &reading.shown);
    let dates = (date_range(stated, t.locale) != date_range(shown, t.locale)).then(|| date_range(stated, t.locale)).flatten();
    [format::time_slot(stated.weekday, stated.start_time.as_deref(), stated.end_time.as_deref(), t.locale), dates].into_iter().flatten().collect::<Vec<_>>().join(" · ")
}

/// The line under a marked exam date: the original where the row shows something else, and what
/// is odd about what it shows as stated.
fn marker_text(reading: &ExamReading, t: &'static Texts) -> String {
    let words = &t.module;
    let odd: Vec<&str> = [(Reason::UnusualTime, words.unusual_time), (Reason::EndsBeforeStart, words.ends_before_start), (Reason::DateOutsideSemester, words.date_outside_semester)]
        .into_iter()
        .filter(|(reason, _)| reading.has(*reason))
        .map(|(_, text)| text)
        .collect();
    match (reading.shown != reading.stated, odd.is_empty()) {
        (true, true) => (words.in_qis)(&stated_slot(reading, t)),
        (true, false) => (words.in_qis_odd)(&stated_slot(reading, t), &odd.join(" · ")),
        (false, _) => (words.as_in_qis)(&odd.join(" · ")),
    }
}

/// The note under the exam dates when any is marked: what the marks mean, once per section.
/// `semester` is the semester of the exam dates, as the page names it.
fn exam_note<'a>(readings: impl Iterator<Item = &'a ExamReading> + Clone, semester: &str, t: &'static Texts) -> Option<String> {
    let words = &t.module;
    let any = |reason: Reason| readings.clone().any(|r| r.has(reason));
    let mut parts: Vec<String> = Vec::new();
    if any(Reason::PlaceholderTime) {
        parts.push(if any(Reason::PlaceholderDate) { (words.placeholder_date_note)(semester) } else { words.placeholder_time_note.to_string() });
    }
    let (from, to) = exam_reading::DAY;
    let hours = format!("{:02}:{:02}–{:02}:{:02}", from / 60, from % 60, to / 60, to % 60);
    let odd: Vec<String> = [
        (Reason::UnusualTime, (words.odd_time)(&hours)),
        (Reason::EndsBeforeStart, words.odd_end.to_string()),
        (Reason::DateOutsideSemester, (words.odd_date)(semester)),
    ]
    .into_iter()
    .filter(|(reason, _)| any(*reason))
    .map(|(_, text)| text)
    .collect();
    if let Some((first, rest)) = odd.split_first() {
        let mut list = first.clone();
        for (i, item) in rest.iter().enumerate() {
            list.push_str(if i + 1 == rest.len() { words.odd_or } else { ", " });
            list.push_str(item);
        }
        let mut letters = list.chars();
        let list: String = letters.next().map(|first| first.to_uppercase().chain(letters).collect()).unwrap_or_default();
        parts.push((words.odd_note)(&list));
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// How many curricula „Studiengänge" shows before the rest runs out under „Alle anzeigen" (owner,
/// 2026-10-17: „zeige da maximal 5 mit Auslauffunktion").
const PROGRAMS_SHOWN: usize = 5;

/// The programs the module belongs to: curricula first, then how many accept it as FÜS. Five at
/// most, the fifth running out under a button that shows the rest (`.runout`, app.css, and
/// enhance.js „runout"); without JavaScript all of them.
#[component]
fn Programs(data: ModuleData) -> impl IntoView {
    let t = i18n::t();
    let words = &t.module;
    let resolved: Vec<_> = data.programs.iter().filter(|l| l.resolve_status.is(ResolveStatus::Resolved)).cloned().collect();
    // Each with the semester its validated plan places the module in, where it has one.
    let curricular: Vec<(ProgramLink, Option<String>)> = curricula(&data).into_iter().map(|link| {
        let semesters = plan_semesters(&data, &link, t.locale);
        (link, semesters)
    }).collect();
    let fues = resolved.iter().filter(|l| l.relation.as_ref().is_some_and(|r| r.is(Relation::Fues))).count();
    let unresolved = data.programs.iter().filter(|l| l.resolve_status.is(ResolveStatus::Unresolved)).count();
    let count = curricular.len();
    let cut = count > PROGRAMS_SHOWN;

    (!data.programs.is_empty()).then(|| view! {
        <div class="section" id="studiengaenge">
            <h3 class="label">{words.programs}<span>{(words.curricula)(count)}</span></h3>
            {curricular.is_empty().then(|| view! { <p class="hint">{words.no_curriculum}</p> })}
            <div class="linklist programs" class:runout=cut id="studiengaenge-liste">
                {curricular.into_iter().map(|(link, semesters)| {
                    let slug = link.program_slug.clone().unwrap_or_default();
                    view! {
                        <a class="pre" href=t.path(&url::program_path(&slug, ProgramTab::Plan))>
                            <b>
                                {link.program_name.clone().unwrap_or_default()}" · "
                                {link.degree_display.clone().or(link.degree_raw.clone()).unwrap_or_default()}
                                <span class="subtitle">"PO "{link.po_version.clone().unwrap_or_default()}{semesters.map(|semesters| format!(" · {semesters}"))}{link.area.clone().map(|area| format!(" · {area}"))}</span>
                            </b>
                            <small><KindBadge kind=link.kind.clone() basis=link.kind_basis.clone() source=link.kind_source.clone()/></small>
                            <Icon name="chevron-right"/>
                        </a>
                    }
                }).collect_view()}
            </div>
            {cut.then(|| view! {
                <button type="button" class="runout-more js-only" data-action="runout" aria-controls="studiengaenge-liste" aria-expanded="false">
                    {(words.programs_all)(count)}<Icon name="chevron-down"/>
                </button>
            })}
            {(fues > 0).then(|| view! { <p class="hint">{(words.fues)(fues)}</p> })}
            {(unresolved > 0).then(|| view! { <p class="hint">{(words.unresolved)(unresolved)}</p> })}
        </div>
    })
}

#[cfg(test)]
mod tests {
    use crate::i18n::{DE, EN};
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
            series: None,
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
            rhythm: Some(folia_model::labels::Code::parse(rhythm)),
            last_date: Some(last.into()),
            ..exam(Some(weekday), Some(times.0), Some(times.1), Some(first))
        }
    }

    /// A series of single dates is one Termin on the page (owner, 2026-10-13: „sehr
    /// unübersichtlich, wenn da Einzeltermine sind"): one slot in the week whose tooltip names its
    /// rhythm and days, one line in the list, and one schema.org `Schedule` in its rhythm with the
    /// day it leaves out. A date of it at another time is a single date.
    #[test]
    fn a_series_of_single_dates_is_one_termin() {
        let date = |on: &str, end: &str| EventDate {
            series: Some(Series { first: 1, weeks: 1 }),
            ..teaching("150001", "Seminar", "[unbenannt]", 2, ("13:45", end), "single", on, on)
        };
        let rows = [date("2026-10-13", "15:15"), date("2026-10-20", "15:15"), date("2026-11-03", "15:15"), date("2026-11-24", "16:00")];
        let slots = own_slots(&rows, &DE);
        let shown: Vec<(&str, &str, &str)> = slots.iter().map(|s| (s.small.as_str(), s.class, s.title.as_str())).collect();
        let abwl = "Allgemeine Betriebswirtschaftslehre II";
        assert_eq!(
            shown,
            [
                ("13:45", "other", format!("Seminar · {abwl} · wöchentlich · 13.10.2026, 20.10.2026, 03.11.2026").as_str()),
                ("1 Termin · 24.11.", "once", format!("Seminar · {abwl} · 24.11.2026").as_str()),
            ]
        );
        assert_eq!(termine(&rows).iter().map(Vec::len).collect::<Vec<_>>(), [3, 1]);
        // What not every date of the line says names its days, or those it leaves out where most
        // have it; what all say does not.
        let mut said: Vec<EventDate> = rows.iter().take(3).cloned().collect();
        for (row, (room, comment)) in said.iter_mut().zip([("ZB/VI.03", None), ("SFB/1.309", Some("online")), ("ZB/VI.03", None)]) {
            (row.room, row.comment, row.instructor) = (Some(room.into()), comment.map(Into::into), Some("Dippner".into()));
        }
        assert_eq!(said_on(&said, EventDate::room_shown, " / ", &DE).as_deref(), Some("ZB/VI.03 (außer 20.10.) / SFB/1.309 (20.10.)"));
        assert_eq!(said_on(&said, |row| row.comment.as_deref(), "; ", &DE).as_deref(), Some("online (20.10.)"));
        assert_eq!(said_on(&said, |row| row.instructor.as_deref(), "; ", &DE).as_deref(), Some("Dippner"));
        assert_eq!(said_on(&said, EventDate::room_shown, " / ", &EN).as_deref(), Some("ZB/VI.03 (except 20 Oct) / SFB/1.309 (20 Oct)"));
        let refs: Vec<&EventDate> = rows.iter().collect();
        let schedules = schedule_of(&refs);
        let field = |n: usize, key: &str| schedules.get(n).and_then(|schedule| schedule.get(key)).cloned();
        assert_eq!(schedules.len(), 2);
        assert_eq!(
            (field(0, "startDate"), field(0, "endDate"), field(0, "repeatFrequency"), field(0, "exceptDate")),
            (Some("2026-10-13".into()), Some("2026-11-03".into()), Some("P1W".into()), Some(serde_json::json!(["2026-10-27"])))
        );
        assert_eq!((field(1, "startDate"), field(1, "endDate"), field(1, "repeatFrequency")), (Some("2026-11-24".into()), Some("2026-11-24".into()), None));
    }

    /// The single dates of one slot are one line with their days, the same date in several rooms
    /// one with all of them, a weekly Termin in two rooms one with both, and one QIS splits into
    /// ranges one with its ranges (owner, 2026-10-14: „mach auch die zwei anderen Bündelungen",
    /// „bündel auch die geteilten Zeiträume"); QIS's „[unbenannt]" names no group. The structured
    /// data keeps a Schedule per date.
    #[test]
    fn single_dates_of_a_slot_and_a_termin_in_several_rooms_are_one_line() {
        let at = |room: &str, date: EventDate| EventDate { room: Some(room.into()), ..date };
        let single = |event: &str, group: &str, on: &str| teaching(event, "Seminar", group, 2, ("14:30", "16:00"), "single", on, on);
        let mut rows: Vec<EventDate> = ["2026-10-06", "2026-10-20", "2026-11-10", "2026-11-24", "2026-12-15"]
            .into_iter()
            .map(|on| at("SFB/1.308", single("152130", "1-Gruppe", on)))
            .collect();
        rows.push(at("SFB/1.309", single("152130", "1-Gruppe", "2027-01-12")));
        rows.extend(["SFB/6.302", "SFB/6.106", "SFB/6.306"].into_iter().map(|room| at(room, single("152463", "[unbenannt]", "2027-01-19"))));
        let weekly = teaching("152512", "Übung", "1-Gruppe", 5, ("12:30", "14:00"), "weekly", "2026-10-09", "2026-10-23");
        rows.extend([at("SFB/1.211", weekly.clone()), at("SFB/1.210", weekly)]);
        // The same Termin in two ranges, one room only in the first.
        let weekly = |first: &str, last: &str| teaching("152512", "Übung", "2-Gruppe", 5, ("14:30", "16:00"), "weekly", first, last);
        rows.extend([at("SFB/1.211", weekly("2026-10-09", "2026-10-23")), at("SFB/1.210", weekly("2026-10-09", "2026-10-23")), at("SFB/1.210", weekly("2026-11-06", "2026-11-27"))]);
        // A row of the slot QIS gives no range (12118's): a line of its own, as before.
        rows.push(at("SFB/1.110", EventDate { first_date: None, last_date: None, ..weekly("", "") }));
        let lines: Vec<String> = termine(&rows)
            .into_iter()
            .map(|line| {
                let line: Vec<EventDate> = line.into_iter().cloned().collect();
                let text = termin_text(&line, &DE).unwrap();
                let dates = match text.dates {
                    Dates::List(spans) => spans.into_iter().map(|span| span_text(span, Locale::De)).collect::<Vec<_>>().join(", "),
                    Dates::Range(first, last) => format!("{first}{}", last.map(|last| format!(" – {last}")).unwrap_or_default()),
                    Dates::None => String::new(),
                };
                format!("{} | {} | {dates} | {}", text.when.unwrap_or_default(), text.head.join(" · "), text.tail.join(" · "))
            })
            .collect();
        assert_eq!(
            lines,
            [
                "Di 14:30–16:00 | Seminar · 1-Gruppe · 6 Einzeltermine | 06.10., 20.10., 10.11., 24.11., 15.12., 12.01. | SFB/1.308 (außer 12.01.) / SFB/1.309 (12.01.)",
                "Di 14:30–16:00 | Seminar · Einzeltermin | 2027-01-19 | SFB/6.302 / SFB/6.106 / SFB/6.306",
                "Fr 12:30–14:00 | Übung · 1-Gruppe · wöchentlich | 2026-10-09 – 2026-10-23 | SFB/1.211 / SFB/1.210",
                "Fr 14:30–16:00 | Übung · 2-Gruppe · wöchentlich | 09.10.–23.10., 06.11.–27.11. | SFB/1.211 (09.10.–23.10.) / SFB/1.210",
                "Fr 14:30–16:00 | Übung · 2-Gruppe · wöchentlich |  | SFB/1.110",
            ]
        );
        let refs: Vec<&EventDate> = rows.iter().collect();
        assert_eq!(schedule_of(&refs).len(), 11, "a Schedule per single date, per range and for the row without one, the rooms of one alike");
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
        let slots = own_slots(&rows, &DE);
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
        // In English the kinds and the dates are the language's; QIS's word stays its own.
        let english = own_slots(&rows, &EN);
        assert_eq!(
            english.first().map(|s| (s.label.as_str(), s.small.as_str(), s.title.as_str())),
            Some(("Lec", "1 date · 23 Feb", "Vorlesung · Allgemeine Betriebswirtschaftslehre II · 23 Feb 2027"))
        );
        // The two Tuesday slots stand side by side.
        let (_, styles) = crate::week::geometry(&slots, crate::week::MIN_HOURS).unwrap();
        assert!(styles[0].ends_with("--lane:0;--lanes:2") && styles[3].ends_with("--lane:1;--lanes:2"), "{styles:?}");
        assert!(styles[1].ends_with("--lane:0;--lanes:1") && styles[2].ends_with("--lane:0;--lanes:1"), "{styles:?}");
    }

    /// A slot of the week leads to the lines of the list its rows stand on (owner, 2026-10-17:
    /// „wenn man auf einen Termin im Kalender klickt, dass sich die Termine automatisch ausklappen
    /// und dann der angeklickte Termin highlighted wird"): the Friday Übung to both of its lines (its
    /// ranges, and the row QIS gives none), the Tuesday single date to its own.
    #[test]
    fn a_slot_leads_to_its_lines() {
        let weekly = |first: &str, last: &str| teaching("152512", "Übung", "2-Gruppe", 5, ("14:30", "16:00"), "weekly", first, last);
        let rows = [
            weekly("2026-10-09", "2026-10-23"),
            teaching("148019", "Vorlesung", "[unbenannt]", 2, ("11:45", "15:00"), "single", "2027-02-23", "2027-02-23"),
            weekly("2026-11-06", "2026-11-27"),
            EventDate { first_date: None, last_date: None, ..weekly("", "") },
        ];
        assert_eq!(termine(&rows).len(), 3);
        let slots = own_slots(&rows, &DE);
        let lines: Vec<(u8, Vec<&str>)> = slots.iter().map(|s| (s.day, s.lines.iter().map(String::as_str).collect())).collect();
        assert_eq!(lines, [(5, vec!["termin-1", "termin-3"]), (2, vec!["termin-2"])]);
        // The week with the plan beside it keeps where its own slots lead.
        assert!(with_overlay(&own_groups(&rows, &DE), &Overlay::default(), &DE).iter().all(|s| !s.lines.is_empty()));
    }

    /// Two facts side by side where their values are short (owner, 2026-10-17); a long one takes the
    /// line to itself.
    #[test]
    fn a_long_fact_takes_its_line() {
        assert!(!fact_wide(Some("Zentralcampus")));
        assert!(!fact_wide(Some("Deutsch / Englisch")));
        assert!(fact_wide(Some("Zentralcampus, Sachsendorf")));
        assert!(!fact_wide(None));
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
        let slots = own_slots(&rows, &DE);
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
        assert_eq!(own_slots(&[weekly], &DE).first().map(|s| s.class), Some(""));
        // A type no kind is known of keeps its word, and a row without one says „Termin".
        let odd = teaching("148374", "Blockwoche", "D", 4, ("08:00", "16:00"), "weekly", "2026-10-15", "2027-01-28");
        let bare = EventDate { event_id: "148375".into(), event_type: None, ..odd.clone() };
        assert_eq!(own_slots(&[odd, bare], &DE).iter().map(|s| (s.label.as_str(), s.title.as_str())).collect::<Vec<_>>(), [("Blockwoche", format!("Blockwoche · {abwl}").as_str()), ("Termin", abwl)]);
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
            season: folia_model::labels::Code::parse(if at.winter { "winter" } else { "summer" }),
            year: i64::from(at.year),
            label: at.label(Locale::De),
            starts_on: String::new(),
            ends_on: String::new(),
            is_current,
            teaching_events: 1,
            exam_events: 1,
        }
    }

    #[test]
    fn einplanen_aims_with_the_teaching_not_with_retakes() {
        use folia_plans::studyplan::PlanDoc;
        let semesters = [semester("2026S", false), semester("2026W", true), semester("2027S", false)];
        let aim = |schedule: &[EventDate]| {
            let (current, newest) = semesters_of(&semesters, schedule);
            let current = current.unwrap();
            let offer = folia_plans::study::Offer { season: Some(TurnusSeason::Summer), parity: None };
            folia_stores::studyplan::target_semester(current, newest, offer, None, &PlanDoc::default())
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
        use folia_pages::OverlaySlot;
        // Analysis I's Übung on Tuesday 09:15 and its lecture on Thursday 11:30.
        let rows = [
            teaching("150132", "Übung", "1-Gruppe", 2, ("09:15", "10:45"), "weekly", "2026-10-13", "2027-01-26"),
            teaching("148663", "Vorlesung", "[unbenannt]", 4, ("11:30", "13:00"), "weekly", "2026-10-15", "2027-01-28"),
        ];
        let own = own_groups(&rows, &DE);
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
        let slots = with_overlay(&own, &overlay, &DE);
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
        assert_eq!(with_overlay(&own, &Overlay::default(), &DE), own_slots);
    }

    fn read(date: &EventDate) -> ExamReading {
        let winter = folia_model::rows::Semester {
            key: "2026W".into(),
            season: folia_model::labels::Code::parse("winter"),
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
        assert_eq!(shown_when(&placeholder.shown, &DE), None);
        assert_eq!(marker_text(&placeholder, &DE), "In QIS: So 01:00–02:30 · 27.12.2015");
        let note = exam_note([&placeholder].into_iter(), "WiSe 2026/27", &DE).unwrap_or_default();
        assert!(note.starts_with("01:00–02:30 ist in QIS ein Platzhalter für eine Prüfung ohne festen Termin") && note.contains("nicht ins WiSe 2026/27"), "{note}");

        // A block of oral exams: its days stay in the row, so the line names the time only.
        let block = EventDate { last_date: Some("2027-02-19".into()), ..exam(None, Some("01:00"), Some("02:30"), Some("2027-02-08")) };
        assert_eq!(marker_text(&read(&block), &DE), "In QIS: 01:00–02:30");
        assert!(exam_note([&read(&block)].into_iter(), "WiSe 2026/27", &DE).unwrap_or_default().contains("ohne feste Uhrzeit"));

        // A deadline: read, not marked, and no note.
        let deadline = read(&exam(Some(7), Some("23:45"), Some("24:00"), Some("2027-03-21")));
        assert_eq!(shown_when(&deadline.shown, &DE).as_deref(), Some("So bis 24:00"));
        assert_eq!(stated_slot(&deadline, &DE), "So 23:45–24:00");
        assert_eq!(exam_note([&deadline].into_iter(), "WiSe 2026/27", &DE), None);

        // Only marked: the row shows the source, the line says what is odd.
        let night = read(&exam(Some(2), Some("03:00"), Some("02:00"), Some("2015-02-10")));
        assert_eq!(shown_when(&night.shown, &DE).as_deref(), Some("Di 03:00–02:00"));
        assert_eq!(marker_text(&night, &DE), "Uhrzeit ungewöhnlich · Ende vor Beginn · Datum außerhalb des Semesters, so steht es in QIS");
        assert_eq!(
            exam_note([&night].into_iter(), "WiSe 2026/27", &DE).as_deref(),
            Some("Eine Uhrzeit außerhalb von 06:00–22:00, ein Ende vor dem Beginn oder ein Datum weit außerhalb des WiSe 2026/27 steht so in QIS, ist für eine Prüfung aber ungewöhnlich, vermutlich ein Eingabefehler."),
        );
        let plain = read(&exam(Some(2), Some("09:00"), Some("11:00"), Some("2027-02-09")));
        assert_eq!(exam_note([&plain].into_iter(), "WiSe 2026/27", &DE), None);

        // The same in English.
        assert_eq!(marker_text(&placeholder, &EN), "In QIS: Sun 01:00–02:30 · 27 Dec 2015");
        assert_eq!(shown_when(&deadline.shown, &EN).as_deref(), Some("Sun until 24:00"));
        assert_eq!(marker_text(&night, &EN), "Unusual time · End before start · Date outside the semester, as stated in QIS");
        assert_eq!(
            exam_note([&night].into_iter(), "Winter 2026/27", &EN).as_deref(),
            Some("A time outside 06:00–22:00, an end before the start or a date far outside Winter 2026/27 is what QIS states, but it is unusual for an exam, probably an input error."),
        );
        assert!(exam_note([&placeholder].into_iter(), "Winter 2026/27", &EN).unwrap_or_default().starts_with("In QIS, 01:00–02:30 is a placeholder for an exam without a fixed date"));
    }

    /// Analysis I as the page shows it, with these Termine and exam dates: taught in the
    /// Informatik B.Sc., whose plan places it in the first semester, and requiring module 11000.
    fn analysis(schedule: Vec<EventDate>, exams: Vec<EventDate>) -> ModuleData {
        use folia_model::labels::Code;
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
            page_lang: Some("de".into()),
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
            kind_basis: None,
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
            plan_places: vec![folia_model::rows_detail::PlanPlace { program_id: "079-82-2008".into(), semester: Some(1), start_semester: Some(1), end_semester: Some(1) }],
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
        let graph = structured(&analysis(schedule, exams), &DE);
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
        assert_eq!(graph[1]["itemListElement"][1]["name"], "Modulkatalog");
    }

    #[test]
    fn structured_data_speaks_the_page_s_language() {
        let schedule = vec![teaching("148663", "Vorlesung", "[unbenannt]", 1, ("09:15", "10:45"), "weekly", "2026-10-12", "2027-01-25")];
        let exams = vec![exam(Some(1), Some("09:00"), Some("11:00"), Some("2027-02-15"))];
        let graph = structured(&analysis(schedule, exams), &EN);
        let course = &graph[0];
        assert_eq!(course["educationalAlignment"][0]["targetName"], "1st semester");
        assert_eq!(course["educationalAlignment"][0]["educationalFramework"], "Standard study plan Informatik (B.Sc.), PO 2008");
        let winter = &course["hasCourseInstance"][0];
        assert_eq!((&winter["name"], &winter["location"][0]["name"]), (&"Analysis I (Winter 2026/27)".into(), &"Central Campus Cottbus".into()));
        // QIS's word for the event stays its own; the exam is the app's word.
        assert_eq!(winter["courseSchedule"][0]["name"], "Vorlesung");
        assert_eq!(winter["subEvent"][0]["name"], "Exam Analysis I");
        assert_eq!(graph[1]["itemListElement"][1]["name"], "Module catalogue");
    }

    #[test]
    fn exams_of_another_semester_are_an_instance_of_their_own() {
        // Taught in the summer, retaken in the winter.
        let summer = EventDate { semester_key: "2026S".into(), semester_label: "SoSe 2026".into(), ..teaching("149001", "Vorlesung", "[unbenannt]", 2, ("09:15", "10:45"), "weekly", "2026-04-14", "2026-07-14") };
        let retake = exam(Some(3), Some("13:00"), Some("15:00"), Some("2026-10-07"));
        let graph = structured(&analysis(vec![summer], vec![retake.clone()]), &DE);
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
        assert!(structured(&analysis(Vec::new(), vec![placeholder]), &DE)[0].get("hasCourseInstance").is_none());
    }
}
