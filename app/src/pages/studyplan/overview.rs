//! The view „Übersicht" of the Studienplan (`view=all`), and taking a Regelstudienplan over
//! (`import=`).
//!
//! The Übersicht is the whole study at a glance: how many credits are planned (of what „Mein
//! Studiengang"'s plan comes to), then one section per semester from the Studienbeginn to the end
//! of the plan, each with its Fachsemester and credits, its modules and the rows of the
//! Regelstudienplan still to fill (placeholders, in the catalog's words for them), and a way to
//! find a module for each. A row over several semesters stands once, under its first one, and
//! counts in none of their sums (a 60-LP row over six semesters would make the first read 90 LP).
//!
//! Taking a Regelstudienplan over (`ImportPanel`) stands above it: the program (the one of the
//! address, else „Mein Studiengang"), its plan and the page that fills the plan's direction row,
//! the Studienbeginn and the Fachsemester to start from, and a preview of what that adds. The
//! choices are the panel's own; only the program the student came with is in the address.
//! „Übernehmen" answers at once and writes after the next frame (R21), the Studienbeginn always
//! and the program when asked, then shows the Übersicht with „Übernommen … · Rückgängig".
//!
//! Both read the plan through memos of the store and the catalog through memos of those, never a
//! memo together with its own source (R16); what they show is built as plain values first
//! (`Overview`, `Form`), which the tests read without rendering.

use std::collections::{BTreeMap, BTreeSet};

use catalog::filter::{CatalogQuery, FitsFilter, KindFilter, ProgramScope, TurnusFilter};
use catalog::labels::{Code, ModuleKind, TurnusSeason};
use catalog::pages::{self, PlanSource};
use catalog::plan;
use catalog::queries;
use catalog::rows::{CatalogRow, Program};
use catalog::studyplan::{self, Import, MineDoc, Placeholder, PlanDoc, Planned, Resolved};
use catalog::timetable::semester::{fachsemester, SemesterKey};
use catalog::url::{CatalogUrl, PlanView, StudyplanUrl};
use catalog::variants::Supplement;
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use super::head::{add_module_href, semester_href};
use super::{PlanCtx, SheetToggle};
use crate::combobox::{ComboItem, Combobox};
use crate::data::{DataError, Source};
use crate::format;
use crate::myprogram::{program_name, MineResolved};
use crate::nav;
use crate::pending::Pending;
use crate::ui::{ErrorState, Icon};

/// How the note of a Regelstudienplan taken over begins; the sidebar has its own („Plan
/// geleert"), and the two share `PlanCtx::undo`.
const IMPORTED: &str = "Übernommen: ";

/// The id of the import's program picker, which an import of „Mein Studiengang" without one
/// focuses.
const PROGRAM_ID: &str = "sp-import-program";

/// How many semesters before the current one „Studienbeginn" offers (as the sidebar does).
const STARTS_BEFORE: i32 = 12;

/// The most semesters the Übersicht lists, from the first: a start stored decades ago lists no
/// more.
const MOST_TERMS: usize = 40;

// ---------- the Übersicht ----------

/// All semesters at a glance: the credits planned, then each semester with its modules and
/// placeholders, and „Übernommen … · Rückgängig" after an import.
#[component]
pub(super) fn OverviewView(ctx: PlanCtx) -> impl IntoView {
    let plan = ctx.plan;
    let mine = ctx.mine;
    let source = ctx.source;
    let resolved = MineResolved::expect();

    // The plan, and apart from it (each from the store, R16) its modules and the programs of its
    // placeholders, which the catalog is asked about.
    let doc = Memo::new(move |_| plan.map(|plan| plan.with(Clone::clone)).unwrap_or_default());
    let ids = Memo::new(move |_| plan.map(|plan| plan.with(module_ids)).unwrap_or_default());
    let programs = Memo::new(move |_| {
        plan.map(|plan| plan.with(|doc| doc.placeholders.iter().map(|p| p.program_id.clone()).collect::<BTreeSet<_>>())).unwrap_or_default()
    });
    let rows = Memo::new(move |_| {
        let ids = ids.get();
        let rows = source.with_value(|source| source.as_ref().and_then(|source| source.run(|db| pages::studyplan_modules(db, &ids)).ok()));
        rows.map(|(rows, _)| rows.into_iter().map(|row| (row.id.clone(), row)).collect::<BTreeMap<_, _>>()).unwrap_or_default()
    });
    let known = Memo::new(move |_| {
        let programs = programs.get();
        source.with_value(|source| programs.into_iter().map(|id| (id.clone(), known_of(source.as_ref(), &id))).collect::<BTreeMap<_, _>>())
    });
    // „Mein Studiengang": its start, and the plan it names, whose total the head counts against.
    let start = Memo::new(move |_| mine.and_then(|mine| mine.start()));
    let named = Memo::new(move |_| mine.map(|mine| mine.with(|doc| (doc.caption.clone(), doc.direction.clone()))).unwrap_or_default());
    let goal = Memo::new(move |_| {
        let info = resolved.and_then(|resolved| resolved.0.get()).filter(|info| info.exact)?;
        let (caption, direction) = named.get();
        let plans = source.with_value(|source| source.as_ref().and_then(|source| source.run(|db| pages::plan_source(db, &info.program.id)).ok().flatten()))?;
        Some(goal_of(&plans, caption.as_deref(), direction.as_deref()))
    });
    let base = Memo::new(move |_| ctx.url.with(|url| url.with_open(None, None)));
    let shown = Memo::new(move |_| {
        let (current, start, base) = (ctx.current.get(), start.get(), base.get());
        doc.with(|doc| {
            // The page's semester, worked out as the page does: the one the module beside the
            // plan shows a module in where it is planned there.
            let page = super::key_of(&base, current, doc, ctx.today);
            rows.with(|rows| known.with(|known| goal.with(|goal| overview(doc, rows, known, goal.as_ref(), start, current, &base, page))))
        })
    });
    let head = Memo::new(move |_| shown.with(|shown| shown.head.clone()));
    let sum = Memo::new(move |_| shown.with(|shown| shown.sum.clone()));
    // The semesters by their key, and each line by what it is: a change within a semester (a
    // module taken out, a placeholder filled) touches its own lines only, and the rows around
    // them stay the elements they were, the focus included.
    let terms = Memo::new(move |_| shown.with(|shown| shown.terms.clone()));
    let keys = Memo::new(move |_| terms.with(|terms| terms.iter().map(|term| term.key).collect::<Vec<_>>()));

    // The note of an import lasts until the student goes on: to another view (this one goes),
    // or to the next import.
    let undo = ctx.undo;
    on_cleanup(move || forget_import(undo));
    let importing = Memo::new(move |_| ctx.url.with(|url| url.import.is_some()));
    Effect::new(move |before: Option<bool>| {
        let now = importing.get();
        if before == Some(false) && now {
            forget_import(undo);
        }
        now
    });

    view! {
        <div class="sp-head">
            <h2>{move || head.get()}</h2>
            <SheetToggle/>
        </div>
        {move || sum.with(|sum| (!sum.is_empty()).then(|| view! { <p class="sp-sum">{sum.clone()}</p> }))}
        <ImportNote ctx/>
        <For each=move || keys.get() key=|key| *key children=move |key: SemesterKey| term_view(ctx, resolved, terms, key)/>
    }
}

/// Takes the note of an import away, when it is one; the sidebar's „Plan geleert" stays.
fn forget_import(undo: RwSignal<Option<(String, PlanDoc)>>) {
    let _ = undo.try_update(|undo| {
        if undo.as_ref().is_some_and(|(note, _)| note.starts_with(IMPORTED)) {
            *undo = None;
        }
    });
}

/// „Übernommen: 13 Module, 10 Platzhalter · Rückgängig": the plan before the import comes back
/// after the next frame (the pages let go of what it added then).
#[component]
fn ImportNote(ctx: PlanCtx) -> impl IntoView {
    let note = Memo::new(move |_| ctx.undo.with(|undo| undo.as_ref().map(|(note, _)| note.clone()).filter(|note| note.starts_with(IMPORTED))));
    let restoring = RwSignal::new(false);
    let restore = move |_| {
        let (Some(plan), Some((_, before))) = (ctx.plan, ctx.undo.get_untracked()) else { return };
        if restoring.get_untracked() {
            return;
        }
        restoring.set(true);
        let undo = ctx.undo;
        plan.update_after_paint(move |doc| {
            *doc = before;
            let _ = undo.try_set(None);
            let _ = restoring.try_set(false);
        });
    };
    move || {
        note.get().map(|note| {
            view! {
                <p class="note quiet">
                    <Icon name="check"/>
                    <span>{note}</span>
                    <button class="mini hit" type="button" aria-busy=move || restoring.get().then_some("true") on:click=restore>"Rückgängig"</button>
                </p>
            }
        })
    }
}

/// One semester: its head, its lines, and „+ Modul". It reads its own part of the Übersicht, and
/// each line its own line (R5).
fn term_view(ctx: PlanCtx, resolved: Option<MineResolved>, terms: Memo<Vec<Term>>, key: SemesterKey) -> impl IntoView {
    let term = Memo::new(move |_| terms.with(|terms| terms.iter().find(|term| term.key == key).cloned()));
    let head = Memo::new(move |_| term.with(|term| term.as_ref().map(|term| (term.label.clone(), term.now, term.lp.clone(), term.week.clone())).unwrap_or_default()));
    let ids = Memo::new(move |_| term.with(|term| term.as_ref().map(|term| term.lines.iter().map(Line::id).collect::<Vec<_>>()).unwrap_or_default()));
    let add = move || add_module_href(resolved, key);
    view! {
        <section class="sp-term" aria-label=move || head.with(|head| head.0.clone())>
            <h3>
                <span>{move || head.with(|head| head.0.clone())}</span>
                {move || head.with(|head| head.1).then(|| view! { <small>"jetzt"</small> })}
                {move || head.with(|head| head.2.clone()).map(|lp| view! { <span class="num">{lp}</span> })}
                <a href=move || head.with(|head| head.3.clone())>"Woche →"</a>
            </h3>
            <For each=move || ids.get() key=|id| id.clone() children=move |id: LineId| line_view(ctx, term, key, id)/>
            <ul class="sp-mods"><li><a class="mini" id=add_id(key) href=add>"+ Modul"</a></li></ul>
        </section>
    }
}

