//! One semester in focus (the mockup's concept A, owner 2026-10-04: „am sichersten … und am besten
//! zwischen Handy und PC"): on a desktop a strip of every semester with what it holds, the
//! semester in focus as a table (a box to select the row, the module with its marks, its area, its
//! credits, its menu) and beside it what fits it (`Fits`); on a phone the semester as a card as
//! wide as the page, turned by ‹ › in its head or a swipe, dots under it. After the last semester
//! comes a page that adds one (owner: „Wenn man bis zum Schluss ist, soll eine Page kommen, wo man
//! dann ein neues Semester hinzufügen kann"), as a semester of leave if the student says so.
//!
//! A row (owner, the same day: „Die klickflächen sind zu klein"): its box as tall as the row, then
//! the module, which a click opens beside the page, then its ⋯, as tall as the row too, with
//! the row's menu (`menu.rs`; a right click opens it as well). Marking a module as passed is done
//! once and not by the way (owner: „Das Abhaken von dem Modul ist viel zu einfach"): it is in the
//! menu, and a row passed says so with a mark at its credits. The boxes select rows for what is
//! done to several at once (owner: „eine Multiselection für Bearbeitung"): on a desktop a row's
//! box shows under the pointer, all of them while rows are selected; a drag over the boxes selects
//! or lets go of every row it passes, Shift a range, Ctrl or ⌘ a row by its module. While rows are
//! selected a click on one selects it or lets it go, and a bar in the head of the table marks them
//! as passed, moves them, takes them out. On a phone a long press selects a row, and the bar is at
//! the bottom of the window. A row dragged onto a semester of the strip moves there, with the
//! others selected if it is one of them. Under the rows one more, „Module hinzufügen"; the
//! semester's own ⋯ in its head holds the rest: the Stundenplan, a semester of leave.

use std::collections::BTreeSet;
use std::time::Duration;

use folia_calendar::semester::SemesterKey;
use folia_model::labels::Season;
use folia_plans::study::{self, Item, Pick, Standing, Subject, When};
use folia_plans::variants;
use folia_routes::url::CatalogUrl;
use leptos::ev::{DragEvent, MouseEvent, PointerEvent};
use leptos::prelude::*;

use folia_design::ui::Icon;

use super::dom;
use super::menu::{self, Act, MenuFor};
use super::picker::needs_text;
use super::{item_of, module_href, n, now_secs, Dialog, Plans, Ready, Selection, SheetToggle, StudyCtx, ViewSwitch};
use crate::i18n::{self, Texts};

#[component]
pub(super) fn Focus(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let focused = Memo::new(move |_| ctx.focused());
    // The page that adds a semester: one after the last.
    let end = Memo::new(move |_| ctx.with_ready(|ready| ready.study.semesters.last().and_then(|last| last.key.plus(1))).flatten());
    // What is selected belongs to the semester in focus: turning to another lets go of it.
    Effect::new(move |_| {
        let now = focused.get();
        if ctx.selection.with_untracked(|selection| selection.semester.is_some() && selection.semester != now) {
            ctx.selection.set(Selection::default());
        }
    });
    view! {
        <section class="panel st-sems" aria-label=s.semesters>
            <div class="st-sems-head st-pc">
                <h2 class="st-kicker">{s.semesters}</h2>
                <ViewSwitch ctx/>
            </div>
            <Strip ctx/>
            <div class="st-focus">
                {move || {
                    let semester = focused.get()?;
                    Some(if Some(semester) == end.get() {
                        view! { <NewSemester ctx/> }.into_any()
                    } else {
                        view! { <SemesterCard ctx semester/> }.into_any()
                    })
                }}
                {move || focused.get().filter(|semester| Some(*semester) != end.get()).map(|semester| view! { <Fits ctx semester/> })}
            </div>
            <Dots ctx/>
            // On a phone the sidebar is a sheet: „Mein Studiengang", „Studiengang ändern …", the ways.
            <p class="st-sheet st-phone"><SheetToggle/></p>
        </section>
    }
}

/// A semester as the strip shows it.
#[derive(Clone, Debug, PartialEq)]
struct Tab {
    key: SemesterKey,
    label: String,
    fs: Option<String>,
    now: bool,
    /// „24 von 32 bestanden", „38 von 30 LP", „noch leer · Plan 30"; its number, and its class.
    figure: Option<String>,
    rest: String,
    class: &'static str,
    /// The load bar: filled, out of.
    load: (f64, f64),
}

impl Tab {
    fn of(ready: &Ready, key: SemesterKey, t: &Texts) -> Option<Self> {
        let s = &t.study;
        let semester = ready.study.semester(key)?;
        let planned = semester.planned;
        let (figure, rest, class, load) = match semester.when {
            When::Past => (Some(n(semester.passed, t)), (s.of_passed)(&n(semester.credits, t)), "ok", (semester.passed, semester.credits)),
            _ if semester.items.is_empty() => (None, planned.map_or_else(|| s.still_empty.to_string(), |plan| format!("{} · {}", s.still_empty, (s.plan_n)(&n(plan, t)))), "", (0.0, 1.0)),
            _ => {
                let heavy = planned.is_some_and(|plan| semester.credits > plan + 0.5);
                let rest = planned.map_or_else(|| (s.credits)(""), |plan| (s.of_planned)(&n(plan, t)));
                (Some(n(semester.credits, t)), rest, if heavy { "heavy" } else { "" }, (semester.credits, planned.unwrap_or(semester.credits).max(semester.credits)))
            }
        };
        Some(Tab { key, label: key.short(t.locale), fs: ready.fs_label(key, t), now: semester.when == When::Now, figure, rest, class, load })
    }
}

