//! The start page's setup (concept A, „Onboarding: in drei Klicks zum eigenen Stundenplan";
//! folia/design/startseite/konzepte.html). No pop-up and no tour (owner, 2026-09-28: „organisch ohne
//! Popup, einfach intuitiv"): on a first visit the setup is the start page itself, a card of three
//! steps on the photo of the season, one click each (owner, the same day: „immer nur so ein Click pro
//! Step"):
//!
//! 1. „Was studierst du?": the program out of the app's combobox, as everywhere (owner, 2026-10-20:
//!    „die suche nach den studiengängen muss unbedingt eine Combobox sein, wie überall sonst"), and
//!    under it a drawing of the programs, a dot each in its faculty's colour, with their numbers.
//! 2. „Seit wann studierst du?": the six semesters back from now, each with its Fachsemester, and a
//!    field for any other (owner, the same day: „die 6 Semester + ein custom feld"); the one the
//!    program's plan usually begins in stands out.
//! 3. „Dein n. Fachsemester": the rows of the Regelstudienplan in that Fachsemester, ticked, to
//!    untick what is not taken; the earlier Fachsemester filled from the plan and marked as passed
//!    (a switch, on: the usual case); „Studium anlegen" on the right, as a confirming action is in
//!    Betula (owner, the same day), full width on a phone with „Ohne Module weiter" under it.
//!
//! „Studium anlegen" stores the program, its study direction and the Studienbeginn in „Mein
//! Studiengang" and takes the rows over into the current semester of the Stundenplan, as its
//! „Fachsemester übernehmen" does (`studyplan::import_fs`): one plan for the Stundenplan and „Mein
//! Studium". Nothing is refused (CLAUDE.md, „Betula does not say no"): a semester before the
//! regulation or beyond the Regelstudienzeit is said beside it. Each step goes back by its chip at
//! the top. The card is as high in every step; what is long scrolls inside it.
//!
//! On the server (R9) the card is its first step, and without JavaScript „Studiengang wählen" leads
//! to the programs.

use std::collections::BTreeSet;

use folia_calendar::semester::{fachsemester, SemesterKey};
use folia_model::labels::{Code, Labelled, ModuleKind};
use folia_model::rows::{CatalogRow, Department, Program};
use folia_pages::ask::{PlanSourceAsk, ProgramsAsk};
use folia_pages::PlanSource;
use folia_plans::plan;
use folia_plans::study::{self, plan_rows};
use folia_plans::studyplan::{self as stored, DoneRow, Import, PlanDoc};
use folia_plans::variants::PlanVariant;
use folia_routes::url;
use leptos::prelude::*;

use folia_data::use_ask;
use folia_design::combobox::{ComboItem, Combobox};
use folia_design::format;
use folia_design::ui::Icon;
use folia_stores::myprogram::{po_of, program_name, MyProgram, ProgramPlans};
use folia_stores::studyplan::Studyplan;

use crate::i18n::{self, Texts};

/// The browser app (`csr`): only there is anything picked or stored.
const APP: bool = cfg!(feature = "csr");

/// How many semesters back step 2 offers; any other is the field's.
const OFFERED: i32 = 6;

/// The most Fachsemester the field takes, as far as Betula counts them (`folia_calendar::semester`).
const MOST_FS: u8 = 30;

/// What the setup did, for the note after it and its „Rückgängig".
#[derive(Clone, Debug, PartialEq)]
pub struct Done {
    /// „Informatik B.Sc. · PO 2008".
    pub program: String,
    pub fs: u8,
    pub start: SemesterKey,
    pub modules: usize,
    pub placeholders: usize,
    /// The credits of the earlier Fachsemester marked as passed, where they were.
    pub passed: Option<f64>,
    /// The plan and the Studienbeginn before it.
    pub plan_before: PlanDoc,
    pub start_before: Option<SemesterKey>,
}

/// One row of the Regelstudienplan in a Fachsemester, as step 3 lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct SetupRow {
    /// `m:<id>` for a module, `r:<ord>` for a row without one (it becomes a placeholder).
    pub key: String,
    pub name: String,
    pub credits: Option<String>,
    /// What it counts in sums: its credits, the least of a range.
    pub weight: f64,
    pub kind: Option<ModuleKind>,
    pub span: (u8, u8),
    pub linked: bool,
}

