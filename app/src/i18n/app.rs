//! Texts of the frame of every page: the rail, the top bar with its search, the bottom bar of a
//! phone (`lib.rs`).

pub struct Texts {
    /// The title of a page that sets none (the start page names itself).
    pub default_title: &'static str,
    pub skip_to_content: &'static str,
    pub not_found_title: &'static str,
    pub not_found_hint: &'static str,
    /// The names of the navigations, for screen readers.
    pub navigation: &'static str,
    pub main_navigation: &'static str,
    /// The areas, as the rail and the bottom bar name them.
    pub home: &'static str,
    pub modules: &'static str,
    pub programs: &'static str,
    /// The programs' tab in the rail, where „Studiengänge" is too long.
    pub study: &'static str,
    pub bookmarks: &'static str,
    pub studyplan: &'static str,
    /// The number on the tab of the Merkliste and the Stundenplan: „3 gemerkt", „5 geplant".
    pub marked_count: fn(usize) -> String,
    pub planned_count: fn(usize) -> String,
    pub logo_label: &'static str,
    pub theme_toggle: &'static str,
    pub search_programs: &'static str,
    pub search_programs_placeholder: &'static str,
    pub search_modules: &'static str,
    pub search_modules_placeholder: &'static str,
}

pub const DE: Texts = Texts {
    default_title: "Modulkatalog der BTU Cottbus-Senftenberg · Betula (inoffiziell)",
    skip_to_content: "Zum Inhalt springen",
    not_found_title: "Seite nicht gefunden",
    not_found_hint: "Diese Adresse gibt es nicht (mehr).",
    navigation: "Navigation",
    main_navigation: "Hauptnavigation",
    home: "Start",
    modules: "Module",
    programs: "Studiengänge",
    study: "Studium",
    bookmarks: "Merkliste",
    studyplan: "Stundenplan",
    marked_count: |n| format!("{n} gemerkt"),
    planned_count: |n| format!("{n} geplant"),
    logo_label: "Betula, zur Startseite",
    theme_toggle: "Hell oder dunkel",
    search_programs: "Studiengänge suchen",
    search_programs_placeholder: "Studiengang suchen",
    search_modules: "Module suchen",
    search_modules_placeholder: "Modul, Nummer oder Thema suchen",
};

pub const EN: Texts = Texts {
    default_title: "Module catalogue of BTU Cottbus-Senftenberg · Betula (unofficial)",
    skip_to_content: "Skip to content",
    not_found_title: "Page not found",
    not_found_hint: "This address does not exist (any more).",
    navigation: "Navigation",
    main_navigation: "Main navigation",
    home: "Home",
    modules: "Modules",
    programs: "Degree programmes",
    study: "Study",
    bookmarks: "Saved",
    studyplan: "Timetable",
    marked_count: |n| format!("{n} saved"),
    planned_count: |n| format!("{n} planned"),
    logo_label: "Betula, to the home page",
    theme_toggle: "Light or dark",
    search_programs: "Search degree programmes",
    search_programs_placeholder: "Search degree programmes",
    search_modules: "Search modules",
    search_modules_placeholder: "Search modules, numbers or topics",
};
