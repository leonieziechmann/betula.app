//! The program overview: one accordion per faculty (owner, 2026-10-14: „Ich mag die akkordion
//! elemente von den Fakultäten"), and in it on a wide page a matrix: a row per subject, a column
//! per cycle of study (Bachelor, Master, the rest), each program a line of text — its degree, the
//! form of study it adds, how long and how many credits its validated plan says. 148 programs are
//! about 80 rows, and everything stands on the same few vertical lines, which is what makes the
//! page calm. (A flow of cards, and a list set in several text columns, were both tried first and
//! read as clutter; boxes of programs in the cells read as „eine Katastrophe" on a phone, owner,
//! 2026-10-14.) On a phone or a narrow page a row per subject, which folds open where it has
//! several programs (owner, the same day: „Variante 1 mit ausklappen"); the fourteen combinations
//! of the Lehramt Primarstufe are one subject.
//!
//! The sidebar filters (degree, form of study, study plan) and jumps to the faculties; the
//! search of the top bar narrows by name. All of it is in the URL (`folia_routes::url::ProgramsUrl`).
//!
//! No source states a program's faculty. It is derived (`folia_pages::faculties`), the
//! sidebar says on what grounds, and programs without a clear answer have a section of their own.
//!
//! In the browser app the head names the visitor's own program („Mein Studiengang", A.10) with
//! the way to „Mein Studium", and the Master after it (or the Bachelor before it); the list marks
//! it.

use std::collections::BTreeMap;

use folia_model::labels::DegreeLevel;
use folia_model::rows::{Department, Program};
use folia_pages::ask::{CounterpartAsk, ProgramsOverviewAsk};
use folia_pages as pages;
use folia_pages::{PlanFacts, ProgramsData};
use folia_routes::url::{self, FormGroup, LevelGroup, ProgramTab, ProgramsUrl};
use leptos::prelude::*;
use leptos_meta::Title;

use folia_data::{use_ask, use_data, PageStatus};
use folia_design::format;
use crate::i18n::{self, use_location, Locale, Texts};
use folia_stores::myprogram::{po_of, program_href, MineResolved, MyProgram};
use folia_design::nav;
use folia_shell::pending::Pending;
use folia_shell::seo::Seo;
use folia_shell::tabs::{self, Tabs};
use folia_shell::skeleton::FilterGroupsStandin;
use folia_shell::frame::{ErrorState, Frame, Plain};
use folia_design::ui::{Icon, ToggleLink};

/// The browser app (`csr`), or the server rendering the page for everybody.
const APP: bool = cfg!(feature = "csr");

/// All programs of one subject („Maschinenbau" with its degrees and forms of study); the
/// combinations of a Lehramt („Lehramt Primarstufe" with „Deutsch – Englisch" …) one subject too.
#[derive(Clone, PartialEq)]
struct Subject {
    title: String,
    programs: Vec<Program>,
    /// The combinations of a Lehramt: a program is named by its combination.
    combos: bool,
}

/// „Lehramt Primarstufe Deutsch-Englisch" → („Lehramt Primarstufe", „Deutsch – Englisch"), and the
/// dual one the same („Lehramt Primarstufe - dual Mathematik-Deutsch"): the school a Lehramt is for
/// and the combination of subjects.
fn lehramt_of(name: &str) -> Option<(String, String)> {
    let rest = name.strip_prefix("Lehramt ")?;
    let (school, combo) = rest.split_once(' ')?;
    let combo = combo.trim().strip_prefix("- dual").map(str::trim).unwrap_or(combo.trim());
    (!combo.is_empty()).then(|| (format!("Lehramt {school}"), combo.replacen('-', " – ", 1)))
}

