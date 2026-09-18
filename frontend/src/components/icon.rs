use leptos::prelude::*;

/// Representation of an icon (Emoji, Image URL, SVG markup, or None).
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub enum IconKind {
    #[default]
    None,
    Emoji(String),
    ImageUrl(String),
    Svg(String),
}

impl IconKind {
    pub fn emoji(s: impl Into<String>) -> Self {
        Self::Emoji(s.into())
    }

    pub fn image(url: impl Into<String>) -> Self {
        Self::ImageUrl(url.into())
    }

    pub fn svg(svg_raw: impl Into<String>) -> Self {
        Self::Svg(svg_raw.into())
    }

    pub fn from_options(
        icon: Option<IconKind>,
        emoji: Option<String>,
        image_url: Option<String>,
        svg: Option<String>,
    ) -> Self {
        if let Some(i) = icon {
            i
        } else if let Some(e) = emoji {
            IconKind::Emoji(e)
        } else if let Some(img) = image_url {
            IconKind::ImageUrl(img)
        } else if let Some(s) = svg {
            IconKind::Svg(s)
        } else {
            IconKind::None
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub fn is_some(&self) -> bool {
        !self.is_none()
    }
}

impl From<&str> for IconKind {
    fn from(s: &str) -> Self {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            IconKind::None
        } else if trimmed.starts_with("<svg") {
            IconKind::Svg(trimmed.to_string())
        } else if trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
            || trimmed.starts_with('/')
            || trimmed.starts_with("./")
            || trimmed.ends_with(".png")
            || trimmed.ends_with(".jpg")
            || trimmed.ends_with(".jpeg")
            || trimmed.ends_with(".svg")
            || trimmed.ends_with(".webp")
        {
            IconKind::ImageUrl(trimmed.to_string())
        } else {
            IconKind::Emoji(trimmed.to_string())
        }
    }
}

impl From<String> for IconKind {
    fn from(s: String) -> Self {
        IconKind::from(s.as_str())
    }
}

/// Generic component to render an icon (Emoji, Image URL, or SVG).
#[component]
pub fn Icon(
    #[prop(into)] icon: IconKind,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let base_class = class.unwrap_or_default();

    match icon {
        IconKind::None => ().into_any(),
        IconKind::Emoji(e) => {
            let cls = if base_class.is_empty() { "icon-emoji".to_string() } else { format!("icon-emoji {}", base_class) };
            view! { <span class=cls aria-hidden="true">{e}</span> }.into_any()
        }
        IconKind::ImageUrl(url) => {
            let cls = if base_class.is_empty() { "icon-image".to_string() } else { format!("icon-image {}", base_class) };
            view! { <img src=url class=cls alt="" aria-hidden="true" /> }.into_any()
        }
        IconKind::Svg(svg_code) => {
            let cls = if base_class.is_empty() { "icon-svg".to_string() } else { format!("icon-svg {}", base_class) };
            view! { <span class=cls inner_html=svg_code aria-hidden="true" /> }.into_any()
        }
    }
}
