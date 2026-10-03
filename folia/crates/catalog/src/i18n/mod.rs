//! The texts of catalog (`folia_design::texts!`; how the languages work: `folia_design::i18n`).


pub use folia_design::i18n::{app_path, locale, of_address, use_location, Locale};

folia_design::texts! {
    app = folia_shell::i18n::app,
    catalog = folia_widgets::i18n::catalog,
    combobox = folia_design::i18n::combobox,
    common = folia_design::i18n::common,
    format = folia_design::i18n::format,
    ground = folia_shell::i18n::ground,
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
