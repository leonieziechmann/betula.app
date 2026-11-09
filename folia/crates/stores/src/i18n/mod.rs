//! The texts of the stores (`folia_design::texts!`; how the languages work: `folia_design::i18n`).

pub mod devices;
pub mod marks;
pub mod myprogram;
pub mod planner;
pub mod profiles;

pub use folia_design::i18n::{app_path, locale, of_address, use_location, Locale};

folia_design::texts! {
    common = folia_design::i18n::common,
    devices = crate::i18n::devices,
    marks = crate::i18n::marks,
    myprogram = crate::i18n::myprogram,
    planner = crate::i18n::planner,
    profiles = crate::i18n::profiles,
    plans_data = folia_plans::i18n,
    timetable_data = folia_timetable::i18n,
    ui = folia_design::i18n::ui,
}
