//! The sidebar of „Mein Studium" on a desktop, top to bottom: the program as one card (its name and
//! degree, the PO and the study direction, the Fachsemester now and the Studienbeginn; a click
//! changes them), the views of the study (owner, 2026-10-14, concept A „Ansichten": „Semester",
//! „Gesamtplan", „Regelstudienplan", „Wahlpflicht", each with a word of where it stands), the ways
//! to the program's own page and to all programs, how the plan reads, and where all of it
//! lives. Nothing in it says twice what the page or the tab bar says (owner, 2026-10-04: „Mein
//! Studium Sidebar ist noch sehr redundant und nicht platz effizient … Zum Stundenplan kann weg der
//! link ist direkt daneben in der Navbar"). A phone has all of it on its overview instead
//! (`phone.rs`), and no sidebar.
//!
//! The program is set once and changed rarely (owner, 2026-10-04: „den eigenen Studiengang zu
//! wählen sollte eher so eine Sache sein, die man für sich halt so einmal macht"): in a dialog that
//! says what of the plan counts in the new one before anything changes (`SwitchDialog`). Nothing of
//! the plan goes with a change; what counts nowhere there shows as „ohne Bereich".

use folia_calendar::semester::SemesterKey;
use folia_model::rows::{CatalogRow, Program};
use folia_pages::ask::{PlanSourceAsk, ProgramsAsk};
use folia_pages::PlanSource;
use folia_plans::choices;
use folia_plans::study::{self, Input};
use folia_plans::studyplan::PlanDoc;
use folia_routes::url::{self, ProgramTab, ProgramUrl, StudyUrl, StudyView};
use leptos::prelude::*;

use folia_design::combobox::{with_first, ComboItem};
use folia_design::ui::Icon;
use folia_shell::pending::Pending;
use folia_stores::myprogram::{po_of, program_name, ProgramPlans};

use super::card::{Ids, Kept, Part, StudyCard};
use super::dialog::DialogHead;
use super::{chosen, input_of, n, rows_of, Dialog, Plans, Ready, Setup, StudyCtx, View};
use crate::i18n::{self, Texts};

#[component]
pub(super) fn StudySidebar(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let ready = Memo::new(move |_| ctx.with_ready(|ready| ready.setup.start_stored).unwrap_or(false));
    let v = &t.study_views;
    move || {
        (!ctx.phone.get()).then(|| {
            view! {
                <div class="fgroup first">
                    <MineCard ctx/>
                </div>
                {move || ready.get().then(|| view! {
                    <div class="fgroup">
                        <p class="flabel label" id="st-views-label">{v.views}</p>
                        <nav class="st-vlist" aria-labelledby="st-views-label"><ViewLinks ctx/></nav>
                    </div>
                })}
                <div class="fgroup">
                    <p class="flabel label">{v.program_group}</p>
                    <nav class="st-ways" aria-label=s.ways>
                        <ProgramPage ctx/>
                        <AllPrograms line=false/>
                    </nav>
                </div>
                {move || ready.get().then(|| view! {
                    <div class="fgroup st-legend">
                        <LegendBar/>
                        <LegendMarks/>
                    </div>
                })}
                <div class="fgroup">
                    <StorageHint/>
                </div>
            }
        })
    }
}

/// Where all of it lives.
#[component]
pub(super) fn StorageHint() -> impl IntoView {
    let t = i18n::t();
    view! { <p class="hint storage-hint"><Icon name="shield-check"/><span>{t.study.storage_hint}</span></p> }
}

/// What the program's card says.
#[derive(Clone, Debug, PartialEq)]
struct Mine {
    name: String,
    degree: String,
    /// „PO 2008", and the study direction where the program has more than one.
    rules: String,
    /// „3. Fachsemester" and „seit WiSe 25/26", „Beginnt im WiSe 26/27": parts that stay whole
    /// where the line breaks.
    when: (String, Option<String>),
}

