//! The page of „Mein Studium": the head (the program, the Fachsemester, the progress), the
//! semesters before the current one folded (to tick off what was passed), and the semesters from
//! the current one on, each a list in the order the study takes it (`folia_plans::study`): what is
//! left over first, then what is due, then the student's own modules.
//!
//! A row is a module or a row of the plan without one: the box ticks it off („bestanden",
//! „erledigt"), its name opens the module beside the page, its marks say why it stands where it
//! stands, and ↑ ↓ move it to the semester before or after that offers it. A row ticked off keeps
//! its place (owner, 2026-09-20 on the Merkliste: nothing jumps under the pointer); what the tick
//! takes out of later semesters goes there. The current semester takes what it holds into the
//! timetable („In den Stundenplan"), which „Rückgängig" takes back.

use folia_calendar::semester::{fachsemester, of_fachsemester, SemesterKey};
use folia_model::labels::Season;
use folia_plans::plan;
use folia_plans::study::{self, Item, Place, Subject};
use folia_plans::variants;
use folia_routes::url::{self, CatalogUrl, LocalView};
use leptos::prelude::*;

use folia_design::combobox::Combobox;
use folia_design::format;
use folia_design::nav;
use folia_design::ui::Icon;
use folia_stores::myprogram::program_name;

use super::side::{keep_program, program_items};
use super::{now_secs, Plans, Ready, SheetToggle, State, StudyCtx};
use crate::i18n::{self, Texts};

/// Which of the page's states is shown; the parts read the rest themselves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Waiting,
    Welcome,
    Ready,
}

#[component]
pub(super) fn StudyMain(ctx: StudyCtx) -> impl IntoView {
    let kind = Memo::new(move |_| {
        ctx.state.with(|state| match state {
            State::Waiting => Kind::Waiting,
            State::NoProgram | State::Gone(_) => Kind::Welcome,
            State::Ready(_) => Kind::Ready,
        })
    });
    move || match kind.get() {
        Kind::Waiting => view! { <Waiting/> }.into_any(),
        Kind::Welcome => view! { <Welcome ctx/> }.into_any(),
        Kind::Ready => view! {
            <Head ctx/>
            <Past ctx/>
            <Terms ctx/>
        }
        .into_any(),
    }
}

/// While the answers are on their way: the head and a semester as bars, as the page's skeleton has
/// them.
#[component]
fn Waiting() -> impl IntoView {
    view! {
        <div class="panel sk-sweep sk-head" aria-hidden="true">
            <i class="sk sk-w2"></i>
            <div class="sk-title-row"><i class="sk sk-w4 sk-big"></i><i class="sk sk-w3"></i></div>
            <i class="sk sk-w6"></i>
        </div>
    }
}

/// Without a program (or with one the catalog lost): what the page is for, and the picker.
#[component]
fn Welcome(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let gone = Memo::new(move |_| {
        ctx.state.with(|state| match state {
            State::Gone(name) => Some(name.clone()),
            _ => None,
        })
    });
    let (all, items) = program_items(ctx);
    view! {
        <section class="panel st-welcome">
            <div class="st-welcome-text">
                <h1 class="st-title">{s.welcome_title}</h1>
                <p>{s.welcome_lead}</p>
                {move || gone.get().map(|name| view! { <p class="st-gone"><Icon name="triangle-alert"/><span>{(s.gone)(&name)}</span></p> })}
            </div>
            <div class="st-welcome-pick">
                <div class="pick-combo">
                    <Combobox
                        id="st-welcome-program"
                        label=s.program
                        placeholder=s.choose_program
                        search_placeholder=s.search_program
                        icon="graduation-cap"
                        min_width=420.0
                        items
                        selected=Signal::derive(|| None::<String>)
                        on_select=keep_program(ctx, all)
                        clearable=false
                    />
                </div>
                <a class="btn secondary" href=t.path(url::PROGRAMS)><Icon name="graduation-cap"/>{s.all_programs}</a>
            </div>
        </section>
    }
}

/// „3. FS" of a semester of the study.
fn fs_label(semester: SemesterKey, start: SemesterKey, t: &Texts) -> Option<String> {
    fachsemester(semester, start).map(t.study.fs)
}