/// A line of a semester, from the semester's lines by what it is.
fn line_view(ctx: PlanCtx, term: Memo<Option<Term>>, key: SemesterKey, id: LineId) -> impl IntoView {
    let line = Memo::new(move |_| term.with(|term| term.as_ref().and_then(|term| term.lines.iter().find(|line| line.id() == id).cloned())));
    move || {
        line.get().map(|line| match line {
            Line::Module(module) => module_view(ctx, term, key, module).into_any(),
            Line::Placeholder(row) => placeholder_view(ctx, term, key, row).into_any(),
        })
    }
}

/// The id of a semester's „+ Modul": where the focus goes when its last line is taken out.
fn add_id(key: SemesterKey) -> String {
    format!("sp-add-{}", key.key())
}

/// Where the focus goes once `gone` is taken out of its semester: the „Entfernen" of the line
/// after it (not one of the modules that filled a placeholder taken out: those move up to the
/// semester's modules), else of the line before it, else the semester's „+ Modul". Empty where
/// the semester is not shown.
fn focus_after(term: Option<&Term>, gone: &LineId) -> String {
    let Some(term) = term else { return String::new() };
    let lines = &term.lines;
    let Some(at) = lines.iter().position(|line| line.id() == *gone) else { return add_id(term.key) };
    let fills = matches!(gone, LineId::Placeholder(_));
    let after = lines.iter().skip(at + 1).find(|line| !(fills && matches!(line, Line::Module(module) if module.filler)));
    let before = at.checked_sub(1).and_then(|index| lines.get(index));
    after.or(before).map_or_else(|| add_id(term.key), |line| line.id().dom(term.key))
}

/// Moves the focus once the plan's change is on the page (the frame after it).
fn focus_next(target: String) {
    if !target.is_empty() {
        request_animation_frame(move || nav::focus_by_id(&target));
    }
}

/// A module: number, title (a link to it beside the plan), what the plan calls it and what it
/// fills, a warning when its half of the year is the other one, credits, and „Entfernen".
fn module_view(ctx: PlanCtx, term: Memo<Option<Term>>, key: SemesterKey, module: ModuleLine) -> impl IntoView {
    // Taking a module out changes what the pages ask the catalog: the button answers at once, the
    // plan follows after the next frame (R21), and the line goes with it; the focus goes on to the
    // next line's „Entfernen".
    let busy = RwSignal::new(false);
    let (id, semester) = (module.id.clone(), module.semester);
    let line = LineId::Module(module.id.clone());
    let dom = line.dom(key);
    let remove = move |_| {
        let Some(plan) = ctx.plan else { return };
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        let (id, source) = (id.clone(), ctx.source);
        let next = term.with_untracked(|term| focus_after(term.as_ref(), &line));
        plan.update_after_paint(move |doc| {
            let others = doc.modules_in(semester);
            let own = source.with_value(|source| own_events(source.as_ref(), semester, &others, &id));
            doc.unplan(semester, &id, &own);
            let _ = busy.try_set(false);
            focus_next(next);
        });
    };
    let title = module.title.clone().unwrap_or_else(|| "nicht im Modulkatalog".to_string());
    view! {
        <div class="sp-row" class:filler=module.filler class:gone=module.title.is_none()>
            <span class="mono">{module.id.clone()}</span>
            <span>
                <a href=module.href.clone() data-noscroll="">{title}</a>
                {module.kind.clone().map(|(code, label)| view! { " " <small class=format!("kind k-{code}")><i></i>{label}</small> })}
                {module.note.clone().map(|note| view! { " " <small>{note}</small> })}
                {module.season.map(|season| view! { " " <small class="badge warn">{season}</small> })}
            </span>
            {module.lp.clone().map(|lp| view! { <span class="lp">{lp}</span> })}
            <button class="icon-btn" id=dom type="button" aria-label="Entfernen" title="Entfernen" aria-busy=move || busy.get().then_some("true") on:click=remove>
                <Icon name="x"/>
            </button>
        </div>
    }
}

/// A placeholder: what the plan asks for, in the catalog's words, the semesters it spans, how
/// much of it is filled, „Modul finden" while something is missing, and „Entfernen".
fn placeholder_view(ctx: PlanCtx, term: Memo<Option<Term>>, key: SemesterKey, row: PlaceholderRow) -> impl IntoView {
    let pid = row.pid;
    let line = LineId::Placeholder(pid);
    let dom = line.dom(key);
    // Only the Übersicht shows placeholders, and no query follows them: taken out at once, and
    // the focus goes on to the next line's „Entfernen".
    let remove = move |_| {
        if let Some(plan) = ctx.plan {
            let next = term.with_untracked(|term| focus_after(term.as_ref(), &line));
            plan.update(|doc| doc.remove_placeholder(pid));
            focus_next(next);
        }
    };
    view! {
        <div class="sp-row placeholder" class:gone=row.changed>
            <span>
                {row.credits.clone().map(|credits| view! { <b>{credits}</b> " " })}
                {row.text.clone()}
                {row.span.clone().map(|span| view! { " " <small>{span}</small> })}
                {row.changed.then(|| view! { " " <small>"nicht mehr im Regelstudienplan"</small> })}
            </span>
            {row.filled.clone().map(|filled| view! { <span class="lp">{filled}</span> })}
            {row.find.clone().map(|href| {
                // A step to the catalog, as a link would take it (R21).
                let find = move |_| {
                    if let Some(going) = Pending::expect() {
                        going.go(&href, NavigateOptions::default());
                    }
                };
                view! { <button class="mini" type="button" on:click=find>"Modul finden"</button> }
            })}
            <button class="icon-btn" id=dom type="button" aria-label="Platzhalter entfernen" title="Entfernen" on:click=remove>
                <Icon name="x"/>
            </button>
        </div>
    }
}

/// The events that only `id` links among the semester's planned modules (`others`, `id` among
/// them), teaching and exams: what the semester hid or chose of them goes with the module
/// (`PlanDoc::unplan`). The same answers as the semester's own page asked, so from the visit's
/// cache where it was shown; nothing where the catalog cannot say.
fn own_events(source: Option<&Source>, semester: SemesterKey, others: &[String], id: &str) -> Vec<u32> {
    let Some(source) = source else { return Vec::new() };
    let key = semester.key();
    let rows = source.run(|db| {
        let mut rows = queries::modules_schedule(db, others, &key)?;
        rows.extend(queries::modules_exams(db, others, &key)?);
        Ok(rows)
    });
    let mut linked: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let rows = rows.unwrap_or_default();
    for row in &rows {
        linked.entry(row.date.event_id.as_str()).or_default().insert(row.module_id.as_str());
    }
    linked.into_iter().filter(|(_, modules)| modules.len() == 1 && modules.contains(id)).filter_map(|(event, _)| event.parse().ok()).collect()
}

// ---------- what the Übersicht shows ----------

/// The Übersicht as plain values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Overview {
    /// „150 von 180 LP geplant", „150 LP geplant".
    head: String,
    /// „13 Module · 10 Platzhalter".
    sum: String,
    terms: Vec<Term>,
}

/// One semester of the Übersicht.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Term {
    key: SemesterKey,
    /// „1. FS · WiSe 2026/27", or the semester alone before the Studienbeginn or without one.
    label: String,
    now: bool,
    /// „30 LP": the modules and the placeholders of this semester alone that nothing fills yet.
    lp: Option<String>,
    /// „Woche →": the semester's Regelwoche.
    week: String,
    lines: Vec<Line>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Line {
    Module(ModuleLine),
    Placeholder(PlaceholderRow),
}

/// What a line of a semester is, whatever it shows: a module stands once in a semester, as
/// itself or under the placeholder it fills.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum LineId {
    Module(String),
    Placeholder(u32),
}

impl Line {
    fn id(&self) -> LineId {
        match self {
            Line::Module(module) => LineId::Module(module.id.clone()),
            Line::Placeholder(row) => LineId::Placeholder(row.pid),
        }
    }
}

