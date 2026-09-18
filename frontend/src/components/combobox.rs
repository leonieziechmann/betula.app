use leptos::prelude::*;
use crate::fuzzy::{self, FuzzyConfig};

#[derive(Clone, Debug, PartialEq)]
pub struct ComboboxItem {
    pub id: String,
    pub label: String,
    pub badge: Option<String>,
}

impl ComboboxItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>, badge: Option<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            badge,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ComboboxOptionEntry {
    item: ComboboxItem,
    is_default: bool,
    score: i32,
}

/// Calculates a fuzzy match score for query against target using the generalized fuzzy search engine.
/// Higher score indicates a better match.
pub fn fuzzy_score(query: &str, target: &str) -> Option<i32> {
    fuzzy_score_with_config(query, target, &FuzzyConfig::combobox())
}

/// Calculates a fuzzy match score using a custom FuzzyConfig.
pub fn fuzzy_score_with_config(query: &str, target: &str, config: &FuzzyConfig) -> Option<i32> {
    fuzzy::fuzzy_score(query, target, config).map(|s| s.round() as i32)
}

fn scroll_index_into_view(options_ref: NodeRef<leptos::html::Div>, index: usize) {
    if let Some(container) = options_ref.get() {
        if let Ok(Some(target)) = container.query_selector(&format!("[data-index=\"{}\"]", index)) {
            let obj = js_sys::Object::new();
            let _ = js_sys::Reflect::set(&obj, &"block".into(), &"nearest".into());
            if let Ok(scroll_fn) = js_sys::Reflect::get(&target, &"scrollIntoView".into()) {
                if scroll_fn.is_function() {
                    let _ = js_sys::Reflect::apply(
                        &scroll_fn.into(),
                        &target,
                        &js_sys::Array::of1(&obj),
                    );
                }
            }
        }
    }
}

