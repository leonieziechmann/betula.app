use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use leptos::prelude::*;
use crate::components::*;
use crate::db::*;
use crate::detail::*;
use crate::models::*;
use crate::program_detail::*;
use crate::storage::*;

use crate::query::{self, Route, ProgramTab};
#[component]
pub fn App() -> impl IntoView {
    let initial_route = web_sys::window().map(|w| query::parse_route(
        &w.location().pathname().unwrap_or_default(), &w.location().search().unwrap_or_default()
    )).unwrap_or(Route::Catalog(FilterOptions::default()));
    let initial_filters = match &initial_route { Route::Catalog(f) => f.clone(), _ => FilterOptions::default() };
    let initial_module = match &initial_route { Route::Course(id) => Some(id.clone()), _ => None };
    let initial_program = match &initial_route { Route::Program(id, _) => Some(id.clone()), _ => None };
    let initial_tab = match &initial_route { Route::Program(_, tab) => *tab, _ => ProgramTab::Plan };
    let (not_found, set_not_found) = signal(initial_route == Route::NotFound);
    let (program_tab, set_program_tab) = signal(initial_tab);

    // Database loading state
    let (db_ready, set_db_ready) = signal(false);
    let (db_progress, set_db_progress) = signal(0);
    let (db_msg, set_db_msg) = signal("Initialisiere SQLite WebAssembly Engine...".to_string());
    let (total_modules_count, set_total_modules_count) = signal(0i64);

    // Sidebar Mobile Drawer State
    let (sidebar_open, set_sidebar_open) = signal(false);

    // Programs and PO versions
    let (all_programs, set_all_programs) = signal(Vec::<ProgramOption>::new());
    let program_combobox_open = RwSignal::new(false);

    // Selected Program State
    let (selected_program_name, set_selected_program_name) = signal("Alle Studiengänge".to_string());
    let (selected_program_id, set_selected_program_id) = signal(String::new());
    let (selected_po_version, set_selected_po_version) = signal(String::new());

    // Filter Options
    let filters = RwSignal::new(initial_filters);

    // Local user data
    let (completed_modules, set_completed_modules) = signal(load_completed());
    let (bookmarked_modules, set_bookmarked_modules) = signal(load_bookmarks());

    // Selected module for detail view (dedicated in-page view)
    let (detail_module_id, set_detail_module_id) = signal(initial_module);

    // Selected study program for detail view (dedicated in-page view)
    let (detail_program_id, set_detail_program_id) = signal(initial_program);

    // Share link toast state
    let (share_toast, set_share_toast) = signal(false);

    // Scroll restoration tracker
    let saved_scroll_y = StoredValue::new(0.0);

    let catalog_link = move || {
        let mut f = filters.get_untracked();
        f.program_id = query::program_slug(&f.program_id, &all_programs.get_untracked());
        query::catalog_url(&f)
    };
    let program_link = move |id: &str, tab: ProgramTab| {
        query::program_url(&query::program_slug(id, &all_programs.get_untracked()), tab)
    };

    // Open module detail view (in-page, replacing catalog view)
    let open_module = move |id: String| {
        if let Some(win) = web_sys::window() {
            saved_scroll_y.set_value(win.scroll_y().unwrap_or(0.0));
            win.scroll_to_with_x_and_y(0.0, 0.0);
            if let Ok(hist) = win.history() {
                let _ = hist.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&query::module_url(&id)));
            }
        }
        set_not_found.set(false);
        set_sidebar_open.set(false);
        set_detail_program_id.set(None);
        set_detail_module_id.set(Some(id));
    };

    // Close module detail view and return to catalog
    let close_module = move || {
        if let Some(win) = web_sys::window() {
            if let Ok(hist) = win.history() {
                let _ = hist.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&catalog_link()));
            }
            let y = saved_scroll_y.get_value();
            win.scroll_to_with_x_and_y(0.0, y);
        }
        set_detail_module_id.set(None);
    };

    // Open study program detail view
    let open_program_page = move |id: String| {
        if let Some(win) = web_sys::window() {
            saved_scroll_y.set_value(win.scroll_y().unwrap_or(0.0));
            win.scroll_to_with_x_and_y(0.0, 0.0);
            if let Ok(hist) = win.history() {
                let _ = hist.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&program_link(&id, ProgramTab::Plan)));
            }
        }
        set_not_found.set(false);
        set_sidebar_open.set(false);
        set_detail_module_id.set(None);
        set_program_tab.set(ProgramTab::Plan);
        set_detail_program_id.set(Some(id));
    };

    // Close study program detail view and return to catalog
    let close_program_page = move || {
        if let Some(win) = web_sys::window() {
            if let Ok(hist) = win.history() {
                let _ = hist.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&catalog_link()));
            }
            let y = saved_scroll_y.get_value();
            win.scroll_to_with_x_and_y(0.0, y);
        }
        set_detail_program_id.set(None);
    };

    let on_program_tab = Callback::new(move |tab: ProgramTab| {
        if let Some(id) = detail_program_id.get_untracked() {
            if let Some(win) = web_sys::window() {
                if let Ok(hist) = win.history() {
                    let _ = hist.push_state_with_url(&JsValue::NULL, "", Some(&program_link(&id, tab)));
                }
            }
            set_program_tab.set(tab);
        }
    });

    // Restore the full route and filter state on browser Back/Forward.
    if let Some(win) = web_sys::window() {
        let on_popstate = Closure::wrap(Box::new(move |_: web_sys::PopStateEvent| {
            if let Some(w) = web_sys::window() {
                let route = query::resolve_route(query::parse_route(&w.location().pathname().unwrap_or_default(), &w.location().search().unwrap_or_default()), &all_programs.get_untracked());
                {
                    set_not_found.set(route == Route::NotFound);
                    match route {
                        Route::Course(id) => { set_detail_program_id.set(None); set_detail_module_id.set(Some(id)); },
                        Route::Program(id, tab) => { set_detail_module_id.set(None); set_program_tab.set(tab); set_detail_program_id.set(Some(id)); },
                        Route::Catalog(f) => { filters.set(f); set_detail_module_id.set(None); set_detail_program_id.set(None); },
                        Route::NotFound => { set_detail_module_id.set(None); set_detail_program_id.set(None); },
                    }
                    set_sidebar_open.set(false);
                }
            }
        }) as Box<dyn FnMut(_)>);
        let _ = win.add_event_listener_with_callback("popstate", on_popstate.as_ref().unchecked_ref());
        on_popstate.forget();
    }

    // Canonical URLs and shareable filters, without creating a history entry per keystroke.
    Effect::new(move |_| {
        let mut f = filters.get();
        if !db_ready.get() || not_found.get() { return; }
        f.program_id = query::program_slug(&f.program_id, &all_programs.get());
        let url = if let Some(id) = detail_program_id.get() {
            program_link(&id, program_tab.get())
        } else if let Some(id) = detail_module_id.get() {
            query::module_url(&id)
        } else { query::catalog_url(&f) };
        if let Some(win) = web_sys::window() {
            if let Ok(hist) = win.history() {
                let _ = hist.replace_state_with_url(&JsValue::NULL, "", Some(&url));
            }
        }
    });

    // Keep the program selector in sync with filters restored from a shared URL.
    Effect::new(move |_| {
        let id = filters.get().program_id;
        let program = all_programs.get().into_iter().find(|p| p.id == id);
        set_selected_program_id.set(id);
        set_selected_program_name.set(program.as_ref().map(|p| p.program_name.clone()).unwrap_or_else(|| "Alle Studiengänge".into()));
        set_selected_po_version.set(program.and_then(|p| p.po_version).unwrap_or_default());
    });
    // Global Escape key listener
    Effect::new(move |_| {
        if let Some(win) = web_sys::window() {
            let on_keydown = Closure::wrap(Box::new(move |evt: web_sys::KeyboardEvent| {
                if evt.key() == "Escape" {
                    if detail_module_id.get().is_some() {
                        close_module();
                    }
                    if detail_program_id.get().is_some() {
                        close_program_page();
                    }
                    set_not_found.set(false);
                    set_sidebar_open.set(false);
                    program_combobox_open.set(false);
                }
            }) as Box<dyn FnMut(_)>);

            let _ = win.add_event_listener_with_callback("keydown", on_keydown.as_ref().unchecked_ref());
            on_keydown.forget();
        }
    });

    // Autocomplete suggestions memo
    let search_suggestions = Memo::new(move |_| {
        let q = filters.get().query;
        if q.trim().len() >= 2 {
            get_autocomplete_suggestions(&q)
        } else {
            Vec::new()
        }
    });

    // Initial database initialization
    Effect::new(move |_| {
        wasm_bindgen_futures::spawn_local(async move {
            let res = init_database(move |pct, msg| {
                set_db_progress.set(pct);
                set_db_msg.set(msg);
            }).await;

            match res {
                Ok(_) => {
                    let total = get_total_count();
                    set_total_modules_count.set(total);
                    let progs = get_all_study_programs();
                    if let Some(id) = detail_program_id.get_untracked() {
                        set_detail_program_id.set(Some(query::resolve_program(&id, &progs)));
                    }
                    filters.update(|f| f.program_id = query::resolve_program(&f.program_id, &progs));
                    set_all_programs.set(progs);
                    set_db_ready.set(true);
                }
                Err(e) => {
                    set_db_msg.set(format!("Fehler: {}", e));
                }
            }
        });
    });

    // Reactive Query Execution
    let filtered_modules = Memo::new(move |_| {
        if !db_ready.get() {
            return Vec::<ModuleCardItem>::new();
        }
        let f = filters.get();
        let comp = completed_modules.get();
        let mut list = query_filtered_modules(&f, &comp);

        if f.is_bookmarks_view {
            let b_set = bookmarked_modules.get();
            list.retain(|m| b_set.contains(&m.id));
        } else if f.is_completed_view {
            let c_set = completed_modules.get();
            list.retain(|m| c_set.contains(&m.id));
        }

        list
    });

    // Split into Regular and FUES modules
    let regular_modules = Memo::new(move |_| {
        filtered_modules.get().into_iter().filter(|m| !m.is_fues_module()).collect::<Vec<_>>()
    });

    let fues_modules = Memo::new(move |_| {
        filtered_modules.get().into_iter().filter(|m| m.is_fues_module()).collect::<Vec<_>>()
    });

    // Toggle Completed
    let toggle_completed = move |id: String| {
        let mut set = completed_modules.get();
        if set.contains(&id) {
            set.remove(&id);
        } else {
            set.insert(id);
        }
        save_completed(&set);
        set_completed_modules.set(set);
    };

    // Toggle Bookmarked
    let toggle_bookmarked = move |id: String| {
        let mut set = bookmarked_modules.get();
        if set.contains(&id) {
            set.remove(&id);
        } else {
            set.insert(id);
        }
        save_bookmarks(&set);
        set_bookmarked_modules.set(set);
    };

    // Program selection handlers
    let on_select_program = Callback::new(move |(id, name, po): (String, String, String)| {
        if id.is_empty() {
            set_selected_program_name.set("Alle Studiengänge".to_string());
            set_selected_program_id.set(String::new());
            set_selected_po_version.set(String::new());
            filters.update(|f| {
                f.program_id = String::new();
                f.semester = None;
            });
        } else {
            set_selected_program_name.set(name);
            set_selected_program_id.set(id.clone());
            set_selected_po_version.set(po);
            filters.update(|f| {
                f.program_id = id;
                f.semester = None;
            });
        }
    });

    let on_clear_program = Callback::new(move |_| {
        set_selected_program_name.set("Alle Studiengänge".to_string());
        set_selected_program_id.set(String::new());
        set_selected_po_version.set(String::new());
        filters.update(|f| {
            f.program_id = String::new();
            f.semester = None;
        });
    });

    // Reset all filters
    let reset_filters = move || {
        on_clear_program.run(());
        filters.set(FilterOptions::default());
    };

    // Active filter count
    let active_filter_count = Memo::new(move |_| {
        let f = filters.get();
        let mut count = 0;
        if !f.query.is_empty() { count += 1; }
        if !f.program_id.is_empty() { count += 1; }
        if f.semester.is_some() { count += 1; }
        if f.module_type != "alle" { count += 1; }
        if !f.prof_includes.is_empty() { count += f.prof_includes.len(); }
        if !f.prof_excludes.is_empty() { count += f.prof_excludes.len(); }
        if !f.department.is_empty() { count += 1; }
        if !f.turnus_all { count += 1; }
        if f.teaching_vorlesung || f.teaching_uebung || f.teaching_praktikum { count += 1; }
        if f.duration != "alle" { count += 1; }
        if f.exam_klausur || f.exam_muendlich || f.exam_beleg { count += 1; }
        if f.grading != "alle" { count += 1; }
        if f.limitation != "ja" { count += 1; }
        if f.fues != "inkl" { count += 1; }
        if f.only_prereqs_met { count += 1; }
        if !f.hide_phase_out { count += 1; }
        if f.min_credits > 0.0 || f.max_credits < 30.0 { count += 1; }
        if f.campus_hauptcampus || f.campus_sachsendorf || f.campus_senftenberg { count += 1; }
        if !f.lang_de || f.lang_en { count += 1; }
        count
    });

    // Copy share link
    let copy_share_link = move || {
        if let Some(win) = web_sys::window() {
            let loc = win.location().href().unwrap_or_default();
            let nav = win.navigator().clipboard();
            let _ = nav.write_text(&loc);
            set_share_toast.set(true);
            wasm_bindgen_futures::spawn_local(async move {
                gloo_timers::future::TimeoutFuture::new(2000).await;
                set_share_toast.set(false);
            });
        }
    };

    // Sorting callback
    let on_sort = Callback::new(move |col: String| {
        filters.update(|f| {
            if f.sort_by == col {
                f.sort_asc = !f.sort_asc;
            } else {
                f.sort_by = col;
                f.sort_asc = true;
            }
        });
    });

    let on_open_module_cb = Callback::new(move |id: String| open_module(id));
    let on_toggle_bkmk_cb = Callback::new(move |id: String| toggle_bookmarked(id));
    let on_toggle_comp_cb = Callback::new(move |id: String| toggle_completed(id));

    view! {
        <AppLayout
            sidebar_open=sidebar_open.into()
            on_close_sidebar=Callback::new(move |_| set_sidebar_open.set(false))
            sidebar=move || view! {
                <FilterSidebar
                    on_close_sidebar=Callback::new(move |_| set_sidebar_open.set(false))
                    all_programs=all_programs.into()
                    total_modules_count=total_modules_count.into()
                    selected_program_id=selected_program_id.into()
                    selected_program_name=selected_program_name.into()
                    selected_po_version=selected_po_version.into()
                    filters=filters
                    program_combobox_open=program_combobox_open
                    on_select_program=on_select_program
                    on_clear_program=on_clear_program
                    on_reset_filters=Callback::new(move |_| reset_filters())
                    active_filter_count=active_filter_count.into()
                    on_open_program_page=Callback::new(move |id: String| open_program_page(id))
                />
            }
            navbar=move || view! {
                <Navbar
                    on_toggle_sidebar=Callback::new(move |_| set_sidebar_open.update(|o| *o = !*o))
                    query=Signal::derive(move || filters.get().query)
                    on_query_change=Callback::new(move |q| filters.update(|f| f.query = q))
                    search_suggestions=search_suggestions.into()
                    on_select_module=on_open_module_cb
                    on_share=Callback::new(move |_| copy_share_link())
                    share_copied=share_toast.into()
                    is_bookmarks_active=Signal::derive(move || filters.get().is_bookmarks_view)
                    bookmarks_count=Signal::derive(move || bookmarked_modules.get().len())
                    on_toggle_bookmarks_view=Callback::new(move |_| {
                        filters.update(|f| {
                            f.is_bookmarks_view = !f.is_bookmarks_view;
                            f.is_completed_view = false;
                        });
                    })
                    is_completed_active=Signal::derive(move || filters.get().is_completed_view)
                    completed_count=Signal::derive(move || completed_modules.get().len())
                    on_toggle_completed_view=Callback::new(move |_| {
                        filters.update(|f| {
                            f.is_completed_view = !f.is_completed_view;
                            f.is_bookmarks_view = false;
                        });
                    })
                />
            }
        >
            // Give each page its own lifetime. Tab/history changes must not
            // dispose signals still referenced by the currently mounted page.
            <For
                each=move || vec![(db_ready.get(), not_found.get(), detail_program_id.get(), detail_module_id.get())]
                key=|page| page.clone()
                children=move |(ready, missing, program, module)| {
                if !ready {
                    // Database Loading Screen
                    view! {
                        <div style="text-align: center; padding: 5rem 2rem; color: var(--text-muted);">
                            <div style="font-size: 2.5rem; margin-bottom: 1rem; animation: bounce 1s infinite;">"⚡"</div>
                            <h3 style="font-size: 1.25rem; font-weight: 700; color: var(--text-main); margin-bottom: 0.5rem;">
                                "Initialisiere BTU Modulkatalog (Client-Side SQLite)"
                            </h3>
                            <p style="font-size: 0.9rem; margin-bottom: 1.5rem;">{move || db_msg.get()}</p>
                            <div style="max-width: 400px; margin: 0 auto; height: 8px; background: #e2e8f0; border-radius: 9999px; overflow: hidden;">
                                <div
                                    style=move || format!("width: {}%; height: 100%; background: var(--primary); transition: width 0.3s ease;", db_progress.get())
                                ></div>
                            </div>
                            <div style="font-size: 0.8rem; margin-top: 0.5rem; font-family: monospace;">
                                {move || format!("{}%", db_progress.get())}
                            </div>
                        </div>
                    }.into_any()
                } else if missing {
                    view! { <div class="program-section-card"><h1>"Seite nicht gefunden"</h1><a href="/catalog">"Zum Modulkatalog"</a></div> }.into_any()
                } else if let Some(prog_id) = program {
                    // Study Program Detail Page
                    view! {
                        <div id="modules-view">
                            <StudyProgramDetailPage
                                program_id=prog_id
                                active_tab=program_tab.into()
                                on_tab_change=on_program_tab
                                on_back=close_program_page
                                on_open_module=open_module
                                on_select_program=move |p_id, p_name, po_ver| {
                                    on_select_program.run((p_id, p_name, po_ver));
                                    close_program_page();
                                }
                                completed_modules=completed_modules.into()
                                bookmarked_modules=bookmarked_modules.into()
                            />
                        </div>
                    }.into_any()
                } else if let Some(mod_id) = module {
                    // Module Detail Page
                    view! {
                        <div id="modules-view">
                            <ModuleDetailPage
                                module_id=mod_id
                                completed_modules=completed_modules.into()
                                bookmarked_modules=bookmarked_modules.into()
                                on_back=close_module
                                on_open_module=open_module
                                on_open_program=open_program_page
                                on_toggle_bookmark=toggle_bookmarked
                                on_toggle_completed=toggle_completed
                            />
                        </div>
                    }.into_any()
                } else {
                    // Catalog Table View
                    (move || {
                    let f = filters.get();
                    let regs = regular_modules.get();
                    let fues = fues_modules.get();
                    let completed = completed_modules.get();
                    let bookmarks = bookmarked_modules.get();
                    let has_regs = !regs.is_empty();
                    let has_fues = !fues.is_empty();

                    view! {
                        <div id="modules-view">
                            // Special View Banners
                            {if f.is_bookmarks_view {
                                view! {
                                    <div class="special-view-banner bookmarks">
                                        <div class="special-view-title">
                                            {format!("⭐ Gemerkte Module ({})", bookmarks.len())}
                                        </div>
                                        <button
                                            type="button"
                                            class="btn-back-search"
                                            on:click=move |_| filters.update(|f| f.is_bookmarks_view = false)
                                        >
                                            "← Zurück zur Suche"
                                        </button>
                                    </div>
                                }.into_any()
                            } else if f.is_completed_view {
                                view! {
                                    <div class="special-view-banner">
                                        <div class="special-view-title">
                                            {format!("✓ Bestandene Module ({})", completed.len())}
                                        </div>
                                        <button
                                            type="button"
                                            class="btn-back-search"
                                            on:click=move |_| filters.update(|f| f.is_completed_view = false)
                                        >
                                            "← Zurück zur Suche"
                                        </button>
                                    </div>
                                }.into_any()
                            } else {
                                view! {}.into_any()
                            }}

                            // Empty state
                            {if !has_regs && !has_fues {
                                view! {
                                    <div class="empty-state-box">
                                        <div class="empty-icon">"🔍"</div>
                                        <h3 class="empty-title">"Keine passenden Module gefunden"</h3>
                                        <p class="empty-desc">
                                            "Für deine aktuellen Filtereinstellungen gibt es keine Treffer. Versuche deine Filter für Semester, Campus oder Voraussetzungen zu lockern."
                                        </p>
                                        <button
                                            type="button"
                                            class="btn-reset-empty"
                                            on:click=move |_| reset_filters()
                                        >
                                            "Filter zurücksetzen"
                                        </button>
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    // Regular Modules Section
                                    {if has_regs {
                                        view! {
                                            <div class="accordion-section" id="accordion-regular">
                                                <div class="accordion-header">
                                                    <div class="accordion-header-left">
                                                        <span class="accordion-title">
                                                            <strong>{regs.len()}</strong> " von " <strong>{total_modules_count.get()}</strong> " Modulen"
                                                            {if !selected_program_id.get().is_empty() {
                                                                view! {
                                                                    <span>" im Studiengang " <strong class="program-highlight">{selected_program_name.get()}</strong></span>
                                                                }.into_any()
                                                            } else {
                                                                view! {
                                                                    <span>" im Gesamtkatalog"</span>
                                                                }.into_any()
                                                            }}
                                                        </span>
                                                        {if !selected_program_id.get().is_empty() {
                                                            let cur_sem = filters.get().semester;
                                                            view! {
                                                                <div style="display: flex; flex-wrap: wrap; gap: 0.35rem; align-items: center; margin-top: 0.5rem;">
                                                                    <span style="font-size: 0.8rem; font-weight: 600; color: var(--text-muted); margin-right: 0.2rem;">"Fachsemester:"</span>
                                                                    {[(None, "Alle"), (Some(1), "1. Sem"), (Some(2), "2. Sem"), (Some(3), "3. Sem"), (Some(4), "4. Sem"), (Some(5), "5. Sem"), (Some(6), "6. Sem"), (Some(0), "Wahlbereich")].into_iter().map(|(sem_val, label)| {
                                                                        let is_active = cur_sem == sem_val;
                                                                        let chip_style = if is_active {
                                                                            "padding: 3px 10px; border-radius: 9999px; font-size: 0.78rem; font-weight: 600; border: 1px solid var(--primary, #005a9c); background: var(--primary, #005a9c); color: #fff; cursor: pointer;"
                                                                        } else {
                                                                            "padding: 3px 10px; border-radius: 9999px; font-size: 0.78rem; font-weight: 500; border: 1px solid #cbd5e1; background: #fff; color: #334155; cursor: pointer;"
                                                                        };
                                                                        view! {
                                                                            <button
                                                                                type="button"
                                                                                style=chip_style
                                                                                on:click=move |_| filters.update(|f| f.semester = sem_val)
                                                                            >
                                                                                {label}
                                                                            </button>
                                                                        }
                                                                    }).collect::<Vec<_>>()}
                                                                </div>
                                                            }.into_any()
                                                        } else {
                                                            view! {}.into_any()
                                                        }}
                                                    </div>
                                                </div>
                                                <div class="accordion-body">
                                                    <ModuleTable
                                                        modules=regular_modules.into()
                                                        completed_modules=completed_modules.into()
                                                        bookmarked_modules=bookmarked_modules.into()
                                                        sort_by=Signal::derive(move || filters.get().sort_by)
                                                        sort_asc=Signal::derive(move || filters.get().sort_asc)
                                                        on_sort=on_sort
                                                        on_open_module=on_open_module_cb
                                                        on_toggle_bookmark=on_toggle_bkmk_cb
                                                        on_toggle_completed=on_toggle_comp_cb
                                                        show_fues_badge=false
                                                        tbody_id="module-table-body"
                                                    />
                                                </div>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! {}.into_any()
                                    }}

                                    // FUES Modules Section
                                    {if has_fues {
                                        view! {
                                            <div class="accordion-section" id="accordion-fues" style=if has_regs { "margin-top: 2rem;" } else { "" }>
                                                <div class="accordion-header">
                                                    <div class="accordion-header-left">
                                                        <span class="accordion-title">
                                                            "🎓 " <strong>{fues.len()}</strong> " von " <strong>{total_modules_count.get()}</strong> " FÜS-Modulen"
                                                        </span>
                                                    </div>
                                                </div>
                                                <div class="accordion-body">
                                                    <ModuleTable
                                                        modules=fues_modules.into()
                                                        completed_modules=completed_modules.into()
                                                        bookmarked_modules=bookmarked_modules.into()
                                                        sort_by=Signal::derive(move || filters.get().sort_by)
                                                        sort_asc=Signal::derive(move || filters.get().sort_asc)
                                                        on_sort=on_sort
                                                        on_open_module=on_open_module_cb
                                                        on_toggle_bookmark=on_toggle_bkmk_cb
                                                        on_toggle_completed=on_toggle_comp_cb
                                                        show_fues_badge=true
                                                        tbody_id="fues-table-body"
                                                    />
                                                </div>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! {}.into_any()
                                    }}
                                }.into_any()
                            }}
                        </div>
                    }.into_any()
                    }).into_any()
                }
            } />
        </AppLayout>
    }
}