impl LineId {
    /// The element id of the line's „Entfernen" in semester `key`, for the focus to go to.
    fn dom(&self, key: SemesterKey) -> String {
        match self {
            LineId::Module(id) => format!("sp-x-{}-{id}", key.key()),
            LineId::Placeholder(pid) => format!("sp-x-p{pid}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ModuleLine {
    id: String,
    semester: SemesterKey,
    /// `None`: not in the catalog.
    title: Option<String>,
    lp: Option<String>,
    /// The kind „Mein Studiengang"'s plan states for it: its code and label („Pflicht").
    kind: Option<(String, String)>,
    /// „für Anwendungsfach": the placeholder of another semester it fills.
    note: Option<String>,
    /// „nur im Sommer": the catalog offers it in the other half of the year.
    season: Option<&'static str>,
    /// The module beside the plan.
    href: String,
    /// It fills the placeholder above it.
    filler: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct PlaceholderRow {
    pid: u32,
    /// „≥ 6 LP", in bold.
    credits: Option<String>,
    /// The rest of the catalog's line: „Anwendungsfach: „Mathematik“, … oder „Physik“".
    text: String,
    /// „5.–6. FS" for a row over several semesters.
    span: Option<String>,
    /// The plan no longer has the row („nicht mehr im Regelstudienplan").
    changed: bool,
    /// „6 von 6 LP", „12 von 10–24 LP", once modules fill it.
    filled: Option<String>,
    /// „Modul finden", while it is not filled.
    find: Option<String>,
}

/// What the catalog knows of a placeholder's program: its plans, or, where the program is gone
/// from the snapshot, the newest PO of its family.
#[derive(Clone, Debug, PartialEq)]
struct Known {
    source: Option<PlanSource>,
    newest: Option<Program>,
}

fn known_of(source: Option<&Source>, program_id: &str) -> Known {
    let Some(source) = source else { return Known { source: None, newest: None } };
    let plans = source.run(|db| pages::plan_source(db, program_id)).ok().flatten();
    let newest = match plans {
        Some(_) => None,
        None => source.run(|db| pages::my_program(db, program_id)).ok().flatten().filter(|info| !info.exact).map(|info| info.program),
    };
    Known { source: plans, newest }
}

/// What „Mein Studiengang"'s plan says for the Übersicht: what it comes to (where it is one
/// number), how many semesters it has, and the kind it states for each of its modules.
#[derive(Clone, Debug, Default, PartialEq)]
struct Goal {
    total: Option<f64>,
    semesters: Option<u8>,
    kinds: BTreeMap<String, Code<ModuleKind>>,
}

/// The plan of `source` stored as „Mein Studiengang"'s (its caption, else the only one), and the
/// page stored as its direction.
fn goal_of(source: &PlanSource, caption: Option<&str>, direction: Option<&str>) -> Goal {
    let only = match source.variants.as_slice() {
        [only] => Some(only),
        _ => None,
    };
    let Some(core) = caption.and_then(|caption| catalog::variants::variant_for(&source.variants, caption)).or(only) else {
        return Goal::default();
    };
    let at = source.variants.iter().position(|variant| std::ptr::eq(variant, core));
    let page = direction.and_then(|direction| {
        source
            .supplements
            .iter()
            .filter(|s| Some(s.core) == at)
            .filter_map(|s| source.variants.get(s.page))
            .find(|page| page.full.trim() == direction.trim())
    });
    let mut kinds = BTreeMap::new();
    for entry in core.entries.iter().chain(page.into_iter().flat_map(|page| page.entries.iter())) {
        if let (Some(id), Some(kind)) = (&entry.module_id, &entry.kind) {
            kinds.entry(id.clone()).or_insert_with(|| kind.clone());
        }
    }
    let exact = (core.credits - core.credits_max).abs() < 0.01 && core.credits > 0.0;
    Goal { total: exact.then_some(core.credits), semesters: u8::try_from(core.semesters).ok().filter(|n| *n > 0), kinds }
}

/// The modules of a plan, each once, in the order they stand in it.
fn module_ids(doc: &PlanDoc) -> Vec<String> {
    let mut seen = BTreeSet::new();
    doc.modules.iter().filter(|m| seen.insert(m.module_id.as_str())).map(|m| m.module_id.clone()).collect()
}

/// The lower end of credits as a plan states them („6", „10–24", „7,5").
fn lower_credits(text: &str) -> Option<f64> {
    let number: String = text.trim().chars().take_while(|c| c.is_ascii_digit() || *c == ',' || *c == '.').collect();
    number.replace(',', ".").parse::<f64>().ok().filter(|credits| credits.is_finite() && *credits >= 0.0)
}

/// How much of a placeholder its modules fill: „6 von 6 LP", „12 von 10–24 LP"; „6 LP" where the
/// plan states no credits for it.
fn filled_label(filled: f64, credits: Option<&str>) -> String {
    match credits.map(str::trim).filter(|credits| !credits.is_empty()) {
        Some(credits) => format!("{} von {credits} LP", format::number(filled)),
        None => format!("{} LP", format::number(filled)),
    }
}

/// The semesters the Übersicht lists: from the Studienbeginn or the plan's first semester,
/// whichever is earlier, to the end of „Mein Studiengang"'s plan, the plan's last semester or
/// the current one, whichever is later.
fn semesters(doc: &PlanDoc, start: Option<SemesterKey>, length: Option<u8>, current: Option<SemesterKey>) -> Vec<SemesterKey> {
    let held = doc.semesters();
    let end = start.zip(length).and_then(|(start, length)| start.plus(i32::from(length) - 1));
    let first = held.first().copied().into_iter().chain(start).min().or(current);
    let last = held.last().copied().into_iter().chain(end).chain(current).max();
    let (Some(first), Some(last)) = (first, last) else { return Vec::new() };
    std::iter::successors(Some(first), |key| key.plus(1)).take_while(|key| *key <= last).take(MOST_TERMS).collect()
}

/// Where „Modul finden" of a placeholder leads: the catalog's answer to its row (the areas it
/// means, the FÜS list, a module by its name: `variants::row_query`), in the half of the year of
/// its semester, fitting the plan of that semester, and planning into it (`fill=`). A row the
/// plan no longer has asks for the program's electives; a program gone from the snapshot for the
/// electives of the newest PO of its family, and without one for what fits the semester.
fn find_href(p: &Placeholder, known: Option<&Known>) -> String {
    let electives = |slug: &str| CatalogQuery {
        program: Some(ProgramScope { program_slug: slug.to_string(), kinds: vec![KindFilter::Stated(ModuleKind::Elective)], ..Default::default() }),
        ..Default::default()
    };
    let query = match known.and_then(|known| known.source.as_ref()) {
        Some(source) => match studyplan::resolve_placeholder(p, Some(&source.variants)) {
            Resolved::Row(variant, entry) => catalog::variants::row_query(&source.program.slug, variant, entry, &source.areas).0,
            Resolved::Changed | Resolved::Gone => electives(&source.program.slug),
        },
        None => known.and_then(|known| known.newest.as_ref()).map(|newest| electives(&newest.slug)).unwrap_or_default(),
    };
    let turnus = TurnusFilter { winter: p.semester.winter, summer: !p.semester.winter, ..Default::default() };
    let query = CatalogQuery { turnus, fits: Some(FitsFilter::all(&p.semester.key())), ..query };
    CatalogUrl { query, fill: Some(p.pid), ..Default::default() }.path()
}

/// A placeholder's line: the catalog's words for its row where the plan still has it
/// (`studyplan::placeholder_line`), its span, how much is filled and where to find more.
fn placeholder_row(p: &Placeholder, known: Option<&Known>, filled: Option<f64>) -> PlaceholderRow {
    let source = known.and_then(|known| known.source.as_ref());
    let resolved = studyplan::resolve_placeholder(p, source.map(|source| source.variants.as_slice()));
    let requirement = match (resolved, source) {
        (Resolved::Row(variant, entry), Some(source)) => Some(plan::requirement_of(entry, &variant.full, &source.areas, &variant.entries)),
        _ => None,
    };
    let line = studyplan::placeholder_line(p, requirement.as_ref());
    let text = studyplan::PlaceholderLine { credits: None, ..line.clone() }.text();
    let lower = p.credits.as_deref().and_then(lower_credits).unwrap_or(0.0);
    let complete = match filled {
        Some(filled) => lower <= 0.0 || filled + 0.01 >= lower,
        None => false,
    };
    PlaceholderRow {
        pid: p.pid,
        credits: line.credits,
        text,
        span: (p.span.0 != p.span.1).then(|| format!("{}.–{}. FS", p.span.0, p.span.1)),
        changed: resolved == Resolved::Changed,
        filled: filled.map(|filled| filled_label(filled, p.credits.as_deref())),
        find: (!complete).then(|| find_href(p, known)),
    }
}

/// „nur im Sommer" for a module the catalog offers in the summer only, planned into a winter;
/// „nur im Winter" the other way round.
fn off_season(row: Option<&CatalogRow>, key: SemesterKey) -> Option<&'static str> {
    match row.and_then(|row| row.turnus_season.as_ref()).and_then(Code::known) {
        Some(TurnusSeason::Summer) if key.winter => Some("nur im Sommer"),
        Some(TurnusSeason::Winter) if !key.winter => Some("nur im Winter"),
        _ => None,
    }
}

/// The Übersicht of a plan: the head, then the semesters (`semesters`), each with its modules in
/// plan order, then its placeholders with the modules of the semester that fill them under each.
/// A semester's credits are its modules' and its placeholders' that stand in it alone and that
/// nothing fills; the head's are those of every module (each once) and every placeholder nothing
/// fills. `page`: the semester of the page (`key_of`), which a module beside the plan is shown in
/// where it is planned there.
#[allow(clippy::too_many_arguments)]
fn overview(
    doc: &PlanDoc,
    rows: &BTreeMap<String, CatalogRow>,
    known: &BTreeMap<String, Known>,
    goal: Option<&Goal>,
    start: Option<SemesterKey>,
    current: Option<SemesterKey>,
    base: &StudyplanUrl,
    page: SemesterKey,
) -> Overview {
    let credits_of = |id: &str| rows.get(id).and_then(|row| row.credits);
    let filled_of = |pid: u32| {
        let fillers: BTreeSet<&str> = doc.fillers(pid).into_iter().map(|m| m.module_id.as_str()).collect();
        (!fillers.is_empty()).then(|| fillers.into_iter().filter_map(&credits_of).sum::<f64>())
    };
    let open_credits = |p: &Placeholder| if doc.fillers(p.pid).is_empty() { p.credits.as_deref().and_then(lower_credits).unwrap_or(0.0) } else { 0.0 };
    let module_line = |planned: &Planned, filler: bool| {
        let row = rows.get(&planned.module_id);
        let fills = planned.fills.and_then(|pid| doc.placeholders.iter().find(|p| p.pid == pid));
        // The module beside the plan shows it in the page's semester where it is planned there,
        // else in the one nearest to it (`aside.rs`). A module planned in more than one semester
        // names the row's in `sem` where that is another, so the row opens its own entry (its
        // Termine, „Entfernen"); every other row keeps the page's address, and opening it stays
        // a step beside the plan (R21).
        let elsewhere = planned.semester != page && doc.planned_in(&planned.module_id).len() > 1;
        let sem = if elsewhere { Some(planned.semester.key()) } else { base.sem.clone() };
        ModuleLine {
            id: planned.module_id.clone(),
            semester: planned.semester,
            title: row.map(|row| row.title.clone()),
            lp: row.and_then(|row| row.credits).map(|credits| format!("{} LP", format::number(credits))),
            kind: goal.and_then(|goal| goal.kinds.get(&planned.module_id)).map(|kind| (kind.code().to_string(), kind.label().to_string())),
            note: fills.filter(|_| !filler).map(|p| format!("für {}", p.name)),
            season: off_season(row, planned.semester),
            href: StudyplanUrl { sem, ..base.with_open(Some(planned.module_id.as_str()), None) }.path(),
            filler,
        }
    };

    let mut terms = Vec::new();
    for key in semesters(doc, start, goal.and_then(|goal| goal.semesters), current) {
        let placeholders = doc.placeholders_in(key);
        let here: BTreeSet<u32> = placeholders.iter().map(|p| p.pid).collect();
        let mut lines = Vec::new();
        // A module stands once in a semester (the store plans it so, and its line is known by
        // it): counted once, and listed once, as itself or under what it fills.
        let mut counted = BTreeSet::new();
        let mut listed = BTreeSet::new();
        let mut lp = 0.0;
        for planned in doc.modules.iter().filter(|m| m.semester == key) {
            if counted.insert(planned.module_id.as_str()) {
                lp += credits_of(&planned.module_id).unwrap_or(0.0);
            }
            // A module that fills a placeholder of this semester stands under it.
            if !planned.fills.is_some_and(|pid| here.contains(&pid)) && listed.insert(planned.module_id.as_str()) {
                lines.push(Line::Module(module_line(planned, false)));
            }
        }
        for p in placeholders {
            if p.span.0 == p.span.1 {
                lp += open_credits(p);
            }
            lines.push(Line::Placeholder(placeholder_row(p, known.get(&p.program_id), filled_of(p.pid))));
            for filler in doc.fillers(p.pid).into_iter().filter(|m| m.semester == key) {
                if listed.insert(filler.module_id.as_str()) {
                    lines.push(Line::Module(module_line(filler, true)));
                }
            }
        }
        let label = match start.and_then(|start| fachsemester(key, start)) {
            Some(fs) => format!("{fs}. FS · {}", key.label()),
            None => key.label(),
        };
        terms.push(Term {
            key,
            label,
            now: current == Some(key),
            lp: (lp > 0.0).then(|| format!("{} LP", format::number(lp))),
            week: semester_href(PlanView::Overview, Some(key)).unwrap_or_default(),
            lines,
        });
    }

    let ids = module_ids(doc);
    let planned = ids.iter().filter_map(|id| credits_of(id)).sum::<f64>() + doc.placeholders.iter().map(open_credits).sum::<f64>();
    let head = match goal.and_then(|goal| goal.total) {
        Some(total) => format!("{} von {} LP geplant", format::number(planned), format::number(total)),
        None => format!("{} LP geplant", format::number(planned)),
    };
    let mut sum = Vec::new();
    if !ids.is_empty() {
        sum.push(format::modules(i64::try_from(ids.len()).unwrap_or(i64::MAX)));
    }
    if !doc.placeholders.is_empty() {
        sum.push(format!("{} Platzhalter", doc.placeholders.len()));
    }
    Overview { head, sum: sum.join(" · "), terms }
}

// ---------- taking a Regelstudienplan over ----------

/// „Regelstudienplan übernehmen", while the address asks for it (`import=`). A new program or
/// plan in the address starts the panel afresh.
#[component]
pub(super) fn ImportPanel(ctx: PlanCtx) -> impl IntoView {
    let asked = Memo::new(move |_| ctx.url.with(|url| url.import.clone().map(|import| (import, url.variant))));
    move || asked.get().map(|(import, variant)| view! { <ImportForm ctx import variant/> })
}

/// The program's plans as the panel loads them.
#[derive(Clone, Debug, PartialEq)]
enum Loaded {
    /// No program picked yet.
    Nothing,
    Failed(DataError),
    /// The program is gone from the snapshot since it was picked.
    Missing,
    Plans(Box<PlanSource>),
}

/// What the student chose in the panel; `None` is the default for each.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Picks {
    core: Option<usize>,
    /// `Some(None)`: „offen".
    page: Option<Option<usize>>,
    start: Option<SemesterKey>,
    from_fs: Option<u8>,
    mine: Option<bool>,
}

/// A plan or a page to choose, as a chip: its place among the program's plans, its label as on
/// the program's page, its whole name (or its first modules) as the title, its credits.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Choice {
    index: usize,
    label: String,
    title: String,
    count: Option<String>,
}

/// A line of the import's preview: „1. FS · WiSe 2026/27 · 4 Module · 1 Platzhalter", and the
/// names of what it gets.
type PreviewLine = (String, String);

/// What an import adds („13 Module · 10 Platzhalter"), and what the plan holds of it already
/// („3 schon geplant").
type Total = (Option<String>, Option<String>);

/// The panel as plain values, for a program with plans.
#[derive(Clone, Debug, PartialEq)]
struct Form {
    /// The plans to take over (those that fill no row of another), when there are two or more.
    cores: Vec<Choice>,
    core: usize,
    /// The pages that fill the core plan's direction row.
    pages: Vec<Choice>,
    /// The chosen page (its index among the plans); `None`: „offen".
    page: Option<usize>,
    starts: Vec<SemesterKey>,
    start: SemesterKey,
    /// The last Fachsemester of the plan, and the one to start from.
    most: u8,
    from_fs: u8,
    /// One line per Fachsemester: „1. FS · WiSe 2026/27 · 4 Module · 1 Platzhalter" and the names.
    preview: Vec<PreviewLine>,
    /// „13 Module · 10 Platzhalter", and „3 schon geplant".
    total: Total,
    /// Whether „Übernehmen" adds anything.
    adds: bool,
    /// „Als meinen Studiengang setzen".
    mine: bool,
}

/// The program an address asks for: its slug, or „Mein Studiengang" (`mine`); and whether a slug
/// names none („Studiengang nicht gefunden.").
fn asked_program(import: &str, programs: &[Program], mine: Option<&str>) -> (Option<String>, bool) {
    if import == "mine" {
        return (mine.filter(|id| programs.iter().any(|program| program.id == *id)).map(str::to_string), false);
    }
    match programs.iter().find(|program| program.slug == import) {
        Some(program) => (Some(program.id.clone()), false),
        None => (None, true),
    }
}

/// The plans a Regelstudienplan is taken over as: every plan that fills no row of another.
fn cores(source: &PlanSource) -> Vec<usize> {
    (0..source.variants.len()).filter(|index| !source.supplements.iter().any(|s| s.page == *index)).collect()
}

/// The pages that fill a row of `core`, in the order of the document.
fn pages_of(source: &PlanSource, core: usize) -> Vec<&Supplement> {
    source.supplements.iter().filter(|s| s.core == core).collect()
}

/// The plan the panel starts with: the one „Mein Studiengang" stored (by its caption), else the
/// one the address names (`variant=`, the core of a page it names), else the first. `form_of`
/// hands in only one of the two.
fn default_core(source: &PlanSource, caption: Option<&str>, variant: Option<usize>) -> Option<usize> {
    let cores = cores(source);
    let stored = caption
        .and_then(|caption| catalog::variants::variant_for(&source.variants, caption))
        .and_then(|found| source.variants.iter().position(|variant| std::ptr::eq(variant, found)));
    let named = variant.and_then(|variant| variant.checked_sub(1)).map(|index| {
        source.supplements.iter().find(|s| s.page == index).map_or(index, |s| s.core)
    });
    stored.into_iter().chain(named).find(|index| cores.contains(index)).or_else(|| cores.first().copied())
}

/// The page the panel starts with for `core`: the direction „Mein Studiengang" stored, else the
/// page the address names, else „offen".
fn default_page(source: &PlanSource, core: usize, direction: Option<&str>, variant: Option<usize>) -> Option<usize> {
    let pages = pages_of(source, core);
    let stored = direction.and_then(|direction| {
        pages.iter().find(|s| source.variants.get(s.page).is_some_and(|page| page.full.trim() == direction.trim())).map(|s| s.page)
    });
    let named = variant.and_then(|variant| variant.checked_sub(1)).filter(|index| pages.iter().any(|s| s.page == *index));
    stored.or(named)
}

/// „Ab Fachsemester" to start with: the one the current semester is of that start, within the plan.
fn default_fs(current: SemesterKey, start: SemesterKey, most: u8) -> u8 {
    match fachsemester(current, start) {
        Some(fs) => fs.clamp(1, most.max(1)),
        None if start > current => 1,
        None => most.max(1),
    }
}

/// The semesters „Studienbeginn" offers: the current one, the twelve before it and the next, and
/// the one chosen outside them.
fn start_options(current: SemesterKey, chosen: SemesterKey) -> Vec<SemesterKey> {
    let around = (-STARTS_BEFORE..=1).filter_map(|n| current.plus(n));
    around.chain([chosen]).collect::<BTreeSet<_>>().into_iter().collect()
}

/// The preview of an import: a line per Fachsemester that gets something, with the names, and
/// the total with what the plan holds already.
fn preview(import: &Import) -> (Vec<PreviewLine>, Total) {
    let lines = import
        .by_fs
        .iter()
        .map(|fs| {
            let mut head = vec![format!("{}. FS", fs.fs), fs.semester.label()];
            if !fs.modules.is_empty() {
                head.push(format::modules(i64::try_from(fs.modules.len()).unwrap_or(i64::MAX)));
            }
            if !fs.placeholders.is_empty() {
                head.push(format!("{} Platzhalter", fs.placeholders.len()));
            }
            let names: Vec<&str> = fs.modules.iter().chain(&fs.placeholders).map(String::as_str).collect();
            (head.join(" · "), names.join(", "))
        })
        .collect();
    let mut adds = Vec::new();
    if !import.modules.is_empty() {
        adds.push(format::modules(i64::try_from(import.modules.len()).unwrap_or(i64::MAX)));
    }
    if !import.placeholders.is_empty() {
        adds.push(format!("{} Platzhalter", import.placeholders.len()));
    }
    let held = (import.skipped > 0).then(|| format!("{} schon geplant", import.skipped));
    ((lines), ((!adds.is_empty()).then(|| adds.join(" · ")), held))
}

/// The label of a plan's credits: „180 LP", „170–190 LP".
fn credits_of_plan(credits: f64, most: f64) -> String {
    if (most - credits).abs() < 0.01 {
        format!("{} LP", format::number(credits))
    } else {
        format!("{}–{} LP", format::number(credits), format::number(most))
    }
}

/// The panel for a program with plans: the defaults (`default_core`, …) where the student chose
/// nothing, and the import they make.
fn form_of(source: &PlanSource, doc: &PlanDoc, mine: &MineDoc, current: SemesterKey, variant: Option<usize>, picks: Picks) -> Option<Form> {
    // What „Mein Studiengang" stored of the plan counts only for that program, and only where the
    // address names no plan of it (`import=mine`, or another program picked in the panel). An
    // `import=<slug>` comes from the plan a program's page shows („In den Studienplan", `variant`
    // 1 where it names none), and that plan is the one meant, also on the student's own program.
    let own = mine.program.as_deref() == Some(source.program.id.as_str());
    let (caption, direction) = match (own, variant) {
        (true, None) => (mine.caption.as_deref(), mine.direction.as_deref()),
        _ => (None, None),
    };
    let core = picks.core.filter(|core| cores(source).contains(core)).or_else(|| default_core(source, caption, variant))?;
    let core_plan = source.variants.get(core)?;
    let pages = pages_of(source, core);
    let page = match picks.page {
        Some(page) => page.filter(|page| pages.iter().any(|s| s.page == *page)),
        None => default_page(source, core, direction, variant),
    };
    let filled_row = page.and_then(|page| pages.iter().find(|s| s.page == page)).map(|s| s.ord);
    let page_plan = page.and_then(|page| source.variants.get(page)).zip(filled_row);
    let start = picks.start.or(mine.start).unwrap_or_else(|| studyplan::intake_start(current, studyplan::intake_season(core_plan, &source.linked)));
    let most = u8::try_from(core_plan.semesters.clamp(1, 30)).unwrap_or(1);
    let from_fs = picks.from_fs.filter(|fs| (1..=most).contains(fs)).unwrap_or_else(|| default_fs(current, start, most));
    let import = studyplan::import(doc, &source.program.id, core_plan, page_plan, start, from_fs);
    let (preview, total) = preview(&import);

    let listed = cores(source);
    let mut cores: Vec<Choice> = match listed.len() {
        0 | 1 => Vec::new(),
        _ => listed
            .iter()
            .filter_map(|index| {
                let plan = source.variants.get(*index)?;
                Some(Choice { index: *index, label: plan.label.clone(), title: plan.full.clone(), count: Some(credits_of_plan(plan.credits, plan.credits_max)) })
            })
            .collect(),
    };
    // Credits tell plans apart only where they differ (a 90- and a 120-LP Master).
    if cores.windows(2).all(|pair| matches!(pair, [a, b] if a.count == b.count)) {
        cores.iter_mut().for_each(|choice| choice.count = None);
    }
    let pages = pages
        .iter()
        .filter_map(|s| {
            let plan = source.variants.get(s.page)?;
            // A page is told apart by what it holds: its first modules.
            let modules: Vec<&str> = plan.entries.iter().filter(|entry| entry.module_id.is_some()).map(|entry| plan::shown_name(&entry.module_name)).take(3).collect();
            Some(Choice { index: s.page, label: plan.label.clone(), title: modules.join(", "), count: None })
        })
        .collect();
    Some(Form {
        cores,
        core,
        pages,
        page,
        starts: start_options(current, start),
        start,
        most,
        from_fs,
        preview,
        adds: !import.modules.is_empty() || !import.placeholders.is_empty(),
        total,
        mine: picks.mine.unwrap_or(mine.program.is_none() || own),
    })
}

/// „Übernommen: 13 Module, 10 Platzhalter": what the import added, each part only where it
/// added any („Übernommen: 10 Platzhalter" when every module was planned already).
fn imported_note(modules: usize, placeholders: usize) -> String {
    let mut parts = Vec::new();
    if modules > 0 {
        parts.push(format::modules(i64::try_from(modules).unwrap_or(i64::MAX)));
    }
    if placeholders > 0 {
        parts.push(format!("{placeholders} Platzhalter"));
    }
    if parts.is_empty() {
        // Another tab planned it all between the preview and the write.
        parts.push("nichts".to_string());
    }
    format!("{IMPORTED}{}", parts.join(", "))
}

/// Opens the program picker for „Mein Studiengang" asked for and not set, so that typing finds
/// the program at once (A.6: „inf" + Enter). A click on its button opens it and puts the focus
/// in its search field. On a phone only the button takes the focus: an open picker brings up the
/// keyboard over a panel the student has not seen yet.
fn open_picker() {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        if nav::is_phone() {
            nav::focus_by_id(PROGRAM_ID);
            return;
        }
        let button = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id(PROGRAM_ID));
        if let Some(button) = button.and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()) {
            button.click();
        }
    }
}

