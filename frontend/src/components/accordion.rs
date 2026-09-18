use leptos::prelude::*;
use crate::components::icon::{Icon, IconKind};

/// Versatile Accordion component with customizable header (title, subtitle, emoji/image/svg icon, extra badges/actions)
/// and expandable body content. Can be used in self-managed mode (default_open) or controlled mode (is_open signal).
#[component]
pub fn Accordion(
    #[prop(optional, into)] title: Option<String>,
    #[prop(optional, into)] subtitle: Option<String>,
    #[prop(optional)] icon: Option<IconKind>,
    #[prop(optional, into)] emoji: Option<String>,
    #[prop(optional, into)] image_url: Option<String>,
    #[prop(optional, into)] svg: Option<String>,
    #[prop(optional)] is_open: Option<RwSignal<bool>>,
    #[prop(optional, default = false)] default_open: bool,
    #[prop(optional)] on_toggle: Option<Callback<bool>>,
    #[prop(optional)] header_extra: Option<Children>,
    #[prop(optional, into)] header_view: Option<AnyView>,
    #[prop(optional, into)] class: Option<String>,
    #[prop(optional, into)] header_class: Option<String>,
    #[prop(optional, into)] content_class: Option<String>,
    children: Children,
) -> impl IntoView {
    // Internal state if is_open signal is not passed
    let internal_open = RwSignal::new(default_open);
    let open_signal = is_open.unwrap_or(internal_open);

    let toggle_cb = on_toggle;
    let handle_toggle = move |_| {
        let new_state = !open_signal.get();
        open_signal.set(new_state);
        if let Some(cb) = toggle_cb {
            cb.run(new_state);
        }
    };

    let base_class = class.unwrap_or_default();
    let resolved_icon = IconKind::from_options(icon, emoji, image_url, svg);
    let has_icon = resolved_icon.is_some();
    let extra_header = header_extra.map(|h| h());

    let head_cls = format!("accordion-header {}", header_class.unwrap_or_default())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let cnt_cls = format!("accordion-body-content {}", content_class.unwrap_or_default())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    view! {
        <div class=move || {
            let open_cls = if open_signal.get() { "is-open" } else { "" };
            format!("accordion-card-item {} {}", open_cls, base_class)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        }>
            <button
                type="button"
                class=head_cls
                on:click=handle_toggle
                aria-expanded=move || if open_signal.get() { "true" } else { "false" }
            >
                {if let Some(hv) = header_view {
                    hv
                } else {
                    view! {
                        <div class="accordion-header-left" style="display: flex; align-items: center; gap: 0.65rem; flex: 1; min-width: 0; text-align: left;">
                            {if has_icon {
                                view! {
                                    <div class="accordion-header-icon" style="font-size: 1.15rem; line-height: 1;">
                                        <Icon icon=resolved_icon />
                                    </div>
                                }.into_any()
                            } else {
                                ().into_any()
                            }}

                            <div style="flex: 1; min-width: 0;">
                                {if let Some(t) = title {
                                    view! { <span class="accordion-title-text">{t}</span> }.into_any()
                                } else {
                                    ().into_any()
                                }}

                                {if let Some(sub) = subtitle {
                                    view! { <div class="accordion-subtitle-text">{sub}</div> }.into_any()
                                } else {
                                    ().into_any()
                                }}
                            </div>
                        </div>
                    }.into_any()
                }}

                <div class="accordion-header-right" style="display: flex; align-items: center; gap: 0.6rem; flex-shrink: 0;">
                    {if let Some(ex) = extra_header {
                        view! {
                            <div class="accordion-header-extra" on:click=move |e| e.stop_propagation()>
                                {ex}
                            </div>
                        }.into_any()
                    } else {
                        ().into_any()
                    }}

                    <span class="accordion-chevron" aria-hidden="true">
                        {move || if open_signal.get() { "▲" } else { "▼" }}
                    </span>
                </div>
            </button>

            <div
                class="accordion-body-collapse"
                style=move || if open_signal.get() { "display: block;" } else { "display: none;" }
            >
                <div class=cnt_cls>
                    {children()}
                </div>
            </div>
        </div>
    }
}
