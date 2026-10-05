//! „Mein Studium" on a phone (owner, 2026-10-05: „Mach das mal so, dass man oben die progressbar
//! hat und darunter gleich die legende dazu. Danach kommt der Studiengang und Dann kommen die links
//! zu regelstudienplan und Fachbereichen. Danach kommt dann eien box studium planen, mit einer
//! preview vom Studium … und wenn man da rauf klickt oder nach links swiped bekommt man den plan
//! vom semester … Unten sind dann die rest links aus anpassen."): two pages, the overview and the
//! semesters after it (`StudyUrl::plan`, so that Back and the tab bar lead out again).
//!
//! The two lie side by side in a row the browser scrolls under a finger, as the semesters do among
//! themselves (owner, the same day: „mach mal die komplette Studiengangsseite in die pages mit
//! rein, aktuell muss man ja in einem bereich nach links sliden aber ich will, dass man die
//! komplette page nach links sliden kann"): a finger anywhere on the overview draws it to the left,
//! and the semesters come in at the one their row was left at (at first the current one); on the
//! first semester a finger to the right brings the overview back. The page coming in shows from its
//! top (`dom::lift`). Where the row comes to rest the address follows: the semesters a step of their
//! own after the overview, the overview the step before them again (back through the history where
//! it came before). Back, the links of the overview and the way back at the head move the row in
//! turn.
//!
//! The overview, top to bottom: the credits (what is passed of what the plan asks, the bar of the
//! areas, and under it what its parts are, each with its credits; the bar and „Bereiche" open the
//! areas), the program's card, the ways to its Regelstudienplan and its areas, the box „Studium
//! planen" with a column for each semester (what is passed and planned there, against what the
//! plan puts into its Fachsemester; a tap on the box leads to the current semester, a tap on a
//! column to that one), and at the end the way to all programs and where all of it lives.
//!
//! The semesters (`pager.rs`) have the way back at the head of the screen, a box left of the search
//! (owner, the same day: „wenn man in der semester ansicht ist, soll es oben links neben der search
//! bar im gleichen style eine quadratische box sein mit einem zurück pfeil"; `chrome::TopBack`). It
//! is no „Zurück" of `enhance.js` (`data-action="back"`), whose Esc would leave the semesters while
//! it closes a menu or a dialog of theirs. Over the tab bar the dots stay (`Dock`).

use folia_calendar::semester::SemesterKey;
use folia_plans::study::When;
use folia_routes::url::{self, StudyUrl};
use leptos::html::Div;
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use folia_design::ui::Icon;
use folia_shell::chrome::{Back, TopBack};
use folia_shell::pending::Pending;

use super::overview::{Bar, Info};
use super::pager::{card_at, Cards, Dots, Pager, QUIET};
use super::side::{AllPrograms, MineCard, ProgramWays, StorageHint};
use super::{dom, n, Dialog, Ready, StudyCtx};
use crate::i18n::{self, Texts};

/// The pages of the row: the overview, then the semesters.
const OVERVIEW: usize = 0;
const SEMESTERS: usize = 1;

/// The address of the semesters' page.
fn plan_path() -> String {
    StudyUrl::default().with_plan(true).path()
}

/// The row of the two pages: the page in view and the one the row rests at.
#[derive(Clone, Copy)]
struct Pages {
    ctx: StudyCtx,
    node: NodeRef<Div>,
    /// The page in the middle of the screen as the row moves: the dots show while it is the
    /// semesters.
    shown: RwSignal<usize>,
    /// The page the row rests at: it alone can be used, the row is as tall as it, and the address
    /// says it.
    here: RwSignal<usize>,
    /// The row has been placed at the page of the address (the first time at once).
    placed: StoredValue<bool>,
    quiet: StoredValue<Option<TimeoutHandle>>,
    /// The page it does not rest at is drawn from its top (`dom::lift`).
    lifted: StoredValue<bool>,
    going: Option<Pending>,
}