/// What the head says.
#[derive(Clone, Debug, PartialEq)]
struct HeadInfo {
    name: String,
    when: String,
    direction: Option<String>,
    progress: String,
    /// What is passed of the plan, 0 to 1; `None` without a plan to measure against.
    share: Option<f64>,
    done: f64,
    total: f64,
    ends: String,
    no_plan: bool,
}

impl HeadInfo {
    fn of(ready: &Ready, t: &Texts) -> Self {
        let s = &t.study;
        let (setup, study) = (ready.setup, &ready.study);
        let when = match fachsemester(setup.now, setup.start) {
            Some(fs) if setup.now >= setup.start => (s.fachsemester_now)(fs, &setup.now.label(t.locale)),
            _ => (s.starts_in)(&setup.start.label(t.locale)),
        };
        let direction = setup.shown.and_then(|shown| ready.plans.iter().find(|(index, _)| *index == shown)).map(|(_, label)| label.clone());
        let total = study.total.0;
        let done = format::number(study.done_credits, t.locale);
        let (progress, share) = match total > 0.0 {
            true => ((s.progress)(&done, &format::number(total, t.locale)), Some((study.done_credits / total).clamp(0.0, 1.0))),
            false => ((s.progress_alone)(&done), None),
        };
        let mut ends = Vec::new();
        if let Some(end) = study.regular_end {
            ends.push((s.regular_end)(&end.label(t.locale)));
        }
        ends.push(match study.end {
            Some(end) => (s.expected_end)(&end.label(t.locale)),
            None => s.all_done.to_string(),
        });
        HeadInfo {
            name: program_name(&ready.program),
            when,
            direction,
            progress,
            share,
            done: study.done_credits,
            total,
            ends: ends.join(" · "),
            no_plan: setup.core.is_none(),
        }
    }
}

/// The head: whose study it is, where it stands, and how much of it is passed.
#[component]
fn Head(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let head = Memo::new(move |_| ctx.state.with(|state| state.ready().map(|ready| HeadInfo::of(ready, t))));
    move || {
        head.get().map(|head| {
            view! {
                <section class="panel st-head">
                    <div class="st-head-top">
                        <p class="st-kicker"><Icon name="star"/>{s.mine}</p>
                        <SheetToggle/>
                    </div>
                    <h1 class="st-title">{head.name}</h1>
                    <p class="st-when">{head.when}{head.direction.map(|direction| format!(" · {direction}"))}</p>
                    <div class="st-progress-line">
                        {head.share.map(|share| view! {
                            <div
                                class="st-progress"
                                role="progressbar"
                                aria-label=s.progress_label
                                aria-valuemin="0"
                                aria-valuemax=format::number(head.total, t.locale)
                                aria-valuenow=format::number(head.done, t.locale)
                                style=format!("--done: {share:.4}")
                            >
                                <i></i>
                            </div>
                        })}
                        <span class="st-progress-text num">{head.progress}</span>
                    </div>
                    <p class="st-ends">{head.ends}</p>
                    {head.no_plan.then(|| view! { <p class="hint">{s.no_plan}</p> })}
                </section>
            }
        })
    }
}

/// The semesters before the current one, folded: open while something there is open (the page
/// asks to tick it off), shut once all of it is.
#[component]
fn Past(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let past = Memo::new(move |_| ctx.state.with(|state| state.ready().map(|ready| ready.study.past.iter().map(|term| term.semester).collect::<Vec<_>>()).unwrap_or_default()));
    let summary = Memo::new(move |_| {
        ctx.state.with(|state| {
            let study = &state.ready()?.study;
            let all = study.past.iter().map(|term| term.items.len()).sum::<usize>();
            let span = (study.past.first().and_then(|term| term.fs), study.past.last().and_then(|term| term.fs));
            Some((span, all - study.past_open, all, study.past_open))
        })
    });
    move || {
        if past.with(Vec::is_empty) {
            return None;
        }
        let open = summary.with_untracked(|summary| summary.is_some_and(|(_, _, _, open)| open > 0));
        Some(view! {
            <details class="panel st-past" open=open>
                <summary>
                    <span class="st-past-title">
                        {s.past}
                        <small>{move || summary.get().and_then(|((from, to), ..)| Some((s.past_span)(from?, to?)))}</small>
                    </span>
                    <span class="st-past-count num">{move || summary.get().map(|(_, done, all, _)| (s.past_count)(done, all))}</span>
                    <Icon name="chevron-down" class="st-fold"/>
                </summary>
                {move || summary.get().filter(|(.., open)| *open > 0).map(|_| view! { <p class="st-prompt"><Icon name="info"/><span>{s.past_prompt}</span></p> })}
                <For each=move || past.get() key=|semester| *semester children=move |semester: SemesterKey| view! { <TermView ctx semester past=true/> }/>
            </details>
        })
    }
}