/// The strip of the semesters, with „+" for one more. Rows dropped on a semester move there.
#[component]
fn Strip(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let keys = Memo::new(move |_| ctx.with_ready(|ready| ready.study.semesters.iter().map(|semester| semester.key).collect::<Vec<_>>()).unwrap_or_default());
    let focused = Memo::new(move |_| ctx.focused());
    let end = Memo::new(move |_| keys.with(|keys| keys.last().and_then(|last| last.plus(1))));
    view! {
        <nav class="st-strip st-pc" aria-label=s.choose_semester>
            <For each=move || keys.get() key=|key| *key children=move |key: SemesterKey| {
                let tab = Memo::new(move |_| ctx.with_ready(|ready| Tab::of(ready, key, t)).flatten());
                let over = RwSignal::new(false);
                let drop = move |ev: DragEvent| {
                    ev.prevent_default();
                    over.set(false);
                    if let Some((from, keys)) = ctx.drag.get_untracked() {
                        ctx.drag.set(None);
                        menu::act(ctx, Act::Move { semester: from, keys, to: key }, t);
                    }
                };
                view! {
                    <button
                        type="button"
                        class=move || format!("st-tab {}", tab.with(|tab| tab.as_ref().map_or("", |tab| tab.class)))
                        class:is-now=move || tab.with(|tab| tab.as_ref().is_some_and(|tab| tab.now))
                        class:is-drop=move || over.get()
                        aria-pressed=move || if focused.get() == Some(key) { "true" } else { "false" }
                        on:click=move |_| ctx.focus.set(Some(key))
                        on:dragover=move |ev: DragEvent| if ctx.drag.with_untracked(|drag| drag.as_ref().is_some_and(|(from, _)| *from != key)) { ev.prevent_default(); over.set(true); }
                        on:dragleave=move |_| over.set(false)
                        on:drop=drop
                    >
                        {move || tab.get().map(|tab| view! {
                            <span class="st-tab-top">
                                <span class="st-tab-name">{tab.label}</span>
                                {if tab.now { view! { <span class="st-now">{s.now}</span> }.into_any() } else { view! { <span class="st-tab-fs">{tab.fs}</span> }.into_any() }}
                            </span>
                            <span class="st-tab-line">{tab.figure.map(|figure| view! { <b>{figure}</b>" " })}{tab.rest}</span>
                            <span class="st-load"><span style=format!("flex-grow: {}", tab.load.0)></span><span style=format!("flex-grow: {}", (tab.load.1 - tab.load.0).max(0.0))></span></span>
                        })}
                    </button>
                }
            }/>
            <button
                type="button"
                class="st-tab add"
                aria-label=s.add_semester
                title=s.add_semester
                aria-pressed=move || if focused.get().is_some() && focused.get() == end.get() { "true" } else { "false" }
                on:click=move |_| ctx.focus.set(end.get_untracked())
            >
                <Icon name="plus"/>
            </button>
        </nav>
    }
}

/// What the head of a semester says.
#[derive(Clone, Debug, PartialEq)]
struct Head {
    label: String,
    now: bool,
    leave: bool,
    beyond: bool,
    sub: String,
    figure: Option<(String, String, bool)>,
    previous: Option<SemesterKey>,
    next: Option<SemesterKey>,
    keys: Vec<String>,
    past: bool,
    /// What the plan has for its Fachsemester and is open (on a phone, under the rows).
    plan_still: Option<String>,
}

impl Head {
    fn of(ready: &Ready, key: SemesterKey, still: Option<String>, t: &Texts) -> Option<Self> {
        let s = &t.study;
        let study = &ready.study;
        let at = study.semesters.iter().position(|semester| semester.key == key)?;
        let semester = study.semesters.get(at)?;
        let sub = match semester.fs {
            Some(fs) => (s.fs_long)(fs),
            None if semester.leave => s.leave.to_string(),
            None => String::new(),
        };
        let figure = match semester.when {
            When::Past => Some(((s.head_past)(&n(semester.passed, t), &n(semester.credits, t)), String::new(), false)),
            _ => {
                let heavy = semester.planned.is_some_and(|plan| semester.credits > plan + 0.5);
                let mut rest = Vec::new();
                if let Some(plan) = semester.planned {
                    rest.push((s.plan_n)(&n(plan, t)));
                    if heavy {
                        rest.push((s.more)(&n(semester.credits - plan, t)));
                    }
                }
                (semester.credits > 0.0 || semester.planned.is_some()).then(|| ((s.credits)(&n(semester.credits, t)), rest.join(" · "), heavy))
            }
        };
        Some(Head {
            label: key.label(t.locale),
            now: semester.when == When::Now,
            leave: semester.leave,
            beyond: semester.beyond,
            sub,
            figure,
            previous: at.checked_sub(1).and_then(|before| study.semesters.get(before)).map(|semester| semester.key),
            next: study.semesters.get(at + 1).map(|semester| semester.key).or_else(|| key.plus(1)),
            keys: semester.items.iter().map(Item::key).collect(),
            past: semester.when == When::Past,
            plan_still: still,
        })
    }
}

/// The names of what the plan has open in semester `key`'s Fachsemester.
fn plan_still(ctx: StudyCtx, key: SemesterKey) -> Option<String> {
    ctx.with_input(|input, ready| {
        let fs = ready.study.semester(key)?.fs?;
        let open: Vec<String> = study::plan_semester(input, &ready.study, fs).into_iter().filter(|suggestion| suggestion.standing == Standing::Open).map(|suggestion| suggestion.name).collect();
        (!open.is_empty()).then(|| open.join(", "))
    })
    .flatten()
}

