//! The planned modules of the Stundenplan (owner, 2026-09-25: „im Style wie beim Modulkatalog oder
//! dem Merken", right of the plan where the screen is wide enough): a head („Module", the credits of
//! the planned modules at the right), then a row per module in plan order, as a row of the
//! catalog's list reads — its title, under it the key the week names it by in its tone („EvS"), its
//! number and what else is to know, its credits at the right. A row opens the module beside the
//! plan; its × at the end takes it out of the plan (as „Merken" does in the Merkliste), and the row
//! stays in its place with „Rückgängig" until the plan changes otherwise. Then the placeholders of
//! the Regelstudienplan, in the plan's order: one no module counts for is a dashed row leading to
//! the catalog's modules for it (`fill=p<n>`, whose „Einplanen" fills it); one a module counts for
//! is a dashed box around its modules (owner, 2026-09-26: „bei allen Containern … sehen, dass das
//! ein Bereich ist, wo ein Modul ausgewählt ist"), with „Weiteres Modul" while its row takes more
//! (`studyplan::takes_more`: a range of credits, not one module). A row over several Fachsemester
//! stands in each semester it was taken over into, and its box shows what counts for it in the
//! others too. Last, dashed as well, „Modul hinzufügen": the catalog's modules that fit the week.
//!
//! The rows are built as plain values first (`ModuleItem`, `AreaPlan`, `Area`, `Entry`), which the
//! tests read without rendering, and keyed by them (R5). What the plan's placeholders say comes
//! from the store (`held`), which works the semester out itself (`key_of`), as `wanted` does
//! (R16); the list reads it beside the semester's data, a sibling. Taking a module out answers in
//! the click and changes the plan after the next frame (R21); „Rückgängig" puts back the plan as it
//! was (`PlanCtx::undo`, which the import and „Plan leeren" share).

use std::collections::{BTreeMap, BTreeSet};

use catalog::filter::{CatalogQuery, KindFilter, ProgramRelation, ProgramScope, TurnusFilter};
use catalog::labels::ModuleKind;
use catalog::pages::{self, StudyplanData};
use catalog::rows::{CatalogRow, Program};
use catalog::studyplan::{self, Placeholder, PlaceholderLine, PlanDoc};
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
    /// The placeholder of another semester it counts for („für „Anwendungsfach“"), where its row
    /// has none in this one; one of this semester holds the module in its box instead.
    counts_for: Option<String>,
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
                credits: row.and_then(|row| row.credits).map(|value| format::number(value, crate::i18n::locale())),
                note,
                counts_for: None,
                hue: hue(tone_at(position)),
            }
        })
        .collect()
}

/// Credits added up, „32", or „≥ 26" where a module states none (R12).
fn credits_text(credits: &[Option<f64>]) -> String {
    let sum = format::number(credits.iter().flatten().sum(), crate::i18n::locale());
    if credits.iter().any(Option::is_none) {
        format!("≥\u{a0}{sum}")
    } else {
        sum
    }
}

/// The credits of the planned modules, the head's sum (owner, 2026-09-25: „die Summe aus allen
/// geplanten Modulen"): „32", „≥ 26" where a module states none (R12). None without a module.
fn credits_sum(data: &StudyplanData) -> Option<String> {
    if data.ids.is_empty() {
        return None;
    }
    let credits: Vec<Option<f64>> = data.ids.iter().map(|id| data.modules.iter().find(|row| row.id == *id).and_then(|row| row.credits)).collect();
    Some(credits_text(&credits))
}

/// A row of the list: a planned module, or the one just taken out, in its place.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Entry {
    Module(ModuleItem),
    Gone { id: String, title: String },
}

/// The module just taken out: the placeholder of the semester whose box it stood in (none: the
/// list's own rows), where its row stood there, its id and its title.
type Gone = (Option<u32>, usize, String, String);