/// The semesters from the current one on.
#[component]
fn Terms(ctx: StudyCtx) -> impl IntoView {
    let terms = Memo::new(move |_| ctx.state.with(|state| state.ready().map(|ready| ready.study.terms.iter().map(|term| term.semester).collect::<Vec<_>>()).unwrap_or_default()));
    view! { <For each=move || terms.get() key=|semester| *semester children=move |semester: SemesterKey| view! { <TermView ctx semester past=false/> }/> }
}

/// What the head of a semester says, and what it lists.
#[derive(Clone, Debug, PartialEq)]
struct TermHead {
    fs: Option<String>,
    now: bool,
    beyond: bool,
    /// What it comes to; nothing where no item says a number of its own here.
    credits: Option<String>,
    planned: Option<String>,
    /// More than the Regelstudienplan puts into it.
    heavy: bool,
    keys: Vec<String>,
    open: usize,
}

/// One semester: its head and its rows; the current one ends in the way into the timetable, one
/// before it in „Alles bestanden" while something there is open.
#[component]
fn TermView(ctx: StudyCtx, semester: SemesterKey, past: bool) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let head = Memo::new(move |_| {
        ctx.state.with(|state| {
            let ready = state.ready()?;
            let term = ready.study.term(semester)?;
            let credits = format::number(term.credits, t.locale);
            Some(TermHead {
                fs: fs_label(semester, ready.setup.start, t),
                now: semester == ready.setup.now,
                beyond: term.beyond,
                credits: (term.credits > 0.0).then(|| (s.credits)(&credits, term.partial)),
                planned: term.planned.map(|planned| (s.planned)(&format::number(planned, t.locale))),
                heavy: term.planned.is_some_and(|planned| term.credits > planned + 0.5),
                keys: term.items.iter().map(Item::key).collect(),
                open: term.open().count(),
            })
        })
    });
    let keys = Memo::new(move |_| head.with(|head| head.as_ref().map(|head| head.keys.clone()).unwrap_or_default()));
    let now = Memo::new(move |_| head.with(|head| head.as_ref().is_some_and(|head| head.now)));
    let (heading, labelled) = (format!("st-{}", semester.key()), format!("st-{}", semester.key()));
    let all_passed = move |_| {
        ctx.change(
            move |doc, input| {
                let study = study::study(input);
                let Some(term) = study.term(semester) else { return };
                for item in term.open() {
                    study::set_done(doc, input, item, semester, true);
                }
            },
            || {},
        );
    };
    view! {
        <section class="st-term" class:panel=!past class:is-past=past class:is-now=move || now.get() aria-labelledby=labelled>
            <header class="st-term-head">
                <h2 id=heading>
                    <span class="st-sem">{semester.label(t.locale)}</span>
                    {move || head.get().map(|head| view! {
                        {head.fs.map(|fs| view! { <span class="st-fs">{fs}</span> })}
                        {head.now.then(|| view! { <span class="st-now">{s.now}</span> })}
                        {head.beyond.then(|| view! { <span class="st-beyond">{s.beyond}</span> })}
                    })}
                </h2>
                <span class="st-credits num" class:heavy=move || head.with(|head| head.as_ref().is_some_and(|head| head.heavy))>
                    {move || head.get().map(|head| view! {
                        {head.credits.map(|credits| view! { <b title=head.heavy.then_some(s.heavy)>{credits}</b> })}
                        {head.planned.map(|planned| view! { <small>{planned}</small> })}
                    })}
                </span>
                {past.then_some(move || head.with(|head| head.as_ref().is_some_and(|head| head.open > 0)).then(|| view! {
                    <button class="mini hit st-all" type="button" on:click=all_passed><Icon name="check-check"/>{s.all_passed}</button>
                }))}
            </header>
            <ul class="st-rows">
                <For each=move || keys.get() key=|key| key.clone() children=move |key: String| view! { <ItemRow ctx semester key past/> }/>
            </ul>
            {move || keys.with(Vec::is_empty).then(|| view! { <p class="hint st-empty">{s.empty}</p> })}
            {move || (!past && now.get()).then(|| view! { <TakeTimetable ctx/> })}
        </section>
    }
}

