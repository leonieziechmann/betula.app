//! The program overview: one section per faculty, and in it a matrix: a row per subject, a
//! column per cycle of study (Bachelor, Master, the rest). 148 programs are about 80 rows, most
//! of them one line, and everything stands on the same few vertical lines, which is what makes
//! the page calm. (A flow of cards, and a list set in several text columns, were both tried
//! first and read as clutter: rows of unequal height with nothing to line up on.)
//! A program that is also offered in other forms of study (dual, extended, …) carries them as
//! segments of the same control.
//!
//! The sidebar filters (degree, form of study, study plan) and jumps to the faculties; the
//! search of the top bar narrows by name. All of it is in the URL (`catalog::url::ProgramsUrl`).
//!
//! No source states a program's faculty. It is derived (`catalog::pages::faculties`), the
//! sidebar says on what grounds, and programs without a clear answer have a section of their own.

use catalog::labels::DegreeLevel;
use catalog::pages::{self, ProgramsData};
use catalog::rows::{Department, Program};
use catalog::url::{self, FormGroup, LevelGroup, ProgramTab, ProgramsUrl};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_location;

use crate::data::{use_source, PageStatus};
use crate::format;
use crate::ui::{ErrorState, Frame, Icon, ToggleLink};

/// All programs of one subject („Maschinenbau" with its degrees and forms of study).
#[derive(Clone, PartialEq)]
struct Subject {
    title: String,
    programs: Vec<Program>,
}

/// A faculty with its subjects; `department` is `None` for programs no faculty could be derived for.
#[derive(Clone, PartialEq)]
struct Faculty {
    department: Option<Department>,
    subjects: Vec<Subject>,
}

impl Faculty {
    fn anchor(&self) -> String {
        match &self.department {
            Some(department) => format!("fakultaet-{}", department.id),
            None => "ohne-fakultaet".to_string(),
        }
    }

    /// „Fakultät 1" for the numbered ones, the abbreviation for the others.
    fn short(&self) -> String {
        match &self.department {
            Some(d) if d.code.chars().all(|c| c.is_ascii_digit()) => format!("Fakultät {}", d.code),
            Some(d) => d.code.clone(),
            None => "Ohne Zuordnung".to_string(),
        }
    }

    fn name(&self) -> String {
        match &self.department {
            Some(d) => d.name_de.clone(),
            None => "Fakultätsübergreifend oder nicht eindeutig zuzuordnen".to_string(),
        }
    }

    fn programs(&self) -> usize {
        self.subjects.iter().map(|subject| subject.programs.len()).sum()
    }
}

/// Current programs, grouped: faculties in their order (1 to 6, then the others, then the
/// programs without one), subjects by name, programs by degree level and year.
fn group(data: &ProgramsData) -> Vec<Faculty> {
    let mut faculties: Vec<Faculty> = Vec::new();
    for program in data.programs.iter().filter(|p| p.is_latest_po) {
        let department = data
            .faculties
            .iter()
            .find(|f| f.program_id == program.id)
            .and_then(|f| data.departments.iter().find(|d| d.id == f.department_id))
            .cloned();
        let faculty = match faculties.iter().position(|f| f.department.as_ref().map(|d| d.id) == department.as_ref().map(|d| d.id)) {
            Some(index) => faculties.get_mut(index),
            None => {
                faculties.push(Faculty { department, subjects: Vec::new() });
                faculties.last_mut()
            }
        };
        let Some(faculty) = faculty else { continue };
        // „Maschinenbau - dual" is a form of the subject „Maschinenbau".
        let key = program.name_key.trim_end_matches("-dual");
        match faculty.subjects.iter_mut().find(|s| s.programs.first().is_some_and(|p| p.name_key.trim_end_matches("-dual") == key)) {
            Some(subject) => subject.programs.push(program.clone()),
            None => faculty.subjects.push(Subject { title: String::new(), programs: vec![program.clone()] }),
        }
    }
    for faculty in &mut faculties {
        for subject in &mut faculty.subjects {
            subject.programs.sort_by_key(|p| (level_order(p), p.study_variant.is_some(), std::cmp::Reverse(p.po_year)));
            subject.title = subject.programs.iter().map(|p| p.name.clone()).min_by_key(|name| name.chars().count()).unwrap_or_default();
        }
        faculty.subjects.sort_by_key(|subject| catalog::search::fold(&subject.title));
    }
    faculties.sort_by_key(|faculty| match &faculty.department {
        Some(d) => (d.code.parse::<u32>().map_or(1, |_| 0), d.code.parse::<u32>().unwrap_or(0), d.code.clone()),
        None => (2, 0, String::new()),
    });
    faculties
}

