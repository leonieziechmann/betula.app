//! The texts of home (`folia_design::texts!`; how the languages work: `folia_design::i18n`).

pub mod home;
pub mod home_detail;
pub mod legal;

pub use folia_design::i18n::{app_path, locale, of_address, use_location, Locale};

folia_design::texts! {
    app = folia_shell::i18n::app,
    catalog = folia_widgets::i18n::catalog,
    combobox = folia_design::i18n::combobox,
    common = folia_design::i18n::common,
    format = folia_design::i18n::format,
    ground = folia_shell::i18n::ground,
    home = crate::i18n::home,
    home_detail = crate::i18n::home_detail,
    legal = crate::i18n::legal,
    marks = folia_stores::i18n::marks,
    module = folia_widgets::i18n::module,
    myprogram = folia_stores::i18n::myprogram,
    planner = folia_stores::i18n::planner,
    plans_data = folia_plans::i18n,
    seo = folia_shell::i18n::seo,
    timetable_data = folia_timetable::i18n,
    ui = folia_design::i18n::ui,
    week = folia_widgets::i18n::week,
}