/// What a row shows besides its name.
#[derive(Clone, Debug, PartialEq)]
struct RowState {
    item: Item,
    /// The marks: class, words, icon.
    marks: Vec<(&'static str, String, Option<&'static str>)>,
    earlier: Option<SemesterKey>,
    later: Option<SemesterKey>,
    /// Moved by the student: „Zurück an seinen Platz".
    reset: bool,
    /// A module of one's own from the current semester on: „Aus dem Plan nehmen".
    remove: bool,
    /// A module of one's own left open before the current semester: „Nachholen" into this one.
    again: Option<SemesterKey>,
}

impl RowState {
    fn of(ready: &Ready, semester: SemesterKey, item: Item, past: bool, t: &Texts) -> Self {
        let s = &t.study;
        let (start, now) = (ready.setup.start, ready.study.now);
        let open = item.is_open();
        let mut marks = Vec::new();
        let plan_fs = item.fs.and_then(|fs| of_fachsemester(start, fs)).and_then(|due| fs_label(due, start, t));
        match item.place {
            Place::Overdue { since } if open => {
                if let Some(fs) = fs_label(since, start, t) {
                    marks.push(("overdue", (s.overdue)(&fs), None));
                }
            }
            Place::Early if item.pinned && open => marks.extend(plan_fs.as_deref().map(|fs| ("moved", (s.moved_earlier)(fs), None))),
            Place::Later if item.pinned && open => marks.extend(plan_fs.as_deref().map(|fs| ("moved", (s.moved_later)(fs), None))),
            Place::Own => marks.push(("own", s.own.to_string(), None)),
            _ => {}
        }
        // The turnus put it here, not the student: the plan's semester (or the current one) does
        // not offer it.
        let shifted = open
            && !item.pinned
            && match item.place {
                Place::Later => true,
                Place::Overdue { .. } => semester > now,
                _ => false,
            };
        if shifted {
            match item.offer.only() {
                Some(Season::Winter) => marks.push(("turnus", s.only_winter.to_string(), Some("snowflake"))),
                Some(Season::Summer) => marks.push(("turnus", s.only_summer.to_string(), Some("sun"))),
                None => {}
            }
        }
        let in_timetable = match &item.subject {
            Subject::Module { id, .. } => ready.in_timetable.contains(id),
            Subject::Row { ord, .. } => ready.rows_in_timetable.contains(ord),
        };
        if open && !past && semester == ready.setup.now && in_timetable {
            marks.push(("timetable", s.in_timetable.to_string(), Some("calendar-check-2")));
        }
        let own = matches!(item.subject, Subject::Module { own: true, .. });
        let movable = open && !past;
        RowState {
            earlier: movable.then(|| study::earlier(&item, semester, now)).flatten(),
            later: movable.then(|| study::later(&item, semester, start)).flatten(),
            reset: movable && item.pinned && !own && matches!(item.place, Place::Early | Place::Later),
            remove: movable && own,
            again: (past && open && own).then(|| item.offer.next(now)),
            marks,
            item,
        }
    }
}

/// The id of a row's box: where the focus goes when the row comes to another semester.
fn tick_id(semester: SemesterKey, key: &str) -> String {
    format!("st-{}-{}", semester.key(), key.replace(':', "-"))
}