/// Gives the focus to the button that was pressed where it is not there already (a script's
/// click; Safari and iOS do not focus a button on a click). The panel goes once the import is
/// written, and the program picker must not hold the focus then: its handlers would run on what
/// the panel took with it.
#[allow(unused_variables)]
fn hold_focus(ev: &leptos::ev::MouseEvent) {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        if let Some(button) = ev.current_target().and_then(|target| target.dyn_into::<web_sys::HtmlElement>().ok()) {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = button.focus_with_options(&options);
        }
    }
}

/// Seconds since 1970, for when the modules were planned; 0 outside the browser.
fn now_secs() -> u64 {
    #[cfg(feature = "csr")]
    {
        let millis = web_sys::js_sys::Date::now();
        if millis.is_finite() && millis > 0.0 {
            return (millis / 1000.0) as u64;
        }
    }
    0
}

#[component]
fn ImportForm(ctx: PlanCtx, import: String, variant: usize) -> impl IntoView {
    let source = ctx.source;
    let mine = ctx.mine;
    let programs = Memo::new(move |_| source.with_value(|source| source.as_ref().and_then(|source| source.run(|db| queries::programs(db)).ok())).unwrap_or_default());
    let stored = untrack(|| mine.and_then(|mine| mine.with(|doc| doc.program.clone())));
    let (asked, unknown) = programs.with_untracked(|programs| asked_program(&import, programs, stored.as_deref()));
    // The plan the address names (`variant=`) belongs to the program it names.
    let named = (import != "mine").then(|| asked.clone()).flatten();
    // The program the picker shows, and the one whose plans are loaded: a pick answers in the
    // picker at once, and its plans (a query) follow after the next frame (R21).
    let picked = RwSignal::new(asked.clone());
    let chosen = RwSignal::new(asked.clone());
    let picks = RwSignal::new(Picks::default());
    let busy = RwSignal::new(false);

    let loaded = Memo::new(move |_| {
        let Some(id) = chosen.get() else { return Loaded::Nothing };
        match source.with_value(|source| source.as_ref().map(|source| source.run(|db| pages::plan_source(db, &id)))) {
            None => Loaded::Failed(DataError { unavailable: true, message: "no data source was provided".to_string() }),
            Some(Err(error)) => Loaded::Failed(error),
            Some(Ok(None)) => Loaded::Missing,
            Some(Ok(Some(plans))) => Loaded::Plans(Box::new(plans)),
        }
    });
    let form = Memo::new(move |_| {
        let (current, picks) = (ctx.current.get()?, picks.get());
        let mine = ctx.mine.map(|mine| mine.with(Clone::clone)).unwrap_or_default();
        // The program from what was loaded for it, not from `picked`, which it follows (R16).
        loaded.with(|loaded| match loaded {
            Loaded::Plans(plans) => {
                let variant = (named.as_deref() == Some(plans.program.id.as_str())).then_some(variant);
                match ctx.plan {
                    Some(plan) => plan.with(|doc| form_of(plans, doc, &mine, current, variant, picks)),
                    None => form_of(plans, &PlanDoc::default(), &mine, current, variant, picks),
                }
            }
            _ => None,
        })
    });
    let ready = Memo::new(move |_| form.with(Option::is_some));
    let state = Memo::new(move |_| {
        loaded.with(|loaded| match loaded {
            Loaded::Nothing => State::Nothing,
            Loaded::Failed(error) => State::Failed(error.clone()),
            Loaded::Missing => State::NoPlan(None),
            Loaded::Plans(plans) if plans.variants.is_empty() => State::NoPlan(Some(plans.program.slug.clone())),
            Loaded::Plans(_) => State::Plans,
        })
    });

    // The programs to pick from, „Mein Studiengang" first under its own heading.
    let resolved = MineResolved::expect();
    let items = Signal::derive(move || {
        let mine = resolved.and_then(|resolved| resolved.0.get()).filter(|info| info.exact).map(|info| info.program);
        let item = |program: &Program| ComboItem::new(program.id.clone(), program.name.clone(), format!("{} · PO {}", program.degree(), program.po_year.map(|year| year.to_string()).unwrap_or_else(|| program.po_version.clone())), i64::from(program.is_latest_po));
        programs.with(|programs| mine.iter().map(|program| item(program).in_group("Mein Studiengang")).chain(programs.iter().map(item)).collect::<Vec<_>>())
    });
    let pick = Callback::new(move |id: Option<String>| {
        if id.is_some() && id != picked.get_untracked() {
            picked.set(id.clone());
            nav::after_paint(move || {
                let _ = chosen.try_set(id);
                // A plan, page or Fachsemester of another program means nothing here; the start
                // stays.
                let _ = picks.try_update(|picks| *picks = Picks { start: picks.start, ..Picks::default() });
            });
        }
    });
    // Until the plans of a pick are there, what stands below is the last program's.
    let loading = Memo::new(move |_| picked.with(|picked| chosen.with(|chosen| picked != chosen)));
    // „Mein Studiengang" was asked for and is not set: the picker is where to begin.
    if import == "mine" && asked.is_none() {
        Effect::new(move |_| request_animation_frame(open_picker));
    }
    let lost = Memo::new(move |_| unknown && picked.with(Option::is_none));

    let take = move |ev: leptos::ev::MouseEvent| {
        // What stands below is still the last program's while a pick's plans are on their way.
        if busy.get_untracked() || loading.get_untracked() {
            return;
        }
        let Some(form) = form.get_untracked().filter(|form| form.adds) else { return };
        let taken = loaded.with_untracked(|loaded| match loaded {
            Loaded::Plans(plans) => {
                let core = plans.variants.get(form.core)?.clone();
                let page = form.page.and_then(|page| {
                    let ord = plans.supplements.iter().find(|s| s.core == form.core && s.page == page)?.ord;
                    Some((plans.variants.get(page)?.clone(), ord))
                });
                Some((plans.program.clone(), core, page))
            }
            _ => None,
        });
        let Some((program, core, page)) = taken else { return };
        busy.set(true);
        hold_focus(&ev);
        let (start, from_fs, keep) = (form.start, form.from_fs, form.mine);
        let (plan, undo, going) = (ctx.plan, ctx.undo, Pending::expect());
        let to = ctx.url.with_untracked(|url| url.without_import().with_view(PlanView::Overview).with_open(None, None).path());
        // After the next frame: the button has answered, then the plan is written once (what it
        // holds meanwhile is left out, so nothing comes twice) and the Übersicht shows it.
        nav::after_paint(move || {
            if let Some(plan) = plan {
                let note = plan.update(|doc| {
                    let before = doc.clone();
                    let import = studyplan::import(doc, &program.id, &core, page.as_ref().map(|(page, ord)| (page, *ord)), start, from_fs);
                    let (modules, placeholders) = doc.apply(&import, now_secs());
                    (imported_note(modules, placeholders), before)
                });
                let _ = undo.try_set(Some(note));
            }
            // The Fachsemester follow the Studienbeginn the plan was placed by.
            if let Some(mine) = mine {
                mine.set_start(Some(start));
                if keep {
                    mine.set_program(&program.id, &program_name(&program), &core.full, page.as_ref().map(|(page, _)| page.full.as_str()));
                }
            }
            if let Some(going) = going {
                going.go(&to, NavigateOptions { replace: true, ..Default::default() });
            }
        });
    };

    view! {
        <section class="sp-import" aria-label="Regelstudienplan übernehmen" aria-busy=move || loading.get().then_some("true")>
            <h2>"Regelstudienplan übernehmen"</h2>
            {move || lost.get().then(|| view! { <p class="note quiet"><span>"Studiengang nicht gefunden."</span></p> })}
            <div class="sp-field">
                <span>"Studiengang"</span>
                <Combobox
                    id=PROGRAM_ID
                    label="Studiengang"
                    placeholder="Studiengang wählen"
                    search_placeholder="Studiengang suchen"
                    icon="graduation-cap"
                    min_width=480.0
                    items
                    selected=Signal::derive(move || picked.get())
                    on_select=pick
                    clearable=false
                />
            </div>
            {move || match state.get() {
                State::Nothing => ().into_any(),
                State::Failed(error) => view! { <ErrorState error/> }.into_any(),
                State::NoPlan(slug) => {
                    let href = slug.map(|slug| {
                        let program = ProgramScope { program_slug: slug, ..Default::default() };
                        CatalogUrl { query: CatalogQuery { program: Some(program), ..Default::default() }, ..Default::default() }.path()
                    });
                    view! {
                        <p class="hint">"Für diesen Studiengang liegt kein Regelstudienplan vor."</p>
                        {href.map(|href| view! { <a class="btn secondary" href=href>"Module im Katalog"</a> })}
                    }
                    .into_any()
                }
                // Without the snapshot's current semester there is nothing to count from.
                State::Plans => view! { {move || ready.get().then(|| view! { <ImportChoices form picks busy take/> })} }.into_any(),
            }}
        </section>
    }
}