/// The semester in focus: its head with ‹ › and its ⋯, its rows, the bar of the rows selected, and
/// „Module hinzufügen" as the last row.
#[component]
fn SemesterCard(ctx: StudyCtx, semester: SemesterKey) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let head = Memo::new(move |_| {
        let still = plan_still(ctx, semester);
        ctx.with_ready(|ready| Head::of(ready, semester, still, t)).flatten()
    });
    let keys = Memo::new(move |_| head.with(|head| head.as_ref().map(|head| head.keys.clone()).unwrap_or_default()));
    // The rows selected here, in their order. A row that goes is let go of.
    let chosen = Memo::new(move |_| {
        ctx.selection.with(|selection| match selection.semester == Some(semester) {
            true => keys.with(|keys| keys.iter().filter(|key| selection.keys.contains(*key)).cloned().collect::<Vec<_>>()),
            false => Vec::new(),
        })
    });
    Effect::new(move |_| {
        let keys = keys.get();
        ctx.selection.maybe_update(|selection| {
            let before = selection.keys.len();
            if selection.semester == Some(semester) {
                selection.keys.retain(|key| keys.contains(key));
            }
            selection.keys.len() != before
        });
    });
    let selecting = Memo::new(move |_| !chosen.with(Vec::is_empty));
    let go = move |to: Option<SemesterKey>| {
        if let Some(to) = to {
            ctx.focus.set(Some(to));
        }
    };
    // A swipe turns the card (on a phone).
    let swipe = RwSignal::new(None::<(i32, i32)>);
    let down = move |ev: PointerEvent| {
        if ev.pointer_type() == "touch" {
            swipe.set(Some((ev.client_x(), ev.client_y())));
        }
    };
    let up = move |ev: PointerEvent| {
        if let Some((x, y)) = swipe.get_untracked() {
            let (dx, dy) = (ev.client_x() - x, ev.client_y() - y);
            if dx.abs() > 48 && dx.abs() > dy.abs() * 2 {
                let head = head.get_untracked();
                go(if dx < 0 { head.and_then(|head| head.next) } else { head.and_then(|head| head.previous) });
            }
        }
        swipe.set(None);
    };
    view! {
        <article class="st-card-sem" class:is-now=move || head.with(|head| head.as_ref().is_some_and(|head| head.now)) aria-labelledby=format!("st-sem-{}", semester.key()) on:pointerdown=down on:pointerup=up on:pointercancel=move |_| swipe.set(None)>
            {move || head.get().map(|head| view! {
                <header class="st-sem-head">
                    <button class="icon-btn st-turn hit" type="button" aria-label=s.previous disabled=head.previous.is_none() on:click=move |_| go(head.previous)><Icon name="chevron-left"/></button>
                    <div class="st-sem-title">
                        <div class="st-sem-name">
                            <h2 id=format!("st-sem-{}", semester.key())>{head.label.clone()}</h2>
                            {head.now.then(|| view! { <span class="st-now">{s.now}</span> })}
                            {head.beyond.then(|| view! { <span class="st-badge warn">{s.beyond}</span> })}
                            <span class="st-sem-fs st-pc">{head.sub.clone()}</span>
                        </div>
                        <p class="st-sem-sub">
                            <span class="st-phone">{head.sub.clone()}{head.figure.is_some().then_some(" · ")}</span>
                            {head.figure.clone().map(|(figure, rest, heavy)| view! {
                                <b class:heavy=heavy class:ok=head.past>{figure}</b>
                                {(!rest.is_empty()).then(|| format!(" · {rest}"))}
                            })}
                        </p>
                    </div>
                    <button
                        class="icon-btn st-sem-more hit"
                        type="button"
                        aria-haspopup="menu"
                        aria-label=(s.more_about)(&head.label)
                        title=(s.more_about)(&head.label)
                        on:click=move |ev: MouseEvent| menu::open_at_button(ctx, MenuFor::Semester(semester), &ev)
                    >
                        <Icon name="ellipsis"/>
                    </button>
                    <button class="icon-btn st-turn hit" type="button" aria-label=s.next disabled=head.next.is_none() on:click=move |_| go(head.next)><Icon name="chevron-right"/></button>
                </header>
            })}
            <div class="st-table" class:is-selecting=selecting>
                <div class="st-cols st-pc">
                    <SelectAll ctx semester keys chosen/>
                    <span>{s.col_module}</span>
                    <span>{s.col_area}</span>
                    <span>{s.col_credits}</span>
                    <span></span>
                </div>
                {move || selecting.get().then(|| view! { <SelectionBar ctx semester keys chosen/> })}
                <ul class="st-rows">
                    <For each=move || keys.get() key=|key| key.clone() children=move |key: String| view! { <ItemRow ctx semester key order=keys/> }/>
                    {move || keys.with(Vec::is_empty).then(|| {
                        let leave = head.with(|head| head.as_ref().is_some_and(|head| head.leave));
                        view! { <li class="st-empty">{if leave { s.empty_leave } else { s.empty_semester }}</li> }
                    })}
                    <li class="st-addrow">
                        <button class="st-add" type="button" on:click=move |_| ctx.open(Dialog::Add { semester, catalog: false, chosen: Vec::new() })>
                            <span class="st-add-icon" aria-hidden="true"><Icon name="plus"/></span>
                            <span>{s.add_modules}</span>
                        </button>
                    </li>
                </ul>
            </div>
            {move || head.with(|head| head.as_ref().and_then(|head| (!head.past).then(|| head.plan_still.clone()).flatten())).map(|names| {
                let fs = head.with(|head| head.as_ref().map(|head| head.sub.clone()).unwrap_or_default());
                view! { <p class="st-still st-phone">{(s.plan_still)(&fs.replace("Fachsemester", "FS"), &names)}</p> }
            })}
            // What a drag of several rows shows under the pointer.
            <div class="st-ghost" aria-hidden="true">{move || (s.n_entries)(chosen.with(Vec::len))}</div>
        </article>
    }
}

/// The box in the head of the table: every row selected, some, none. A click selects all of them,
/// or lets all of them go where all are.
#[component]
fn SelectAll(ctx: StudyCtx, semester: SemesterKey, keys: Memo<Vec<String>>, chosen: Memo<Vec<String>>) -> impl IntoView {
    let t = i18n::t();
    let state = Memo::new(move |_| match (chosen.with(Vec::len), keys.with(Vec::len)) {
        (0, _) => "false",
        (some, all) if some < all => "mixed",
        _ => "true",
    });
    let click = move |_| {
        if state.get_untracked() == "true" {
            ctx.selection.set(Selection::default());
        } else {
            menu::act(ctx, Act::SelectAll(semester), t);
        }
    };
    view! {
        <button class="st-sel all" type="button" role="checkbox" aria-checked=move || state.get() aria-label=t.study.select_all disabled=move || keys.with(Vec::is_empty) on:click=click>
            <span class="st-box" class:with=move || state.get() != "false" class:mixed=move || state.get() == "mixed" aria-hidden="true">
                <Icon name="check"/>
                <Icon name="minus" class="part"/>
            </span>
        </button>
    }
}

