//! Texts of the small building blocks of every page (`ui.rs`): states, badges, facts, the frame.

use folia_model::labels::KindSource;

pub struct Texts {
    pub unavailable_title: &'static str,
    pub unavailable_hint: &'static str,
    pub failed_title: &'static str,
    pub failed_hint: &'static str,
    pub reload: &'static str,
    /// The banner of a browser app that crashed, and its link.
    pub crashed: &'static str,
    pub reload_page: &'static str,
    /// „ · abgeleitet" after a kind no source states and Betula infers.
    pub kind_inferred: &'static str,
    /// The title of such a kind: from what it is inferred.
    pub kind_inferred_title: fn(Option<KindSource>) -> &'static str,
    /// „Nebenfach" instead of „Wahlpflicht" for a module in the program's minor, and its title.
    pub kind_minor: &'static str,
    pub kind_minor_title: &'static str,
    pub resize_sidebar: &'static str,
    pub resize_preview: &'static str,
    /// „Zurück (Esc)".
    pub back_title: &'static str,
    pub to_top: &'static str,
    /// The switch for Betula's single keys (`keys::LettersSwitch`): its name, its two choices, and
    /// what it does.
    pub letters: &'static str,
    pub letters_on: &'static str,
    pub letters_off: &'static str,
    pub letters_hint: &'static str,
}

pub const DE: Texts = Texts {
    unavailable_title: "Der Katalog ist gerade nicht verfügbar",
    unavailable_hint: "Die Daten werden noch geladen. Bitte versuche es gleich noch einmal.",
    failed_title: "Etwas ist schiefgelaufen",
    failed_hint: "Die Seite konnte nicht geladen werden. Bitte lade sie neu.",
    reload: "Neu laden",
    crashed: "Etwas ist schiefgelaufen.",
    reload_page: "Seite neu laden",
    kind_inferred: " · abgeleitet",
    kind_inferred_title: |source| match source {
        Some(KindSource::PdfPlan) => "Abgeleitet: Der Regelstudienplan sieht einen Wahlpflichtplatz vor, der aus diesem Bereich des Modulbaums belegt wird. Ausdrücklich nennt keine Quelle die Art.",
        Some(KindSource::QisTree) => "Abgeleitet aus dem Modulbaum und dem Titel des Moduls. Ausdrücklich nennt keine Quelle die Art.",
        _ => "Abgeleitet. Ausdrücklich nennt keine Quelle die Art.",
    },
    kind_minor: "Nebenfach",
    kind_minor_title: "Im Nebenfach des Studiengangs, so steht es im Modulbaum: belegt als Teil des gewählten Nebenfachs.",
    resize_sidebar: "Breite der Seitenleiste ändern (Pfeiltasten, Doppelklick setzt zurück)",
    resize_preview: "Breite der Vorschau ändern (Pfeiltasten, Doppelklick setzt zurück)",
    back_title: "Zurück (Esc)",
    to_top: "Nach oben",
    letters: "Einzelne Tasten",
    letters_on: "An",
    letters_off: "Aus",
    letters_hint: "F, V, M, P, / und ? (auch mit Shift) wirken ohne Strg. Aus, tun sie nichts und stehen nirgends; Strg + K, Tab, die Pfeile, Enter und Esc bleiben, und jeder Knopf sowieso. Für Spracheingabe und wer sich vertippt.",
};

pub const EN: Texts = Texts {
    unavailable_title: "The catalogue is not available right now",
    unavailable_hint: "The data is still loading. Please try again in a moment.",
    failed_title: "Something went wrong",
    failed_hint: "The page could not be loaded. Please reload it.",
    reload: "Reload",
    crashed: "Something went wrong.",
    reload_page: "Reload the page",
    kind_inferred: " · inferred",
    kind_inferred_title: |source| match source {
        Some(KindSource::PdfPlan) => "Inferred: the study plan has a compulsory elective slot that is filled from this area of the module tree. No source states the kind itself.",
        Some(KindSource::QisTree) => "Inferred from the module tree and the module's title. No source states the kind itself.",
        _ => "Inferred. No source states the kind itself.",
    },
    kind_minor: "Minor",
    kind_minor_title: "In the program's minor, as its module tree has it: taken as part of the chosen minor.",
    resize_sidebar: "Change the width of the sidebar (arrow keys; a double click resets it)",
    resize_preview: "Change the width of the preview (arrow keys; a double click resets it)",
    back_title: "Back (Esc)",
    to_top: "Back to top",
    letters: "Single keys",
    letters_on: "On",
    letters_off: "Off",
    letters_hint: "F, V, M, P, / and ? (with Shift as well) work without Ctrl. Off, they do nothing and are shown nowhere; Ctrl + K, Tab, the arrows, Enter and Esc stay, and every button anyway. For voice input and for typos.",
};