/// What the panel shows under the program.
#[derive(Clone, Debug, PartialEq)]
enum State {
    Nothing,
    Failed(DataError),
    /// No plan: the program's slug for „Module im Katalog", where it is still there.
    NoPlan(Option<String>),
    Plans,
}

/// The choices of a program with plans, the preview, and „Übernehmen". Each control reads a memo
/// of its own (R5).
#[component]
fn ImportChoices(form: Memo<Option<Form>>, picks: RwSignal<Picks>, busy: RwSignal<bool>, take: impl Fn(leptos::ev::MouseEvent) + Copy + Send + Sync + 'static) -> impl IntoView {
    let cores = Memo::new(move |_| form.with(|form| form.as_ref().map(|form| form.cores.clone()).unwrap_or_default()));
    let pages = Memo::new(move |_| form.with(|form| form.as_ref().map(|form| form.pages.clone()).unwrap_or_default()));
    let core = Memo::new(move |_| form.with(|form| form.as_ref().map(|form| form.core)));
    let page = Memo::new(move |_| form.with(|form| form.as_ref().and_then(|form| form.page)));
    let starts = Memo::new(move |_| form.with(|form| form.as_ref().map(|form| form.starts.clone()).unwrap_or_default()));
    let start = Memo::new(move |_| form.with(|form| form.as_ref().map(|form| form.start)));
    let semesters = Memo::new(move |_| form.with(|form| form.as_ref().map(|form| (1..=form.most).collect::<Vec<u8>>()).unwrap_or_default()));
    let from_fs = Memo::new(move |_| form.with(|form| form.as_ref().map(|form| form.from_fs)));
    let lines = Memo::new(move |_| form.with(|form| form.as_ref().map(|form| form.preview.clone()).unwrap_or_default()));
    let total = Memo::new(move |_| form.with(|form| form.as_ref().map(|form| form.total.clone()).unwrap_or_default()));
    let adds = Memo::new(move |_| form.with(|form| form.as_ref().is_some_and(|form| form.adds)));
    let mine = Memo::new(move |_| form.with(|form| form.as_ref().is_some_and(|form| form.mine)));

    let pick_core = move |index: usize| picks.update(|picks| *picks = Picks { core: Some(index), page: None, from_fs: None, ..*picks });
    let pick_page = move |index: Option<usize>| picks.update(|picks| picks.page = Some(index));
    let pick_start = move |ev: leptos::ev::Event| {
        if let Some(start) = SemesterKey::parse(&event_target_value(&ev)) {
            picks.update(|picks| *picks = Picks { start: Some(start), from_fs: None, ..*picks });
        }
    };

    view! {
        {move || {
            (!cores.with(Vec::is_empty)).then(|| view! {
                <div class="sp-field">
                    <span>"Studienrichtung"</span>
                    <p class="chip-links" role="radiogroup" aria-label="Studienrichtung">
                        <For
                            each=move || cores.get()
                            key=|choice| choice.clone()
                            children=move |choice: Choice| {
                                let index = choice.index;
                                let on = Memo::new(move |_| core.get() == Some(index));
                                chip(choice, on, move || pick_core(index))
                            }
                        />
                    </p>
                </div>
            })
        }}
        {move || {
            (!pages.with(Vec::is_empty)).then(|| view! {
                <div class="sp-field">
                    <span>"Richtung"</span>
                    <p class="chip-links" role="radiogroup" aria-label="Richtung">
                        <For
                            each=move || pages.get()
                            key=|choice| choice.clone()
                            children=move |choice: Choice| {
                                let index = choice.index;
                                let on = Memo::new(move |_| page.get() == Some(index));
                                chip(choice, on, move || pick_page(Some(index)))
                            }
                        />
                        {
                            let open = Choice { index: usize::MAX, label: "offen".to_string(), title: "Die Zeile bleibt ein Platzhalter.".to_string(), count: None };
                            let on = Memo::new(move |_| page.with(Option::is_none));
                            chip(open, on, move || pick_page(None))
                        }
                    </p>
                </div>
            })
        }}
        <label class="sp-field">
            <span>"Studienbeginn"</span>
            <select prop:value=move || start.get().map(SemesterKey::key).unwrap_or_default() on:change=pick_start>
                <For
                    each=move || starts.get()
                    key=|key| *key
                    children=move |key: SemesterKey| view! { <option value=key.key() selected=move || start.get() == Some(key)>{key.label()}</option> }
                />
            </select>
        </label>
        <div class="sp-field">
            <span>"Ab Fachsemester"</span>
            <div class="seg" role="radiogroup" aria-label="Ab Fachsemester">
                <For
                    each=move || semesters.get()
                    key=|fs| *fs
                    children=move |fs: u8| {
                        let on = Memo::new(move |_| from_fs.get() == Some(fs));
                        view! {
                            <button type="button" role="radio" aria-checked=move || if on.get() { "true" } else { "false" } on:click=move |_| picks.update(|picks| picks.from_fs = Some(fs))>
                                {fs.to_string()}
                            </button>
                        }
                    }
                />
            </div>
        </div>
        <div class="sp-preview" aria-live="polite">
            <For each=move || lines.get() key=|line| line.clone() children=|(head, names): (String, String)| view! { <p>{head}<small>{names}</small></p> }/>
            <p>
                {move || {
                    let (adds, held) = total.get();
                    let lead = adds.clone().map(|adds| view! { <b>{adds}</b> });
                    let gap = (adds.is_some() && held.is_some()).then_some(" · ");
                    let none = (adds.is_none() && held.is_none()).then_some("Nichts zu übernehmen");
                    view! { {lead}{gap}{held}{none} }
                }}
            </p>
        </div>
        // Nothing to add, nothing to take over: the preview says what the plan holds already.
        {move || {
            adds.get().then(|| view! {
                <label class="check">
                    <input type="checkbox" prop:checked=move || mine.get() on:change=move |ev| picks.update(|picks| picks.mine = Some(event_target_checked(&ev)))/>
                    <span>"Als meinen Studiengang setzen"</span>
                </label>
                <button class="btn primary" type="button" aria-busy=move || busy.get().then_some("true") on:click=take>
                    "Übernehmen"
                </button>
            })
        }}
    }
}

