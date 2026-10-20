//! The texts of the shell (`folia_design::texts!`; how the languages work: `folia_design::i18n`).

pub mod app;
pub mod commands;
pub mod ground;
pub mod keys;
pub mod seo;

pub use folia_design::i18n::{app_path, locale, of_address, use_location, Locale};

folia_design::texts! {
    app = crate::i18n::app,
    commands = crate::i18n::commands,
    common = folia_design::i18n::common,
    ground = crate::i18n::ground,
    keys = crate::i18n::keys,
    seo = crate::i18n::seo,
    ui = folia_design::i18n::ui,
}
