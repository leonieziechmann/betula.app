//! „Einplanen" where no study is set up (owner, 2026-10-18, of concept E: „Aber das Studium
//! einrichten muss definitiv ausführlicher sein. Also so, dass man ein modal bekommt und dann da zwar
//! schon die defaults drin sind. Aber man trotzdem alles änderen kann. Aber das einplanen von Modul
//! soll dann auch mit in dem modal sein, so dass man im best case nur auf ok drücken muss"): one
//! dialog with „Dein Studium" as the card of „Mein Studium" has it — the program and the
//! Fachsemester, a sentence over them — and the semester the module goes into under it.
//! „Einrichten und einplanen" stores the study in „Mein Studiengang" and plans the module; „Ohne
//! Studium einplanen" plans it alone, and the session asks no more (`Planning::plan_from_setup`).
//!
//! The program comes from the catalog's filter, else from the Stundenplan's of the session, else
//! from „Mein Studiengang". Without any, none is assumed (the same day: „Weil der assumed
//! Studiengang wahrscheinlich falsch ist. Deswegen möchte ich das gleich expandiert haben, wo man
//! einen Studiengang auswählen kann aus maximal 3, und darunter gleich eine combobox"): the module's
//! programs stand open, three at most, compulsory first, the newest regulation of each, and the
//! search for any other under them, the app's picker as it is, as wide as the programs (the same
//! day: „Erstmal ist es zu weit links, dann wird es breiter wenn man rein klickt und die combox ist
//! nicht so styled, wie alle anderen in der App"). A program known is a row as in „Mein Studium",
//! its picker the way to another with the module's programs first (the same day: „was ist das für
//! ein clusterfuck, wenn man schon ein Studiengangs ausgewählt hat und den doch nochmal ändern
//! möchte"). Once a program is known, its Regelstudienplan fills in what the student has not set
//! (`folia_stores::studyplan::fit`); the filter's semester is read by the time. A semester behind
//! the student is allowed, with a warning (the same day: „Betula sollte bei sowas nicht nein
//! sagen"). Where „Weitere Module auch nach …" aimed the click, the module's row says so, with
//! „Aufheben".
//!
//! A native `<dialog>` shown modally, in the frame of the app's dialogs (app.css „The dialogs"): in
//! the middle of the window on a desktop, a sheet from below on a phone.

use folia_calendar::semester::SemesterKey;
use folia_model::labels::{DegreeLevel, Labelled, ModuleKind, Relation, ResolveStatus};
use folia_model::rows::Program;
use folia_pages::ask::{unless_pending, ModuleAsk, PlanSourceAsk, ProgramsAsk};
use folia_pages::{ModuleData, PlanSource};
use folia_plans::study;
use folia_plans::studyplan::intake_season;
use folia_routes::filter::PlanSemesterFilter;
use folia_routes::url::{self, CatalogUrl};
use leptos::prelude::*;

use folia_data::use_data;
use folia_design::combobox::{with_first, ComboItem, Combobox};
use folia_design::ui::{DialogClose, Icon};
use folia_stores::myprogram::{po_of, program_name, MyProgram};
use folia_stores::studyplan::fit::{self, Filled, FsFrom, Known};
use folia_stores::studyplan::{target_semester, Planning, SetupAsk, Studyplan};

use crate::i18n::{self, Texts};

/// How many of the module's programs stand open.
const OFFERED: usize = 3;
/// How far › goes after the current semester.
const AHEAD: i32 = 12;
/// The last Fachsemester + counts to.
const MOST_FS: u8 = 30;

