//! The planned modules of the Stundenplan (owner, 2026-09-25: „im Style wie beim Modulkatalog oder
//! dem Merken", right of the plan where the screen is wide enough): a head („Module", the credits of
//! the planned modules at the right), then a row per module in plan order, as a row of the
//! catalog's list reads — its title, under it the key the week names it by in its tone („EvS"), its
//! number and what else is to know, its credits at the right. A row opens the module beside the
//! plan; its × at the end takes it out of the plan (as „Merken" does in the Merkliste), and the row
//! stays in its place with „Rückgängig" until the plan changes otherwise. The placeholders of the
//! Regelstudienplan that no module fills yet follow, dashed, each leading to the catalog's modules
//! for it (`fill=p<n>`, whose „Einplanen" fills it), and last, dashed as well, „Modul hinzufügen":
//! the catalog's modules that fit the week.
//!
//! The rows are built as plain values first (`ModuleItem`, `OpenSlot`, `Entry`), which the tests
//! read without rendering, and keyed by them (R5). What the semester's data says and what the
//! plan's placeholders say are two memos: the placeholders come from the store and work the
//! semester out themselves (`key_of`), as `wanted` does (R16). Taking a module out answers in the
//! click and changes the plan after the next frame (R21); „Rückgängig" puts back the plan as it was
//! (`PlanCtx::undo`, which the import and „Plan leeren" share).

use catalog::filter::{CatalogQuery, KindFilter, ProgramRelation, ProgramScope, TurnusFilter};
use catalog::labels::ModuleKind;
use catalog::pages::StudyplanData;
use catalog::rows::Program;
use catalog::studyplan::{self, PlaceholderLine, PlanDoc};
use catalog::timetable::semester::SemesterKey;
use catalog::url::{self, CatalogUrl, StudyplanUrl};
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use super::aside::only_its_events;
use super::head::{add_module_href, hue, is_past, tone_at};
use super::{key_of, PlanCtx};
use crate::format;
use crate::myprogram::MineResolved;
use crate::pages::catalog::finder_on;
use crate::pending::{Change, Pending};
use crate::ui::Icon;

/// How the note of a module taken out of the list begins (`PlanCtx::undo`), the module's id after
/// it: the import's note and „Plan geleert" are the others.
const REMOVED: &str = "Entfernt: ";

/// A planned module as a row.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ModuleItem {
    id: String,
    /// The abbreviation the week names the module by („EvS"); none where the week names it by its
    /// short name, which the title begins with.
    key: Option<String>,
    /// The catalog's title; „Modul 12345" for a module the catalog does not know.
    title: String,
    /// „6", „7,5"; none where the catalog states none.
    credits: Option<String>,
    /// „keine Termine" (the semester has dates, the module none), „nicht im Modulkatalog".
    note: Option<&'static str>,
    hue: &'static str,
}

/// The rows of a semester's planned modules, in plan order, each in the tone the week gives it. A
/// module without a dated Termin says so where the semester's dates are there to be had (neither
/// past, whose dates are gone, nor unpublished, which the head says for all of them).
fn module_items(data: &StudyplanData) -> Vec<ModuleItem> {
    let dated = !data.counts.is_empty() && !is_past(data);
    let names = data.slot_names();
    data.ids
        .iter()
        .enumerate()
        .map(|(position, id)| {
            let row = data.modules.iter().find(|row| row.id == *id);
            let undated = dated && !data.schedule.iter().any(|date| date.module_id == *id && date.ord.is_some());
            let note = match (row, undated) {
                (None, _) => Some("nicht im Modulkatalog"),
                (Some(_), true) => Some("keine Termine"),
                (Some(_), false) => None,
            };
            ModuleItem {
                id: id.clone(),
                key: data.abbrevs.get(id).filter(|abbrev| names.get(id) == Some(*abbrev)).cloned(),
                title: row.map_or_else(|| format!("Modul {id}"), |row| row.title.clone()),
                credits: row.and_then(|row| row.credits).map(format::number),
                note,
                hue: hue(tone_at(position)),
            }
        })
        .collect()
}