/// The program (owner, 2026-10-04: „Die Studiengang card ist sehr unordentlich und nimmt zu viel
/// Platz ein ohne die Informationen gut zu vermitteln"): three lines without labels, each clear by
/// itself, and the card as a whole the button that changes program, direction or Studienbeginn
/// („Studiengang wechseln"), which is done rarely.
#[component]
pub(super) fn MineCard(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let mine = Memo::new(move |_| {
        ctx.with_ready(|ready| {
            let setup = &ready.setup;
            setup.start_stored.then(|| {
                let start = setup.start.short(t.locale);
                let when = match study::fs_of(setup.start, setup.now, &setup.leave) {
                    Some(fs) => ((s.fs_long)(fs), Some((s.since)(&start))),
                    None if setup.now < setup.start => ((s.begins_in)(&start), None),
                    None => (s.leave.to_string(), Some((s.since)(&start))),
                };
                let direction = setup.shown.and_then(|shown| ready.plans.iter().find(|(index, _)| *index == shown)).map(|(_, label)| label.clone());
                let rules = std::iter::once(format!("PO {}", po_of(&ready.program))).chain(direction).collect::<Vec<_>>().join(" · ");
                Mine { name: ready.program.name.clone(), degree: ready.program.degree().to_string(), rules, when }
            })
        })
        .flatten()
    });
    move || {
        mine.get().map(|mine| {
            view! {
                <button class="st-mine" id="st-mine" type="button" title=s.change_program on:click=move |_| ctx.open(Dialog::Switch)>
                    <span class="st-mine-name">{mine.name}" "<span class="st-mine-degree">{mine.degree}</span></span>
                    <Icon name="pencil"/>
                    <span class="st-mine-line">{mine.rules}</span>
                    <span class="st-mine-line"><span>{mine.when.0}</span>{mine.when.1.map(|since| view! { " · "<span>{since}</span> })}</span>
                    <span class="visually-hidden">{s.change_program}</span>
                </button>
            }
        })
    }
}

/// What the ways to the views say of the study.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Ways {
    /// „jetzt 3. FS".
    pub now: Option<String>,
    /// „72 LP" still to choose; none where everything is chosen.
    pub to_choose: Option<String>,
    /// „Soll und Ist · 1.–2. FS: 52 von 60 LP bestanden".
    pub plan_line: String,
    /// „Noch 72 LP zu wählen · Nebenfach, Fachstudium".
    pub choices_line: String,
}

impl Ways {
    pub fn of(input: &Input, ready: &Ready, t: &Texts) -> Self {
        let v = &t.study_views;
        let study = &ready.study;
        let view = choices::plan_view(input, study);
        let current = study.semester(study.now).and_then(|semester| semester.fs);
        let plan_line = match current.filter(|fs| *fs > 1) {
            Some(fs) if !view.lines.is_empty() => {
                let before: Vec<&choices::PlanLine> = view.lines.iter().filter(|line| line.last < fs).collect();
                let all: f64 = before.iter().filter_map(|line| line.credits).sum();
                let passed: f64 = before.iter().filter(|line| matches!(line.state, choices::RowState::Passed(_))).filter_map(|line| line.credits).sum();
                (v.plan_way)(&(t.study.fs_span)(1, fs - 1), &n(passed, t), &n(all, t))
            }
            _ => v.plan_way_plain.to_string(),
        };
        let mut open_areas: Vec<String> = Vec::new();
        for choice in view.choices.iter().filter(|choice| !choice.complete()) {
            let name = ready.area_name(choice.area, t);
            if !open_areas.contains(&name) {
                open_areas.push(name);
            }
        }
        let choices_line = if view.choices.is_empty() {
            v.nothing_to_choose.to_string()
        } else if view.to_choose <= 0.0 {
            v.all_chosen.to_string()
        } else {
            (v.choices_way)(&n(view.to_choose, t), &open_areas.join(", "))
        };
        Ways {
            now: current.map(|fs| (v.now_fs)(&(t.study.fs)(fs))),
            to_choose: (view.to_choose > 0.0).then(|| (t.study.credits)(&n(view.to_choose, t))),
            plan_line,
            choices_line,
        }
    }
}

/// The address of a view.
fn view_href(view: StudyView, t: &Texts) -> String {
    t.path(&StudyUrl::default().with_view(view).path())
}