fn level_order(program: &Program) -> usize {
    let level = LevelGroup::of(&program.degree_level);
    LevelGroup::ALL.iter().position(|l| *l == level).unwrap_or(LevelGroup::ALL.len())
}

fn matches(program: &Program, url: &ProgramsUrl) -> bool {
    let text = format!("{} {} {}", program.name, program.degree(), program.degree_level.label());
    (url.levels.is_empty() || url.levels.contains(&LevelGroup::of(&program.degree_level)))
        && (url.forms.is_empty() || FormGroup::of(program.study_variant.as_ref()).is_some_and(|form| url.forms.contains(&form)))
        && (!url.with_plan || program.has_plan)
        && catalog::search::matches(&text, &url.text)
}

/// The columns of the matrix: the cycles of study. Lehramt counts to the cycle it is part of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Bachelor,
    Master,
    /// Doctoral programs and programs without a degree.
    Other,
}

impl Stage {
    fn of(program: &Program) -> Self {
        match program.degree_level.known() {
            Some(DegreeLevel::Bachelor | DegreeLevel::TeachingBachelor) => Stage::Bachelor,
            Some(DegreeLevel::Master | DegreeLevel::TeachingMaster) => Stage::Master,
            _ => Stage::Other,
        }
    }

    fn code(self) -> &'static str {
        match self {
            Stage::Bachelor => "bachelor",
            Stage::Master => "master",
            Stage::Other => "other",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Stage::Bachelor => "Bachelor",
            Stage::Master => "Master",
            Stage::Other => "Weitere",
        }
    }
}

/// The degree as a control says it: „B.Sc.", else „Bachelor", else „ohne Abschluss".
fn degree_short(program: &Program) -> String {
    program.degree_display.clone().unwrap_or_else(|| program.degree_level.label().to_string())
}

/// What tells a form of study from the plain program of its subject: „dual, Praxis", „erweitert".
/// „Maschinenbau - dual" adds the „dual" of its name where the form does not say it already.
fn form_label(program: &Program, subject: &str) -> Option<String> {
    let added = program.name.strip_prefix(subject).map(|rest| rest.trim_matches([' ', '-'])).filter(|rest| !rest.is_empty());
    match (added, program.study_variant.as_ref().map(format::variant_short)) {
        (Some(added), Some(variant)) if !variant.contains(added) => Some(format!("{added}, {variant}")),
        (Some(added), None) => Some(added.to_string()),
        (_, variant) => variant,
    }
}

/// A program with the other forms of study it is offered in (same degree, same PO year).
struct Group {
    main: Program,
    forms: Vec<Program>,
}

fn groups(programs: Vec<Program>, subject: &str) -> Vec<Group> {
    let is_form = |p: &Program| p.study_variant.is_some() || p.name != subject;
    let mut programs = programs;
    programs.sort_by_key(|p| (is_form(p), std::cmp::Reverse(p.po_year), degree_short(p)));
    let mut groups: Vec<Group> = Vec::new();
    for program in programs {
        let base = match is_form(&program) {
            true => groups.iter_mut().find(|g| !is_form(&g.main) && degree_short(&g.main) == degree_short(&program) && g.main.po_year == program.po_year),
            false => None,
        };
        match base {
            Some(group) => group.forms.push(program),
            None => groups.push(Group { main: program, forms: Vec::new() }),
        }
    }
    groups
}