/// A chip of a single choice: its label, credits where it has them, and the whole name as its
/// title.
fn chip(choice: Choice, on: Memo<bool>, pick: impl Fn() + 'static) -> impl IntoView {
    view! {
        <button
            class="chip"
            type="button"
            role="radio"
            aria-checked=move || if on.get() { "true" } else { "false" }
            data-state=move || if on.get() { "with" } else { "off" }
            title=(!choice.title.is_empty()).then(|| choice.title.clone())
            on:click=move |_| pick()
        >
            <span class="chip-label">{choice.label.clone()}</span>
            {choice.count.clone().map(|count| view! { <span class="chip-count num">{count}</span> })}
        </button>
    }
}

#[cfg(test)]
mod tests {
    use catalog::labels::OfferStatus;
    use catalog::rows_detail::PlanEntry;
    use catalog::studyplan::ImportFs;
    use catalog::variants::plan_variants;

    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    fn entry(ord: i64, module: Option<&str>, name: &str, span: (i64, i64), credits: f64, kind: Option<&str>, plan: &str) -> PlanEntry {
        PlanEntry {
            ord,
            module_id: module.map(str::to_string),
            module_name: name.to_string(),
            semester: (span.0 == span.1).then_some(span.0),
            start_semester: (span.0 != span.1).then_some(span.0),
            end_semester: (span.0 != span.1).then_some(span.1),
            semester_span: None,
            credits: Some(credits),
            min_credits: None,
            max_credits: None,
            kind: kind.map(Code::parse),
            kind_raw: None,
            study_section: None,
            subject_area: None,
            specialization: (!plan.is_empty()).then(|| plan.to_string()),
            catalog_title: None,
            credits_differ_from_catalog: false,
            source_page: None,
        }
    }