/// The views of the study (owner, 2026-10-14, concept A): the semesters one by one, the
/// Gesamtplan, the Regelstudienplan and the Wahlpflicht, each with a word of where it stands. The
/// first two are one address, which way the semesters are shown a setting of the browser's (R13).
#[component]
fn ViewLinks(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let v = &t.study_views;
    let ways = Memo::new(move |_| ctx.with_input(|input, ready| Ways::of(input, ready, t)));
    // The view shown: where the app is going, else where it is.
    let going = Pending::expect();
    let shown = Memo::new(move |_| going.and_then(|going| going.search_on(url::STUDY)).map_or_else(|| ctx.url.with(|url| url.view), |search| StudyUrl::parse(&search).view));
    let semesters = move |kind: View, icon: &'static str, label: &'static str| {
        let current = move || (shown.get() == StudyView::Semesters && ctx.view.get() == kind).then_some("page");
        let small = move || (kind == View::Semester).then(|| ways.with(|ways| ways.as_ref().and_then(|ways| ways.now.clone()))).flatten();
        view! {
            <a class="st-vl" href=view_href(StudyView::Semesters, t) aria-current=current on:click=move |_| {
                ctx.selection.set(super::Selection::default());
                ctx.view.set(kind);
            }>
                <Icon name=icon/><span>{label}</span><small>{small}</small>
            </a>
        }
    };
    let other = move |view: StudyView, icon: &'static str, label: &'static str| {
        let current = move || (shown.get() == view).then_some("page");
        let small = move || {
            ways.with(|ways| match (view, ways.as_ref()) {
                (StudyView::Plan, Some(_)) => Some(v.plan_side.to_string()),
                (StudyView::Choices, Some(ways)) => Some(ways.to_choose.clone().unwrap_or_else(|| "✓".to_string())),
                _ => None,
            })
        };
        view! { <a class="st-vl" href=view_href(view, t) aria-current=current><Icon name=icon/><span>{label}</span><small>{small}</small></a> }
    };
    view! {
        {semesters(View::Semester, "layout-list", t.study.view_semester)}
        {semesters(View::Grid, "layout-grid", t.study.view_grid)}
        {other(StudyView::Plan, "file-check-2", v.plan)}
        {other(StudyView::Choices, "list-checks", v.choices)}
    }
}

/// The ways to the views on a phone's overview, each with a line of where it stands.
#[component]
pub(super) fn ViewWays(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let v = &t.study_views;
    let ways = Memo::new(move |_| ctx.with_input(|input, ready| Ways::of(input, ready, t)));
    move || {
        ways.get().map(|ways| view! {
            <a class="action st-way" href=view_href(StudyView::Plan, t)><Icon name="file-check-2"/><span>{v.plan}<small>{ways.plan_line}</small></span><Icon name="chevron-right"/></a>
            <a class="action st-way" href=view_href(StudyView::Choices, t)><Icon name="list-checks"/><span>{v.choices}<small>{ways.choices_line}</small></span><Icon name="chevron-right"/></a>
        })
    }
}

/// The program's own page: its regulations and documents, with the plan studied.
#[component]
fn ProgramPage(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let v = &t.study_views;
    let link = Memo::new(move |_| {
        ctx.with_ready(|ready| {
            let page = ProgramUrl::new(&ready.program.slug, ProgramTab::Plan).with_variant(ready.setup.shown.map_or(1, |shown| shown + 1)).path();
            (format!("{page}#dokumente"), format!("PO {}", ready.program.po_version))
        })
    });
    move || {
        link.get().map(|(href, po)| view! {
            <a class="action" href=t.path(&href)><Icon name="file-check-2"/><span>{v.documents}<small>{po}</small></span><Icon name="chevron-right"/></a>
        })
    }
}

/// The way to all programs; on a phone's overview with a line of what is there.
#[component]
pub(super) fn AllPrograms(line: bool) -> impl IntoView {
    let t = i18n::t();
    let v = &t.study_views;
    view! {
        <a class="action" class:st-way=line href=t.path(url::PROGRAMS)>
            <Icon name="graduation-cap"/>
            <span>{v.all_programs}{line.then(|| view! { <small>{v.all_programs_line}</small> })}</span>
            <Icon name="chevron-right"/>
        </a>
    }
}

