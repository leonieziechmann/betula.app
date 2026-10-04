//! The Gesamtplan (the mockup's concept C, owner 2026-10-04: „hat Potential, genial zu werden …
//! aber zu unsortiert und aufgeregt"): the areas as rows, the semesters as columns, calm — what is
//! planned as plain lines, what the plan still has open in grey where it fits next (one click adds
//! it), the semesters before the current one folded into „Bisher". A click into a cell offers the
//! modules of that area for that semester (`picker::CellDialog`); a click on a module opens its
//! menu; a module dragged into another column moves there. A desktop's view.

use folia_calendar::semester::SemesterKey;
use folia_plans::study::{Item, Line, Pick, When};
use leptos::prelude::*;

use folia_design::ui::Icon;

use super::focus::{add_pick, move_to};
use super::picker::short_title;
use super::{n, Dialog, Ready, StudyCtx, ViewSwitch};
use crate::i18n::{self, Texts};

/// A column of the grid: a semester, or the ones before the current one together.
#[derive(Clone, Debug, PartialEq)]
enum Column {
    Past { span: String, credits: String },
    Semester(Head),
}

/// The head of a semester's column.
#[derive(Clone, Debug, PartialEq)]
struct Head {
    key: SemesterKey,
    label: String,
    now: bool,
    fs: Option<String>,
    figure: String,
    heavy: bool,
    load: (f64, f64),
}

/// An area's row.
#[derive(Clone, Debug, PartialEq)]
struct AreaRow {
    area: Option<usize>,
    name: String,
    tone: &'static str,
    figure: String,
    over: bool,
    /// passed, planned, open, beyond need.
    bar: (f64, f64, f64, f64),
    /// What the semesters before the current one hold of it, folded: „✓ 32 LP · 5 Module".
    past: Option<String>,
}

/// What a cell lists.
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    key: String,
    name: String,
    full: String,
    credits: Option<String>,
    passed: bool,
    failed: bool,
    retake: bool,
    unoffered: bool,
    over: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct Layout {
    columns: Vec<Column>,
    rows: Vec<AreaRow>,
}

impl Layout {
    fn of(ready: &Ready, open_past: bool, t: &Texts) -> Self {
        let s = &t.study;
        let study = &ready.study;
        let past: Vec<_> = study.semesters.iter().filter(|semester| semester.when == When::Past).collect();
        let mut columns = Vec::new();
        let head = |semester: &folia_plans::study::Semester| {
            let heavy = semester.planned.is_some_and(|plan| semester.credits > plan + 0.5);
            let figure = match (semester.when, semester.planned) {
                (When::Past, _) => format!("{} / {}", n(semester.passed, t), (s.credits)(&n(semester.credits, t))),
                (_, Some(plan)) => format!("{} / {}", n(semester.credits, t), (s.credits)(&n(plan, t))),
                (_, None) => (s.credits)(&n(semester.credits, t)),
            };
            let load = match semester.when {
                When::Past => (semester.passed, semester.credits.max(1.0)),
                _ => (semester.credits, semester.planned.unwrap_or(semester.credits).max(semester.credits).max(1.0)),
            };
            Column::Semester(Head { key: semester.key, label: semester.key.short(t.locale), now: semester.when == When::Now, fs: ready.fs_label(semester.key, t), figure, heavy, load })
        };
        if !past.is_empty() {
            if open_past {
                columns.extend(past.iter().map(|semester| head(semester)));
            } else {
                let fs: Vec<u8> = past.iter().filter_map(|semester| semester.fs).collect();
                let span = match (fs.first(), fs.last()) {
                    (Some(from), Some(to)) => (s.fs_span)(*from, *to),
                    _ => String::new(),
                };
                let credits = (s.credits)(&n(past.iter().map(|semester| semester.passed).sum(), t));
                columns.push(Column::Past { span, credits });
            }
        }
        columns.extend(study.ahead().map(head));

        let past_of = |area: Option<usize>| {
            let items: Vec<&Item> = past.iter().flat_map(|semester| semester.items.iter()).filter(|item| item.area == area && item.passed).collect();
            (!items.is_empty()).then(|| (s.past_cell)(&n(items.iter().filter_map(|item| item.credits).sum(), t), &folia_design::format::modules(i64::try_from(items.len()).unwrap_or(0), t.locale)))
        };
        let mut rows: Vec<AreaRow> = study
            .progress
            .iter()
            .enumerate()
            .map(|(i, progress)| {
                let counted = progress.passed + progress.planned + progress.over;
                AreaRow {
                    area: Some(i),
                    name: ready.area_name(Some(i), t),
                    tone: ready.tone(Some(i)),
                    figure: format!("{}/{}", n(counted, t), n(progress.area.required, t)),
                    over: progress.over > 0.0,
                    bar: (progress.passed, progress.planned, progress.open(), progress.over),
                    past: past_of(Some(i)),
                }
            })
            .collect();
        let outside = study.semesters.iter().any(|semester| semester.items.iter().any(|item| item.area.is_none()));
        if outside || study.progress.is_empty() {
            let (passed, planned) = study.outside;
            rows.push(AreaRow { area: None, name: s.outside.to_string(), tone: "var(--text-3)", figure: n(passed + planned, t), over: false, bar: (passed, planned, 0.0, 0.0), past: past_of(None) });
        }
        Layout { columns, rows }
    }
}

