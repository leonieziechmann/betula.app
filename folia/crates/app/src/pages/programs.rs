//! The program overview: one section per faculty, and in it a matrix: a row per subject, a
//! column per cycle of study (Bachelor, Master, the rest). 148 programs are about 80 rows, most
//! of them one line, and everything stands on the same few vertical lines, which is what makes
//! the page calm. (A flow of cards, and a list set in several text columns, were both tried
//! first and read as clutter: rows of unequal height with nothing to line up on.)
//! A program that is also offered in other forms of study (dual, extended, …) carries them as
//! segments of the same control.
//!
//! The sidebar filters (degree, form of study, study plan) and jumps to the faculties; the
//! search of the top bar narrows by name. All of it is in the URL (`folia_routes::url::ProgramsUrl`).
//!
//! No source states a program's faculty. It is derived (`folia_pages::faculties`), the
//! sidebar says on what grounds, and programs without a clear answer have a section of their own.
//!
//! In the browser app the head names the visitor's own program („Mein Studiengang", A.10) and
//! leads to it.

use folia_model::labels::DegreeLevel;
use folia_model::rows::{Department, Program};
use folia_pages::ask::ProgramsOverviewAsk;
use folia_pages as pages;
use folia_pages::ProgramsData;
use folia_routes::url::{self, FormGroup, LevelGroup, ProgramTab, ProgramsUrl};
use leptos::prelude::*;
use leptos_meta::Title;

use crate::data::{use_ask, use_data, PageStatus};
use folia_design::format;
use crate::i18n::{self, use_location, Locale, Texts};
use crate::myprogram::{po_of, program_href, program_name, MineResolved, MyProgram};
use folia_design::nav;
use crate::pending::Pending;
use crate::seo::Seo;
use crate::tabs::{self, Tabs};
use crate::skeleton::FilterGroupsStandin;
use crate::frame::{ErrorState, Frame, Plain};
use folia_design::ui::{Icon, ToggleLink};

