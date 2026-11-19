//! Texts of „Mein Studium" with the keyboard (folia/design/tastatur/konzept.html, „Die anderen
//! Seiten"; enhance.js, „the keyboard"): the sidebar as a mode and as a command of „Gehe zu", and
//! what „Befehle" offers at a row of a semester and in the Gesamtplan.

pub struct Texts {
    /// The sidebar as a command of „Gehe zu" (F), and the way out of it that a screen reader hears
    /// at either of its ends.
    pub sidebar: &'static str,
    pub leave: &'static str,
    /// The sidebar's first section, the card of the program, by its name.
    pub card: &'static str,
    /// A row's menu as a command, with its key (Shift + F10), at the row the keyboard is on.
    pub row_menu: &'static str,
}

pub const DE: Texts = Texts {
    sidebar: "Ansichten und Studiengang",
    leave: "Escape führt zur Ansicht",
    card: "Dein Studiengang",
    row_menu: "Menü öffnen",
};

pub const EN: Texts = Texts {
    sidebar: "Views and programme",
    leave: "Escape leads to the view",
    card: "Your programme",
    row_menu: "Open the menu",
};