#[component]
pub(super) fn Grid(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let open_past = RwSignal::new(false);
    let layout = Memo::new(move |_| ctx.with_ready(|ready| Layout::of(ready, open_past.get(), t)));
    let any_past = Memo::new(move |_| ctx.with_ready(|ready| ready.study.semesters.iter().any(|semester| semester.when == When::Past)).unwrap_or(false));
    view! {
        <section class="panel st-grid-panel" aria-label=s.grid_title>
            <div class="st-sems-head">
                <h2>{s.grid_title}</h2>
                <span class="st-grow"></span>
                {move || any_past.get().then(|| view! {
                    <button class="st-link" type="button" aria-expanded=move || if open_past.get() { "true" } else { "false" } on:click=move |_| open_past.update(|open| *open = !*open)>
                        {move || if open_past.get() { s.collapse_past } else { s.expand_past }}
                    </button>
                })}
                <ViewSwitch ctx/>
            </div>
            {move || layout.get().map(|layout| {
                let past_column = layout.columns.iter().any(|column| matches!(column, Column::Past { .. }));
                let semesters = layout.columns.len() - usize::from(past_column);
                let template = format!("grid-template-columns: 172px {}repeat({semesters}, minmax(150px, 1fr))", if past_column { "132px " } else { "" });
                let columns = layout.columns.clone();
                view! {
                    <div class="st-grid-scroll">
                        <div class="st-grid" style=template>
                            <div class="st-grid-corner"></div>
                            {columns.iter().cloned().map(|column| match column {
                                Column::Past { span, credits } => view! {
                                    <div class="st-col-head past"><div class="st-col-name">{s.so_far}</div><div class="st-sub">{span}" · "<b class="ok">{credits}</b></div></div>
                                }.into_any(),
                                Column::Semester(head) => {
                                    let key = head.key;
                                    view! {
                                        <button type="button" class="st-col-head" class:is-now=head.now on:click=move |_| {
                                            ctx.focus.set(Some(key));
                                            ctx.view.set(super::View::Semester);
                                        }>
                                            <div class="st-col-name">{head.label}{head.now.then(|| view! { <span class="st-now">{s.now}</span> })}</div>
                                            <div class="st-sub"><span>{head.fs}</span><b class:heavy=head.heavy>{head.figure}</b></div>
                                            <span class="st-load"><span style=format!("flex-grow: {}", head.load.0)></span><span style=format!("flex-grow: {}", (head.load.1 - head.load.0).max(0.0))></span></span>
                                        </button>
                                    }.into_any()
                                }
                            }).collect_view()}
                            {layout.rows.into_iter().map(|row| {
                                let (passed, planned, open, over) = row.bar;
                                let area = row.area;
                                view! {
                                    <button type="button" class="st-row-head" style=format!("--c: {}", row.tone) on:click=move |_| ctx.open(Dialog::Area(area))>
                                        <span class="st-card-head"><span class="st-dot"></span><span class="st-card-name">{row.name.clone()}</span><span class="st-card-lp" class:over=row.over>{row.figure.clone()}</span></span>
                                        <span class="st-mini">
                                            <span class="passed" style=format!("flex-grow: {passed}")></span>
                                            <span class="planned" style=format!("flex-grow: {planned}")></span>
                                            <span class="open" style=format!("flex-grow: {open}")></span>
                                            <span class="over" style=format!("flex-grow: {over}")></span>
                                        </span>
                                    </button>
                                    {columns.iter().cloned().map(|column| match column {
                                        Column::Past { .. } => view! { <div class="st-cell past">{row.past.clone().unwrap_or_else(|| "–".to_string())}</div> }.into_any(),
                                        Column::Semester(head) => view! { <Cell ctx area semester=head.key now=head.now/> }.into_any(),
                                    }).collect_view()}
                                }
                            }).collect_view()}
                        </div>
                    </div>
                }
            })}
            <p class="st-grid-legend">
                <span><Icon name="plus"/>{s.grid_grey}</span>
                <span><Icon name="repeat"/>{s.legend_retake.0}</span>
                <span class="warn"><Icon name="triangle-alert"/>{s.legend_offer.0}</span>
                <span><b class="over">"+"</b>{s.legend_over.0}</span>
                <span class="st-grow"></span>
                <span>{s.grid_help}</span>
            </p>
        </section>
    }
}

