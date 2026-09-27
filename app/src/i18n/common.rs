//! Texts many parts of the app share: actions, the name of the site, the language switch.

pub struct Texts {
    /// What is not known (R12): „nicht angegeben", never a guess.
    pub not_stated: &'static str,
    pub back: &'static str,
    pub close: &'static str,
    /// „Vollbild": the module shown beside a page fills the page.
    pub full_view: &'static str,
    pub undo: &'static str,
    pub reset: &'static str,
    pub to_catalog: &'static str,
    pub to_programs: &'static str,
    /// Under the wordmark: what Betula is, and that it is not the BTU's.
    pub tagline: &'static str,
    /// The unit of credits: „LP" (Leistungspunkte).
    pub credits_unit: &'static str,
    /// The name of the language switch, for screen readers.
    pub language: &'static str,
    /// The keys of the search shortcut as the keyboard labels them.
    pub search_shortcut: &'static str,
}

pub const DE: Texts = Texts {
    not_stated: "nicht angegeben",
    back: "Zurück",
    close: "Schließen",
    full_view: "Vollbild",
    undo: "Rückgängig",
    reset: "Zurücksetzen",
    to_catalog: "Zum Modulkatalog",
    to_programs: "Zu den Studiengängen",
    tagline: "Modulkatalog · inoffiziell",
    credits_unit: "LP",
    language: "Sprache",
    search_shortcut: "Strg K",
};

pub const EN: Texts = Texts {
    not_stated: "not stated",
    back: "Back",
    close: "Close",
    full_view: "Full page",
    undo: "Undo",
    reset: "Reset",
    to_catalog: "To the module catalogue",
    to_programs: "To the degree programmes",
    tagline: "Module catalogue · unofficial",
    credits_unit: "CP",
    language: "Language",
    search_shortcut: "Ctrl K",
};