/// The bar of the rows selected (owner, 2026-10-04: „Ich finde die Optionen als bestanden
/// markieren und Löschen als sinnvoll"): how many and their credits; „Als bestanden markieren" (or
/// taking that back where all of them are), „Verschieben nach", „Entfernen"; and letting go of
/// them. In the head of the table on a desktop, at the bottom of the window on a phone.
#[component]
fn SelectionBar(ctx: StudyCtx, semester: SemesterKey, keys: Memo<Vec<String>>, chosen: Memo<Vec<String>>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    // How many, their credits, whether all of them are passed.
    let summary = Memo::new(move |_| {
        let chosen = chosen.get();
        ctx.with_ready(|ready| {
            let items: Vec<Item> = chosen.iter().filter_map(|key| item_of(ready, semester, key)).collect();
            let credits: f64 = items.iter().filter_map(|item| item.credits).sum();
            (items.len(), credits, !items.is_empty() && items.iter().all(|item| item.passed))
        })
        .unwrap_or((0, 0.0, false))
    });
    let all_passed = Memo::new(move |_| summary.get().2);
    let passed = move |_| menu::act(ctx, Act::Passed { semester, keys: chosen.get_untracked(), passed: !all_passed.get_untracked() }, t);
    let remove = move |_| menu::act(ctx, Act::Remove { semester, keys: chosen.get_untracked() }, t);
    view! {
        <div class="st-selbar" role="toolbar" aria-label=s.selection>
            <SelectAll ctx semester keys chosen/>
            <span class="st-selbar-count" aria-live="polite">
                {move || (s.n_selected)(summary.get().0)}
                <span class="st-pc">{move || format!(" · {}", (s.credits)(&n(summary.get().1, t)))}</span>
            </span>
            <span class="st-grow"></span>
            <button class="st-act" type="button" id="st-sel-passed" on:click=passed>
                {move || if all_passed.get() { view! { <Icon name="rotate-ccw"/> }.into_any() } else { view! { <Icon name="circle-check-big"/> }.into_any() }}
                <span class="st-pc">{move || if all_passed.get() { s.unmark_passed } else { s.mark_passed }}</span>
                <span class="st-phone">{move || if all_passed.get() { s.unmark_short } else { s.act_passed }}</span>
            </button>
            <button class="st-act" type="button" id="st-sel-move" aria-haspopup="menu" on:click=move |ev: MouseEvent| menu::open_at_button(ctx, MenuFor::Move(semester), &ev)>
                <Icon name="arrow-right-left"/>
                <span class="st-pc">{s.move_to}</span>
                <span class="st-phone">{s.act_move}</span>
                <Icon name="chevron-down" class="st-pc"/>
            </button>
            <button class="st-act danger" type="button" id="st-sel-remove" on:click=remove>
                <Icon name="trash-2"/>
                <span>{s.remove}</span>
            </button>
            <button class="icon-btn st-act-close hit" type="button" aria-label=s.clear_selection title=s.clear_selection on:click=move |_| ctx.selection.set(Selection::default())>
                <Icon name="x"/>
            </button>
        </div>
    }
}

/// What a row shows besides its name.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    item: Item,
    area: String,
    tone: &'static str,
    credits: Option<String>,
    /// The marks: class, words, icon.
    chips: Vec<(&'static str, String, Option<&'static str>)>,
    /// A module not passed here and not planned again: where „Einplanen" puts it.
    again: Option<SemesterKey>,
    /// The catalog of a row's modules.
    find: Option<String>,
}

/// The season of a semester as the marks say it: „WiSe", „SoSe".
fn season_word(s: SemesterKey, t: &Texts) -> &'static str {
    if s.winter { t.format.winter_short } else { t.format.summer_short }
}

impl Row {
    fn of(ctx: StudyCtx, ready: &Ready, semester: SemesterKey, item: Item, t: &Texts) -> Self {
        let s = &t.study;
        let study = &ready.study;
        let mut chips = Vec::new();
        if let Some(before) = item.retake {
            let from = ready.fs_label(before, t).unwrap_or_else(|| before.short(t.locale));
            chips.push(("", (s.chip_retake)(&from), Some("repeat")));
        }
        if item.unoffered && !item.passed {
            let only = match item.offer.only() {
                Some(Season::Winter) => format!(" · {}", s.only_winter),
                Some(Season::Summer) => format!(" · {}", s.only_summer),
                None => String::new(),
            };
            chips.push(("warn", format!("{}{only}", (s.unoffered)(season_word(semester, t))), Some("triangle-alert")));
        }
        // What the module asks for and is not in place: said here, as the only place it was before
        // (the row's dialog) is gone.
        if let (Some(id), false) = (item.module_id(), item.passed) {
            let doc = ctx.plan.map(|plan| plan.with(Clone::clone)).unwrap_or_default();
            if let Some((text, true)) = ctx.needs.with(|needs| needs_text(&doc, needs, id, &item.name, semester, study.now, t)) {
                chips.push(("warn long", text, Some("triangle-alert")));
            }
        }
        if item.over > 0.0 {
            let area = ready.area_name(item.area, t);
            let required = item.area.and_then(|area| study.progress.get(area)).map_or(0.0, |progress| progress.area.required);
            chips.push(("over", (s.over_full)(&area, &n(required, t)), None));
        }
        if let Subject::Row { .. } = item.subject {
            if item.fillers.is_empty() {
                chips.push(("", s.placeholder.to_string(), None));
            } else {
                let names: Vec<String> = ctx.rows.with_untracked(|rows| item.fillers.iter().map(|id| rows.iter().find(|row| row.id == *id).map_or_else(|| id.clone(), |row| row.title.clone())).collect());
                chips.push(("", (s.filled_with)(&names.join(", ")), None));
            }
        }
        let mut again = None;
        if item.failed {
            match item.again {
                Some(later) => chips.push(("quiet", (s.failed_again)(&later.label(t.locale)), None)),
                None => {
                    chips.push(("quiet warn", s.failed_open.to_string(), None));
                    let next = item.offer.next(study.now);
                    again = Some(if study.semester(next).is_some() { next } else { study.now });
                }
            }
        }
        let credits = match &item.subject {
            Subject::Row { credits: Some(text), .. } => Some(folia_plans::plan::credits_in(text, t.locale)),
            _ => item.credits.map(|credits| n(credits, t)),
        };
        let find = match &item.subject {
            Subject::Row { caption, ord, .. } if item.fillers.is_empty() => find_href(ctx, caption, *ord, t),
            _ => None,
        };
        Row { area: ready.area_name(item.area, t), tone: ready.tone(item.area), credits, chips, again, find, item }
    }
}

