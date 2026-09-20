//! Small building blocks every page uses. (The full design system is phase 3; these
//! already follow its rules: classes from the stylesheet, no inline styles.)

use catalog::labels::{Code, ModuleKind, OfferStatus, KIND_UNKNOWN};
use leptos::prelude::*;
use leptos_meta::Title;

use crate::data::DataError;

/// The mark of Betula: birch bark that also reads as the rows of a list. Drawn on a 32 px grid, the
/// size it has in the rail (`.logo`), so its bars fall on whole pixels. Larger cuts: `design/logo`.
#[component]
pub fn Mark() -> impl IntoView {
    view! {
        <svg class="mark" viewBox="0 0 32 32" aria-hidden="true">
            <rect width="32" height="32" rx="7"/>
            <path d="M0 7h13v3H0zM23 12h9v3h-9zM0 17h5v3H0zM17 22h15v3H17z"/>
        </svg>
    }
}

/// The wordmark: B, T and U stand out of BᴇTUʟᴀ, the E sits under the bar of the T. The spacing
/// lives in the stylesheet (`.wordmark`); each run keeps its own element because the spacing hangs
/// on them. `small` is the cut for sizes under 28 px.
#[component]
pub fn Wordmark(#[prop(optional)] small: bool) -> impl IntoView {
    view! {
        <span class="wordmark" class:small=small role="img" aria-label="Betula">
            <span aria-hidden="true"><span>"B"</span><span class="sc e">"E"</span><span>"T"</span><span class="u">"U"</span><span class="sc la">"LA"</span></span>
        </span>
    }
}

/// A query failed or there is no snapshot: say so, never an empty list.
#[component]
pub fn ErrorState(error: DataError) -> impl IntoView {
    let (title, hint) = if error.unavailable {
        ("Der Katalog ist gerade nicht verfügbar", "Die Daten werden noch geladen. Bitte versuche es gleich noch einmal.")
    } else {
        ("Etwas ist schiefgelaufen", "Die Seite konnte nicht geladen werden. Bitte lade sie neu.")
    };
    view! {
        <Title text=title/>
        <section class="state state-error" role="alert">
            <h1>{title}</h1>
            <p>{hint}</p>
            <p class="state-detail">{error.message}</p>
            <p><a class="button" href="">"Neu laden"</a></p>
        </section>
    }
}

#[component]
pub fn NotFound(#[prop(into)] title: String, #[prop(into)] hint: String) -> impl IntoView {
    view! {
        <Title text=title.clone()/>
        <section class="state">
            <h1>{title}</h1>
            <p>{hint}</p>
            <p>
                <a class="button" href=catalog::url::CATALOG>"Zum Modulkatalog"</a>" "
                <a class="button button-quiet" href=catalog::url::PROGRAMS>"Zu den Studiengängen"</a>
            </p>
        </section>
    }
}

#[component]
pub fn EmptyState(#[prop(into)] title: String, #[prop(into)] hint: String) -> impl IntoView {
    view! {
        <div class="state state-empty">
            <p class="state-title">{title}</p>
            <p>{hint}</p>
        </div>
    }
}

/// „Pflicht", „Wahlpflicht" …, or „Art nicht angegeben": never a default.
#[component]
pub fn KindBadge(kind: Option<Code<ModuleKind>>) -> impl IntoView {
    match kind {
        Some(kind) => view! { <span class=format!("kind k-{}", kind.code())><i></i>{kind.label().to_string()}</span> }.into_any(),
        None => view! { <span class="kind k-none"><i></i>{KIND_UNKNOWN}</span> }.into_any(),
    }
}

/// Only what deviates from "is offered" gets a badge.
#[component]
pub fn OfferBadge(status: Code<OfferStatus>) -> impl IntoView {
    (!status.is(OfferStatus::Active))
        .then(|| view! { <span class="flag">{status.label().to_string()}</span> })
}

/// A label with its value; shows „nicht angegeben" instead of hiding an unknown value.
#[component]
pub fn Fact(#[prop(into)] label: String, value: Option<String>, #[prop(default = "info")] icon: &'static str, #[prop(optional)] wide: bool) -> impl IntoView {
    let unknown = value.is_none();
    view! {
        <div class="fact" class:wide=wide>
            <span class="ico"><Icon name=icon/></span>
            <div>
                <dt>{label}</dt>
                <dd class:unknown=unknown>{value.unwrap_or_else(|| "nicht angegeben".to_string())}</dd>
            </div>
        </div>
    }
}

/// Free text from a module page: paragraphs on blank lines, line breaks kept.
#[component]
pub fn Prose(text: String) -> impl IntoView {
    text.split("\n\n")
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
        .map(|paragraph| view! { <p class="prose">{paragraph.to_string()}</p> })
        .collect_view()
}

