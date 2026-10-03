//! Small building blocks every page uses. (The full design system is phase 3; these
//! already follow its rules: classes from the stylesheet, no inline styles.)

use folia_model::labels::{Code, ModuleKind, OfferStatus};
use folia_model::text::{self, Block, Inline, Item, List, ListKind};
use leptos::prelude::*;
use leptos_meta::Title;

use crate::data::DataError;
use crate::i18n;

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

#[component]
pub fn NotFound(#[prop(into)] title: String, #[prop(into)] hint: String) -> impl IntoView {
    let t = i18n::t();
    view! {
        <Title text=title.clone()/>
        <section class="state">
            <h1>{title}</h1>
            <p>{hint}</p>
            <p>
                <a class="button" href=t.path(folia_routes::url::CATALOG)>{t.common.to_catalog}</a>" "
                <a class="button button-quiet" href=t.path(folia_routes::url::PROGRAMS)>{t.common.to_programs}</a>
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
    let t = i18n::t();
    match kind {
        Some(kind) => view! { <span class=format!("kind k-{}", kind.code())><i></i>{kind.label(t.locale).to_string()}</span> }.into_any(),
        None => view! { <span class="kind k-none"><i></i>{t.ui.kind_unknown}</span> }.into_any(),
    }
}

/// Only what deviates from "is offered" gets a badge.
#[component]
pub fn OfferBadge(status: Code<OfferStatus>) -> impl IntoView {
    let t = i18n::t();
    (!status.is(OfferStatus::Active)).then(|| view! { <span class="flag">{status.label(t.locale).to_string()}</span> })
}

/// A label with its value; shows „nicht angegeben" instead of hiding an unknown value.
#[component]
pub fn Fact(#[prop(into)] label: String, value: Option<String>, #[prop(default = "info")] icon: &'static str, #[prop(optional)] wide: bool) -> impl IntoView {
    let t = i18n::t();
    let unknown = value.is_none();
    view! {
        <div class="fact" class:wide=wide>
            <span class="ico"><Icon name=icon/></span>
            <div>
                <dt>{label}</dt>
                <dd class:unknown=unknown>{value.unwrap_or_else(|| t.common.not_stated.to_string())}</dd>
            </div>
        </div>
    }
}

/// A free text of a module, which is Markdown (`folia_model::text`): its paragraphs and lists, its
/// strong and emphasized words and the line breaks it keeps, set in Blocksatz (app.css „prose").
/// `lang` is the language the text is written in, which need not be the page's: a German text on
/// the English page is hyphenated by the German rules and read out in German.
#[component]
pub fn Prose(text: String, #[prop(optional_no_strip)] lang: Option<String>) -> impl IntoView {
    view! { <div class="prose" lang=lang>{blocks_view(text::blocks(&text))}</div> }
}

fn blocks_view(blocks: Vec<Block>) -> AnyView {
    blocks.into_iter().map(block_view).collect_view().into_any()
}

/// A paragraph that is all strong is a heading the page sets so („**Modulabschlussprüfung:**"): it
/// is not justified, nor hyphenated.
fn block_view(block: Block) -> AnyView {
    match block {
        Block::Paragraph(inlines) => {
            let head = inlines.iter().any(|inline| matches!(inline, Inline::Strong(_)))
                && inlines.iter().all(|inline| matches!(inline, Inline::Strong(_)) || matches!(inline, Inline::Text(text) if text.trim().is_empty()));
            match head {
                true => view! { <p class="head">{inlines_view(inlines)}</p> }.into_any(),
                false => view! { <p>{inlines_view(inlines)}</p> }.into_any(),
            }
        }
        Block::List(list) => list_view(list),
    }
}

/// A list of labels („(1)", „a)", „IV.") sets them where the markers stand, as wide as the widest
/// (`w2` … `w6`, in characters).
fn list_view(list: List) -> AnyView {
    let width = list.items.iter().filter_map(|item| item.label.as_ref()).map(|label| label.chars().count()).max().unwrap_or(0);
    let items = list.items.into_iter().map(item_view).collect_view();
    match list.kind {
        ListKind::Bullets => view! { <ul>{items}</ul> }.into_any(),
        ListKind::Numbers(start) => view! { <ol start=(start != 1).then(|| start.to_string())>{items}</ol> }.into_any(),
        ListKind::Labels => view! { <ol class=format!("labels w{}", width.clamp(2, 6))>{items}</ol> }.into_any(),
    }
}

/// An item of one paragraph sets it without <p>, as a list of lines is set; one of more sets each.
fn item_view(item: Item) -> AnyView {
    let paragraphs = item.blocks.iter().filter(|block| matches!(block, Block::Paragraph(_))).count();
    let content = item
        .blocks
        .into_iter()
        .map(|block| match block {
            Block::Paragraph(inlines) if paragraphs == 1 => inlines_view(inlines),
            block => block_view(block),
        })
        .collect_view();
    view! { <li>{item.label.map(|label| view! { <span class="li-label">{label}</span> })}{content}</li> }.into_any()
}

fn inlines_view(inlines: Vec<Inline>) -> AnyView {
    inlines
        .into_iter()
        .map(|inline| match inline {
            Inline::Text(text) => text.into_any(),
            Inline::Strong(inner) => view! { <strong>{inlines_view(inner)}</strong> }.into_any(),
            Inline::Emphasis(inner) => view! { <em>{inlines_view(inner)}</em> }.into_any(),
            Inline::Break => view! { <br/> }.into_any(),
        })
        .collect_view()
        .into_any()
}

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

/// „Nach oben" (owner, 2026-09-26: after a while in the catalog's list it was hard to get back to
/// its top): one button for every page, floating in the corner of what scrolls — the page on a
/// wide screen (left of a module or an area that floats beside it), the window on a phone, above
/// the bottom bar. `enhance.js` shows it once the page is more than a screen down and takes the
/// page back to its top. It needs JavaScript (R15), and the server's page is the same for
/// everybody (R9), so it is part of every page and stays out of sight until it has a way to go.
#[component]
pub fn ToTop() -> impl IntoView {
    let t = i18n::t();
    view! {
        <button class="to-top js-only" id="to-top" type="button" data-action="to-top" title=t.ui.to_top aria-label=t.ui.to_top>
            <Icon name="arrow-up"/>
        </button>
    }
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

// Rendering to HTML needs the server's build (`ssr`), as in `cargo test -p folia-app -p folia-server`.
#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;

    fn prose(markdown: &str, lang: Option<&str>) -> String {
        view! { <Prose text=markdown.to_string() lang=lang.map(str::to_string)/> }.to_html().replace("<!>", "")
    }

    /// A module's text is set as text: paragraphs, lists of the three kinds, a heading of its own, a
    /// line break, the language it is written in — and nothing of the page's markup but its words.
    #[test]
    fn a_text_is_set_as_paragraphs_and_lists() {
        let html = prose(
            "**Modulabschlussprüfung:**\n\n- Klausur, 90 min. **ODER**\n- mündliche Prüfung\n\n3. drei\n4. vier\n\n- (a) Absorption\\\n  Licht\n- (b) Elektronen\n\n<b>roh</b> und *betont*",
            Some("de"),
        );
        assert!(html.starts_with(r#"<div lang="de" class="prose">"#), "{html}");
        for part in [
            r#"<p class="head"><strong>Modulabschlussprüfung:</strong></p>"#,
            "<ul><li>Klausur, 90 min. <strong>ODER</strong></li><li>mündliche Prüfung</li></ul>",
            r#"<ol start="3"><li>drei</li><li>vier</li></ol>"#,
            r#"<ol class="labels w3"><li><span class="li-label">(a)</span>Absorption<br>Licht</li><li><span class="li-label">(b)</span>Elektronen</li></ol>"#,
            "<p>&lt;b&gt;roh&lt;/b&gt; und <em>betont</em></p>",
        ] {
            assert!(html.contains(part), "{part}\nin {html}");
        }
        assert!(prose("Text", None).starts_with(r#"<div class="prose">"#));
    }
}