/// The catalog of a plan row's modules (`variants::row_query`).
pub(super) fn find_href(ctx: StudyCtx, caption: &str, ord: i64, t: &Texts) -> Option<String> {
    ctx.plans.with_untracked(|plans| {
        let Plans::Found(source) = plans else { return None };
        let variant = source.variants.iter().find(|variant| variant.full == caption)?;
        let entry = variant.entries.iter().find(|entry| entry.ord == ord)?;
        let (query, _) = variants::row_query(&source.program.slug, variant, entry, &source.areas, t.locale);
        Some(CatalogUrl { query, page: 1, open: None, fill: None }.path())
    })
}

/// A drag over the boxes: the row it began at, whether it selects or lets go, what was selected
/// before it.
#[derive(Clone, Debug, PartialEq)]
struct Paint {
    start: usize,
    select: bool,
    base: BTreeSet<String>,
}

/// What is selected while a drag over the boxes is at row `at`: the rows between it and where the
/// drag began selected (or let go of), the others as they were before.
fn painted(order: &[String], paint: &Paint, at: usize) -> BTreeSet<String> {
    let (from, to) = (paint.start.min(at), paint.start.max(at));
    let mut keys = paint.base.clone();
    for key in order.get(from..=to).unwrap_or_default() {
        if paint.select {
            keys.insert(key.clone());
        } else {
            keys.remove(key);
        }
    }
    keys
}

/// Selects the row `key` of semester `s`, or lets it go.
fn toggle(ctx: StudyCtx, s: SemesterKey, key: &str) {
    ctx.selection.update(|selection| {
        if selection.semester != Some(s) {
            *selection = Selection { semester: Some(s), ..Selection::default() };
        }
        if !selection.keys.remove(key) {
            selection.keys.insert(key.to_string());
        }
        selection.anchor = Some(key.to_string());
    });
}

/// Selects the rows from the last one clicked to `key` (Shift), besides those selected.
fn extend(ctx: StudyCtx, s: SemesterKey, order: &[String], key: &str) {
    let Some(at) = order.iter().position(|other| other == key) else { return };
    ctx.selection.update(|selection| {
        if selection.semester != Some(s) {
            *selection = Selection { semester: Some(s), ..Selection::default() };
        }
        let from = selection.anchor.as_ref().and_then(|anchor| order.iter().position(|other| other == anchor)).unwrap_or(at);
        selection.keys.extend(order.get(from.min(at)..=from.max(at)).unwrap_or_default().iter().cloned());
        selection.anchor.get_or_insert_with(|| key.to_string());
    });
}

