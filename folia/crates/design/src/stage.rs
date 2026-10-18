//! The stages of the pages (folia/design/tabwechsel/konzept.html, „Tabs, die stehen bleiben"; owner,
//! 2026-10-18: „Es ist nice zu sehen, dass die Seiten gleich da sind. Das muss auf jeden Fall
//! rein"). In the browser every tab's page stays built on a stage of its own (`folia_shell::stage`),
//! and a change of tab only swaps which stage is in front. A page on a stage
//!
//! - reads where its stage is (`i18n::use_location` gives the stage's place, `i18n::use_params_map`
//!   the params of its path): the router's while the stage is the one the router shows, else where
//!   the stage was left;
//! - knows whether its stage is in front (`front`): what a page does to the whole document (a
//!   listener of the window, a step of its own in the address, its tags for search engines) it
//!   does only then; its title (`Title`) the stages give the document while it is;
//! - finds its parts on its own stage (`by_id`, `select`, `select_all`), where another stage has
//!   parts of the same name (every list has a `#preview`, every page a `#page-scroll`).
//!
//! Outside the stages (on the server, in the chrome around the pages) a page is always in front,
//! and the document is its stage.

use std::sync::atomic::{AtomicU64, Ordering};

use leptos::oco::Oco;
use leptos::prelude::*;
use leptos::text_prop::TextProp;
use leptos_router::location::Location;
use leptos_router::params::ParamsMap;

/// What a stage tells the page on it.
#[derive(Clone)]
pub struct Stage {
    /// Where the stage is (path without the language's prefix, query, fragment).
    pub location: Location,
    /// The params of its path: a module's `id`, a program's `slug` and `tab`.
    pub params: Memo<ParamsMap>,
    /// Whether the stage is in front.
    pub front: Signal<bool>,
    /// The stage's element, once it is there.
    pub node: NodeRef<leptos::html::Div>,
    /// The titles its page wrote (`Title`).
    pub titles: Titles,
}

impl Stage {
    /// The stage of the page this is called in (the stages give it as context, each in an owner of
    /// its own); `None` outside the stages.
    pub fn here() -> Option<Self> {
        use_context::<Self>()
    }
}

/// Whether the page this is called in stands in front (tracked where it is read): outside the
/// stages always. Take it while the component is built, and read it in a listener with
/// `get_untracked`.
pub fn front() -> Signal<bool> {
    Stage::here().map_or_else(|| Signal::stored(true), |stage| stage.front)
}

/// The titles the page on a stage wrote (`Title`), in the order they came, as `leptos_meta` keeps
/// them for the document: the last is the page's (a failure's over that of the list it is in).
/// The stages give the document the title of the stage in front (`folia_shell::stage`).
#[derive(Clone, Copy)]
pub struct Titles(RwSignal<Vec<(u64, TextProp)>>);

impl Default for Titles {
    fn default() -> Self {
        Self(RwSignal::new(Vec::new()))
    }
}

impl Titles {
    /// The title the stage shows: the last one written, "" while its page has none (tracked).
    pub fn shown(self) -> Oco<'static, str> {
        let last = self.0.with(|titles| titles.last().map(|(_, text)| text.clone()));
        last.map_or(Oco::Borrowed(""), |text| text.get())
    }

    /// `text` is the stage's title from now until the component that wrote it is cleaned up.
    fn write(self, text: TextProp) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        self.0.update(|titles| titles.push((id, text)));
        on_cleanup(move || {
            self.0.try_update(|titles| titles.retain(|(written, _)| *written != id));
        });
    }
}

/// The page's title, as `leptos_meta::Title` writes it: on a stage the stage's (`Titles`), so that
/// the document has the title of the page that is seen while every stage keeps its page built,
/// and a stage that comes to the front brings its title along. Outside the stages (the server) it
/// is `leptos_meta::Title`.
///
/// A `leptos_meta::Title` itself cannot follow the front: it holds no place in the page, so
/// `tachys` cannot put it back where it took it away (`TitleViewState::insert_before_this`), and
/// one that comes back would go on top of those of the same page that never left.
#[component]
pub fn Title(#[prop(into)] text: TextProp) -> impl IntoView {
    match Stage::here() {
        Some(stage) => {
            stage.titles.write(text);
            ().into_any()
        }
        None => view! { <leptos_meta::Title text=text/> }.into_any(),
    }
}

