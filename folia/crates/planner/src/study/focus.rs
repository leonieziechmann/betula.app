//! One semester in focus (the mockup's concept A, owner 2026-10-04: „am sichersten … und am besten
//! zwischen Handy und PC"): on a desktop a strip of every semester with what it holds, the
//! semester in focus as a table (the box that ticks it off, the module with its marks, its area,
//! its credits, its menu) and beside it what fits it (`Fits`); on a phone the semester as a card as
//! wide as the page, turned by ‹ › in its head or a swipe, dots under it. After the last semester
//! comes a page that adds one (owner: „Wenn man bis zum Schluss ist, soll eine Page kommen, wo man
//! dann ein neues Semester hinzufügen kann"), as a semester of leave if the student says so.
//!
//! A row is ticked off in place; its menu (`menu.rs`) moves it and takes it out, and on a desktop
//! a row dragged onto a semester of the strip moves there.

use folia_calendar::semester::SemesterKey;
use folia_model::labels::Season;
use folia_plans::study::{self, Item, Pick, Standing, Subject, When};
use folia_plans::variants;
use folia_routes::url::{CatalogUrl, StudyplanUrl};
use leptos::prelude::*;

use folia_design::ui::Icon;

use super::{item_of, module_href, n, now_secs, Dialog, Plans, Ready, SheetToggle, StudyCtx, ViewSwitch};
use crate::i18n::{self, Texts};

