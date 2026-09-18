mod app;
pub mod components;
mod db;
mod detail;
pub mod fuzzy;
mod models;
mod program_detail;
mod storage;

use app::App;
use leptos::mount::mount_to_body;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
