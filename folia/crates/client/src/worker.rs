//! The page's side of the data worker (docs/folia/folia-refactor.md §6.2): `BrowserData` asks it
//! every question of the app's pages through `window.betulaData` (`folia/assets/boot.js`, which
//! starts the worker before the app), and `snapshot_changed` hears when it answers from a newer
//! snapshot. The worker's side is `folia-worker`.

use std::cell::RefCell;
use std::sync::Arc;

use folia_data::{Answerer, DataClient, Later};
use folia_pages::ask::Lane;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

thread_local! {
    /// The page's client, for `snapshot_changed`.
    static CLIENT: RefCell<Option<DataClient>> = const { RefCell::new(None) };
}

/// On the page: the data worker answers from a newer snapshot now (`boot.js` hears it): every
/// answer kept is forgotten, and what shows one asks again and shows the new one in place.
#[wasm_bindgen]
pub fn snapshot_changed() {
    if let Some(client) = CLIENT.with_borrow(Clone::clone) {
        client.forget();
    }
}

/// The data worker as the page asks it: `window.betulaData`, asked anew on every call, so it holds
/// no JavaScript object of its own.
struct BrowserData;

impl BrowserData {
    fn data() -> Option<JsValue> {
        js_sys::Reflect::get(&web_sys::window()?.into(), &"betulaData".into()).ok().filter(JsValue::is_object)
    }
}

impl Answerer for BrowserData {
    fn ask(&self, name: &'static str, lane: Lane, question: String) -> Later<Option<String>> {
        Box::pin(async move {
            let data = Self::data()?;
            let function: js_sys::Function = js_sys::Reflect::get(&data, &"ask".into()).ok()?.dyn_into().ok()?;
            let lane = JsValue::from_str(match lane {
                Lane::Page => "page",
                Lane::Typing => "typing",
            });
            let promise = function.call3(&data, &JsValue::from_str(name), &lane, &JsValue::from_str(&question)).ok()?;
            // An empty text: a newer question of the lane took this one's place; `null`: no answer.
            wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&promise)).await.ok()?.as_string()
        })
    }
}

/// The client of the page: the data worker's, where `boot.js` started one.
pub(crate) fn client() -> Option<DataClient> {
    let client = BrowserData::data().map(|_| DataClient::remote(Arc::new(BrowserData)))?;
    CLIENT.with_borrow_mut(|kept| *kept = Some(client.clone()));
    Some(client)
}