/// The plan `index` of `source` (0-based, the program page's order): its core plan, and the page
/// that fills a row of it with that row's `ord` (`variants::supplements`), as „Mein Studium"
/// reads a kept study direction.
pub fn chosen(source: &PlanSource, index: usize) -> (Option<&PlanVariant>, Option<(&PlanVariant, i64)>) {
    let Some(last) = source.variants.len().checked_sub(1) else { return (None, None) };
    let shown = index.min(last);
    match source.supplements.iter().find(|supplement| supplement.page == shown) {
        Some(supplement) => (source.variants.get(supplement.core), source.variants.get(supplement.page).map(|page| (page, supplement.ord))),
        None => (source.variants.get(shown), None),
    }
}

/// The rows of the plan in Fachsemester `fs`, as „Fachsemester übernehmen" takes them
/// (`studyplan::import_fs`): a module once, a row without one as a placeholder; rows of prose are
/// none.
pub fn rows_in(source: &PlanSource, core: Option<&PlanVariant>, page: Option<(&PlanVariant, i64)>, fs: u8) -> Vec<SetupRow> {
    rows_where(source, core, page, |first, last| first <= fs && fs <= last)
}

/// The rows of the Fachsemester before `fs`, each once (a row over two of them counts once).
pub fn rows_before(source: &PlanSource, core: Option<&PlanVariant>, page: Option<(&PlanVariant, i64)>, fs: u8) -> Vec<SetupRow> {
    rows_where(source, core, page, |first, _| first < fs)
}

fn rows_where(source: &PlanSource, core: Option<&PlanVariant>, page: Option<(&PlanVariant, i64)>, take: impl Fn(u8, u8) -> bool) -> Vec<SetupRow> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut rows = Vec::new();
    for row in plan_rows(core, page) {
        if !take(row.first, row.last) {
            continue;
        }
        let entry = row.entry;
        let module = entry.module_id.as_deref().filter(|id| url::is_module_id(id));
        let key = match module {
            Some(id) => format!("m:{id}"),
            None => format!("r:{}", entry.ord),
        };
        if !seen.insert(key.clone()) {
            continue;
        }
        let linked: Option<&CatalogRow> = module.and_then(|id| source.linked.iter().find(|linked| linked.id == id));
        let name = linked.map(|linked| linked.title.clone()).unwrap_or_else(|| plan::shown_name(&entry.module_name).to_string());
        let weight = entry.credits.or(entry.min_credits).or(entry.max_credits).or_else(|| linked.and_then(|linked| linked.credits)).unwrap_or(0.0);
        rows.push(SetupRow {
            key,
            name,
            credits: plan::credits_of(entry).or_else(|| linked.and_then(|linked| linked.credits).map(|credits| format::number(credits, folia_locale::Locale::De))),
            weight,
            kind: entry.kind.as_ref().and_then(Code::known),
            span: (row.first, row.last),
            linked: module.is_some(),
        });
    }
    rows
}

/// The programs counted by faculty (`HomeData::faculties`), those of none at the end.
pub type Counted = Vec<(Option<Department>, usize)>;

/// The faculties of the drawing: the numbered ones by their number, the rest together.
pub struct Faculties {
    /// Code („1"), a short name and the full one, and how many current programs it has.
    pub numbered: Vec<(String, String, String, usize)>,
    pub other: usize,
}

impl Faculties {
    pub fn of(counted: &[(Option<Department>, usize)], t: &Texts) -> Self {
        let mut numbered: Vec<(String, String, String, usize)> = Vec::new();
        let mut other = 0;
        for (department, programs) in counted {
            match department.as_ref().filter(|d| !d.code.is_empty() && d.code.chars().all(|c| c.is_ascii_digit())) {
                Some(d) => {
                    let full = match t.locale {
                        folia_locale::Locale::De => d.name_de.clone(),
                        folia_locale::Locale::En => d.name_en.clone().unwrap_or_else(|| d.name_de.clone()),
                    };
                    match numbered.iter_mut().find(|(code, ..)| *code == d.code) {
                        Some((.., count)) => *count += programs,
                        None => numbered.push((d.code.clone(), short_name(&full), full, *programs)),
                    }
                }
                None => other += programs,
            }
        }
        numbered.sort_by(|a, b| a.0.cmp(&b.0));
        Faculties { numbered, other }
    }

