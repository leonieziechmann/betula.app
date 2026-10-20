//! Texts of „Befehle" (`crate::commands`): the button in the search field, the dialog with its
//! field, its groups and its foot, the settings it offers itself, and the note that teaches a key.

pub struct Texts {
    /// The button in the search field, and its tooltip before its keys.
    pub button: &'static str,
    pub button_title: &'static str,
    /// The dialog's name, its field's name and what the field says while empty.
    pub title: &'static str,
    pub field: &'static str,
    pub placeholder: &'static str,
    /// The headings of the groups the palette has itself (a page names its own, `data-commands`).
    pub go: &'static str,
    pub page: &'static str,
    pub modules: &'static str,
    pub settings: &'static str,
    pub help: &'static str,
    /// Under the commands while nothing is typed: what typing finds besides them.
    pub typing_finds: &'static str,
    pub finds_filters: &'static str,
    pub finds_modules: &'static str,
    pub finds_settings: &'static str,
    /// Where nothing is found.
    pub nothing: &'static str,
    /// The modules still being looked for.
    pub searching: &'static str,
    /// The foot: the keys of the list, Shift + Enter, Esc.
    pub foot_choose: &'static str,
    pub foot_run: &'static str,
    pub foot_alt: &'static str,
    pub foot_close: &'static str,
    /// What a screen reader is told of the list: „12 Treffer".
    pub count: fn(usize) -> String,
    /// After a command with a key ran by its name: „Geht auch mit M".
    pub taught: fn(&str) -> String,
    /// The settings: light, dark or the system's, the other language, Betula's single keys.
    pub theme_light: &'static str,
    pub theme_dark: &'static str,
    pub theme_system: &'static str,
    pub theme_words: &'static str,
    /// The mark at the setting that holds now.
    pub now: &'static str,
    pub language_words: &'static str,
    pub letters_off: &'static str,
    pub letters_on: &'static str,
    pub letters_words: &'static str,
    /// The search field and the side of a page as commands of „Gehe zu".
    pub search_field: &'static str,
    /// A module found: what Shift + Enter does with it, and the words it is found by besides its
    /// title and number.
    pub module_words: &'static str,
}

pub const DE: Texts = Texts {
    button: "Befehle",
    button_title: "Befehle: eine Aktion, einen Filter, ein Modul oder eine Seite tippen",
    title: "Befehle",
    field: "Befehl oder Modul suchen",
    placeholder: "Was soll Betula tun? Ein Modul, ein Filter, eine Aktion …",
    go: "Gehe zu",
    page: "Auf dieser Seite",
    modules: "Module",
    settings: "Einstellungen",
    help: "Hilfe",
    typing_finds: "Tippen findet auch",
    finds_filters: "jeden Filter, z. B. „klausur“ oder „englisch“",
    finds_modules: "jedes Modul, z. B. „datenbanken“ oder „12330“",
    finds_settings: "die Einstellungen, z. B. „dunkel“ oder „english“",
    nothing: "Nichts gefunden. Betula sucht in Befehlen, Filtern, Modulen und Einstellungen.",
    searching: "Module werden gesucht …",
    foot_choose: "wählen",
    foot_run: "ausführen",
    foot_alt: "Filter „ohne“, Modul im Vollbild",
    foot_close: "schließen",
    count: |n| if n == 1 { "1 Treffer".to_string() } else { format!("{n} Treffer") },
    taught: |keys| format!("Geht auch mit {keys}"),
    theme_light: "Hell",
    theme_dark: "Dunkel",
    theme_system: "Hell oder dunkel wie das System",
    theme_words: "design farben modus nacht licht theme",
    now: "jetzt",
    language_words: "sprache language",
    letters_off: "Einzelne Tasten (F, ?, M …) ausschalten",
    letters_on: "Einzelne Tasten (F, ?, M …) einschalten",
    letters_words: "barrierefrei tastatur kürzel shortcuts",
    search_field: "Suchfeld",
    module_words: "modul öffnen",
};

pub const EN: Texts = Texts {
    button: "Commands",
    button_title: "Commands: type an action, a filter, a module or a page",
    title: "Commands",
    field: "Search commands or modules",
    placeholder: "What should Betula do? A module, a filter, an action …",
    go: "Go to",
    page: "On this page",
    modules: "Modules",
    settings: "Settings",
    help: "Help",
    typing_finds: "Typing also finds",
    finds_filters: "every filter, e.g. “exam” or “english”",
    finds_modules: "every module, e.g. “databases” or “12330”",
    finds_settings: "the settings, e.g. “dark” or “deutsch”",
    nothing: "Nothing found. Betula searches commands, filters, modules and settings.",
    searching: "Looking for modules …",
    foot_choose: "choose",
    foot_run: "run",
    foot_alt: "filter “without”, module in full",
    foot_close: "close",
    count: |n| if n == 1 { "1 result".to_string() } else { format!("{n} results") },
    taught: |keys| format!("Also works with {keys}"),
    theme_light: "Light",
    theme_dark: "Dark",
    theme_system: "Light or dark as the system is",
    theme_words: "design colours colors mode night theme",
    now: "now",
    language_words: "language sprache",
    letters_off: "Turn single keys (F, ?, M …) off",
    letters_on: "Turn single keys (F, ?, M …) on",
    letters_words: "accessibility keyboard shortcuts",
    search_field: "Search field",
    module_words: "module open",
};