#[cfg(feature = "csr")]
thread_local! {
    /// The element of the stage in front, as the stages set it.
    static FRONT: std::cell::RefCell<Option<web_sys::Element>> = const { std::cell::RefCell::new(None) };
}

/// The stage in front is now `element` (`folia_shell::stage`, at every change of it).
#[cfg(feature = "csr")]
pub fn set_front(element: Option<web_sys::Element>) {
    FRONT.with(|front| *front.borrow_mut() = element);
}

/// The element of the stage in front; `None` where there are no stages.
#[cfg(feature = "csr")]
pub fn front_element() -> Option<web_sys::Element> {
    FRONT.with(|front| front.borrow().clone())
}

/// Where a page looks for its parts: the stage of the page that asks (an effect of it runs with
/// its owner), else the stage in front (a listener has no owner; only the stage in front takes
/// input). `None` without stages: the whole document then.
#[cfg(feature = "csr")]
fn scope() -> Option<web_sys::Element> {
    use wasm_bindgen::JsCast;
    Stage::here().and_then(|stage| stage.node.get_untracked()).map(|node| node.unchecked_into::<web_sys::Element>()).or_else(front_element)
}

/// The first element of the document that matches `selector` outside every stage: the bars and
/// dialogs around the pages.
#[cfg(feature = "csr")]
fn outside(document: &web_sys::Document, selector: &str) -> Option<web_sys::Element> {
    use wasm_bindgen::JsCast;
    let all = document.query_selector_all(selector).ok()?;
    (0..all.length()).filter_map(|i| all.item(i)?.dyn_into::<web_sys::Element>().ok()).find(|element| element.closest(".page-stage").ok().flatten().is_none())
}

/// The element with the id `id` on the page's stage (`scope`), else outside every stage; the
/// document's without stages.
#[cfg(feature = "csr")]
pub fn by_id(id: &str) -> Option<web_sys::Element> {
    let document = web_sys::window()?.document()?;
    let Some(stage) = scope() else { return document.get_element_by_id(id) };
    let selector = format!("[id=\"{}\"]", id.replace('"', "\\\""));
    stage.query_selector(&selector).ok().flatten().or_else(|| outside(&document, &selector))
}

/// The first element that matches `selector` on the page's stage, else outside every stage; the
/// document's without stages.
#[cfg(feature = "csr")]
pub fn select(selector: &str) -> Option<web_sys::Element> {
    let document = web_sys::window()?.document()?;
    let Some(stage) = scope() else { return document.query_selector(selector).ok().flatten() };
    stage.query_selector(selector).ok().flatten().or_else(|| outside(&document, selector))
}

/// Every element that matches `selector` on the page's stage and outside every stage; the
/// document's without stages.
#[cfg(feature = "csr")]
pub fn select_all(selector: &str) -> Vec<web_sys::Element> {
    use wasm_bindgen::JsCast;
    let elements = |list: Option<web_sys::NodeList>| -> Vec<web_sys::Element> {
        list.map(|list| (0..list.length()).filter_map(|i| list.item(i)?.dyn_into::<web_sys::Element>().ok()).collect()).unwrap_or_default()
    };
    let Some(document) = web_sys::window().and_then(|window| window.document()) else { return Vec::new() };
    let Some(stage) = scope() else { return elements(document.query_selector_all(selector).ok()) };
    let mut found = elements(stage.query_selector_all(selector).ok());
    found.extend(elements(document.query_selector_all(selector).ok()).into_iter().filter(|element| element.closest(".page-stage").ok().flatten().is_none()));
    found
}