/// One module or row of the plan in one semester.
#[component]
fn ItemRow(ctx: StudyCtx, semester: SemesterKey, key: String, past: bool) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let row = {
        let key = key.clone();
        Memo::new(move |_| {
            ctx.state.with(|state| {
                let ready = state.ready()?;
                let item = ready.study.term(semester)?.items.iter().find(|item| item.key() == key)?.clone();
                Some(RowState::of(ready, semester, item, past, t))
            })
        })
    };
    // What a row is stays what it is: its subject, name and credits.
    let Some(first) = row.get_untracked().map(|row| row.item) else { return ().into_any() };
    let done = Memo::new(move |_| row.with(|row| row.as_ref().is_some_and(|row| !row.item.is_open())));
    // What the last click said, until the plan has it.
    let said = RwSignal::new(None::<bool>);
    let checked = Memo::new(move |_| said.get().unwrap_or_else(|| done.get()));
    let moving = RwSignal::new(false);
    let toggle = {
        let item = first.clone();
        move |_| {
            let next = !checked.get_untracked();
            said.set(Some(next));
            let item = item.clone();
            ctx.change(
                move |doc, input| {
                    study::set_done(doc, input, &item, semester, next);
                },
                move || {
                    said.try_set(None);
                },
            );
        }
    };
    // Moves the row to another semester; the focus follows it there.
    let go = {
        let (item, key) = (first.clone(), key.clone());
        move |to: SemesterKey| {
            moving.set(true);
            let item = item.clone();
            let id = tick_id(to, &key);
            ctx.change(
                move |doc, input| {
                    study::move_item(doc, input, &item, to, now_secs());
                },
                move || {
                    moving.try_set(false);
                    request_animation_frame(move || nav::focus_by_id(&id));
                },
            );
        }
    };
    let back = {
        let item = first.clone();
        move |_| {
            moving.set(true);
            let item = item.clone();
            ctx.change(move |doc, input| study::reset_item(doc, input, &item), move || {
                moving.try_set(false);
            });
        }
    };
    let remove = {
        let item = first.clone();
        move |_| {
            moving.set(true);
            let Some(id) = item.module_id().map(str::to_string) else { return };
            ctx.change(move |doc, _| doc.unplan(semester, &id, &[]), move || {
                moving.try_set(false);
            });
        }
    };

    let name = first.name.clone();
    let label = match first.subject {
        Subject::Module { .. } => (s.passed_label)(&name),
        Subject::Row { .. } => (s.done_label)(&name),
    };
    let credits = match &first.subject {
        Subject::Row { credits: Some(text), .. } => Some((s.credits)(&plan::credits_in(text, t.locale), false)),
        _ => first.credits.map(|credits| (s.credits)(&format::number(credits, t.locale), false)),
    };
    // The module beside the page; a row's modules in the catalog.
    let what = match &first.subject {
        Subject::Module { id, .. } => {
            let id = id.clone();
            let href = {
                let id = id.clone();
                move || t.path(&ctx.url.with(|url| url.with_open(Some(&id)).path()))
            };
            let current = move || ctx.url.with(|url| url.open.as_deref() == Some(id.as_str())).then_some("true");
            view! { <a class="st-name" href=href data-noscroll="" aria-current=current>{name.clone()}</a> }.into_any()
        }
        Subject::Row { ord, .. } => {
            let find = find_href(ctx, *ord, t);
            view! {
                <span class="st-name">{name.clone()}</span>
                {find.map(|href| view! { <a class="st-find" href=t.path(&href) rel="nofollow"><Icon name="layout-list"/>{s.find_modules}</a> })}
            }
            .into_any()
        }
    };
    let marks = move || {
        row.get().map(|row| {
            row.marks
                .into_iter()
                .map(|(class, text, icon)| view! { <span class=format!("st-mark {class}")>{icon.map(|name| view! { <Icon name=name/> })}{text}</span> })
                .collect_view()
        })
    };
    let actions = move || {
        let row = row.get()?;
        let earlier = row.earlier.map(|to| {
            let go = go.clone();
            view! { <button class="icon-btn st-act hit" type="button" title=(s.earlier)(&to.label(t.locale)) aria-label=(s.earlier)(&to.label(t.locale)) on:click=move |_| go(to)><Icon name="arrow-up"/></button> }
        });
        let later = row.later.map(|to| {
            let go = go.clone();
            view! { <button class="icon-btn st-act hit" type="button" title=(s.later)(&to.label(t.locale)) aria-label=(s.later)(&to.label(t.locale)) on:click=move |_| go(to)><Icon name="arrow-down"/></button> }
        });
        let reset = row.reset.then(|| view! { <button class="icon-btn st-act hit" type="button" title=s.reset aria-label=s.reset on:click=back.clone()><Icon name="rotate-ccw"/></button> });
        let out = row.remove.then(|| view! { <button class="icon-btn st-act hit" type="button" title=s.remove aria-label=s.remove on:click=remove.clone()><Icon name="x"/></button> });
        let again = row.again.map(|to| {
            let go = go.clone();
            view! { <button class="mini hit" type="button" on:click=move |_| go(to)>{(s.again)(&to.label(t.locale))}</button> }
        });
        Some(view! { {earlier}{later}{reset}{out}{again} })
    };
    view! {
        <li class="st-row" class:is-done=move || checked.get() class:is-moving=move || moving.get()>
            <button
                class="st-tick hit"
                type="button"
                id=tick_id(semester, &key)
                role="checkbox"
                aria-checked=move || if checked.get() { "true" } else { "false" }
                aria-label=label
                aria-busy=move || said.get().map(|_| "true")
                on:click=toggle
            >
                <span class="box" class:with=move || checked.get()><Icon name="check"/></span>
            </button>
            <div class="st-what">
                {what}
                <span class="st-marks">{marks}</span>
            </div>
            <span class="st-lp num">{credits}</span>
            <span class="st-acts">{actions}</span>
        </li>
    }
    .into_any()
}

