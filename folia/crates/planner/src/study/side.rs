//! The sidebar of „Mein Studium", top to bottom: „Mein Studiengang" (the program, its study
//! direction, the Studienbeginn and the Fachsemester now, and „Studiengang ändern …"), the ways to
//! the program's pages, the way to the Stundenplan, how the plan reads, and where all of it lives.
//! On a phone the sidebar is the sheet „Anpassen".
//!
//! The program is set once and changed rarely (owner, 2026-10-04: „den eigenen Studiengang zu
//! wählen sollte eher so eine Sache sein, die man für sich halt so einmal macht"): in a dialog that
//! says what of the plan counts in the new one before anything changes (`SwitchDialog`). Nothing of
//! the plan goes with a change; what counts nowhere there shows as „ohne Bereich".

use std::collections::BTreeSet;

use folia_calendar::semester::SemesterKey;
use folia_model::rows::{CatalogRow, Program};
use folia_pages::ask::{PlanSourceAsk, ProgramsAsk};
use folia_pages::PlanSource;
use folia_plans::study;
use folia_plans::studyplan::PlanDoc;
use folia_routes::url::{self, ProgramTab, ProgramUrl};
use leptos::prelude::*;

use folia_design::combobox::{ComboItem, Combobox};
use folia_design::ui::Icon;
use folia_stores::myprogram::{po_of, program_name, ProgramPlans};

use super::dialog::DialogHead;
use super::{chosen, input_of, n, rows_of, Dialog, Plans, Setup, StudyCtx};
use crate::i18n::{self, Texts};

/// How many semesters before the current one the Studienbeginn's select offers, and after it.
const STARTS_BEFORE: i32 = 16;
const STARTS_AFTER: i32 = 2;

#[component]
pub(super) fn StudySidebar(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let ready = Memo::new(move |_| ctx.with_ready(|ready| ready.setup.start_stored).unwrap_or(false));
    view! {
        <MineCard ctx/>
        <WaysGroup ctx/>
        {move || ready.get().then(|| view! {
            <div class="fgroup actions">
                <p class="flabel label">{s.timetable}</p>
                <a class="action" href=t.path(url::STUDYPLAN) rel="nofollow"><Icon name="calendar-range"/><span>{s.to_timetable}</span><Icon name="chevron-right"/></a>
            </div>
            <LegendGroup/>
        })}
        <div class="fgroup">
            <p class="hint storage-hint"><Icon name="shield-check"/><span>{s.storage_hint}</span></p>
        </div>
    }
}