/// What the page shows for a URL: the faculties with what is left of them.
#[derive(Clone, PartialEq)]
struct Shown {
    faculties: Vec<Faculty>,
    programs: usize,
    /// Whether any program is left in these columns: a column nothing is in is not drawn.
    bachelor: bool,
    master: bool,
    text: String,
    filtered: bool,
}

#[component]
pub fn ProgramsPage() -> impl IntoView {
    let source = use_source();
    let status = PageStatus::capture();
    let location = use_location();
    let url = Memo::new(move |_| ProgramsUrl::parse(&location.search.get()));
    // Loaded and grouped once; the URL only decides what of it is shown.
    let loaded = source.and_then(|source| source.run(pages::programs_overview));
    let all = match loaded {
        Ok(data) => group(&data),
        Err(error) => {
            status.for_error(&error);
            return view! { <Title text="Studiengänge"/><div class="page"><ErrorState error/></div> }.into_any();
        }
    };
    let total: usize = all.iter().map(Faculty::programs).sum();
    let count_where = |keep: &dyn Fn(&Program) -> bool| all.iter().flat_map(|f| &f.subjects).flat_map(|s| &s.programs).filter(|p| keep(p)).count();
    let level_counts: Vec<(LevelGroup, usize)> = LevelGroup::ALL.iter().map(|level| (*level, count_where(&|p| LevelGroup::of(&p.degree_level) == *level))).collect();
    let form_counts: Vec<(FormGroup, usize)> = FormGroup::ALL.iter().map(|form| (*form, count_where(&|p| FormGroup::of(p.study_variant.as_ref()) == Some(*form)))).collect();
    let plan_count = count_where(&|p| p.has_plan);

    let all = StoredValue::new(all);
    let shown = Memo::new(move |_| {
        let url = url.get();
        let faculties: Vec<Faculty> = all.with_value(|all| {
            all.iter()
                .map(|faculty| Faculty {
                    department: faculty.department.clone(),
                    subjects: faculty
                        .subjects
                        .iter()
                        .map(|subject| Subject { title: subject.title.clone(), programs: subject.programs.iter().filter(|p| matches(p, &url)).cloned().collect() })
                        .filter(|subject| !subject.programs.is_empty())
                        .collect(),
                })
                .filter(|faculty| !faculty.subjects.is_empty())
                .collect()
        });
        let any = |stage: Stage| faculties.iter().flat_map(|f| &f.subjects).flat_map(|s| &s.programs).any(|p| Stage::of(p) == stage);
        Shown {
            programs: faculties.iter().map(Faculty::programs).sum(),
            bachelor: any(Stage::Bachelor),
            master: any(Stage::Master),
            faculties,
            text: url.text.clone(),
            filtered: url.is_filtered(),
        }
    });

    // Every control of the sidebar is a link to the overview it leads to (as in the catalog).
    let toggled = move |change: fn(&mut ProgramsUrl, usize), index: usize| {
        Signal::derive(move || {
            let mut next = url.get();
            change(&mut next, index);
            next.path()
        })
    };
    fn flip<T: PartialEq + Copy>(list: &mut Vec<T>, all: &[T], value: Option<&T>) {
        let Some(value) = value else { return };
        if list.contains(value) {
            list.retain(|v| v != value);
        } else {
            // Kept in the order of `all`, so equal filters have equal URLs.
            *list = all.iter().copied().filter(|v| list.contains(v) || v == value).collect();
        }
    }

    let sidebar = move || {
        view! {
            <div class="fgroup first">
                <div class="flabel label">"Abschluss"</div>
                <div class="chips">
                    {level_counts.iter().enumerate().filter(|(_, (_, count))| *count > 0).map(|(index, (level, count))| {
                        let level = *level;
                        view! {
                            <ToggleLink
                                label=level.label()
                                count=*count
                                on=Signal::derive(move || url.with(|u| u.levels.contains(&level)))
                                href=toggled(|u, index| flip(&mut u.levels, LevelGroup::ALL, LevelGroup::ALL.get(index)), index)
                            />
                        }
                    }).collect_view()}
                </div>
            </div>
            <div class="fgroup">
                <div class="flabel label">"Studienform"</div>
                <div class="chips">
                    {form_counts.iter().enumerate().filter(|(_, (_, count))| *count > 0).map(|(index, (form, count))| {
                        let form = *form;
                        view! {
                            <ToggleLink
                                label=form.label()
                                count=*count
                                on=Signal::derive(move || url.with(|u| u.forms.contains(&form)))
                                href=toggled(|u, index| flip(&mut u.forms, FormGroup::ALL, FormGroup::ALL.get(index)), index)
                            />
                        }
                    }).collect_view()}
                </div>
            </div>
            <div class="fgroup">
                <div class="flabel label">"Daten"</div>
                <div class="chips">
                    <ToggleLink
                        label="Mit Regelstudienplan"
                        count=plan_count
                        on=Signal::derive(move || url.with(|u| u.with_plan))
                        href=toggled(|u, _| u.with_plan = !u.with_plan, 0)
                    />
                </div>
            </div>
            <nav class="fgroup toc jumps" aria-label="Fakultäten">
                <p class="flabel label">"Fakultäten"</p>
                {move || shown.get().faculties.into_iter().map(|faculty| view! {
                    <a href=format!("#{}", faculty.anchor()) data-action="jump" title=faculty.name()>
                        <b>{faculty.short()}</b>
                        <span class="toc-name">{faculty.name()}</span>
                        <span class="num">{faculty.programs()}</span>
                    </a>
                }).collect_view()}
                <p class="hint">
                    "Die BTU nennt zu einem Studiengang keine Fakultät. Zugeordnet ist die Fakultät des Abschlussmoduls, "
                    "sonst die, die den größten Teil des Curriculums anbietet."
                </p>
            </nav>
            <div class="filter-actions sheet-only">
                <a class="btn primary" href="#" data-action="sheet-close">{move || format::count(shown.with(|s| s.programs as u64))}" Studiengänge anzeigen"</a>
            </div>
        }
    };
    let head = move || {
        view! {
            {move || url.with(ProgramsUrl::is_filtered).then(|| view! {
                <a class="ghost" href=url::PROGRAMS data-noscroll=""><Icon name="rotate-ccw"/>"Zurücksetzen"</a>
            })}
        }
    };

    view! {
        <Title text="Studiengänge"/>
        <Frame title="Filter" head sidebar sheet=true>
            <div class="page-inner">
                // The same opening as the catalog's list: the number, then what it counts.
                <header class="summary">
                    <h1>
                        <span class="count num">{move || format::count(shown.with(|s| s.programs as u64))}</span>
                        <span class="count-label">
                            {move || shown.with(|shown| match (shown.filtered, shown.text.is_empty()) {
                                (false, _) => "Studiengänge in ihrer aktuellen Prüfungsordnung".to_string(),
                                (true, true) => format!("von {total} Studiengängen"),
                                (true, false) => format!("von {total} Studiengängen passen zu „{}“", shown.text),
                            })}
                        </span>
                    </h1>
                    <a class="sheet-toggle" href="#sidebar" data-action="sheet-open"><Icon name="sliders-horizontal"/>"Filter"</a>
                </header>
                {move || {
                    let shown = shown.get();
                    if shown.faculties.is_empty() {
                        return view! {
                            <div class="state">
                                <p class="state-title">"Kein Studiengang gefunden"</p>
                                <p>"Nimm einen Filter zurück oder suche nach einem Teil des Namens."</p>
                                <a class="btn secondary" href=url::PROGRAMS>"Alle Studiengänge zeigen"</a>
                            </div>
                        }.into_any();
                    }
                    let (bachelor, master) = (shown.bachelor, shown.master);
                    shown.faculties.into_iter().map(|faculty| view! { <FacultySection faculty bachelor master/> }).collect_view().into_any()
                }}
            </div>
        </Frame>
    }
    .into_any()
}