/// The frame of a page (owner decision 2026-09-20: a basic element of the layout, R17): a sidebar
/// as wide as the filter panel of the catalog, with the same handle and the same remembered
/// width, and the page next to it. Going from one area to another, nothing jumps. The sidebar
/// holds what belongs to the page as a whole: its filters, its sections, its actions.
/// The catalog is the one page that builds this frame itself (its sidebar is the filter form).
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
    children: Children,
) -> impl IntoView {
    let label = title.clone();
    view! {
        <div class="work framed" class:sidebar-first=sidebar_first>
            <aside class="panel sidebar" class:sheet=sheet id="sidebar" aria-label=label>
                <div class="panel-head">
                    <h2>{title}</h2>
                    {head.run()}
                    {sheet.then(|| view! { <a class="icon-btn sheet-close" href="#" data-action="sheet-close" aria-label="Schließen"><Icon name="x"/></a> })}
                </div>
                <div class="body scroll" data-keep-scroll="sidebar">{sidebar.run()}</div>
            </aside>
            <div class="resizer between js-only" data-action="resize-filters" role="separator" aria-orientation="vertical" aria-controls="sidebar" aria-label="Breite der Seitenleiste ändern (Pfeiltasten, Doppelklick setzt zurück)" tabindex="0"></div>
            <div class="page" id="page-scroll">{children()}</div>
        </div>
    }
}

/// „Zurück" on a page inside an area (a module, a program): up to the area's list as it was left.
/// If that list is where the visitor came from, `enhance.js` goes back through the browser
/// history instead (`data-back="history"`), so the list is the same history entry as before and
/// the history does not grow. Esc does the same (R10).
#[component]
pub fn BackLink(area: crate::tabs::Area) -> impl IntoView {
    let location = leptos_router::hooks::use_location();
    let tabs = crate::tabs::Tabs::expect();
    let now = crate::tabs::location_of(&location.pathname.get_untracked(), &location.search.get_untracked());
    let list = tabs.map(|tabs| tabs.list(area)).unwrap_or_else(|| area.root().to_string());
    let came_from_it = tabs.is_some_and(|tabs| tabs.before(&now) == list);
    view! {
        <a class="ghost" href=list data-action="back" data-back=came_from_it.then_some("history") title="Zurück (Esc)">
            <Icon name="arrow-left"/>"Zurück"<Shortcut keys="Esc"/>
        </a>
    }
}

/// A two-state toggle that is a link to the page with the other state: the same look and the
/// same rules as the toggles of the catalog's filter panel (no handler, works without
/// JavaScript, the space bar flips it).
#[component]
pub fn ToggleLink(
    #[prop(into)] href: Signal<String>,
    #[prop(into)] on: Signal<bool>,
    #[prop(into)] label: String,
    /// A number shown at the right end.
    #[prop(optional)] count: Option<usize>,
) -> impl IntoView {
    view! {
        <a
            class="chip"
            href=move || href.get()
            role="checkbox"
            rel="nofollow"
            draggable="false"
            data-noscroll=""
            data-state=move || if on.get() { "with" } else { "off" }
            aria-checked=move || if on.get() { "true" } else { "false" }
        >
            <span class="box"><Icon name="check"/><Icon name="x"/></span>
            <span class="chip-label">{label}</span>
            {count.map(|count| view! { <span class="chip-count num">{count}</span> })}
        </a>
    }
}

/// Wraps what only works with JavaScript: shortcut hints, drag handles, the theme switch. The
/// server sends the same HTML to everybody and cannot know who has scripts (R9), so the parts
/// are in the page and the stylesheet hides them until the script in the head has marked the
/// document (`html.js`, before the first paint: nothing flashes). Single elements can carry the
/// class `js-only` themselves; the wrapper has no box of its own.
#[component]
pub fn JsOnly(children: Children) -> impl IntoView {
    view! { <span class="js-only">{children()}</span> }
}

/// Virtual oversizing for one element (class `hit`): how far, in pixels, it reacts to the pointer
/// beyond what it shows. The stylesheet explains the mechanism and sets the sizes of whole
/// families of controls; this is for a single control whose surroundings are its own, as in
/// `<a class="ghost hit" style=Hit::y(7.0).style()>`. Towards a neighbouring control stay at or
/// below half the gap to it, so that two areas never overlap.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hit {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Hit {
    pub const fn all(px: f32) -> Self {
        Self { top: px, right: px, bottom: px, left: px }
    }

    /// Left and right.
    pub const fn x(px: f32) -> Self {
        Self { top: 0.0, right: px, bottom: 0.0, left: px }
    }

    /// Above and below.
    pub const fn y(px: f32) -> Self {
        Self { top: px, right: 0.0, bottom: px, left: 0.0 }
    }

    pub const fn sides(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self { top, right, bottom, left }
    }

    /// The custom properties for the `style` attribute.
    pub fn style(self) -> String {
        format!("--hit-t:{}px;--hit-r:{}px;--hit-b:{}px;--hit-l:{}px", self.top, self.right, self.bottom, self.left)
    }
}

/// A shortcut written next to its control (R10).
#[component]
pub fn Shortcut(keys: &'static str) -> impl IntoView {
    view! { <JsOnly><kbd>{keys}</kbd></JsOnly> }
}

/// An icon of the inlined set (`crate::icons`). Unknown names render an empty box, never panic.
#[component]
pub fn Icon(name: &'static str, #[prop(optional)] class: &'static str) -> impl IntoView {
    let markup = crate::icons::markup(name).unwrap_or_default();
    let class = if class.is_empty() { "icon".to_string() } else { format!("icon {class}") };
    view! { <svg class=class viewBox="0 0 24 24" aria-hidden="true" inner_html=markup></svg> }
}