/// How the plan reads (owner, 2026-10-04: „Die legende ist an sich gut aber der text ist etwas zu
/// Lang. Und die Abgrenzung zu Progressbar und Zeichen für den content ist nicht vorhanden"): the
/// parts of the bar, then the marks a module has in the semesters (`LegendMarks`), each under its
/// own label and each a word.
#[component]
fn LegendBar() -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let part = |class: &'static str, name: &'static str| view! { <li><span class=format!("st-legend-part {class}") aria-hidden="true"></span>{name}</li> };
    view! {
        <p class="flabel label" id="st-legend-bar">{s.legend_bar}</p>
        <ul class="st-legend-parts" aria-labelledby="st-legend-bar">
            {part("passed", s.legend_passed)}
            {part("planned", s.legend_planned)}
            {part("open", s.legend_open)}
            {part("over", s.legend_over)}
        </ul>
    }
}

/// The marks a module has in the semesters, each a word.
#[component]
fn LegendMarks() -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let mark = |class: &'static str, icon: &'static str, name: &'static str| {
        view! { <li><span class=format!("st-legend-mark {class}") aria-hidden="true"><Icon name=icon/></span>{name}</li> }
    };
    view! {
        <p class="flabel label" id="st-legend-marks">{s.legend_marks}</p>
        <ul class="st-legend-marks" aria-labelledby="st-legend-marks">
            {mark("passed", "circle-check-big", s.legend_passed)}
            {mark("st-legend-pill", "repeat", s.legend_retake)}
            {mark("st-legend-pill warn", "triangle-alert", s.legend_offer)}
        </ul>
    }
}

