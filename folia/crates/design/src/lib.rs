//! The design system of the minimal version: the icon set, the mark, the page's frame and a
//! skeleton. Markup over data and nothing else, so the site and the app render the same; its
//! styles are `style.css` beside this crate (prefix `ds-`, layer `design`).

use leptos::prelude::*;

// Today's icon table, used as it is (it depends on nothing).
#[path = "../../../../app/src/icons.rs"]
pub mod icons;

/// Where the server serves the sprite.
pub const SPRITE: &str = icons::SPRITE;

#[component]
pub fn Icon(name: &'static str) -> impl IntoView {
    let markup = icons::markup(name).map(|_| format!("<use href=\"{SPRITE}#{name}\"/>")).unwrap_or_default();
    view! { <svg class="icon" viewBox="0 0 24 24" aria-hidden="true" inner_html=markup></svg> }
}

#[component]
pub fn Mark() -> impl IntoView {
    view! {
        <svg class="mark" viewBox="0 0 32 32" aria-hidden="true">
            <rect width="32" height="32" rx="7"/>
            <path d="M0 7h13v3H0zM23 12h9v3h-9zM0 17h5v3H0zM17 22h15v3H17z"/>
        </svg>
    }
}

#[component]
pub fn Wordmark() -> impl IntoView {
    view! {
        <span class="wordmark small" role="img" aria-label="Betula">
            <span aria-hidden="true"><span>"B"</span><span class="sc e">"E"</span><span>"T"</span><span class="u">"U"</span><span class="sc la">"LA"</span></span>
        </span>
    }
}

/// The page's frame: a sidebar, the content, and a panel beside it. One component for every page
/// that wants it (docs/folia-refactor.md §5.2); the minimal version has no handles yet.
#[component]
pub fn Frame(#[prop(into)] title: String, #[prop(into)] sidebar: ViewFn, children: Children) -> impl IntoView {
    let label = title.clone();
    view! {
        <div class="work framed ds-frame">
            <aside class="panel sidebar" aria-label=label>
                <div class="panel-head"><h2>{title}</h2></div>
                <div class="body scroll">{sidebar.run()}</div>
            </aside>
            <div class="page">{children()}</div>
        </div>
    }
}

/// Rows standing in for a list that is on its way.
#[component]
pub fn SkeletonRows(count: usize) -> impl IntoView {
    view! {
        <div class="ds-skeleton" aria-hidden="true">
            {(0..count).map(|_| view! { <div class="ds-skeleton-row"><span></span><span></span></div> }).collect_view()}
        </div>
    }
}