/// The dialog, once in the app (`folia_app::App`): shown while „Einplanen" asks for the study.
#[component]
pub fn PlanSetup() -> impl IntoView {
    let Some(planning) = Planning::expect() else { return ().into_any() };
    let node = NodeRef::<leptos::html::Dialog>::new();
    let ask = Memo::new(move |_| planning.setup());
    let open = Memo::new(move |_| ask.with(Option::is_some));
    Effect::new(move |_| {
        let open = open.get();
        let Some(element) = node.get() else { return };
        if open && !element.open() {
            let _ = element.show_modal();
        } else if !open && element.open() {
            element.close();
        }
    });
    view! {
        <dialog
            class="st-dialog plan-setup"
            node_ref=node
            data-sheet=""
            aria-labelledby="plan-setup-title"
            on:close=move |_| planning.close_setup()
            // A click on the dim backdrop is a click on the dialog itself.
            on:click=move |ev: leptos::ev::MouseEvent| {
                let on_backdrop = ev.target().and_then(|target| node.get_untracked().map(|dialog| {
                    let dialog: &leptos::web_sys::EventTarget = dialog.as_ref();
                    dialog == &target
                }));
                if on_backdrop == Some(true) {
                    planning.close_setup();
                }
            }
        >
            {move || ask.get().map(|ask| view! { <div class="st-dlg"><SetupDialog ask planning/></div> })}
        </dialog>
    }
    .into_any()
}

/// Where the program the dialog starts with comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum From {
    Filter,
    Timetable,
    Mine,
    Picked,
}

/// One of the module's programs, as the list offers it.
#[derive(Clone, Debug, PartialEq)]
struct Offered {
    id: String,
    name: String,
    kind: Option<ModuleKind>,
    at: Option<i64>,
}

