//! What search engines and link previews read of a page: description, canonical address, Open
//! Graph tags and structured data. Every page states them once, with `Seo`; nothing of it is set
//! for the whole app, so no page carries two descriptions.
//!
//! The rules (docs/frontend.md „Search engines"):
//! - A page has one address. Filters, pages of the list and the preview of the catalog are views of
//!   `/catalog` and are not indexed (`noindex, follow`: their links are still followed); the
//!   preview of a module points to the module's own page.
//! - Addresses in the tags are absolute. The host says what the site is called from outside
//!   (`SiteUrl`, the server's `--public-url`).
//! - Structured data only states what the page shows.

use std::sync::Arc;

use leptos::prelude::*;
use leptos_meta::{Link, Meta, Script};

/// The address of the site as the world sees it, without a slash at the end.
#[derive(Clone)]
pub struct SiteUrl(pub Arc<str>);

pub const DEFAULT_SITE_URL: &str = "https://betula.app";
pub const SITE_NAME: &str = "Betula";
/// The university the catalog is about, as structured data names it.
pub const UNIVERSITY: &str = "Brandenburgische Technische Universität Cottbus-Senftenberg";
pub const UNIVERSITY_URL: &str = "https://www.b-tu.de/";

pub fn site_url() -> String {
    use_context::<SiteUrl>().map(|site| site.0.trim_end_matches('/').to_string()).unwrap_or_else(|| DEFAULT_SITE_URL.to_string())
}

/// `path` (with its query, if it belongs to the page) as an absolute address.
pub fn absolute(path: &str) -> String {
    format!("{}{}", site_url(), path)
}

/// Text for a description: one line, at most `limit` characters, cut at a word.
pub fn excerpt(text: &str, limit: usize) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= limit {
        return line;
    }
    let cut: String = line.chars().take(limit).collect();
    let cut = cut.rsplit_once(' ').map(|(head, _)| head.to_string()).unwrap_or(cut);
    format!("{} …", cut.trim_end_matches([',', ';', ':', '.', ' ']))
}

/// JSON for a `<script>`: `<` must not end the element.
pub fn json_ld(value: &serde_json::Value) -> String {
    value.to_string().replace('<', "\\u003c")
}

/// The way from the start page to this page, as structured data: (name, path).
pub fn breadcrumbs(trail: &[(&str, String)]) -> serde_json::Value {
    serde_json::json!({
        "@type": "BreadcrumbList",
        "itemListElement": trail.iter().enumerate().map(|(i, (name, path))| serde_json::json!({
            "@type": "ListItem",
            "position": i + 1,
            "name": name,
            "item": absolute(path),
        })).collect::<Vec<_>>(),
    })
}

/// The tags of a page. Renders nothing in place; put it inside the page's frame.
#[component]
pub fn Seo(
    /// For link previews: the page's title without the name of the site.
    #[prop(into)]
    title: String,
    #[prop(into)] description: String,
    /// The one address of this page: path, plus the query if it makes a different page.
    #[prop(into)]
    path: String,
    /// A view of another page (a filter, a page of a list): follow its links, do not list it.
    #[prop(optional)]
    noindex: bool,
    /// Structured data (schema.org), the members of `@graph`.
    #[prop(optional)]
    data: Vec<serde_json::Value>,
) -> impl IntoView {
    let address = absolute(&path);
    let image = absolute(crate::OG_IMAGE);
    let data = (!data.is_empty()).then(|| json_ld(&serde_json::json!({ "@context": "https://schema.org", "@graph": data })));
    view! {
        <Meta name="description" content=description.clone()/>
        <Link rel="canonical" href=address.clone()/>
        {noindex.then(|| view! { <Meta name="robots" content="noindex, follow"/> })}
        <Meta property="og:site_name" content=SITE_NAME/>
        <Meta property="og:type" content="website"/>
        <Meta property="og:locale" content="de_DE"/>
        <Meta property="og:title" content=title/>
        <Meta property="og:description" content=description/>
        <Meta property="og:url" content=address/>
        <Meta property="og:image" content=image/>
        <Meta property="og:image:width" content="1200"/>
        <Meta property="og:image:height" content="630"/>
        <Meta name="twitter:card" content="summary_large_image"/>
        {data.map(|json| view! { <Script type_="application/ld+json">{json}</Script> })}
    }
}
