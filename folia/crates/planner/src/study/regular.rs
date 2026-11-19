//! „Regelstudienplan", a view of „Mein Studium" (`/study?view=plan`; owner, 2026-10-14, concept A
//! „Ansichten": „Regelstudienplan und Wahlpflicht werden Ansichten von „Mein Studium" – mit deinem
//! Stand darin"). The plan as its regulation prints it, each row with where it stands in the study
//! (`folia_plans::choices::plan_view`): passed, planned, taken again, not passed, open.
//!
//! On a desktop a table, alone in the page (the overview and its bar stay with the semesters;
//! owner, 2026-11-04): a row a row of the plan, grouped by the areas of the progress and in their
//! colours; a column each Fachsemester (the semester it is for the student under it, the current
//! one marked); the credits in the cell of the row's Fachsemester, in the
//! look of where it stands, and where the student has it in another semester a cell there too; at
//! the end „Bei dir": the semester it is passed or planned in, else what can be done — „Einplanen"
//! into the semester it fits next, „Wählen" for a choice (the Wahlpflicht). „Mein Stand" switches
//! the stand off: the plan as printed. The foot: what the plan puts into each Fachsemester, and
//! what the student has there.
//!
//! On a phone a page of its own behind the overview (the way back at the head, `chrome::TopBack`;
//! owner, 2026-11-18, folia/design/regelstudienplan, concept A: „Ja Option a ist super! Setz das
//! bitte um", after „dass man nicht richtig erkennen kann, welches Modul in welchem Semester
//! belegt werden soll und die Anzeige der Module sehr unruhig ist"): a panel per Fachsemester,
//! whose head — the Fachsemester, its semester, „jetzt", a bar and „24 von 30 LP eingeplant" —
//! stays under the bar at the top while its rows scroll, until the next head pushes it out; a tap
//! on a head lists the Fachsemester to jump to. A row is its mark (the marks of „Mein Studium"),
//! its name and its credits, a line under the name only where something differs from the plan
//! (where the student has it, a retake, what fills it, what is taken of its choice); a module's
//! row opens the module as the sheet, a choice's leads to it in the Wahlpflicht. A row stands in
//! the Fachsemester the plan puts it into, and only there.

use std::collections::VecDeque;

use folia_calendar::semester::SemesterKey;
use folia_model::labels::Labelled;
use folia_plans::bench;
use folia_plans::choices::{self, PlanLine, RowState, Taken};
use folia_plans::study::{self, Input, Pick, When};
use folia_routes::url::{ProgramTab, ProgramUrl, StudyUrl, StudyView};
use leptos::prelude::*;

use folia_design::modal::{Modal, ModalHead, ModalSize, Sheet as SheetWay};
use folia_design::ui::{EmptyState, Icon};
use folia_shell::chrome::{Back, TopBack};

use super::focus::{Glyph, OpenGlyph, Status};
use super::{n, now_secs, Ready, StudyCtx};
use crate::i18n::{self, Texts};

/// What a row offers to do.
#[derive(Clone, Debug, PartialEq)]
enum Act {
    None,
    /// „Einplanen" into that semester: the module of the row.
    Plan(String, SemesterKey),
    /// „Wählen ›": the choice (an index of `PlanView::choices`) in the workbench, for the semester
    /// of the row's first Fachsemester, not before the current one (`bench::enter_choice`).
    Choose(usize, SemesterKey),
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
    /// The choice it is one of (an index of `PlanView::choices`): on a phone its row leads there,
    /// in the Wahlpflicht.
    choice: Option<usize>,
    /// Its name on a phone: a choice's as the Wahlpflicht says it (`bench::short_name`:
    /// „Praktische Mathematik", „Wahlpflicht"), a module's as it is.
    short: String,
    /// The choices the regulation sums over that it is one of (an index of `Sheet::sums`): on a
    /// phone a line over the first of them in a Fachsemester.
    sum: Option<usize>,
    /// Its mark on a phone.
    mark: Mark,
    /// On a phone the line under its name, where something differs from the plan.
    line: Option<Line>,
}