#[component]
fn SetupDialog(ask: SetupAsk, planning: Planning) -> impl IntoView {
    let t = i18n::t();
    let p = &t.planner;
    let source = use_data().ok();
    let plan = Studyplan::expect();
    let mine = MyProgram::expect();
    let location = i18n::use_location();
    let what = ask.what.clone();
    let now = what.current;

    // All programs: to search, and to know one by its id or its slug.
    let programs = {
        let source = source.clone();
        Memo::new(move |before: Option<&Vec<Program>>| match source.as_ref().map(|source| source.now(&ProgramsAsk {})) {
            Some(now) => unless_pending(now, before, Result::unwrap_or_default),
            None => Vec::new(),
        })
    };
    // The module: its programs, and where their plans place it.
    let module = {
        let (source, id) = (source.clone(), what.id.clone());
        Memo::new(move |before: Option<&Option<ModuleData>>| match source.as_ref().map(|source| source.now(&ModuleAsk { id: id.clone() })) {
            Some(now) => unless_pending(now, before, |now| now.ok().flatten()),
            None => None,
        })
    };
    // The catalog's filter, where the dialog was opened over the catalog: its program and semester.
    let filter = untrack(|| {
        (location.pathname.get() == url::CATALOG).then(|| CatalogUrl::parse(&location.search.get())).and_then(|url| url.query.program).map(|scope| (scope.program_slug, scope.plan_semester))
    });
    let filter_slug = filter.as_ref().map(|(slug, _)| slug.clone());
    let filter_semester = filter.as_ref().and_then(|(_, semester)| match semester {
        Some(PlanSemesterFilter::Semester(n)) => Some(*n),
        _ => None,
    });
    // The program until one is picked: the filter's, the Stundenplan's of the session, „Mein
    // Studiengang".
    let found = Memo::new(move |_| {
        let by_slug = filter_slug.as_ref().and_then(|slug| programs.with(|all| all.iter().find(|program| program.slug == *slug).map(|program| program.id.clone())));
        if let Some(id) = by_slug {
            return Some((id, From::Filter));
        }
        if let Some(id) = plan.and_then(|plan| plan.with_untracked(|doc| doc.program.clone())) {
            return Some((id, From::Timetable));
        }
        mine.and_then(|mine| untrack(|| mine.with(|doc| doc.program.clone()))).map(|id| (id, From::Mine))
    });
    let picked = RwSignal::new(None::<String>);
    let program_id = Memo::new(move |_| picked.get().or_else(|| found.with(|found| found.as_ref().map(|(id, _)| id.clone()))));
    let from = Memo::new(move |_| match (picked.with(Option::is_some), found.with(|found| found.as_ref().map(|(_, from)| *from))) {
        (false, Some(from)) => from,
        _ => From::Picked,
    });
    let program = Memo::new(move |_| program_id.with(|id| id.as_ref().and_then(|id| programs.with(|all| all.iter().find(|program| program.id == *id).cloned()))));
    // Its plans, for the season a Bachelor's Fachsemester fall in.
    let plans = {
        let source = source.clone();
        Memo::new(move |before: Option<&Option<PlanSource>>| {
            let id = program_id.get()?;
            let now = source.as_ref()?.now(&PlanSourceAsk { program_id: id, locale: t.locale });
            unless_pending(now, before, |now| now.ok().flatten())
        })
    };
    let intake = Memo::new(move |_| {
        let bachelor = program.with(|program| program.as_ref().is_some_and(|program| matches!(program.degree_level.known(), Some(DegreeLevel::Bachelor | DegreeLevel::TeachingBachelor))));
        let stated = plans.with(|plans| plans.as_ref().and_then(|plans| plans.variants.first().and_then(|core| intake_season(core, &plans.linked))));
        fit::intake_of(bachelor, stated)
    });
    // Where the program's Regelstudienplan places the module, and as what.
    let place = Memo::new(move |_| program_id.with(|id| module.with(|module| module.as_ref().zip(id.as_ref()).and_then(|(module, id)| place_of(module, id)))));
    let offered = Memo::new(move |_| module.with(|module| module.as_ref().map(offers_of).unwrap_or_default()));
    // The filter's Fachsemester, read by the time and the season.
    let filter_fs = Memo::new(move |_| filter_semester.map(|n| fit::filter_fs(n, now, planning.session(now), intake.get())));
    // What the student set, and what is filled in from it.
    let fs_set = RwSignal::new(None::<u8>);
    // A page that means a semester (the finder's, a placeholder's) has set the module's.
    let means = what.hint.as_ref().is_some_and(|hint| hint.semester.is_some() || hint.fill.is_some());
    let semester_set = RwSignal::new(means.then_some(ask.target));
    // Where the click aimed; „Weitere Module auch nach …" aimed it (the semester chosen for the next
    // modules, P3): the module's row says so and offers to clear it, which aims as without it —
    // the session's semester where the module is offered then, as `target_semester` (owner,
    // 2026-10-18, a dialog that stood at „SoSe 2034 · 16. FS" without a word why).
    let aim = RwSignal::new(ask.target);
    let chosen_aim = RwSignal::new(!means && untrack(|| planning.chosen()) == Some(ask.target));
    let unchosen = chosen_aim.get_untracked().then(|| plan.map(|plan| plan.with_untracked(|doc| target_semester(planning.session(now), what.newest, what.offer, None, doc)))).flatten();
    let clear = unchosen.map(|unchosen| {
        Callback::new(move |()| {
            planning.choose(None);
            chosen_aim.set(false);
            aim.set(unchosen);
        })
    });
    let filled = {
        let offer = what.offer;
        Memo::new(move |_| {
            fit::fill(&Known {
                now,
                aim: aim.get(),
                offer,
                program: program_id.with(Option::is_some),
                place: place.get().and_then(|(n, _)| n),
                intake: intake.get(),
                filter_fs: filter_fs.get().map(|(fs, _)| fs),
                fs: fs_set.get(),
                semester: semester_set.get(),
            })
        })
    };
    let leave = Memo::new(move |_| mine.map(MyProgram::leave).unwrap_or_default());
    // The Studienbeginn the Fachsemester means, and the earliest semester ‹ reaches.
    let start = Memo::new(move |_| filled.get().fs.and_then(|fs| study::start_of_fs(now, fs, &leave.get())));
    let floor = Memo::new(move |_| start.get().filter(|start| *start <= now).unwrap_or_else(|| now.plus(-6).unwrap_or(now)));
    let pick = Callback::new(move |id: Option<String>| {
        if let Some(id) = id {
            picked.set(Some(id));
        }
    });
    let step_fs = move |by: i32| {
        let next = match filled.get_untracked().fs {
            Some(fs) => (i32::from(fs) + by).clamp(1, i32::from(MOST_FS)),
            None => 1,
        };
        let next = u8::try_from(next).unwrap_or(1);
        fs_set.set(Some(next));
        // A semester chosen before the new first Fachsemester comes along to it.
        if let (Some(at), Some(start)) = (semester_set.get_untracked(), study::start_of_fs(now, next, &leave.get_untracked())) {
            if at < start {
                semester_set.set(Some(start));
            }
        }
    };
    let step_semester = move |by: i32| {
        let at = filled.get_untracked().semester;
        let last = now.plus(AHEAD).unwrap_or(now);
        if let Some(next) = at.plus(by).filter(|next| *next >= floor.get_untracked() && *next <= last) {
            semester_set.set(Some(next));
        }
    };
    let close = move || planning.close_setup();
    let set_up = {
        let (ask, source) = (ask.clone(), source.clone());
        move |_| {
            let (Some(program), Some(start), Some(plan)) = (program.get_untracked(), start.get_untracked(), plan) else { return };
            if let Some(mine) = mine {
                // The program „Mein Studiengang" has already keeps its plan and direction.
                if untrack(|| mine.with(|doc| doc.program.as_deref() == Some(program.id.as_str()))) {
                    mine.set_start(Some(start));
                } else {
                    mine.set_study(&program.id, &program_name(&program), "", None, start);
                }
            }
            planning.plan_from_setup(plan, source.clone(), &ask, filled.get_untracked().semester, false);
        }
    };
    let without = {
        let ask = ask.clone();
        move |_| {
            if let Some(plan) = plan {
                planning.plan_from_setup(plan, source.clone(), &ask, filled.get_untracked().semester, true);
            }
        }
    };
    let ready = Memo::new(move |_| program.with(Option::is_some) && start.with(Option::is_some));

    // The sentence over the rows.
    let say = move || match program.get() {
        Some(program) => {
            let fs = filled.get().fs.unwrap_or(1);
            view! { {p.say_before}<em>{format!("{} {}", program.name, program.degree())}</em>{p.say_middle}<em>{(p.say_fs)(fs)}</em>{p.say_end} }.into_any()
        }
        None => view! { {p.ask_program} }.into_any(),
    };
    // The program: while none is known the module's programs stand open with the search under them
    // (`ProgramList`); once one is, it is a row as in „Mein Studium", its picker the way to another,
    // the module's programs first in it.
    let title = what.title.clone();
    let known = Memo::new(move |_| program.with(Option::is_some));
    let items = Signal::derive(move || {
        let first = mine.and_then(|mine| mine.with(|doc| doc.program.clone()));
        let all = programs.with(|all| program_items(all, first.as_deref(), t));
        offered.with(|offered| offered.iter().map(|offered| ComboItem::new(offered.id.clone(), offered.name.clone(), detail_of(offered.kind, offered.at, t), 0).in_group(p.with_module)).chain(all).collect::<Vec<_>>())
    });
    let program_row = move || {
        if !known.get() {
            return view! { <ProgramList what=title.clone() offered pick programs mine/> }.into_any();
        }
        view! {
            <div class="st-srow plan-setup-program">
                <span class="label">{p.program_row}</span>
                <Combobox
                    id="plan-setup-program"
                    label=p.program_row
                    placeholder=p.search_program
                    search_placeholder=p.search_placeholder
                    min_width=360.0
                    items
                    selected=Signal::derive(move || program_id.get())
                    on_select=pick
                    clearable=false
                />
                <p class="st-srow-note"><span>{move || program_note(from.get(), place.get(), t)}</span></p>
            </div>
        }
        .into_any()
    };
    let fs_value = move || filled.get().fs.map_or_else(|| "–".to_string(), |fs| (p.fs_long)(fs));
    let fs_note = move || fs_note_of(filled.get(), start.get(), filter_fs.get().map(|(_, at)| at), filter_semester, t);
    // The module's row.
    let module_value = move || {
        let filled = filled.get();
        let at = filled.semester;
        let fs = start.get().map(|start| match study::fs_of(start, at, &leave.get()) {
            Some(fs) => (p.fs_short)(fs),
            None => p.before_start.to_string(),
        });
        match fs {
            Some(fs) => format!("{} · {fs}", at.label(t.locale)),
            None => at.label(t.locale),
        }
    };
    let module_note = {
        let offer = what.offer;
        move || module_note_of(filled.get(), place.get().and_then(|(n, _)| n), now, chosen_aim.get().then(|| aim.get()), clear, offer, t)
    };
    let warning = move || warning_of(filled.get(), place.get().and_then(|(n, _)| n), now, semester_set.with(Option::is_some), t);

    view! {
        <header class="st-dlg-head">
            <span class="st-grip" aria-hidden="true"></span>
            <div>
                <h2 id="plan-setup-title">{(p.setup_title)(&what.title)}</h2>
                <p>{move || if program_id.with(Option::is_some) && from.get() != From::Picked { p.setup_lead_filled } else { p.setup_lead }}</p>
            </div>
            <DialogClose on_close=Callback::new(move |()| close())/>
        </header>
        <div class="st-dlg-body plan-setup-body">
            <section class="st-study" aria-label=p.program_row>
                <p class="st-study-say" aria-live="polite">{say}</p>
                {program_row}
                <div class="st-srow">
                    <span class="label">{p.fs_row}</span>
                    <div class="st-fs-line">
                        <span class="st-fs-value" class:unset=move || filled.get().fs.is_none()>{fs_value}</span>
                        <span class="st-stepper" role="group" aria-label=p.fs_row>
                            <button type="button" aria-label=p.fs_less title=p.fs_less disabled=move || filled.get().fs.is_none_or(|fs| fs <= 1) on:click=move |_| step_fs(-1)>"−"</button>
                            <button type="button" aria-label=p.fs_more title=p.fs_more disabled=move || filled.get().fs.is_some_and(|fs| fs >= MOST_FS) on:click=move |_| step_fs(1)>"+"</button>
                        </span>
                    </div>
                    <p class="st-srow-note">{fs_note}</p>
                </div>
            </section>
            <section class="st-study plan-setup-module" aria-label=(p.semester_of)(&what.title)>
                <div class="st-srow">
                    <span class="label">{p.module_row}</span>
                    <div class="st-fs-line">
                        <span class="st-fs-value">{module_value}</span>
                        <span class="st-stepper" role="group" aria-label=(p.semester_of)(&what.title)>
                            <button type="button" aria-label=p.earlier title=p.earlier disabled=move || filled.get().semester <= floor.get() on:click=move |_| step_semester(-1)>"‹"</button>
                            <button type="button" aria-label=p.later title=p.later on:click=move |_| step_semester(1)>"›"</button>
                        </span>
                    </div>
                    <p class="st-srow-note">{module_note}</p>
                </div>
                {warning}
            </section>
        </div>
        <footer class="st-dlg-foot plan-setup-foot">
            <button class="plan-setup-without" type="button" on:click=without>{p.without_study}</button>
            <button class="btn secondary plan-setup-cancel" type="button" on:click=move |_| close()>{p.cancel}</button>
            <button class="btn primary" type="button" id="plan-setup-go" disabled=move || !ready.get() title=move || (!ready.get()).then_some(p.choose_first) on:click=set_up>
                {p.set_up_and_plan}
            </button>
        </footer>
    }
}

