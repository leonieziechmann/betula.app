//! Texts of the small building blocks of every page (`ui.rs`): states, badges, facts, the frame.

pub struct Texts {
    pub unavailable_title: &'static str,
    pub unavailable_hint: &'static str,
    pub failed_title: &'static str,
    pub failed_hint: &'static str,
    pub reload: &'static str,
    /// The banner of a browser app that crashed, and its link.
    pub crashed: &'static str,
    pub reload_page: &'static str,
    /// „Art nicht angegeben": no source states the kind of a module in a program.
    pub kind_unknown: &'static str,
    pub resize_sidebar: &'static str,
    pub resize_preview: &'static str,
    /// „Zurück (Esc)".
    pub back_title: &'static str,
    pub to_top: &'static str,
}

pub const DE: Texts = Texts {
    unavailable_title: "Der Katalog ist gerade nicht verfügbar",
    unavailable_hint: "Die Daten werden noch geladen. Bitte versuche es gleich noch einmal.",
    failed_title: "Etwas ist schiefgelaufen",
    failed_hint: "Die Seite konnte nicht geladen werden. Bitte lade sie neu.",
    reload: "Neu laden",
    crashed: "Etwas ist schiefgelaufen.",
    reload_page: "Seite neu laden",
    kind_unknown: "Art nicht angegeben",
    resize_sidebar: "Breite der Seitenleiste ändern (Pfeiltasten, Doppelklick setzt zurück)",
    resize_preview: "Breite der Vorschau ändern (Pfeiltasten, Doppelklick setzt zurück)",
    back_title: "Zurück (Esc)",
    to_top: "Nach oben",
};

pub const EN: Texts = Texts {
    unavailable_title: "The catalogue is not available right now",
    unavailable_hint: "The data is still loading. Please try again in a moment.",
    failed_title: "Something went wrong",
    failed_hint: "The page could not be loaded. Please reload it.",
    reload: "Reload",
    crashed: "Something went wrong.",
    reload_page: "Reload the page",
    kind_unknown: "Kind not stated",
    resize_sidebar: "Change the width of the sidebar (arrow keys; a double click resets it)",
    resize_preview: "Change the width of the preview (arrow keys; a double click resets it)",
    back_title: "Back (Esc)",
    to_top: "Back to top",
};
