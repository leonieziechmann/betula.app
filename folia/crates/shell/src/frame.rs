//! The frame of a page: the sidebar, the page, the panel on the right and the ground after them
//! (`Frame`), a page without a sidebar (`Plain`), and the way back up to an area's list
//! (`BackLink`).

use leptos::prelude::*;

use leptos_meta::Title;

use folia_data::DataError;
use crate::i18n;
use folia_design::ui::{Icon, Shortcut};

/// The frame of a page (owner decision 2026-09-20: a basic element of the layout, R17): a sidebar
/// as wide as the filter panel of the catalog, with the same handle and the same remembered
/// width, and the page next to it. Going from one area to another, nothing jumps. The sidebar
/// holds what belongs to the page as a whole: its filters, its sections, its actions.
/// The catalog is the one page that builds this frame itself (its sidebar is the filter form); the
/// start page is the one without it (owner, 2026-09-28: its sidebar confused a first visit).
///
/// A page can put a panel on the right as well (`aside`): what belongs to what the visitor
/// picked. It is the catalog's module preview in all but its content — as wide, with the same
/// handle and remembered width, floating over the page docked to its right edge, and there only
/// while something is picked (`aside_picked`). A column of its own left a 13-inch screen too
/// little room for the page (owner, 2026-09-23).
///
/// On the desktop the frame is one scroll area (`#page-scroll`, app.css „one scroll area"): the
/// page and the ground after it scroll as one; the sidebar and the panel on the right are pinned.
#[component]
pub fn Frame(
    /// Heading of the sidebar.
    #[prop(into)] title: String,
    /// What else the head of the sidebar shows: a code, a reset link.
    #[prop(optional, into)] head: ViewFn,
    #[prop(into)] sidebar: ViewFn,
    /// On a phone the sidebar comes before the page (views) instead of after it (actions).
    #[prop(optional)] sidebar_first: bool,
    /// On a phone the sidebar is a sheet that a „Filter" button of the page opens
    /// (`data-action="sheet-open"`), like the filter panel of the catalog.
    #[prop(optional)] sheet: bool,
    /// The panel on the right, with its own handle. It brings its own box
    /// (`<section class="panel detail aside">`), so a module preview can be used as it is.
    #[prop(optional, into)] aside: Option<ViewFn>,
    /// The visitor picked something (a module, an area): the panel on the right is there only
    /// then. On a phone, where nothing stands beside a page, it is the page: the page and the
    /// sidebar step back until it is closed. (The browser app renders such a pick as a page of
    /// its own; this is for the same HTML without it.)
    #[prop(optional, into)] aside_picked: Signal<bool>,
    children: Children,
) -> impl IntoView {
    let t = i18n::t();
    let label = title.clone();
    let has_aside = aside.is_some();
    view! {
        // One scroll area with the ground at its end (app.css „one scroll area"): the sidebar and
        // the panel on the right are pinned beside the page.
        <div class="work framed flowing" id="page-scroll" class:sidebar-first=sidebar_first class:with-aside=has_aside class:aside-picked=move || has_aside && aside_picked.get()>
            <aside class="panel sidebar" class:sheet=sheet id="sidebar" aria-label=label>
                <div class="panel-head">
                    <h2>{title}</h2>
                    {head.run()}
                    {sheet.then(|| view! { <a class="icon-btn sheet-close" href="#" data-action="sheet-close" aria-label=t.common.close><Icon name="x"/></a> })}
                </div>
                <div class="body scroll" data-keep-scroll="sidebar">{sidebar.run()}</div>
            </aside>
            <div class="resizer between js-only" data-action="resize-filters" role="separator" aria-orientation="vertical" aria-controls="sidebar" aria-label=t.ui.resize_sidebar tabindex="0"></div>
            <div class="page">{children()}</div>
            {aside.map(|aside| move || aside_picked.get().then(|| view! {
                // The handle is a sibling of the panel, as in the catalog: a child would be clipped.
                <div class="resizer preview-edge js-only" data-action="resize-preview" role="separator" aria-orientation="vertical" aria-controls="preview" aria-label=t.ui.resize_preview tabindex="0"></div>
                {aside.run()}
            }))}
            <crate::ground::Ground/>
        </div>
    }
}

/// A page without a frame (one that failed, one not found, an area picked on a phone): like a
/// framed page one scroll area with the ground at its end (app.css „one scroll area"), the page
/// across the whole width. The start page builds the same itself.
#[component]
pub fn Plain(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    view! {
        <div class=format!("work flowing solo {class}") id="page-scroll">
            <div class="page">{children()}</div>
            <crate::ground::Ground/>
        </div>
    }
}

/// „Zurück" on a page inside an area (a module, a program): up to the area's list as it was
/// left, or to the page `to` names (a module opened beside a program leads back to that program).
/// If that is where the visitor came from, `enhance.js` goes back through the browser history
/// instead (`data-back="history"`), so it is the same history entry as before and the history
/// does not grow. Esc does the same (R10).
///
/// Where it leads is read again on every change of the address: after closing the module beside
/// a program the visitor did *not* come from the list any more, so Esc must follow the link and
/// not walk the history back into the module it has just closed.
#[component]
pub fn BackLink(area: crate::tabs::Area, #[prop(optional_no_strip)] to: Option<String>) -> impl IntoView {
    let t = i18n::t();
    let location = i18n::use_location();
    let tabs = crate::tabs::Tabs::expect();
    let target = to.unwrap_or_else(|| tabs.map(|tabs| tabs.list(area)).unwrap_or_else(|| area.root().to_string()));
    let came_from_it = {
        let target = target.clone();
        move || {
            let now = crate::tabs::location_of(&location.pathname.get(), &location.search.get());
            tabs.is_some_and(|tabs| tabs.before(&now) == target).then_some("history")
        }
    };
    view! {
        <a class="ghost" href=t.path(&target) data-action="back" data-back=came_from_it title=t.ui.back_title>
            <Icon name="arrow-left"/>{t.common.back}<Shortcut keys="Esc"/>
        </a>
    }
}

/// A query failed or there is no snapshot: say so, never an empty list. An answer still on its
/// way (`DataError::pending`) is no error: nothing shows (the shell holds the page meanwhile).
#[component]
pub fn ErrorState(error: DataError) -> impl IntoView {
    if error.is_pending() {
        return ().into_any();
    }
    let t = i18n::t();
    let (title, hint) = if error.unavailable { (t.ui.unavailable_title, t.ui.unavailable_hint) } else { (t.ui.failed_title, t.ui.failed_hint) };
    view! {
        <Title text=title/>
        <section class="state state-error" role="alert">
            <h1>{title}</h1>
            <p>{hint}</p>
            <p class="state-detail">{error.message}</p>
            <p><a class="button" href="">{t.ui.reload}</a></p>
        </section>
    }
    .into_any()
}