/// What „Mein Studiengang" says.
#[derive(Clone, Debug, PartialEq)]
struct Mine {
    name: String,
    degree: String,
    direction: Option<String>,
    start: String,
    /// „Jetzt im" and „3. Fachsemester", or „Beginnt" and the Studienbeginn.
    now: (&'static str, String),
}

/// „Mein Studiengang": a card, not a picker; „Studiengang ändern …" opens the dialog.
#[component]
fn MineCard(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let mine = Memo::new(move |_| {
        ctx.with_ready(|ready| {
            let setup = &ready.setup;
            ready.setup.start_stored.then(|| {
                let now = match study::fs_of(setup.start, setup.now, &setup.leave) {
                    Some(fs) => (s.now_in, (s.fs_long)(fs)),
                    None if setup.now < setup.start => (s.begins, setup.start.label(t.locale)),
                    None => (s.now_in, s.leave.to_string()),
                };
                Mine {
                    name: ready.program.name.clone(),
                    degree: format!("{} · PO {}", ready.program.degree(), po_of(&ready.program)),
                    direction: setup.shown.and_then(|shown| ready.plans.iter().find(|(index, _)| *index == shown)).map(|(_, label)| label.clone()),
                    start: setup.start.label(t.locale),
                    now,
                }
            })
        })
        .flatten()
    });
    move || {
        mine.get().map(|mine| {
            view! {
                <div class="fgroup first st-mine">
                    <p class="flabel label">{s.mine}</p>
                    <div class="st-mine-card">
                        <div class="st-mine-top">
                            <span class="st-mine-icon"><Icon name="graduation-cap"/></span>
                            <div>
                                <p class="st-mine-name">{mine.name}</p>
                                <p class="st-mine-degree">{mine.degree}</p>
                            </div>
                        </div>
                        <dl>
                            {mine.direction.map(|direction| view! { <dt>{s.direction}</dt><dd>{direction}</dd> })}
                            <dt>{s.start}</dt><dd>{mine.start}</dd>
                            <dt>{mine.now.0}</dt><dd>{mine.now.1}</dd>
                        </dl>
                        <button class="st-link" type="button" on:click=move |_| ctx.open(Dialog::Switch)>{s.change_program}</button>
                    </div>
                </div>
            }
        })
    }
}

/// The ways to the program's pages (with the plan studied) and to all programs.
#[component]
fn WaysGroup(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let links = Memo::new(move |_| {
        ctx.with_ready(|ready| {
            let plan = ProgramUrl::new(&ready.program.slug, ProgramTab::Plan).with_variant(ready.setup.shown.map_or(1, |shown| shown + 1)).path();
            (plan, url::program_path(&ready.program.slug, ProgramTab::Areas))
        })
    });
    view! {
        <nav class="fgroup st-ways" aria-label=s.ways>
            {move || links.get().map(|(plan, areas)| view! {
                <a class="action" href=t.path(&plan)><Icon name="file-check-2"/><span>{s.to_plan}</span><Icon name="chevron-right"/></a>
                <a class="action" href=t.path(&areas)><Icon name="layout-list"/><span>{s.to_areas}</span><Icon name="chevron-right"/></a>
            })}
            <a class="action" href=t.path(url::PROGRAMS)><Icon name="graduation-cap"/><span>{s.all_programs}</span><Icon name="chevron-right"/></a>
        </nav>
    }
}

/// How the plan reads: the parts of the bar and the marks of a row.
#[component]
fn LegendGroup() -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let entry = |class: &'static str, (name, what): (&'static str, &'static str), icon: Option<&'static str>| {
        view! { <li><span class=format!("st-legend-mark {class}") aria-hidden="true">{icon.map(|icon| view! { <Icon name=icon/> })}</span><span><b>{name}</b>" – "{what}</span></li> }
    };
    view! {
        <div class="fgroup st-legend">
            <p class="flabel label">{s.legend}</p>
            <ul>
                {entry("passed", s.legend_passed, None)}
                {entry("planned", s.legend_planned, None)}
                {entry("open", s.legend_open, None)}
                {entry("over", s.legend_over, None)}
                {entry("chip", s.legend_retake, Some("repeat"))}
                {entry("chip warn", s.legend_offer, Some("triangle-alert"))}
            </ul>
        </div>
    }
}

/// Every program of the snapshot, as the pickers offer them: newest PO first.
pub(super) fn program_items(ctx: StudyCtx) -> (Memo<Vec<Program>>, Signal<Vec<ComboItem>>) {
    let all = Memo::new(move |before| match ctx.source.with_value(|source| source.as_ref().map(|source| source.now(&ProgramsAsk {}))) {
        Some(now) => folia_pages::ask::unless_pending(now, before, Result::unwrap_or_default),
        None => Vec::new(),
    });
    let items = Signal::derive(move || {
        all.with(|all| {
            all.iter()
                .map(|program| ComboItem::new(program.id.clone(), program.name.clone(), format!("{} · PO {}", program.degree(), po_of(program)), i64::from(program.is_latest_po)))
                .collect::<Vec<_>>()
        })
    });
    (all, items)
}

/// The plans of the program `id`, as the catalog answers; `None` while on their way.
pub(super) fn plans_of(ctx: StudyCtx, id: Signal<Option<String>>) -> Memo<Option<Option<PlanSource>>> {
    let t = i18n::t();
    Memo::new(move |before: Option<&Option<Option<PlanSource>>>| {
        let id = id.get()?;
        // The program kept: its plans are there already.
        if let Some(kept) = ctx.plans.with(|plans| match plans {
            Plans::Found(source) if source.program.id == id => Some((**source).clone()),
            _ => None,
        }) {
            return Some(Some(kept));
        }
        let now = ctx.source.with_value(|source| source.as_ref().map(|source| source.now(&PlanSourceAsk { program_id: id.clone(), locale: t.locale })))?;
        match now {
            Ok(found) => Some(found),
            Err(error) if error.is_pending() => before.cloned().flatten().filter(|source| source.as_ref().is_some_and(|source| source.program.id == id)),
            Err(_) => Some(None),
        }
    })
}

/// The semesters a study may have begun in, each with the Fachsemester it makes the current one.
pub(super) fn start_options(now: SemesterKey, leave: &BTreeSet<SemesterKey>, t: &Texts) -> Vec<(String, String)> {
    (-STARTS_AFTER..=STARTS_BEFORE)
        .filter_map(|n| now.plus(-n))
        .map(|start| {
            let label = match study::fs_of(start, now, leave) {
                Some(fs) => (t.study.start_entry)(&start.label(t.locale), fs),
                None => start.label(t.locale),
            };
            (start.key(), label)
        })
        .collect()
}

