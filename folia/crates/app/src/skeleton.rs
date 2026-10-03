//! What stands in for a page, a list or a panel while it is being computed (`pending`): its
//! frame, with grey bars where the text will be. The frames use the layout classes of the real
//! ones (`.work`, `.framed`, `.panel`, `.sidebar`, `.page`, `.row` …), so every panel stands where
//! the page will put its own and nothing moves much when the page replaces the skeleton, in
//! either layout. Everything else is a class of its own (`sk-…`), so that nothing that looks for
//! the parts of a page (a count, a chip, a module's page) finds a skeleton instead. Nothing in
//! here is a link or a heading, and all of it is `aria-hidden`: `main` says `aria-busy`.

use leptos::prelude::*;

use crate::pending::{Change, Pending, Shape};

/// A bar where a line of text will be. `class` sets its width and height (app.css, `.sk`).
fn bar(class: &'static str) -> impl IntoView {
    view! { <i class=format!("sk {class}")></i> }
}

fn lines(classes: &'static [&'static str]) -> impl IntoView {
    view! { <div class="sk-lines">{classes.iter().map(|class| bar(class)).collect_view()}</div> }
}

/// The skeleton over the page for another page, or over its column for another view of the same
/// page; placed in `main` next to the routes.
#[component]
pub fn PendingPage() -> impl IntoView {
    let pending = Pending::expect();
    move || {
        let pending = pending?;
        let (columns, shape) = match pending.change()? {
            Change::Page(shape) if pending.waits(Change::Page(shape)) => (false, shape),
            Change::Column(shape) if pending.waits(Change::Column(shape)) => (true, shape),
            _ => return None,
        };
        Some(view! {
            <div class="pending-page" class:columns=columns aria-hidden="true">{frame(shape)}</div>
        })
    }
}

/// What the server writes for a route of the app (the Merkliste, the Stundenplan) in the place of
/// its page (docs/folia/folia-refactor.md §4.2): the page's skeleton until the app runs and takes
/// the page over, and for a browser without JavaScript what the view needs instead.
#[component]
pub fn AppStandin(shape: Shape, title: &'static str, hint: &'static str) -> impl IntoView {
    view! {
        <div class="app-standin" aria-hidden="true">{frame(shape)}</div>
        <noscript>
            <crate::ui::Plain>
                <section class="panel">
                    <crate::ui::EmptyState title hint>
                        <a class="btn secondary" href=crate::i18n::t().path(folia_routes::url::CATALOG)>{crate::i18n::t().common.to_catalog}</a>
                    </crate::ui::EmptyState>
                </section>
            </crate::ui::Plain>
        </noscript>
    }
}

/// The catalog's filter panel on the server's page, which has none (§4.1): its place with its
/// bars, so the list stands where the app's will and nothing moves when the app takes over. A
/// phone has the panel as a sheet, and so nothing in its place.
#[component]
pub fn FiltersStandin() -> impl IntoView {
    view! {
        <div class="panel filters sk-sweep sk-standin" aria-hidden="true">
            <div class="panel-head">{bar("sk-w2 sk-tall")}</div>
            <div class="body">{filter_groups(&[3, 6, 6, 2])}</div>
        </div>
    }
}

/// The groups of a sidebar's filters on the server's page, which has none (§4.1): their bars in
/// their place, chips per group as `groups` says, until the app puts the filters there.
#[component]
pub fn FilterGroupsStandin(groups: &'static [usize]) -> impl IntoView {
    view! { <div class="sk-sweep sk-groups" aria-hidden="true">{filter_groups(groups)}</div> }
}

