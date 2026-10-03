//! What search engines and link previews read of a page: description, canonical address, Open
//! Graph tags and structured data. Every page states them once, with `Seo`; nothing of it is set
//! for the whole app, so no page carries two descriptions.
//!
//! The rules (docs/folia/frontend.md „Search engines"):
//! - A page has one address. Filters, pages of the list and the preview of the catalog are views of
//!   `/catalog` and are not indexed (`noindex, follow`: their links are still followed); the
//!   preview of a module points to the module's own page.
//! - A crawler is led to pages only: a link to what search engines do not list carries
//!   `rel="nofollow"` — a view (a filter, an order, a page of a filtered list) or what the visitor
//!   keeps in the browser (the Merkliste, the Stundenplan, „Mein Plan"); `url::listed` says which,
//!   and `nofollow` below writes it where the link's target decides. The pages of the unfiltered
//!   catalog (`/catalog?page=<n>`) are pages, so their pager is followed. robots.txt keeps
//!   crawlers out of the views of the lists as well (`server::api::robots`).
//! - Addresses in the tags are absolute. The host says what the site is called from outside
//!   (`SiteUrl`, the server's `--public-url`).
//! - A page is one page in every language: its canonical address is its own language's, and it
//!   names the same page in every other (`hreflang`; the default language's also for everybody
//!   else, `x-default`).
//! - Structured data only states what the page shows, and says it compactly: what search engines
//!   and the answers built on their index read of a module is its Termine, exams and semesters in
//!   the study plans, not a second copy of the page.

use std::sync::Arc;

use folia_calendar::day::{berlin_offset, clock, minutes, Day};
use leptos::prelude::*;
use leptos_meta::{Link, Meta, Script};

use crate::i18n::{self, Locale};

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

/// `path`, a page of the app (with its query, if it belongs to the page), as the absolute address
/// of that page in the language being rendered.
pub fn absolute(path: &str) -> String {
    format!("{}{}", site_url(), i18n::locale().path(path))
}

/// `path`, a file (a picture), as an absolute address: the same in every language.
pub fn absolute_file(path: &str) -> String {
    format!("{}{}", site_url(), path)
}

/// The `rel` of a link to `path` (a path of the app with its query, before `Texts::path` puts the
/// language in front): `nofollow` where it leads to no page search engines list (`url::listed`).
pub fn nofollow(path: &str) -> Option<&'static str> {
    (!folia_routes::url::listed(path)).then_some("nofollow")
}

/// The pictures the server draws for link previews (`folia/crates/server/src/cards.rs`): 1200 × 630, the title
/// and a few facts in the look of the site.
pub fn module_card(id: &str) -> String {
    format!("/cards/module/{id}.png")
}

pub fn program_card(slug: &str) -> String {
    format!("/cards/program/{slug}.png")
}

/// The pictures of the Merkliste and the Stundenplan: what the page is, the same for everybody
/// (what a visitor keeps lives in their browser, R20).
pub const BOOKMARKS_CARD: &str = "/cards/bookmarks.png";
pub const STUDYPLAN_CARD: &str = "/cards/studyplan.png";

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

/// The university as structured data names it: the provider of every module and program.
pub fn university() -> serde_json::Value {
    serde_json::json!({ "@type": "CollegeOrUniversity", "name": UNIVERSITY, "url": UNIVERSITY_URL })
}

/// A day of the week as schema.org names it (`Schedule.byDay`), 1 = Monday … 7 = Sunday as in the
/// event tables.
pub fn day_of_week(day: u8) -> Option<String> {
    const DAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
    DAYS.get(usize::from(day).checked_sub(1)?).map(|name| format!("https://schema.org/{name}"))
}

