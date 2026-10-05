//! „Mein Studium" on a phone (owner, 2026-10-05: „Mach das mal so, dass man oben die progressbar
//! hat und darunter gleich die legende dazu. Danach kommt der Studiengang und Dann kommen die links
//! zu regelstudienplan und Fachbereichen. Danach kommt dann eien box studium planen, mit einer
//! preview vom Studium … und wenn man da rauf klickt oder nach links swiped bekommt man den plan
//! vom semester … Unten sind dann die rest links aus anpassen."): two pages, the overview and the
//! semesters behind it (`StudyUrl::plan`, so that Back and the tab bar lead out again).
//!
//! The overview, top to bottom: the credits (what is passed of what the plan asks, the bar of the
//! areas, and under it what its parts are, each with its credits; the bar and „Bereiche" open the
//! areas), the program's card, the ways to its Regelstudienplan and its areas, the box „Studium
//! planen" with a column for each semester (what is passed and planned there, against what the
//! plan puts into its Fachsemester), and at the end the way to all programs and where all of it
//! lives. Everything the sheet „Anpassen" had is there, and the sheet is gone.
//!
//! The box leads to the semesters: a tap on it to the current one, a tap on a column to that one,
//! and a finger drawn to the left takes the box along and, let go far enough, goes there too. The
//! semesters slide in from the right, the overview back in from the left (`Enter`); their page has
//! a way back over the cards, which goes back through the history where the overview is what came
//! before, and the marks of the rows under them. It is no „Zurück" of `enhance.js`
//! (`data-action="back"`), whose Esc would leave the semesters while it closes a menu or a dialog
//! of theirs.

use folia_calendar::semester::SemesterKey;
use folia_plans::study::When;
use folia_routes::url::{self, StudyUrl};
use leptos::ev::{MouseEvent, PointerEvent, TouchEvent};
use leptos::html::Div;
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use folia_design::ui::Icon;
use folia_shell::pending::Pending;

use super::overview::{Bar, Info};
use super::pager::{band, Finger, Pager, FLICK, FLICK_MS, NO_TAP};
use super::side::{AllPrograms, LegendMarks, MineCard, ProgramWays, StorageHint};
use super::{dom, n, Dialog, Ready, StudyCtx};
use crate::i18n::{self, Texts};

/// How a page of the two comes: from the right (the semesters, after the overview), from the left
/// (the overview, after them), or as it is (the first page shown).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Enter {
    Still,
    FromRight,
    FromLeft,
}

impl Enter {
    fn class(self) -> &'static str {
        match self {
            Enter::Still => "",
            Enter::FromRight => "st-enter-right",
            Enter::FromLeft => "st-enter-left",
        }
    }
}

/// The address of the semesters' page.
fn plan_path() -> String {
    StudyUrl::default().with_plan(true).path()
}

/// The overview or the semesters, as the address says.
#[component]
pub(super) fn Phone(ctx: StudyCtx) -> impl IntoView {
    let plan = Memo::new(move |_| ctx.url.with(|url| url.plan));
    // What was shown before: the other page slides in from its side.
    let before = StoredValue::new(None::<bool>);
    move || {
        let now = plan.get();
        let came = before.get_value();
        before.set_value(Some(now));
        let enter = match came {
            Some(false) if now => Enter::FromRight,
            Some(true) if !now => Enter::FromLeft,
            _ => Enter::Still,
        };
        if now {
            // The overview was the step before: „Übersicht" goes back to it.
            if came == Some(false) {
                ctx.from_overview.set_value(true);
            }
            view! { <PlanPage ctx enter/> }.into_any()
        } else {
            ctx.from_overview.set_value(false);
            view! { <Landing ctx enter/> }.into_any()
        }
    }
}

#[component]
fn Landing(ctx: StudyCtx, enter: Enter) -> impl IntoView {
    let t = i18n::t();
    view! {
        <div class=format!("st-phone-page {}", enter.class())>
            <Progress ctx/>
            <section class="panel st-mine-panel" aria-label=t.study.program>
                <MineCard ctx/>
                <nav class="st-ways" aria-label=t.study.ways>
                    <ProgramWays ctx/>
                </nav>
            </section>
            <PlanBox ctx/>
            <div class="st-rest">
                <nav class="st-ways"><AllPrograms/></nav>
                <StorageHint/>
            </div>
        </div>
    }
}