/// A select of `options` (value, label) with `chosen`; `pick` gets the value picked.
#[component]
pub(super) fn PlainSelect(
    id: &'static str,
    label: &'static str,
    #[prop(into)] options: Signal<Vec<(String, String)>>,
    #[prop(into)] chosen: Signal<String>,
    pick: Callback<String>,
    #[prop(optional, into)] disabled: Signal<bool>,
) -> impl IntoView {
    view! {
        <span class="select-wrap plain">
            <select id=id aria-label=label prop:value=move || chosen.get() disabled=move || disabled.get() on:change=move |ev| pick.run(event_target_value(&ev))>
                <For
                    each=move || options.get()
                    key=|entry| entry.clone()
                    children=move |(value, text): (String, String)| {
                        let here = value.clone();
                        view! { <option value=value selected=move || chosen.with(|chosen| *chosen == here)>{text}</option> }
                    }
                />
            </select>
            <Icon name="chevrons-up-down"/>
        </span>
    }
}

/// The plan picked of `source` (an index of its plans), as „Mein Studiengang" keeps it: its
/// caption and the page of a direction.
pub(super) fn kept_plan(source: &PlanSource, index: usize) -> (String, Option<String>) {
    if source.variants.is_empty() {
        return (String::new(), None);
    }
    ProgramPlans::new(&source.variants, source.supplements.clone()).kept(index.min(source.variants.len() - 1))
}

/// The plans of `source` to choose a study direction from: index and label, where there are two.
pub(super) fn direction_options(source: &PlanSource) -> Vec<(String, String)> {
    match source.variants.len() {
        0 | 1 => Vec::new(),
        _ => source.variants.iter().enumerate().map(|(index, plan)| (index.to_string(), plan.label.clone())).collect(),
    }
}

/// What of the plan counts in another program: credits passed and planned, what of them counts
/// there, and the modules that count nowhere there.
#[derive(Clone, Debug, PartialEq)]
struct Counts {
    name: String,
    passed: (f64, f64),
    planned: (f64, f64),
    nowhere: Vec<(String, f64, bool)>,
}

fn counts(source: &PlanSource, setup: &Setup, doc: &PlanDoc, rows: &[CatalogRow]) -> Counts {
    let input = input_of(source, setup, doc, rows);
    let there = study::study(&input);
    let mut counts = Counts { name: source.program.name.clone(), passed: (0.0, 0.0), planned: (0.0, 0.0), nowhere: Vec::new() };
    for item in there.semesters.iter().flat_map(|semester| semester.items.iter()).filter(|item| item.passed || !item.failed) {
        let credits = item.credits.unwrap_or(0.0);
        let counted = item.area.is_some();
        let part = if item.passed { &mut counts.passed } else { &mut counts.planned };
        part.0 += credits;
        if counted {
            part.1 += credits;
        } else if credits > 0.0 {
            counts.nowhere.push((item.name.clone(), credits, item.passed));
        }
    }
    counts
}

