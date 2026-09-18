use leptos::prelude::*;
use crate::components::icon::{Icon, IconKind};

/// Metric / KPI display card for key properties (credits, duration, turnus, language, exams, faculty, etc.).
/// Supports emojis, image URLs, and SVGs inside the stylized icon wrapper.
#[component]
pub fn DisplayCard(
    #[prop(optional, into)] label: Option<String>,
    #[prop(optional, into)] value: Option<String>,
    #[prop(optional, into)] subtext: Option<String>,
    #[prop(optional)] icon: Option<IconKind>,
    #[prop(optional, into)] emoji: Option<String>,
    #[prop(optional, into)] image_url: Option<String>,
    #[prop(optional, into)] svg: Option<String>,
    #[prop(optional, into)] icon_class: Option<String>,
    #[prop(optional, default = false)] wide: bool,
    #[prop(optional, into)] tooltip: Option<String>,
    #[prop(optional, into)] class: Option<String>,
    #[prop(optional)] on_click: Option<Callback<()>>,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    let base_class = class.unwrap_or_default();
    let wide_class = if wide { "detail-kpi-wide" } else { "" };
    let click_class = if on_click.is_some() { "is-clickable" } else { "" };

    let card_class = format!("detail-kpi-card {} {} {}", wide_class, click_class, base_class)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let icon_color_class = icon_class.unwrap_or_else(|| "kpi-blue".to_string());
    let icon_wrapper_class = format!("kpi-icon-wrap {}", icon_color_class);

    let resolved_icon = IconKind::from_options(icon, emoji, image_url, svg);
    let has_icon = resolved_icon.is_some();
    let click_handler = on_click;

    view! {
        <div
            class=card_class
            title=tooltip.clone()
            on:click=move |_| {
                if let Some(cb) = click_handler {
                    cb.run(());
                }
            }
        >
            {if has_icon {
                view! {
                    <div class=icon_wrapper_class>
                        <Icon icon=resolved_icon />
                    </div>
                }.into_any()
            } else {
                ().into_any()
            }}

            <div class="kpi-content">
                {if let Some(lbl) = label {
                    view! { <span class="kpi-label">{lbl}</span> }.into_any()
                } else {
                    ().into_any()
                }}

                {if let Some(val) = value {
                    view! { <div class="kpi-value" title=tooltip.clone()>{val}</div> }.into_any()
                } else if let Some(c) = children {
                    view! { <div class="kpi-value">{c()}</div> }.into_any()
                } else {
                    ().into_any()
                }}

                {if let Some(sub) = subtext {
                    view! { <div class="kpi-subtext">{sub}</div> }.into_any()
                } else {
                    ().into_any()
                }}
            </div>
        </div>
    }
}
