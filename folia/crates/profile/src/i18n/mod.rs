//! The texts of the profile page (`folia_design::texts!`; how the languages work:
//! `folia_design::i18n`).

pub mod profile;
pub mod scan;
pub mod sync;

pub use folia_design::i18n::{app_path, locale, of_address, use_location, Locale};

folia_design::texts! {
    common = folia_design::i18n::common,
    format = folia_design::i18n::format,
    ground = folia_shell::i18n::ground,
    profile = crate::i18n::profile,
    profiles = folia_stores::i18n::profiles,
    scan = crate::i18n::scan,
    sync = crate::i18n::sync,
    syncpair = folia_widgets::i18n::syncpair,
    ui = folia_design::i18n::ui,
}