    fn program(id: &str, slug: &str) -> Program {
        Program {
            id: id.into(),
            slug: slug.into(),
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

    fn row(id: &str, title: &str, credits: f64, turnus: Option<&str>) -> CatalogRow {
        CatalogRow {
            id: id.into(),
            title: title.into(),
            title_de: None,
            title_en: None,
            credits: Some(credits),
            turnus_season: turnus.map(Code::parse),
            turnus_parity: None,
            offer_status: Code::Known(OfferStatus::Active),
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

    fn placeholder(pid: u32, semester: &str, ord: i64, span: (u8, u8), credits: &str, name: &str) -> Placeholder {
        Placeholder {
            pid,
            semester: key(semester),
            program_id: "079-82-2008".into(),
            ord,
            span,
            credits: Some(credits.into()),
            kind: Some("elective".into()),
            caption: String::new(),
            name: name.into(),
        }
    }

    /// Informatik as far as these tests need it: two modules in FS 1, an elective row in FS 1, a
    /// FÜS row in FS 2, a budget row over FS 5–6.
    fn informatik() -> PlanSource {
        let entries = vec![
            entry(1, Some("12104"), "Entwicklung von Softwaresystemen", (1, 1), 8.0, Some("compulsory"), ""),
            entry(2, Some("11112"), "Mathematik IT-1", (1, 1), 8.0, Some("compulsory"), ""),
            entry(3, None, "Wahlpflichtmodul", (1, 1), 6.0, Some("elective"), ""),
            entry(4, None, "Fachübergreifendes Studium", (2, 2), 6.0, Some("fues"), ""),
            entry(5, None, "Komplex Praktische Informatik", (5, 6), 12.0, Some("elective"), ""),
        ];
        let variants = plan_variants(&entries, &[]);
        PlanSource { program: program("079-82-2008", "bachelor-informatik-2008"), supplements: Vec::new(), variants, areas: Vec::new(), linked: Vec::new() }
    }

    #[test]
    fn the_import_preview_says_what_each_fachsemester_gets() {
        let import = Import {
            modules: vec![(key("2026W"), "12104".into()), (key("2026W"), "11112".into())],
            placeholders: vec![placeholder(0, "2026W", 3, (1, 1), "6", "Wahlpflichtmodul")],
            skipped: 3,
            by_fs: vec![ImportFs { fs: 1, semester: key("2026W"), modules: vec!["Entwicklung von Softwaresystemen".into(), "Mathematik IT-1".into()], placeholders: vec!["Wahlpflichtmodul".into()] }],
        };
        let (lines, total) = preview(&import);
        assert_eq!(lines, vec![("1. FS · WiSe 2026/27 · 2 Module · 1 Platzhalter".to_string(), "Entwicklung von Softwaresystemen, Mathematik IT-1, Wahlpflichtmodul".to_string())]);
        assert_eq!(total, (Some("2 Module · 1 Platzhalter".to_string()), Some("3 schon geplant".to_string())));
        // Everything planned already: only what the plan holds.
        let again = Import { modules: Vec::new(), placeholders: Vec::new(), skipped: 23, by_fs: Vec::new() };
        assert_eq!(preview(&again), (Vec::new(), (None, Some("23 schon geplant".to_string()))));
    }

    #[test]
    fn the_form_starts_where_the_study_is() {
        let source = informatik();
        let mine = MineDoc::default();
        let form = form_of(&source, &PlanDoc::default(), &mine, key("2026W"), None, Picks::default()).unwrap();
        // One plan: no chips; no page; a start of the current semester (nothing tells the season
        // here), FS 1 of 6, everything new, and „Mein Studiengang" unset: set it.
        assert!(form.cores.is_empty() && form.pages.is_empty() && form.page.is_none());
        assert_eq!((form.start, form.from_fs, form.most, form.adds, form.mine), (key("2026W"), 1, 6, true, true));
        assert_eq!(form.total.0.as_deref(), Some("2 Module · 3 Platzhalter"));
        // A study begun a year ago starts at FS 3, and FS 1 and 2 are left out.
        let earlier = MineDoc { start: Some(key("2025W")), ..MineDoc::default() };
        let form = form_of(&source, &PlanDoc::default(), &earlier, key("2026W"), None, Picks::default()).unwrap();
        assert_eq!((form.from_fs, form.total.0.as_deref()), (3, Some("1 Platzhalter")));
        assert_eq!(form.preview.first().map(|line| line.0.as_str()), Some("5. FS · WiSe 2027/28 · 1 Platzhalter"));
        // Another program as „Mein Studiengang": not checked.
        let other = MineDoc { program: Some("048-82-2022".into()), ..MineDoc::default() };
        assert!(!form_of(&source, &PlanDoc::default(), &other, key("2026W"), None, Picks::default()).unwrap().mine);
        assert_eq!(default_fs(key("2026W"), key("2027S"), 6), 1);
        assert_eq!(default_fs(key("2026W"), key("2015W"), 6), 6);
    }

    #[test]
    fn a_budget_says_how_much_of_it_is_filled() {
        assert_eq!(filled_label(12.0, Some("10–24")), "12 von 10–24 LP");
        assert_eq!(filled_label(6.0, Some("6")), "6 von 6 LP");
        assert_eq!(filled_label(7.5, None), "7,5 LP");
        assert_eq!(lower_credits("10–24"), Some(10.0));
        assert_eq!(lower_credits("7,5"), Some(7.5));
        assert_eq!(lower_credits("viel"), None);
    }

    fn doc() -> PlanDoc {
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key("2026W"), "12104", 1, None));
        assert!(doc.plan(key("2026W"), "11112", 1, None));
        doc.placeholders.push(placeholder(1, "2026W", 3, (1, 1), "6", "Wahlpflichtmodul"));
        doc.placeholders.push(placeholder(2, "2028W", 5, (5, 6), "12", "Komplex Praktische Informatik"));
        doc
    }

    fn rows() -> BTreeMap<String, CatalogRow> {
        [row("12104", "Entwicklung von Softwaresystemen", 8.0, Some("winter")), row("11112", "Mathematik IT-1", 8.0, Some("summer")), row("12330", "Datenbanken", 6.0, None)]
            .into_iter()
            .map(|row| (row.id.clone(), row))
            .collect()
    }

    #[test]
    fn a_placeholder_over_several_semesters_counts_in_none_of_them() {
        let doc = doc();
        let known: BTreeMap<String, Known> = [("079-82-2008".to_string(), Known { source: Some(informatik()), newest: None })].into_iter().collect();
        let goal = goal_of(&informatik(), Some(""), None);
        let shown = overview(&doc, &rows(), &known, Some(&goal), Some(key("2026W")), Some(key("2026W")), &StudyplanUrl::default(), key("2026W"));
        // 16 of the modules, 6 and 12 of the rows nothing fills; the plan's rows come to 40.
        assert_eq!((shown.head.as_str(), shown.sum.as_str()), ("34 von 40 LP geplant", "2 Module · 2 Platzhalter"));
        let heads: Vec<(String, Option<String>, bool)> = shown.terms.iter().map(|term| (term.label.clone(), term.lp.clone(), term.now)).collect();
        assert_eq!(
            heads,
            vec![
                ("1. FS · WiSe 2026/27".to_string(), Some("22 LP".to_string()), true),
                ("2. FS · SoSe 2027".to_string(), None, false),
                ("3. FS · WiSe 2027/28".to_string(), None, false),
                ("4. FS · SoSe 2028".to_string(), None, false),
                // The row over FS 5–6 stands here and counts in no semester.
                ("5. FS · WiSe 2028/29".to_string(), None, false),
                ("6. FS · SoSe 2029".to_string(), None, false),
            ]
        );
        let Some(Term { lines, .. }) = shown.terms.first() else { panic!("a first semester") };
        let Some(Line::Module(first)) = lines.first() else { panic!("a module first") };
        assert_eq!((first.kind.clone(), first.season, first.href.as_str()), (Some(("compulsory".to_string(), "Pflicht".to_string())), None, "/studyplan?open=12104"));
        let Some(Line::Module(second)) = lines.get(1) else { panic!("a second module") };
        assert_eq!(second.season, Some("nur im Sommer"));
        let Some(Term { lines, .. }) = shown.terms.get(4) else { panic!("a fifth semester") };
        let Some(Line::Placeholder(budget)) = lines.first() else { panic!("the budget row") };
        assert_eq!((budget.span.as_deref(), budget.filled.as_ref(), budget.find.is_some()), (Some("5.–6. FS"), None, true));

        // Datenbanken fills the elective row: it stands under it, and the row is filled.
        let mut filled = doc.clone();
        assert!(filled.plan(key("2026W"), "12330", 2, Some(1)));
        let shown = overview(&filled, &rows(), &known, Some(&goal), Some(key("2026W")), Some(key("2026W")), &StudyplanUrl::default(), key("2026W"));
        let Some(term @ Term { lines, lp, .. }) = shown.terms.first() else { panic!("a first semester") };
        assert_eq!(lp.as_deref(), Some("22 LP"));
        let kinds: Vec<String> = lines
            .iter()
            .map(|line| match line {
                Line::Module(m) => format!("{}{}", if m.filler { "  " } else { "" }, m.id),
                Line::Placeholder(p) => format!("p{} {}", p.pid, p.filled.clone().unwrap_or_default()),
            })
            .collect();
        assert_eq!(kinds, vec!["12104", "11112", "p1 6 von 6 LP", "  12330"]);
        let Some(Line::Placeholder(row)) = lines.get(2) else { panic!("the row") };
        assert!(row.find.is_none());
        assert_eq!(shown.head, "34 von 40 LP geplant");

        // Taken out, a line hands the focus to the next line's „Entfernen", the last one to the
        // line before it; a placeholder skips the modules that filled it (they move up).
        let module = |id: &str| LineId::Module(id.to_string());
        assert_eq!(focus_after(Some(term), &module("12104")), "sp-x-2026W-11112");
        assert_eq!(focus_after(Some(term), &module("12330")), "sp-x-p1");
        assert_eq!(focus_after(Some(term), &LineId::Placeholder(1)), "sp-x-2026W-11112");
        let alone = Term { lines: lines.iter().take(1).cloned().collect(), ..term.clone() };
        assert_eq!(focus_after(Some(&alone), &module("12104")), "sp-add-2026W");
        assert_eq!(focus_after(None, &module("12104")), "");
    }

    #[test]
    fn a_module_planned_twice_opens_the_semester_of_its_row() {
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key("2026W"), "12104", 1, None));
        assert!(doc.plan(key("2027S"), "12104", 1, None));
        assert!(doc.plan(key("2027S"), "11112", 1, None));
        let known = BTreeMap::new();
        let hrefs = |page: &str, base: &StudyplanUrl| {
            let shown = overview(&doc, &rows(), &known, None, Some(key("2026W")), Some(key("2026W")), base, key(page));
            shown.terms.iter().flat_map(|term| term.lines.iter()).filter_map(|line| match line {
                Line::Module(m) => Some(format!("{} {}", m.semester.key(), m.href)),
                Line::Placeholder(_) => None,
            }).collect::<Vec<_>>()
        };
        let overview_url = StudyplanUrl { view: PlanView::Overview, ..StudyplanUrl::default() };
        // The page shows WiSe: its row keeps the page's address, the SoSe row names its semester;
        // a module planned once opens where it is planned without it.
        assert_eq!(
            hrefs("2026W", &overview_url),
            vec!["2026W /studyplan?view=all&open=12104", "2027S /studyplan?sem=2027S&view=all&open=12104", "2027S /studyplan?view=all&open=11112"]
        );
        // After such a click the page is of SoSe: now the WiSe row names its own.
        let sose = StudyplanUrl { sem: Some("2027S".to_string()), ..overview_url.clone() };
        assert_eq!(
            hrefs("2027S", &sose),
            vec!["2026W /studyplan?sem=2026W&view=all&open=12104", "2027S /studyplan?sem=2027S&view=all&open=12104", "2027S /studyplan?sem=2027S&view=all&open=11112"]
        );
    }