/// One module or row of the plan in a semester: its box, itself, its ⋯.
#[component]
fn ItemRow(ctx: StudyCtx, semester: SemesterKey, key: String, order: Memo<Vec<String>>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let row = {
        let key = key.clone();
        Memo::new(move |_| ctx.with_ready(|ready| item_of(ready, semester, &key).map(|item| Row::of(ctx, ready, semester, item, t))).flatten())
    };
    // What a row is stays what it is: its subject and name.
    let Some(first) = row.get_untracked().map(|row| row.item) else { return ().into_any() };
    let key = StoredValue::new(key);
    let selected = Memo::new(move |_| key.with_value(|key| ctx.selection.with(|selection| selection.has(semester, key))));
    let selecting = Memo::new(move |_| ctx.selection.with(|selection| selection.any_in(semester)));
    let passed = Memo::new(move |_| row.with(|row| row.as_ref().is_some_and(|row| row.item.passed)));

    // The box: a click selects the row or lets it go; pressed and drawn over the other boxes, it
    // does the same to every row it passes.
    let paint = StoredValue::new(None::<Paint>);
    // A press the pointer handled: the click after it is no second one.
    let drawn = StoredValue::new(false);
    let box_down = move |ev: PointerEvent| {
        if ev.button() != 0 || (ev.pointer_type() == "touch" && !selecting.get_untracked()) {
            return;
        }
        ev.prevent_default();
        drawn.set_value(true);
        let order = order.get_untracked();
        let key = key.get_value();
        if ev.shift_key() {
            extend(ctx, semester, &order, &key);
            return;
        }
        let Some(at) = order.iter().position(|other| *other == key) else { return };
        dom::capture(&ev);
        let base = ctx.selection.with_untracked(|selection| if selection.semester == Some(semester) { selection.keys.clone() } else { BTreeSet::new() });
        let start = Paint { start: at, select: !base.contains(&key), base };
        ctx.selection.set(Selection { semester: Some(semester), keys: painted(&order, &start, at), anchor: Some(key) });
        paint.set_value(Some(start));
    };
    let box_move = move |ev: PointerEvent| {
        if paint.with_value(Option::is_none) {
            return;
        }
        let (x, y) = (f64::from(ev.client_x()), f64::from(ev.client_y()));
        let Some(over) = dom::row_at(x, y).or_else(|| dom::row_nearest(y)) else { return };
        let order = order.get_untracked();
        let Some(at) = order.iter().position(|other| *other == over) else { return };
        let keys = paint.with_value(|paint| paint.as_ref().map(|paint| painted(&order, paint, at)));
        if let Some(keys) = keys.filter(|keys| ctx.selection.with_untracked(|selection| selection.keys != *keys)) {
            ctx.selection.update(|selection| selection.keys = keys);
        }
    };
    let box_up = move |_| paint.set_value(None);
    let box_click = move |_| {
        if drawn.get_value() {
            drawn.set_value(false);
            return;
        }
        // Space or Enter.
        toggle(ctx, semester, &key.get_value());
    };

    // The row itself: a click opens the module beside the page; while rows are selected, or with
    // Ctrl or ⌘, it selects the row; with Shift the rows up to it. A finger held on it a while
    // selects it (a phone's way to begin).
    let press = StoredValue::new(None::<(TimeoutHandle, i32, i32)>);
    let pressed = StoredValue::new(false);
    let touched = StoredValue::new(false);
    let stop_press = move || {
        if let Some((handle, ..)) = press.get_value() {
            handle.clear();
            press.set_value(None);
        }
    };
    let body_down = move |ev: PointerEvent| {
        touched.set_value(ev.pointer_type() == "touch");
        pressed.set_value(false);
        stop_press();
        if ev.pointer_type() != "touch" || selecting.get_untracked() {
            return;
        }
        let held = set_timeout_with_handle(
            move || {
                press.set_value(None);
                pressed.set_value(true);
                let key = key.get_value();
                ctx.selection.set(Selection { semester: Some(semester), keys: BTreeSet::from([key.clone()]), anchor: Some(key) });
            },
            Duration::from_millis(450),
        );
        if let Ok(handle) = held {
            press.set_value(Some((handle, ev.client_x(), ev.client_y())));
        }
    };
    let body_move = move |ev: PointerEvent| {
        if let Some((_, x, y)) = press.get_value() {
            if (ev.client_x() - x).abs() > 10 || (ev.client_y() - y).abs() > 10 {
                stop_press();
            }
        }
    };
    let body_click = move |ev: MouseEvent| {
        if pressed.get_value() {
            pressed.set_value(false);
            ev.prevent_default();
            return;
        }
        if ev.shift_key() {
            ev.prevent_default();
            extend(ctx, semester, &order.get_untracked(), &key.get_value());
            return;
        }
        let by_key = ev.ctrl_key() || ev.meta_key();
        if by_key || selecting.get_untracked() {
            // The module's link with Ctrl or ⌘ opens it apart, as a link does; the row's own
            // controls („Modul wählen", „Einplanen") do what they say.
            if (by_key && dom::on_name(&ev)) || (dom::on_control(&ev) && !dom::on_name(&ev)) {
                return;
            }
            ev.prevent_default();
            toggle(ctx, semester, &key.get_value());
            return;
        }
        if !dom::on_control(&ev) {
            dom::follow_name(&ev);
        }
    };
    // A right click opens the row's menu where the pointer is; a long press of a finger does not.
    let context = move |ev: MouseEvent| {
        ev.prevent_default();
        if !touched.get_value() {
            menu::open_at_pointer(ctx, MenuFor::Item { semester, key: key.get_value() }, &ev);
        }
    };
    // Dragged onto a semester of the strip: the row, or every row selected if it is one of them.
    let drag_start = move |ev: DragEvent| {
        let keys = if selected.get_untracked() { menu::selected_keys(ctx, semester) } else { vec![key.get_value()] };
        dom::drag_data(&ev, &keys);
        if keys.len() > 1 {
            dom::drag_picture(&ev, ".st-card-sem .st-ghost");
        }
        ctx.drag.set(Some((semester, keys)));
    };
    let menu_open = Memo::new(move |_| ctx.menu.with(|menu| menu.as_ref().is_some_and(|menu| matches!(&menu.what, MenuFor::Item { semester: there, key: of } if *there == semester && key.with_value(|key| key == of)))));

    let name = first.name.clone();
    let what = match &first.subject {
        Subject::Module { id } => {
            let id = id.clone();
            let href = {
                let id = id.clone();
                move || module_href(ctx, &id, t)
            };
            let current = move || ctx.url.with(|url| url.open.as_deref() == Some(id.as_str())).then_some("true");
            view! { <a class="st-name" href=href data-noscroll="" draggable="false" aria-current=current>{name.clone()}</a> }.into_any()
        }
        Subject::Row { .. } => view! { <span class="st-name">{name.clone()}</span> }.into_any(),
    };
    view! {
        <li
            class="st-row"
            data-key=key.get_value()
            style=move || row.with(|row| row.as_ref().map(|row| format!("--c: {}", row.tone)))
            class:is-passed=move || passed.get()
            class:is-failed=move || row.with(|row| row.as_ref().is_some_and(|row| row.item.failed))
            class:is-selected=move || selected.get()
            on:contextmenu=context
        >
            <button
                class="st-sel"
                type="button"
                role="checkbox"
                aria-checked=move || if selected.get() { "true" } else { "false" }
                aria-label=(s.select)(&first.name)
                on:pointerdown=box_down
                on:pointermove=box_move
                on:pointerup=box_up
                on:pointercancel=box_up
                on:click=box_click
            >
                <span class="st-box" class:with=move || selected.get() aria-hidden="true"><Icon name="check"/></span>
            </button>
            <div
                class="st-body"
                class:opens=first.module_id().is_some()
                draggable=move || if ctx.phone.get() { "false" } else { "true" }
                on:dragstart=drag_start
                on:dragend=move |_| ctx.drag.set(None)
                on:pointerdown=body_down
                on:pointermove=body_move
                on:pointerup=move |_| stop_press()
                on:pointercancel=move |_| stop_press()
                on:click=body_click
            >
                <div class="st-what">
                    {what}
                    {move || row.get().map(|row| view! {
                        <span class="st-area st-phone"><span class="st-dot" style=format!("--c: {}", row.tone)></span>{row.area.clone()}</span>
                        {row.chips.into_iter().map(|(class, text, icon)| view! { <span class=format!("st-chip {class}")>{icon.map(|icon| view! { <Icon name=icon/> })}{text}</span> }).collect_view()}
                        {row.find.map(|href| view! { <a class="st-find" href=t.path(&href) rel="nofollow" draggable="false"><Icon name="search"/>{s.choose_module}</a> })}
                        {row.again.map(|to| {
                            let key = row.item.key();
                            view! { <button class="st-link" type="button" on:click=move |_| { ctx.focus.set(Some(to)); ctx.open(Dialog::Add { semester: to, catalog: false, chosen: vec![key.clone()] }); }>{s.plan_it}</button> }
                        })}
                    })}
                </div>
                <span class="st-area st-pc">{move || row.get().map(|row| view! { <span class="st-dot" style=format!("--c: {}", row.tone)></span>{row.area} })}</span>
                <span class="st-lp num">
                    {move || passed.get().then(|| view! { <span class="st-passed" title=s.passed_word><Icon name="circle-check-big"/><span class="visually-hidden">{s.passed_word}</span></span> })}
                    {move || row.with(|row| row.as_ref().and_then(|row| row.credits.clone()))}
                </span>
            </div>
            <button
                class="icon-btn st-more"
                type="button"
                aria-haspopup="menu"
                aria-expanded=move || if menu_open.get() { "true" } else { "false" }
                aria-label=(s.more_about)(&first.name)
                title=(s.more_about)(&first.name)
                on:click=move |ev: MouseEvent| menu::open_at_button(ctx, MenuFor::Item { semester, key: key.get_value() }, &ev)
            >
                <Icon name="ellipsis"/>
            </button>
        </li>
    }
    .into_any()
}