/// A time of a day at the BTU as one moment, with the offset of `Europe/Berlin` on that day:
/// 2027-02-15 and „09:00" → `2027-02-15T09:00:00+01:00`. `None` for what is no time of a day
/// (24:00 included: it is the next day's midnight).
pub fn berlin_time(day: Day, hhmm: &str) -> Option<String> {
    let at = minutes(hhmm).filter(|at| *at < 24 * 60)?;
    Some(format!("{}T{}:00{}", day.iso(), clock(at), berlin_offset(day, at)))
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
    /// The page's own picture for link previews (`module_card`, `program_card`); without one the
    /// site's standard picture is named.
    #[prop(optional, into)]
    card: Option<String>,
    /// A view of another page (a filter, a page of a list): follow its links, do not list it.
    #[prop(optional)]
    noindex: bool,
    /// Structured data (schema.org), the members of `@graph`.
    #[prop(optional)]
    data: Vec<serde_json::Value>,
) -> impl IntoView {
    let t = i18n::t();
    let address = absolute(&path);
    // The same page in every language, and for whoever speaks none of them.
    let site = site_url();
    let alternates = Locale::ALL
        .iter()
        .map(|locale| (locale.code(), format!("{site}{}", locale.path(&path))))
        .chain([("x-default", format!("{site}{}", Locale::default().path(&path)))])
        .map(|(language, href)| view! { <Link rel="alternate" hreflang=language href=href/> })
        .collect_view();
    let other_territories = Locale::ALL
        .iter()
        .filter(|locale| **locale != t.locale)
        .map(|locale| view! { <Meta property="og:locale:alternate" content=locale.territory()/> })
        .collect_view();
    // A card says the page's title; the standard picture says what the site is.
    let alt = if card.is_some() { format!("{title} · {SITE_NAME}") } else { t.seo.image_alt.to_string() };
    // A card and the standard picture speak the page's language (`/en/cards/…`, `/en/assets/og.png`).
    let image = absolute_file(&t.path(card.as_deref().unwrap_or(crate::OG_IMAGE)));
    let data = (!data.is_empty()).then(|| json_ld(&serde_json::json!({ "@context": "https://schema.org", "@graph": data })));
    view! {
        <Meta name="description" content=description.clone()/>
        <Link rel="canonical" href=address.clone()/>
        {alternates}
        {noindex.then(|| view! { <Meta name="robots" content="noindex, follow"/> })}
        <Meta property="og:site_name" content=SITE_NAME/>
        <Meta property="og:type" content="website"/>
        <Meta property="og:locale" content=t.locale.territory()/>
        {other_territories}
        <Meta property="og:title" content=title.clone()/>
        <Meta property="og:description" content=description.clone()/>
        <Meta property="og:url" content=address/>
        <Meta property="og:image" content=image.clone()/>
        <Meta property="og:image:type" content="image/png"/>
        <Meta property="og:image:width" content="1200"/>
        <Meta property="og:image:height" content="630"/>
        <Meta property="og:image:alt" content=alt.clone()/>
        // X reads the og: tags too, but only with its own it shows the large card everywhere.
        <Meta name="twitter:card" content="summary_large_image"/>
        <Meta name="twitter:title" content=title/>
        <Meta name="twitter:description" content=description/>
        <Meta name="twitter:image" content=image/>
        <Meta name="twitter:image:alt" content=alt/>
        {data.map(|json| view! { <Script type_="application/ld+json">{json}</Script> })}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_and_days_as_schema_org_reads_them() {
        let day = |iso| Day::parse(iso).unwrap();
        assert_eq!(berlin_time(day("2027-02-15"), "09:00").as_deref(), Some("2027-02-15T09:00:00+01:00"));
        assert_eq!(berlin_time(day("2026-10-07"), "13:00").as_deref(), Some("2026-10-07T13:00:00+02:00"));
        assert_eq!(berlin_time(day("2027-03-01"), "24:00"), None);
        assert_eq!(berlin_time(day("2027-03-01"), "offen"), None);
        assert_eq!(day_of_week(1).as_deref(), Some("https://schema.org/Monday"));
        assert_eq!(day_of_week(7).as_deref(), Some("https://schema.org/Sunday"));
        assert_eq!((day_of_week(0), day_of_week(8)), (None, None));
    }
}