/// Every program for the search, „Mein Studiengang" once more at their head (`with_first`).
fn program_items(all: &[Program], first: Option<&str>, t: &'static Texts) -> Vec<ComboItem> {
    let items: Vec<ComboItem> = all.iter().map(|program| ComboItem::new(program.id.clone(), program.name.clone(), format!("{} · PO {}", program.degree(), po_of(program)), i64::from(program.is_latest_po))).collect();
    with_first(&items, first, t.planner.my_program)
}

/// While no program is known: the module's programs (three at most, compulsory first, `offers_of`)
/// as buttons, and the search for any other under them — the app's picker as it is, as wide as the
/// buttons (app.css).
#[component]
fn ProgramList(what: String, offered: Memo<Vec<Offered>>, pick: Callback<Option<String>>, programs: Memo<Vec<Program>>, mine: Option<MyProgram>) -> impl IntoView {
    let t = i18n::t();
    let p = &t.planner;
    let items = Signal::derive(move || {
        let first = mine.and_then(|mine| mine.with(|doc| doc.program.clone()));
        programs.with(|all| program_items(all, first.as_deref(), t))
    });
    let buttons = move || {
        offered.get().into_iter().map(|offered| {
            let id = offered.id.clone();
            let detail = detail_of(offered.kind, offered.at, t);
            view! {
                <button type="button" role="radio" aria-checked="false" on:click=move |_| pick.run(Some(id.clone()))>
                    <span>{offered.name}</span>
                    <small class:req=matches!(offered.kind, Some(ModuleKind::Compulsory))>{detail}</small>
                </button>
            }
        }).collect_view()
    };
    view! {
        <div class="plan-setup-list">
            <div class="plan-setup-offers" role="radiogroup" aria-label=(p.programs_of)(&what)>{buttons}</div>
            <Combobox
                id="plan-setup-search"
                label=p.program_row
                placeholder=p.search_program
                search_placeholder=p.search_placeholder
                min_width=360.0
                items
                selected=Signal::derive(|| None::<String>)
                on_select=pick
                icon="search"
                clearable=false
            />
        </div>
    }
}