/// The frame of a page with the bars of its content.
fn frame(shape: Shape) -> AnyView {
    match shape {
        Shape::Catalog => view! {
            <div class="work sk-frame">
                <div class="panel filters sk-sweep">
                    <div class="panel-head">{bar("sk-w2 sk-tall")}</div>
                    <div class="body">{filter_groups(&[3, 6, 6, 2])}</div>
                </div>
                <div class="panel list sk-sweep">
                    <div class="list-head"><div class="count-row">{bar("sk-count")}{bar("sk-w1")}{bar("sk-tools")}</div></div>
                    <div class="rows scroll"><div class="cols label">{bar("sk-w1 sk-tall")}</div>{rows(12)}</div>
                </div>
            </div>
        }
        .into_any(),
        Shape::Bookmarks => framed(
            Side::Filters(&[3, 4]),
            view! {
                <div class="panel sk-sweep sk-fill">
                    <div class="list-head"><div class="count-row">{bar("sk-count")}{bar("sk-w1")}</div></div>
                    <div class="cols label">{bar("sk-w1 sk-tall")}</div>
                    <div class="rows scroll">{rows(10)}</div>
                </div>
            }
            .into_any(),
        ),
        Shape::Module => framed(Side::Toc(8), module().into_any()),
        // The Stundenplan: its head (the semester, the numbers) over the Regelwoche, and its
        // modules in a column beside them where the page is wide enough (`.sk-plan`, as the page
        // lays them out).
        Shape::Studyplan => framed(Side::Filters(&[3, 5, 3]), view! {
            <div class="page-inner">
                <div class="panel sk-sweep sk-block sk-fill sk-plan">
                    <div class="sk-plan-main">
                        {lines(&["sk-w3 sk-big", "sk-w5"])}
                        <i class="sk sk-week sk-week-tall"></i>
                    </div>
                    {lines(&["sk-w2", "sk-w6 sk-tall", "sk-w3", "sk-w5 sk-tall", "sk-w3", "sk-w6 sk-tall", "sk-w3"])}
                </div>
            </div>
        }.into_any()),
        Shape::Program => framed(Side::Toc(6), view! {
            <div class="page-inner">
                <div class="panel sk-sweep sk-head">
                    {bar("sk-w1")}
                    <div class="sk-title-row">{bar("sk-w4 sk-big")}{bar("sk-w3")}</div>
                    {bar("sk-w3")}
                </div>
                <div class="panel sk-sweep sk-block sk-fill">
                    {lines(&["sk-w2 sk-tall", "sk-w5"])}
                    {table(16)}
                </div>
            </div>
        }.into_any()),
        Shape::Programs => framed(Side::Filters(&[4, 3, 1]), view! {
            <div class="page-inner">
                <div class="sk-summary">{bar("sk-count")}{bar("sk-w3")}</div>
                {[8usize, 5, 4].iter().map(|rows| view! {
                    <div class="panel sk-sweep sk-faculty">
                        <div class="sk-faculty-head">{bar("sk-code")}{bar("sk-w4 sk-tall")}</div>
                        <div class="sk-subject sk-subject-head">{bar("sk-w2")}{bar("sk-w2")}{bar("sk-w2")}</div>
                        {(0..*rows).map(|i| view! {
                            <div class="sk-subject">
                                {bar(if i % 2 == 0 { "sk-w4 sk-tall" } else { "sk-w3 sk-tall" })}
                                <span>{(i % 3 != 2).then(|| bar("sk-pill"))}</span>
                                <span>{(i % 3 != 1).then(|| bar("sk-pill"))}</span>
                            </div>
                        }).collect_view()}
                    </div>
                }).collect_view()}
            </div>
        }.into_any()),
        Shape::Home => framed(Side::Toc(4), view! {
            <div class="page-inner">
                <div class="panel sk-sweep sk-home">
                    <div class="sk-block">
                        {bar("sk-pill sk-wide-pill")}
                        {lines(&["sk-w5 sk-big", "sk-w4 sk-big", "sk-w6", "sk-w4"])}
                        <div class="sk-chips">{bar("sk-button")}{bar("sk-button")}</div>
                    </div>
                    <div class="sk-figures">{(0..4).map(|_| view! { <div>{bar("sk-figure")}{bar("sk-w1")}</div> }).collect_view()}</div>
                </div>
                <div class="panel sk-sweep sk-stage"></div>
            </div>
        }.into_any()),
        Shape::Text => framed(Side::Toc(3), view! {
            <div class="panel sk-sweep sk-block">
                {lines(&["sk-w4 sk-big", "sk-w6", "sk-w6", "sk-w5", "sk-w6", "sk-w3"])}
            </div>
        }.into_any()),
    }
}