    pub fn programs(&self) -> usize {
        self.numbered.iter().map(|(.., n)| n).sum::<usize>() + self.other
    }
}

/// A faculty's name in a word or two for the key of the drawing: „MINT", „Umwelt", „Maschinenbau";
/// "Human Sciences" where the first word alone says too little.
fn short_name(full: &str) -> String {
    let words: Vec<&str> = full.split([' ', ',', '-']).filter(|word| !word.is_empty()).collect();
    match words.as_slice() {
        [first, second, ..] if first.ends_with("al") || first.ends_with("an") => format!("{first} {second}"),
        [first, ..] => (*first).to_string(),
        [] => full.to_string(),
    }
}

/// Seconds since 1970, for when a module is planned; 0 outside the browser.
fn now_secs() -> u64 {
    #[cfg(feature = "csr")]
    {
        (web_sys::js_sys::Date::now() / 1000.0) as u64
    }
    #[cfg(not(feature = "csr"))]
    0
}

/// The card. `now` is the current semester; `faculties` draws step 1; `on_done` hears what
/// „Studium anlegen" did, `on_look` „Erst mal umschauen".
#[component]
pub fn SetupCard(now: Signal<Option<SemesterKey>>, faculties: Signal<Option<Counted>>, on_done: Callback<Done>, on_look: Callback<()>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.setup;
    let step = RwSignal::new(1u8);
    let program = RwSignal::new(None::<String>);
    let start = RwSignal::new(None::<SemesterKey>);
    let fs_text = RwSignal::new(String::new());
    let fs_wrong = RwSignal::new(false);
    // The rows unticked in step 3 (`SetupRow::key`), the switch of the earlier Fachsemester, the
    // plan of a program with several (an index of its plans).
    let off = RwSignal::new(BTreeSet::<String>::new());
    let past = RwSignal::new(true);
    let direction = RwSignal::new(0usize);

    let programs = use_ask(move || APP.then_some(ProgramsAsk {}));
    let items = Signal::derive(move || {
        programs.with(|answer| {
            answer.as_ref().and_then(|answer| answer.as_ref().ok()).map_or_else(Vec::new, |all: &Vec<Program>| {
                all.iter().map(|p| ComboItem::new(p.id.clone(), p.name.clone(), format!("{} · PO {}", p.degree(), po_of(p)), i64::from(p.is_latest_po))).collect()
            })
        })
    });
    // The plans of the program picked: the answer for another program (the one before, while this
    // one's is on its way) does not count.
    let asked = use_ask(move || program.get().map(|id| PlanSourceAsk { program_id: id, locale: t.locale }));
    let source = Memo::new(move |_| {
        let id = program.get()?;
        asked.with(|answer| answer.as_ref().and_then(|answer| answer.as_ref().ok()).cloned().flatten().filter(|source| source.program.id == id))
    });

    let pick = Callback::new(move |picked: Option<String>| {
        let Some(picked) = picked else { return };
        if program.get_untracked().as_ref() != Some(&picked) {
            program.set(Some(picked));
            direction.set(0);
            off.set(BTreeSet::new());
        }
        step.set(2);
    });

    // The Studienbeginn the program's plan usually means now: the current semester, or the one
    // before where the plan begins in the other half of the year.
    let usual = Memo::new(move |_| {
        let now = now.get()?;
        let intake = source.with(|source| {
            let source = source.as_ref()?;
            let (core, _) = chosen(source, direction.get());
            stored::intake_season(core?, &source.linked)
        });
        Some(stored::intake_start(now, intake))
    });
    let fs_now = Memo::new(move |_| fachsemester(now.get()?, start.get()?));
    let choose_start = move |key: SemesterKey| {
        start.set(Some(key));
        fs_wrong.set(false);
        off.set(BTreeSet::new());
        step.set(3);
    };
    let custom = move || {
        let Some(now) = now.get_untracked() else { return };
        match fs_text.get_untracked().trim().parse::<u8>().ok().filter(|fs| (1..=MOST_FS).contains(fs)).and_then(|fs| now.plus(1 - i32::from(fs))) {
            Some(key) => choose_start(key),
            None => fs_wrong.set(true),
        }
    };

    let rows = Memo::new(move |_| {
        let fs = fs_now.get()?;
        source.with(|source| {
            let source = source.as_ref()?;
            let (core, page) = chosen(source, direction.get());
            Some(rows_in(source, core, page, fs))
        })
    });
    let before = Memo::new(move |_| {
        let fs = fs_now.get()?;
        source.with(|source| {
            let source = source.as_ref()?;
            let (core, page) = chosen(source, direction.get());
            Some(rows_before(source, core, page, fs))
        })
    });

    let create = move |with_modules: bool| {
        let (Some(mine), Some(plan_store)) = (MyProgram::expect(), Studyplan::expect()) else { return };
        let (Some(source), Some(start), Some(now)) = (source.get_untracked(), start.get_untracked(), now.get_untracked()) else { return };
        let fs = fachsemester(now, start).unwrap_or(1);
        let index = direction.get_untracked();
        let (caption, page_direction) = match source.variants.len() {
            0 => (String::new(), None),
            len => ProgramPlans::new(&source.variants, source.supplements.clone()).kept(index.min(len - 1)),
        };
        let start_before = mine.start_untracked();
        mine.set_study(&source.program.id, &program_name(&source.program), &caption, page_direction.as_deref(), start);
        let off = off.get_untracked();
        let mark_past = past.get_untracked() && with_modules;
        let plan_before = plan_store.with_untracked(Clone::clone);
        let mut doc = plan_before.clone();
        let (core, page) = chosen(&source, index);
        let at = now_secs();
        let (mut modules, mut placeholders) = (0, 0);
        if let (true, Some(core)) = (with_modules, core) {
            let import = stored::import_fs(&doc, &source.program.id, core, page, now, fs);
            let import = Import {
                modules: import.modules.into_iter().filter(|(_, id)| !off.contains(&format!("m:{id}"))).collect(),
                placeholders: import.placeholders.into_iter().filter(|p| !off.contains(&format!("r:{}", p.ord))).collect(),
                ..import
            };
            (modules, placeholders) = doc.apply(&import, at);
        }
        let mut passed = None;
        if mark_past && fs > 1 {
            let leave = BTreeSet::new();
            let snapshot = doc.clone();
            let input = study::Input { program_id: &source.program.id, core, page, start, now, leave: &leave, until: None, doc: &snapshot, rows: &source.linked, tree: &source.areas, placements: &source.placements, fues: &source.fues };
            let had: BTreeSet<(SemesterKey, String)> = doc.modules.iter().map(|m| (m.semester, m.module_id.clone())).collect();
            let had_rows: BTreeSet<u32> = doc.placeholders.iter().map(|p| p.pid).collect();
            study::fill_past(&mut doc, &input, at);
            let weights: Vec<SetupRow> = rows_before(&source, core, page, fs);
            let weight_of = |key: &str| weights.iter().find(|row| row.key == key).map_or(0.0, |row| row.weight);
            let mut credits = 0.0;
            let new_modules: Vec<(SemesterKey, String)> = doc.modules.iter().filter(|m| m.semester < now && !had.contains(&(m.semester, m.module_id.clone()))).map(|m| (m.semester, m.module_id.clone())).collect();
            for (semester, id) in new_modules {
                if doc.passed_in(&id).is_none() && doc.pass(semester, &id) {
                    credits += weight_of(&format!("m:{id}"));
                }
            }
            let new_rows: Vec<DoneRow> = doc
                .placeholders
                .iter()
                .filter(|p| p.semester < now && !had_rows.contains(&p.pid))
                .map(|p| DoneRow { semester: p.semester, program_id: p.program_id.clone(), ord: p.ord, caption: p.caption.clone(), name: p.name.clone() })
                .collect();
            for row in new_rows {
                let key = format!("r:{}", row.ord);
                if doc.finish_row(row) {
                    credits += weight_of(&key);
                }
            }
            passed = (credits > 0.0).then_some(credits);
        }
        plan_store.update_after_paint(move |stored| *stored = doc);
        on_done.run(Done { program: program_name(&source.program), fs, start, modules, placeholders, passed, plan_before, start_before });
    };

    let chips = move || {
        let now_step = step.get();
        // The program as the picker lists it, at once; its plans may still be on their way.
        let program_label = program.with(|id| {
            let id = id.as_ref()?;
            programs.with(|answer| {
                let all: &Vec<Program> = answer.as_ref()?.as_ref().ok()?;
                all.iter().find(|p| &p.id == id).map(|p| format!("{} {}", p.name, p.degree()))
            })
        });
        let start_label = start.get().zip(fs_now.get()).map(|(key, fs)| format!("{} · {fs}.\u{a0}{}", key.short(t.locale), s.fs_short));
        [(1u8, s.step_program, program_label), (2, s.step_start, start_label), (3, s.step_semester, None)]
            .into_iter()
            .map(|(n, name, value)| {
                let done = now_step > n;
                let label = if done { value.unwrap_or_else(|| name.to_string()) } else { name.to_string() };
                view! {
                    <button type="button" class="ob-chip" class:on=now_step == n class:done=done disabled=!done on:click=move |_| step.set(n) aria-current=(now_step == n).then_some("step")>
                        <i aria-hidden="true">{if done { view! { <Icon name="check"/> }.into_any() } else { view! { {n} }.into_any() }}</i>
                        {done.then(|| view! { <span class="visually-hidden">{s.done}</span> })}
                        <span>{label}</span>
                    </button>
                }
            })
            .collect_view()
    };

    let foot = move || {
        view! {
            <div class="ob-foot">
                <button type="button" class="ob-link" title=s.look_around_title on:click=move |_| on_look.run(()) data-command=""><Icon name="search"/>{s.look_around}</button>
                <a class="ob-link" href=t.path(url::PROFILE) rel="nofollow" title=s.take_profile_title data-command=""><Icon name="upload"/>{s.take_profile}</a>
                <a class="ob-link ob-about" href=t.path(url::ABOUT) data-command="">{t.today.what_is_betula}<Icon name="arrow-right"/></a>
            </div>
        }
    };

    let step_one = move || {
        view! {
            <h1 class="ob-title">{s.program_title}</h1>
            <p class="ob-lede">{s.program_lead}</p>
            <div class="ob-combo">
                <Combobox
                    id="ob-program"
                    label=t.catalog.program
                    placeholder=t.home.choose_program
                    search_placeholder=t.catalog.search_program
                    icon="graduation-cap"
                    min_width=420.0
                    items
                    selected=Signal::derive(move || program.get())
                    on_select=pick
                    clearable=false
                />
            </div>
            <Drawing faculties/>
            {foot}
        }
    };

    let step_two = move || {
        let now = now.get();
        let usual = usual.get();
        let (regular, po_year) = source.with(|source| {
            source.as_ref().map_or((None, None), |source| {
                let (core, _) = chosen(source, direction.get_untracked());
                (core.and_then(|core| u8::try_from(core.semesters).ok()).filter(|n| *n > 0), source.program.po_year)
            })
        });
        let semesters = now.map(|now| (0..OFFERED).filter_map(|back| now.plus(-back)).collect::<Vec<SemesterKey>>()).unwrap_or_default();
        view! {
            <h1 class="ob-title">{s.start_title}</h1>
            <p class="ob-lede">{s.start_lead}</p>
            <div class="ob-fill" data-scroll-shade="">
                <div class="ob-sems">
                    {semesters.into_iter().map(|key| {
                        let fs = now.and_then(|now| fachsemester(now, key)).unwrap_or(1);
                        let mut notes: Vec<String> = Vec::new();
                        if regular.is_some_and(|regular| fs > regular) {
                            notes.push((s.beyond_regular)(regular.unwrap_or(fs)));
                        }
                        if po_year.is_some_and(|year| i64::from(key.year) < year) {
                            notes.push((s.before_po)(&po_year.map(|year| year.to_string()).unwrap_or_default()));
                        }
                        let what = if fs == 1 { s.starting.to_string() } else { (s.now_in)(fs) };
                        let command = format!("{} · {what}", key.label(t.locale));
                        view! {
                            <button type="button" class="ob-sem" class:usual=usual == Some(key) on:click=move |_| choose_start(key) data-command=command>
                                <b>{key.label(t.locale)}</b>
                                <span class="ob-fs num">{fs}"."<small>{s.fs_short}</small></span>
                                <small class="ob-sem-what">{what}{(!notes.is_empty()).then(|| view! { " · "<span class="ob-warn">{notes.join(", ")}</span> })}</small>
                            </button>
                        }
                    }).collect_view()}
                </div>
            </div>
            <div class="ob-custom">
                <label for="ob-fs">{s.other_fs}</label>
                <span class="ob-num">
                    <input
                        id="ob-fs"
                        data-command=s.other_fs
                        type="number"
                        inputmode="numeric"
                        min="1"
                        max=MOST_FS.to_string()
                        placeholder="7"
                        prop:value=move || fs_text.get()
                        on:input=move |ev| { fs_text.set(event_target_value(&ev)); fs_wrong.set(false); }
                        on:keydown=move |ev: leptos::ev::KeyboardEvent| if ev.key() == "Enter" { ev.prevent_default(); custom(); }
                    />
                    {s.fs_short}
                </span>
                <button type="button" class="btn secondary ob-go" on:click=move |_| custom()>{s.go_on}</button>
            </div>
            {move || fs_wrong.get().then(|| view! { <p class="ob-note warn" role="alert"><Icon name="triangle-alert"/><span>{s.fs_range}</span></p> })}
        }
    };

    let step_three = move || {
        let fs = fs_now.get().unwrap_or(1);
        let semester = now.get().map(|now| now.label(t.locale)).unwrap_or_default();
        let (plans, has_plan, last) = source.with(|source| {
            source.as_ref().map_or((Vec::new(), false, 0), |source| {
                let labels: Vec<String> = if source.variants.len() > 1 { source.variants.iter().map(|plan| plan.label.clone()).collect() } else { Vec::new() };
                let (core, _) = chosen(source, direction.get());
                (labels, core.is_some(), core.map_or(0, |core| core.semesters))
            })
        });
        let listed = rows.get().unwrap_or_default();
        // What is ticked changes the sentence, never the list (which keeps its scroll).
        let lede = {
            let listed = listed.clone();
            let semester = semester.clone();
            move || {
                let (count, credits) = off.with(|off| listed.iter().filter(|row| !off.contains(&row.key)).fold((0, 0.0), |(n, sum), row| (n + 1, sum + row.weight)));
                if listed.is_empty() { (s.semester_lead_empty)(&semester) } else { (s.semester_lead)(&semester, count, &format::number(credits, t.locale)) }
            }
        };
        let note = if !has_plan {
            Some(s.no_plan.to_string())
        } else if listed.is_empty() && i64::from(fs) > last && last > 0 {
            Some((s.beyond_plan)(u8::try_from(last).unwrap_or(fs)))
        } else {
            None
        };
        let earlier = before.get().unwrap_or_default();
        let earlier_credits: f64 = earlier.iter().map(|row| row.weight).sum();
        view! {
            <h1 class="ob-title">{(s.semester_title)(fs)}</h1>
            <p class="ob-lede">{lede}</p>
            {(!plans.is_empty()).then(|| view! {
                <div class="ob-plans" role="radiogroup" aria-label=s.direction>
                    {plans.into_iter().enumerate().map(|(index, label)| view! {
                        <button type="button" role="radio" class="ob-plan" aria-checked=move || (direction.get() == index).to_string() on:click=move |_| { direction.set(index); off.set(BTreeSet::new()); }>{label}</button>
                    }).collect_view()}
                </div>
            })}
            <div class="ob-fill" data-scroll-shade="">
                {note.map(|note| view! { <p class="ob-note info"><Icon name="info"/><span>{note}</span></p> })}
                <div class="ob-mods">
                    {listed.into_iter().map(|row| {
                        let key = row.key.clone();
                        let on = Memo::new({ let key = key.clone(); move |_| !off.with(|off| off.contains(&key)) });
                        let kind = row.kind.map_or(s.module, |kind| kind.label(t.locale));
                        let what = match (row.linked, row.span) {
                            (true, (from, to)) if from != to => format!("{kind} · {}", (s.span)(from, to)),
                            (true, _) => kind.to_string(),
                            (false, _) => format!("{kind} · {}", s.placeholder),
                        };
                        view! {
                            <button type="button" class="ob-mod" class:ph=!row.linked role="checkbox" aria-checked=move || on.get().to_string() on:click=move |_| off.update(|off| { if !off.remove(&key) { off.insert(key.clone()); } })>
                                <span class="ob-box" aria-hidden="true"><Icon name="check"/></span>
                                <span class="ob-mod-text"><b>{row.name.clone()}</b><small>{what}</small></span>
                                <span class="ob-lp num">{row.credits.clone().unwrap_or_else(|| "–".to_string())}<small>{t.today.credits}</small></span>
                            </button>
                        }
                    }).collect_view()}
                </div>
            </div>
            {(fs > 1 && has_plan).then(|| view! {
                <button type="button" class="ob-switch" role="switch" aria-checked=move || past.get().to_string() on:click=move |_| past.update(|on| *on = !*on) data-command=s.past_switch data-command-state=move || past.get().then_some(s.switch_on)>
                    <span class="ob-toggle" aria-hidden="true"></span>
                    <span><b>{s.past_switch}</b><small>{(s.past_detail)(earlier.len(), &format::number(earlier_credits, t.locale), fs - 1)}</small></span>
                </button>
            })}
            <div class="ob-actions">
                {has_plan.then(|| view! { <button type="button" class="btn ghost ob-without" on:click=move |_| create(false) data-command="">{s.without_modules}</button> })}
                <button type="button" class="btn primary ob-create" on:click=move |_| create(true) data-command=""><Icon name="check"/>{s.create}</button>
            </div>
        }
    };

    view! {
        // Its commands for „Befehle" (R25), under „Studium einrichten".
        <div class="ob" data-step=move || step.get().to_string() data-commands=t.today.set_up_study>
            <nav class="ob-chips" aria-label=s.steps_label>{chips}</nav>
            {move || match step.get() {
                1 => step_one().into_any(),
                2 => step_two().into_any(),
                _ => step_three().into_any(),
            }}
        </div>
    }
}

