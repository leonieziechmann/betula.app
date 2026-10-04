//! „Module hinzufügen" (owner, 2026-10-04: „eine Auswahl an den in dem Semester geplanten Modulen,
//! danach die Wiederholer, wo man die anwählen kann und dann einfügen"): for a semester, the rows of
//! a Fachsemester of the Regelstudienplan (its own, or any other: „Plan-Semester") with where each
//! stands, the Wiederholer, and the catalog's search; each one ticked, „Einfügen" puts them in
//! together. Nothing is refused: a module planned elsewhere moves here, one the semester does not
//! offer is said so.
//!
//! The Gesamtplan's cell (`CellDialog`) offers the modules of one area for one semester: the
//! module tree's areas below it as filters, those offered then first, the others folded.

use std::collections::BTreeSet;

use folia_calendar::semester::SemesterKey;
use folia_model::rows::{CatalogRow, Prerequisite};
use folia_pages::ask::{CatalogRowsAsk, StudyModulesAsk};
use folia_plans::study::{self, AreaKind, Need, Offer, Pick, Stand, Standing};
use folia_plans::studyplan::PlanDoc;
use folia_routes::filter::{CatalogQuery, ProgramRelation, ProgramScope};
use leptos::prelude::*;

use folia_data::use_ask;
use folia_design::ui::Icon;

use super::dialog::DialogHead;
use super::focus::find_href;
use super::{n, now_secs, Plans, Ready, StudyCtx};
use crate::i18n::{self, Texts};

/// How many of the catalog's modules a search shows.
const HITS: u64 = 40;