/// Where the program's Regelstudienplan places the module (its first semester there) and as what.
fn place_of(module: &ModuleData, program_id: &str) -> Option<(Option<u8>, Option<ModuleKind>)> {
    let at = module.plan_semesters(program_id).first().and_then(|(from, _)| u8::try_from(*from).ok());
    let kinds: Vec<ModuleKind> = curricula(module).filter(|link| link.program_id.as_deref() == Some(program_id)).filter_map(|link| link.kind.as_ref().and_then(|kind| kind.known())).collect();
    let kind = kinds.iter().copied().min_by_key(|kind| rank(Some(*kind)));
    (at.is_some() || kind.is_some()).then_some((at, kind))
}

/// The links of the programs whose curricula the module belongs to.
fn curricula(module: &ModuleData) -> impl Iterator<Item = &folia_model::rows_detail::ProgramLink> {
    module.programs.iter().filter(|link| link.resolve_status.is(ResolveStatus::Resolved) && link.relation.as_ref().is_some_and(|relation| relation.is(Relation::Curricular)) && link.program_id.is_some())
}

/// Compulsory first, then compulsory elective, then the rest.
fn rank(kind: Option<ModuleKind>) -> u8 {
    match kind {
        Some(ModuleKind::Compulsory) => 0,
        Some(ModuleKind::Elective) => 1,
        _ => 2,
    }
}