/// The card as the server writes it (R9): the first step, and „Studiengang wählen" a link to the
/// programs, which the app's picker takes the place of.
#[component]
pub fn ServerCard(faculties: Signal<Option<Counted>>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.setup;
    view! {
        <div class="ob" data-step="1">
            <nav class="ob-chips" aria-label=s.steps_label>
                {[(1u8, s.step_program), (2, s.step_start), (3, s.step_semester)].map(|(n, name)| view! {
                    <button type="button" class="ob-chip" class:on=n == 1 disabled=true><i aria-hidden="true">{n}</i><span>{name}</span></button>
                })}
            </nav>
            <h1 class="ob-title">{s.program_title}</h1>
            <p class="ob-lede">{s.program_lead}</p>
            <div class="ob-combo">
                <div class="combo">
                    <a class="combo-trigger" id="ob-program" href=t.path(url::PROGRAMS)>
                        <Icon name="graduation-cap"/>
                        <span class="combo-value placeholder">{t.home.choose_program}</span>
                        <Icon name="chevrons-up-down" class="combo-chevron"/>
                    </a>
                </div>
            </div>
            <Drawing faculties/>
            <div class="ob-foot">
                <a class="ob-link" href=t.path(url::CATALOG)><Icon name="search"/>{s.look_around}</a>
                <a class="ob-link" href=t.path(url::PROFILE) rel="nofollow" title=s.take_profile_title><Icon name="upload"/>{s.take_profile}</a>
                <a class="ob-link ob-about" href=t.path(url::ABOUT)>{t.today.what_is_betula}<Icon name="arrow-right"/></a>
            </div>
        </div>
    }
}

