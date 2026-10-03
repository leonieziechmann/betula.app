//! The texts of the app's parts that live in this crate, and the groups of the crates below it
//! that its pages write (`folia_design::texts!`; how the languages work: `folia_design::i18n`).


pub use folia_design::i18n::{app_path, locale, of_address, use_location, Locale};

folia_design::texts! {
    app = folia_shell::i18n::app,
    bookmarks = folia_bookmarks::i18n::bookmarks,
    catalog = folia_widgets::i18n::catalog,
    combobox = folia_design::i18n::combobox,
    common = folia_design::i18n::common,
    format = folia_design::i18n::format,
    ground = folia_shell::i18n::ground,
    home = folia_home::i18n::home,
    home_detail = folia_home::i18n::home_detail,
    legal = folia_home::i18n::legal,
    marks = folia_stores::i18n::marks,
    module = folia_widgets::i18n::module,
    myprogram = folia_stores::i18n::myprogram,
    planner = folia_stores::i18n::planner,
    plans_data = folia_plans::i18n,
    program = folia_programs::i18n::program,
    programs = folia_programs::i18n::programs,
    seo = folia_shell::i18n::seo,
    studyplan = folia_planner::i18n::studyplan,
    studyplan_aside = folia_planner::i18n::studyplan_aside,
    studyplan_exams = folia_planner::i18n::studyplan_exams,
    studyplan_export = folia_planner::i18n::studyplan_export,
    studyplan_head = folia_planner::i18n::studyplan_head,
    studyplan_import = folia_planner::i18n::studyplan_import,
    studyplan_modules = folia_planner::i18n::studyplan_modules,
    studyplan_share = folia_planner::i18n::studyplan_share,
    studyplan_side = folia_planner::i18n::studyplan_side,
    studyplan_week = folia_planner::i18n::studyplan_week,
    timetable_data = folia_timetable::i18n,
    ui = folia_design::i18n::ui,
    week = folia_widgets::i18n::week,
}