    #[test]
    fn the_note_names_only_what_was_added() {
        assert_eq!(imported_note(13, 10), "Übernommen: 13 Module, 10 Platzhalter");
        assert_eq!(imported_note(0, 10), "Übernommen: 10 Platzhalter");
        assert_eq!(imported_note(1, 0), "Übernommen: 1 Modul");
        assert_eq!(imported_note(0, 0), "Übernommen: nichts");
    }

    #[test]
    fn modul_finden_asks_the_catalog_what_the_row_means() {
        let known = Known { source: Some(informatik()), newest: None };
        let fues = Placeholder { kind: Some("fues".into()), ..placeholder(3, "2027S", 4, (2, 2), "6", "Fachübergreifendes Studium") };
        assert_eq!(find_href(&fues, Some(&known)), "/catalog?program=bachelor-informatik-2008&list=fues&turnus=summer&fits=2027S&fill=p3");
        // A row the plan no longer has: the program's electives.
        let renamed = placeholder(1, "2026W", 3, (1, 1), "6", "Wahlpflicht Informatik (alt)");
        assert!(matches!(studyplan::resolve_placeholder(&renamed, Some(&informatik().variants)), Resolved::Changed));
        assert_eq!(find_href(&renamed, Some(&known)), "/catalog?program=bachelor-informatik-2008&kind=elective&turnus=winter&fits=2026W&fill=p1");
        // The program is gone: the newest PO of its family, else only what fits the semester.
        let gone = Known { source: None, newest: Some(program("079-82-2022", "bachelor-informatik-2022")) };
        assert_eq!(find_href(&renamed, Some(&gone)), "/catalog?program=bachelor-informatik-2022&kind=elective&turnus=winter&fits=2026W&fill=p1");
        let nothing = Known { source: None, newest: None };
        assert_eq!(find_href(&renamed, Some(&nothing)), "/catalog?turnus=winter&fits=2026W&fill=p1");
        // And the line says what the plan says.
        let row = placeholder_row(&fues, Some(&known), None);
        assert_eq!((row.credits.as_deref(), row.text.as_str(), row.changed), (Some("≥\u{a0}6\u{a0}LP"), "Fachübergreifendes Studium", false));
        assert!(placeholder_row(&renamed, Some(&known), None).changed);
        assert!(!placeholder_row(&renamed, Some(&gone), None).changed);
    }

    #[test]
    fn the_panel_offers_the_plans_and_the_pages_that_fill_them() {
        // Two plans of one program, of equal credits: chips by what tells them apart, no credits.
        let first = "Regelstudienplan der Studienrichtungen MIT und EET im grundständigen Studium";
        let second = "Regelstudienplan der Studienrichtungen PA und IoT im grundständigen Studium";
        let entries = vec![entry(1, Some("12104"), "Modul A", (1, 1), 30.0, None, first), entry(2, Some("11112"), "Modul B", (1, 1), 30.0, None, second)];
        let two = PlanSource { program: program("048-82-2022", "bachelor-elektrotechnik-2022"), supplements: Vec::new(), variants: plan_variants(&entries, &[]), areas: Vec::new(), linked: Vec::new() };
        let form = form_of(&two, &PlanDoc::default(), &MineDoc::default(), key("2026W"), None, Picks::default()).unwrap();
        let chips: Vec<(&str, Option<&str>)> = form.cores.iter().map(|choice| (choice.label.as_str(), choice.count.as_deref())).collect();
        assert_eq!((chips, form.core), (vec![("MIT und EET", None), ("PA und IoT", None)], 0));
        // „Mein Studiengang" stored the second. The address of a program's page names the plan it
        // showed, the first (`import=<slug>`): that one. `import=mine` names none: the stored one.
        let mine = MineDoc { program: Some("048-82-2022".into()), caption: Some(second.into()), ..MineDoc::default() };
        assert_eq!(form_of(&two, &PlanDoc::default(), &mine, key("2026W"), Some(1), Picks::default()).unwrap().core, 0);
        assert_eq!(form_of(&two, &PlanDoc::default(), &mine, key("2026W"), None, Picks::default()).unwrap().core, 1);
        assert_eq!(form_of(&two, &PlanDoc::default(), &MineDoc::default(), key("2026W"), Some(2), Picks::default()).unwrap().core, 1);

        // A core plan whose direction row (60 LP over FS 1–6) a page fills.
        let entries = vec![
            entry(1, Some("12104"), "Modul A", (1, 1), 30.0, Some("compulsory"), "Studienplan · Seite 5"),
            entry(16, None, "gem. der gewählten Studienrichtung", (1, 6), 60.0, Some("elective"), "Studienplan · Seite 5"),
            entry(1, Some("11112"), "Modul X", (1, 1), 30.0, Some("compulsory"), "Studienplan · Seite 7"),
            entry(2, Some("12330"), "Modul Y", (2, 2), 30.0, Some("compulsory"), "Studienplan · Seite 7"),
        ];
        let variants = plan_variants(&entries, &[]);
        let supplements = catalog::variants::supplements(&variants);
        let paged = PlanSource { program: program("370-82-2019", "bachelor-wirtschaftsingenieurwesen-2019"), supplements, variants, areas: Vec::new(), linked: Vec::new() };
        // The address names the page: its core, with the page chosen. (The chips say what the
        // program's page says: „7" where two plans share „Seite".)
        let form = form_of(&paged, &PlanDoc::default(), &MineDoc::default(), key("2026W"), Some(2), Picks::default()).unwrap();
        let pages: Vec<(&str, &str)> = form.pages.iter().map(|choice| (choice.label.as_str(), choice.title.as_str())).collect();
        assert_eq!((form.cores.len(), form.core, form.page, pages), (0, 0, Some(1), vec![("7", "Modul X, Modul Y")]));
        // The page is taken with the core, and the row it fills is not.
        assert_eq!(form.total.0.as_deref(), Some("3 Module"));
        // „offen": the row stays, a placeholder over its semesters.
        let open = form_of(&paged, &PlanDoc::default(), &MineDoc::default(), key("2026W"), Some(2), Picks { page: Some(None), ..Picks::default() }).unwrap();
        assert_eq!((open.page, open.total.0.as_deref()), (None, Some("1 Modul · 1 Platzhalter")));
        assert_eq!(open.preview.first().map(|line| line.0.as_str()), Some("1. FS · WiSe 2026/27 · 1 Modul · 1 Platzhalter"));
        // „Mein Studiengang" keeps the page. The core plan shown on the program's page is the core
        // without it („offen"); `import=mine` takes the page.
        let core = paged.variants.first().map(|plan| plan.full.clone());
        let page = paged.variants.get(1).map(|plan| plan.full.clone());
        let mine = MineDoc { program: Some("370-82-2019".into()), caption: core, direction: page, ..MineDoc::default() };
        let shown = form_of(&paged, &PlanDoc::default(), &mine, key("2026W"), Some(1), Picks::default()).unwrap();
        let stored = form_of(&paged, &PlanDoc::default(), &mine, key("2026W"), None, Picks::default()).unwrap();
        assert_eq!(((shown.core, shown.page), (stored.core, stored.page)), ((0, None), (0, Some(1))));
    }

    #[test]
    fn the_address_names_a_program_by_its_slug_or_as_mine() {
        let programs = vec![program("079-82-2008", "bachelor-informatik-2008")];
        assert_eq!(asked_program("bachelor-informatik-2008", &programs, None), (Some("079-82-2008".to_string()), false));
        assert_eq!(asked_program("bachelor-nichts", &programs, None), (None, true));
        assert_eq!(asked_program("mine", &programs, Some("079-82-2008")), (Some("079-82-2008".to_string()), false));
        // „Mein Studiengang" unset, or gone from the snapshot: nothing picked, and no complaint.
        assert_eq!(asked_program("mine", &programs, None), (None, false));
        assert_eq!(asked_program("mine", &programs, Some("079-82-1999")), (None, false));
        assert_eq!(program_name(&programs[0]), "Informatik B.Sc. · PO 2008");
    }

    #[test]
    fn the_uebersicht_lists_the_study_from_its_start() {
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key("2027W"), "12104", 1, None));
        // From the start to the end of the plan, over the current semester.
        let all = semesters(&doc, Some(key("2026W")), Some(4), Some(key("2026W")));
        assert_eq!(all, vec![key("2026W"), key("2027S"), key("2027W"), key("2028S")]);
        // Without a start: what the plan holds, up to the current semester.
        assert_eq!(semesters(&doc, None, None, Some(key("2026W"))), vec![key("2027W")]);
        assert_eq!(semesters(&doc, None, None, Some(key("2028S"))), vec![key("2027W"), key("2028S")]);
    }
}