/// One link of the matrix. `as_form`: a segment behind its plain program, saying only what differs.
fn program_link(program: &Program, subject: &str, as_form: bool) -> impl IntoView {
    let year = program.po_year.map(|year| year.to_string()).unwrap_or_else(|| program.po_version.clone());
    let form = form_label(program, subject);
    let described = format!(
        "{} · {} · PO {}{}",
        program.name,
        program.degree(),
        program.po_version,
        program.study_variant.as_ref().map(|v| format!(" · {}", v.label())).unwrap_or_default()
    );
    let tooltip = format!(
        "{described} · {} Module · {}",
        program.curricular_modules,
        if program.has_plan { "geprüfter Regelstudienplan" } else { "noch kein geprüfter Regelstudienplan" }
    );
    let text = if as_form {
        view! { <span>{form.unwrap_or_else(|| program.name.clone())}</span> }.into_any()
    } else {
        view! {
            <b>{degree_short(program)}</b>
            <span class="num">{year}</span>
            {form.map(|form| view! { <span class="variant">{form}</span> })}
        }
        .into_any()
    };
    view! {
        <a
            class="program-pill"
            class:form=as_form
            class:no-plan=!program.has_plan
            data-walk="program-link"
            href=url::program_path(&program.slug, ProgramTab::Plan)
            title=tooltip
            aria-label=described
        >
            {text}
        </a>
    }
}

