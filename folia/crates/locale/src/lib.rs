//! The languages of the site: which there are, where each lives in an address, and the words the
//! data contract itself writes (labels of codes, weekdays, months, the sentences of the
//! Stundenplan). The app has its own texts (`app::i18n`); both follow the same pattern.
//!
//! **One language is one address** (R9: the server's HTML depends on the address and the
//! snapshot only). German, the language of the BTU and of the data, keeps the plain addresses
//! (`/catalog`); every other language lives under its code (`/en/catalog`). The paths inside the
//! app never carry the prefix: it is added where a link is written (`Locale::path`) and taken
//! off where an address is read (`Locale::split`).
//!
//! **Texts are Rust values.** A group of texts is a struct with one field per text, and each
//! language is a `const` of that struct (`common::DE`, `common::EN`): a text a language lacks
//! does not compile. A text with values in it is a function (`fn(u64) -> String`), so every
//! language builds its own sentence, plural and word order included. How to add a language:
//! `docs/folia/i18n.md`.
//!
//! What the data says is not translated (owner, 2026-09-27: „du musst die module nicht
//! übersetzen. Nur das, was im ui der app steht"): titles, descriptions, the names of study
//! programs, areas and rooms are shown as the BTU writes them, in every language. What the app
//! writes itself is translated: its words, and the labels of the codes (`labels`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod common;

use serde::{Deserialize, Serialize};

/// A language of the site. `De` is the default: the BTU's language and the plain addresses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Locale {
    #[default]
    De,
    En,
}

impl Locale {
    /// Every language of the site, the default first. The order is the order of the switcher.
    pub const ALL: &'static [Locale] = &[Locale::De, Locale::En];