/// The credits of the planned modules, the head's sum (owner, 2026-09-25: „die Summe aus allen
/// geplanten Modulen"): „32", „≥ 26" where a module states none (R12). None without a module.
fn credits_sum(data: &StudyplanData) -> Option<String> {
    if data.ids.is_empty() {
        return None;
    }
    let credits: Vec<Option<f64>> = data.ids.iter().map(|id| data.modules.iter().find(|row| row.id == *id).and_then(|row| row.credits)).collect();
    let sum = format::number(credits.iter().flatten().sum());
    Some(if credits.iter().any(Option::is_none) { format!("≥\u{a0}{sum}") } else { sum })
}

/// A row of the list: a planned module, or the one just taken out, in its place.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Entry {
    Module(ModuleItem),
    Gone { id: String, title: String },
}

/// The module just taken out: where its row stood, its id and its title.
type Gone = (usize, String, String);

/// The rows: the planned modules, and the one just taken out in its place — in the click, while
/// the plan still holds it, instead of its row, and then where its row stood.
fn entries(items: &[ModuleItem], gone: Option<&Gone>) -> Vec<Entry> {
    let gone_row = |id: &str, title: &str| Entry::Gone { id: id.to_string(), title: title.to_string() };
    let mut entries: Vec<Entry> = items
        .iter()
        .map(|item| match gone {
            Some((_, id, title)) if *id == item.id => gone_row(id, title),
            _ => Entry::Module(item.clone()),
        })
        .collect();
    if let Some((at, id, title)) = gone.filter(|(_, id, _)| !items.iter().any(|item| item.id == *id)) {
        entries.insert((*at).min(entries.len()), gone_row(id, title));
    }
    entries
}

/// A placeholder of the Regelstudienplan that no module fills yet, as a row: what it asks for, its
/// credits, and where its modules are found.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct OpenSlot {
    pid: u32,
    /// „Fachübergreifendes Studium", „Wahlpflichtmodul 3"
    text: String,
    /// „≥ 6", „10–24" (the row states a choice of at least, or a range); none where it states none.
    credits: Option<String>,
    href: String,
}

/// The placeholders standing in `key` that no module fills, in the plan's order. `mine`: „Mein
/// Studiengang" while the snapshot has it, whose electives a placeholder of it is found among. The
/// catalog it leads to has the finder on, comparing what it compared the last time (`finder_on`).
fn open_slots(doc: &PlanDoc, key: SemesterKey, mine: Option<&Program>) -> Vec<OpenSlot> {
    doc.placeholders_in(key)
        .into_iter()
        .filter(|p| doc.fillers(p.pid).is_empty())
        .map(|p| {
            let line = studyplan::placeholder_line(p, None);
            let credits = line.credits.as_deref().map(|credits| credits.trim_end_matches("\u{a0}LP").to_string());
            // Without the plan's row, only a row that names one module has a tail („unter diesem
            // Namen nicht im Katalog"): the row is not repeated here.
            let single = line.tail.is_some();
            let text = PlaceholderLine { credits: None, tail: None, ..line }.text();
            // As the program's page finds a row's modules (`variants::row_query`), as far as the
            // placeholder alone tells: the FÜS list, a module by its name, the electives. The
            // program's lists only where it is „Mein Studiengang", whose slug the page has; else
            // the BTU's FÜS modules, and whatever fits the week.
            let program = mine.filter(|program| program.id == p.program_id).map(|program| ProgramScope { program_slug: program.slug.clone(), ..Default::default() });
            let query = if studyplan::is_fues_placeholder(p) {
                match program {
                    Some(scope) => CatalogQuery { program: Some(ProgramScope { relation: ProgramRelation::Fues, ..scope }), ..Default::default() },
                    None => CatalogQuery { fues: Some(true), ..Default::default() },
                }
            } else if single {
                CatalogQuery { text: text.clone(), ..Default::default() }
            } else {
                let program = program.map(|scope| ProgramScope { kinds: vec![KindFilter::Stated(ModuleKind::Elective)], ..scope });
                CatalogQuery { program, ..Default::default() }
            };
            let turnus = TurnusFilter { winter: key.winter, summer: !key.winter, ..Default::default() };
            let query = CatalogQuery { turnus, fits: Some(finder_on(key)), ..query };
            OpenSlot { pid: p.pid, text, credits, href: CatalogUrl { query, fill: Some(p.pid), ..Default::default() }.path() }
        })
        .collect()
}

