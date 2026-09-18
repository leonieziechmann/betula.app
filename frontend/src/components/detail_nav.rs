use leptos::prelude::*;

/// An item in the breadcrumb navigation trail.
#[derive(Clone)]
pub struct BreadcrumbItem {
    pub label: String,
    pub on_click: Option<Callback<()>>,
    pub is_current: bool,
}

impl BreadcrumbItem {
    /// Create a clickable breadcrumb link.
    pub fn link(label: impl Into<String>, on_click: impl Into<Callback<()>>) -> Self {
        Self {
            label: label.into(),
            on_click: Some(on_click.into()),
            is_current: false,
        }
    }

    /// Create a non-clickable text item in the trail.
    pub fn static_text(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            on_click: None,
            is_current: false,
        }
    }

    /// Create the current (active / terminal) breadcrumb item.
    pub fn current(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            on_click: None,
            is_current: true,
        }
    }
}

/// Unified top navigation bar for detail views (e.g. Module, Study Program, etc.).
/// Provides:
/// 1. Back button with custom text and optional shortcut label (e.g. "Esc")
/// 2. Linkable breadcrumb trail
/// 3. Right-aligned action buttons slot
#[component]
pub fn DetailNavBar(
    #[prop(optional, into)] back_text: Option<String>,
    #[prop(optional, into)] shortcut_label: Option<String>,
    #[prop(optional)] on_back: Option<Callback<()>>,
    #[prop(optional, default = true)] show_back: bool,
    #[prop(optional)] breadcrumbs: Option<Vec<BreadcrumbItem>>,
    #[prop(optional, into)] class: Option<String>,
    #[prop(optional)] actions: Option<Children>,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    let text = back_text.unwrap_or_else(|| "Zurück zum Katalog".to_string());
    let shortcut = shortcut_label.or_else(|| Some("Esc".to_string()));
    let crumbs = breadcrumbs.unwrap_or_default();
    let base_class = class.unwrap_or_default();
    let container_class = if base_class.is_empty() {
        "detail-nav-bar".to_string()
    } else {
        format!("detail-nav-bar {}", base_class)
    };

    let title_tooltip = format!("{} (Taste: {})", text, shortcut.as_deref().unwrap_or("Esc"));

    let actions_view = if let Some(a) = actions {
        Some(a())
    } else {
        children.map(|c| c())
    };

    view! {
        <nav class=container_class aria-label="Detailnavigation">
            <div class="detail-nav-left">
                {if show_back {
                    let back_cb = on_back;
                    view! {
                        <button
                            type="button"
                            class="btn-detail-back"
                            on:click=move |_| {
                                if let Some(cb) = back_cb {
                                    cb.run(());
                                }
                            }
                            title=title_tooltip
                        >
                            <span class="back-arrow">"←"</span>
                            <span class="back-text">{text.clone()}</span>
                            {if let Some(sc) = shortcut.clone() {
                                if !sc.is_empty() {
                                    view! { <kbd class="kbd-shortcut">{sc}</kbd> }.into_any()
                                } else {
                                    ().into_any()
                                }
                            } else {
                                ().into_any()
                            }}
                        </button>
                    }.into_any()
                } else {
                    ().into_any()
                }}

                {if !crumbs.is_empty() {
                    let last_idx = crumbs.len().saturating_sub(1);
                    view! {
                        <div class="detail-breadcrumbs" aria-label="Pfadnavigation">
                            {crumbs.into_iter().enumerate().map(|(i, item)| {
                                let is_last = i == last_idx || item.is_current;
                                let cb = item.on_click;
                                let lbl = item.label.clone();
                                view! {
                                    {if let Some(click_fn) = cb {
                                        view! {
                                            <span
                                                class="breadcrumb-item breadcrumb-link"
                                                role="button"
                                                tabindex="0"
                                                on:click=move |_| click_fn.run(())
                                            >
                                                {lbl}
                                            </span>
                                        }.into_any()
                                    } else if is_last {
                                        view! {
                                            <span class="breadcrumb-item breadcrumb-current" aria-current="page">
                                                {lbl}
                                            </span>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <span class="breadcrumb-item">
                                                {lbl}
                                            </span>
                                        }.into_any()
                                    }}
                                    {if !is_last {
                                        view! { <span class="breadcrumb-separator" aria-hidden="true">"/"</span> }.into_any()
                                    } else {
                                        ().into_any()
                                    }}
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }.into_any()
                } else {
                    ().into_any()
                }}
            </div>

            {if let Some(av) = actions_view {
                view! {
                    <div class="detail-nav-actions">
                        {av}
                    </div>
                }.into_any()
            } else {
                ().into_any()
            }}
        </nav>
    }
}