/// Every program of the snapshot, as the pickers offer them: newest PO first, and „Mein
/// Studiengang" once more at their head.
pub(super) fn program_items(ctx: StudyCtx) -> (Memo<Vec<Program>>, Signal<Vec<ComboItem>>) {
    let t = i18n::t();
    let all = Memo::new(move |before| match ctx.source.with_value(|source| source.as_ref().map(|source| source.now(&ProgramsAsk {}))) {
        Some(now) => folia_pages::ask::unless_pending(now, before, Result::unwrap_or_default),
        None => Vec::new(),
    });
    // „Mein Studiengang" first, under its own heading, as in every program picker (owner,
    // 2026-10-15); it stays in its place among the others as well.
    let mine = Memo::new(move |_| ctx.mine.and_then(|mine| mine.with(|doc| doc.program.clone())));
    let items = Signal::derive(move || {
        let mine = mine.get();
        all.with(|all| {
            let items: Vec<ComboItem> = all.iter().map(|program| ComboItem::new(program.id.clone(), program.name.clone(), format!("{} · PO {}", program.degree(), po_of(program)), i64::from(program.is_latest_po))).collect();
            with_first(&items, mine.as_deref(), t.study.my_program)
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

/// The plan picked of `source` (an index of its plans), as „Mein Studiengang" keeps it: its
/// caption and the page of a direction.
pub(super) fn kept_plan(source: &PlanSource, index: usize) -> (String, Option<String>) {
    if source.variants.is_empty() {
        return (String::new(), None);
    }
    ProgramPlans::new(&source.variants, source.supplements.clone()).kept(index.min(source.variants.len() - 1))
}

/// The plans of `source` to choose a study direction from: index and label, where there are two.
pub(super) fn direction_options(source: &PlanSource) -> Vec<ComboItem> {
    match source.variants.len() {
        0 | 1 => Vec::new(),
        _ => source.variants.iter().enumerate().map(|(index, plan)| ComboItem::new(index.to_string(), plan.label.clone(), "", 0)).collect(),
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
    let mut counts = Counts { name: format!("{} {}", source.program.name, source.program.degree()), passed: (0.0, 0.0), planned: (0.0, 0.0), nowhere: Vec::new() };
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

/// The length of the plan `index` of `source` (its first where there is no such), under the
/// program of the card: „Regelstudienplan: 6 Semester, 180 LP"; without a plan, that there is none.
pub(super) fn plan_line(source: &PlanSource, index: usize, t: &Texts) -> String {
    match source.variants.get(index).or_else(|| source.variants.first()) {
        Some(plan) => {
            let credits = if plan.credits_max - plan.credits > 0.5 { format!("{}–{}", n(plan.credits, t), n(plan.credits_max, t)) } else { n(plan.credits, t) };
            (t.study.plan_line)(plan.semesters, &credits)
        }
        None => t.study.no_plan_line.to_string(),
    }
}

/// „Informatik B.Sc." of the program `id` among `all`.
pub(super) fn title_of(all: &[Program], id: Option<&str>) -> Option<String> {
    let id = id?;
    all.iter().find(|program| program.id == id).map(|program| format!("{} {}", program.name, program.degree()))
}

/// „Studiengang wechseln" (owner, 2026-10-16, concept A „Karte"): the study as a card (`card.rs`),
/// a row each for the program, its study direction and the Fachsemester, each changed where it
/// stands; under it, once the program or its direction changed, what of the plan counts there,
/// before anything changes. The program kept, picked again, brings everything back. The frame and
/// what is in it keep their size while it is used (owner: „das layout jumped, wenn man zum
/// beispiel das semester ändert. Das darf auf keinen fall passieren"): what is under the card has
/// one height whatever it says, and the button says „Übernehmen" whatever changes.
#[component]
pub(super) fn SwitchDialog(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let (all, items) = program_items(ctx);
    let before = Memo::new(move |_| ctx.with_ready(|ready| (ready.program.clone(), ready.setup.clone(), ready.plans.clone())));
    let program = RwSignal::new(before.get_untracked().map(|(program, ..)| program.id));
    let start = RwSignal::new(before.get_untracked().map(|(_, setup, _)| setup.start));
    let direction = RwSignal::new(before.get_untracked().and_then(|(_, setup, _)| setup.shown).unwrap_or(0));
    let source = plans_of(ctx, program.into());
    let kept = Memo::new(move |_| {
        before.with(|before| {
            before.as_ref().map(|(program, setup, plans)| Kept {
                program: program.id.clone(),
                name: program_name(program),
                direction: setup.shown.unwrap_or(0),
                direction_label: setup.shown.and_then(|shown| plans.iter().find(|(index, _)| *index == shown).map(|(_, label)| label.clone())),
                start: setup.start,
            })
        })
    });
    // Another program starts with its first plan, in its first Fachsemester now: who changes
    // program mostly begins the new one (owner, 2026-10-16: „wenn man den studiengang wechselt, dann
    // ist es wahrscheinlich, dass das fachsemester wieder bei 1 ist. Aber wenn man zurück den
    // aktuellen studiengang auswählt. Dann sollten die werte wieder zurück"). The program kept,
    // picked again, brings back its Studienbeginn and its study direction.
    let pick_program = Callback::new(move |id: Option<String>| {
        if id.is_none() || id == program.get_untracked() {
            return;
        }
        if let Some(kept) = kept.get_untracked() {
            let same = id.as_deref() == Some(kept.program.as_str());
            start.set(Some(if same { kept.start } else { ctx.now.get_untracked() }));
            direction.set(if same { kept.direction } else { 0 });
        }
        program.set(id);
    });
    let back = Callback::new(move |part: Part| {
        let Some(kept) = kept.get_untracked() else { return };
        match part {
            Part::Program => {
                program.set(Some(kept.program));
                start.set(Some(kept.start));
                direction.set(kept.direction);
            }
            Part::Direction => direction.set(kept.direction),
            Part::Start => start.set(Some(kept.start)),
        }
    });
    let title = Memo::new(move |_| program.with(|program| all.with(|all| title_of(all, program.as_deref()))));
    // `None` while the plans are on their way (the card's row of them stays as it is meanwhile).
    let directions = Memo::new(move |_| source.with(|source| source.as_ref().map(|source| source.as_ref().map(direction_options).unwrap_or_default())));
    let line = Memo::new(move |_| source.with(|source| source.as_ref().and_then(Option::as_ref).map(|source| plan_line(source, direction.get(), t))));
    // What changed: the program, its direction (of the same program), the Studienbeginn.
    let changed = Memo::new(move |_| {
        kept.with(|kept| {
            kept.as_ref().map_or((false, false, false), |kept| {
                let other = program.with(|program| program.as_deref() != Some(kept.program.as_str()));
                let direction = !other && directions.with(|directions| directions.as_ref().is_some_and(|directions| !directions.is_empty())) && direction.get() != kept.direction;
                (other, direction, start.get() != Some(kept.start))
            })
        })
    });
    // What counts there; while the plans of a program picked are on their way, what counted in the
    // one before, so that nothing under the card comes and goes.
    let preview = Memo::new(move |shown: Option<&Option<Counts>>| {
        let now = (|| {
            let (_, setup, _) = before.get()?;
            let source = source.get()??;
            let start = start.get()?;
            let (caption, page) = kept_plan(&source, direction.get());
            let (shown, core, page) = chosen(&source, Some(&caption), page.as_deref());
            let setup = Setup { shown, core, page, start, start_stored: true, ..setup };
            let doc = ctx.plan.map(|plan| plan.with(Clone::clone)).unwrap_or_default();
            let rows = ctx.rows.with(|rows| rows_of(&source, rows));
            Some(counts(&source, &setup, &doc, &rows))
        })();
        now.or_else(|| shown.cloned().flatten())
    });
    let apply = move |_| {
        let (Some(mine), Some(Some(source)), Some(start)) = (ctx.mine, source.get_untracked(), start.get_untracked()) else { return };
        let (caption, page) = kept_plan(&source, direction.get_untracked());
        mine.set_study(&source.program.id, &program_name(&source.program), &caption, page.as_deref(), start);
        ctx.dialog.set(None);
    };
    let effect = move || {
        let (other, direction, start) = changed.get();
        let note = |text: &'static str| view! { <p class="st-effect-note"><Icon name="info"/><span>{text}</span></p> }.into_any();
        if !(other || direction) {
            return note(if start { s.effect_start } else { s.effect_idle });
        }
        let Some(counts) = preview.get() else { return note(s.effect_idle) };
        let figure = |label: &'static str, (all, there): (f64, f64)| view! {
            <div class="st-count">
                <p class="label">{label}</p>
                <p class="st-count-n"><b>{n(there, t)}</b>" "<span>{format!("{} {}", (s.of_credits)(&n(all, t)), s.counts_there)}</span></p>
            </div>
        };
        let names: Vec<&str> = counts.nowhere.iter().take(3).map(|(name, ..)| name.as_str()).collect();
        let nowhere = (!names.is_empty()).then(|| (s.not_there)(&names.join(", "), counts.nowhere.len().saturating_sub(names.len())));
        let whole = nowhere.clone();
        view! {
            <h3>{(s.counts_title)(&counts.name)}</h3>
            <div class="st-count-row">
                {figure(s.counts_passed, counts.passed)}
                {figure(s.counts_planned, counts.planned)}
            </div>
            <p class="st-nowhere-line" title=whole>{nowhere}</p>
            <p class="st-safe"><Icon name="shield-check"/><span>{s.nothing_lost}</span></p>
        }
        .into_any()
    };
    let count = move || {
        let (other, direction, start) = changed.get();
        usize::from(other) + usize::from(direction) + usize::from(start)
    };
    view! {
        <DialogHead ctx title=s.switch_title/>
        <div class="st-dlg-body st-switch">
            <StudyCard
                ids=Ids { program: "st-switch-program", direction: "st-switch-direction", less: "st-switch-less", more: "st-switch-more" }
                items
                program=program
                on_program=pick_program
                title=title
                plan_line=line
                directions=directions
                direction=direction
                on_direction=Callback::new(move |index: usize| direction.set(index))
                start=start
                on_start=Callback::new(move |key: SemesterKey| start.set(Some(key)))
                now=ctx.now
                leave=Signal::derive(move || before.with(|before| before.as_ref().map(|(_, setup, _)| setup.leave.clone()).unwrap_or_default()))
                kept=kept
                on_back=back
            />
            <section class="st-effect" aria-live="polite">{effect}</section>
        </div>
        <footer class="st-dlg-foot">
            <p class="st-foot-state">{move || (s.changes)(count())}</p>
            <button class="btn secondary" type="button" on:click=move |_| ctx.dialog.set(None)>{s.cancel}</button>
            <button class="btn primary" type="button" id="st-switch-go" disabled=move || count() == 0 || source.with(|source| !matches!(source, Some(Some(_)))) on:click=apply>
                {s.apply}
            </button>
        </footer>
    }
}