/// The drawing under the picker (owner, 2026-10-20: „Im zweifelsfall mach da halt ein paar stats
/// oder eine zeichnung rein"): every current program a dot in its faculty's colour, the faculties
/// one after the other, and the key with their numbers.
#[component]
fn Drawing(faculties: Signal<Option<Counted>>) -> impl IntoView {
    let t = i18n::t();
    move || {
        faculties.get().map(|counted| {
            let faculties = Faculties::of(&counted, t);
            let dots = faculties
                .numbered
                .iter()
                .flat_map(|(code, ..)| std::iter::repeat_n(format!("fac-{code}"), faculties.numbered.iter().find(|(c, ..)| c == code).map_or(0, |(.., n)| *n)))
                .chain(std::iter::repeat_n("fac-0".to_string(), faculties.other))
                .collect::<Vec<String>>();
            view! {
                <div class="ob-art" aria-hidden="true">
                    <p>{(t.setup.drawing)(faculties.programs(), faculties.numbered.len())}</p>
                    <div class="ob-dots">{dots.into_iter().map(|class| view! { <i class=class></i> }).collect_view()}</div>
                    <p class="ob-key">
                        {faculties.numbered.iter().map(|(code, short, full, n)| view! {
                            <span title=full.clone()><i class=format!("fac-{code}")></i>{short.clone()}" "<b class="num">{*n}</b></span>
                        }).collect_view()}
                        {(faculties.other > 0).then(|| view! { <span><i class="fac-0"></i>{t.setup.across_faculties}" "<b class="num">{faculties.other}</b></span> })}
                    </p>
                </div>
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_faculty_is_named_in_a_word_or_two() {
        assert_eq!(short_name("MINT - Mathematik, Informatik, Physik, Elektro- und Informationstechnik"), "MINT");
        assert_eq!(short_name("Umwelt und Naturwissenschaften"), "Umwelt");
        assert_eq!(short_name("Maschinenbau, Elektro- und Energiesysteme"), "Maschinenbau");
        assert_eq!(short_name("Mechanical Engineering, Electrical and Energy Systems"), "Mechanical Engineering");
        assert_eq!(short_name("Human Sciences"), "Human Sciences");
        assert_eq!(short_name("Business, Law and Social Sciences"), "Business");
    }

    /// The snapshot the tests read: `FOLIA_TEST_SNAPSHOT`, else the one `snapshot/current.json`
    /// names.
    fn plans_of(id: &str) -> PlanSource {
        use std::sync::Mutex;

        use folia_data::{CatalogSource, DataClient};
        use folia_model::native::NativeDatabase;
        use folia_model::{Database, DbError};

        struct Snapshot(Mutex<NativeDatabase>);
        impl CatalogSource for Snapshot {
            fn with_db(&self, job: &mut dyn FnMut(&dyn Database)) -> Result<(), DbError> {
                let db = self.0.lock().map_err(|_| DbError::Unavailable("poisoned".to_string()))?;
                job(&*db);
                Ok(())
            }
        }
        let path = std::env::var("FOLIA_TEST_SNAPSHOT").map(std::path::PathBuf::from).unwrap_or_else(|_| {
            let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../snapshot");
            let pointer = std::fs::read_to_string(dir.join("current.json")).unwrap();
            dir.join(pointer.split("\"file\"").nth(1).and_then(|rest| rest.split('"').nth(1)).unwrap())
        });
        let source = DataClient::new(folia_data::Source(std::sync::Arc::new(Snapshot(Mutex::new(NativeDatabase::open(&path).unwrap())))));
        source.now(&PlanSourceAsk { program_id: id.to_string(), locale: folia_locale::Locale::De }).unwrap().unwrap()
    }

    /// Informatik B.Sc. in its first and third Fachsemester: what step 3 lists is what „Fachsemester
    /// übernehmen" takes, and what the switch counts is what lies before.
    #[test]
    fn the_rows_of_a_fachsemester_are_the_imports() {
        let source = plans_of("079-82-2008");
        let (core, page) = chosen(&source, 0);
        let first = rows_in(&source, core, page, 1);
        let names: Vec<&str> = first.iter().map(|row| row.name.as_str()).collect();
        assert!(names.contains(&"Entwicklung von Softwaresystemen") && names.contains(&"Mathematik IT-1 (Diskrete Mathematik)"), "{names:?}");
        let key = SemesterKey::parse("2026W").unwrap();
        let import = stored::import_fs(&PlanDoc::default(), &source.program.id, core.unwrap(), page, key, 1);
        assert_eq!(first.iter().filter(|row| row.linked).count(), import.modules.len());
        assert_eq!(first.iter().filter(|row| !row.linked).count(), import.placeholders.len());
        let earlier = rows_before(&source, core, page, 3);
        assert!(earlier.len() > first.len() && earlier.iter().all(|row| row.span.0 < 3));
        assert!(rows_before(&source, core, page, 1).is_empty());
    }
}