/// The module's programs the list offers (owner, 2026-10-18: „zeige da maximal 3, und bevorzugt die
/// wo das ein Pflichtmodul ist"): each program once, by its newest regulation; compulsory before
/// elective before the rest, of each those whose Regelstudienplan places the module first (their
/// Fachsemester can be filled in), in the order the catalog has them; three at most.
fn offers_of(module: &ModuleData) -> Vec<Offered> {
    let mut families: Vec<(String, &folia_model::rows_detail::ProgramLink)> = Vec::new();
    for link in curricula(module) {
        let family = format!("{} {}", link.program_name.as_deref().unwrap_or_default(), link.degree_display.as_deref().or(link.degree_raw.as_deref()).unwrap_or_default());
        match families.iter_mut().find(|(known, _)| *known == family) {
            Some((_, kept)) => {
                if link.is_latest_po == Some(true) && kept.is_latest_po != Some(true) {
                    *kept = link;
                }
            }
            None => families.push((family, link)),
        }
    }
    let mut offered: Vec<Offered> = families
        .into_iter()
        .filter_map(|(name, link)| {
            let id = link.program_id.clone()?;
            let at = module.plan_semesters(&id).first().map(|(from, _)| *from);
            Some(Offered { at, kind: link.kind.as_ref().and_then(|kind| kind.known()), name, id })
        })
        .collect();
    offered.sort_by_key(|offered| (rank(offered.kind), offered.at.is_none()));
    offered.truncate(OFFERED);
    offered
}

/// „Pflicht · 3. Sem.": what the module is in a program, and where its plan places it.
fn detail_of(kind: Option<ModuleKind>, at: Option<i64>, t: &'static Texts) -> String {
    let kind = kind.map(|kind| kind.label(t.locale).to_string());
    let at = at.map(|at| format!("{} {}", (t.format.semester_one)(at), t.planner.semester_short));
    [kind, at].into_iter().flatten().collect::<Vec<_>>().join(" · ")
}