/// The rows of one part of the list — its own rows (`group` none), or the box of placeholder
/// `group` — with the module just taken out in its place: in the click, while the plan still holds
/// it, instead of its row, and then where its row stood.
fn entries(items: &[ModuleItem], gone: Option<&Gone>, group: Option<u32>) -> Vec<Entry> {
    let gone = gone.filter(|(of, ..)| *of == group);
    let gone_row = |id: &str, title: &str| Entry::Gone { id: id.to_string(), title: title.to_string() };
    let mut entries: Vec<Entry> = items
        .iter()
        .map(|item| match gone {
            Some((_, _, id, title)) if *id == item.id => gone_row(id, title),
            _ => Entry::Module(item.clone()),
        })
        .collect();
    if let Some((_, at, id, title)) = gone.filter(|(_, _, id, _)| !items.iter().any(|item| item.id == *id)) {
        entries.insert((*at).min(entries.len()), gone_row(id, title));
    }
    entries
}

/// A placeholder standing in the semester, as the store says it: a row of the Regelstudienplan,
/// what it asks for, where its modules are found, and what counts for it here and in its row's
/// other semesters (`PlanDoc::same_row`).
#[derive(Clone, Debug, PartialEq)]
struct AreaPlan {
    placeholder: Placeholder,
    /// „Fachübergreifendes Studium", „Komplex Praktische Informatik"
    text: String,
    /// „≥ 6", „10–24" (the row states a choice of at least, or a range); none where it states none.
    credits: Option<String>,
    /// „5.–6. FS" for a row over several Fachsemester.
    span: Option<String>,
    href: String,
    /// The semester's modules that count for its row, in plan order.
    members: Vec<String>,
    /// Those of its row's other semesters, by semester.
    others: Vec<(SemesterKey, String)>,
}

/// What the store says of a semester's placeholders (`areas_of`).
#[derive(Clone, Debug, Default, PartialEq)]
struct Held {
    areas: Vec<AreaPlan>,
    /// The semester's modules that count for a placeholder of another semester whose row has none
    /// in this one, with its name.
    elsewhere: BTreeMap<String, String>,
    /// The catalog's rows of the areas' `others`, for their titles and credits.
    rows: Vec<CatalogRow>,
}

/// A placeholder's name as a row says it, without its credits: „Fachübergreifendes Studium".
fn placeholder_text(p: &Placeholder) -> String {
    let line = studyplan::placeholder_line(p, None, crate::i18n::locale());
    PlaceholderLine { credits: None, tail: None, ..line }.text()
}

/// The placeholders standing in `key`, in the plan's order, with what counts for them. `mine`:
/// „Mein Studiengang" while the snapshot has it, whose electives a placeholder of it is found
/// among. The catalog a placeholder leads to has the finder on, comparing what it compared the
/// last time (`finder_on`).
fn areas_of(doc: &PlanDoc, key: SemesterKey, mine: Option<&Program>) -> Held {
    let here = doc.placeholders_in(key);
    let areas = here
        .iter()
        .map(|p| {
            let line = studyplan::placeholder_line(p, None, crate::i18n::locale());
            let credits = line.amount.clone();
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
            let row: Vec<u32> = doc.same_row(p).iter().map(|q| q.pid).collect();
            let counts = |fills: Option<u32>| fills.is_some_and(|pid| row.contains(&pid));
            AreaPlan {
                placeholder: (*p).clone(),
                text,
                credits,
                span: (p.span.0 != p.span.1).then(|| format!("{}.–{}.\u{a0}FS", p.span.0, p.span.1)),
                href: CatalogUrl { query, fill: Some(p.pid), ..Default::default() }.path(),
                members: doc.modules.iter().filter(|m| m.semester == key && counts(m.fills)).map(|m| m.module_id.clone()).collect(),
                others: doc.modules.iter().filter(|m| m.semester != key && counts(m.fills)).map(|m| (m.semester, m.module_id.clone())).collect(),
            }
        })
        .collect();
    let rows_here: BTreeSet<(&str, i64)> = here.iter().map(|p| (p.program_id.as_str(), p.ord)).collect();
    let elsewhere = doc
        .modules
        .iter()
        .filter(|m| m.semester == key)
        .filter_map(|m| {
            let p = doc.placeholders.iter().find(|p| Some(p.pid) == m.fills)?;
            (!rows_here.contains(&(p.program_id.as_str(), p.ord))).then(|| (m.module_id.clone(), placeholder_text(p)))
        })
        .collect();
    Held { areas, elsewhere, rows: Vec::new() }
}

/// A module of another semester that counts for a box's row: its title, credits and semester.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Other {
    id: String,
    title: String,
    credits: Option<String>,
    /// „WiSe 2026/27"
    semester: String,
}