impl Pages {
    fn new(ctx: StudyCtx) -> Self {
        let start = if ctx.url.with_untracked(|url| url.plan) { SEMESTERS } else { OVERVIEW };
        Pages {
            ctx,
            node: NodeRef::new(),
            shown: RwSignal::new(start),
            here: RwSignal::new(start),
            placed: StoredValue::new(false),
            quiet: StoredValue::new(None),
            lifted: StoredValue::new(false),
            going: Pending::expect(),
        }
    }

    /// Where the row is: the page in the middle, whether it rests there.
    fn at(self) -> Option<(usize, bool)> {
        let row = self.node.get_untracked()?;
        let (x, step) = dom::scrolled_cards(&row)?;
        Some(card_at(x, step, 2))
    }

    /// The page coming in is drawn from its top (a finger on the row, the address moving it).
    fn lift(self) {
        if let Some(row) = self.node.get_untracked() {
            dom::lift(&row);
            self.lifted.set_value(true);
        }
    }

    /// The row moved: the page in the middle, and the rest once nothing moves for a while.
    fn moved(self) {
        if !self.lifted.get_value() {
            self.lift();
        }
        if let Some((index, _)) = self.at() {
            if index != self.shown.get_untracked() {
                self.shown.set(index);
            }
        }
        if let Some(handle) = self.quiet.get_value() {
            handle.clear();
        }
        self.quiet.set_value(set_timeout_with_handle(move || self.settle(), QUIET).ok());
    }

    /// The row rests: at the other page, the window shows it from its top, and the address
    /// follows.
    fn settle(self) {
        if let Some(handle) = self.quiet.get_value() {
            handle.clear();
            self.quiet.set_value(None);
        }
        let Some((index, true)) = self.at() else { return };
        if index != self.shown.get_untracked() {
            self.shown.set(index);
        }
        let moved = index != self.here.get_untracked();
        if let Some(row) = self.node.get_untracked() {
            dom::land(&row, moved);
            self.lifted.set_value(false);
        }
        if moved {
            self.here.set(index);
            self.follow(index);
        }
    }

    /// The address follows the page the row came to rest at: the semesters a step after the
    /// overview, the overview back through the history where it was the step before them.
    fn follow(self, index: usize) {
        let plan = index == SEMESTERS;
        if self.ctx.url.with_untracked(|url| url.plan) == plan {
            return;
        }
        if !plan && self.ctx.from_overview.get_untracked() {
            dom::history_back();
            return;
        }
        let to = if plan { plan_path() } else { url::STUDY.to_string() };
        if let Some(going) = self.going {
            going.go(&to, NavigateOptions { scroll: false, ..Default::default() });
        }
    }

    /// The row at the page the address says: at once where it opens, smoothly where the address
    /// changes otherwise (Back, a link of the overview, the way back at the head). A row not laid
    /// out yet is placed in a frame to come (`tries` more).
    fn place(self, index: usize, tries: u8) {
        let Some(row) = self.node.get_untracked() else { return };
        let first = !self.placed.get_value();
        if !first && index == self.here.get_untracked() {
            return;
        }
        let Some((_, step)) = dom::scrolled_cards(&row) else {
            if tries > 0 {
                request_animation_frame(move || self.place(index, tries - 1));
            }
            return;
        };
        if !first {
            self.lift();
        }
        dom::scroll_row_to(&row, index as f64 * step, !first);
        if first {
            self.placed.set_value(true);
            self.shown.set(index);
            self.here.set(index);
        }
    }
}

