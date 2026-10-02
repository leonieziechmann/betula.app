//! The app of the minimal version: the shell mounted once as the parent route, the catalog's two
//! pages inside it, and one route of the app alone (the Merkliste's place). `start` gets the data
//! worker `boot.js` started while this bundle loaded, asks it for the current page, and takes the
//! page over once the answer is there, so the first render has its data.

use folia_catalog_ui::{ask_of, CatalogPage, ModulePage};
use leptos::lazy;
use folia_data::DataClient;
use folia_shell::{Area, Shell};
use leptos::prelude::*;
use leptos_router::components::{Outlet, ParentRoute, Route, Router, Routes};
use leptos_router::hooks::use_location;
use leptos_router::path;
use wasm_bindgen::prelude::*;

#[component]
fn Layout() -> impl IntoView {
    let location = use_location();
    let area = Memo::new(move |_| Area::of(&location.pathname.get()));
    let status = DataClient::expect().status;
    view! { <Shell area=area status=status><Outlet/></Shell> }
}

/// The module page's code, loaded when a module is first opened (where the bundle is split).
#[lazy]
fn module_page() -> AnyView {
    view! { <ModulePage/> }.into_any()
}

#[component]
fn LazyModulePage() -> impl IntoView {
    view! { <Suspense fallback=|| ()>{Suspend::new(async { module_page().await })}</Suspense> }
}

#[component]
fn Bookmarks() -> impl IntoView {
    view! { <div class="work flowing"><section class="panel"><p class="state">"Die Merkliste: eine Route der App allein (Minimalversion)."</p></section></div> }
}

#[component]
fn App() -> impl IntoView {
    view! {
        <Router>
            <Routes fallback=|| view! { <p class="state">"Nicht gefunden"</p> }>
                <ParentRoute path=path!("") view=Layout>
                    <Route path=path!("/catalog") view=CatalogPage/>
                    <Route path=path!("/catalog/module/:id") view=LazyModulePage/>
                    <Route path=path!("/bookmarks") view=Bookmarks/>
                </ParentRoute>
            </Routes>
        </Router>
    }
}

fn mark(name: &str) {
    if let Some(window) = web_sys::window() {
        let at = window.performance().map_or(0.0, |p| p.now());
        let _ = js_sys::Reflect::set(&window, &name.into(), &at.into());
    }
}

/// Called by `boot.js` with the data worker it started.
#[wasm_bindgen]
pub fn start(worker: web_sys::Worker) {
    console_error_panic_hook::set_once();
    mark("__foliaBundle");
    let client = DataClient::new(worker);
    let Some(location) = web_sys::window().map(|w| w.location()) else { return };
    let path = location.pathname().unwrap_or_default();
    let search = location.search().unwrap_or_default();
    let first = ask_of(&path, search.trim_start_matches('?'));
    // Leptos's executor starts with the mount; until then the browser's.
    wasm_bindgen_futures::spawn_local(async move {
        // The page's data first, then the takeover: no skeleton over a page the server finished.
        if let Some(ask) = first {
            let _ = client.ask(ask).await;
        }
        if let Some(body) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.body()) {
            body.set_inner_html("");
        }
        let provided = client.clone();
        leptos::mount::mount_to_body(move || {
            provide_context(provided.clone());
            view! { <App/> }
        });
        mark("__foliaReady");
    });
}