/// An entry of „Passt in dieses Semester".
#[derive(Clone, Debug, PartialEq)]
struct Fit {
    pick: Pick,
    name: String,
    line: String,
}

/// What „Passt in dieses Semester" lists (`study::fits`): nothing of a Fachsemester after the
/// semester's, nothing the semester does not offer.
#[derive(Clone, Debug, PartialEq)]
struct Fits {
    fs: Option<String>,
    /// The plan's rows of its Fachsemester, open and offered; where there are none, why.
    plan: Vec<Fit>,
    rest: Rest,
    retakes: Vec<Fit>,
    earlier: Vec<Fit>,
}

/// Why „Laut Regelstudienplan" lists nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rest {
    /// The plan has nothing for the Fachsemester.
    Nothing,
    /// All of it is planned or passed.
    Planned,
    /// What is open is not offered in the semester.
    Unoffered,
}

/// How many of what was open in the Fachsemester before „Passt in dieses Semester" lists.
const EARLIER: usize = 4;

impl Fits {
    fn of(ctx: StudyCtx, semester: SemesterKey, t: &Texts) -> Option<Self> {
        let s = &t.study;
        ctx.with_input(|input, ready| {
            let fits = study::fits(input, &ready.study, &ready.lines, semester);
            let line = |suggestion: &study::Suggestion| {
                let credits = suggestion.credits_text.as_ref().map(|text| folia_plans::plan::credits_in(text, t.locale)).or_else(|| suggestion.credits.map(|credits| n(credits, t)));
                [credits.map(|credits| (s.credits)(&credits)), Some(ready.area_name(suggestion.area, t))].into_iter().flatten().collect::<Vec<_>>().join(" · ")
            };
            let fit = |suggestion: &study::Suggestion, also: Option<String>| Fit {
                pick: suggestion.pick.clone(),
                name: suggestion.name.clone(),
                line: std::iter::once(line(suggestion)).chain(also).collect::<Vec<_>>().join(" · "),
            };
            let rest = match (fits.rows, fits.open) {
                (0, _) => Rest::Nothing,
                (_, 0) => Rest::Planned,
                _ => Rest::Unoffered,
            };
            let retakes = fits
                .retakes
                .iter()
                .filter_map(|l| {
                    let failed = l.failed_in?;
                    let from = ready.fs_label(failed, t).unwrap_or_else(|| failed.short(t.locale));
                    Some(fit(&l.suggestion, Some((s.from_fs)(&from))))
                })
                .collect();
            let earlier = fits.earlier.iter().take(EARLIER).map(|l| fit(&l.suggestion, l.plan_fs.map(|plan| (s.plan_says)(&(s.fs)(plan))))).collect();
            Fits { fs: fits.fs.map(s.fs), plan: fits.plan.iter().map(|suggestion| fit(suggestion, None)).collect(), rest, retakes, earlier }
        })
    }
}

/// Adds `pick` to semester `s` at once, with a note to take it back.
pub(super) fn add_pick(ctx: StudyCtx, s: SemesterKey, picks: Vec<Pick>, t: &'static Texts) {
    let note = (t.study.added)(picks.len());
    ctx.change_noted(Some(note), move |doc, input| {
        study::add(doc, input, s, &picks, now_secs());
    }, || {});
}