#[component]
fn FacultySection(faculty: Faculty, bachelor: bool, master: bool) -> impl IntoView {
    let programs = faculty.programs();
    let (anchor, short, name) = (faculty.anchor(), faculty.short(), faculty.name());
    let has_other = faculty.subjects.iter().flat_map(|s| &s.programs).any(|p| Stage::of(p) == Stage::Other);
    // The columns are the same in every section (so they line up down the whole page); the
    // narrow last one is labelled only where something is in it.
    let stages: Vec<Stage> = [(Stage::Bachelor, bachelor), (Stage::Master, master), (Stage::Other, true)].iter().filter(|(_, shown)| *shown).map(|(stage, _)| *stage).collect();
    let head = stages.clone();
    view! {
        <section class="panel faculty" class:has-bachelor=bachelor class:has-master=master id=anchor>
            <header class="faculty-head">
                <span class="faculty-code">{short}</span>
                <h2>{name.clone()}</h2>
                <span class="tab-count">{programs}{if programs == 1 { " Studiengang" } else { " Studiengänge" }}</span>
            </header>
            {faculty.department.is_none().then(|| view! {
                <p class="hint">"Für diese Studiengänge lässt sich aus den Daten keine Fakultät eindeutig ableiten, zum Beispiel weil mehrere Fakultäten sie gemeinsam tragen."</p>
            })}
            <div class="subjects" role="table" aria-label=name>
                <div class="matrix-row matrix-head label" role="row">
                    <span role="columnheader">"Fach"</span>
                    {head.into_iter().map(|stage| view! {
                        <span role="columnheader">{(stage != Stage::Other || has_other).then(|| stage.label())}</span>
                    }).collect_view()}
                </div>
                {faculty.subjects.into_iter().map(|subject| {
                    let Subject { title, programs } = subject;
                    view! {
                        <div class="matrix-row subject" role="row">
                            <span class="subject-name" role="rowheader">{title.clone()}</span>
                            {stages.iter().map(|stage| {
                                let of_stage: Vec<Program> = programs.iter().filter(|p| Stage::of(p) == *stage).cloned().collect();
                                let empty = of_stage.is_empty();
                                view! {
                                    <span class="cell" class:empty=empty role="cell" data-stage=stage.code()>
                                        {groups(of_stage, &title).into_iter().map(|group| view! {
                                            <span class="pill-group">
                                                {program_link(&group.main, &title, false)}
                                                {group.forms.iter().map(|form| program_link(form, &title, true)).collect_view()}
                                            </span>
                                        }).collect_view()}
                                    </span>
                                }
                            }).collect_view()}
                        </div>
                    }
                }).collect_view()}
            </div>
        </section>
    }
}