/// One entry of the picker.
#[derive(Clone, Debug, PartialEq)]
struct Choice {
    /// `m:<id>`, `r:<caption>:<ord>` (`Item::key`).
    key: String,
    pick: Pick,
    name: String,
    credits: Option<f64>,
    credits_text: Option<String>,
    area: String,
    tone: &'static str,
    /// Notes under it: class and words.
    notes: Vec<(&'static str, String)>,
    /// Nothing to pick: passed, or planned here already (the words).
    done: Option<String>,
    /// A plan row without a module: the catalog of its modules.
    find: Option<String>,
    /// Open, and offered in the semester: what the picker ticks for a semester's own Fachsemester.
    fresh: bool,
}

/// „jedes SoSe", „jedes Semester", „im SoSe nicht angeboten · nur WiSe".
fn offer_note(offer: Offer, s: SemesterKey, t: &Texts) -> (&'static str, String) {
    let st = &t.study;
    let season = |winter: bool| if winter { t.format.winter_short } else { t.format.summer_short };
    if !offer.offered(s) {
        let only = offer.only().map(|only| format!(" · {}", if only == folia_model::labels::Season::Winter { st.only_winter } else { st.only_summer })).unwrap_or_default();
        return ("warn", format!("{}{only}", (st.unoffered)(season(s.winter))));
    }
    match offer.only() {
        Some(only) => ("", (st.every)(season(only == folia_model::labels::Season::Winter))),
        None => ("", st.every_semester.to_string()),
    }
}

/// The name of a module as a short note has room for: what stands before a bracket
/// („Mathematik IT-1 (Diskrete Mathematik)" → „Mathematik IT-1"). Two modules of one name with
/// different brackets („Programmierpraktikum (IMT)") are alternatives of each other.
pub(super) fn short_title(title: &str) -> &str {
    match title.split_once(" (") {
        Some((before, _)) if before.chars().count() >= 4 => before,
        _ => title,
    }
}

/// What module `id` asks for, seen from semester `s`: „empfiehlt Mathematik IT-2 ✓, Algorithmen
/// und Datenstrukturen im WiSe 26/27" while everything is in place; where something is not, only
/// that („setzt voraus Programmierpraktikum – noch nicht eingeplant"), and whether that is
/// something asked for (`true`), not only recommended. Modules of one name count as one; one of
/// the name of the module itself (`own`) is left out.
pub(super) fn needs_text(doc: &PlanDoc, prerequisites: &[Prerequisite], id: &str, own: &str, s: SemesterKey, now: SemesterKey, t: &Texts) -> Option<(String, bool)> {
    let st = &t.study;
    // A module of its own name is a version of it, no module before it.
    let needs: Vec<Need> = study::needs(doc, prerequisites, id, s, now).into_iter().filter(|need| short_title(&need.title) != short_title(own)).collect();
    if needs.is_empty() {
        return None;
    }
    let fine = |need: &Need| matches!(need.stand, Stand::Passed(_) | Stand::Before(_) | Stand::Same);
    // One entry per name: in place where any of its modules is.
    let mut named: Vec<(&str, bool, &Need)> = Vec::new();
    for need in &needs {
        let title = short_title(&need.title);
        match named.iter_mut().find(|(known, ..)| *known == title) {
            Some(entry) => {
                entry.1 |= need.mandatory;
                if fine(need) && !fine(entry.2) {
                    entry.2 = need;
                }
            }
            None => named.push((title, need.mandatory, need)),
        }
    }
    let word = |title: &str, need: &Need| match &need.stand {
        Stand::Passed(_) => format!("{title} ✓"),
        Stand::Before(at) => format!("{title} {}", (st.need_before)(&at.short(t.locale))),
        Stand::Same => format!("{title} {}", st.need_same),
        Stand::After(at) => format!("{title} – {}", (st.need_after)(&at.short(t.locale))),
        Stand::Failed(at) => format!("{title} – {}", (st.failed_in)(&at.short(t.locale))),
        Stand::Open => format!("{title} – {}", st.need_open),
    };
    let missing: Vec<&(&str, bool, &Need)> = named.iter().filter(|(.., need)| !fine(need)).collect();
    let shown: Vec<&(&str, bool, &Need)> = if missing.is_empty() { named.iter().collect() } else { missing };
    let mut parts = Vec::new();
    for mandatory in [true, false] {
        let list: Vec<String> = shown.iter().filter(|(_, m, _)| *m == mandatory).map(|(title, _, need)| word(title, need)).collect();
        if !list.is_empty() {
            parts.push((if mandatory { st.requires } else { st.recommends })(&list.join(", ")));
        }
    }
    let warn = shown.iter().any(|(_, mandatory, need)| *mandatory && !fine(need));
    Some((parts.join(" · "), warn))
}

/// The plan's rows of Fachsemester `fs` and the Wiederholer, as the picker offers them for `s`.
fn plan_choices(ctx: StudyCtx, s: SemesterKey, fs: u8, t: &Texts) -> (Vec<Choice>, Vec<Choice>, f64) {
    let st = &t.study;
    let doc = ctx.plan.map(|plan| plan.with(Clone::clone)).unwrap_or_default();
    let needs = ctx.needs.get();
    ctx.with_input(|input, ready| {
        let study = &ready.study;
        let mut total = 0.0;
        let plan = study::plan_semester(input, study, fs)
            .into_iter()
            .map(|suggestion| {
                total += suggestion.credits.unwrap_or(0.0);
                let (key, find) = match &suggestion.pick {
                    Pick::Module { id, .. } => (format!("m:{id}"), None),
                    Pick::Row { caption, ord } => (format!("r:{caption}:{ord}"), find_href(ctx, caption, *ord, t)),
                };
                let mut notes = Vec::new();
                let mut done = None;
                match &suggestion.standing {
                    Standing::Passed(at) => done = Some((st.passed_in)(&at.label(t.locale))),
                    Standing::Planned(at) if at.contains(&s) => done = Some(st.planned_here.to_string()),
                    Standing::Planned(at) => {
                        let later = at.iter().any(|at| *at >= study.now);
                        let shown = at.iter().max().map(|at| at.label(t.locale)).unwrap_or_default();
                        notes.push(("", format!("{}{}", (st.already_planned)(&shown), if later && matches!(suggestion.pick, Pick::Module { .. }) { st.would_move } else { "" })));
                    }
                    Standing::Open => {}
                }
                match &suggestion.pick {
                    Pick::Module { id, .. } => {
                        notes.push(offer_note(suggestion.offer, s, t));
                        if let Some((text, warn)) = needs_text(&doc, &needs, id, &suggestion.name, s, study.now, t).filter(|_| done.is_none()) {
                            notes.push((if warn { "warn" } else { "" }, text));
                        }
                    }
                    Pick::Row { .. } => notes.push(("", st.placeholder_note.to_string())),
                }
                let fresh = suggestion.standing == Standing::Open && suggestion.offer.offered(s) && matches!(suggestion.pick, Pick::Module { .. }) && !notes.iter().any(|(class, _)| *class == "warn");
                Choice {
                    fresh,
                    key,
                    pick: suggestion.pick,
                    name: suggestion.name,
                    credits: suggestion.credits,
                    credits_text: suggestion.credits_text,
                    area: ready.area_name(suggestion.area, t),
                    tone: ready.tone(suggestion.area),
                    notes,
                    done,
                    find,
                }
            })
            .collect();
        let retakes = study
            .retakes
            .iter()
            .map(|retake| {
                let item = &retake.item;
                let pick = match &item.subject {
                    study::Subject::Module { id } => Pick::Module { id: id.clone(), from_plan: item.plan_fs.is_some() },
                    study::Subject::Row { caption, ord, .. } => Pick::Row { caption: caption.clone(), ord: *ord },
                };
                let when = match ready.fs_label(retake.failed_in, t) {
                    Some(fs) => format!("{} ({fs})", (st.failed_in)(&retake.failed_in.label(t.locale))),
                    None => (st.failed_in)(&retake.failed_in.label(t.locale)),
                };
                let mut notes = Vec::new();
                if item.module_id().is_some() {
                    notes.push(offer_note(item.offer, s, t));
                }
                notes.push(("", when));
                if let Some(id) = item.module_id() {
                    if let Some((text, warn)) = needs_text(&doc, &needs, id, &item.name, s, study.now, t) {
                        notes.push((if warn { "warn" } else { "" }, text));
                    }
                }
                Choice { key: item.key(), pick, name: item.name.clone(), credits: item.credits, credits_text: None, area: ready.area_name(item.area, t), tone: ready.tone(item.area), notes, done: None, find: None, fresh: false }
            })
            .collect();
        (plan, retakes, total)
    })
    .unwrap_or_default()
}

/// The catalog's rows as the picker offers them for `s`.
fn catalog_choices(ctx: StudyCtx, ready: &Ready, rows: &[CatalogRow], needs: &[Prerequisite], s: SemesterKey, t: &Texts) -> Vec<Choice> {
    let mut choices = catalog_rows(ctx, ready, rows, needs, s, t);
    // What is passed or planned here already, last.
    choices.sort_by_key(|choice| choice.done.is_some());
    choices
}

fn catalog_rows(ctx: StudyCtx, ready: &Ready, rows: &[CatalogRow], needs: &[Prerequisite], s: SemesterKey, t: &Texts) -> Vec<Choice> {
    let st = &t.study;
    let doc = ctx.plan.map(|plan| plan.with(Clone::clone)).unwrap_or_default();
    let tree_area = |id: &str| {
        ctx.plans.with(|plans| {
            let Plans::Found(source) = plans else { return None };
            let placed: Vec<i64> = source.placements.iter().filter(|(module, _)| module == id).map(|(_, area)| *area).collect();
            ready.study.progress.iter().position(|progress| placed.iter().any(|at| progress.area.tree.contains(at)) || (progress.area.kind == AreaKind::Fues && source.fues.iter().any(|fues| fues == id)))
        })
    };
    rows.iter()
        .map(|row| {
            let area = tree_area(&row.id);
            let mut notes = vec![offer_note(Offer::of(row), s, t)];
            let planned = doc.planned_in(&row.id);
            let mut done = None;
            if let Some(passed) = doc.passed_in(&row.id) {
                done = Some((st.passed_in)(&passed.label(t.locale)));
            } else if planned.contains(&s) {
                done = Some(st.planned_here.to_string());
            } else if let Some(at) = planned.iter().max() {
                let later = *at >= ready.study.now;
                notes.push(("", format!("{}{}", (st.already_planned)(&at.label(t.locale)), if later { st.would_move } else { "" })));
            }
            if let Some((text, warn)) = needs_text(&doc, needs, &row.id, &row.title, s, ready.study.now, t).filter(|_| done.is_none()) {
                notes.push((if warn { "warn" } else { "" }, text));
            }
            Choice {
                key: format!("m:{}", row.id),
                pick: Pick::Module { id: row.id.clone(), from_plan: false },
                name: row.title.clone(),
                credits: row.credits,
                credits_text: None,
                area: ready.area_name(area, t),
                tone: ready.tone(area),
                notes,
                done,
                find: None,
                fresh: false,
            }
        })
        .collect()
}

/// A list of entries to tick.
#[component]
fn ChoiceList(choices: Vec<Choice>, picked: RwSignal<Vec<(String, Pick, f64)>>) -> impl IntoView {
    let t = i18n::t();
    let st = &t.study;
    view! {
        <ul class="st-choices">
            {choices.into_iter().map(|choice| {
                let key = choice.key.clone();
                let on = {
                    let key = key.clone();
                    Memo::new(move |_| picked.with(|picked| picked.iter().any(|(k, ..)| *k == key)))
                };
                let toggle = {
                    let (key, pick, credits) = (key.clone(), choice.pick.clone(), choice.credits.unwrap_or(0.0));
                    move |_| picked.update(|picked| match picked.iter().position(|(k, ..)| *k == key) {
                        Some(at) => {
                            picked.remove(at);
                        }
                        None => picked.push((key.clone(), pick.clone(), credits)),
                    })
                };
                let credits = choice.credits_text.clone().map(|text| folia_plans::plan::credits_in(&text, t.locale)).or_else(|| choice.credits.map(|credits| n(credits, t))).map(|credits| (st.credits)(&credits));
                let label = (st.select)(&choice.name);
                view! {
                    <li class="st-choice-row" class:is-done=choice.done.is_some()>
                        {match choice.done.clone() {
                            Some(_) => view! { <span class="st-tick-static" aria-hidden="true"><span class="st-box with"><Icon name="check"/></span></span> }.into_any(),
                            None => view! {
                                <button class="st-tick hit" type="button" role="checkbox" aria-checked=move || if on.get() { "true" } else { "false" } aria-label=label on:click=toggle>
                                    <span class="st-box" class:with=move || on.get()><Icon name="check"/></span>
                                </button>
                            }.into_any(),
                        }}
                        <div class="st-choice-what">
                            <div class="st-line"><span class="st-choice-name">{choice.name.clone()}</span><span class="st-lp">{credits}</span></div>
                            <div class="st-sub">
                                <span class="st-area"><span class="st-dot" style=format!("--c: {}", choice.tone)></span>{choice.area.clone()}</span>
                                {choice.done.clone().map(|done| view! { <span class="ok">{done}</span> })}
                                {choice.notes.into_iter().map(|(class, text)| view! { <span class=class>{text}</span> }).collect_view()}
                                {choice.find.map(|href| view! { <a href=t.path(&href) rel="nofollow">{st.choose_module}</a> })}
                            </div>
                        </div>
                    </li>
                }
            }).collect_view()}
        </ul>
    }
}

/// What the footer says: „3 ausgewählt · 18 LP", „SoSe 2027 danach: 30 LP – so viel wie im
/// Regelstudienplan", and „Einfügen (3)".
#[component]
fn PickFoot(ctx: StudyCtx, semester: SemesterKey, picked: RwSignal<Vec<(String, Pick, f64)>>) -> impl IntoView {
    let t = i18n::t();
    let st = &t.study;
    let figures = Memo::new(move |_| ctx.with_ready(|ready| ready.study.semester(semester).map(|semester| (semester.credits, semester.planned))).flatten());
    let insert = move |_| {
        let picks: Vec<Pick> = picked.get_untracked().into_iter().map(|(_, pick, _)| pick).collect();
        if picks.is_empty() {
            return;
        }
        let note = (st.added)(picks.len());
        ctx.change_noted(Some(note), move |doc, input| {
            study::add(doc, input, semester, &picks, now_secs());
        }, || {});
        ctx.focus.set(Some(semester));
        ctx.dialog.set(None);
    };
    view! {
        <footer class="st-dlg-foot">
            <div class="st-foot-text">
                <p><b>{move || {
                    let (count, credits) = picked.with(|picked| (picked.len(), picked.iter().map(|(.., credits)| credits).sum::<f64>()));
                    (st.selected)(count, &n(credits, t))
                }}</b></p>
                <p class="st-sub">{move || figures.get().map(|(credits, planned)| {
                    let after = credits + picked.with(|picked| picked.iter().map(|(.., credits)| credits).sum::<f64>());
                    let compare = match planned {
                        Some(plan) if (after - plan).abs() < 0.5 => st.as_planned.to_string(),
                        Some(plan) if after > plan => (st.more_than_plan)(&n(after - plan, t)),
                        Some(plan) => (st.less_than_plan)(&n(plan - after, t)),
                        None => String::new(),
                    };
                    format!("{}{compare}", (st.after_add)(&semester.label(t.locale), &n(after, t)))
                })}</p>
            </div>
            <button class="btn secondary st-pc" type="button" on:click=move |_| ctx.dialog.set(None)>{st.cancel}</button>
            <button class="btn primary" type="button" id="st-insert" disabled=move || picked.with(Vec::is_empty) on:click=insert><Icon name="plus"/>{move || (st.insert)(picked.with(Vec::len))}</button>
        </footer>
    }
}

/// The catalog's search: what is typed, a moment after the last key.
fn typed_search(text: RwSignal<String>) -> RwSignal<String> {
    let settled = RwSignal::new(text.get_untracked());
    Effect::new(move |_| {
        let now = text.get();
        set_timeout(
            move || {
                if text.with_untracked(|text| *text == now) {
                    settled.try_set(now);
                }
            },
            std::time::Duration::from_millis(220),
        );
    });
    settled
}

/// „Module hinzufügen" for semester `semester`.
#[component]
pub(super) fn AddDialog(ctx: StudyCtx, semester: SemesterKey, catalog: bool, chosen: Vec<String>) -> impl IntoView {
    let t = i18n::t();
    let st = &t.study;
    let in_catalog = RwSignal::new(catalog);
    // The Fachsemester of the semester, else the nearest; and how many the plan has.
    let fs_of = Memo::new(move |_| {
        ctx.with_ready(|ready| {
            let study = &ready.study;
            let most = ready.named.iter().filter_map(|named| named.span.map(|(_, to)| to)).max().unwrap_or(0);
            let fs = study.semester(semester).and_then(|s| s.fs).or_else(|| study.semesters.iter().filter(|s| s.key > semester).find_map(|s| s.fs)).unwrap_or(1);
            (fs.min(most.max(1)), most)
        })
        .unwrap_or((1, 0))
    });
    let fs = RwSignal::new(fs_of.get_untracked().0);
    let lists = Memo::new(move |_| plan_choices(ctx, semester, fs.get(), t));
    // Picked to begin with: what the dialog was opened with, and the plan's open modules that the
    // semester offers, for its own Fachsemester.
    let picked = RwSignal::new(Vec::<(String, Pick, f64)>::new());
    {
        let (plan, retakes, _) = lists.get_untracked();
        let own = fs_of.get_untracked().0 == fs.get_untracked() && ctx.with_ready(|ready| ready.study.semester(semester).is_some_and(|s| s.fs.is_some() && s.when != study::When::Past)).unwrap_or(false);
        let start: Vec<(String, Pick, f64)> = plan
            .iter()
            .chain(retakes.iter())
            .filter(|choice| choice.done.is_none())
            .filter(|choice| chosen.contains(&choice.key) || (own && choice.fresh))
            .map(|choice| (choice.key.clone(), choice.pick.clone(), choice.credits.unwrap_or(0.0)))
            .collect();
        picked.set(start);
    }
    let sub = Memo::new(move |_| {
        ctx.with_ready(|ready| {
            let semester_now = ready.study.semester(semester)?;
            let fs = match semester_now.fs {
                Some(fs) => (st.fs_long)(fs),
                None => st.leave.to_string(),
            };
            Some((st.add_sub)(&semester.label(t.locale), &fs, &n(semester_now.credits, t)))
        })
        .flatten()
        .unwrap_or_default()
    });
    let options = Memo::new(move |_| (1..=fs_of.get().1).map(|fs| (fs.to_string(), (st.fs_long)(fs))).collect::<Vec<_>>());

    // The catalog.
    let text = RwSignal::new(String::new());
    let only_program = RwSignal::new(true);
    let settled = typed_search(text);
    let slug = Memo::new(move |_| ctx.with_ready(|ready| ready.program.slug.clone()));
    let hits = use_ask(move || {
        let text = settled.get();
        if text.trim().chars().count() < 2 {
            return None;
        }
        let program = only_program.get().then(|| slug.get()).flatten().map(|program_slug| ProgramScope { program_slug, ..Default::default() });
        Some(CatalogRowsAsk { query: CatalogQuery { text, program, ..Default::default() }, offset: 0, limit: HITS })
    });
    let hit_rows = Memo::new(move |_| hits.with(|hits| hits.as_ref().and_then(|hits| hits.as_ref().ok()).map(|page| page.rows.clone()).unwrap_or_default()));
    let hit_needs = use_ask(move || {
        let ids: Vec<String> = hit_rows.with(|rows| rows.iter().map(|row| row.id.clone()).collect());
        (!ids.is_empty()).then_some(StudyModulesAsk { ids })
    });
    let found = Memo::new(move |_| {
        let needs = hit_needs.with(|needs| needs.as_ref().and_then(|needs| needs.as_ref().ok()).map(|modules| modules.prerequisites.clone()).unwrap_or_default());
        hit_rows.with(|rows| ctx.with_ready(|ready| catalog_choices(ctx, ready, rows, &needs, semester, t)).unwrap_or_default())
    });

    view! {
        <DialogHead ctx title=st.add_title sub=sub.get_untracked()/>
        <div class="st-dlg-bar">
            <div class="st-tabs" role="group">
                <button type="button" aria-pressed=move || if in_catalog.get() { "false" } else { "true" } on:click=move |_| in_catalog.set(false)>{st.tab_plan}</button>
                <button type="button" aria-pressed=move || if in_catalog.get() { "true" } else { "false" } on:click=move |_| in_catalog.set(true)>{st.tab_catalog}</button>
            </div>
            {move || (!in_catalog.get() && fs_of.get().1 > 0).then(|| view! {
                <label class="st-inline">
                    <span class="label">{st.plan_semester}</span>
                    <super::side::PlainSelect
                        id="st-plan-fs"
                        label=st.plan_semester
                        options=Signal::derive(move || options.get())
                        chosen=Signal::derive(move || fs.get().to_string())
                        pick=Callback::new(move |value: String| fs.set(value.parse().unwrap_or(1)))
                    />
                </label>
            })}
        </div>
        <div class="st-dlg-body st-pick">
            {move || if in_catalog.get() {
                view! {
                    <div class="st-search">
                        <label class="st-search-box">
                            <Icon name="search"/>
                            <input type="search" id="st-search" placeholder=st.search_placeholder aria-label=st.tab_catalog autofocus=true prop:value=move || text.get() on:input=move |ev| text.set(event_target_value(&ev))/>
                        </label>
                        <button class="st-check" type="button" role="checkbox" aria-checked=move || if only_program.get() { "true" } else { "false" } on:click=move |_| only_program.update(|only| *only = !*only)>
                            <span class="st-box" class:with=move || only_program.get() aria-hidden="true"><Icon name="check"/></span>{st.only_program}
                        </button>
                    </div>
                    {move || {
                        let found = found.get();
                        if settled.with(|text| text.trim().chars().count() < 2) {
                            view! { <p class="st-sub st-pad">{st.type_to_search}</p> }.into_any()
                        } else if found.is_empty() {
                            view! { <p class="st-sub st-pad">{st.no_hits}</p> }.into_any()
                        } else {
                            view! { <ChoiceList choices=found picked/> }.into_any()
                        }
                    }}
                }.into_any()
            } else {
                view! {
                    {move || {
                        let (plan, retakes, total) = lists.get();
                        view! {
                            <section aria-label=(st.plan_fs_heading)(&(st.fs)(fs.get()))>
                                <div class="st-pick-head"><h3>{(st.plan_fs_heading)(&(st.fs)(fs.get()))}</h3><span>{(total > 0.0).then(|| (st.credits)(&n(total, t)))}</span></div>
                                {if plan.is_empty() { view! { <p class="st-sub st-pad">{st.fits_nothing}</p> }.into_any() } else { view! { <ChoiceList choices=plan picked/> }.into_any() }}
                            </section>
                            {(!retakes.is_empty()).then(|| view! {
                                <section aria-label=st.retakes>
                                    <div class="st-pick-head"><h3>{st.retakes}" "<span class="st-sub">{format!("· {}", st.retakes_sub)}</span></h3></div>
                                    <ChoiceList choices=retakes picked/>
                                </section>
                            })}
                        }
                    }}
                }.into_any()
            }}
        </div>
        <PickFoot ctx semester picked/>
    }
}

/// The areas of the module tree below an area, to choose from: id and name.
fn sub_areas(ctx: StudyCtx, tree: &BTreeSet<i64>) -> Vec<(i64, String)> {
    ctx.plans.with(|plans| {
        let Plans::Found(source) = plans else { return Vec::new() };
        source.areas.iter().filter(|area| tree.contains(&area.id) && area.choice && area.modules > 0).map(|area| (area.id, area.name().to_string())).collect()
    })
}

/// A cell of the Gesamtplan: the modules of area `area` for semester `semester`.
#[component]
pub(super) fn CellDialog(ctx: StudyCtx, area: Option<usize>, semester: SemesterKey) -> impl IntoView {
    let t = i18n::t();
    let st = &t.study;
    let head = Memo::new(move |_| {
        ctx.with_ready(|ready| {
            let name = area.and_then(|area| ready.named.get(area)).map_or_else(|| st.outside.to_string(), |named| named.full.clone());
            let open = area.and_then(|area| ready.study.progress.get(area)).map(|progress| (st.cell_open)(&n(progress.open(), t)));
            (format!("{name} · {}", semester.label(t.locale)), open, ready.tone(area))
        })
    });
    let scope = Memo::new(move |_| {
        ctx.with_ready(|ready| {
            let progress = area.and_then(|area| ready.study.progress.get(area));
            let base = ProgramScope { program_slug: ready.program.slug.clone(), ..Default::default() };
            match progress {
                Some(progress) if progress.area.kind == AreaKind::Fues => Some((ProgramScope { relation: ProgramRelation::Fues, ..base }, Vec::new())),
                Some(progress) if !progress.area.tree.is_empty() => {
                    let filters = sub_areas(ctx, &progress.area.tree);
                    Some((ProgramScope { areas: progress.area.tree.iter().copied().collect(), ..base }, filters))
                }
                _ => None,
            }
        })
        .flatten()
    });
    let filter = RwSignal::new(None::<i64>);
    let text = RwSignal::new(String::new());
    let settled = typed_search(text);
    let hits = use_ask(move || {
        let text = settled.get();
        let program = match scope.get() {
            Some((scope, _)) => Some(match filter.get() {
                Some(id) => ProgramScope { areas: vec![id], ..scope },
                None => scope,
            }),
            // An area without modules of its own (the thesis, what counts nowhere): the search.
            None if text.trim().chars().count() >= 2 => None,
            None => return None,
        };
        Some(CatalogRowsAsk { query: CatalogQuery { text, program, ..Default::default() }, offset: 0, limit: 200 })
    });
    let rows = Memo::new(move |_| hits.with(|hits| hits.as_ref().and_then(|hits| hits.as_ref().ok()).map(|page| page.rows.clone()).unwrap_or_default()));
    let needs = use_ask(move || {
        let ids: Vec<String> = rows.with(|rows| rows.iter().map(|row| row.id.clone()).collect());
        (!ids.is_empty()).then_some(StudyModulesAsk { ids })
    });
    // The plan's open rows of the area, first; the modules offered then; the others folded.
    let lists = Memo::new(move |_| {
        let needs = needs.with(|needs| needs.as_ref().and_then(|needs| needs.as_ref().ok()).map(|modules| modules.prerequisites.clone()).unwrap_or_default());
        ctx.with_ready(|ready| {
            let lines: Vec<Choice> = ready
                .lines
                .iter()
                .filter(|line| line.suggestion.area == area && line.semester == semester)
                .map(|line| {
                    let suggestion = &line.suggestion;
                    let (key, find) = match &suggestion.pick {
                        Pick::Module { id, .. } => (format!("m:{id}"), None),
                        Pick::Row { caption, ord } => (format!("r:{caption}:{ord}"), find_href(ctx, caption, *ord, t)),
                    };
                    let note = match line.failed_in {
                        Some(failed) => (st.line_retake)(&failed.label(t.locale)),
                        None => line.plan_fs.map(|fs| (st.plan_says)(&(st.fs)(fs))).unwrap_or_default(),
                    };
                    Choice { key, pick: suggestion.pick.clone(), name: suggestion.name.clone(), credits: suggestion.credits, credits_text: suggestion.credits_text.clone(), area: ready.area_name(area, t), tone: ready.tone(area), notes: vec![("", note)], done: None, find, fresh: false }
                })
                .collect();
            let all = rows.with(|rows| catalog_choices(ctx, ready, rows, &needs, semester, t));
            let offered_ids: BTreeSet<String> = rows.with(|rows| rows.iter().filter(|row| Offer::of(row).offered(semester)).map(|row| row.id.clone()).collect());
            let (offered, other): (Vec<Choice>, Vec<Choice>) = all.into_iter().filter(|choice| !lines.iter().any(|line| line.key == choice.key)).partition(|choice| match &choice.pick {
                Pick::Module { id, .. } => offered_ids.contains(id),
                Pick::Row { .. } => true,
            });
            (lines, offered, other)
        })
        .unwrap_or_default()
    });
    let picked = RwSignal::new(Vec::<(String, Pick, f64)>::new());
    let show_other = RwSignal::new(false);
    move || {
        let Some((title, open, tone)) = head.get() else { return ().into_any() };
        view! {
            <DialogHead ctx title=title.clone() sub=open.unwrap_or_default() tone=tone/>
            <div class="st-dlg-bar column">
                {move || scope.get().and_then(|(_, filters)| (!filters.is_empty()).then(|| view! {
                    <div class="st-filters" role="group">
                        <button type="button" aria-pressed=move || if filter.get().is_none() { "true" } else { "false" } on:click=move |_| filter.set(None)>{st.all}</button>
                        {filters.into_iter().map(|(id, name)| view! {
                            <button type="button" aria-pressed=move || if filter.get() == Some(id) { "true" } else { "false" } on:click=move |_| filter.set(Some(id))>{name}</button>
                        }).collect_view()}
                    </div>
                }))}
                <label class="st-search-box">
                    <Icon name="search"/>
                    <input type="search" id="st-cell-search" placeholder=st.search_placeholder aria-label=st.tab_catalog prop:value=move || text.get() on:input=move |ev| text.set(event_target_value(&ev))/>
                </label>
            </div>
            <div class="st-dlg-body st-pick">
                {move || {
                    let (lines, offered, other) = lists.get();
                    let count = other.len();
                    view! {
                        {(!lines.is_empty()).then(|| view! { <ChoiceList choices=lines picked/> })}
                        {if offered.is_empty() && count == 0 {
                            (scope.with(Option::is_none) && settled.with(|text| text.trim().chars().count() < 2)).then(|| view! { <p class="st-sub st-pad">{st.type_to_search}</p> }).into_any()
                        } else {
                            view! { <ChoiceList choices=offered picked/> }.into_any()
                        }}
                        {(count > 0).then(|| view! {
                            <button class="st-link st-pad" type="button" aria-expanded=move || if show_other.get() { "true" } else { "false" } on:click=move |_| show_other.update(|show| *show = !*show)>
                                {(st.not_offered_group)(&semester.short(t.locale), count)}<Icon name="chevron-down"/>
                            </button>
                            {move || show_other.get().then(|| view! { <ChoiceList choices=other.clone() picked/> })}
                        })}
                    }
                }}
            </div>
            <PickFoot ctx semester picked/>
        }
        .into_any()
    }
}