/// Under the program: where it comes from, and where its plan places the module.
fn program_note(from: From, place: Option<(Option<u8>, Option<ModuleKind>)>, t: &'static Texts) -> String {
    let p = &t.planner;
    match from {
        From::Filter => p.from_filter.to_string(),
        From::Timetable => p.from_timetable.to_string(),
        From::Mine => p.from_mine.to_string(),
        From::Picked => match place {
            Some((Some(at), kind)) => {
                let at = i64::from(at);
                match kind {
                    Some(ModuleKind::Compulsory) => (p.compulsory_at)(at),
                    Some(ModuleKind::Elective) => (p.elective_at)(at),
                    _ => (p.other_at)(at),
                }
            }
            _ => p.not_in_plan.to_string(),
        },
    }
}

/// Under the Fachsemester: since when, and where it comes from.
fn fs_note_of(filled: Filled, start: Option<SemesterKey>, filter_at: Option<SemesterKey>, filter_semester: Option<u8>, t: &'static Texts) -> AnyView {
    let p = &t.planner;
    let since = start.map(|start| (p.since)(&start.label(t.locale)));
    let from = match filled.from {
        FsFrom::Student => None,
        FsFrom::Filter => filter_semester.zip(filter_at).map(|(n, at)| (p.fs_filter)(n, &at.short(t.locale))),
        FsFrom::Plan => Some(p.fs_fit.to_string()),
        FsFrom::Assumed => Some(p.fs_assumed.to_string()),
        FsFrom::Unknown => return view! { <span>{p.fs_follows}</span> }.into_any(),
    };
    view! { <span>{since}{from.map(|from| view! { " · "<b>{from}</b> })}</span> }.into_any()
}

/// Under the module's semester: behind the student, not offered, as the plan places it, where the
/// plan places it; the semester „Weitere Module auch nach …" chose (`chosen`, while the module stays
/// there), with „Aufheben" (`clear`); else this semester.
fn module_note_of(filled: Filled, place: Option<u8>, now: SemesterKey, chosen: Option<SemesterKey>, clear: Option<Callback<()>>, offer: folia_plans::study::Offer, t: &'static Texts) -> AnyView {
    let p = &t.planner;
    let at = filled.semester;
    let fs_there = filled.fs.map(|fs| i32::from(fs) + i32::from(at.index()) - i32::from(now.index()));
    let (text, class) = if at < now {
        (p.in_the_past.to_string(), "warn")
    } else if !offer.offered(at) {
        (p.not_offered_there.to_string(), "warn")
    } else if let Some(n) = place.filter(|n| fs_there == Some(i32::from(*n))) {
        ((p.as_in_plan)(i64::from(n)), "good")
    } else if let Some(n) = place.filter(|_| filled.fs.is_some()) {
        ((p.in_plan_at)(i64::from(n)), "")
    } else if chosen == Some(at) {
        let clear = clear.map(|clear| view! { <button class="st-back" type="button" on:click=move |_| clear.run(())>{p.chosen_clear}</button> });
        return view! { <span>{p.chosen_note}</span>{clear} }.into_any();
    } else if at == now {
        (p.this_semester.to_string(), "")
    } else {
        (String::new(), "")
    };
    view! { <span class=class>{text}</span> }.into_any()
}

/// The warning under the module: a semester behind the student, or a place in the plan behind them
/// that the module was moved on from (unless the student chose its semester).
fn warning_of(filled: Filled, place: Option<u8>, now: SemesterKey, chosen: bool, t: &'static Texts) -> Option<AnyView> {
    let p = &t.planner;
    let at = filled.semester;
    let (head, rest) = if at < now {
        ((p.past_head)(&at.label(t.locale)), p.past_rest.to_string())
    } else {
        let (n, fs) = place.zip(filled.fs).filter(|_| !chosen)?;
        let there = fit::place_semester(now, fs, n).filter(|there| *there < now)?;
        ((p.behind_head)(n, &there.label(t.locale)), (p.behind_rest)(&at.label(t.locale)))
    };
    Some(view! { <p class="plan-setup-warn" role="note"><Icon name="triangle-alert"/><span><b>{head}</b>" "{rest}</span></p> }.into_any())
}
