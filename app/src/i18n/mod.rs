//! The app in every language of the site (`catalog::i18n` has the languages, the addresses and
//! the words of the data; this module has the words of the pages).
//!
//! - **Which language** a page is in, its address says (`/en/catalog` is English, `/catalog`
//!   German; `Locale::split`). The server reads it from the request, the browser app from the
//!   window; `App` provides it as context (`locale`), and the router works below the language's
//!   prefix (`<Router base>`), so the routes and every path inside the app stay without it.
//! - **Links** carry the prefix: whatever writes an address of the app into a page writes
//!   `t.path(…)` (`Texts::path`). What reads the address uses `use_location`, whose `pathname`
//!   is the app's path without the prefix. A link into another language is a new page load
//!   (`rel="external"`): the language of a running app never changes.
//! - **Texts** come in groups, one per part of the app (`app`, `catalog`, `module` …), each a
//!   struct with one field per text and one `const` per language (`catalog::DE`,
//!   `catalog::EN`). A component takes them once, `let t = i18n::t();`, and writes
//!   `{t.catalog.filters}`; a text with values in it is a function: `{(t.catalog.found)(n)}`.
//!   A language that lacks a text does not compile. How to add a language: `docs/i18n.md`.

pub mod app;
pub mod bookmarks;
pub mod catalog;
pub mod combobox;
pub mod common;
pub mod format;
pub mod ground;
pub mod home;
pub mod home_detail;
pub mod legal;
pub mod marks;
pub mod module;
pub mod myprogram;
pub mod planner;
pub mod program;
pub mod programs;
pub mod seo;
pub mod studyplan;
pub mod studyplan_aside;
pub mod studyplan_exams;
pub mod studyplan_export;
pub mod studyplan_head;
pub mod studyplan_import;
pub mod studyplan_modules;
pub mod studyplan_share;
pub mod studyplan_side;
pub mod studyplan_week;
pub mod ui;
pub mod week;

pub use ::catalog::i18n::Locale;
use leptos::prelude::*;
use leptos_router::location::{Location, RequestUrl};

/// Every text of the app in one language, and the language itself.
pub struct Texts {
    pub locale: Locale,
    /// The words of the data contract: labels of codes, weekdays, months (`catalog::i18n`).
    pub data: &'static ::catalog::i18n::Texts,
    pub app: app::Texts,
    pub bookmarks: bookmarks::Texts,
    pub catalog: catalog::Texts,
    pub combobox: combobox::Texts,
    pub common: common::Texts,
    pub format: format::Texts,
    pub ground: ground::Texts,
    pub home: home::Texts,
    pub home_detail: home_detail::Texts,
    pub legal: legal::Texts,
    pub marks: marks::Texts,
    pub module: module::Texts,
    pub myprogram: myprogram::Texts,
    pub planner: planner::Texts,
    pub program: program::Texts,
    pub programs: programs::Texts,
    pub seo: seo::Texts,
    pub studyplan: studyplan::Texts,
    pub studyplan_aside: studyplan_aside::Texts,
    pub studyplan_exams: studyplan_exams::Texts,
    pub studyplan_export: studyplan_export::Texts,
    pub studyplan_head: studyplan_head::Texts,
    pub studyplan_import: studyplan_import::Texts,
    pub studyplan_modules: studyplan_modules::Texts,
    pub studyplan_share: studyplan_share::Texts,
    pub studyplan_side: studyplan_side::Texts,
    pub studyplan_week: studyplan_week::Texts,
    pub ui: ui::Texts,
    pub week: week::Texts,
}

/// Every group of texts in one language, from the groups' `const`s of that language.
macro_rules! language {
    ($locale:expr, $data:expr, $lang:ident) => {
        Texts {
            locale: $locale,
            data: $data,
            app: app::$lang,
            bookmarks: bookmarks::$lang,
            catalog: catalog::$lang,
            combobox: combobox::$lang,
            common: common::$lang,
            format: format::$lang,
            ground: ground::$lang,
            home: home::$lang,
            home_detail: home_detail::$lang,
            legal: legal::$lang,
            marks: marks::$lang,
            module: module::$lang,
            myprogram: myprogram::$lang,
            planner: planner::$lang,
            program: program::$lang,
            programs: programs::$lang,
            seo: seo::$lang,
            studyplan: studyplan::$lang,
            studyplan_aside: studyplan_aside::$lang,
            studyplan_exams: studyplan_exams::$lang,
            studyplan_export: studyplan_export::$lang,
            studyplan_head: studyplan_head::$lang,
            studyplan_import: studyplan_import::$lang,
            studyplan_modules: studyplan_modules::$lang,
            studyplan_share: studyplan_share::$lang,
            studyplan_side: studyplan_side::$lang,
            studyplan_week: studyplan_week::$lang,
            ui: ui::$lang,
            week: week::$lang,
        }
    };
}

pub static DE: Texts = language!(Locale::De, &::catalog::i18n::DE, DE);
pub static EN: Texts = language!(Locale::En, &::catalog::i18n::EN, EN);

/// The texts of a language.
pub fn texts(locale: Locale) -> &'static Texts {
    match locale {
        Locale::De => &DE,
        Locale::En => &EN,
    }
}

impl Texts {
    /// `path`, an address of the app (`/catalog?q=x`), as a link of a page in this language
    /// writes it (`/en/catalog?q=x`).
    pub fn path(&self, path: &str) -> String {
        self.locale.path(path)
    }
}

/// The language of the page being rendered: what `App` provides, else what the address says.
pub fn locale() -> Locale {
    use_context::<Locale>().unwrap_or_else(of_address)
}

/// The texts of the page being rendered (`locale`).
pub fn t() -> &'static Texts {
    texts(locale())
}

/// The language the address being rendered names: the window's in the browser app, the
/// request's on the server. The default where there is neither (a test).
pub fn of_address() -> Locale {
    #[cfg(feature = "csr")]
    if let Some(path) = web_sys::window().and_then(|window| window.location().pathname().ok()) {
        return Locale::split(&path).0;
    }
    use_context::<RequestUrl>().and_then(|url| url.parse().ok()).map_or_else(Locale::default, |url| Locale::split(url.path()).0)
}

/// The router's location with the path of the app: `pathname` without the language's prefix
/// (`/en/catalog` reads `/catalog`), the rest as the router has it. Use it instead of
/// `leptos_router::hooks::use_location` wherever a path is compared or remembered.
pub fn use_location() -> Location {
    let location = leptos_router::hooks::use_location();
    let pathname = location.pathname;
    Location { pathname: Memo::new(move |_| pathname.with(|path| Locale::split(path).1.to_string())), ..location }
}

/// An address the browser is at or goes to (`/en/catalog`), as the app's path (`/catalog`), if
/// it is in the language of this app: a link into another language is no step within the app.
pub fn app_path(address: &str) -> Option<&str> {
    let (language, path) = Locale::split(address);
    (language == locale()).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_language_has_its_own_texts() {
        for locale in Locale::ALL.iter().copied() {
            assert_eq!(texts(locale).locale, locale);
            assert_eq!(texts(locale).path("/catalog"), locale.path("/catalog"));
        }
        assert_eq!(EN.path("/catalog/module/11103"), "/en/catalog/module/11103");
        assert_eq!(DE.path("/catalog/module/11103"), "/catalog/module/11103");
    }
}
