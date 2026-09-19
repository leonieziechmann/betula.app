use leptos::prelude::*;
use crate::models::ModuleCardItem;

/// Sticky Navbar with mobile sidebar toggle, search autocomplete with keyboard support, and action buttons
#[component]
pub fn Navbar(
    on_toggle_sidebar: Callback<()>,
    query: Signal<String>,
    on_query_change: Callback<String>,
    search_suggestions: Signal<Vec<ModuleCardItem>>,
    on_select_module: Callback<String>,
    on_share: Callback<()>,
    share_copied: Signal<bool>,
    is_bookmarks_active: Signal<bool>,
    bookmarks_count: Signal<usize>,
    on_toggle_bookmarks_view: Callback<()>,
    is_completed_active: Signal<bool>,
    completed_count: Signal<usize>,
    on_toggle_completed_view: Callback<()>,
) -> impl IntoView {
    let (search_focused, set_search_focused) = signal(false);
    let (highlighted_suggestion, set_highlighted_suggestion) = signal(-1i32);

    let on_input = {
        let on_query_change = on_query_change;
        move |ev| {
            let val = event_target_value(&ev);
            on_query_change.run(val);
            set_highlighted_suggestion.set(-1);
        }
    };

    let on_keydown = {
        let search_suggestions = search_suggestions;
        let on_select_module = on_select_module;
        move |ev: web_sys::KeyboardEvent| {
            let suggs = search_suggestions.get();
            if !suggs.is_empty() {
                match ev.key().as_str() {
                    "ArrowDown" => {
                        ev.prevent_default();
                        set_highlighted_suggestion.update(|idx| {
                            *idx = (*idx + 1).min(suggs.len() as i32 - 1);
                        });
                    }
                    "ArrowUp" => {
                        ev.prevent_default();
                        set_highlighted_suggestion.update(|idx| {
                            *idx = (*idx - 1).max(-1);
                        });
                    }
                    "Enter" => {
                        let cur = highlighted_suggestion.get();
                        if cur >= 0 && (cur as usize) < suggs.len() {
                            ev.prevent_default();
                            on_select_module.run(suggs[cur as usize].id.clone());
                        }
                    }
                    "Escape" => {
                        set_search_focused.set(false);
                    }
                    _ => {}
                }
            }
        }
    };

    view! {
        <header class="navbar">
            <div class="navbar-left">
                <button
                    type="button"
                    class="btn-sidebar-toggle"
                    on:click=move |_| on_toggle_sidebar.run(())
                >
                    <span class="toggle-icon">"☰"</span>
                    <span class="toggle-label">"Filter"</span>
                </button>
                <a href="/catalogue" class="mobile-brand">
                    <span class="brand-badge">"BTU"</span>
                    <span class="brand-mobile-title">"Modulkatalog"</span>
                </a>
            </div>

            // Search input with autocomplete
            <div class="nav-search" style="position: relative;">
                <span class="search-icon">"🔍"</span>
                <input
                    type="search"
                    class="search-input"
                    placeholder="Modulname, Modulnummer (z. B. 11101), Dozent oder Stichwort..."
                    prop:value=query
                    on:focus=move |_| set_search_focused.set(true)
                    on:blur=move |_| {
                        wasm_bindgen_futures::spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(200).await;
                            set_search_focused.set(false);
                        });
                    }
                    on:input=on_input
                    on:keydown=on_keydown
                />
                {move || {
                    let suggs = search_suggestions.get();
                    if search_focused.get() && !suggs.is_empty() {
                        view! {
                            <div class="search-suggestions" id="search-suggestions" style="display: block; position: absolute; top: 100%; left: 0; right: 0; z-index: 1000;">
                                {suggs.into_iter().enumerate().map(|(idx, s)| {
                                    let s_id = s.id.clone();
                                    let is_highlighted = move || highlighted_suggestion.get() == idx as i32;
                                    let on_select = on_select_module;
                                    view! {
                                        <div
                                            class=move || format!("suggestion-item {}", if is_highlighted() { "highlighted" } else { "" })
                                            on:mousedown=move |_| on_select.run(s_id.clone())
                                        >
                                            <div class="suggestion-main">
                                                <span class="badge badge-id">{s.id.clone()}</span>
                                                <strong class="suggestion-title">{s.title_de.clone()}</strong>
                                            </div>
                                            <div class="suggestion-meta">
                                                <span>{s.formatted_credits()} " ECTS"</span>
                                                <span>{s.formatted_turnus()}</span>
                                            </div>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        }.into_any()
                    } else {
                        ().into_any()
                    }
                }}
            </div>

            // Navbar Action Buttons
            <div class="nav-actions">
                <button
                    type="button"
                    class="btn-icon-text"
                    on:click=move |_| on_share.run(())
                    title="Link teilen"
                >
                    <span>"🔗 " <span class="nav-action-label">{move || if share_copied.get() { "Kopiert!" } else { "Teilen" }}</span></span>
                </button>

                <button
                    type="button"
                    class=move || if is_bookmarks_active.get() { "btn-icon-text active" } else { "btn-icon-text" }
                    on:click=move |_| on_toggle_bookmarks_view.run(())
                    title="Alle gemerkten Module anzeigen"
                >
                    <span>"⭐ " <span class="nav-action-label">"Gemerkt"</span></span>
                    <span class="badge-count">{move || bookmarks_count.get()}</span>
                </button>

                <button
                    type="button"
                    class=move || if is_completed_active.get() { "btn-icon-text active" } else { "btn-icon-text" }
                    on:click=move |_| on_toggle_completed_view.run(())
                    title="Alle bestandenen Module anzeigen"
                >
                    <span>"✓ " <span class="nav-action-label">"Bestanden"</span></span>
                    <span class="badge-count">{move || completed_count.get()}</span>
                </button>
            </div>
        </header>
    }
}
