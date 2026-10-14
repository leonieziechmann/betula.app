//! The texts of the profile page (`folia_design::texts!`; how the languages work:
//! `folia_design::i18n`).

pub mod profile;

pub use folia_design::i18n::{app_path, locale, of_address, use_location, Locale};

folia_design::texts! {
    common = folia_design::i18n::common,
    format = folia_design::i18n::format,
    profile = crate::i18n::profile,
    profiles = folia_stores::i18n::profiles,
    ui = folia_design::i18n::ui,
}