/// What the sidebar of a page holds: its sections (and actions), or filters.
enum Side {
    Toc(usize),
    Filters(&'static [usize]),
}

/// A page in the frame every page has (`ui::Frame`): the sidebar and the page. What stands beside
/// a page is there only while something is picked, and has a skeleton of its own (`DetailSkeleton`).
fn framed(side: Side, page: AnyView) -> AnyView {
    let sidebar = match side {
        Side::Toc(entries) => view! {
            <div class="sk-lines sk-toc">
                {bar("sk-w2")}
                {(0..entries).map(|i| bar(if i % 2 == 0 { "sk-w5" } else { "sk-w4" })).collect_view()}
            </div>
        }
        .into_any(),
        Side::Filters(groups) => filter_groups(groups).into_any(),
    };
    view! {
        <div class="work framed sk-frame">
            <div class="panel sidebar sk-sweep">
                <div class="panel-head">{bar("sk-w3 sk-tall")}</div>
                <div class="body">{sidebar}</div>
            </div>
            <div class="page">{page}</div>
        </div>
    }
    .into_any()
}

/// Groups of toggles, as the filter panels have them: a label and a few chips.
fn filter_groups(groups: &'static [usize]) -> impl IntoView {
    groups
        .iter()
        .map(|chips| {
            view! {
                <div class="fgroup">
                    {bar("sk-w2")}
                    <div class="sk-chips">{(0..*chips).map(|_| bar("sk-chip")).collect_view()}</div>
                </div>
            }
        })
        .collect_view()
}

/// Skeleton rows as far as a box reaches (the fills of the catalog's virtual list, and a row whose
/// page is not loaded yet): one row of the list's columns (`.row` gives it its grid, so each column
/// lies where the rows have it), as tall as the box, each column painting its bar again every row
/// (app.css, `.sk-cols`). Eight elements however far it reaches, where a row of `rows` has sixteen.
pub fn fill() -> impl IntoView {
    view! {
        <div class="row sk-cols">
            <span class="t"></span>
            <span class="resp"></span>
            <span class="exam"></span>
            <span class="lp"></span>
            <span class="turnus"></span>
            <span class="lang"></span>
            <span class="events"></span>
        </div>
    }
}

/// Skeleton cards of the catalog's list on a phone, one element each: a card as the rows are
/// (rounded, framed, its shadow), with its bars in its background (app.css, `.sk-card`).
pub fn cards(count: usize) -> impl IntoView {
    (0..count).map(|_| view! { <div class="sk-card"></div> }).collect_view()
}

/// Rows in the columns of the catalog's list (`.row` gives them its grid, its height and, on a
/// phone, its card), so that each bar lies where its column will be.
pub fn rows(count: usize) -> impl IntoView {
    (0..count)
        .map(|i| {
            // Titles of different lengths, as the list has them.
            let title = match i % 3 {
                0 => "sk-w5 sk-tall",
                1 => "sk-w6 sk-tall",
                _ => "sk-w4 sk-tall",
            };
            view! {
                <div class="row sk-row">
                    <div class="t">{bar(title)}{bar("sk-w2")}</div>
                    <span class="resp">{bar("sk-w4")}</span>
                    <span class="exam">{bar("sk-w3")}</span>
                    <span class="lp">{bar("sk-w1")}</span>
                    <span class="turnus">{bar("sk-w3")}</span>
                    <span class="lang">{bar("sk-w1")}</span>
                    <span class="events">{bar("sk-w4")}</span>
                </div>
            }
        })
        .collect_view()
}

fn table(count: usize) -> impl IntoView {
    view! {
        <div class="sk-table">
            {(0..count).map(|i| view! {
                <div class="sk-trow">{bar(if i % 2 == 0 { "sk-w5" } else { "sk-w4" })}{bar("sk-w1")}{bar("sk-w1")}</div>
            }).collect_view()}
        </div>
    }
}

/// The head of a module (number, title, subtitle, badges): the same in the preview, beside a
/// program and on its own page.
fn module_head() -> impl IntoView {
    view! {
        {bar("sk-code")}
        {bar("sk-w5 sk-big")}
        {bar("sk-w3")}
        <div class="sk-chips">{bar("sk-badge")}{bar("sk-badge")}{bar("sk-badge")}</div>
    }
}

/// A module's own page: its head, then the text and beside it (with room) the times and facts.
fn module() -> impl IntoView {
    view! {
        <div class="sk-module">
            <div class="panel sk-sweep sk-block">{module_head()}</div>
            // Times and facts first, as on the page: on a phone they come before the text, with
            // room they stand beside it (`.module-grid`).
            <div class="module-grid">
                <aside class="panel sk-sweep sk-block">
                    {lines(&["sk-w2"])}
                    <i class="sk sk-week"></i>
                </aside>
                <div class="panel sk-sweep sk-block">
                    {lines(&["sk-w2", "sk-w6", "sk-w6", "sk-w5", "sk-w6", "sk-w4"])}
                    {lines(&["sk-w2", "sk-w6", "sk-w5", "sk-w6", "sk-w3"])}
                </div>
            </div>
        </div>
    }
}

/// The module's panel beside a list or a program while it is being opened.
#[component]
pub fn DetailSkeleton(
    /// Beside a program's page it has the head of a frame's panel (`aside`); it floats either way.
    #[prop(optional)]
    aside: bool,
    /// A module was open there already: the panel stays where it is instead of sliding in.
    #[prop(optional)]
    calm: bool,
) -> impl IntoView {
    view! {
        <section class="panel detail sk-detail sk-sweep" class:aside=aside class:calm=calm aria-hidden="true">
            <div class="sk-block">
                {module_head()}
                {lines(&["sk-w2"])}
                <i class="sk sk-week"></i>
                <div class="sk-facts">{(0..4).map(|_| bar("sk-fact")).collect_view()}</div>
            </div>
        </section>
    }
}

/// Over the rows of the catalog's list while another filter is being applied: sticks to the top
/// of what is visible of the list, whatever it is scrolled to.
#[component]
pub fn RowsSkeleton() -> impl IntoView {
    view! {
        <div class="rows-pending" aria-hidden="true">
            <div class="rows-pending-fill sk-sweep">{rows(14)}</div>
        </div>
    }
}
