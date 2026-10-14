//! „Regelstudienplan", a view of „Mein Studium" (`/study?view=plan`; owner, 2026-10-14, concept A
//! „Ansichten": „Regelstudienplan und Wahlpflicht werden Ansichten von „Mein Studium" – mit deinem
//! Stand darin"). The plan as its regulation prints it, each row with where it stands in the study
//! (`folia_plans::choices::plan_view`): passed, planned, taken again, not passed, open.
//!
//! On a desktop a table under the overview: a row a row of the plan, grouped by the areas of the
//! progress and in their colours; a column each Fachsemester (the semester it is for the student
//! under it, the current one marked); the credits in the cell of the row's Fachsemester, in the
//! look of where it stands, and where the student has it in another semester a cell there too; at
//! the end „Bei dir": the semester it is passed or planned in, else what can be done — „Einplanen"
//! into the semester it fits next, „Wählen" for a choice (the Wahlpflicht). „Mein Stand" switches
//! the stand off: the plan as printed. The foot: what the plan puts into each Fachsemester, and
//! what the student has there.
//!
//! On a phone a page of its own behind the overview (the way back at the head, `chrome::TopBack`):
//! the Fachsemester as sections, a strip of them at the top to jump to, each row a card with its
//! stand.

use folia_calendar::semester::SemesterKey;
use folia_model::labels::Labelled;
use folia_plans::choices::{self, PlanLine, RowState};
use folia_plans::study::{self, Input, Pick, When};
use folia_routes::url::{LocalView, ProgramTab, ProgramUrl, StudyUrl, StudyView};
use leptos::prelude::*;

use folia_design::ui::{EmptyState, Icon};
use folia_shell::chrome::{Back, TopBack};

use super::{n, now_secs, Ready, StudyCtx};
use crate::i18n::{self, Texts};

/// What a row offers to do.
#[derive(Clone, Debug, PartialEq)]
enum Act {
    None,
    /// „Einplanen" into that semester: the module of the row.
    Plan(String, SemesterKey),
    /// „Wählen": the choice (an index of `PlanView::choices`), in the Wahlpflicht.
    Choose(usize),
}

/// A row as the view draws it.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    key: String,
    name: String,
    module_id: Option<String>,
    credits: Option<String>,
    kind: Option<&'static str>,
    first: u8,
    last: u8,
    tone: &'static str,
    area: String,
    state: RowState,
    /// The Fachsemester the student has it in, where that is not the plan's.
    moved: Option<u8>,
    /// Where it stands, in words: „bestanden · WiSe 25/26".
    stand: Option<String>,
    /// The semester of it, short: „WiSe 25/26".
    when: Option<String>,
    /// What fills it, or what is taken of its choice: „mit Proseminar", „8 von 12 LP gewählt".
    note: Option<String>,
    act: Act,
}

impl Row {
    /// The class of its cells: where it stands.
    fn class(&self) -> &'static str {
        match self.state {
            RowState::Passed(_) => "is-passed",
            RowState::Planned(_) => "is-planned",
            RowState::Again { .. } => "is-again",
            RowState::Failed(_) => "is-failed",
            RowState::Open => "is-open",
        }
    }
}

/// An area of the progress with its rows.
#[derive(Clone, Debug, PartialEq)]
struct Group {
    name: String,
    tone: &'static str,
    /// „38/66": what is passed or planned of what it asks.
    figure: String,
    rows: Vec<Row>,
}

/// A Fachsemester as a column.
#[derive(Clone, Debug, PartialEq)]
struct Column {
    fs: u8,
    /// The semester it is for the student, short: „WiSe 25/26".
    when: Option<String>,
    now: bool,
}

/// A Fachsemester, or several the plan's rows span together, as a phone's section.
#[derive(Clone, Debug, PartialEq)]
struct Section {
    id: String,
    /// „3. FS", „5. und 6. FS".
    title: String,
    /// What the strip says: „3. FS", „5.–6.".
    chip: String,
    /// „WiSe 26/27", „WiSe 27/28 – SoSe 28".
    when: Option<String>,
    now: bool,
    /// Over: all of it passed („✓" in the strip).
    done: bool,
    /// „24 von 32 bestanden", „32 von 30 geplant".
    stand: String,
    rows: Vec<Row>,
}

