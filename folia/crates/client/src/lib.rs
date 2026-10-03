//! The browser app: the same Leptos components as on the server, rendered in the browser. Their
//! questions go to the data worker (`worker`), which answers them with a bundle of its own
//! (`folia-worker`) from the catalog in sql.js.
//!
//! `start()` is called once the data worker has opened the catalog. It replaces the
//! server-rendered page by the app; from then on every click is handled here and no page is
//! loaded again.

use std::sync::Arc;

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

mod worker;

/// A panic cannot unwind in WASM and would leave a frozen page: say so and offer a reload.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        console_error_panic_hook::hook(info);
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            if let (Ok(banner), Some(body)) = (document.create_element("div"), document.body()) {
                banner.set_class_name("fatal");
                let t = folia_app::i18n::texts(folia_app::i18n::of_address());
                banner.set_inner_html(&format!("{} <a href=\"\">{}</a>", t.ui.crashed, t.ui.reload_page));
                let _ = body.append_child(&banner);
            }
        }
    }));
}

/// The map of the programs as `boot.js` got it from the server (`window.betulaMap`, JSON). The
/// app does not lay anything out; without a map the landing page leaves the section out.
fn program_map() -> Option<folia_data::ProgramMapHandle> {
    let text = js_sys::Reflect::get(&web_sys::window()?.into(), &"betulaMap".into()).ok()?.as_string()?;
    serde_json::from_str(&text).ok().map(|map| folia_data::ProgramMapHandle(Arc::new(map)))
}

/// The semantic search as `boot.js` loads it (`window.betulaSemantic`): asked anew on every call,
/// so it holds no JavaScript object of its own.
struct BrowserSemantic;

impl BrowserSemantic {
    /// `window.betulaSemantic[method](...args)`, awaited; `None` when there is no such thing.
    async fn call(method: &str, args: &[JsValue]) -> Option<JsValue> {
        let semantic = js_sys::Reflect::get(&web_sys::window()?.into(), &"betulaSemantic".into()).ok().filter(|v| v.is_object())?;
        let promise = match method {
            "ready" => js_sys::Reflect::get(&semantic, &"ready".into()).ok()?,
            _ => {
                let function: js_sys::Function = js_sys::Reflect::get(&semantic, &method.into()).ok()?.dyn_into().ok()?;
                function.apply(&semantic, &args.iter().collect::<js_sys::Array>()).ok()?
            }
        };
        wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&promise)).await.ok().filter(|answer| !answer.is_null() && !answer.is_undefined())
    }
}

impl folia_data::SemanticSearch for BrowserSemantic {
    fn ready(&self) -> folia_data::Later<bool> {
        Box::pin(async { BrowserSemantic::call("ready", &[]).await.is_some() })
    }

    fn search(&self, query: &str, k: usize) -> folia_data::Later<Option<Vec<folia_data::SemanticHit>>> {
        let args = [JsValue::from_str(query), JsValue::from_f64(k as f64)];
        Box::pin(async move {
            let answer = BrowserSemantic::call("search", &args).await?;
            let hits = js_sys::Reflect::get(&answer, &"hits".into()).ok()?;
            let hits = js_sys::Array::from(&hits)
                .iter()
                .filter_map(|hit| {
                    let module_id = js_sys::Reflect::get(&hit, &"id".into()).ok()?.as_string()?;
                    let score = js_sys::Reflect::get(&hit, &"score".into()).ok()?.as_f64()? as f32;
                    Some(folia_data::SemanticHit { module_id, score })
                })
                .collect();
            Some(hits)
        })
    }
}

/// The build the server wrote the page with: the `?v=` of its stylesheet (`folia_app::BuildId`), which
/// stays in the head when the app takes the body over.
fn build_of_page(document: &web_sys::Document) -> Option<String> {
    let href = document.query_selector("link[rel=stylesheet]").ok()??.get_attribute("href")?;
    let (_, build) = href.split_once("?v=")?;
    Some(build.split(['&', '#']).next()?.to_string()).filter(|build| !build.is_empty())
}

/// Called by `boot.js` when the local database is open.
#[wasm_bindgen]
pub fn start() {
    install_panic_hook();
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    let Some(body) = document.body() else { return };
    // The language of the address (`folia_app::i18n`): the page the service worker kept for a start
    // without a network may be of another one.
    if let Some(root) = document.document_element() {
        let _ = root.set_attribute("lang", folia_app::i18n::of_address().code());
    }
    // The page's questions go to the data worker (`worker`), where `boot.js` started one;
    // without it a page answers that the catalog is not available.
    let data = worker::client();
    // Not hydration: the local database may be older than the server's page, so the app renders
    // fresh. Same components, same markup, so nothing visibly changes; and while the app's first
    // answers are on their way, the server's page stays in front of it as a picture (`boot.js`
    // took it before it called this; `answered` says when it can go).
    body.set_inner_html("");
    // The same goes for what the server wrote into the head for this page (`folia_shell::seo`): the app
    // writes its own, and what stayed would describe the first page on every later one.
    let stale = "meta[name=description], meta[name=robots], link[rel=canonical], link[rel=alternate][hreflang], meta[property^='og:'], meta[name^='twitter:'], script[type='application/ld+json']";
    if let Ok(tags) = document.query_selector_all(stale) {
        for i in 0..tags.length() {
            if let Some(tag) = tags.item(i) {
                if let Some(parent) = tag.parent_node() {
                    let _ = parent.remove_child(&tag);
                }
            }
        }
    }
    let map = program_map();
    let site = web_sys::window().and_then(|w| w.location().origin().ok());
    let build = build_of_page(&document);
    let client = data.clone();
    leptos::mount::mount_to_body(move || {
        if let Some(data) = client.clone() {
            provide_context(data);
        }
        // Loaded by `boot.js` once the app runs; nothing waits for it. The catalog's „Ähnliche Module"
        // ask it (`folia_catalog::catalog`).
        provide_context(folia_data::Semantic(Arc::new(BrowserSemantic)));
        // The icons point into the sprite of this build (`folia_design::icons`), as the server's page did.
        if let Some(build) = build.clone() {
            provide_context(folia_app::BuildId(build.into()));
        }
        if let Some(map) = map.clone() {
            provide_context(map);
        }
        if let Some(site) = site.clone() {
            provide_context(folia_shell::seo::SiteUrl(site.into()));
        }
        view! { <folia_app::App/> }
    });
    answered(data);
}

/// Tells `boot.js` (`window.betulaAnswered`) once the app's first answers are in: no question on
/// its way in two looks a frame apart, or after `TAKEOVER_MS` whatever is still on its way.
fn answered(data: Option<folia_data::DataClient>) {
    const TAKEOVER_MS: f64 = 4000.0;
    let tell = || {
        if let Some(window) = web_sys::window() {
            if let Ok(function) = js_sys::Reflect::get(&window, &"betulaAnswered".into()).and_then(|f| f.dyn_into::<js_sys::Function>()) {
                let _ = function.call0(&window);
            }
        }
    };
    let Some(data) = data else { return tell() };
    let started = js_sys::Date::now();
    wasm_bindgen_futures::spawn_local(async move {
        let mut quiet = 0;
        loop {
            sleep(16).await;
            quiet = if untrack(|| data.waiting()) == 0 { quiet + 1 } else { 0 };
            if quiet >= 2 || js_sys::Date::now() - started > TAKEOVER_MS {
                break;
            }
        }
        tell();
    });
}

/// A timer as a future (no crate for it: one promise).
async fn sleep(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}
