//! The texts of the app's parts that live in this crate, and the groups of the crates below it
//! that its pages write (`folia_design::texts!`; how the languages work: `folia_design::i18n`).

pub mod app;
pub mod bookmarks;
pub mod catalog;
pub mod home;
pub mod home_detail;
pub mod legal;
pub mod module;
pub mod program;
pub mod programs;
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
pub mod week;

pub use folia_design::i18n::{app_path, locale, of_address, use_location, Locale};

folia_design::texts! {
    app = crate::i18n::app,
    bookmarks = crate::i18n::bookmarks,
    catalog = crate::i18n::catalog,
    combobox = folia_design::i18n::combobox,
    common = folia_design::i18n::common,
    format = folia_design::i18n::format,
    ground = folia_shell::i18n::ground,
    home = crate::i18n::home,
    home_detail = crate::i18n::home_detail,
    legal = crate::i18n::legal,
    marks = folia_stores::i18n::marks,
    module = crate::i18n::module,
    myprogram = folia_stores::i18n::myprogram,
    planner = folia_stores::i18n::planner,
    plans_data = folia_plans::i18n,
    program = crate::i18n::program,
    programs = crate::i18n::programs,
    seo = folia_shell::i18n::seo,
    studyplan = crate::i18n::studyplan,
    studyplan_aside = crate::i18n::studyplan_aside,
    studyplan_exams = crate::i18n::studyplan_exams,
    studyplan_export = crate::i18n::studyplan_export,
    studyplan_head = crate::i18n::studyplan_head,
    studyplan_import = crate::i18n::studyplan_import,
    studyplan_modules = crate::i18n::studyplan_modules,
    studyplan_share = crate::i18n::studyplan_share,
    studyplan_side = crate::i18n::studyplan_side,
    studyplan_week = crate::i18n::studyplan_week,
    timetable_data = folia_timetable::i18n,
    ui = folia_design::i18n::ui,
    week = crate::i18n::week,
}
