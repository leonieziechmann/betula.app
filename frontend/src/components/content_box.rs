use leptos::prelude::*;
use crate::components::icon::{Icon, IconKind};

/// Structured content card with a stylized header (title, subtitle, icon, extra actions/badges) and body.
/// Supports emojis, image URLs, and SVGs in the header icon.
#[component]
pub fn ContentBox(
    #[prop(optional, into)] title: Option<String>,
    #[prop(optional, into)] subtitle: Option<String>,
    #[prop(optional)] icon: Option<IconKind>,
    #[prop(optional, into)] emoji: Option<String>,
    #[prop(optional, into)] image_url: Option<String>,
    #[prop(optional, into)] svg: Option<String>,
    #[prop(optional, default = false)] sticky: bool,
    #[prop(optional, into)] class: Option<String>,
    #[prop(optional, into)] header_class: Option<String>,
    #[prop(optional, into)] body_class: Option<String>,
    #[prop(optional, default = false)] no_body_padding: bool,
    #[prop(optional)] header_extra: Option<Children>,
    children: Children,
) -> impl IntoView {
    let base_class = class.unwrap_or_default();
    let sticky_class = if sticky { "detail-card-sticky" } else { "" };
    let container_class = format!("detail-card {} {}", sticky_class, base_class)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let head_cls = format!("detail-card-header {}", header_class.unwrap_or_default())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let b_cls = format!(
        "detail-card-body {} {}",
        if no_body_padding { "no-padding" } else { "" },
        body_class.unwrap_or_default()
    )
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ");

    let resolved_icon = IconKind::from_options(icon, emoji, image_url, svg);
    let has_icon = resolved_icon.is_some();
    let has_header = title.is_some() || has_icon || header_extra.is_some();

    view! {
        <section class=container_class>
            {if has_header {
                let extra = header_extra.map(|h| h());
                view! {
                    <div class=head_cls>
                        <div class="content-box-header-main" style="display: flex; align-items: center; gap: 0.65rem; flex: 1; min-width: 0;">
                            {if has_icon {
                                view! {
                                    <div class="card-header-icon">
                                        <Icon icon=resolved_icon />
                                    </div>
                                }.into_any()
                            } else {
                                ().into_any()
                            }}

                            <div style="flex: 1; min-width: 0;">
                                {if let Some(t) = title {
                                    view! { <h2 class="detail-card-title">{t}</h2> }.into_any()
                                } else {
                                    ().into_any()
                                }}

                                {if let Some(sub) = subtitle {
                                    view! { <div class="card-header-subtitle">{sub}</div> }.into_any()
                                } else {
                                    ().into_any()
                                }}
                            </div>
                        </div>

                        {if let Some(ex) = extra {
                            view! {
                                <div class="content-box-header-extra" style="display: flex; align-items: center; gap: 0.5rem; flex-shrink: 0;">
                                    {ex}
                                </div>
                            }.into_any()
                        } else {
                            ().into_any()
                        }}
                    </div>
                }.into_any()
            } else {
                ().into_any()
            }}

            <div class=b_cls>
                {children()}
            </div>
        </section>
    }
}