/// What the view shows.
#[derive(Clone, Debug, PartialEq)]
struct Sheet {
    /// „Informatik B.Sc. · PO 2008 · 180 LP".
    program: String,
    /// The study direction, where the program has several plans.
    direction: Option<String>,
    columns: Vec<Column>,
    groups: Vec<Group>,
    /// The foot: what the plan puts into the Fachsemester (a span where it sums several together),
    /// and what the student has there.
    plan_foot: Vec<(usize, Option<String>)>,
    mine_foot: Vec<(usize, String, &'static str)>,
    /// „52 LP bestanden", „44 LP geplant".
    mine_total: (String, String),
    sections: Vec<Section>,
    /// On a phone under the head: what is passed of the Fachsemester before the current one, and
    /// a retake.
    so_far: Option<String>,
    /// The plan on the program's page.
    source: String,
}

/// „WiSe 25/26".
fn short(s: SemesterKey, t: &Texts) -> String {
    s.short(t.locale)
}

impl Sheet {
    fn of(input: &Input, ready: &Ready, t: &'static Texts) -> Option<Self> {
        let v = &t.study_views;
        let study = &ready.study;
        let core = input.core?;
        let view = choices::plan_view(input, study);
        if view.lines.is_empty() {
            return None;
        }
        let fs_of = |s: SemesterKey| study.semester(s).and_then(|semester| semester.fs);
        let row_of = |line: &PlanLine| -> Row {
            let module_id = line.module_id.clone();
            let when = line.state.semester().or(match line.state {
                RowState::Failed(s) => Some(s),
                _ => None,
            });
            let moved = line.state.semester().and_then(fs_of).filter(|fs| !(line.first..=line.last).contains(fs));
            let stand = match line.state {
                RowState::Passed(s) => Some((v.state_passed)(&short(s, t))),
                RowState::Planned(s) => Some((v.state_planned)(&short(s, t))),
                RowState::Again { planned, .. } => Some((v.state_again)(&short(planned, t))),
                RowState::Failed(s) => Some((v.state_failed)(&short(s, t))),
                RowState::Open => None,
            };
            let choice = line.choice.and_then(|at| view.choices.get(at).map(|choice| (at, choice)));
            let note = if !line.fillers.is_empty() {
                Some((v.filled_by)(&line.fillers.iter().map(|filler| filler.name.as_str()).collect::<Vec<_>>().join(", ")))
            } else {
                choice.filter(|(_, choice)| !choice.taken.is_empty() && matches!(line.state, RowState::Open | RowState::Failed(_))).map(|(_, choice)| {
                    (v.choice_taken)(&n(choice.taken_credits(), t), &folia_plans::plan::credits_in(&choice.credits_text, t.locale))
                })
            };
            // What can be done: a module open or not passed into the semester it fits next; a
            // choice in the Wahlpflicht.
            let act = match (&module_id, line.state, choice) {
                (Some(id), RowState::Open | RowState::Failed(_), _) => {
                    let next = ready.lines.iter().find(|open| matches!(&open.suggestion.pick, Pick::Module { id: other, .. } if other == id)).map(|open| open.semester);
                    let fallback = line.offer.next(study.now);
                    Act::Plan(id.clone(), next.unwrap_or(if study.semester(fallback).is_some() { fallback } else { study.now }))
                }
                (None, RowState::Open | RowState::Failed(_), Some((at, _))) => Act::Choose(at),
                _ => Act::None,
            };
            Row {
                key: format!("{}:{}", line.caption, line.ord),
                name: line.name.clone(),
                credits: line.credits_text.as_deref().map(|text| folia_plans::plan::credits_in(text, t.locale)).or_else(|| line.credits.map(|credits| n(credits, t))),
                kind: line.kind.map(|kind| kind.label(t.locale)),
                first: line.first,
                last: line.last,
                tone: ready.tone(line.area),
                area: ready.area_name(line.area, t),
                state: line.state,
                moved,
                stand,
                when: when.map(|s| short(s, t)),
                note,
                act,
                module_id,
            }
        };
        let rows: Vec<Row> = view.lines.iter().map(row_of).collect();

        // The groups: the areas of the progress, in their order.
        let mut groups: Vec<Group> = study
            .progress
            .iter()
            .enumerate()
            .map(|(at, progress)| Group {
                name: ready.named.get(at).map_or_else(String::new, |named| named.full.clone()),
                tone: ready.tone(Some(at)),
                figure: format!("{}/{}", n(progress.passed + progress.planned + progress.over, t), n(progress.area.required, t)),
                rows: Vec::new(),
            })
            .collect();
        let mut loose = Group { name: t.study.other.to_string(), tone: "var(--text-3)", figure: String::new(), rows: Vec::new() };
        for (line, row) in view.lines.iter().zip(&rows) {
            match line.area.and_then(|at| groups.get_mut(at)) {
                Some(group) => group.rows.push(row.clone()),
                None => loose.rows.push(row.clone()),
            }
        }
        groups.push(loose);
        groups.retain(|group| !group.rows.is_empty());

        // The columns: every Fachsemester of the plan.
        let last = view.lines.iter().map(|line| line.last).max().unwrap_or(1);
        let semester_of = |fs: u8| study::semester_of_fs(study.start, fs, &ready.setup.leave);
        let columns: Vec<Column> = (1..=last).map(|fs| Column { fs, when: semester_of(fs).map(|s| short(s, t)), now: semester_of(fs) == Some(study.now) }).collect();

        // The foot: the plan's sums, a span where it sums several Fachsemester together; what the
        // student has in each (passed in one that is over, else passed and planned).
        let mut plan_foot = Vec::new();
        let mut spans: Vec<(u8, u8)> = Vec::new();
        let mut fs = 1u8;
        while fs <= last {
            if let Some(credits) = core.semester_credits(i64::from(fs)) {
                plan_foot.push((1, Some(n(credits, t))));
                spans.push((fs, fs));
                fs += 1;
                continue;
            }
            let together = (fs + 1..=last).find_map(|to| core.stated_for(i64::from(fs), i64::from(to)).map(|(credits, _)| (to, credits)));
            match together {
                Some((to, credits)) => {
                    plan_foot.push((usize::from(to - fs) + 1, Some(n(credits, t))));
                    spans.push((fs, to));
                    fs = to + 1;
                }
                None => {
                    plan_foot.push((1, None));
                    spans.push((fs, fs));
                    fs += 1;
                }
            }
        }
        let held = |from: u8, to: u8| -> (f64, f64, bool) {
            let semesters: Vec<_> = study.semesters.iter().filter(|semester| semester.fs.is_some_and(|fs| fs >= from && fs <= to)).collect();
            let passed: f64 = semesters.iter().map(|semester| semester.passed).sum();
            let planned: f64 = semesters.iter().flat_map(|semester| semester.items.iter()).filter(|item| !item.passed && !item.failed).filter_map(|item| item.credits).sum();
            let over = !semesters.is_empty() && semesters.iter().all(|semester| semester.when == When::Past);
            (passed, planned, over)
        };
        let mine_foot = spans
            .iter()
            .map(|(from, to)| {
                let (passed, planned, over) = held(*from, *to);
                let span = usize::from(to - from) + 1;
                match (over, passed + planned > 0.0) {
                    (true, _) => (span, n(passed, t), "ok"),
                    (false, true) => (span, n(passed + planned, t), "plan"),
                    (false, false) => (span, "–".to_string(), ""),
                }
            })
            .collect();
        let mine_total = (
            (v.foot_passed)(&(t.study.credits)(&n(study.passed + study.over_passed + study.outside.0, t))),
            (v.foot_planned)(&(t.study.credits)(&n(study.planned + study.over - study.over_passed + study.outside.1, t))),
        );

        // A phone's sections: the rows by their first Fachsemester; rows over several together
        // where all of a section's do.
        let mut sections: Vec<Section> = Vec::new();
        let mut firsts: Vec<u8> = rows.iter().map(|row| row.first).collect();
        firsts.sort_unstable();
        firsts.dedup();
        for first in firsts {
            let of: Vec<Row> = rows.iter().filter(|row| row.first == first).cloned().collect();
            let to = of.iter().map(|row| row.last).min().unwrap_or(first).max(first);
            let together = to > first && of.iter().all(|row| row.last == to);
            let (title, chip, span) = if together {
                ((t.study.fs_span)(first, to), format!("{first}.–{to}."), (first, to))
            } else {
                ((t.study.fs)(first), (t.study.fs)(first), (first, first))
            };
            let semesters: Vec<SemesterKey> = (span.0..=span.1).filter_map(semester_of).collect();
            let when = match (semesters.first(), semesters.last()) {
                (Some(a), Some(b)) if a != b => Some(format!("{} – {}", short(*a, t), short(*b, t))),
                (Some(a), _) => Some(short(*a, t)),
                _ => None,
            };
            let now = semesters.contains(&study.now);
            let over = !semesters.is_empty() && semesters.iter().all(|s| *s < study.now);
            let sum = match span {
                (a, b) if a == b => core.semester_credits(i64::from(a)),
                (a, b) => core.stated_for(i64::from(a), i64::from(b)).map(|(credits, _)| credits),
            }
            .unwrap_or_else(|| view.lines.iter().filter(|line| line.first == first).filter_map(|line| line.credits).sum());
            let credits_of = |pick: &dyn Fn(&RowState) -> bool| -> f64 { view.lines.iter().filter(|line| line.first == first && pick(&line.state)).filter_map(|line| line.credits).sum() };
            let passed = credits_of(&|state| matches!(state, RowState::Passed(_)));
            let planned = credits_of(&|state| matches!(state, RowState::Passed(_) | RowState::Planned(_) | RowState::Again { .. }));
            let stand = if over {
                (v.section_passed)(&n(passed, t), &(t.study.credits)(&n(sum, t)))
            } else if planned > 0.0 {
                (v.section_planned)(&n(planned, t), &(t.study.credits)(&n(sum, t)))
            } else {
                (v.section_plain)(&n(sum, t))
            };
            let done = over && of.iter().all(|row| matches!(row.state, RowState::Passed(_)));
            sections.push(Section { id: format!("st-fs-{first}"), title, chip, when, now, done, stand, rows: of });
        }

        // What is passed of the Fachsemester before the current one, and the first retake.
        let current = study.semester(study.now).and_then(|semester| semester.fs);
        let so_far = current.filter(|fs| *fs > 1).map(|fs| {
            let before: Vec<&PlanLine> = view.lines.iter().filter(|line| line.last < fs).collect();
            let all: f64 = before.iter().filter_map(|line| line.credits).sum();
            let passed: f64 = before.iter().filter(|line| matches!(line.state, RowState::Passed(_))).filter_map(|line| line.credits).sum();
            let mut text = (v.so_far)(&(t.study.fs_span)(1, fs - 1), &n(passed, t), &n(all, t));
            let retake = view.lines.iter().find_map(|line| match line.state {
                RowState::Again { planned, .. } => Some((v.retake_line)(&line.name, &short(planned, t))),
                RowState::Failed(_) => Some((v.retake_open)(&line.name)),
                _ => None,
            });
            if let Some(retake) = retake {
                text.push_str(" · ");
                text.push_str(&retake);
            }
            text
        });

        let shown = ready.setup.shown.map_or(1, |shown| shown + 1);
        let direction = ready.setup.shown.and_then(|shown| ready.plans.iter().find(|(index, _)| *index == shown)).map(|(_, label)| label.clone());
        let program = format!("{} {} · PO {} · {}", ready.program.name, ready.program.degree(), folia_stores::myprogram::po_of(&ready.program), (t.study.credits)(&n(study.total, t)));
        Some(Sheet {
            program,
            direction,
            columns,
            groups,
            plan_foot,
            mine_foot,
            mine_total,
            sections,
            so_far,
            source: ProgramUrl::new(&ready.program.slug, ProgramTab::Plan).with_variant(shown).path(),
        })
    }
}

/// Plans the module `id` into semester `s`, with a note that „Rückgängig" takes it back by.
fn plan_into(ctx: StudyCtx, id: String, name: String, s: SemesterKey, t: &'static Texts) {
    let note = (t.study_views.planned_note)(&name, &s.label(t.locale));
    ctx.change_noted(
        Some(note),
        move |doc, input| {
            study::add(doc, input, s, &[Pick::Module { id, from_plan: true }], now_secs());
        },
        || {},
    );
}

/// The address of the Wahlpflicht.
fn choices_href(ctx: StudyCtx, t: &Texts) -> String {
    t.path(&ctx.url.with_untracked(|url| url.with_view(StudyView::Choices).with_open(None).path()))
}

/// What a row offers: „Einplanen", „Wählen ›".
fn act_view(ctx: StudyCtx, row: &Row, t: &'static Texts) -> AnyView {
    let v = &t.study_views;
    match row.act.clone() {
        Act::Plan(id, s) => {
            let name = row.name.clone();
            let title = (v.plan_into)(&s.label(t.locale));
            view! {
                <button class="st-addb" type="button" title=title on:click=move |_| plan_into(ctx, id.clone(), name.clone(), s, t)>
                    <Icon name="plus"/>{v.plan_it}
                </button>
            }
            .into_any()
        }
        Act::Choose(at) => view! {
            <a class="st-addb" href=choices_href(ctx, t) on:click=move |_| ctx.choice.set(Some(at))>
                {v.choose}<Icon name="chevron-right"/>
            </a>
        }
        .into_any(),
        Act::None => ().into_any(),
    }
}

/// A row's name: a link that opens its module beside the page.
fn name_view(ctx: StudyCtx, row: &Row, t: &Texts) -> AnyView {
    match row.module_id.clone() {
        Some(id) => view! { <a class="st-rsp-name" href=super::module_href(ctx, &id, t) data-noscroll="">{row.name.clone()}</a> }.into_any(),
        None => view! { <span class="st-rsp-name is-slot">{row.name.clone()}</span> }.into_any(),
    }
}

/// The view: a table on a desktop, the Fachsemester on a phone.
#[component]
pub(super) fn Regular(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    view_back(ctx);
    let sheet = Memo::new(move |_| ctx.with_input(|input, ready| Sheet::of(input, ready, t)).flatten());
    move || {
        let phone = ctx.phone.get();
        match sheet.get() {
            None => view! {
                <section class="panel">
                    <EmptyState title=t.study_views.no_plan hint=t.study_views.no_plan_hint/>
                </section>
            }
            .into_any(),
            Some(sheet) if phone => view! { <PlanPhone ctx sheet/> }.into_any(),
            Some(sheet) => view! { <PlanTable ctx sheet/> }.into_any(),
        }
    }
}

/// The table on a desktop.
#[component]
fn PlanTable(ctx: StudyCtx, sheet: Sheet) -> impl IntoView {
    let t = i18n::t();
    let v = &t.study_views;
    let mine = RwSignal::new(true);
    let width = sheet.columns.len() + 3;
    let columns = sheet.columns.clone();
    let lead = match &sheet.direction {
        Some(direction) => format!("{} · {direction}", v.plan_lead),
        None => v.plan_lead.to_string(),
    };
    let cells = move |row: &Row| -> Vec<AnyView> {
        let mut out = Vec::new();
        let mut fs = 1u8;
        let last = columns.len() as u8;
        while fs <= last {
            let now = columns.get(usize::from(fs - 1)).is_some_and(|column| column.now);
            if fs == row.first {
                let span = row.last.saturating_sub(row.first).min(last - fs) + 1;
                let class = format!("st-rsp-c {}{}", row.class(), if span > 1 { " is-span" } else { "" });
                let credits = row.credits.clone().unwrap_or_else(|| "·".to_string());
                out.push(view! { <td class="fs" class:now=now colspan=span><span class=class style=format!("--c: {}", row.tone)>{credits}</span></td> }.into_any());
                fs += span;
            } else if Some(fs) == row.moved {
                out.push(view! { <td class="fs" class:now=now><span class="st-rsp-c is-moved" style=format!("--c: {}", row.tone)>{row.credits.clone()}</span></td> }.into_any());
                fs += 1;
            } else {
                out.push(view! { <td class="fs" class:now=now></td> }.into_any());
                fs += 1;
            }
        }
        out
    };
    let mine_cell = move |row: &Row| -> AnyView {
        let when = row.when.clone().unwrap_or_default();
        let body = match row.state {
            RowState::Passed(_) => view! { <span class="st-mine-c ok"><Icon name="check"/>{when}</span> }.into_any(),
            RowState::Planned(_) => view! { <span class="st-mine-c plan">{when}</span> }.into_any(),
            RowState::Again { .. } => view! { <span class="st-mine-c warn"><Icon name="repeat"/>{row.stand.clone()}</span> }.into_any(),
            RowState::Failed(_) => view! { <span class="st-mine-c warn">{row.stand.clone()}</span>{act_view(ctx, row, t)} }.into_any(),
            RowState::Open => act_view(ctx, row, t),
        };
        view! { <div class="st-mine-cell">{body}</div> }.into_any()
    };
    view! {
        <section class="panel st-rsp" class:plain=move || !mine.get() aria-labelledby="st-rsp-title">
            <header class="st-view-head">
                <div>
                    <h2 id="st-rsp-title">{v.plan}</h2>
                    <p>{lead}" · "{sheet.program.clone()}</p>
                </div>
                <button class="st-toggle" type="button" aria-pressed=move || if mine.get() { "true" } else { "false" } on:click=move |_| mine.update(|on| *on = !*on)>
                    <i aria-hidden="true"></i>{v.mine}
                </button>
            </header>
            <div class="table-scroll">
                <table class="st-mx" style=format!("--fs: {}", sheet.columns.len())>
                    <colgroup>
                        <col class="c-name"/>
                        <col class="c-kind"/>
                        <col class="c-fs" span=sheet.columns.len()/>
                        <col class="c-mine"/>
                    </colgroup>
                    <thead>
                        <tr>
                            <th scope="col" class="nm">{v.col_module}</th>
                            <th scope="col" class="st-kindc">{v.col_kind}</th>
                            {sheet.columns.iter().map(|column| view! {
                                <th scope="col" class="fs" class:now=column.now>
                                    <b>{column.fs}</b>
                                    {column.when.clone().map(|when| view! { <small>{when}</small> })}
                                    {column.now.then(|| view! { <small class="st-now-w">{t.study.now}</small> })}
                                </th>
                            }).collect_view()}
                            <th scope="col" class="st-minec">{move || if mine.get() { v.col_mine } else { v.col_credits }}</th>
                        </tr>
                    </thead>
                    {sheet.groups.into_iter().map(|group| {
                        let cells = cells.clone();
                        view! {
                            <tbody>
                                <tr class="g">
                                    <th colspan=width scope="rowgroup">
                                        <span class="st-g" style=format!("--c: {}", group.tone)><span class="st-dot"></span><b>{group.name}</b><span class="st-g-n">{group.figure}</span></span>
                                    </th>
                                </tr>
                                {group.rows.into_iter().map(|row| {
                                    let credits = row.credits.clone();
                                    let (class, key) = (format!("st-rsp-row {}", row.class()), row.key.clone());
                                    view! {
                                        <tr class=class data-key=key>
                                            <th scope="row" class="nm">
                                                {name_view(ctx, &row, t)}
                                                {row.note.clone().map(|note| view! { <small><Icon name="arrow-right"/>{note}</small> })}
                                            </th>
                                            <td class="st-kindc">{row.kind}</td>
                                            {cells(&row)}
                                            <td class="st-minec">
                                                {move || if mine.get() { mine_cell(&row) } else { view! { <span class="num">{credits.clone()}</span> }.into_any() }}
                                            </td>
                                        </tr>
                                    }
                                }).collect_view()}
                            </tbody>
                        }
                    }).collect_view()}
                    <tfoot>
                        <tr>
                            <th scope="row" class="nm">{v.per_plan}</th>
                            <td class="st-kindc"></td>
                            {sheet.plan_foot.into_iter().map(|(span, sum)| view! { <td class="fs c" colspan=span>{sum}</td> }).collect_view()}
                            <td class="st-minec"></td>
                        </tr>
                        <tr class="st-foot-mine">
                            <th scope="row" class="nm">{v.per_mine}</th>
                            <td class="st-kindc"></td>
                            {sheet.mine_foot.into_iter().map(|(span, text, class)| view! { <td class="fs c" colspan=span><span class=format!("st-mine-c {class}")>{text}</span></td> }).collect_view()}
                            <td class="st-minec"><span class="st-mine-c st-mine-sum"><span>{sheet.mine_total.0.clone()}</span><span>{sheet.mine_total.1.clone()}</span></span></td>
                        </tr>
                    </tfoot>
                </table>
            </div>
            <p class="hint st-rsp-source"><Icon name="file-check-2"/><a href=t.path(&sheet.source)>{v.plan_source}</a></p>
        </section>
    }
}

/// The way back at the head of a view on a phone: to the overview, through the history where it is
/// the step before. Set once for the view, whatever it draws meanwhile, and taken away with it.
pub(super) fn view_back(ctx: StudyCtx) {
    let t = i18n::t();
    let back = TopBack::expect();
    Effect::new(move |_| {
        let phone = ctx.phone.get();
        let history = ctx.view_back.get();
        if let Some(back) = back {
            back.set(phone.then(|| Back { href: StudyUrl::default().path(), label: t.study_views.to_overview, history }));
        }
    });
    on_cleanup(move || {
        if let Some(back) = back {
            back.set(None);
        }
    });
}

/// The Fachsemester on a phone.
#[component]
fn PlanPhone(ctx: StudyCtx, sheet: Sheet) -> impl IntoView {
    let t = i18n::t();
    let v = &t.study_views;
    view! {
        <div class="st-phone-page st-rsp-phone">
            <header class="st-vhead">
                <h1>{v.plan}</h1>
                <p>{sheet.program.clone()}{sheet.direction.clone().map(|direction| format!(" · {direction}"))}</p>
            </header>
            <nav class="st-fsidx" aria-label=v.fs_strip>
                {sheet.sections.iter().map(|section| view! {
                    <a href=format!("#{}", section.id) data-action="jump" class:now=section.now class:done=section.done>
                        {section.chip.clone()}
                        {section.done.then(|| view! { <Icon name="check"/> })}
                        {section.now.then(|| format!(" · {}", t.study.now))}
                    </a>
                }).collect_view()}
            </nav>
            {sheet.so_far.clone().map(|line| view! { <p class="st-soll"><Icon name="circle-check-big"/><span>{line}</span></p> })}
            {sheet.sections.into_iter().map(|section| view! {
                <section class="st-fs-sec" id=section.id.clone() aria-label=section.title.clone()>
                    <h2 class="st-sech">
                        <span>{section.title.clone()}</span>
                        {section.now.then(|| view! { <span class="st-now">{t.study.now}</span> })}
                        {section.when.clone().map(|when| view! { <small>{when}</small> })}
                        <span class="ln"></span>
                        <small>{section.stand.clone()}</small>
                    </h2>
                    <ul class="st-pcards">
                        {section.rows.into_iter().map(|row| view! { <li>{plan_card(ctx, row, t)}</li> }).collect_view()}
                    </ul>
                </section>
            }).collect_view()}
            <p class="hint st-rsp-source"><Icon name="file-check-2"/><a href=t.path(&sheet.source)>{v.plan_source}</a></p>
        </div>
    }
}

/// A row of the plan as a phone's card: its name, its number and area, its credits, where it
/// stands, what it offers.
fn plan_card(ctx: StudyCtx, row: Row, t: &'static Texts) -> AnyView {
    let class = row.class();
    let chip = row.stand.clone().map(|stand| {
        let (icon, kind) = match row.state {
            RowState::Passed(_) => (Some("check"), "ok"),
            RowState::Again { .. } => (Some("repeat"), "warn"),
            RowState::Failed(_) => (Some("triangle-alert"), "warn"),
            _ => (None, ""),
        };
        view! { <span class=format!("st-chip {kind}")>{icon.map(|icon| view! { <Icon name=icon/> })}{stand}</span> }
    });
    let act = act_view(ctx, &row, t);
    view! {
        <div class=format!("st-pcard {class}") style=format!("--c: {}", row.tone)>
            <div class="st-pcard-t">
                {name_view(ctx, &row, t)}
                <span class="st-meta">
                    {row.module_id.clone().map(|id| view! { <span class="st-code mono">{id}</span> })}
                    <span class="st-area"><span class="st-dot"></span>{row.area.clone()}</span>
                </span>
                {row.note.clone().map(|note| view! { <span class="st-pcard-note"><Icon name="arrow-right"/>{note}</span> })}
                {(chip.is_some() || !matches!(row.act, Act::None)).then(|| view! { <span class="st-pcard-chips">{chip}{act}</span> })}
            </div>
            <span class="st-lp">{row.credits.clone()}<small>{t.study_views.col_credits}</small></span>
        </div>
    }
    .into_any()
}
