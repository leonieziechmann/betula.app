//! Small building blocks every page uses. (The full design system is phase 3; these
//! already follow its rules: classes from the stylesheet, no inline styles.)

use catalog::labels::{Code, ModuleKind, OfferStatus, KIND_UNKNOWN};
use leptos::prelude::*;
use leptos_meta::Title;

use crate::data::DataError;

/// The mark of Betula: a birch leaf, white, with the black marks of birch bark on it, which also
/// read as the rows of a list, on the green of the leaf. Drawn on the 32 px grid, the size it has
/// in the rail (`.logo`), where its rows fall on whole pixels; the green is the element's
/// background (`.mark`), so the picture needs no ids and can stand on a page more than once. The
/// paths come from `design/logo/mark.mjs`, with the larger cuts.
#[component]
pub fn Mark() -> impl IntoView {
    view! {
        <svg class="mark" viewBox="0 0 32 32" aria-hidden="true">
            <path class="mark-stem" d="M16 22Q16.27 24.2 15.17 26.07"/>
            <path class="mark-leaf" d="M16 4.9C16.53 7.87 19.97 12.33 22.53 16.4C23.9 18.43 23.83 20.13 22.53 21.03C20.57 21.97 17.97 22.33 16 22.73C14.03 22.33 11.43 21.97 9.47 21.03C8.17 20.13 8.1 18.43 9.47 16.4C12.03 12.33 15.47 7.87 16 4.9Z"/>
            <path class="mark-rows" d="M16.3 10H13.64C13.24 10.65 12.81 11.32 12.36 12H16.3ZM16.8 13H20.3C20.75 13.67 21.19 14.34 21.63 15H16.8ZM12.9 16H9.72C9.64 16.13 9.55 16.27 9.47 16.4C9.09 16.96 8.82 17.5 8.66 18H12.9ZM14.2 19H23.53C23.56 19.84 23.24 20.52 22.58 21H14.2Z"/>
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

/// Nothing to show: what is missing, and a hint. `children` are the ways on from here (links
/// or buttons, the first the one to take), set in a row under the hint.
#[component]
pub fn EmptyState(#[prop(into)] title: String, #[prop(into)] hint: String, #[prop(optional)] children: Option<Children>) -> impl IntoView {
    view! {
        <div class="state state-empty">
            <p class="state-title">{title}</p>
            <p>{hint}</p>
            {children.map(|children| view! { <div class="state-actions">{children()}</div> })}
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
///
/// A page can put a panel on the right as well (`aside`): what belongs to what the visitor
/// picked. It is the catalog's module preview in all but its content — as wide, with the same
/// handle and remembered width, floating over the page docked to its right edge, and there only
/// while something is picked (`aside_picked`). A column of its own left a 13-inch screen too
/// little room for the page (owner, 2026-09-23).
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
    let label = title.clone();
    let has_aside = aside.is_some();
    view! {
        <div class="work framed" class:sidebar-first=sidebar_first class:with-aside=has_aside class:aside-picked=move || has_aside && aside_picked.get()>
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
            {aside.map(|aside| move || aside_picked.get().then(|| view! {
                // The handle is a sibling of the panel, as in the catalog: a child would be clipped.
                <div class="resizer preview-edge js-only" data-action="resize-preview" role="separator" aria-orientation="vertical" aria-controls="preview" aria-label="Breite der Vorschau ändern (Pfeiltasten, Doppelklick setzt zurück)" tabindex="0"></div>
                {aside.run()}
            }))}
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
    let location = leptos_router::hooks::use_location();
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
        <a class="ghost" href=target data-action="back" data-back=came_from_it title="Zurück (Esc)">
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

/// An icon of the set (`crate::icons`): a pointer into the sprite the server serves once, linked
/// with the build of the page like the stylesheet (`crate::asset`). Unknown names render an empty
/// box, never panic.
#[component]
pub fn Icon(name: &'static str, #[prop(optional)] class: &'static str) -> impl IntoView {
    let markup = crate::icons::markup(name).map(|_| format!("<use href=\"{}#{name}\"/>", crate::asset(crate::icons::SPRITE))).unwrap_or_default();
    let class = if class.is_empty() { "icon".to_string() } else { format!("icon {class}") };
    view! { <svg class=class viewBox="0 0 24 24" aria-hidden="true" inner_html=markup></svg> }
}
