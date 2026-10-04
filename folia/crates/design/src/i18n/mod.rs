//! The languages of the app (`folia_locale` has the languages, the addresses and the words of the
//! data; the crates of the app have the words of their pages).
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
//!   `catalog::EN`). A group lives in the crate of its part. Each crate puts the groups it
//!   writes together with `texts!` (its own `i18n::Texts`, `DE`, `EN`, `texts`, `t`): a
//!   component takes them once, `let t = i18n::t();`, and writes `{t.catalog.filters}`; a text
//!   with values in it is a function: `{(t.catalog.found)(n)}`. A language that lacks a text does
//!   not compile. How to add a language: `docs/folia/i18n.md`.

pub mod combobox;
pub mod common;
pub mod format;
pub mod ui;

use leptos::prelude::*;
use leptos_router::location::{Location, RequestUrl};

pub use folia_locale::Locale;

/// The words of the data contract: labels of codes, weekdays, months (`folia_locale`), for
/// `texts!`, which gives every crate's texts them as `data`.
pub use folia_locale::common as data_words;

/// A crate's texts in every language: `Texts` with the language (`locale`), the words of the
/// data (`data`) and one field per group named, `DE` and `EN`, `texts(locale)` and `t()` (the
/// texts of the page being rendered). Each group is a module with `Texts`, `DE` and `EN`:
///
/// ```ignore
/// folia_design::texts! {
///     common = folia_design::i18n::common,
///     catalog = crate::i18n::catalog,
/// }
/// ```
#[macro_export]
macro_rules! texts {
    ($($group:ident = $($segment:ident)::+),* $(,)?) => {
        /// Every text of this crate in one language, and the language itself.
        pub struct Texts {
            pub locale: $crate::i18n::Locale,
            /// The words of the data contract: labels of codes, weekdays, months.
            pub data: &'static $crate::i18n::data_words::Texts,
            $(pub $group: $($segment)::+::Texts,)*
        }

        /// The texts in German.
        pub static DE: Texts = Texts {
            locale: $crate::i18n::Locale::De,
            data: &$crate::i18n::data_words::DE,
            $($group: $($segment)::+::DE,)*
        };

        /// The texts in English.
        pub static EN: Texts = Texts {
            locale: $crate::i18n::Locale::En,
            data: &$crate::i18n::data_words::EN,
            $($group: $($segment)::+::EN,)*
        };

        /// The texts of a language.
        pub fn texts(locale: $crate::i18n::Locale) -> &'static Texts {
            match locale {
                $crate::i18n::Locale::De => &DE,
                $crate::i18n::Locale::En => &EN,
            }
        }

        /// The texts of the page being rendered (`folia_design::i18n::locale`).
        pub fn t() -> &'static Texts {
            texts($crate::i18n::locale())
        }

        impl Texts {
            /// `path`, an address of the app (`/catalog?q=x`), as a link of a page in this
            /// language writes it (`/en/catalog?q=x`).
            pub fn path(&self, path: &str) -> String {
                self.locale.path(path)
            }
        }
    };
}

texts! {
    combobox = crate::i18n::combobox,
    common = crate::i18n::common,
    format = crate::i18n::format,
    ui = crate::i18n::ui,
}

/// The language of the page being rendered: what `App` provides, else what the address says.
pub fn locale() -> Locale {
    use_context::<Locale>().unwrap_or_else(of_address)
}

/// The language the address being rendered names: the window's in the browser app, the
/// request's on the server. The default where there is neither (a test).
pub fn of_address() -> Locale {
    #[cfg(all(feature = "csr", target_arch = "wasm32"))]
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