/// A placeholder of the semester as the list shows it: a dashed row while nothing counts for its
/// row, else a box around what does.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Area {
    pid: u32,
    text: String,
    credits: Option<String>,
    span: Option<String>,
    href: String,
    /// The semester's modules that count for it, and the one just taken out of the box in its
    /// place.
    entries: Vec<Entry>,
    others: Vec<Other>,
    /// What counts for its row, added up: „12", „≥ 6".
    chosen: Option<String>,
    /// Its row takes another module (`studyplan::takes_more`): „Weiteres Modul".
    more: bool,
}

impl Area {
    /// Nothing counts for its row: the dashed row that leads to its modules.
    fn is_open(&self) -> bool {
        self.entries.is_empty() && self.others.is_empty()
    }
}

/// The list: the modules that count for no placeholder of the semester, then the placeholders.
#[derive(Clone, Debug, Default, PartialEq)]
struct Listed {
    rows: Vec<Entry>,
    areas: Vec<Area>,
}

impl Listed {
    /// Where a planned module's row stands, and its title: the box it is in (none: the list's own
    /// rows) and its place among the modules there.
    fn place_of(&self, id: &str) -> Option<(Option<u32>, usize, String)> {
        let find = |entries: &[Entry]| {
            entries
                .iter()
                .filter_map(|entry| match entry {
                    Entry::Module(item) => Some(item),
                    Entry::Gone { .. } => None,
                })
                .enumerate()
                .find(|(_, item)| item.id == id)
                .map(|(at, item)| (at, item.title.clone()))
        };
        find(&self.rows)
            .map(|(at, title)| (None, at, title))
            .or_else(|| self.areas.iter().find_map(|area| find(&area.entries).map(|(at, title)| (Some(area.pid), at, title))))
    }
}

