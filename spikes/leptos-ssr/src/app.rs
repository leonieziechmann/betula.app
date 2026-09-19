//! Spike: the same components render on the server (complete HTML, no JS needed) and
//! hydrate in the browser. Data comes from the shared `catalog` crate.
use catalog::filter::{ProgramRelation, TurnusFilter};
use catalog::labels::{TeachingForm, KIND_UNKNOWN};
use catalog::rows::{CatalogPage, Module, Program, ProgramModule};
use catalog::CatalogQuery;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Title};
use leptos_router::components::{Form, Route, Router, Routes, A};
use leptos_router::hooks::{use_params_map, use_query_map};
use leptos_router::path;
use serde::{Deserialize, Serialize};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="de">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[cfg(feature = "ssr")]
fn open() -> Result<catalog::native::NativeDatabase, ServerFnError> {
    let path = std::env::var("BTU_SNAPSHOT").map_err(|_| ServerFnError::new("BTU_SNAPSHOT is not set"))?;
    catalog::native::NativeDatabase::open(std::path::Path::new(&path)).map_err(|e| ServerFnError::new(e.to_string()))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramData {
    pub program: Program,
    pub curricular: Vec<ProgramModule>,
    pub fues: Vec<ProgramModule>,
}

#[server]
pub async fn load_programs() -> Result<Vec<Program>, ServerFnError> {
    let db = open()?;
    catalog::queries::programs(&db).map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn load_program(slug: String) -> Result<Option<ProgramData>, ServerFnError> {
    let db = open()?;
    let fail = |e: catalog::DbError| ServerFnError::new(e.to_string());
    let Some(program) = catalog::queries::program_by_slug(&db, &slug).map_err(fail)? else { return Ok(None) };
    let curricular = catalog::queries::program_modules(&db, &program.id, ProgramRelation::Curricular).map_err(fail)?;
    let fues = catalog::queries::program_modules(&db, &program.id, ProgramRelation::Fues).map_err(fail)?;
    Ok(Some(ProgramData { program, curricular, fues }))
}

#[server]
pub async fn load_module(id: String) -> Result<Option<Module>, ServerFnError> {
    let db = open()?;
    catalog::queries::module(&db, &id).map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn load_catalog(text: String, winter: bool, exercise: bool, offset: u64) -> Result<CatalogPage, ServerFnError> {
    let db = open()?;
    let query = CatalogQuery {
        text,
        turnus: TurnusFilter { winter, ..Default::default() },
        teaching_forms: if exercise { vec![TeachingForm::Exercise] } else { vec![] },
        ..Default::default()
    };
    catalog::queries::catalog_page(&db, &query, offset, 50).map_err(|e| ServerFnError::new(e.to_string()))
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    view! {
        <Router>
            <nav>
                <A href="/katalog">"Katalog"</A>" · "<A href="/studiengaenge">"Studiengänge"</A>
            </nav>
            <main>
                <Routes fallback=|| view! { <h1>"Seite nicht gefunden"</h1> }>
                    <Route path=path!("/") view=CatalogPageView/>
                    <Route path=path!("/katalog") view=CatalogPageView/>
                    <Route path=path!("/studiengaenge") view=ProgramsPage/>
                    <Route path=path!("/studiengang/:slug") view=ProgramPage/>
                    <Route path=path!("/studiengang/:slug/:tab") view=ProgramPage/>
                    <Route path=path!("/module/:id") view=ModulePage/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn ProgramsPage() -> impl IntoView {
    let programs = Resource::new(|| (), |_| load_programs());
    view! {
        <Title text="Studiengänge"/>
        <h1>"Studiengänge"</h1>
        <Suspense fallback=|| view! { <p>"Lädt…"</p> }>
            {move || Suspend::new(async move {
                match programs.await {
                    Err(e) => view! { <p class="error">{e.to_string()}</p> }.into_any(),
                    Ok(list) => view! {
                        <p id="total">{list.len()}" Studiengänge"</p>
                        <ul id="programs">
                            {list.into_iter().map(|p| view! {
                                <li><A href=format!("/studiengang/{}", p.slug)>{p.name.clone()}" ("{p.degree().to_string()}", PO "{p.po_version.clone()}")"</A></li>
                            }).collect_view()}
                        </ul>
                    }.into_any(),
                }
            })}
        </Suspense>
    }
}

#[component]
fn ProgramPage() -> impl IntoView {
    // Navigation state comes from the router; the page stays mounted when the slug changes.
    let params = use_params_map();
    let slug = Memo::new(move |_| params.read().get("slug").unwrap_or_default());
    let tab = Memo::new(move |_| params.read().get("tab").unwrap_or_else(|| "module".to_string()));
    let data = Resource::new(move || slug.get(), load_program);

    // Page-local UI state plus an effect that reads it together with navigation state:
    // the combination that crashed in the old app.
    let area = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        let _ = (slug.get(), tab.get(), area.get());
        #[cfg(feature = "hydrate")]
        web_sys::console::log_1(&format!("program effect {} {}", slug.get_untracked(), tab.get_untracked()).into());
    });
    // A new program starts without an area filter.
    Effect::new(move |_| {
        slug.track();
        area.set(None);
    });

    view! {
        <Suspense fallback=|| view! { <p>"Lädt…"</p> }>
            {move || Suspend::new(async move {
                match data.await {
                    Err(e) => view! { <p class="error">{e.to_string()}</p> }.into_any(),
                    Ok(None) => view! { <h1>"Studiengang nicht gefunden"</h1> }.into_any(),
                    Ok(Some(d)) => {
                        let base = format!("/studiengang/{}", d.program.slug);
                        let areas: Vec<String> = {
                            let mut a: Vec<String> = d.curricular.iter().filter_map(|m| m.area.clone()).collect();
                            a.sort(); a.dedup(); a
                        };
                        let curricular = d.curricular.clone();
                        let fues = d.fues.clone();
                        view! {
                            <Title text=d.program.name.clone()/>
                            <h1 id="program-name">{d.program.name.clone()}" · "{d.program.degree().to_string()}</h1>
                            <p id="counts">{d.curricular.len()}" Module im Curriculum · "{d.fues.len()}" FÜS-Module"</p>
                            <p class="tabs">
                                <A href=format!("{base}/plan")>"Regelstudienplan"</A>" | "
                                <A href=format!("{base}/bereiche")>"Bereiche"</A>" | "
                                <A href=format!("{base}/module")>"Alle Module"</A>
                            </p>
                            <p id="chips">
                                {areas.into_iter().take(6).map(|a| {
                                    let value = a.clone();
                                    view! { <button class="chip" on:click=move |_| area.set(Some(value.clone()))>{a}</button> }
                                }).collect_view()}
                            </p>
                            <h2 id="tab">{move || tab.get()}</h2>
                            <ul id="modules">
                                {move || {
                                    let selected = area.get();
                                    let list = if tab.get() == "fues" { fues.clone() } else { curricular.clone() };
                                    list.into_iter()
                                        .filter(|m| selected.is_none() || m.area == selected)
                                        .map(|m| view! {
                                            <li>
                                                <A href=format!("/module/{}", m.module_id)>{m.module_id.clone()}" "{m.module_title.clone()}</A>
                                                " – "{m.kind.as_ref().map(|k| k.label().to_string()).unwrap_or_else(|| KIND_UNKNOWN.to_string())}
                                            </li>
                                        })
                                        .collect_view()
                                }}
                            </ul>
                        }.into_any()
                    }
                }
            })}
        </Suspense>
    }
}

#[component]
fn ModulePage() -> impl IntoView {
    let params = use_params_map();
    let id = Memo::new(move |_| params.read().get("id").unwrap_or_default());
    let module = Resource::new(move || id.get(), load_module);
    view! {
        <Suspense fallback=|| view! { <p>"Lädt…"</p> }>
            {move || Suspend::new(async move {
                match module.await {
                    Err(e) => view! { <p class="error">{e.to_string()}</p> }.into_any(),
                    Ok(None) => view! { <h1>"Modul nicht gefunden"</h1> }.into_any(),
                    Ok(Some(m)) => view! {
                        <Title text=m.title.clone()/>
                        <h1 id="module-title">{m.id.clone()}" "{m.title.clone()}</h1>
                        <p>{m.credits.map(|c| format!("{c} LP")).unwrap_or_else(|| "LP nicht angegeben".into())}
                           " · "{m.turnus_season.as_ref().map(|t| t.label().to_string()).unwrap_or_else(|| "Turnus nicht angegeben".into())}</p>
                        <p>{m.contents.clone().unwrap_or_default()}</p>
                    }.into_any(),
                }
            })}
        </Suspense>
    }
}

/// The catalog as a GET form: works without JavaScript, and with it the router turns the
/// same form into client-side navigation. The URL is the only filter state.
#[component]
fn CatalogPageView() -> impl IntoView {
    let query = use_query_map();
    let text = Memo::new(move |_| query.read().get("q").unwrap_or_default());
    let winter = Memo::new(move |_| query.read().get("winter").is_some());
    let exercise = Memo::new(move |_| query.read().get("uebung").is_some());
    let offset = Memo::new(move |_| query.read().get("ab").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0));
    let page = Resource::new(
        move || (text.get(), winter.get(), exercise.get(), offset.get()),
        |(text, winter, exercise, offset)| load_catalog(text, winter, exercise, offset),
    );
    view! {
        <Title text="Modulkatalog"/>
        <h1>"Modulkatalog"</h1>
        <Form method="GET" action="/katalog">
            <input type="search" name="q" prop:value=move || text.get() placeholder="Suche"/>
            <label><input type="checkbox" name="winter" value="1" prop:checked=move || winter.get()/>"Wintersemester"</label>
            <label><input type="checkbox" name="uebung" value="1" prop:checked=move || exercise.get()/>"mit Übung"</label>
            <button type="submit">"Filtern"</button>
        </Form>
        <Suspense fallback=|| view! { <p>"Lädt…"</p> }>
            {move || Suspend::new(async move {
                match page.await {
                    Err(e) => view! { <p class="error">{e.to_string()}</p> }.into_any(),
                    Ok(p) => view! {
                        <p id="total">{p.total}" Module"</p>
                        <ul id="rows">
                            {p.rows.into_iter().map(|r| view! {
                                <li><A href=format!("/module/{}", r.id)>{r.id.clone()}" "{r.title.clone()}</A></li>
                            }).collect_view()}
                        </ul>
                    }.into_any(),
                }
            })}
        </Suspense>
    }
}