/// The credits: what is passed of what the plan asks, the bar of the areas, and under it what its
/// parts are, each with its credits („46 bestanden"); the bar and „Bereiche" open the areas.
#[component]
fn Progress(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let info = Memo::new(move |_| ctx.with_ready(|ready| Info::of(ready, t)));
    // „46 LP" as the legend says it: „46".
    let short = |text: &str| text.trim_end_matches("\u{a0}LP").trim_end_matches("\u{a0}CP").to_string();
    move || {
        info.get().map(|info| {
            let over = info.has_over();
            view! {
                <section class="panel st-over st-progress" aria-label=s.overview>
                    <div class="st-progress-top">
                        <p class="st-headline">{info.headline.clone()}</p>
                        <button class="st-link" type="button" on:click=move |_| ctx.open(Dialog::Areas)>{s.areas}<Icon name="chevron-right"/></button>
                    </div>
                    <button class="st-bar-open" type="button" aria-label=s.areas_title on:click=move |_| ctx.open(Dialog::Areas)>
                        <Bar segments=info.segments()/>
                    </button>
                    <ul class="st-legend-n" aria-label=s.legend_bar>
                        <li><span class="st-key passed"></span><b>{short(&info.passed)}</b>" "{s.passed_short}</li>
                        <li><span class="st-key planned"></span><b>{short(&info.planned)}</b>" "{s.planned_short}</li>
                        <li><span class="st-key open"></span><b>{short(&info.open)}</b>" "{s.open_short}</li>
                        {over.then(|| view! { <li><span class="st-key over"></span>{s.over_need}</li> })}
                    </ul>
                </section>
            }
        })
    }
}

/// A semester as the box draws it.
#[derive(Clone, Debug, PartialEq)]
struct Column {
    key: SemesterKey,
    /// Under it: „3", „Urlaub".
    label: String,
    now: bool,
    /// Its parts as shares of the highest column: passed, planned (not passed, not failed), and
    /// what the plan puts into its Fachsemester.
    passed: f64,
    planned: f64,
    plan: f64,
    /// All of it in words, for screen readers: „SoSe 2027 · 4. FS · 12 von 30 LP".
    title: String,
}

/// What the box shows: a column for each semester, and the current one in words.
#[derive(Clone, Debug, PartialEq)]
struct Preview {
    columns: Vec<Column>,
    /// „Jetzt WiSe 2026/27 · 8 von 32 LP", „Beginnt im WiSe 2026/27".
    line: Option<String>,
}

/// „8 von 32 LP": what a semester holds, passed or planned, of what the plan puts into it.
fn figure(taken: f64, plan: Option<f64>, t: &Texts) -> String {
    match plan {
        Some(plan) => format!("{} {}", n(taken, t), (t.study.of_planned)(&n(plan, t))),
        None => (t.study.credits)(&n(taken, t)),
    }
}

impl Preview {
    fn of(ready: &Ready, t: &Texts) -> Self {
        let s = &t.study;
        let study = &ready.study;
        // Passed and planned: what failed counts for nothing here, as on the bar.
        let parts: Vec<(f64, f64)> = study
            .semesters
            .iter()
            .map(|semester| (semester.passed, semester.items.iter().filter(|item| !item.passed && !item.failed).filter_map(|item| item.credits).sum()))
            .collect();
        let top = study.semesters.iter().zip(&parts).map(|(semester, (passed, planned))| semester.planned.unwrap_or(0.0).max(passed + planned)).fold(1.0, f64::max);
        let columns = study
            .semesters
            .iter()
            .zip(&parts)
            .map(|(semester, (passed, planned))| {
                let label = match semester.fs {
                    Some(fs) => fs.to_string(),
                    None if semester.leave => s.leave_short.to_string(),
                    None => "–".to_string(),
                };
                let now = semester.when == When::Now;
                let title = [Some(semester.key.label(t.locale)), ready.fs_label(semester.key, t), now.then(|| s.now.to_string()), Some(figure(passed + planned, semester.planned, t))]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · ");
                Column { key: semester.key, label, now, passed: passed / top, planned: planned / top, plan: semester.planned.unwrap_or(0.0) / top, title }
            })
            .collect();
        let now = study.semesters.iter().zip(&parts).find(|(semester, _)| semester.when == When::Now);
        let line = match now {
            Some((semester, (passed, planned))) => Some((s.box_now)(&semester.key.label(t.locale), &figure(passed + planned, semester.planned, t))),
            None if study.now < study.start => Some((s.begins_in)(&study.start.label(t.locale))),
            None => None,
        };
        Preview { columns, line }
    }
}