/// The list of a semester: its data, what the store says of its placeholders, and the module just
/// taken out.
fn listed(data: &StudyplanData, held: &Held, gone: Option<&Gone>) -> Listed {
    let items = module_items(data);
    let credits_here = |id: &str| data.modules.iter().find(|row| row.id == id).and_then(|row| row.credits);
    let boxed: BTreeSet<&str> = held.areas.iter().flat_map(|area| area.members.iter().map(String::as_str)).collect();
    let own: Vec<ModuleItem> = items
        .iter()
        .filter(|item| !boxed.contains(item.id.as_str()))
        .map(|item| ModuleItem { counts_for: held.elsewhere.get(&item.id).cloned(), ..item.clone() })
        .collect();
    let areas = held
        .areas
        .iter()
        .map(|area| {
            let members: Vec<ModuleItem> = area.members.iter().filter_map(|id| items.iter().find(|item| item.id == *id).cloned()).collect();
            let row_of = |id: &str| held.rows.iter().find(|row| row.id == id);
            let others: Vec<Other> = area
                .others
                .iter()
                .map(|(semester, id)| Other {
                    id: id.clone(),
                    title: row_of(id).map_or_else(|| format!("Modul {id}"), |row| row.title.clone()),
                    credits: row_of(id).and_then(|row| row.credits).map(|value| format::number(value, crate::i18n::locale())),
                    semester: semester.label(crate::i18n::locale()),
                })
                .collect();
            let chosen: Vec<Option<f64>> =
                members.iter().map(|item| credits_here(&item.id)).chain(area.others.iter().map(|(_, id)| row_of(id).and_then(|row| row.credits))).collect();
            Area {
                pid: area.placeholder.pid,
                text: area.text.clone(),
                credits: area.credits.clone(),
                span: area.span.clone(),
                href: area.href.clone(),
                entries: entries(&members, gone, Some(area.placeholder.pid)),
                others,
                chosen: (!chosen.is_empty()).then(|| credits_text(&chosen)),
                more: studyplan::takes_more(&area.placeholder, &chosen),
            }
        })
        .collect();
    Listed { rows: entries(&own, gone, None), areas }
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
/// with what counts for them, and „Modul hinzufügen".
#[component]
pub(super) fn ModuleList(ctx: PlanCtx) -> impl IntoView {
    let sum = Memo::new(move |_| ctx.data.with(|data| data.as_ref().ok().and_then(credits_sum)));
    let resolved = MineResolved::expect();
    let held = Memo::new(move |_| {
        let (url, current) = (ctx.url.get(), ctx.current.get());
        let mine = resolved.and_then(MineResolved::exact);
        let mut held = ctx.plan.map(|plan| plan.with(|doc| areas_of(doc, key_of(&url, current, doc, ctx.today), mine.as_ref()))).unwrap_or_default();
        // The titles and credits of what counts for a row in its other semesters: one question to
        // the catalog, and none where nothing does.
        let ids: Vec<String> = held.areas.iter().flat_map(|area| area.others.iter().map(|(_, id)| id.clone())).collect();
        if !ids.is_empty() {
            let rows = ctx.source.with_value(|source| source.as_ref().and_then(|source| source.run(|db| pages::studyplan_modules(db, &ids)).ok()));
            held.rows = rows.map(|(rows, _)| rows).unwrap_or_default();
        }
        held
    });
    let open = open_shown(ctx);
    let add = move || add_module_href(resolved, ctx.key.get());

    // The module just taken out keeps its place while its note is the plan's last (another
    // „Rückgängig" replaces it: the import, „Plan leeren", another module taken out).
    let gone = RwSignal::new(None::<Gone>);
    let noted = Memo::new(move |_| ctx.undo.with(|undo| undo.as_ref().and_then(|(note, _)| note.strip_prefix(REMOVED).map(str::to_string))));
    let gone_shown = Memo::new(move |_| {
        let noted = noted.get();
        gone.get().filter(|(_, _, id, _)| noted.as_deref() == Some(id.as_str()))
    });
    // The semester's data and what the store says of its placeholders are siblings, both from the
    // plan (R16).
    let list = Memo::new(move |_| {
        let gone = gone_shown.get();
        held.with(|held| ctx.data.with(|data| data.as_ref().map(|data| listed(data, held, gone.as_ref())).unwrap_or_default()))
    });
    let rows = Memo::new(move |_| list.with(|list| list.rows.clone()));
    let areas = Memo::new(move |_| list.with(|list| list.areas.clone()));

    let going = Pending::expect();
    let remove = move |id: String| {
        let Some(plan) = ctx.plan else { return };
        let Some((group, at, title)) = list.with_untracked(|list| list.place_of(&id)) else { return };
        let key = ctx.key.get_untracked();
        let events = ctx.table.with_untracked(|table| table.as_ref().filter(|table| table.key == key).map(|table| only_its_events(table, &id)).unwrap_or_default());
        gone.set(Some((group, at, id.clone(), title)));
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

    // A link that opens a module beside the plan, marked while it is the one there.
    let beside = move |id: String| {
        let current = {
            let id = id.clone();
            Memo::new(move |_| open.with(|open| open.as_deref() == Some(id.as_str())))
        };
        let href = move || ctx.url.with(|url| url.with_open(Some(&id), None).path());
        (href, move || current.get().then_some("true"))
    };
    let row = move |entry: Entry| match entry {
        Entry::Module(item) => {
            let (href, current) = beside(item.id.clone());
            let key = match item.key {
                Some(key) => view! { <span class="sp-key">{key}</span> }.into_any(),
                None => view! { <i class="sp-key"></i> }.into_any(),
            };
            let counts_for = item.counts_for.map(|name| view! { <span>{format!("für „{name}“")}</span> });
            let (id, label) = (item.id.clone(), format!("„{}“ aus dem Stundenplan nehmen", item.title));
            view! {
                <div class="sp-row-wrap">
                    <a class=format!("sp-row {}", item.hue) href=href aria-current=current data-noscroll="">
                        <span class="t">
                            <b>{item.title}</b>
                            <small>{key}<span class="mono">{item.id}</span>{item.note.map(|note| view! { <span>{note}</span> })}{counts_for}</small>
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
    // A module of the row's other semester: opens beside the plan as well, but is taken out there.
    let other = move |other: Other| {
        let (href, current) = beside(other.id.clone());
        view! {
            <a class="sp-row other" href=href aria-current=current data-noscroll="">
                <span class="t">
                    <b>{other.title}</b>
                    <small><span>{other.semester}</span><span class="mono">{other.id}</span></small>
                </span>
                <span class="lp num">{other.credits.map(|credits| view! { {credits}<small>"LP"</small> })}</span>
            </a>
        }
    };
    let area = move |area: Area| {
        let credits = area.credits.clone().map(|credits| view! { {credits}<small>"LP"</small> });
        if area.is_open() {
            let small = ["Platzhalter".to_string()].into_iter().chain(area.span.clone()).chain(["Modul\u{a0}finden".to_string()]).collect::<Vec<_>>().join(" · ");
            return view! {
                <a class="sp-row open-slot" href=area.href>
                    <span class="t">
                        <b>{area.text}</b>
                        <small><span>{small}</span></small>
                    </span>
                    <span class="lp num">{credits}</span>
                </a>
            }
            .into_any();
        }
        // What counts for it so far, while it takes more; the span of a row over several
        // Fachsemester.
        let chosen = area.chosen.clone().filter(|_| area.more).map(|chosen| format!("{chosen}\u{a0}LP geplant"));
        let small: Vec<String> = area.span.clone().into_iter().chain(chosen).collect();
        let small = (!small.is_empty()).then(|| view! { <small><span>{small.join(" · ")}</span></small> });
        let label = format!("Bereich „{}“", area.text);
        view! {
            <div class="sp-area" role="group" aria-label=label>
                <div class="sp-area-head">
                    <span class="t"><b>{area.text}</b>{small}</span>
                    <span class="lp num">{credits}</span>
                </div>
                {area.entries.into_iter().map(row).collect_view()}
                {area.others.into_iter().map(other).collect_view()}
                {area.more.then(|| view! {
                    <a class="sp-area-add" href=area.href><Icon name="plus"/>"Weiteres Modul"</a>
                })}
            </div>
        }
        .into_any()
    };
    view! {
        <section class="sp-list" aria-label="Module im Stundenplan">
            <div class="sp-list-head">
                <h2 class="label">"Module"</h2>
                {move || sum.get().map(|sum| view! { <span class="lp num">{sum}<small>"LP"</small></span> })}
            </div>
            <For each=move || rows.get() key=|entry| entry.clone() children=row/>
            <For each=move || areas.get() key=|area| area.clone() children=area/>
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
            locale: catalog::Locale::De,
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
            counts_for: None,
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

    fn ids(entries: &[Entry]) -> Vec<String> {
        entries
            .iter()
            .map(|entry| match entry {
                Entry::Module(item) => item.id.clone(),
                Entry::Gone { id, .. } => format!("-{id}"),
            })
            .collect()
    }

    #[test]
    fn a_module_taken_out_keeps_its_place() {
        let items = module_items(&first_semester());
        let gone: Gone = (None, 1, "12107".into(), "Elektrische und elektronische Grundlagen der Informatik".into());
        // In the click, while the plan still holds it: in place of its row.
        assert_eq!(ids(&entries(&items, Some(&gone), None)), ["12104", "-12107", "11112", "13000"]);
        // Once the plan has let it go: where its row stood.
        let without: Vec<ModuleItem> = items.iter().filter(|item| item.id != "12107").cloned().collect();
        assert_eq!(ids(&entries(&without, Some(&gone), None)), ["12104", "-12107", "11112", "13000"]);
        // A place past the end is the end; nothing taken out, nothing in between.
        assert_eq!(ids(&entries(&without, Some(&(None, 9, "12107".into(), String::new())), None)), ["12104", "11112", "13000", "-12107"]);
        assert_eq!(ids(&entries(&items, None, None)), ["12104", "12107", "11112", "13000"]);
        // One taken out of a placeholder's box stays in that box, and nowhere else.
        let boxed: Gone = (Some(4), 0, "12107".into(), String::new());
        assert_eq!(ids(&entries(&without, Some(&boxed), None)), ["12104", "11112", "13000"]);
        assert_eq!(ids(&entries(&[], Some(&boxed), Some(4))), ["-12107"]);
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

    fn informatik() -> Program {
        Program {
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
        }
    }

    #[test]
    fn a_placeholder_leads_to_its_modules() {
        let mut doc = PlanDoc {
            placeholders: vec![
                placeholder(1, "fues", "Fachübergreifendes Studium", "6"),
                placeholder(2, "elective", "Wahlpflichtmodul Informatik", "6"),
                placeholder(3, "compulsory", "Raumbezogene Datenbanken", "5"),
                placeholder(4, "elective", "Anwendungsfach", "10–24"),
            ],
            ..PlanDoc::default()
        };
        // A module counts for the fourth: it is in its box.
        assert!(doc.plan(key("2026W"), "12104", 1, Some(4)));
        let held = areas_of(&doc, key("2026W"), Some(&informatik()));
        let shown: Vec<(u32, &str, Option<&str>, Vec<&str>)> =
            held.areas.iter().map(|area| (area.placeholder.pid, area.text.as_str(), area.credits.as_deref(), area.members.iter().map(String::as_str).collect())).collect();
        assert_eq!(
            shown,
            [
                (1, "Fachübergreifendes Studium", Some("≥\u{a0}6"), vec![]),
                (2, "Wahlpflichtmodul Informatik", Some("≥\u{a0}6"), vec![]),
                (3, "Raumbezogene Datenbanken", Some("5"), vec![]),
                (4, "Anwendungsfach", Some("10–24"), vec!["12104"]),
            ]
        );
        let hrefs: Vec<&str> = held.areas.iter().map(|area| area.href.as_str()).collect();
        assert!(hrefs.iter().all(|href| href.contains("fits=2026W") && href.contains("turnus=winter")), "{hrefs:?}");
        assert!(hrefs.first().is_some_and(|href| href.contains("program=bachelor-informatik-2008&list=fues") && href.ends_with("fill=p1")), "{hrefs:?}");
        assert!(hrefs.get(1).is_some_and(|href| href.contains("program=bachelor-informatik-2008&kind=elective") && href.ends_with("fill=p2")), "{hrefs:?}");
        assert!(hrefs.get(2).is_some_and(|href| href.contains("q=Raumbezogene") && href.ends_with("fill=p3")), "{hrefs:?}");
        assert!(hrefs.get(3).is_some_and(|href| href.ends_with("fill=p4")), "{hrefs:?}");
        // Without „Mein Studiengang" (or with another one): the BTU's FÜS modules, and for the
        // electives whatever fits the week.
        let alone = areas_of(&doc, key("2026W"), None);
        assert!(alone.areas.first().is_some_and(|area| area.href.contains("fues=only")), "{alone:?}");
        assert!(alone.areas.get(1).is_some_and(|area| !area.href.contains("program=")), "{alone:?}");
        // Another semester has none of them.
        assert!(areas_of(&doc, key("2027S"), None).areas.is_empty());
    }

    /// Informatik's fifth semester: the Bachelor-Arbeit, and two Komplexe of 10–24 LP over the fifth
    /// and sixth Fachsemester, the first with Betriebssysteme II.
    fn fifth_semester() -> (PlanDoc, StudyplanData) {
        let w = key("2026W");
        let komplex = |pid: u32, name: &str| Placeholder { span: (5, 6), ord: i64::from(pid) + 18, ..placeholder(pid, "elective", name, "10–24") };
        let mut doc = PlanDoc { placeholders: vec![komplex(1, "Komplex Grundlagen der Informatik"), komplex(2, "Komplex Praktische Informatik")], ..PlanDoc::default() };
        assert!(doc.plan(w, "12333", 1, None) && doc.plan(w, "12339", 2, Some(1)));
        let data = StudyplanData {
            ids: vec!["12333".into(), "12339".into()],
            modules: vec![catalog_row("12333", "Bachelor-Arbeit", Some(12.0)), catalog_row("12339", "Betriebssysteme II", Some(6.0))],
            missing: Vec::new(),
            schedule: vec![dated("12333"), dated("12339")],
            abbrevs: BTreeMap::new(),
            ..first_semester()
        };
        (doc, data)
    }

    #[test]
    fn a_module_for_a_placeholder_stands_in_its_box() {
        let (doc, data) = fifth_semester();
        let list = listed(&data, &areas_of(&doc, key("2026W"), None), None);
        // The Bachelor-Arbeit counts for none: a row of the list's own.
        assert_eq!(ids(&list.rows), ["12333"]);
        let [first, second] = list.areas.as_slice() else { panic!("{list:?}") };
        // Betriebssysteme II in the box of its Komplex, which takes more up to 24 LP.
        assert_eq!((ids(&first.entries), first.span.as_deref(), first.chosen.as_deref(), first.more, first.is_open()), (vec!["12339".to_string()], Some("5.–6.\u{a0}FS"), Some("6"), true, false));
        assert!(first.href.ends_with("fill=p1"));
        // The other Komplex: nothing counts for it, the dashed row.
        assert!(second.is_open() && second.more && second.chosen.is_none());
        assert_eq!(list.place_of("12339"), Some((Some(1), 0, "Betriebssysteme II".to_string())));
        assert_eq!(list.place_of("12333"), Some((None, 0, "Bachelor-Arbeit".to_string())));
        // Taken out of its box: it stays there with „Rückgängig", and the box stays a box.
        let gone: Gone = (Some(1), 0, "12339".into(), "Betriebssysteme II".into());
        let mut without = doc.clone();
        without.unplan(key("2026W"), "12339", &[]);
        let data_without = StudyplanData { ids: vec!["12333".into()], ..data.clone() };
        let after = listed(&data_without, &areas_of(&without, key("2026W"), None), Some(&gone));
        assert_eq!((ids(&after.rows), ids(&after.areas[0].entries)), (vec!["12333".to_string()], vec!["-12339".to_string()]));
        assert!(!after.areas[0].is_open());
    }

    #[test]
    fn a_full_row_takes_no_more_and_a_row_counts_in_all_its_semesters() {
        let (w, s) = (key("2026W"), key("2027S"));
        let (mut doc, data) = fifth_semester();
        // Two more modules of 6 LP in the winter and one of 6 in the summer come to 24: full.
        assert!(doc.plan(w, "12340", 3, Some(1)) && doc.plan(w, "12341", 3, Some(1)));
        // The sixth Fachsemester taken over into the summer: the Komplex stands there too.
        let summer = Placeholder { pid: 3, semester: s, ..doc.placeholders[0].clone() };
        doc.placeholders.push(summer);
        assert!(doc.plan(s, "12342", 4, Some(3)));
        let data = StudyplanData {
            ids: vec!["12333".into(), "12339".into(), "12340".into(), "12341".into()],
            modules: [data.modules, vec![catalog_row("12340", "Rechnernetze", Some(6.0)), catalog_row("12341", "Compilerbau", Some(6.0))]].concat(),
            ..data
        };
        let mut held = areas_of(&doc, w, None);
        assert_eq!(held.areas[0].others, [(s, "12342".to_string())]);
        held.rows = vec![catalog_row("12342", "Datenbanken II", Some(6.0))];
        let list = listed(&data, &held, None);
        let komplex = &list.areas[0];
        assert_eq!(ids(&komplex.entries), ["12339", "12340", "12341"]);
        assert_eq!(komplex.others, [Other { id: "12342".into(), title: "Datenbanken II".into(), credits: Some("6".into()), semester: "SoSe 2027".into() }]);
        assert_eq!((komplex.chosen.as_deref(), komplex.more), (Some("24"), false));
        // In the summer: its own module in its box, the winter's three beside it.
        let summer = areas_of(&doc, s, None);
        assert_eq!((summer.areas.len(), summer.areas[0].members.clone(), summer.areas[0].others.len()), (1, vec!["12342".to_string()], 3));
    }

    #[test]
    fn a_module_for_a_placeholder_of_another_semester_says_so() {
        let (w, s) = (key("2026W"), key("2027S"));
        let (mut doc, data) = fifth_semester();
        // The Bachelor-Arbeit counts for a placeholder of the summer whose row the winter lacks.
        doc.placeholders.push(Placeholder { pid: 7, semester: s, ord: 40, span: (6, 6), ..placeholder(7, "elective", "Wahlpflichtmodul Informatik", "6") });
        doc.set_fills(w, "12333", Some(7));
        let held = areas_of(&doc, w, None);
        assert_eq!(held.elsewhere, BTreeMap::from([("12333".to_string(), "Wahlpflichtmodul Informatik".to_string())]));
        let list = listed(&data, &held, None);
        assert!(matches!(list.rows.as_slice(), [Entry::Module(item)] if item.counts_for.as_deref() == Some("Wahlpflichtmodul Informatik")), "{list:?}");
    }
}