    /// The language's code: `lang` of the document, `hreflang`, and the first segment of its
    /// addresses (but for the default's, which has none).
    pub fn code(self) -> &'static str {
        match self {
            Locale::De => "de",
            Locale::En => "en",
        }
    }

    /// The language named in itself, as the switcher offers it.
    pub fn name(self) -> &'static str {
        match self {
            Locale::De => "Deutsch",
            Locale::En => "English",
        }
    }

    /// Language and territory as Open Graph writes them (`og:locale`).
    pub fn territory(self) -> &'static str {
        match self {
            Locale::De => "de_DE",
            Locale::En => "en_GB",
        }
    }

    /// The language of a code: `en`, `EN`, `en-GB` and `en_US` are English. `None` for a
    /// language the site does not speak.
    pub fn from_code(code: &str) -> Option<Self> {
        let primary = code.trim().split(['-', '_']).next().unwrap_or_default();
        Self::ALL.iter().copied().find(|locale| locale.code().eq_ignore_ascii_case(primary))
    }

    /// The language a browser asks for first among those the site speaks
    /// (`Accept-Language: en-US,en;q=0.9,de;q=0.8`, or `navigator.languages` joined by commas).
    /// Weights are honoured; `None` when it names none of them.
    pub fn negotiate(accept: &str) -> Option<Self> {
        let mut best: Option<(Self, u16)> = None;
        for entry in accept.split(',') {
            let mut parts = entry.split(';');
            let Some(locale) = parts.next().and_then(Self::from_code) else { continue };
            // q in thousandths; a malformed weight counts as 1, as browsers never write one.
            let weight = parts
                .find_map(|part| part.trim().strip_prefix("q="))
                .map_or(1000, |q| q.trim().parse::<f32>().map_or(1000, |q| (q.clamp(0.0, 1.0) * 1000.0) as u16));
            // Ties go to the earlier entry.
            if weight > 0 && best.is_none_or(|(_, w)| weight > w) {
                best = Some((locale, weight));
            }
        }
        best.map(|(locale, _)| locale)
    }

    /// Where the language's addresses begin: nothing for the default, `/en` for English.
    pub fn prefix(self) -> &'static str {
        match self {
            Locale::De => "",
            Locale::En => "/en",
        }
    }

    /// `path`, an address of the app (`/catalog?q=x`, `/`), as a page in this language links
    /// it: `/en/catalog?q=x`, `/en`. What is no path of the app (`#top`, `?q=x`, `https://…`,
    /// `//host`) stays as it is.
    pub fn path(self, path: &str) -> String {
        let prefix = self.prefix();
        if prefix.is_empty() || !path.starts_with('/') || path.starts_with("//") {
            return path.to_string();
        }
        // The start page is `/en`, and its query or fragment follow that directly.
        match path.strip_prefix('/') {
            Some(rest) if rest.is_empty() || rest.starts_with(['?', '#']) => format!("{prefix}{rest}"),
            _ => format!("{prefix}{path}"),
        }
    }

    /// The language of an address and the address of the app it names: `/en/catalog` →
    /// (English, `/catalog`), `/en` → (English, `/`), `/catalog` → (German, `/catalog`). A
    /// query stays with the path; `/de/…` is no address of the site (the default has none).
    pub fn split(address: &str) -> (Self, &str) {
        for locale in Self::ALL.iter().copied() {
            let prefix = locale.prefix();
            if prefix.is_empty() {
                continue;
            }
            if let Some(rest) = address.strip_prefix(prefix) {
                if rest.is_empty() {
                    return (locale, "/");
                }
                if rest.starts_with('/') {
                    return (locale, rest);
                }
                if rest.starts_with(['?', '#']) {
                    // `/en?x` is the start page with a query: the path of the app is `/?x`,
                    // which has no slice of its own in `address`.
                    return (locale, "/");
                }
            }
        }
        (Locale::default(), address)
    }

    /// The words every crate shares in this language (`common`); a crate's own words are its
    /// `i18n::texts` (`folia_plans::i18n`, `folia_timetable::i18n`).
    pub fn texts(self) -> &'static common::Texts {
        match self {
            Locale::De => &common::DE,
            Locale::En => &common::EN,
        }
    }

    /// `7.5` as a number reads in this language: „7,5", "7.5"; whole numbers without a
    /// fraction („6"). An empty sum of floats is -0.0, which reads „0".
    pub fn decimal(self, value: f64) -> String {
        let value = if value == 0.0 { 0.0 } else { value };
        if value.fract() == 0.0 {
            return format!("{value:.0}");
        }
        let plain = format!("{value}");
        match self.texts().decimal_separator {
            '.' => plain,
            separator => plain.replace('.', &separator.to_string()),
        }
    }

    /// `1234` with its thousands grouped: „1.234", "1,234".
    pub fn thousands(self, value: u64) -> String {
        let digits = value.to_string();
        let separator = self.texts().thousands_separator;
        let mut out = String::with_capacity(digits.len() + digits.len() / 3);
        for (i, digit) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                out.push(separator);
            }
            out.push(digit);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_language_lives_under_its_prefix() {
        assert_eq!(Locale::De.path("/catalog?q=x"), "/catalog?q=x");
        assert_eq!(Locale::En.path("/catalog?q=x"), "/en/catalog?q=x");
        assert_eq!(Locale::En.path("/"), "/en");
        assert_eq!(Locale::En.path("/?q=x"), "/en?q=x");
        assert_eq!(Locale::En.path("/#website"), "/en#website");
        assert_eq!(Locale::En.path("/programs#fak-1"), "/en/programs#fak-1");
        for other in ["#top", "?q=x", "https://www.b-tu.de/", "//example.org/x", ""] {
            assert_eq!(Locale::En.path(other), other);
        }
    }

    #[test]
    fn an_address_says_its_language() {
        assert_eq!(Locale::split("/en/catalog"), (Locale::En, "/catalog"));
        assert_eq!(Locale::split("/en/catalog?q=x"), (Locale::En, "/catalog?q=x"));
        assert_eq!(Locale::split("/en"), (Locale::En, "/"));
        assert_eq!(Locale::split("/en/"), (Locale::En, "/"));
        assert_eq!(Locale::split("/en?q=x"), (Locale::En, "/"));
        assert_eq!(Locale::split("/catalog"), (Locale::De, "/catalog"));
        assert_eq!(Locale::split("/english"), (Locale::De, "/english"));
        assert_eq!(Locale::split("/"), (Locale::De, "/"));
        for locale in Locale::ALL.iter().copied() {
            for path in ["/", "/catalog", "/catalog/module/11103", "/programs/informatik/plan?variant=2"] {
                assert_eq!(Locale::split(&locale.path(path)), (locale, path));
            }
        }
    }

    #[test]
    fn a_browser_names_its_language() {
        assert_eq!(Locale::from_code("en-GB"), Some(Locale::En));
        assert_eq!(Locale::from_code("DE_at"), Some(Locale::De));
        assert_eq!(Locale::from_code("fr"), None);
        assert_eq!(Locale::negotiate("en-US,en;q=0.9,de;q=0.8"), Some(Locale::En));
        assert_eq!(Locale::negotiate("fr-FR,fr;q=0.9,de;q=0.7,en;q=0.5"), Some(Locale::De));
        assert_eq!(Locale::negotiate("de;q=0.4, en;q=0.6"), Some(Locale::En));
        assert_eq!(Locale::negotiate("fr, it"), None);
        assert_eq!(Locale::negotiate("en;q=0"), None);
        assert_eq!(Locale::negotiate(""), None);
    }

    #[test]
    fn numbers_read_as_the_language_writes_them() {
        assert_eq!((Locale::De.decimal(7.5), Locale::En.decimal(7.5)), ("7,5".to_string(), "7.5".to_string()));
        assert_eq!((Locale::De.decimal(6.0), Locale::En.decimal(-0.0)), ("6".to_string(), "0".to_string()));
        assert_eq!((Locale::De.thousands(1234567), Locale::En.thousands(1234)), ("1.234.567".to_string(), "1,234".to_string()));
        assert_eq!(Locale::De.thousands(999), "999");
    }

    #[test]
    fn every_language_has_a_code_of_its_own() {
        for (i, a) in Locale::ALL.iter().enumerate() {
            assert_eq!(Locale::from_code(a.code()), Some(*a));
            for b in Locale::ALL.iter().skip(i + 1) {
                assert_ne!(a.code(), b.code());
                assert_ne!(a.prefix(), b.prefix());
            }
        }
        assert_eq!(Locale::ALL.first(), Some(&Locale::default()));
        assert_eq!(Locale::default().prefix(), "");
    }
}
