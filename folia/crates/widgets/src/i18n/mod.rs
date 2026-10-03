//! The texts of the widgets (`folia_design::texts!`; how the languages work: `folia_design::i18n`).
//! `catalog` is the catalog's group: its rows and chips are widgets, its page writes it as well.

pub mod catalog;
pub mod module;
pub mod week;

pub use folia_design::i18n::{app_path, locale, of_address, use_location, Locale};

folia_design::texts! {
    catalog = crate::i18n::catalog,
    combobox = folia_design::i18n::combobox,
    common = folia_design::i18n::common,
    format = folia_design::i18n::format,
    marks = folia_stores::i18n::marks,
    module = crate::i18n::module,
    myprogram = folia_stores::i18n::myprogram,
    planner = folia_stores::i18n::planner,
    plans_data = folia_plans::i18n,
    seo = folia_shell::i18n::seo,
    timetable_data = folia_timetable::i18n,
    ui = folia_design::i18n::ui,
    week = crate::i18n::week,
}