/// A cell: what area `area` holds in semester `semester`, what the plan has open there (grey, a
/// click adds it), and „Modul hinzufügen".
#[component]
fn Cell(ctx: StudyCtx, area: Option<usize>, semester: SemesterKey, now: bool) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let entries = Memo::new(move |_| {
        ctx.with_ready(|ready| {
            let items = ready.study.semester(semester).map(|semester| semester.items.clone()).unwrap_or_default();
            items
                .into_iter()
                .filter(|item| item.area == area)
                .map(|item| Entry {
                    key: item.key(),
                    name: short_title(&item.name).to_string(),
                    full: item.name.clone(),
                    credits: item.credits.map(|credits| n(credits, t)),
                    passed: item.passed,
                    failed: item.failed,
                    retake: item.retake.is_some(),
                    unoffered: item.unoffered && !item.passed,
                    over: item.over > 0.0,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
    });
    let lines = Memo::new(move |_| ctx.with_ready(|ready| ready.lines.iter().filter(|line| line.suggestion.area == area && line.semester == semester).cloned().collect::<Vec<Line>>()).unwrap_or_default());
    let over = RwSignal::new(false);
    let drop = move |ev: leptos::ev::DragEvent| {
        ev.prevent_default();
        over.set(false);
        if let Some((from, key)) = ctx.drag.get_untracked() {
            ctx.drag.set(None);
            move_to(ctx, from, semester, &key, t);
        }
    };
    view! {
        <div
            class="st-cell"
            class:is-now=now
            class:is-drop=move || over.get()
            on:dragover=move |ev: leptos::ev::DragEvent| if ctx.drag.with_untracked(|drag| drag.as_ref().is_some_and(|(from, _)| *from != semester)) { ev.prevent_default(); over.set(true); }
            on:dragleave=move |_| over.set(false)
            on:drop=drop
        >
            <For each=move || entries.get() key=|entry| (entry.key.clone(), entry.passed, entry.failed, entry.over, entry.unoffered) children=move |entry: Entry| {
                let key = entry.key.clone();
                let drag_key = entry.key.clone();
                let title = format!("{}{}", entry.full, entry.credits.as_ref().map(|credits| format!(" · {}", (s.credits)(credits))).unwrap_or_default());
                view! {
                    <button
                        type="button"
                        class="st-entry"
                        class:passed=entry.passed
                        class:failed=entry.failed
                        title=title
                        draggable="true"
                        on:dragstart=move |ev: leptos::ev::DragEvent| {
                            ctx.drag.set(Some((semester, drag_key.clone())));
                            #[cfg(feature = "csr")]
                            if let Some(data) = ev.data_transfer() {
                                let _ = data.set_data("text/plain", &drag_key);
                                data.set_effect_allowed("move");
                            }
                            #[cfg(not(feature = "csr"))]
                            let _ = ev;
                        }
                        on:dragend=move |_| ctx.drag.set(None)
                        on:click=move |_| ctx.open(Dialog::Item { semester, key: key.clone() })
                    >
                        {entry.passed.then(|| view! { <Icon name="check" class="ok"/> })}
                        <span class="st-entry-name">{entry.name}</span>
                        {entry.retake.then(|| view! { <Icon name="repeat"/> })}
                        {entry.unoffered.then(|| view! { <Icon name="triangle-alert" class="warn"/> })}
                        {entry.over.then(|| view! { <b class="over">"+"</b> })}
                        <span class="st-lp">{entry.credits}</span>
                    </button>
                }
            }/>
            {move || lines.get().into_iter().map(|line| {
                let title = match line.failed_in {
                    Some(failed) => (s.line_retake)(&failed.label(t.locale)),
                    None => (s.line_title)(&line.plan_fs.map(s.fs).unwrap_or_default()),
                };
                let pick: Pick = line.suggestion.pick.clone();
                let credits = line.suggestion.credits_text.clone().map(|text| folia_plans::plan::credits_in(&text, t.locale)).or_else(|| line.suggestion.credits.map(|credits| n(credits, t)));
                let unoffered = !line.suggestion.offer.offered(semester);
                view! {
                    <button type="button" class="st-entry line" title=title.clone() aria-label=format!("{} – {title}", line.suggestion.name) on:click=move |_| add_pick(ctx, semester, vec![pick.clone()], t)>
                        <Icon name=if line.failed_in.is_some() { "repeat" } else { "plus" }/>
                        <span class="st-entry-name">{short_title(&line.suggestion.name).to_string()}</span>
                        {unoffered.then(|| view! { <Icon name="triangle-alert" class="warn"/> })}
                        <span class="st-lp">{credits}</span>
                    </button>
                }
            }).collect_view()}
            <button type="button" class="st-cell-add" on:click=move |_| ctx.open(Dialog::Cell { area, semester })><Icon name="plus"/>{s.add_in_cell}</button>
        </div>
    }
}
