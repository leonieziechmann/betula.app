mod app;
pub mod components;
mod db;
mod detail;
pub mod fuzzy;
mod models;
mod program_detail;
mod study_plan;
mod plan_view;
mod query;
mod storage;

use app::App;
use leptos::mount::{mount_to, mount_to_body};
use wasm_bindgen::JsCast;

fn main() {
    console_error_panic_hook::set_once();
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        if let Some(root) = doc.get_element_by_id("root").and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok()) {
            root.set_inner_html("");
            mount_to(root, App).forget();
            return;
        }
    }
    mount_to_body(App);
}