/// „Passt in dieses Semester": what the plan has open for its Fachsemester, the Wiederholer, what
/// is still open of the Fachsemester before; of each only what the semester offers, each a click
/// from being planned here. A desktop's.
#[component]
fn Fits(ctx: StudyCtx, semester: SemesterKey) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let fits = Memo::new(move |_| Fits::of(ctx, semester, t));
    let entry = move |fit: Fit| {
        let label = (s.plan_here)(&fit.name);
        let pick = fit.pick.clone();
        view! {
            <li class="st-fit">
                <div>
                    <p class="st-fit-name">{fit.name}</p>
                    <p class="st-sub">{fit.line}</p>
                </div>
                <button class="icon-btn st-plus-btn hit" type="button" aria-label=label.clone() title=label on:click=move |_| add_pick(ctx, semester, vec![pick.clone()], t)><Icon name="plus"/></button>
            </li>
        }
    };
    view! {
        <aside class="st-fits st-pc" aria-label=s.fits_title>
            <h3>{s.fits_title}</h3>
            {move || fits.get().map(|fits| view! {
                {fits.fs.clone().map(|fs| view! {
                    <div class="st-fits-part">
                        <p class="label">{(s.fits_plan)(&fs)}</p>
                        {match (fits.plan.is_empty(), fits.rest) {
                            (false, _) => view! { <ul>{fits.plan.into_iter().map(entry).collect_view()}</ul> }.into_any(),
                            (true, Rest::Planned) => view! { <p class="st-fits-done"><Icon name="check"/>{s.fits_all}</p> }.into_any(),
                            (true, Rest::Unoffered) => view! { <p class="st-sub">{(s.fits_unoffered)(season_word(semester, t))}</p> }.into_any(),
                            (true, Rest::Nothing) => view! { <p class="st-sub">{s.fits_nothing}</p> }.into_any(),
                        }}
                    </div>
                })}
                {(!fits.retakes.is_empty()).then(|| view! {
                    <div class="st-fits-part">
                        <p class="label">{s.retakes}</p>
                        <ul>{fits.retakes.into_iter().map(entry).collect_view()}</ul>
                    </div>
                })}
                {(!fits.earlier.is_empty()).then(|| view! {
                    <div class="st-fits-part">
                        <p class="label">{s.fits_earlier}</p>
                        <ul>{fits.earlier.into_iter().map(entry).collect_view()}</ul>
                    </div>
                })}
            })}
            <button class="st-link" type="button" on:click=move |_| ctx.open(Dialog::Add { semester, catalog: true, chosen: Vec::new() })><Icon name="search"/>{s.search_catalog}</button>
        </aside>
    }
}

/// The page after the last semester: one more, as a semester of leave if the student says so.
#[component]
fn NewSemester(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let info = Memo::new(move |_| {
        ctx.with_ready(|ready| {
            let study = &ready.study;
            let last = study.semesters.last()?;
            let next = last.key.plus(1)?;
            let fs = study.semesters.iter().filter_map(|semester| semester.fs).max().map(|fs| fs.saturating_add(1));
            let plan_fs = ready.setup.core.and(study.regular_end).and_then(|end| study.semester(end)).and_then(|semester| semester.fs);
            Some((last.key, next, fs, plan_fs))
        })
        .flatten()
    });
    let leave = RwSignal::new(false);
    let append = move |_| {
        let (Some(mine), Some((_, next, ..))) = (ctx.mine, info.get_untracked()) else { return };
        if leave.get_untracked() {
            mine.set_leave(next, true);
        }
        mine.set_until(Some(next));
        ctx.focus.set(Some(next));
    };
    move || {
        info.get().map(|(last, next, fs, plan_fs)| view! {
            <article class="st-card-sem st-new" aria-labelledby="st-new-title">
                <header class="st-sem-head">
                    <button class="icon-btn st-turn hit" type="button" aria-label=s.previous on:click=move |_| ctx.focus.set(Some(last))><Icon name="chevron-left"/></button>
                    <div class="st-sem-title">
                        <div class="st-sem-name"><h2 id="st-new-title">{s.new_semester}</h2></div>
                        <p class="st-sem-sub">{(s.after)(&last.label(t.locale))}</p>
                    </div>
                    <button class="icon-btn st-turn hit" type="button" aria-label=s.next disabled=true><Icon name="chevron-right"/></button>
                </header>
                <div class="st-new-body">
                    <span class="st-new-icon" aria-hidden="true"><Icon name="calendar-plus"/></span>
                    <p class="st-new-title">{s.append_title}</p>
                    <p class="st-sub">{plan_fs.map_or_else(|| s.append_lead_plain.to_string(), s.append_lead)}</p>
                    <button class="btn primary" type="button" id="st-append" on:click=append>
                        <span>{(s.append)(&next.label(t.locale))}</span>
                        {move || (!leave.get()).then(|| fs.map(|fs| view! { <small>{(s.fs_long)(fs)}</small> })).flatten()}
                    </button>
                    <button class="st-check" type="button" role="checkbox" aria-checked=move || if leave.get() { "true" } else { "false" } on:click=move |_| leave.update(|leave| *leave = !*leave)>
                        <span class="st-box" class:with=move || leave.get() aria-hidden="true"><Icon name="check"/></span>
                        <span><b>{s.as_leave}</b><span class="st-sub">{s.leave_note}</span></span>
                    </button>
                </div>
            </article>
        })
    }
}

/// Where the card stands among the semesters, on a phone.
#[component]
fn Dots(ctx: StudyCtx) -> impl IntoView {
    let dots = Memo::new(move |_| {
        let focused = ctx.focused();
        ctx.with_ready(|ready| {
            let mut dots: Vec<(bool, bool)> = ready.study.semesters.iter().map(|semester| (Some(semester.key) == focused, semester.when == When::Now)).collect();
            let end = ready.study.semesters.last().and_then(|last| last.key.plus(1));
            dots.push((end.is_some() && end == focused, false));
            dots
        })
        .unwrap_or_default()
    });
    view! {
        <div class="st-dots st-phone" aria-hidden="true">
            {move || dots.get().into_iter().map(|(here, now)| view! { <span class:here=here class:now=now></span> }).collect_view()}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A drag over the boxes selects what it passes, from where it began, and keeps the rest as it
    /// was; begun on a selected row it lets go instead; back over a row, that row is as before.
    #[test]
    fn a_drag_over_the_boxes_selects_what_it_passes() {
        let order: Vec<String> = ["a", "b", "c", "d", "e"].map(String::from).to_vec();
        let set = |keys: &[&str]| keys.iter().map(|key| key.to_string()).collect::<BTreeSet<_>>();
        let down = Paint { start: 1, select: true, base: set(&["e"]) };
        assert_eq!(painted(&order, &down, 3), set(&["b", "c", "d", "e"]));
        assert_eq!(painted(&order, &down, 0), set(&["a", "b", "e"]), "upwards");
        assert_eq!(painted(&order, &down, 1), set(&["b", "e"]), "back where it began");
        let off = Paint { start: 2, select: false, base: set(&["a", "b", "c", "d"]) };
        assert_eq!(painted(&order, &off, 3), set(&["a", "b"]));
        assert_eq!(painted(&order, &down, 9), set(&["e"]), "beyond the rows: nothing");
    }
}