/// Generic Searchable Combobox with dropdown, fuzzy filtering, badges, parent anchoring, and clear button
#[component]
pub fn Combobox(
    selected_label: Signal<String>,
    has_selection: Signal<bool>,
    items: Signal<Vec<ComboboxItem>>,
    #[prop(default = "Auswählen...")] placeholder: &'static str,
    #[prop(default = "Suchen...")] search_placeholder: &'static str,
    #[prop(optional)] default_item_label: Option<&'static str>,
    #[prop(optional)] default_item_badge: Option<Signal<String>>,
    is_open: RwSignal<bool>,
    on_select: Callback<ComboboxItem>,
    on_clear: Callback<()>,
    #[prop(optional, into)] container_class: Option<String>,
    #[prop(optional)] anchor_to_parent: bool,
    #[prop(optional)] fuzzy_config: Option<FuzzyConfig>,
) -> impl IntoView {
    let (search_query, set_search_query) = signal(String::new());
    let highlighted_index = RwSignal::new(0usize);
    let input_ref = NodeRef::<leptos::html::Input>::new();
    let options_ref = NodeRef::<leptos::html::Div>::new();
    let active_cfg = fuzzy_config.unwrap_or_else(FuzzyConfig::combobox);

    let mut classes = vec!["combobox-container".to_string()];
    if anchor_to_parent {
        classes.push("anchor-parent".to_string());
    }
    if let Some(extra) = container_class {
        classes.push(extra);
    }
    let container_cls = classes.join(" ");

    // Compute filtered & fuzzy ranked items
    let filtered_items = Memo::new(move |_| {
        let q = search_query.get();
        let q_trimmed = q.trim();
        let mut entries = Vec::new();

        if q_trimmed.is_empty() {
            if let Some(label) = default_item_label {
                entries.push(ComboboxOptionEntry {
                    item: ComboboxItem::new("", label, None),
                    is_default: true,
                    score: 0,
                });
            }
            for item in items.get() {
                entries.push(ComboboxOptionEntry {
                    item,
                    is_default: false,
                    score: 0,
                });
            }
        } else {
            if let Some(label) = default_item_label {
                if let Some(score) = fuzzy_score_with_config(q_trimmed, label, &active_cfg) {
                    entries.push(ComboboxOptionEntry {
                        item: ComboboxItem::new("", label, None),
                        is_default: true,
                        score,
                    });
                }
            }
            for item in items.get() {
                if let Some(score) = fuzzy_score_with_config(q_trimmed, &item.label, &active_cfg) {
                    entries.push(ComboboxOptionEntry {
                        item,
                        is_default: false,
                        score,
                    });
                }
            }
            // Sort by score descending (highest score first)
            entries.sort_by(|a, b| b.score.cmp(&a.score));
        }

        entries
    });

    // Focus, select search input, and scroll to currently selected item ONLY when combobox opens
    Effect::new(move |_| {
        if is_open.get() {
            untrack(move || {
                // Reset search query to empty on open so full list is visible
                set_search_query.set(String::new());

                let cur_label = selected_label.get();
                let has_sel = has_selection.get();

                let initial_idx = filtered_items.with(|list| {
                    list.iter().position(|entry| {
                        if entry.is_default {
                            !has_sel || cur_label.is_empty() || cur_label == entry.item.label
                        } else {
                            has_sel && cur_label == entry.item.label
                        }
                    }).unwrap_or(0)
                });

                highlighted_index.set(initial_idx);

                request_animation_frame(move || {
                    if let Some(input) = input_ref.get() {
                        let _ = input.focus();
                        input.select();
                    }
                    scroll_index_into_view(options_ref, initial_idx);
                });

                gloo_timers::callback::Timeout::new(25, move || {
                    if let Some(input) = input_ref.get() {
                        let _ = input.focus();
                        input.select();
                    }
                    scroll_index_into_view(options_ref, initial_idx);
                }).forget();
            });
        }
    });

    view! {
        <div class=container_cls>
            <div
                class="combobox-display"
                class:active=move || is_open.get()
                tabindex="0"
                on:click=move |_| is_open.update(|o| *o = !*o)
                on:keydown=move |ev: web_sys::KeyboardEvent| {
                    match ev.key().as_str() {
                        "ArrowDown" | "Enter" | " " if !is_open.get() => {
                            ev.prevent_default();
                            is_open.set(true);
                        }
                        _ => {}
                    }
                }
            >
                <span style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                    {move || {
                        let text = selected_label.get();
                        if text.is_empty() { placeholder.to_string() } else { text }
                    }}
                </span>
                <div style="display: flex; align-items: center; gap: 0.3rem;">
                    {move || if has_selection.get() {
                        view! {
                            <button
                                type="button"
                                class="btn-clear-prog"
                                on:click=move |ev| {
                                    ev.stop_propagation();
                                    on_clear.run(());
                                }
                                title="Auswahl aufheben"
                            >
                                "×"
                            </button>
                        }.into_any()
                    } else {
                        view! {}.into_any()
                    }}
                    <span style="font-size: 0.7rem; color: var(--text-muted);">"▼"</span>
                </div>
            </div>

            {move || if is_open.get() {
                let badge_sig = default_item_badge;
                let on_select = on_select;

                view! {
                    <div class="combobox-dropdown" style="display: flex; flex-direction: column;">
                        <div class="combobox-search-wrap">
                            <input
                                node_ref=input_ref
                                type="text"
                                class="combobox-search-input"
                                placeholder=search_placeholder
                                prop:value=search_query
                                on:input=move |ev| {
                                    set_search_query.set(event_target_value(&ev));
                                    highlighted_index.set(0);
                                }
                                on:click=move |ev| ev.stop_propagation()
                                on:focus=move |ev| {
                                    let _ = event_target::<web_sys::HtmlInputElement>(&ev).select();
                                }
                                on:keydown=move |ev: web_sys::KeyboardEvent| {
                                    match ev.key().as_str() {
                                        "ArrowDown" => {
                                            ev.prevent_default();
                                            let len = filtered_items.get().len();
                                            if len > 0 {
                                                let next = (highlighted_index.get() + 1) % len;
                                                highlighted_index.set(next);
                                                scroll_index_into_view(options_ref, next);
                                            }
                                        }
                                        "ArrowUp" => {
                                            ev.prevent_default();
                                            let len = filtered_items.get().len();
                                            if len > 0 {
                                                let current = highlighted_index.get();
                                                let next = if current == 0 { len - 1 } else { current - 1 };
                                                highlighted_index.set(next);
                                                scroll_index_into_view(options_ref, next);
                                            }
                                        }
                                        "Enter" => {
                                            ev.prevent_default();
                                            let list = filtered_items.get();
                                            let idx = highlighted_index.get();
                                            if let Some(entry) = list.get(idx).or_else(|| list.first()) {
                                                on_select.run(entry.item.clone());
                                                is_open.set(false);
                                            }
                                        }
                                        "Escape" => {
                                            ev.prevent_default();
                                            is_open.set(false);
                                        }
                                        _ => {}
                                    }
                                }
                            />
                        </div>
                        <div node_ref=options_ref class="combobox-options">
                            {move || {
                                let list = filtered_items.get();
                                let curr_highlight = highlighted_index.get();
                                let cur_label = selected_label.get();
                                let has_sel = has_selection.get();

                                if list.is_empty() {
                                    return view! {
                                        <div class="combobox-no-results" style="padding: 0.75rem; text-align: center; color: var(--text-muted); font-size: 0.82rem;">
                                            "Keine Ergebnisse gefunden"
                                        </div>
                                    }.into_any();
                                }

                                list.into_iter().enumerate().map(|(idx, entry)| {
                                    let item_click = entry.item.clone();
                                    let is_highlighted = curr_highlight == idx;
                                    let is_def = entry.is_default;
                                    let badge_opt = entry.item.badge.clone();
                                    let label = entry.item.label.clone();
                                    let is_selected = if is_def {
                                        !has_sel || cur_label.is_empty() || cur_label == label
                                    } else {
                                        has_sel && cur_label == label
                                    };

                                    view! {
                                        <div
                                            class="combobox-option"
                                            class:selected=is_selected
                                            class:highlighted=is_highlighted
                                            data-index=idx.to_string()
                                            on:mouseenter=move |_| highlighted_index.set(idx)
                                            on:click=move |_| {
                                                on_select.run(item_click.clone());
                                                is_open.set(false);
                                            }
                                        >
                                            <span class="combobox-option-label">{label}</span>
                                            {if is_def {
                                                if let Some(sig) = badge_sig {
                                                    view! { <span class="badge badge-id">{move || sig.get()}</span> }.into_any()
                                                } else {
                                                    view! {}.into_any()
                                                }
                                            } else if let Some(badge) = badge_opt {
                                                view! { <span class="badge badge-id">{badge}</span> }.into_any()
                                            } else {
                                                view! {}.into_any()
                                            }}
                                        </div>
                                    }
                                }).collect::<Vec<_>>().into_any()
                            }}
                        </div>
                    </div>
                }.into_any()
            } else {
                view! {}.into_any()
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_score_empty() {
        assert_eq!(fuzzy_score("", "Informatik"), Some(0));
        assert_eq!(fuzzy_score("   ", "Informatik"), Some(0));
    }

    #[test]
    fn test_fuzzy_score_exact() {
        let score = fuzzy_score("Informatik", "Informatik");
        assert!(score.is_some());
        assert_eq!(score.unwrap(), 2000);
    }

    #[test]
    fn test_fuzzy_score_matching() {
        let score_prefix = fuzzy_score("inf", "Informatik (B.Sc.)");
        let score_mid = fuzzy_score("inf", "Medizinische Informatik (M.Sc.)");
        let score_none = fuzzy_score("xyz", "Informatik");

        assert!(score_prefix.is_some());
        assert!(score_mid.is_some());
        assert!(score_none.is_none());

        // Prefix match on title should score higher than match in second word
        assert!(score_prefix.unwrap() > score_mid.unwrap());
    }

    #[test]
    fn test_fuzzy_score_word_boundary() {
        let score_acronym = fuzzy_score("ai", "Artificial Intelligence");
        assert!(score_acronym.is_some());
    }
}

