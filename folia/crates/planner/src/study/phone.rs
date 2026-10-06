//! „Mein Studium" on a phone (owner, 2026-10-05: „Mach das mal so, dass man oben die progressbar
//! hat und darunter gleich die legende dazu. Danach kommt der Studiengang und Dann kommen die links
//! zu regelstudienplan und Fachbereichen. Danach kommt dann eien box studium planen, mit einer
//! preview vom Studium … Unten sind dann die rest links aus anpassen."): the overview, a page of the
//! row the semesters are pages of (`pager.rs`; owner, 2026-10-06: „die landing page für mein
//! Studium mit auf den pager, so dass man gleich durch swipen kann"), between the semesters that
//! are over and the current one. A finger anywhere on it draws the row to the semester beside it.
//!
//! The overview, top to bottom: the credits (what is passed of what the plan asks, the bar of the
//! areas, and under it what its parts are, each with its credits; a tap anywhere on them opens the
//! areas), the program's card, the ways to its Regelstudienplan and its areas, the box „Studium
//! planen" with a column for each semester (what is passed and planned there, against what the
//! plan puts into its Fachsemester; a tap on the box leads to the current semester, a tap on a
//! column to that one), and at the end the way to all programs and where all of it lives.
//!
//! On a semester, on either side of the overview, the way back to it is at the head of the screen,
//! a box left of the search (owner, 2026-10-05: „wenn man in der semester ansicht ist, soll es oben
//! links neben der search bar im gleichen style eine quadratische box sein mit einem zurück pfeil";
//! 2026-10-06: „Mach bei beiden Richtungen weiterhin den pfeil oben hin um zurück zu kommen";
//! `chrome::TopBack`), from the moment the row leaves the overview. It and the tab of „Studium"
//! tapped again bring the row back at once, wherever it is or goes. It is no „Zurück" of `enhance.js` (`data-action="back"`), whose Esc would
//! leave the semesters while it closes a menu or a dialog of theirs. Over the tab bar the dots of
//! the row stay (`Dock`).

use folia_calendar::semester::SemesterKey;
use folia_plans::study::When;
use folia_routes::url;
use leptos::prelude::*;

use folia_design::ui::Icon;
use folia_shell::chrome::{Back, TopBack};
use folia_shell::tabs::{Area, TabAgain};

use super::overview::{Bar, Info};
use super::pager::{plan_path, Dots, Pager, Pages};
use super::side::{AllPrograms, MineCard, ProgramWays, StorageHint};
use super::{n, Dialog, Ready, StudyCtx};
use crate::i18n::{self, Texts};

/// The row of pages: the semesters that are over, the overview, the current semester and the ones
/// to come.
#[component]
pub(super) fn Phone(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let pages = Pages::new(ctx);
    provide_context(pages);
    // Where a semester comes after the overview in the history, the overview is the step before it.
    Effect::new(move |before: Option<bool>| {
        let plan = ctx.url.with(|url| url.plan);
        if !plan {
            ctx.from_overview.set(false);
        } else if before == Some(false) {
            ctx.from_overview.set(true);
        }
        plan
    });
    // The way back at the head, as soon as the row leaves the overview and while it is anywhere
    // else (owner, 2026-10-06: „der sollte eigentlich sofort eingeblendet werden, wenn man weg
    // swiped, von der main page").
    let back = TopBack::expect();
    Effect::new(move |_| {
        let away = pages.away();
        let history = ctx.from_overview.get();
        if let Some(back) = back {
            back.set(away.then(|| Back { href: url::STUDY.to_string(), label: s.overview, history }));
        }
    });
    // The tab of „Studium" tapped again, or the way back pressed: back to the overview at once,
    // wherever the row is or goes (owner, 2026-10-06: „Wenn man den anklickt muss es sofort wieder
    // an die standard position gehen"). Where the address says a semester, the link (or Back
    // through the history) changes it, and the row follows the address; where it says the overview
    // still (the row on its way from it, or come to a semester a moment ago), the link leads where
    // the app is, and the row goes back here.
    let again = TabAgain::expect();
    Effect::new(move |before: Option<(u32, u32)>| {
        let now = (again.map_or(0, |again| again.count(Area::Programs)), back.map_or(0, TopBack::presses));
        if before.is_some_and(|before| before != now) && !ctx.url.with_untracked(|url| url.plan) {
            pages.home();
        }
        now
    });
    on_cleanup(move || {
        if let Some(back) = back {
            back.set(None);
        }
    });
    view! {
        <h1 class="visually-hidden">{t.study.title}</h1>
        <Pager ctx pages/>
        <Dock ctx pages/>
    }
}

/// The overview: the credits, the program and its ways, „Studium planen", the rest.
#[component]
pub(super) fn Overview(ctx: StudyCtx) -> impl IntoView {
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
/// parts are, each with its credits („46 bestanden"). A tap anywhere on them opens the areas
/// (owner, 2026-10-06: „die progress bar komplett zu einer clickfläche werden, um das menu für den
/// Progress anzuzeigen"): „Bereiche ›" reaches over the whole box (app.css), one way for a finger
/// and one for the keys and a screen reader.
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
                        <button class="st-link st-progress-open" type="button" aria-haspopup="dialog" on:click=move |_| ctx.open(Dialog::Areas)>{s.areas}<Icon name="chevron-right"/></button>
                    </div>
                    <Bar segments=info.segments()/>
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
/// current one, a tap on a column to that one; the row of pages slides there (`pager.rs`).
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
/// die swiping dots sollten fest über der nav bar sein"): where the page in view stands in the row.
/// No box and no legend (owner, the same day, after a look: „Mach mal die Legende weg und die
/// Punkte nicht in eine box, sondern einfach nur den grauen Hintergrund mit blur zum content"): the
/// dots on the page's ground, frosted over what passes under it as the bar at the top is (app.css);
/// the ground at the end of the page comes over it.
#[component]
fn Dock(ctx: StudyCtx, pages: Pages) -> impl IntoView {
    view! {
        <div class="st-dock">
            <Dots ctx pages/>
        </div>
    }
}
