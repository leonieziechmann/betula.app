//! The site (docs/folia-refactor.md §4): whole pages as HTML, the shell around the catalog's views,
//! and the app document for a route of the app alone. Rendered by Leptos on the server, from the
//! crates the app renders in the browser.

use catalog::Database;
use folia_catalog_ui::{ListView, ModuleView};
use folia_pages::{answer, Answer, Ask};
use folia_shell::{Area, Shell};
use leptos::prelude::*;

/// A whole document: the head with the stylesheets (today's and the crates'), the boot script, and
/// `body` (rendered HTML).
pub fn document(title: &str, body: &str, build: &str) -> String {
    format!(
        "<!DOCTYPE html><html lang=\"de\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<meta name=\"color-scheme\" content=\"light dark\"><title>{title} · Betula</title>\
<script>document.documentElement.classList.add('js');var n=new Date().getMonth();document.documentElement.dataset.season=n<2||n>10?'winter':n<5?'spring':n<8?'summer':'autumn';</script>\
<link rel=\"preload\" as=\"font\" type=\"font/woff2\" crossorigin=\"anonymous\" href=\"/assets/inter-latin.woff2\">\
<link rel=\"stylesheet\" href=\"/assets/app.css\"><link rel=\"stylesheet\" href=\"/assets/folia.css?v={build}\">\
<script type=\"module\" src=\"/assets/next-boot.js?v={build}\"></script></head><body>{body}</body></html>"
    )
}

fn framed(area: Area, page: impl IntoView + 'static) -> String {
    let owner = Owner::new();
    owner.with(|| view! { <Shell area=Signal::stored(area) status=Signal::stored(None::<String>)>{page}</Shell> }.to_html())
}

/// A page of the site, or `None` where the address is not one (404).
pub fn page(db: &dyn Database, ask: &Ask, build: &str) -> Result<Option<String>, String> {
    Ok(match answer(db, ask)? {
        Answer::Catalog(data) => Some(document("Module", &framed(Area::Catalog, view! { <ListView data=*data/> }), build)),
        Answer::Module(Some(data)) => {
            let title = data.module.title.clone();
            Some(document(&title, &framed(Area::Catalog, view! { <ModuleView data=*data/> }), build))
        }
        Answer::Module(None) => None,
    })
}

/// The app document: the shell with an empty page, for a route of the app alone.
pub fn app_document(title: &str, area: Area, build: &str) -> String {
    let body = framed(area, view! { <noscript><p class="state">"Diese Ansicht braucht JavaScript."</p></noscript> });
    document(title, &body, build)
}