/// The catalog of a row's modules (`variants::row_query`: the areas its name means, the FÜS list,
/// the program's electives), for the row of the plan with this `ord`.
fn find_href(ctx: StudyCtx, ord: i64, t: &Texts) -> Option<String> {
    let setup = ctx.state.with_untracked(|state| state.ready().map(|ready| ready.setup))?;
    ctx.plans.with_untracked(|plans| {
        let Plans::Found(source) = plans else { return None };
        let parts = setup.core.and_then(|core| source.variants.get(core)).into_iter().chain(setup.page.and_then(|(page, _)| source.variants.get(page)));
        let (variant, entry) = parts.flat_map(|variant| variant.entries.iter().map(move |entry| (variant, entry))).find(|(_, entry)| entry.ord == ord && entry.module_id.is_none())?;
        let (query, _) = variants::row_query(&source.program.slug, variant, entry, &source.areas, t.locale);
        Some(CatalogUrl { query, page: 1, open: None, fill: None }.path())
    })
}

/// „In den Stundenplan": the current semester's modules and rows into its timetable, which
/// „Rückgängig" takes back; and the way there.
#[component]
fn TakeTimetable(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let to_take = Memo::new(move |_| ctx.state.with(|state| state.ready().map(|ready| ready.to_take)));
    let note = Memo::new(move |_| ctx.undo.with(|undo| undo.as_ref().map(|(note, _)| note.clone())));
    let busy = RwSignal::new(false);
    let take = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        let undo = ctx.undo;
        ctx.change(
            move |doc, input| {
                let before = doc.clone();
                let study = study::study(input);
                let Some(term) = study.term(input.now) else { return };
                let import = study::timetable_import(input, term);
                let (modules, placeholders) = doc.apply(&import, now_secs());
                let mut parts = Vec::new();
                if modules > 0 {
                    parts.push(format::modules(i64::try_from(modules).unwrap_or(i64::MAX), t.locale));
                }
                if placeholders > 0 {
                    parts.push((t.studyplan_head.placeholders)(placeholders));
                }
                undo.try_set(Some(((s.taken)(&parts.join(", ")), before)));
            },
            move || {
                busy.try_set(false);
            },
        );
    };
    let restoring = RwSignal::new(false);
    let restore = move |_| {
        let (Some(plan), Some((_, before))) = (ctx.plan, ctx.undo.get_untracked()) else { return };
        if restoring.get_untracked() {
            return;
        }
        restoring.set(true);
        let undo = ctx.undo;
        plan.update_after_paint(move |doc| {
            *doc = before;
            undo.try_set(None);
            restoring.try_set(false);
        });
    };
    view! {
        <footer class="st-take">
            {move || match (note.get(), to_take.get()) {
                (Some(note), _) => view! {
                    <p class="action note-action">
                        <Icon name="check"/>
                        <span>{note}</span>
                        <button class="mini hit" type="button" aria-busy=move || restoring.get().then_some("true") on:click=restore>{t.common.undo}</button>
                    </p>
                }
                .into_any(),
                (None, Some((modules, placeholders))) if modules + placeholders > 0 => view! {
                    <button class="btn primary" type="button" id="st-take" aria-busy=move || busy.get().then_some("true") on:click=take>
                        <Icon name="calendar-plus"/>{(s.take)(modules + placeholders)}
                    </button>
                }
                .into_any(),
                (None, Some(_)) => view! { <span class="st-taken"><Icon name="calendar-check-2"/>{s.taken_all}</span> }.into_any(),
                (None, None) => ().into_any(),
            }}
            <a class="ghost" href=t.path(url::STUDYPLAN) rel="nofollow"><Icon name="calendar-range"/>{s.to_timetable}<Icon name="chevron-right"/></a>
        </footer>
    }
}