/// A row's mark on a phone (app.css `.st-glyph`): the statuses of „Mein Studium", a retake amber
/// with its arrows, what stands open a ring of dashes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mark {
    Status(Status),
    Again,
    Open,
}

/// The line under a row's name on a phone: where the student has it, where that is another
/// Fachsemester than the plan's („bei dir im WiSe 27/28", after an arrow), a retake or a module not
/// passed (`warn`), what fills it, what is taken of its choice.
#[derive(Clone, Debug, PartialEq)]
struct Line {
    text: String,
    warn: bool,
    moved: bool,
}

/// Where a share of a row's credits stands, for a phone's bar.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Part {
    Passed,
    /// Planned into the current semester.
    Running,
    Planned,
    /// Planned into a semester of another Fachsemester than the row's.
    Elsewhere,
    /// Not passed, and planned again.
    Again,
}

/// What the student holds of what the plan asks of a phone's section, by where it stands: the
/// parts of its bar.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Held {
    passed: f64,
    running: f64,
    planned: f64,
    elsewhere: f64,
    again: f64,
}

impl Held {
    fn add(&mut self, part: Part, credits: f64) {
        match part {
            Part::Passed => self.passed += credits,
            Part::Running => self.running += credits,
            Part::Planned => self.planned += credits,
            Part::Elsewhere => self.elsewhere += credits,
            Part::Again => self.again += credits,
        }
    }

    /// All of it: what the head says is planned.
    fn all(&self) -> f64 {
        self.passed + self.running + self.planned + self.elsewhere + self.again
    }

