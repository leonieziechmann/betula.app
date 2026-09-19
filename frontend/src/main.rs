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
use leptos::mount::mount_to_body;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