/// What the box does with the finger on it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Rest,
    Drag,
    /// Back to its place.
    Glide,
}

/// „Studium planen": the way to the semesters, with a column for each of them. A tap leads to the
/// current one, a tap on a column to that one, and a finger drawn to the left takes the box along
/// and, let go past a fifth of it or flicked, leads to the current one as well.
#[component]
fn PlanBox(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let preview = Memo::new(move |_| ctx.with_ready(|ready| Preview::of(ready, t)));
    let href = t.path(&plan_path());
    let going = Pending::expect();
    let node = NodeRef::<Div>::new();
    let finger = StoredValue::new(None::<Finger>);
    let swiped = StoredValue::new(f64::NEG_INFINITY);
    let phase = RwSignal::new(Phase::Rest);
    let pull = RwSignal::new(0.0_f64);

    let down = move |ev: PointerEvent| {
        finger.set_value(None);
        swiped.set_value(f64::NEG_INFINITY);
        if ev.is_primary() && ev.button() == 0 && phase.get_untracked() == Phase::Rest {
            finger.set_value(Some(Finger::new(&ev)));
        }
    };
    let glide_back = move || {
        phase.set(Phase::Glide);
        pull.set(0.0);
        set_timeout(
            move || {
                phase.try_set(Phase::Rest);
            },
            std::time::Duration::from_millis(320),
        );
    };
    let moving = move |ev: PointerEvent| {
        let Some(mut now) = finger.get_value() else { return };
        if ev.pointer_id() != now.id {
            return;
        }
        if !now.taken {
            match now.sideways(&ev) {
                None => return,
                Some(false) => {
                    finger.set_value(None);
                    return;
                }
                Some(true) => {
                    let Some(element) = node.get_untracked() else { return };
                    let _ = element.set_pointer_capture(now.id);
                    now.taken = true;
                    now.width = f64::from(element.offset_width());
                    phase.set(Phase::Drag);
                }
            }
        }
        now.follow(f64::from(ev.client_x()), ev.time_stamp());
        // To the left with the finger, as far as the box is wide; to the right nothing comes.
        pull.set(if now.pull < 0.0 { now.pull.max(-now.width) } else { band(now.pull, 24.0) });
        finger.set_value(Some(now));
    };
    let let_go = move |lifted: Option<(f64, f64)>| {
        let Some(mut now) = finger.get_value() else { return };
        finger.set_value(None);
        if !now.taken {
            return;
        }
        if let Some(element) = node.get_untracked() {
            let _ = element.release_pointer_capture(now.id);
        }
        let Some((x, at)) = lifted else { return glide_back() };
        swiped.set_value(at + NO_TAP);
        now.follow(x, at);
        let flicked = at - now.at < FLICK_MS && now.speed < -FLICK && now.pull <= -24.0;
        if now.pull <= -now.width / 5.0 || flicked {
            ctx.focus.set(None);
            if let Some(going) = going {
                going.go(&plan_path(), NavigateOptions::default());
            }
        } else {
            glide_back();
        }
    };
    let up = move |ev: PointerEvent| {
        if finger.get_value().is_some_and(|now| now.id == ev.pointer_id()) {
            let_go(Some((f64::from(ev.client_x()), ev.time_stamp())));
        }
    };
    let cancel = move |ev: PointerEvent| {
        if finger.get_value().is_some_and(|now| now.id == ev.pointer_id()) {
            let_go(None);
        }
    };
    // A swipe is no tap; its moves are the box's alone (`pager.rs`).
    let click = move |ev: MouseEvent| {
        if ev.time_stamp() < swiped.get_value() {
            ev.prevent_default();
            ev.stop_propagation();
        }
    };
    let touch_move = move |ev: TouchEvent| {
        if phase.get_untracked() == Phase::Drag && ev.cancelable() {
            ev.prevent_default();
        }
    };
    let phase_attr = move || match phase.get() {
        Phase::Rest => None,
        Phase::Drag => Some("drag"),
        Phase::Glide => Some("glide"),
    };
    let head_href = href.clone();
    view! {
        <div
            class="st-planbox-wrap"
            node_ref=node
            data-phase=phase_attr
            style=move || (phase.get() != Phase::Rest).then(|| format!("--pull:{:.1}px", pull.get()))
            on:pointerdown=down
            on:pointermove=moving
            on:pointerup=up
            on:pointercancel=cancel
            on:touchmove=touch_move
            on:click:capture=click
        >
            <div class="st-planbox-ground" aria-hidden="true">
                <span>{s.semesters}</span>
                <Icon name="chevron-right"/>
            </div>
            <section class="panel st-planbox" aria-labelledby="st-planbox-title">
                <a class="st-planbox-head" id="st-planbox-title" href=head_href on:click=move |_| ctx.focus.set(None)>
                    <span>{s.plan_box}</span>
                    <Icon name="chevron-right"/>
                </a>
                {move || preview.get().map(|preview| {
                    let href = href.clone();
                    view! {
                        {preview.line.map(|line| view! { <p class="st-planbox-line">{line}</p> })}
                        <ol class="st-chart" aria-label=s.chart_label>
                            {preview.columns.into_iter().map(|column| {
                                let key = column.key;
                                // Passed under planned, a seam of the box's ground between them.
                                let stacked = column.passed > 0.0 && column.planned > 0.0;
                                let style = format!("--plan:{:.4};--passed:{:.4};--planned:{:.4}{}", column.plan, column.passed, column.planned, if stacked { ";--seam:2px" } else { "" });
                                view! {
                                    <li class="st-col" class:is-now=column.now>
                                        <a href=href.clone() aria-label=column.title on:click=move |_| ctx.focus.set(Some(key))>
                                            <span class="st-col-bar" class:stacked=stacked style=style aria-hidden="true">
                                                <i class="track"></i>
                                                <i class="passed"></i>
                                                <i class="planned"></i>
                                            </span>
                                            <span class="st-col-label" aria-hidden="true">{column.label}</span>
                                        </a>
                                    </li>
                                }
                            }).collect_view()}
                        </ol>
                    }
                })}
            </section>
        </div>
    }
}

/// The semesters: the way back to the overview, the cards, the marks of the rows.
#[component]
fn PlanPage(ctx: StudyCtx, enter: Enter) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    // Back through the history where the overview came before, else to it.
    let back = move |ev: MouseEvent| {
        if ctx.from_overview.get_value() && ev.button() == 0 && !(ev.ctrl_key() || ev.meta_key() || ev.shift_key() || ev.alt_key()) {
            ev.prevent_default();
            dom::history_back();
        }
    };
    view! {
        <div class=format!("st-phone-page st-plan-page {}", enter.class())>
            <nav class="st-plan-top">
                <a class="st-back" href=t.path(url::STUDY) on:click=back>
                    <Icon name="chevron-left"/>{s.overview}
                </a>
            </nav>
            <h1 class="visually-hidden">{s.plan_box}</h1>
            <Pager ctx/>
            <div class="st-legend st-plan-legend">
                <LegendMarks/>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_semesters_have_an_address_of_their_own() {
        assert_eq!(plan_path(), "/study?plan=1");
        assert_eq!((Enter::FromRight.class(), Enter::FromLeft.class(), Enter::Still.class()), ("st-enter-right", "st-enter-left", ""));
    }
}