#[component]
pub(super) fn Focus(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let focused = Memo::new(move |_| ctx.focused());
    // The page that adds a semester: one after the last.
    let end = Memo::new(move |_| ctx.with_ready(|ready| ready.study.semesters.last().and_then(|last| last.key.plus(1))).flatten());
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

/// The strip of the semesters, with „+" for one more. A row dropped on a semester moves there.
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
                let drop = move |ev: leptos::ev::DragEvent| {
                    ev.prevent_default();
                    over.set(false);
                    if let Some((from, item)) = ctx.drag.get_untracked() {
                        ctx.drag.set(None);
                        move_to(ctx, from, key, &item, t);
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
                        on:dragover=move |ev: leptos::ev::DragEvent| if ctx.drag.with_untracked(|drag| drag.as_ref().is_some_and(|(from, _)| *from != key)) { ev.prevent_default(); over.set(true); }
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

/// Moves `item` (an `Item::key`) of semester `from` to `to`, with a note to take it back.
pub(super) fn move_to(ctx: StudyCtx, from: SemesterKey, to: SemesterKey, key: &str, t: &'static Texts) {
    let Some(item) = ctx.with_ready(|ready| item_of(ready, from, key)).flatten() else { return };
    if from == to {
        return;
    }
    let note = (t.study.moved)(&item.name, &to.label(t.locale));
    ctx.change_noted(Some(note), move |doc, _| {
        study::move_item(doc, from, to, &item);
    }, || {});
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
    /// The last semester added beyond the plan, empty: it can go again.
    removable: bool,
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
        let last = at + 1 == study.semesters.len();
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
            removable: last && semester.items.is_empty() && ready.setup.until == Some(key) && study.regular_end.is_none_or(|end| key > end) && key > study.now,
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

/// The semester in focus: its head with ‹ ›, its rows, and what adds to it.
#[component]
fn SemesterCard(ctx: StudyCtx, semester: SemesterKey) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let head = Memo::new(move |_| {
        let still = plan_still(ctx, semester);
        ctx.with_ready(|ready| Head::of(ready, semester, still, t)).flatten()
    });
    let keys = Memo::new(move |_| head.with(|head| head.as_ref().map(|head| head.keys.clone()).unwrap_or_default()));
    let go = move |to: Option<SemesterKey>| {
        if let Some(to) = to {
            ctx.focus.set(Some(to));
        }
    };
    // A swipe turns the card (on a phone).
    let swipe = RwSignal::new(None::<(i32, i32)>);
    let down = move |ev: leptos::ev::PointerEvent| {
        if ev.pointer_type() == "touch" {
            swipe.set(Some((ev.client_x(), ev.client_y())));
        }
    };
    let up = move |ev: leptos::ev::PointerEvent| {
        if let Some((x, y)) = swipe.get_untracked() {
            let (dx, dy) = (ev.client_x() - x, ev.client_y() - y);
            if dx.abs() > 48 && dx.abs() > dy.abs() * 2 {
                let head = head.get_untracked();
                go(if dx < 0 { head.and_then(|head| head.next) } else { head.and_then(|head| head.previous) });
            }
        }
        swipe.set(None);
    };
    let leave = move |_| {
        let (Some(mine), Some(head)) = (ctx.mine, head.get_untracked()) else { return };
        mine.set_leave(semester, !head.leave);
    };
    let remove = move |_| {
        let Some(mine) = ctx.mine else { return };
        let previous = head.get_untracked().and_then(|head| head.previous);
        let keep = ctx.with_ready(|ready| ready.study.regular_end.max(Some(ready.study.now))).flatten();
        mine.set_leave(semester, false);
        mine.set_until(previous.filter(|previous| Some(*previous) > keep));
        ctx.focus.set(previous);
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
                    <button class="icon-btn st-turn hit" type="button" aria-label=s.next disabled=head.next.is_none() on:click=move |_| go(head.next)><Icon name="chevron-right"/></button>
                </header>
            })}
            <div class="st-cols st-pc" aria-hidden="true"><span></span><span>{s.col_module}</span><span>{s.col_area}</span><span>{s.col_credits}</span><span></span></div>
            <ul class="st-rows">
                <For each=move || keys.get() key=|key| key.clone() children=move |key: String| view! { <ItemRow ctx semester key/> }/>
            </ul>
            {move || keys.with(Vec::is_empty).then(|| {
                let leave = head.with(|head| head.as_ref().is_some_and(|head| head.leave));
                view! { <p class="st-empty">{if leave { s.empty_leave } else { s.empty_semester }}</p> }
            })}
            <footer class="st-sem-foot">
                <button class="btn secondary st-add" type="button" on:click=move |_| ctx.open(Dialog::Add { semester, catalog: false, chosen: Vec::new() })><Icon name="plus"/>{s.add_modules}</button>
                <a class="st-link" href=t.path(&StudyplanUrl { sem: Some(semester.key()), ..Default::default() }.path()) rel="nofollow"><Icon name="calendar-range"/>{s.in_timetable}</a>
                <span class="st-grow"></span>
                {move || head.get().map(|head| view! {
                    <button class="st-check" type="button" role="checkbox" aria-checked=if head.leave { "true" } else { "false" } on:click=leave>
                        <span class="st-box" class:with=head.leave aria-hidden="true">{head.leave.then(|| view! { <Icon name="check"/> })}</span>{s.as_leave}
                    </button>
                    {head.removable.then(|| view! { <button class="st-link danger" type="button" on:click=remove><Icon name="trash-2"/>{s.remove_semester}</button> })}
                })}
                <span class="st-drag-hint st-pc">{s.drag_hint}</span>
            </footer>
            {move || head.with(|head| head.as_ref().and_then(|head| (!head.past).then(|| head.plan_still.clone()).flatten())).map(|names| {
                let fs = head.with(|head| head.as_ref().map(|head| head.sub.clone()).unwrap_or_default());
                view! { <p class="st-still st-phone">{(s.plan_still)(&fs.replace("Fachsemester", "FS"), &names)}</p> }
            })}
        </article>
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

/// One module or row of the plan in a semester.
#[component]
fn ItemRow(ctx: StudyCtx, semester: SemesterKey, key: String) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let row = {
        let key = key.clone();
        Memo::new(move |_| ctx.with_ready(|ready| item_of(ready, semester, &key).map(|item| Row::of(ctx, ready, semester, item, t))).flatten())
    };
    // What a row is stays what it is: its subject and name.
    let Some(first) = row.get_untracked().map(|row| row.item) else { return ().into_any() };
    let passed = Memo::new(move |_| row.with(|row| row.as_ref().is_some_and(|row| row.item.passed)));
    // What the last click said, until the plan has it.
    let said = RwSignal::new(None::<bool>);
    let checked = Memo::new(move |_| said.get().unwrap_or_else(|| passed.get()));
    let toggle = {
        let item = first.clone();
        move |_| {
            let next = !checked.get_untracked();
            said.set(Some(next));
            let item = item.clone();
            ctx.change(
                move |doc, _| {
                    study::set_passed(doc, semester, &item, next);
                },
                move || {
                    said.try_set(None);
                },
            );
        }
    };
    let name = first.name.clone();
    let what = match &first.subject {
        Subject::Module { id } => {
            let id = id.clone();
            let href = {
                let id = id.clone();
                move || module_href(ctx, &id, t)
            };
            let current = move || ctx.url.with(|url| url.open.as_deref() == Some(id.as_str())).then_some("true");
            view! { <a class="st-name" href=href data-noscroll="" aria-current=current>{name.clone()}</a> }.into_any()
        }
        Subject::Row { .. } => view! { <span class="st-name">{name.clone()}</span> }.into_any(),
    };
    let menu_key = key.clone();
    let drag_key = key.clone();
    view! {
        <li
            class="st-row"
            style=move || row.with(|row| row.as_ref().map(|row| format!("--c: {}", row.tone)))
            class:is-passed=move || checked.get()
            class:is-failed=move || row.with(|row| row.as_ref().is_some_and(|row| row.item.failed)) && !checked.get()
            draggable="true"
            on:dragstart=move |ev: leptos::ev::DragEvent| {
                ctx.drag.set(Some((semester, drag_key.clone())));
                #[cfg(feature = "csr")]
                if let Some(data) = ev.data_transfer() {
                    let _ = data.set_data("text/plain", &drag_key);
                    data.set_effect_allowed("move");
                }
                #[cfg(not(feature = "csr"))]
                let _ = ev;
            }
            on:dragend=move |_| ctx.drag.set(None)
        >
            <button
                class="st-tick hit"
                type="button"
                role="checkbox"
                aria-checked=move || if checked.get() { "true" } else { "false" }
                aria-label=(s.tick)(&first.name)
                aria-busy=move || said.get().map(|_| "true")
                on:click=toggle
            >
                <span class="st-box" class:with=move || checked.get()><Icon name="check"/></span>
            </button>
            <div class="st-what">
                {what}
                {move || row.get().map(|row| view! {
                    <span class="st-area st-phone"><span class="st-dot" style=format!("--c: {}", row.tone)></span>{row.area.clone()}</span>
                    {row.chips.into_iter().map(|(class, text, icon)| view! { <span class=format!("st-chip {class}")>{icon.map(|icon| view! { <Icon name=icon/> })}{text}</span> }).collect_view()}
                    {row.find.map(|href| view! { <a class="st-find" href=t.path(&href) rel="nofollow"><Icon name="search"/>{s.choose_module}</a> })}
                    {row.again.map(|to| {
                        let key = row.item.key();
                        view! { <button class="st-link" type="button" on:click=move |_| { ctx.focus.set(Some(to)); ctx.open(Dialog::Add { semester: to, catalog: false, chosen: vec![key.clone()] }); }>{s.plan_it}</button> }
                    })}
                })}
            </div>
            <span class="st-area st-pc">{move || row.get().map(|row| view! { <span class="st-dot" style=format!("--c: {}", row.tone)></span>{row.area} })}</span>
            <span class="st-lp num">{move || row.with(|row| row.as_ref().and_then(|row| row.credits.clone()))}</span>
            <button class="icon-btn st-more hit" type="button" aria-label=(s.more_about)(&first.name) aria-haspopup="dialog" on:click=move |_| ctx.open(Dialog::Item { semester, key: menu_key.clone() })>
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
    warn: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct Fits {
    fs: Option<String>,
    /// The rows of the plan for its Fachsemester, open; whether it has any.
    plan: Vec<Fit>,
    any: bool,
    retakes: Vec<Fit>,
    elsewhere: Vec<Fit>,
}

impl Fits {
    fn of(ctx: StudyCtx, semester: SemesterKey, t: &Texts) -> Option<Self> {
        let s = &t.study;
        ctx.with_input(|input, ready| {
            let study = &ready.study;
            let fs = study.semester(semester).and_then(|semester| semester.fs);
            let line = |credits: Option<f64>, text: Option<&String>, area: Option<usize>| {
                let credits = text.map(|text| folia_plans::plan::credits_in(text, t.locale)).or_else(|| credits.map(|credits| n(credits, t)));
                [credits.map(|credits| (s.credits)(&credits)), Some(ready.area_name(area, t))].into_iter().flatten().collect::<Vec<_>>().join(" · ")
            };
            let warn = |offer: study::Offer| (!offer.offered(semester)).then(|| (s.unoffered)(season_word(semester, t)));
            let rows = fs.map(|fs| study::plan_semester(input, study, fs)).unwrap_or_default();
            let any = !rows.is_empty();
            let plan = rows
                .into_iter()
                .filter(|suggestion| suggestion.standing == Standing::Open)
                .map(|suggestion| Fit { line: line(suggestion.credits, suggestion.credits_text.as_ref(), suggestion.area), warn: warn(suggestion.offer), name: suggestion.name, pick: suggestion.pick })
                .collect();
            let retakes = ready
                .lines
                .iter()
                .filter_map(|l| {
                    let failed = l.failed_in?;
                    let from = ready.fs_label(failed, t).unwrap_or_else(|| failed.short(t.locale));
                    Some(Fit { pick: l.suggestion.pick.clone(), name: l.suggestion.name.clone(), line: format!("{} · {}", line(l.suggestion.credits, l.suggestion.credits_text.as_ref(), l.suggestion.area), (s.from_fs)(&from)), warn: warn(l.suggestion.offer) })
                })
                .collect();
            let mut elsewhere: Vec<&study::Line> = ready.lines.iter().filter(|l| l.failed_in.is_none() && l.plan_fs != fs).collect();
            elsewhere.sort_by_key(|l| (l.plan_fs.map(|plan| (i16::from(plan) - i16::from(fs.unwrap_or(0))).abs()), l.plan_fs));
            let elsewhere = elsewhere
                .into_iter()
                .take(4)
                .map(|l| {
                    let says = l.plan_fs.map(|plan| (s.plan_says)(&(s.fs)(plan)));
                    Fit { pick: l.suggestion.pick.clone(), name: l.suggestion.name.clone(), line: [Some(line(l.suggestion.credits, l.suggestion.credits_text.as_ref(), l.suggestion.area)), says].into_iter().flatten().collect::<Vec<_>>().join(" · "), warn: warn(l.suggestion.offer) }
                })
                .collect();
            Fits { fs: fs.map(s.fs), plan, any, retakes, elsewhere }
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
/// is open elsewhere in the areas; each a click from being planned here. A desktop's.
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
                    {fit.warn.map(|warn| view! { <span class="st-chip warn"><Icon name="triangle-alert"/>{warn}</span> })}
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
                        {if fits.plan.is_empty() {
                            if fits.any {
                                view! { <p class="st-fits-done"><Icon name="check"/>{s.fits_all}</p> }.into_any()
                            } else {
                                view! { <p class="st-sub">{s.fits_nothing}</p> }.into_any()
                            }
                        } else {
                            view! { <ul>{fits.plan.into_iter().map(entry).collect_view()}</ul> }.into_any()
                        }}
                    </div>
                })}
                {(!fits.retakes.is_empty()).then(|| view! {
                    <div class="st-fits-part">
                        <p class="label">{s.retakes}</p>
                        <ul>{fits.retakes.into_iter().map(entry).collect_view()}</ul>
                    </div>
                })}
                {(!fits.elsewhere.is_empty()).then(|| view! {
                    <div class="st-fits-part">
                        <p class="label">{s.open_areas}</p>
                        <ul>{fits.elsewhere.into_iter().map(entry).collect_view()}</ul>
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