/// The overview and the semesters, side by side.
#[component]
pub(super) fn Phone(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let pages = Pages::new(ctx);
    // The semesters are there once the row rests at them: the pages further out are drawn after
    // the swipe that brought them, not under it.
    let cards = Cards::new(ctx, Signal::derive(move || pages.here.get() == SEMESTERS));
    provide_context(cards);
    // The row follows the address; where the semesters come after the overview, the overview is
    // the step before them in the history.
    Effect::new(move |before: Option<bool>| {
        let plan = ctx.url.with(|url| url.plan);
        if !plan {
            ctx.from_overview.set(false);
        } else if before == Some(false) {
            ctx.from_overview.set(true);
        }
        if pages.node.get().is_some() {
            pages.place(if plan { SEMESTERS } else { OVERVIEW }, 10);
        }
        plan
    });
    // A finger on the row: the page coming in is drawn from its top before it moves.
    Effect::new(move |_| {
        if let Some(row) = pages.node.get() {
            dom::on_finger(&row, move |down| {
                if down {
                    pages.lift();
                }
            });
        }
    });
    // The way back at the head, while the row rests at the semesters.
    let back = TopBack::expect();
    Effect::new(move |_| {
        let semesters = pages.here.get() == SEMESTERS;
        let history = ctx.from_overview.get();
        if let Some(back) = back {
            back.set(semesters.then(|| Back { href: url::STUDY.to_string(), label: s.overview, history }));
        }
    });
    on_cleanup(move || {
        if let Some(back) = back {
            back.set(None);
        }
    });
    // The page the row rests at is the one to be used; the other is out of reach once the row has
    // come to rest (not under the finger: a whole page let go of costs its frame).
    let page = move |index: usize| {
        (
            move || (pages.here.get() == index).then_some(""),
            move || pages.here.get() != index,
            move || (pages.here.get() != index).then_some("true"),
        )
    };
    let (overview_here, overview_inert, overview_hidden) = page(OVERVIEW);
    let (plan_here, plan_inert, plan_hidden) = page(SEMESTERS);
    view! {
        <div class="st-pages" node_ref=pages.node on:scroll=move |_| pages.moved() on:scrollend=move |_| pages.settle()>
            <div class="st-page" data-here=overview_here inert=overview_inert aria-hidden=overview_hidden>
                <Overview ctx/>
            </div>
            <div class="st-page st-plan-page" data-here=plan_here inert=plan_inert aria-hidden=plan_hidden>
                <h1 class="visually-hidden">{s.plan_box}</h1>
                <Pager ctx cards/>
            </div>
        </div>
        <Dock ctx cards on=Signal::derive(move || pages.shown.get() == SEMESTERS)/>
    }
}

/// The overview: the credits, the program and its ways, „Studium planen", the rest.
#[component]
fn Overview(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    view! {
        <div class="st-phone-page">
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

/// „Studium planen": the way to the semesters, with a column for each of them. A tap leads to the
/// current one, a tap on a column to that one; the row of the pages slides there (`Phone`).
#[component]
fn PlanBox(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let preview = Memo::new(move |_| ctx.with_ready(|ready| Preview::of(ready, t)));
    let href = t.path(&plan_path());
    let head_href = href.clone();
    view! {
        <section class="panel st-planbox" aria-labelledby="st-planbox-title">
            <a class="st-planbox-head" id="st-planbox-title" href=head_href data-noscroll="" on:click=move |_| ctx.focus.set(None)>
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
                                    <a href=href.clone() data-noscroll="" aria-label=column.title on:click=move |_| ctx.focus.set(Some(key))>
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
    }
}

/// Over the tab bar, where they stay while the page scrolls (owner, 2026-10-05: „Die Legende und
/// die swiping dots sollten fest über der nav bar sein"): where the semester in view stands among
/// the semesters. No box and no legend (owner, the same day, after a look: „Mach mal die Legende
/// weg und die Punkte nicht in eine box, sondern einfach nur den grauen Hintergrund mit blur zum
/// content"): the dots on the page's ground, frosted over what passes under it as the bar at the
/// top is (app.css). There while the semesters are the page in view (`on`); the ground at the end
/// of the page comes over it.
#[component]
fn Dock(ctx: StudyCtx, cards: Cards, on: Signal<bool>) -> impl IntoView {
    view! {
        <div class="st-dock" class:on=move || on.get() aria-hidden=move || (!on.get()).then_some("true")>
            <Dots ctx cards/>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_semesters_have_an_address_of_their_own() {
        assert_eq!(plan_path(), "/study?plan=1");
    }
}