/// The module beside the plan, and while one is on its way there (`pending`), that one: its row is
/// marked in the next frame (R21).
fn open_shown(ctx: PlanCtx) -> Memo<Option<String>> {
    let going = Pending::expect();
    Memo::new(move |_| {
        let target = going.filter(|going| going.change() == Some(Change::Aside)).and_then(|going| going.search_on(url::STUDYPLAN));
        match target {
            Some(search) => StudyplanUrl::parse(&search).open,
            None => ctx.url.with(|url| url.open.clone()),
        }
    })
}

/// „Module" and the credits of the planned modules, a row per module with its ×, the placeholders
/// no module fills yet, and „Modul hinzufügen".
#[component]
pub(super) fn ModuleList(ctx: PlanCtx) -> impl IntoView {
    let items = Memo::new(move |_| ctx.data.with(|data| data.as_ref().map(module_items).unwrap_or_default()));
    let sum = Memo::new(move |_| ctx.data.with(|data| data.as_ref().ok().and_then(credits_sum)));
    let resolved = MineResolved::expect();
    let slots = Memo::new(move |_| {
        let (url, current) = (ctx.url.get(), ctx.current.get());
        let mine = resolved.and_then(MineResolved::exact);
        ctx.plan.map(|plan| plan.with(|doc| open_slots(doc, key_of(&url, current, doc, ctx.today), mine.as_ref()))).unwrap_or_default()
    });
    let open = open_shown(ctx);
    let add = move || add_module_href(resolved, ctx.key.get());

    // The module just taken out keeps its place while its note is the plan's last (another
    // „Rückgängig" replaces it: the import, „Plan leeren", another module taken out).
    let gone = RwSignal::new(None::<Gone>);
    let noted = Memo::new(move |_| ctx.undo.with(|undo| undo.as_ref().and_then(|(note, _)| note.strip_prefix(REMOVED).map(str::to_string))));
    let gone_shown = Memo::new(move |_| {
        let noted = noted.get();
        gone.get().filter(|(_, id, _)| noted.as_deref() == Some(id.as_str()))
    });
    let rows = Memo::new(move |_| {
        let gone = gone_shown.get();
        items.with(|items| entries(items, gone.as_ref()))
    });

    let going = Pending::expect();
    let remove = move |id: String| {
        let Some(plan) = ctx.plan else { return };
        let Some((at, title)) = items.with_untracked(|items| items.iter().position(|item| item.id == id).and_then(|at| Some((at, items.get(at)?.title.clone())))) else {
            return;
        };
        let key = ctx.key.get_untracked();
        let events = ctx.table.with_untracked(|table| table.as_ref().filter(|table| table.key == key).map(|table| only_its_events(table, &id)).unwrap_or_default());
        gone.set(Some((at, id.clone(), title)));
        ctx.undo.set(Some((format!("{REMOVED}{id}"), plan.with_untracked(Clone::clone))));
        // The module beside the plan goes with it, as the panel's „Entfernen" closes it.
        if let Some(going) = going.filter(|_| ctx.url.with_untracked(|url| url.open.as_deref() == Some(id.as_str()))) {
            going.go(&ctx.url.with_untracked(|url| url.with_open(None, None).path()), NavigateOptions { replace: true, scroll: false, ..Default::default() });
        }
        plan.update_after_paint(move |doc| doc.unplan(key, &id, &events));
    };
    let restoring = RwSignal::new(false);
    let restore = move |_| {
        if restoring.get_untracked() {
            return;
        }
        let (Some(plan), Some((_, before))) = (ctx.plan, ctx.undo.get_untracked()) else { return };
        restoring.set(true);
        let undo = ctx.undo;
        plan.update_after_paint(move |doc| {
            *doc = before;
            let _ = undo.try_set(None);
            let _ = gone.try_set(None);
            let _ = restoring.try_set(false);
        });
    };

    let row = move |entry: Entry| match entry {
        Entry::Module(item) => {
            let current = {
                let id = item.id.clone();
                Memo::new(move |_| open.with(|open| open.as_deref() == Some(id.as_str())))
            };
            let href = {
                let id = item.id.clone();
                move || ctx.url.with(|url| url.with_open(Some(&id), None).path())
            };
            let key = match item.key {
                Some(key) => view! { <span class="sp-key">{key}</span> }.into_any(),
                None => view! { <i class="sp-key"></i> }.into_any(),
            };
            let (id, label) = (item.id.clone(), format!("„{}“ aus dem Stundenplan nehmen", item.title));
            view! {
                <div class="sp-row-wrap">
                    <a class=format!("sp-row {}", item.hue) href=href aria-current=move || current.get().then_some("true") data-noscroll="">
                        <span class="t">
                            <b>{item.title}</b>
                            <small>{key}<span class="mono">{item.id}</span>{item.note.map(|note| view! { <span>{note}</span> })}</small>
                        </span>
                        <span class="lp num">{item.credits.map(|credits| view! { {credits}<small>"LP"</small> })}</span>
                    </a>
                    // Beside the row's link, not inside it (as „Merken" beside a row of the list).
                    <button class="icon-btn sp-remove" type="button" title="Aus dem Stundenplan nehmen" aria-label=label on:click=move |_| remove(id.clone())>
                        <Icon name="x"/>
                    </button>
                </div>
            }
            .into_any()
        }
        Entry::Gone { title, .. } => view! {
            <div class="sp-row-wrap">
                <div class="sp-row gone">
                    <span class="t">
                        <b>{title}</b>
                        <small><span>"aus dem Stundenplan genommen"</span></small>
                    </span>
                    <button class="mini hit" type="button" aria-busy=move || restoring.get().then_some("true") on:click=restore>"Rückgängig"</button>
                </div>
            </div>
        }
        .into_any(),
    };
    let slot = |slot: OpenSlot| {
        view! {
            <a class="sp-row open-slot" href=slot.href>
                <span class="t">
                    <b>{slot.text}</b>
                    <small><span>"Platzhalter · Modul finden"</span></small>
                </span>
                <span class="lp num">{slot.credits.map(|credits| view! { {credits}<small>"LP"</small> })}</span>
            </a>
        }
    };
    view! {
        <section class="sp-list" aria-label="Module im Stundenplan">
            <div class="sp-list-head">
                <h2 class="label">"Module"</h2>
                {move || sum.get().map(|sum| view! { <span class="lp num">{sum}<small>"LP"</small></span> })}
            </div>
            <For each=move || rows.get() key=|entry| entry.clone() children=row/>
            <For each=move || slots.get() key=|slot| slot.clone() children=slot/>
            // „+ Modul" as a box like the placeholders' (owner, 2026-09-25): the modules that fit.
            <a class="sp-row open-slot sp-add" href=add>
                <span class="t"><b><Icon name="plus"/>"Modul hinzufügen"</b></span>
            </a>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use catalog::labels::Code;
    use catalog::rows::{CatalogRow, Meta};
    use catalog::rows_detail::{DateCount, DateRow, EventDate};
    use catalog::studyplan::Placeholder;

    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    fn catalog_row(id: &str, title: &str, credits: Option<f64>) -> CatalogRow {
        CatalogRow {
            id: id.into(),
            title: title.into(),
            title_de: None,
            title_en: None,
            credits,
            turnus_season: None,
            turnus_parity: None,
            offer_status: Code::parse("active"),
            teaches_german: None,
            teaches_english: None,
            is_fues: false,
            is_limited: None,
            department: None,
            teaching_events: 0,
            exam_form: None,
            responsible: None,
            kind: None,
            plan_semester: None,
            area: None,
        }
    }

    fn dated(module: &str) -> DateRow {
        DateRow {
            module_id: module.into(),
            ord: Some(1),
            cancelled_dates: None,
            date: EventDate {
                semester_key: "2026W".into(),
                semester_label: "WiSe 2026/27".into(),
                event_id: "148701".into(),
                event_number: None,
                event_title: String::new(),
                event_type: Some("Vorlesung".into()),
                group_name: None,
                weekday: Some(2),
                start_time: Some("11:30".into()),
                end_time: Some("13:00".into()),
                rhythm: Some(Code::parse("weekly")),
                rhythm_raw: None,
                first_date: Some("2026-10-06".into()),
                last_date: Some("2027-01-26".into()),
                room: None,
                campus: None,
                instructor: None,
                comment: None,
                source_url: None,
                room_short: None,
            },
        }
    }

    /// Informatik's first semester in plan order: 12104 with its abbreviation and a Termin, 12107
    /// whose abbreviation another planned module shares, 11112 without any Termin, and 13000,
    /// which the catalog does not know.
    fn first_semester() -> StudyplanData {
        StudyplanData {
            key: key("2026W"),
            label: "WiSe 2026/27".into(),
            semester: None,
            meta: Meta { current_semester: Some("2026W".into()), ..Default::default() },
            ids: vec!["12104".into(), "12107".into(), "11112".into(), "13000".into()],
            modules: vec![
                catalog_row("11112", "Mathematik IT-1", Some(8.0)),
                catalog_row("12104", "Entwicklung von Softwaresystemen", Some(6.0)),
                catalog_row("12107", "Elektrische und elektronische Grundlagen der Informatik", Some(7.5)),
            ],
            missing: vec!["13000".into()],
            schedule: vec![dated("12104"), dated("12107")],
            exams: Vec::new(),
            sws: Vec::new(),
            counts: vec![DateCount { rhythm: Some(Code::parse("weekly")), first_date: "2026-10-05".into(), last_date: Some("2027-01-29".into()), dates: 40 }],
            abbrevs: [("12104", "EvS"), ("12107", "MIT"), ("11112", "MIT")].iter().map(|(id, abbrev)| (id.to_string(), abbrev.to_string())).collect(),
            program: Some("079-82-2008".into()),
        }
    }

    #[test]
    fn a_planned_module_reads_as_a_row_of_the_catalog() {
        let item = |id: &str, key: Option<&str>, title: &str, credits: Option<&str>, note: Option<&'static str>, hue: &'static str| ModuleItem {
            id: id.into(),
            key: key.map(str::to_string),
            title: title.into(),
            credits: credits.map(str::to_string),
            note,
            hue,
        };
        assert_eq!(
            module_items(&first_semester()),
            [
                item("12104", Some("EvS"), "Entwicklung von Softwaresystemen", Some("6"), None, "t-ice"),
                // An abbreviation two planned modules share names neither: the week says its name.
                item("12107", None, "Elektrische und elektronische Grundlagen der Informatik", Some("7,5"), None, "t-sun"),
                item("11112", None, "Mathematik IT-1", Some("8"), Some("keine Termine"), "t-violet"),
                item("13000", None, "Modul 13000", None, Some("nicht im Modulkatalog"), "t-teal"),
            ]
        );
        // A semester without published dates says nothing of a module's own.
        let unpublished = StudyplanData { counts: Vec::new(), ..first_semester() };
        assert_eq!(module_items(&unpublished).get(2).and_then(|item| item.note), None);
    }

    #[test]
    fn the_head_sums_the_planned_modules() {
        // 6 + 7,5 + 8; 13000 states none, so the sum is the least they come to.
        assert_eq!(credits_sum(&first_semester()).as_deref(), Some("≥\u{a0}21,5"));
        let known = StudyplanData { ids: vec!["12104".into(), "12107".into()], ..first_semester() };
        assert_eq!(credits_sum(&known).as_deref(), Some("13,5"));
        assert_eq!(credits_sum(&StudyplanData { ids: Vec::new(), ..first_semester() }), None);
    }

    #[test]
    fn a_module_taken_out_keeps_its_place() {
        let items = module_items(&first_semester());
        let gone: Gone = (1, "12107".into(), "Elektrische und elektronische Grundlagen der Informatik".into());
        let ids = |entries: Vec<Entry>| {
            entries
                .into_iter()
                .map(|entry| match entry {
                    Entry::Module(item) => item.id,
                    Entry::Gone { id, .. } => format!("-{id}"),
                })
                .collect::<Vec<_>>()
        };
        // In the click, while the plan still holds it: in place of its row.
        assert_eq!(ids(entries(&items, Some(&gone))), ["12104", "-12107", "11112", "13000"]);
        // Once the plan has let it go: where its row stood.
        let without: Vec<ModuleItem> = items.iter().filter(|item| item.id != "12107").cloned().collect();
        assert_eq!(ids(entries(&without, Some(&gone))), ["12104", "-12107", "11112", "13000"]);
        // A place past the end is the end; nothing taken out, nothing in between.
        assert_eq!(ids(entries(&without, Some(&(9, "12107".into(), String::new())))), ["12104", "11112", "13000", "-12107"]);
        assert_eq!(ids(entries(&items, None)), ["12104", "12107", "11112", "13000"]);
    }

    fn placeholder(pid: u32, kind: &str, name: &str, credits: &str) -> Placeholder {
        Placeholder {
            pid,
            semester: key("2026W"),
            program_id: "079-82-2008".into(),
            ord: i64::from(pid),
            span: (1, 1),
            credits: Some(credits.into()),
            kind: Some(kind.into()),
            caption: String::new(),
            name: name.into(),
        }
    }

    #[test]
    fn a_placeholder_no_module_fills_leads_to_its_modules() {
        let mut doc = PlanDoc {
            placeholders: vec![
                placeholder(1, "fues", "Fachübergreifendes Studium", "6"),
                placeholder(2, "elective", "Wahlpflichtmodul Informatik", "6"),
                placeholder(3, "compulsory", "Raumbezogene Datenbanken", "5"),
                placeholder(4, "elective", "Anwendungsfach", "10–24"),
            ],
            ..PlanDoc::default()
        };
        // A module fills the fourth: it has its row as a module.
        assert!(doc.plan(key("2026W"), "12104", 1, Some(4)));
        let informatik = Program {
            id: "079-82-2008".into(),
            slug: "bachelor-informatik-2008".into(),
            name: "Informatik".into(),
            degree_level: Code::parse("bachelor"),
            study_variant: None,
            degree_label: Some("B.Sc.".into()),
            degree_raw: "Bachelor".into(),
            degree_display: Some("B.Sc.".into()),
            po_version: "2008".into(),
            po_year: Some(2008),
            family_key: "079-82".into(),
            name_key: "informatik".into(),
            is_latest_po: true,
            source_url: String::new(),
            has_plan: true,
            plan_status: None,
            curricular_modules: 0,
            fues_modules: 0,
            documents: 0,
        };
        let slots = open_slots(&doc, key("2026W"), Some(&informatik));
        let shown: Vec<(u32, &str, Option<&str>)> = slots.iter().map(|slot| (slot.pid, slot.text.as_str(), slot.credits.as_deref())).collect();
        assert_eq!(
            shown,
            [(1, "Fachübergreifendes Studium", Some("≥\u{a0}6")), (2, "Wahlpflichtmodul Informatik", Some("≥\u{a0}6")), (3, "Raumbezogene Datenbanken", Some("5"))]
        );
        let hrefs: Vec<&str> = slots.iter().map(|slot| slot.href.as_str()).collect();
        assert!(hrefs.iter().all(|href| href.contains("fits=2026W") && href.contains("turnus=winter")), "{hrefs:?}");
        assert!(hrefs.first().is_some_and(|href| href.contains("program=bachelor-informatik-2008&list=fues") && href.ends_with("fill=p1")), "{hrefs:?}");
        assert!(hrefs.get(1).is_some_and(|href| href.contains("program=bachelor-informatik-2008&kind=elective") && href.ends_with("fill=p2")), "{hrefs:?}");
        assert!(hrefs.get(2).is_some_and(|href| href.contains("q=Raumbezogene") && href.ends_with("fill=p3")), "{hrefs:?}");
        // Without „Mein Studiengang" (or with another one): the BTU's FÜS modules, and for the
        // electives whatever fits the week.
        let alone = open_slots(&doc, key("2026W"), None);
        assert!(alone.first().is_some_and(|slot| slot.href.contains("fues=only")), "{alone:?}");
        assert!(alone.get(1).is_some_and(|slot| !slot.href.contains("program=")), "{alone:?}");
        // Another semester has none of them.
        assert!(open_slots(&doc, key("2027S"), None).is_empty());
    }
}