    /// Its parts as the bar draws them, in this order (app.css `.st-rsp-bar`): their class and
    /// credits.
    fn parts(&self) -> [(&'static str, f64); 5] {
        [("pa", self.passed), ("ru", self.running), ("pl", self.planned), ("el", self.elsewhere), ("ag", self.again)]
    }
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
    /// The semester it is for the student, short, its half of the year and its year: („WiSe",
    /// „25/26"), („SoSe", „26").
    when: Option<(String, String)>,
    now: bool,
}

/// „WiSe 25/26" as its half of the year and its year, each a line of a column's head.
fn halves(short: &str) -> (String, String) {
    match short.split_once(' ') {
        Some((season, year)) => (season.to_string(), year.to_string()),
        None => (short.to_string(), String::new()),
    }
}

/// A Fachsemester, or several the plan's rows span together, as a phone's panel.
#[derive(Clone, Debug, PartialEq)]
struct Section {
    id: String,
    /// „3. Fachsemester", „5. und 6. Fachsemester".
    title: String,
    /// „WiSe 26/27", „WiSe 27/28 – SoSe 28".
    when: Option<String>,
    now: bool,
    /// What the plan asks of it, what the student holds of that (the bar of its head), and the
    /// words of the head: „32 von 32 LP bestanden" once it is over, else „24 von 30 LP
    /// eingeplant".
    sum: f64,
    held: Held,
    figure: String,
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
    /// On a phone the line over the choices the regulation sums over: „Komplexe des Fachstudiums ·
    /// zusammen 44 LP".
    sums: Vec<String>,
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
                (None, RowState::Open | RowState::Failed(_), Some((at, _))) => {
                    let due = study::semester_of_fs(study.start, line.first, &ready.setup.leave).map_or(study.now, |s| s.max(study.now));
                    Act::Choose(at, due)
                }
                _ => Act::None,
            };
            let mark = match line.state {
                RowState::Passed(_) => Mark::Status(Status::Passed),
                RowState::Planned(s) if s == study.now => Mark::Status(Status::Running),
                RowState::Planned(_) => Mark::Status(Status::Planned),
                RowState::Again { .. } => Mark::Again,
                RowState::Failed(_) => Mark::Status(Status::Failed),
                RowState::Open => Mark::Open,
            };
            // Under the name only what differs from the plan: the semester the student has it in,
            // where that is not the plan's; a retake; what fills it; what is taken of its choice.
            let line_of = match line.state {
                RowState::Again { planned, .. } => Some(Line { text: (v.again_note)(&short(planned, t)), warn: true, moved: false }),
                RowState::Failed(s) => Some(Line { text: (v.state_failed)(&short(s, t)), warn: true, moved: false }),
                state => match (moved.and(state.semester()), note.clone()) {
                    (Some(s), Some(note)) => Some(Line { text: format!("{note} · {}", (v.moved_note)(&short(s, t))), warn: false, moved: true }),
                    (Some(s), None) => Some(Line { text: (v.moved_note)(&short(s, t)), warn: false, moved: true }),
                    (None, Some(note)) => Some(Line { text: note, warn: false, moved: false }),
                    (None, None) => None,
                },
            };
            Row {
                key: format!("{}:{}", line.caption, line.ord),
                name: line.name.clone(),
                credits: line.credits_text.as_deref().map(|text| folia_plans::plan::credits_in(text, t.locale)).or_else(|| line.credits.map(|credits| n(credits, t))),
                kind: line.kind.map(|kind| kind.label(t.locale)),
                first: line.first,
                last: line.last,
                tone: ready.tone(line.area),
                state: line.state,
                moved,
                stand,
                when: when.map(|s| short(s, t)),
                note,
                act,
                module_id,
                choice: line.choice,
                short: if line.module_id.is_some() { line.name.clone() } else { bench::short_name(&line.name) },
                sum: line.choice.and_then(|at| view.groups.iter().position(|group| group.members.contains(&at))),
                mark,
                line: line_of,
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
        let columns: Vec<Column> = (1..=last).map(|fs| Column { fs, when: semester_of(fs).map(|s| halves(&short(s, t))), now: semester_of(fs) == Some(study.now) }).collect();

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
        let totals = study.totals();
        let mine_total = ((v.foot_passed)(&(t.study.credits)(&n(totals.passed_all(), t))), (v.foot_planned)(&(t.study.credits)(&n(totals.planned_all(), t))));

        // Where each row's credits stand, for a phone's bars: its own state; for a row of a choice
        // that nothing fills, its share of what the choice's modules hold beyond the rows they
        // fill, shared out over its open rows in the plan's order, what is passed first.
        let part_of = |s: SemesterKey, line: &PlanLine, passed: bool| -> Part {
            let outside = fs_of(s).is_some_and(|fs| !(line.first..=line.last).contains(&fs));
            match (passed, outside) {
                (true, _) => Part::Passed,
                (false, true) => Part::Elsewhere,
                (false, false) if s == study.now => Part::Running,
                (false, false) => Part::Planned,
            }
        };
        let mut parts: Vec<Vec<(Part, f64)>> = view
            .lines
            .iter()
            .map(|line| {
                let credits = line.credits.unwrap_or(0.0);
                match line.state {
                    RowState::Passed(_) => vec![(Part::Passed, credits)],
                    RowState::Planned(s) => vec![(part_of(s, line, false), credits)],
                    RowState::Again { .. } => vec![(Part::Again, credits)],
                    RowState::Failed(_) | RowState::Open => Vec::new(),
                }
            })
            .collect();
        for choice in &view.choices {
            let mut skip: f64 = choice.lines.iter().filter_map(|at| view.lines.get(*at)).filter(|line| !matches!(line.state, RowState::Open | RowState::Failed(_))).filter_map(|line| line.credits).sum();
            let mut taken: Vec<&Taken> = choice.taken.iter().collect();
            taken.sort_by_key(|taken| !taken.passed);
            let mut left: VecDeque<(f64, &Taken)> = VecDeque::new();
            for taken in taken {
                let credits = taken.credits.unwrap_or(0.0);
                let used = credits.min(skip);
                skip -= used;
                if credits - used > 1e-6 {
                    left.push_back((credits - used, taken));
                }
            }
            for at in &choice.lines {
                let (Some(line), Some(share)) = (view.lines.get(*at), parts.get_mut(*at)) else { continue };
                if line.state != RowState::Open {
                    continue;
                }
                let mut room = line.credits.unwrap_or(0.0);
                while room > 1e-6 {
                    let Some((credits, taken)) = left.front_mut() else { break };
                    let used = credits.min(room);
                    share.push((part_of(taken.semester, line, taken.passed), used));
                    *credits -= used;
                    room -= used;
                    if *credits <= 1e-6 {
                        left.pop_front();
                    }
                }
            }
        }

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
            let span = if together { (first, to) } else { (first, first) };
            let title = if together { (v.term_span)(first, to) } else { (t.study.fs_long)(first) };
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
            let mut held = Held::default();
            for (_, share) in view.lines.iter().zip(&parts).filter(|(line, _)| line.first == first) {
                for (part, credits) in share {
                    held.add(*part, *credits);
                }
            }
            let figure = if over { (v.section_passed)(&n(held.passed, t), &n(sum, t)) } else { (v.section_planned)(&n(held.all(), t), &n(sum, t)) };
            sections.push(Section { id: format!("st-fs-{first}"), title, when, now, sum, held, figure, rows: of });
        }

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
            sums: view.groups.iter().map(|group| (v.group_line)(&group.label, &n(group.together, t))).collect(),
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
        Act::Choose(at, s) => view! {
            <button class="st-addb" type="button" title=format!("{} · {}", v.choose, s.label(t.locale)) on:click=move |_| super::bench::enter_choice(ctx, s, at)>
                {v.choose}<Icon name="chevron-right"/>
            </button>
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
            // One stop of Tab: ↑ ↓ from module to module, ← → to „Einplanen" in its row
            // (enhance.js, `data-stop="grid"`).
            <div class="table-scroll" data-stop="grid">
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
                                // Every head the same lines, so that the numbers stand in one row
                                // (owner, 2026-10-16: „die semester so durcheinander angezeigt";
                                // „WiSe 26/27" broke into two lines and „SoSe 27" did not): the
                                // mark of the current one (its place kept in the others), the
                                // Fachsemester, the half of the year, the year.
                                <th scope="col" class="fs" class:now=column.now>
                                    <span class="st-fs-head">
                                        <span class="st-fs-now">{column.now.then(|| view! { <span class="st-now">{t.study.now}</span> })}</span>
                                        <b>{column.fs}</b>
                                        {column.when.clone().map(|(season, year)| view! { <small>{season}</small><small>{year}</small> })}
                                    </span>
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
    TopBack::follow(move || {
        let history = ctx.view_back.get();
        ctx.phone.get().then(|| Back { href: StudyUrl::default().path(), label: t.study_views.to_overview, history })
    });
}

/// The Fachsemester on a phone (folia/design/regelstudienplan, concept A): a panel each, whose
/// head stays under the bar at the top while its rows scroll and, tapped, lists the Fachsemester to
/// jump to (a sheet as tall as they are); the marks under the title once.
#[component]
fn PlanPhone(ctx: StudyCtx, sheet: Sheet) -> impl IntoView {
    let t = i18n::t();
    let v = &t.study_views;
    let s = &t.study;
    // The list of the Fachsemester to jump to, open while it names the one it was opened from.
    let asked: RwSignal<Option<String>> = RwSignal::new(None);
    let close = Callback::new(move |()| asked.set(None));
    let jumps = sheet.sections.clone();
    let sums = sheet.sums.clone();
    let marks = [
        (Mark::Status(Status::Passed), s.passed_short),
        (Mark::Status(Status::Running), s.running_short),
        (Mark::Status(Status::Planned), s.planned_short),
        (Mark::Open, s.open_short),
        (Mark::Again, v.legend_again),
    ];
    view! {
        <div class="st-phone-page st-rsp-phone">
            <header class="st-vhead">
                <h1>{v.plan}</h1>
                <p>{sheet.program.clone()}{sheet.direction.clone().map(|direction| format!(" · {direction}"))}</p>
            </header>
            <ul class="st-rsp-legend" aria-label=s.legend_marks>
                {marks.into_iter().map(|(mark, word)| view! { <li>{mark_view(mark)}{word}</li> }).collect_view()}
            </ul>
            {sheet.sections.into_iter().map(|section| {
                let id = section.id.clone();
                view! {
                    <section class="st-rsp-sec" class:now=section.now id=section.id.clone() aria-label=section.title.clone()>
                        <button type="button" class="st-rsp-head" aria-haspopup="dialog" title=v.jump on:click=move |_| asked.set(Some(id.clone()))>
                            <span class="st-rsp-h1">
                                <b>{section.title.clone()}</b>
                                {section.now.then(|| view! { <span class="st-now">{s.now}</span> })}
                                <span class="st-rsp-w">{section.when.clone().map(|when| view! { <small>{when}</small> })}<Icon name="chevrons-up-down"/></span>
                            </span>
                            <span class="st-rsp-h2">{bar_view(&section.held, section.sum)}<small>{section.figure.clone()}</small></span>
                        </button>
                        <ul class="st-rsp-rows">
                            {rows_view(ctx, section.rows, &sums, t)}
                        </ul>
                    </section>
                }
            }).collect_view()}
            <p class="hint st-rsp-source"><Icon name="file-check-2"/><a href=t.path(&sheet.source)>{v.plan_source}</a></p>
            <Modal open=Signal::derive(move || asked.with(Option::is_some)) on_close=close size=ModalSize::S sheet=Signal::derive(|| SheetWay::Menu) class="st-rsp-jumps" labelled_by="st-rsp-jumps-title">
                <ModalHead id="st-rsp-jumps-title" title=v.jump_title sub=Some(v.jump_hint.to_string()) on_close=close/>
                <div class="st-dlg-body">
                    <ul class="st-rsp-jump">
                        {jumps.iter().map(|section| {
                            let id = section.id.clone();
                            let here = id.clone();
                            view! {
                                <li>
                                    <a
                                        href=format!("#{id}")
                                        class:here=move || asked.with(|asked| asked.as_deref() == Some(here.as_str()))
                                        on:click=move |ev: leptos::ev::MouseEvent| {
                                            ev.prevent_default();
                                            asked.set(None);
                                            jump_to(id.clone());
                                        }
                                    >
                                        <span class="st-rsp-jt">
                                            <b>{section.title.clone()}{section.now.then(|| view! { <span class="st-now">{s.now}</span> })}</b>
                                            {section.when.clone().map(|when| view! { <small>{when}</small> })}
                                        </span>
                                        <span class="st-rsp-jf"><small>{section.figure.clone()}</small>{bar_view(&section.held, section.sum)}</span>
                                    </a>
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                </div>
            </Modal>
        </div>
    }
}

/// A Fachsemester's rows, the line of a sum the regulation prints over the first of its choices.
fn rows_view(ctx: StudyCtx, rows: Vec<Row>, sums: &[String], t: &'static Texts) -> impl IntoView {
    let mut shown: Option<usize> = None;
    let mut out = Vec::new();
    for row in rows {
        if let Some(sum) = row.sum.filter(|sum| shown != Some(*sum)) {
            if let Some(line) = sums.get(sum) {
                out.push(view! { <li class="st-rsp-grp">{line.clone()}</li> }.into_any());
            }
        }
        shown = row.sum;
        out.push(view! { <li>{plan_row(ctx, row, t)}</li> }.into_any());
    }
    out
}

/// A Fachsemester's bar: what is passed, under way, planned, planned in another Fachsemester and
/// taken again, of what the plan asks of it (app.css `.st-rsp-bar`).
fn bar_view(held: &Held, sum: f64) -> impl IntoView {
    let parts = held.parts();
    view! {
        <span class="st-rsp-bar" aria-hidden="true">
            {parts.into_iter().filter(|(_, credits)| *credits > 0.0 && sum > 0.0).map(|(class, credits)| view! {
                <i class=class style=format!("width: {:.2}%", (credits / sum * 100.0).min(100.0))></i>
            }).collect_view()}
        </span>
    }
}

/// A row's mark (app.css `.st-glyph`).
fn mark_view(mark: Mark) -> AnyView {
    match mark {
        Mark::Status(status) => view! { <Glyph status=Signal::stored(status)/> }.into_any(),
        Mark::Again => view! { <span class="st-glyph again" aria-hidden="true"><Icon name="repeat"/></span> }.into_any(),
        Mark::Open => view! { <OpenGlyph/> }.into_any(),
    }
}

/// The word of a row's mark, for screen readers.
fn mark_word(mark: Mark, t: &Texts) -> &'static str {
    let s = &t.study;
    match mark {
        Mark::Status(status) => status.word(t),
        Mark::Again => t.study_views.legend_again,
        Mark::Open => s.status_open,
    }
}

/// A row of the plan on a phone: its mark, its name with the line under it where something
/// differs, its credits. A module's opens the module as the sheet; a choice's leads to it in the
/// Wahlpflicht, whose way back comes back here.
fn plan_row(ctx: StudyCtx, row: Row, t: &'static Texts) -> AnyView {
    let body = |row: &Row| {
        view! {
            {mark_view(row.mark)}
            <span class="st-rsp-t">
                <b>{row.short.clone()}</b>
                <span class="visually-hidden">{format!(", {}", mark_word(row.mark, t))}</span>
                {row.line.clone().map(|line| view! { <small class:warn=line.warn>{line.moved.then(|| view! { <Icon name="arrow-right"/> })}{line.text}</small> })}
            </span>
            <span class="st-lp">{row.credits.clone()}<small>{t.study_views.col_credits}</small></span>
        }
    };
    let class = format!("st-rsp-row {}", row.class());
    match (row.module_id.clone(), row.choice) {
        (Some(id), _) => view! { <a class=class href=super::module_href(ctx, &id, t) data-noscroll="">{body(&row)}<span></span></a> }.into_any(),
        (None, Some(at)) => {
            let href = t.path(&StudyUrl::default().with_view(StudyView::Choices).with_choice(Some(at)).path());
            view! { <a class=format!("{class} is-choice") href=href data-push="" on:click=move |_| ctx.choice_from_plan.set(true)>{body(&row)}<Icon name="chevron-right"/></a> }.into_any()
        }
        (None, None) => view! { <div class=class>{body(&row)}<span></span></div> }.into_any(),
    }
}

/// Scrolls the page to a Fachsemester, its head at the top, once the list of them has closed.
#[allow(unused_variables)]
fn jump_to(id: String) {
    #[cfg(feature = "csr")]
    request_animation_frame(move || {
        let Some(window) = web_sys::window() else { return };
        let Some(section) = folia_design::stage::by_id(&id) else { return };
        let reduced = window.match_media("(prefers-reduced-motion: reduce)").ok().flatten().is_some_and(|query| query.matches());
        let options = web_sys::ScrollIntoViewOptions::new();
        options.set_block(web_sys::ScrollLogicalPosition::Start);
        options.set_behavior(if reduced { web_sys::ScrollBehavior::Instant } else { web_sys::ScrollBehavior::Smooth });
        section.scroll_into_view_with_scroll_into_view_options(&options);
    });
}