/// What puts programs into one subject: the combinations of a Lehramt by their school, the others
/// by their name („Maschinenbau - dual" is a form of the subject „Maschinenbau").
fn subject_key(program: &Program) -> String {
    match lehramt_of(&program.name) {
        Some((school, _)) => format!("lehramt:{school}"),
        None => program.name_key.trim_end_matches("-dual").to_string(),
    }
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
        let key = subject_key(program);
        match faculty.subjects.iter_mut().find(|s| s.programs.first().is_some_and(|p| subject_key(p) == key)) {
            Some(subject) => subject.programs.push(program.clone()),
            None => faculty.subjects.push(Subject { title: String::new(), programs: vec![program.clone()], combos: false }),
        }
    }
    for faculty in &mut faculties {
        for subject in &mut faculty.subjects {
            subject.programs.sort_by_key(|p| (level_order(p), p.study_variant.is_some(), std::cmp::Reverse(p.po_year)));
            match subject.programs.first().and_then(|p| lehramt_of(&p.name)).filter(|_| subject.programs.len() > 1) {
                Some((school, _)) => {
                    subject.title = school;
                    subject.combos = true;
                }
                None => subject.title = subject.programs.iter().map(|p| p.name.clone()).min_by_key(|name| name.chars().count()).unwrap_or_default(),
            }
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
    other: bool,
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
    let facts_of = |data: &ProgramsData| data.facts.iter().map(|facts| (facts.program_id.clone(), facts.clone())).collect::<BTreeMap<String, PlanFacts>>();
    let loaded = match (&ready, asked.get()) {
        (Some(ready), _) => Ok((group(&ready.0), facts_of(&ready.0))),
        (None, Some(answer)) => answer.map(|data| (group(&data), facts_of(&data))),
        (None, None) => return None,
    };
    let status = status.clone();
    let (all, facts) = match loaded {
        Ok(loaded) => loaded,
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

    // Coming back from a program's page, the overview shows that program again: its faculty open,
    // on a phone its subject too.
    let now = tabs::location_of(&location.pathname.get_untracked(), &location.search.get_untracked());
    let left_at = Tabs::expect().and_then(|tabs| tabs::page_below(&tabs.before(&now), url::PROGRAMS));
    if let Some(slug) = left_at.filter(|slug| slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')) {
        Effect::new(move |_| {
            // Now, and once more after the browser has restored its own idea of the scroll position.
            let selector = shown_link(&slug);
            open_around(&selector);
            nav::reveal_selector(&selector);
            set_timeout(move || { nav::reveal_selector(&selector); }, std::time::Duration::from_millis(220));
        });
    }

    let all = StoredValue::new(all);
    let facts = StoredValue::new(facts);
    // „Mein Studiengang" while the catalog knows it in this PO: the list marks it (the app's, R9).
    let resolved = MineResolved::expect();
    let mine = Memo::new(move |_| resolved.and_then(|resolved| resolved.0.with(|info| info.as_ref().filter(|info| info.exact).map(|info| info.program.slug.clone()))));
    let shown = Memo::new(move |_| {
        let url = url.get();
        let faculties: Vec<Faculty> = all.with_value(|all| {
            all.iter()
                .map(|faculty| Faculty {
                    department: faculty.department.clone(),
                    subjects: faculty
                        .subjects
                        .iter()
                        .map(|subject| Subject { title: subject.title.clone(), programs: subject.programs.iter().filter(|p| matches(p, &url, t.locale)).cloned().collect(), combos: subject.combos })
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
            other: any(Stage::Other),
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
                    <a class="sheet-toggle" href="#sidebar" data-action="sheet-open"><Icon name="sliders-horizontal"/>{t.programs.filters}</a>
                </header>
                <MineStrip all facts/>
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
                    // Searched or filtered, every faculty with what is left is open; else they wait to be
                    // opened.
                    let (bachelor, master, other, open) = (shown.bachelor, shown.master, shown.other, shown.filtered);
                    view! {
                        <div class="faccs">
                            {shown.faculties.into_iter().map(|faculty| view! { <FacultySection faculty bachelor master other open facts mine/> }).collect_view()}
                        </div>
                    }
                    .into_any()
                }}
            </div>
        </Frame>
    }
    .into_any();
    Some(page)
    }
}

/// The visitor's own program at the head of the overview (the app's, R9; owner, 2026-10-14,
/// concept A: „Dein Studiengang und der Master danach oben"): the program, the way to „Mein
/// Studium", and the program after it or before it (the best-matching Master of a Bachelor,
/// `v_program_counterpart`, as its page names it) with its plan. The server writes an empty box
/// in its place, which keeps the strip's room from the first paint where the browser keeps a
/// program (`html.mine`, set by `HEAD_SCRIPT`), so the list does not move when the app takes over
/// (R15).
///
/// A program gone from the snapshot keeps the strip, which says so first and leads to the newest PO
/// of the program (taken instead in „Mein Studium", as the Studienplan's „PO 2008 übernehmen"
/// does); without one it leads nowhere.
#[component]
fn MineStrip(all: StoredValue<Vec<Faculty>>, facts: StoredValue<BTreeMap<String, PlanFacts>>) -> impl IntoView {
    if !APP {
        return view! { <div class="mine-strip mine-room" aria-hidden="true"></div> }.into_any();
    }
    let t = i18n::t();
    let p = &t.programs;
    let mine = MyProgram::expect();
    let resolved = MineResolved::expect();
    // Siblings, each from its own source (R16): what the store says, and what the catalog knows —
    // the program while it is in the snapshot (`true`), else the newest PO of its family, if any.
    let stored = Memo::new(move |_| mine.and_then(|mine| mine.with(|doc| doc.program.as_ref().map(|id| doc.name.clone().unwrap_or_else(|| id.clone())))));
    let known = Memo::new(move |_| resolved?.0.with(|info| info.as_ref().map(|info| (info.program.clone(), info.exact))));
    let exact = Memo::new(move |_| known.with(|known| known.as_ref().filter(|(_, exact)| *exact).map(|(program, _)| program.id.clone())));
    let counterpart = use_ask(move || exact.get().map(|program_id| CounterpartAsk { program_id }));
    let program_of = move |slug: &str| all.with_value(|all| all.iter().flat_map(|f| &f.subjects).flat_map(|s| &s.programs).find(|program| program.slug == slug).cloned());
    (move || {
        let name = stored.get()?;
        Some(match known.get() {
            Some((program, true)) => {
                let facts_line = facts.with_value(|facts| facts_text(&program, facts, true, t));
                let line = [Some(format!("PO {}", program.po_version)), facts_line, Some(p.in_study.to_string())].into_iter().flatten().collect::<Vec<_>>().join(" · ");
                let other = counterpart.get().and_then(Result::ok).flatten().and_then(|other| program_of(&other.slug).map(|program| (other, program)));
                let after = Stage::of(&program) == Stage::Bachelor;
                view! {
                    <section class="panel mine-strip" aria-label=p.yours>
                        <a class="ms-col" href=t.path(url::STUDY)>
                            <Icon name="star"/>
                            <span class="ms-t"><small>{p.yours}</small><b>{format!("{} {}", program.name, program.degree())}</b><span>{line}</span></span>
                            <span class="ms-go">{p.to_study}<Icon name="chevron-right"/></span>
                        </a>
                        {other.map(|(other, found)| {
                            let line = [Some(format!("PO {}", other.po_version)), facts.with_value(|facts| facts_text(&found, facts, true, t))].into_iter().flatten().collect::<Vec<_>>().join(" · ");
                            view! {
                                <a class="ms-col" href=t.path(&url::program_path(&other.slug, ProgramTab::Plan)) data-id=other.slug.clone()>
                                    <Icon name="graduation-cap"/>
                                    <span class="ms-t"><small>{if after { p.after } else { p.before }}</small><b>{format!("{} {}", found.name, found.degree())}</b><span>{line}</span></span>
                                    <span class="ms-go">{p.to_plan}<Icon name="chevron-right"/></span>
                                </a>
                            }
                        })}
                    </section>
                }
                .into_any()
            }
            latest => {
                let gone_text = (p.gone)(&name);
                let text = view! { <Icon name="star"/><span class="ms-t"><small>{p.gone_prefix}</small><b>{name}</b></span> };
                match latest.map(|(latest, _)| latest) {
                    Some(latest) => {
                        let href = program_href(use_data().ok().as_ref(), &latest, None, None);
                        let title = format!("{gone_text} {}", (p.to_po)(&po_of(&latest)));
                        view! { <section class="panel mine-strip"><a class="ms-col" href=t.path(&href) title=title>{text}<span class="ms-go">{(p.to_po)(&po_of(&latest))}<Icon name="chevron-right"/></span></a></section> }.into_any()
                    }
                    None => view! { <section class="panel mine-strip"><span class="ms-col" title=gone_text>{text}</span></section> }.into_any(),
                }
            }
        })
    })
    .into_any()
}

/// The link of a program as the page shows it at its width: in the matrix, or in a phone's list.
fn shown_link(slug: &str) -> String {
    let list = if nav::is_phone() || narrow_list() { ".fx-phone" } else { ".fx-desk" };
    format!("{list} a[data-id=\"{slug}\"]")
}

/// The faculties are too narrow for their matrix (app.css: the container query of `.faccs`).
fn narrow_list() -> bool {
    #[cfg(feature = "csr")]
    {
        leptos::web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.query_selector(".faccs").ok().flatten())
            .is_some_and(|faccs| faccs.get_bounding_client_rect().width() < 761.0)
    }
    #[cfg(not(feature = "csr"))]
    false
}

/// Opens the accordions around what `selector` finds: its faculty, on a phone its subject.
fn open_around(selector: &str) {
    #[cfg(feature = "csr")]
    {
        let Some(found) = leptos::web_sys::window().and_then(|window| window.document()).and_then(|document| document.query_selector(selector).ok().flatten()) else { return };
        let mut at = found.parent_element();
        while let Some(element) = at {
            if element.tag_name().eq_ignore_ascii_case("details") {
                let _ = element.set_attribute("open", "");
            }
            at = element.parent_element();
        }
    }
    #[cfg(not(feature = "csr"))]
    let _ = selector;
}

/// How long a program is and how many credits it comes to, as its validated plan says: „6 Sem. ·
/// 180 LP" (`short`), „6 Semester · 180 LP"; `None` without a validated plan, or where it says
/// neither.
fn facts_text(program: &Program, facts: &BTreeMap<String, PlanFacts>, short: bool, t: &Texts) -> Option<String> {
    let facts = facts.get(&program.id).filter(|_| program.has_plan)?;
    let span = |(low, high): (f64, f64)| match (high - low).abs() < 0.01 {
        true => format::number(low, t.locale),
        false => format!("{}–{}", format::number(low, t.locale), format::number(high, t.locale)),
    };
    let semesters = facts.semesters.map(|(low, high)| span((low as f64, high as f64))).map(|n| if short { (t.programs.semesters_short)(&n) } else { (t.programs.semesters)(&n) });
    let credits = facts.credits.map(span).map(|n| (t.programs.credits)(&n));
    let parts: Vec<String> = [semesters, credits].into_iter().flatten().collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// A program as a line of the list: its degree (with its PO where its column has two of one
/// degree), its combination of a Lehramt, or the form of study it adds to the program above it.
#[derive(Clone)]
struct Line {
    program: Program,
    /// What it is called in its cell: „B.Sc.", „Deutsch – Englisch", „dual, Praxis".
    label: String,
    /// „PO 2008" where its column has two of its degree.
    po: Option<String>,
    /// The form of study of the first line of a program: „dual, Praxis".
    form: Option<String>,
    /// A form of study under its program.
    sub: bool,
    /// „6 Semester · 180 LP" and „6 Sem. · 180 LP"; `None` without a validated plan.
    facts: Option<String>,
    facts_short: Option<String>,
}

/// The lines of the programs of one column of a subject.
fn lines_of(programs: Vec<Program>, subject: &Subject, facts: &BTreeMap<String, PlanFacts>, t: &Texts) -> Vec<Line> {
    let line = |program: Program, label: String, po: Option<String>, form: Option<String>, sub: bool| Line {
        facts: facts_text(&program, facts, false, t),
        facts_short: facts_text(&program, facts, true, t),
        program,
        label,
        po,
        form,
        sub,
    };
    if subject.combos {
        let mut programs = programs;
        programs.sort_by_key(|program| lehramt_of(&program.name).map(|(_, combo)| combo).unwrap_or_default());
        return programs
            .into_iter()
            .map(|program| {
                let combo = lehramt_of(&program.name).map_or_else(|| program.name.clone(), |(_, combo)| combo);
                let form = program.study_variant.as_ref().map(|variant| format::variant_short(variant, t.locale));
                line(program, combo, None, form, false)
            })
            .collect();
    }
    let groups = groups(programs, &subject.title, t.locale);
    let two = groups.len() > 1;
    groups
        .into_iter()
        .flat_map(|group| {
            let main = {
                let form = form_label(&group.main, &subject.title, t.locale);
                let po = two.then(|| format!("PO {}", group.main.po_year.map(|year| year.to_string()).unwrap_or_else(|| group.main.po_version.clone())));
                line(group.main.clone(), degree_short(&group.main, t.locale), po, form, false)
            };
            let forms: Vec<Line> = group
                .forms
                .into_iter()
                .map(|program| {
                    let label = form_label(&program, &subject.title, t.locale).unwrap_or_else(|| program.name.clone());
                    line(program, label, None, None, true)
                })
                .collect();
            std::iter::once(main).chain(forms)
        })
        .collect()
}

/// A program's title: what it is, with its PO and form, whether its plan is checked.
fn described(program: &Program, t: &Texts) -> (String, String) {
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
    (described, tooltip)
}

/// A line of a cell of the desktop's matrix: the degree or combination, the form, the plan's facts.
fn desk_line(line: Line, mine: Memo<Option<String>>, t: &'static Texts) -> impl IntoView {
    let slug = line.program.slug.clone();
    let here = slug.clone();
    let is_mine = Signal::derive(move || mine.with(|mine| mine.as_deref() == Some(here.as_str())));
    let (described, tooltip) = described(&line.program, t);
    let facts = line.facts_short.clone().unwrap_or_else(|| t.programs.no_plan.to_string());
    view! {
        <a class="prog-line" class:sub=line.sub class:quiet=!line.program.has_plan class:is-mine=is_mine
            data-walk="program-link" data-id=slug.clone() href=t.path(&url::program_path(&slug, ProgramTab::Plan)) title=tooltip aria-label=described>
            <b>
                {line.label}
                {line.po.map(|po| view! { <span class="pl-po">{po}</span> })}
                {line.form.map(|form| view! { <span class="pl-form">{form}</span> })}
                {move || is_mine.get().then(|| view! { <Icon name="star" class="mine-star"/> })}
            </b>
            <span class="pl-f">{facts}</span>
        </a>
    }
}

/// An offer of a phone's folded subject: as `desk_line`, with the way on at its end.
fn phone_line(line: Line, mine: Memo<Option<String>>, t: &'static Texts) -> impl IntoView {
    let slug = line.program.slug.clone();
    let here = slug.clone();
    let is_mine = Signal::derive(move || mine.with(|mine| mine.as_deref() == Some(here.as_str())));
    let (described, tooltip) = described(&line.program, t);
    let facts = line.facts_short.clone().unwrap_or_else(|| t.programs.no_plan.to_string());
    view! {
        <a class="of" class:sub=line.sub class:quiet=!line.program.has_plan class:is-mine=is_mine
            data-walk="program-link" data-id=slug.clone() href=t.path(&url::program_path(&slug, ProgramTab::Plan)) title=tooltip aria-label=described>
            <span class="of-t">
                <b>{line.label}{line.po.map(|po| view! { <span class="pl-po">{po}</span> })}</b>
                {line.form.map(|form| view! { <span class="of-form">{form}</span> })}
                {move || is_mine.get().then(|| view! { <em><Icon name="star" class="mine-star"/>{t.programs.mine_mark}</em> })}
            </span>
            <span class="of-f">{facts}</span>
            <Icon name="chevron-right" class="chev"/>
        </a>
    }
}

/// What a faculty has, by degree: „12 Bachelor · 11 Master · 3 Lehramt · 1 weitere".
fn summary_of(faculty: &Faculty, t: &Texts) -> String {
    let programs: Vec<&Program> = faculty.subjects.iter().flat_map(|s| &s.programs).collect();
    let teaching = |p: &&Program| matches!(p.degree_level.known(), Some(DegreeLevel::TeachingBachelor | DegreeLevel::TeachingMaster));
    let count = |stage: Stage| programs.iter().filter(|p| Stage::of(p) == stage && !teaching(p)).count();
    let parts = [
        (count(Stage::Bachelor), t.programs.sum_bachelor),
        (count(Stage::Master), t.programs.sum_master),
        (programs.iter().filter(|p| teaching(p)).count(), t.programs.sum_teaching),
        (count(Stage::Other), t.programs.sum_other),
    ];
    parts.into_iter().filter(|(n, _)| *n > 0).map(|(n, say)| say(n)).collect::<Vec<_>>().join(" · ")
}

/// A faculty as an accordion (owner, 2026-10-14: „Ich mag die akkordion elemente von den
/// Fakultäten"): its head (the faculty, its name, what it has, how many), and in it its programs —
/// on a wide page the matrix of the cycles of study, a row a subject, each program a line of text
/// (its degree, its form, how long and how many credits its plan says); on a phone or a narrow page
/// a row a subject, which folds open where it has several programs (owner, the same day: „Variante
/// 1 mit ausklappen"). Both are `<details>`: they open without JavaScript, and the page's address
/// does not change. The server's HTML holds both, the stylesheet shows the one the width asks for.
#[component]
fn FacultySection(faculty: Faculty, bachelor: bool, master: bool, other: bool, open: bool, facts: StoredValue<BTreeMap<String, PlanFacts>>, mine: Memo<Option<String>>) -> impl IntoView {
    let t = i18n::t();
    let p = &t.programs;
    let programs = faculty.programs();
    let (anchor, short, name, summary) = (faculty.anchor(), faculty.short(t), faculty.name(t), summary_of(&faculty, t));
    let has_other = faculty.subjects.iter().flat_map(|s| &s.programs).any(|p| Stage::of(p) == Stage::Other);
    // The columns are the same in every faculty (so they line up down the whole page); the last
    // one is named only where something is in it.
    let stages: Vec<Stage> = [(Stage::Bachelor, bachelor), (Stage::Master, master), (Stage::Other, other)].iter().filter(|(_, shown)| *shown).map(|(stage, _)| *stage).collect();
    let desk_rows = faculty
        .subjects
        .iter()
        .map(|subject| {
            let cells = stages
                .iter()
                .map(|stage| {
                    let of_stage: Vec<Program> = subject.programs.iter().filter(|p| Stage::of(p) == *stage).cloned().collect();
                    let lines = facts.with_value(|facts| lines_of(of_stage, subject, facts, t));
                    view! { <td class="fx-cell" data-stage=stage.code()>{lines.into_iter().map(|line| desk_line(line, mine, t)).collect_view()}</td> }
                })
                .collect_view();
            let combos = subject.combos.then(|| view! { <small>{(p.combinations)(subject.programs.len())}</small> });
            view! { <tr><th scope="row" class="fx-name">{subject.title.clone()}{combos}</th>{cells}</tr> }
        })
        .collect_view();
    let phone_rows = faculty
        .subjects
        .iter()
        .map(|subject| {
            let all: Vec<Line> = facts.with_value(|facts| {
                [Stage::Bachelor, Stage::Master, Stage::Other]
                    .into_iter()
                    .flat_map(|stage| lines_of(subject.programs.iter().filter(|p| Stage::of(p) == stage).cloned().collect(), subject, facts, t))
                    .collect()
            });
            let slugs: Vec<String> = subject.programs.iter().map(|program| program.slug.clone()).collect();
            let has_mine = move || mine.with(|mine| mine.as_ref().is_some_and(|mine| slugs.contains(mine)));
            if let [only] = all.as_slice() {
                let only = only.clone();
                let slug = only.program.slug.clone();
                let degree = match &only.form {
                    Some(form) => format!("{} · {form}", degree_short(&only.program, t.locale)),
                    None => degree_short(&only.program, t.locale),
                };
                let facts = only.facts.clone().unwrap_or_else(|| p.no_plan.to_string());
                let (described, tooltip) = described(&only.program, t);
                return view! {
                    <a class="sj one" class:quiet=!only.program.has_plan data-walk="program-link" data-id=slug.clone() href=t.path(&url::program_path(&slug, ProgramTab::Plan)) title=tooltip aria-label=described>
                        <span class="sj-t"><b>{subject.title.clone()}{move || has_mine().then(|| view! { <Icon name="star" class="mine-star"/> })}</b><small><span class="dg">{degree}</span>" · "{facts}</small></span>
                        <Icon name="chevron-right" class="chev"/>
                    </a>
                }
                .into_any();
            }
            let mut degrees: Vec<String> = Vec::new();
            for line in &all {
                let degree = degree_short(&line.program, t.locale);
                let degree = if subject.combos { degree.trim_start_matches("Lehramt ").to_string() } else { degree };
                if !degrees.contains(&degree) {
                    degrees.push(degree);
                }
            }
            let count = if subject.combos { (p.combinations)(all.len()) } else { (p.offers)(all.len()) };
            let levels = [Stage::Bachelor, Stage::Master, Stage::Other]
                .into_iter()
                .filter_map(|stage| {
                    let of_stage: Vec<Line> = all.iter().filter(|line| Stage::of(&line.program) == stage).cloned().collect();
                    (!of_stage.is_empty()).then(|| view! { <p class="of-lvl">{stage.label(t)}</p>{of_stage.into_iter().map(|line| phone_line(line, mine, t)).collect_view()} })
                })
                .collect_view();
            view! {
                <details class="sj-w">
                    <summary class="sj">
                        <span class="sj-t"><b>{subject.title.clone()}{move || has_mine().then(|| view! { <Icon name="star" class="mine-star"/> })}</b><small><span class="dg">{degrees.join(" · ")}</span>" · "{count}</small></span>
                        <span class="sj-n">{all.len()}</span>
                        <Icon name="chevron-down" class="chev"/>
                    </summary>
                    <div class="offers">{levels}</div>
                </details>
            }
            .into_any()
        })
        .collect_view();
    view! {
        <details class="panel facc" id=anchor open=open>
            <summary class="facc-h">
                <span class="facc-tag">{short}</span>
                <span class="facc-name">{name.clone()}</span>
                <span class="facc-sum">{summary}</span>
                <span class="facc-n">{programs}<span class="visually-hidden">{(p.programs_after)(programs)}</span></span>
                <Icon name="chevron-down" class="facc-chev"/>
            </summary>
            {faculty.department.is_none().then(|| view! { <p class="hint facc-hint">{p.unassigned_hint}</p> })}
            <table class="fx-desk" aria-label=name>
                <thead>
                    <tr>
                        <th scope="col" class="fx-name">{p.subject}</th>
                        {stages.iter().map(|stage| view! { <th scope="col" data-stage=stage.code()>{(*stage != Stage::Other || has_other).then(|| stage.label(t))}</th> }).collect_view()}
                    </tr>
                </thead>
                <tbody>{desk_rows}</tbody>
            </table>
            <div class="fx-phone">{phone_rows}</div>
        </details>
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