/// „Studiengang wechseln": another program, its study direction and the Studienbeginn; what of the
/// plan counts there, before anything changes.
#[component]
pub(super) fn SwitchDialog(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let (all, items) = program_items(ctx);
    let before = Memo::new(move |_| ctx.with_ready(|ready| (ready.program.clone(), ready.setup.clone())));
    let program = RwSignal::new(before.get_untracked().map(|(program, _)| program.id));
    let start = RwSignal::new(before.get_untracked().map(|(_, setup)| setup.start));
    let direction = RwSignal::new(before.get_untracked().and_then(|(_, setup)| setup.shown).unwrap_or(0));
    let source = plans_of(ctx, program.into());
    // A program picked anew starts with its first plan.
    let pick_program = Callback::new(move |id: Option<String>| {
        if id.is_some() && id != program.get_untracked() {
            program.set(id);
            direction.set(0);
        }
    });
    let preview = Memo::new(move |_| {
        let (_, setup) = before.get()?;
        let source = source.get()??;
        let start = start.get()?;
        let (shown, core, page) = chosen(&source, Some(&kept_plan(&source, direction.get()).0), kept_plan(&source, direction.get()).1.as_deref());
        let setup = Setup { shown, core, page, start, start_stored: true, ..setup };
        let doc = ctx.plan.map(|plan| plan.with(Clone::clone)).unwrap_or_default();
        let rows = ctx.rows.with(|rows| rows_of(&source, rows));
        Some(counts(&source, &setup, &doc, &rows))
    });
    let changed = Memo::new(move |_| before.with(|before| before.as_ref().map(|(program, _)| program.id.clone())) != program.get());
    let options = Memo::new(move |_| before.with(|before| before.as_ref().map(|(_, setup)| start_options(setup.now, &setup.leave, t)).unwrap_or_default()));
    let directions = Memo::new(move |_| source.with(|source| source.as_ref().and_then(|source| source.as_ref().map(direction_options)).unwrap_or_default()));
    let apply = move |_| {
        let (Some(mine), Some(Some(source)), Some(start)) = (ctx.mine, source.get_untracked(), start.get_untracked()) else { return };
        let (caption, page) = kept_plan(&source, direction.get_untracked());
        mine.set_study(&source.program.id, &program_name(&source.program), &caption, page.as_deref(), start);
        ctx.dialog.set(None);
    };
    let name_of = move || before.with(|before| before.as_ref().map(|(program, setup)| (program_name(program), setup.start.label(t.locale))));
    view! {
        <DialogHead ctx title=s.switch_title sub=s.switch_lead/>
        <div class="st-dlg-body st-switch">
            {move || name_of().map(|(name, since)| view! {
                <p class="st-before"><span class="label">{s.switch_before}</span><b>{name}</b><span>{(s.since)(&since)}</span></p>
            })}
            <div class="st-fields">
                <label class="st-field wide">
                    <span class="label">{s.new_program}</span>
                    <Combobox
                        id="st-switch-program"
                        label=s.program
                        placeholder=s.choose_program
                        search_placeholder=s.search_program
                        icon="graduation-cap"
                        min_width=420.0
                        items
                        selected=Signal::derive(move || program.get())
                        on_select=pick_program
                        clearable=false
                    />
                </label>
                <label class="st-field">
                    <span class="label">{s.start}</span>
                    <PlainSelect
                        id="st-switch-start"
                        label=s.start
                        options=Signal::derive(move || options.get())
                        chosen=Signal::derive(move || start.get().map(SemesterKey::key).unwrap_or_default())
                        pick=Callback::new(move |value: String| start.set(SemesterKey::parse(&value)))
                    />
                </label>
                {move || (!directions.with(Vec::is_empty)).then(|| view! {
                    <label class="st-field">
                        <span class="label">{s.direction}</span>
                        <PlainSelect
                            id="st-switch-direction"
                            label=s.direction
                            options=Signal::derive(move || directions.get())
                            chosen=Signal::derive(move || direction.get().to_string())
                            pick=Callback::new(move |value: String| direction.set(value.parse().unwrap_or(0)))
                        />
                    </label>
                })}
            </div>
            {move || preview.get().map(|counts| {
                let figure = |label: &'static str, (all, there): (f64, f64)| view! {
                    <div class="st-count">
                        <p class="label">{label}</p>
                        <p class="st-count-n"><b>{n(there, t)}</b>" "<span>{(s.of_credits)(&n(all, t))}</span></p>
                        <p class="st-sub">{s.counts_there}</p>
                    </div>
                };
                view! {
                    <section class="st-counts" aria-label=(s.counts_title)(&counts.name)>
                        <h3>{(s.counts_title)(&counts.name)}</h3>
                        <div class="st-count-row">
                            {figure(s.counts_passed, counts.passed)}
                            {figure(s.counts_planned, counts.planned)}
                        </div>
                        {(!counts.nowhere.is_empty()).then(|| view! {
                            <div class="st-nowhere">
                                <p>{s.not_there}</p>
                                <ul>
                                    {counts.nowhere.iter().map(|(name, credits, passed)| view! {
                                        <li><span>{name.clone()}</span><span class="st-sub">{format!("{} · {}", (s.credits)(&n(*credits, t)), if *passed { s.passed_word } else { s.planned_word })}</span></li>
                                    }).collect_view()}
                                </ul>
                            </div>
                        })}
                        <p class="st-safe"><Icon name="shield-check"/><span>{s.nothing_lost}</span></p>
                    </section>
                }
            })}
        </div>
        <footer class="st-dlg-foot">
            <button class="btn secondary" type="button" on:click=move |_| ctx.dialog.set(None)>{s.cancel}</button>
            <button class="btn primary" type="button" id="st-switch-go" disabled=move || source.with(|source| !matches!(source, Some(Some(_)))) on:click=apply>
                {move || {
                    let name = all.with(|all| all.iter().find(|p| Some(&p.id) == program.get().as_ref()).map(|p| p.name.clone()));
                    match (changed.get(), name) {
                        (true, Some(name)) => (s.switch_to)(&name),
                        _ => s.apply.to_string(),
                    }
                }}
            </button>
        </footer>
    }
}