/// The browser app (`csr`), or the server rendering the page for everybody.
const APP: bool = cfg!(feature = "csr");

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
    fn short(&self, t: &Texts) -> String {
        match &self.department {
            Some(d) if d.code.chars().all(|c| c.is_ascii_digit()) => (t.programs.faculty)(&d.code),
            Some(d) => d.code.clone(),
            None => t.programs.unassigned_short.to_string(),
        }
    }

    /// The faculty's name as the BTU writes it (data: the same in every language); the section of
    /// programs without a faculty is named in the page's language.
    fn name(&self, t: &Texts) -> String {
        match &self.department {
            Some(d) => d.name_de.clone(),
            None => t.programs.unassigned.to_string(),
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
        faculty.subjects.sort_by_key(|subject| folia_search::fold(&subject.title));
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

/// Whether a program is left by the filters and the search of `url`; the search also finds the
/// degree level as the page's language names it.
fn matches(program: &Program, url: &ProgramsUrl, locale: Locale) -> bool {
    let text = format!("{} {} {}", program.name, program.degree(), program.degree_level.label(locale));
    (url.levels.is_empty() || url.levels.contains(&LevelGroup::of(&program.degree_level)))
        && (url.forms.is_empty() || FormGroup::of(program.study_variant.as_ref()).is_some_and(|form| url.forms.contains(&form)))
        && (!url.with_plan || program.has_plan)
        && folia_search::matches(&text, &url.text)
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

    /// Bachelor and Master as the filter of degrees names them.
    fn label(self, t: &Texts) -> &'static str {
        match self {
            Stage::Bachelor => LevelGroup::Bachelor.label(t.locale),
            Stage::Master => LevelGroup::Master.label(t.locale),
            Stage::Other => t.programs.stage_other,
        }
    }
}

/// The degree as a control says it: „B.Sc.", else „Bachelor", else „ohne Abschluss".
fn degree_short(program: &Program, locale: Locale) -> String {
    program.degree_display.clone().unwrap_or_else(|| program.degree_level.label(locale).to_string())
}

/// What tells a form of study from the plain program of its subject: „dual, Praxis", „erweitert".
/// „Maschinenbau - dual" adds the „dual" of its name where the form does not say it already.
fn form_label(program: &Program, subject: &str, locale: Locale) -> Option<String> {
    let added = program.name.strip_prefix(subject).map(|rest| rest.trim_matches([' ', '-'])).filter(|rest| !rest.is_empty());
    match (added, program.study_variant.as_ref().map(|variant| format::variant_short(variant, locale))) {
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

fn groups(programs: Vec<Program>, subject: &str, locale: Locale) -> Vec<Group> {
    let is_form = |p: &Program| p.study_variant.is_some() || p.name != subject;
    let degree = |p: &Program| degree_short(p, locale);
    let mut programs = programs;
    programs.sort_by_key(|p| (is_form(p), std::cmp::Reverse(p.po_year), degree(p)));
    let mut groups: Vec<Group> = Vec::new();
    for program in programs {
        let base = match is_form(&program) {
            true => groups.iter_mut().find(|g| !is_form(&g.main) && degree(&g.main) == degree(&program) && g.main.po_year == program.po_year),
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

/// The data of the overview, made once per snapshot by a host that can (the server,
/// `folia/crates/server/src/snapshot.rs`) and handed to every render: every filter of the overview shows a part
/// of the same programs, and loading them anew was most of the 31 ms such a page cost the server
/// (load test 2026-09-26).
#[derive(Clone)]
pub struct ProgramsReady(pub std::sync::Arc<pages::ProgramsData>);

#[component]
pub fn ProgramsPage() -> impl IntoView {
    let t = i18n::t();
    let status = PageStatus::capture();
    let location = use_location();
    let url = Memo::new(move |_| ProgramsUrl::parse(&location.search.get()));
    // Loaded and grouped once; the URL only decides what of it is shown. A host that has the data
    // ready for the snapshot (`ProgramsReady`) hands it over; otherwise the page comes once its
    // answer is there (the shell holds the page before it meanwhile).
    let ready = use_context::<ProgramsReady>();
    let has_ready = ready.is_some();
    let asked = use_ask(move || (!has_ready).then_some(ProgramsOverviewAsk {}));
    move || {
    let loaded = match (&ready, asked.get()) {
        (Some(ready), _) => Ok(group(&ready.0)),
        (None, Some(answer)) => answer.map(|data| group(&data)),
        (None, None) => return None,
    };
    let status = status.clone();
    let all = match loaded {
        Ok(all) => all,
        Err(error) => {
            status.for_error(&error);
            return Some(view! { <Title text=t.app.programs/><Plain><ErrorState error/></Plain> }.into_any());
        }
    };
    let total: usize = all.iter().map(Faculty::programs).sum();
    let count_where = |keep: &dyn Fn(&Program) -> bool| all.iter().flat_map(|f| &f.subjects).flat_map(|s| &s.programs).filter(|p| keep(p)).count();
    let level_counts: Vec<(LevelGroup, usize)> = LevelGroup::ALL.iter().map(|level| (*level, count_where(&|p| LevelGroup::of(&p.degree_level) == *level))).collect();
    let form_counts: Vec<(FormGroup, usize)> = FormGroup::ALL.iter().map(|form| (*form, count_where(&|p| FormGroup::of(p.study_variant.as_ref()) == Some(*form)))).collect();
    let plan_count = count_where(&|p| p.has_plan);

    // Coming back from a program's page, the overview shows that program again.
    let now = tabs::location_of(&location.pathname.get_untracked(), &location.search.get_untracked());
    let left_at = Tabs::expect().and_then(|tabs| tabs::page_below(&tabs.before(&now), url::PROGRAMS));
    if let Some(slug) = left_at.filter(|slug| slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')) {
        Effect::new(move |_| {
            // Now, and once more after the browser has restored its own idea of the scroll position.
            let selector = format!(".program-pill[data-id=\"{slug}\"]");
            nav::reveal_selector(&selector);
            set_timeout(move || { nav::reveal_selector(&selector); }, std::time::Duration::from_millis(220));
        });
    }

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
                        .map(|subject| Subject { title: subject.title.clone(), programs: subject.programs.iter().filter(|p| matches(p, &url, t.locale)).cloned().collect() })
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

    // Every control of the sidebar is a link to the overview it leads to (as in the catalog), in the
    // page's language. They show the filter the app is going to at once (`pending`); the matrix
    // follows the address.
    let going = Pending::expect();
    let going_url = Memo::new(move |_| going.and_then(|going| going.search_on(url::PROGRAMS)).map(|search| ProgramsUrl::parse(&search)));
    let shown_url = Memo::new(move |_| going_url.get().unwrap_or_else(|| url.get()));
    let toggled = move |change: fn(&mut ProgramsUrl, usize), index: usize| {
        Signal::derive(move || {
            let mut next = shown_url.get();
            change(&mut next, index);
            t.path(&next.path())
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

    // The site's overview is the list by faculty and its jumps (§4.1): its filters are the app's,
    // and until it runs their place holds their bars.
    let sidebar = move || {
        view! {
            {(!APP).then(|| view! { <FilterGroupsStandin groups=&[4, 3, 1]/> })}
            {APP.then(|| view! {
            <div class="fgroup first">
                <div class="flabel label">{t.programs.level}</div>
                <div class="chips">
                    {level_counts.iter().enumerate().filter(|(_, (_, count))| *count > 0).map(|(index, (level, count))| {
                        let level = *level;
                        view! {
                            <ToggleLink
                                label=level.label(t.locale)
                                count=*count
                                on=Signal::derive(move || shown_url.with(|u| u.levels.contains(&level)))
                                href=toggled(|u, index| flip(&mut u.levels, LevelGroup::ALL, LevelGroup::ALL.get(index)), index)
                            />
                        }
                    }).collect_view()}
                </div>
            </div>
            <div class="fgroup">
                <div class="flabel label">{t.programs.form}</div>
                <div class="chips">
                    {form_counts.iter().enumerate().filter(|(_, (_, count))| *count > 0).map(|(index, (form, count))| {
                        let form = *form;
                        view! {
                            <ToggleLink
                                label=form.label(t.locale)
                                count=*count
                                on=Signal::derive(move || shown_url.with(|u| u.forms.contains(&form)))
                                href=toggled(|u, index| flip(&mut u.forms, FormGroup::ALL, FormGroup::ALL.get(index)), index)
                            />
                        }
                    }).collect_view()}
                </div>
            </div>
            <div class="fgroup">
                <div class="flabel label">{t.programs.data}</div>
                <div class="chips">
                    <ToggleLink
                        label=t.programs.with_plan
                        count=plan_count
                        on=Signal::derive(move || url.with(|u| u.with_plan))
                        href=toggled(|u, _| u.with_plan = !u.with_plan, 0)
                    />
                </div>
            </div>
            })}
            <nav class="fgroup toc jumps" aria-label=t.programs.faculties>
                <p class="flabel label">{t.programs.faculties}</p>
                {move || shown.get().faculties.into_iter().map(|faculty| view! {
                    <a href=format!("#{}", faculty.anchor()) data-action="jump" title=faculty.name(t)>
                        <b>{faculty.short(t)}</b>
                        <span class="toc-name">{faculty.name(t)}</span>
                        <span class="num">{faculty.programs()}</span>
                    </a>
                }).collect_view()}
                <p class="hint">{t.programs.faculties_hint}</p>
            </nav>
            <div class="filter-actions sheet-only">
                <a class="btn primary" href="#" data-action="sheet-close">
                    {move || {
                        let n = shown.with(|s| s.programs as u64);
                        (t.programs.show_programs)(n, &format::count(n, t.locale))
                    }}
                </a>
            </div>
        }
    };
    let head = move || {
        view! {
            {move || url.with(ProgramsUrl::is_filtered).then(|| view! {
                <a class="ghost" href=t.path(url::PROGRAMS) data-noscroll=""><Icon name="rotate-ccw"/>{t.common.reset}</a>
            })}
        }
    };

    let page = view! {
        <Title text=t.programs.title/>
        <Frame title=t.programs.filters head sidebar sheet=true>
            <div class="page-inner">
                {move || {
                    let here = url.get();
                    view! {
                        <Seo
                            title=t.programs.seo_title
                            description=(t.programs.seo_description)(total)
                            path=here.path()
                            noindex=here.is_filtered()
                        />
                    }
                }}
                // The same opening as the catalog's list: the number, then what it counts.
                <header class="summary">
                    <h1>
                        <span class="count num">{move || format::count(shown.with(|s| s.programs as u64), t.locale)}</span>
                        <span class="count-label">
                            {move || shown.with(|shown| match (shown.filtered, shown.text.is_empty()) {
                                (false, _) => t.programs.count_all.to_string(),
                                (true, true) => (t.programs.count_of)(total),
                                (true, false) => (t.programs.count_of_matching)(total, &shown.text),
                            })}
                        </span>
                    </h1>
                    <MineLine/>
                    <a class="sheet-toggle" href="#sidebar" data-action="sheet-open"><Icon name="sliders-horizontal"/>{t.programs.filters}</a>
                </header>
                {move || {
                    let shown = shown.get();
                    if shown.faculties.is_empty() {
                        return view! {
                            <div class="state">
                                <p class="state-title">{t.programs.none_title}</p>
                                <p>{t.programs.none_hint}</p>
                                <a class="btn secondary" href=t.path(url::PROGRAMS)>{t.programs.show_all}</a>
                            </div>
                        }.into_any();
                    }
                    let (bachelor, master) = (shown.bachelor, shown.master);
                    shown.faculties.into_iter().map(|faculty| view! { <FacultySection faculty bachelor master/> }).collect_view().into_any()
                }}
            </div>
        </Frame>
    }
    .into_any();
    Some(page)
    }
}

/// „Mein Studiengang: Informatik B.Sc. · PO 2008" in the head of the overview (A.10): the way to the
/// visitor's own program, with the stored Studienrichtung's plan (`program_href`). It is the app's
/// (R9): the server writes an empty box in its place, which keeps the line's room from the first
/// paint where the browser keeps a program (`html.mine`, set by `HEAD_SCRIPT`), so the list does
/// not move when the app takes over (R15).
///
/// A program gone from the snapshot keeps the line, which says so first: on a phone the line ends
/// in „…" where it is too long, and that must take the name, not the news. It leads to the newest
/// PO of the program, where „Als meinen Studiengang setzen" takes that one instead (as the
/// Studienplan's „PO 2008 übernehmen" does); without one it leads nowhere.
#[component]
fn MineLine() -> impl IntoView {
    if !APP {
        return view! { <span class="mine-line mine-room" aria-hidden="true"></span> }.into_any();
    }
    let t = i18n::t();
    let mine = MyProgram::expect();
    let resolved = MineResolved::expect();
    let source = use_data().ok();
    // Siblings, each from its own source (R16): what the store says, and what the catalog knows —
    // the program while it is in the snapshot (`true`), else the newest PO of its family, if any.
    let stored = Memo::new(move |_| {
        mine.and_then(|mine| mine.with(|doc| doc.program.as_ref().map(|id| (doc.name.clone().unwrap_or_else(|| id.clone()), doc.caption.clone(), doc.direction.clone()))))
    });
    let known = Memo::new(move |_| resolved?.0.with(|info| info.as_ref().map(|info| (info.program.clone(), info.exact))));
    (move || {
        let (name, caption, direction) = stored.get()?;
        Some(match known.get() {
            Some((program, true)) => {
                let (href, name) = (program_href(source.as_ref(), &program, caption.as_deref(), direction.as_deref()), program_name(&program));
                let title = (t.programs.mine_title)(&name);
                view! {
                    <a class="mine-line" href=t.path(&href) title=title>
                        <Icon name="star"/><span><small>{t.programs.mine_prefix}</small>{name}</span><Icon name="chevron-right"/>
                    </a>
                }
                .into_any()
            }
            latest => {
                let gone_text = (t.programs.gone)(&name);
                let text = view! { <Icon name="star"/><span><small>{t.programs.gone_prefix}</small>{name}</span> };
                match latest.map(|(latest, _)| latest) {
                    Some(latest) => {
                        let (href, title) = (program_href(source.as_ref(), &latest, None, None), format!("{gone_text} {}", (t.programs.to_po)(&po_of(&latest))));
                        view! { <a class="mine-line" href=t.path(&href) title=title>{text}<Icon name="chevron-right"/></a> }.into_any()
                    }
                    None => view! { <span class="mine-line" title=gone_text>{text}</span> }.into_any(),
                }
            }
        })
    })
    .into_any()
}

/// One link of the matrix. `as_form`: a segment behind its plain program, saying only what differs.
fn program_link(program: &Program, subject: &str, as_form: bool, t: &'static Texts) -> impl IntoView {
    let year = program.po_year.map(|year| year.to_string()).unwrap_or_else(|| program.po_version.clone());
    let form = form_label(program, subject, t.locale);
    let described = format!(
        "{} · {} · PO {}{}",
        program.name,
        program.degree(),
        program.po_version,
        program.study_variant.as_ref().map(|v| format!(" · {}", v.label(t.locale))).unwrap_or_default()
    );
    let tooltip = format!(
        "{described} · {} · {}",
        (t.programs.modules)(program.curricular_modules),
        if program.has_plan { t.programs.plan_checked } else { t.programs.plan_unchecked }
    );
    let text = if as_form {
        view! { <span>{form.unwrap_or_else(|| program.name.clone())}</span> }.into_any()
    } else {
        view! {
            <b>{degree_short(program, t.locale)}</b>
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
            data-id=program.slug.clone()
            href=t.path(&url::program_path(&program.slug, ProgramTab::Plan))
            title=tooltip
            aria-label=described
        >
            {text}
        </a>
    }
}

#[component]
fn FacultySection(faculty: Faculty, bachelor: bool, master: bool) -> impl IntoView {
    let t = i18n::t();
    let programs = faculty.programs();
    let (anchor, short, name) = (faculty.anchor(), faculty.short(t), faculty.name(t));
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
                <span class="tab-count">{programs}{(t.programs.programs_after)(programs)}</span>
            </header>
            {faculty.department.is_none().then(|| view! {
                <p class="hint">{t.programs.unassigned_hint}</p>
            })}
            <div class="subjects" role="table" aria-label=name>
                <div class="matrix-row matrix-head label" role="row">
                    <span role="columnheader">{t.programs.subject}</span>
                    {head.into_iter().map(|stage| view! {
                        <span role="columnheader">{(stage != Stage::Other || has_other).then(|| stage.label(t))}</span>
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
                                        {groups(of_stage, &title, t.locale).into_iter().map(|group| view! {
                                            <span class="pill-group">
                                                {program_link(&group.main, &title, false, t)}
                                                {group.forms.iter().map(|form| program_link(form, &title, true, t)).collect_view()}
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The page names a faculty in its language; the faculty's own name is the BTU's.
    #[test]
    fn a_faculty_is_named_in_the_page_s_language() {
        let (de, en) = (i18n::texts(Locale::De), i18n::texts(Locale::En));
        let faculty = |code: &str| Faculty {
            department: Some(Department { id: 1, code: code.to_string(), label: String::new(), name_de: "Mathematik, Informatik, Physik".to_string(), name_en: None, modules: 0 }),
            subjects: Vec::new(),
        };
        let none = Faculty { department: None, subjects: Vec::new() };
        assert_eq!((faculty("1").short(de), faculty("1").short(en), faculty("ZE").short(en)), ("Fakultät 1".to_string(), "Faculty 1".to_string(), "ZE".to_string()));
        assert_eq!((none.short(de), none.short(en)), ("Ohne Zuordnung".to_string(), "Unassigned".to_string()));
        assert_eq!(faculty("1").name(en), "Mathematik, Informatik, Physik");
        assert_eq!((Stage::Other.label(de), Stage::Other.label(en), Stage::Master.label(en)), ("Weitere", "Other", "Master"));
        assert_eq!(((de.programs.show_programs)(1, "1"), (en.programs.show_programs)(1, "1")), ("1 Studiengänge anzeigen".to_string(), "Show 1 degree programme".to_string()));
        assert_eq!((en.programs.show_programs)(1204, "1,204"), "Show 1,204 degree programmes");
    }
}
