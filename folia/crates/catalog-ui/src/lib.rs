//! The catalog's list and the module page. `ListView` and `ModuleView` are markup over the data a
//! loader returns, rendered alike by the site and by the app; `CatalogPage` and `ModulePage` (feature
//! `web`) are the app's pages, which ask the data worker for it.

use catalog::pages::{CatalogData, ModuleData};
use catalog::url;
use folia_design::Frame;
use leptos::prelude::*;

pub use folia_pages::Ask;

/// One page of the unfiltered catalog.
#[component]
pub fn ListView(data: CatalogData) -> impl IntoView {
    let page = data.page;
    let size = url::PAGE_SIZE;
    let number = page.offset / size + 1;
    let last = page.total.div_ceil(size).max(1);
    view! {
        <Frame title="Module" sidebar=move || view! { <p class="cat-note">{format!("{} Module", page.total)}</p> }>
            <section class="panel list cat-list">
                <div class="count-row"><b class="num">{page.total}</b>" Module · Seite "{number}" von "{last}</div>
                <div class="rows">
                    {page.rows.into_iter().map(|row| view! {
                        <div class="row-wrap">
                            <a class="row" href=url::module_path(&row.id) data-id=row.id.clone()>
                                <div class="t"><b>{row.title}</b><small><span class="mono">{row.id.clone()}</span></small></div>
                                <span class="lp num">{row.credits.map(|c| format!("{c}"))}<small>" LP"</small></span>
                            </a>
                        </div>
                    }).collect_view()}
                </div>
                <nav class="cat-pager">
                    {(number > 1).then(|| view! { <a href=format!("/catalog?page={}", number - 1)>"← zurück"</a> })}
                    {(number < last).then(|| view! { <a href=format!("/catalog?page={}", number + 1)>"weiter →"</a> })}
                </nav>
            </section>
        </Frame>
    }
}

/// A module's page.
#[component]
pub fn ModuleView(data: ModuleData) -> impl IntoView {
    let module = data.module;
    let contents = module.contents.as_deref().map(catalog::text::plain);
    view! {
        <Frame title="Modul" sidebar=move || view! { <p class="cat-note"><a href="/catalog">"← Katalog"</a></p> }>
            <article class="panel detail cat-module">
                <header class="hero">
                    <h2>{module.title}</h2>
                    <p><span class="mono">{module.id}</span>{module.credits.map(|c| format!(" · {c} LP"))}</p>
                </header>
                {contents.map(|text| view! { <p class="cat-text">{text}</p> })}
                <p class="cat-note">{format!("{} Termine, {} Prüfungen, {} Studiengänge", data.schedule.len(), data.exams.len(), data.programs.len())}</p>
            </article>
        </Frame>
    }
}

/// What the route `/catalog?page=<n>` and `/catalog/module/<id>` ask for.
pub fn ask_of(path: &str, query: &str) -> Option<Ask> {
    if path == "/catalog" {
        let page = url::parse_pairs(query).into_iter().find(|(k, _)| k == "page").and_then(|(_, v)| v.parse().ok()).unwrap_or(1);
        return Some(Ask::Catalog { page });
    }
    path.strip_prefix("/catalog/module/").map(|id| Ask::Module { id: id.to_string() })
}

#[cfg(feature = "web")]
mod web {
    use super::*;
    use folia_data::{use_ask, Loaded};
    use folia_design::SkeletonRows;
    use folia_pages::Answer;
    use leptos_router::hooks::{use_location, use_params_map};

    /// What a page shows of its data: the data, its error, its skeleton after the threshold, or
    /// nothing in the moment before that.
    fn shown(loaded: Loaded, page: impl Fn(Answer) -> AnyView + Send + Sync + 'static) -> impl IntoView {
        move || match loaded.value.get() {
            Some(Ok(answer)) => page(answer),
            Some(Err(error)) => view! { <p class="state">{error}</p> }.into_any(),
            None if loaded.slow.get() => view! { <SkeletonRows count=12/> }.into_any(),
            None => ().into_any(),
        }
    }

    #[component]
    pub fn CatalogPage() -> impl IntoView {
        let location = use_location();
        let loaded = use_ask(move || ask_of("/catalog", &location.search.get()).unwrap_or(Ask::Catalog { page: 1 }));
        shown(loaded, |answer| match answer {
            Answer::Catalog(data) => view! { <ListView data=*data/> }.into_any(),
            _ => ().into_any(),
        })
    }

    #[component]
    pub fn ModulePage() -> impl IntoView {
        let params = use_params_map();
        let loaded = use_ask(move || Ask::Module { id: params.with(|p| p.get("id").unwrap_or_default()) });
        shown(loaded, |answer| match answer {
            Answer::Module(Some(data)) => view! { <ModuleView data=*data/> }.into_any(),
            Answer::Module(None) => view! { <p class="state">"Dieses Modul kennt der Katalog nicht."</p> }.into_any(),
            _ => ().into_any(),
        })
    }
}

#[cfg(feature = "web")]
pub use web::{CatalogPage, ModulePage};
