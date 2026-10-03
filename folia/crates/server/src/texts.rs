//! What the server writes itself, in every language of the site (docs/folia/i18n.md): the link-preview
//! cards, the manifest of the installed app, the login page of closed testing, the answers that
//! are no page. The pages' own words are the app's (`folia_app::i18n`).

use folia_locale::Locale;

pub struct Texts {
    /// Cards (`cards`, `api::*_card`).
    pub card_module: fn(&str) -> String,
    pub card_program: &'static str,
    pub teaches_both: &'static str,
    pub teaches_german: &'static str,
    pub teaches_english: &'static str,
    /// „Prüfungsordnung 2008".
    pub regulations: fn(&str) -> String,
    /// „114 Module im Curriculum"; the number is written already.
    pub curricular_modules: fn(&str) -> String,
    pub with_plan: &'static str,
    pub bookmarks_eyebrow: &'static str,
    pub bookmarks_title: &'static str,
    pub bookmarks_facts: [&'static str; 2],
    pub bookmarks_note: &'static str,
    pub studyplan_eyebrow: &'static str,
    /// „Stundenplan · WiSe 2026/27".
    pub studyplan_of: fn(&str) -> String,
    pub studyplan_title: &'static str,
    pub studyplan_facts: [&'static str; 3],
    pub studyplan_note: &'static str,
    /// The manifest of the installed app.
    pub app_name: &'static str,
    pub app_description: &'static str,
    /// The login page of closed testing (`access`).
    pub gate_title: &'static str,
    pub gate_text: &'static str,
    pub gate_password: &'static str,
    pub gate_open: &'static str,
    pub gate_wrong: &'static str,
    pub gate_closed: &'static str,
    /// A calendar address that names no plan.
    pub no_calendar: &'static str,
}

pub const DE: Texts = Texts {
    card_module: |id| format!("Modul {id}"),
    card_program: "Studiengang",
    teaches_both: "Deutsch und Englisch",
    teaches_german: "Deutsch",
    teaches_english: "Englisch",
    regulations: |version| format!("Prüfungsordnung {version}"),
    curricular_modules: |n| format!("{n} Module im Curriculum"),
    with_plan: "mit Regelstudienplan",
    bookmarks_eyebrow: "Merkliste",
    bookmarks_title: "Module merken und wiederfinden",
    bookmarks_facts: ["Kein Konto", "per Link auf ein anderes Gerät"],
    bookmarks_note: "Die Merkliste liegt nur im eigenen Browser.",
    studyplan_eyebrow: "Stundenplan",
    studyplan_of: |semester| format!("Stundenplan · {semester}"),
    studyplan_title: "Die Woche deiner Module",
    studyplan_facts: ["Termine", "Prüfungen", "Kalender-Abo"],
    studyplan_note: "Der Stundenplan liegt nur im eigenen Browser.",
    app_name: "Betula: Modulkatalog für die BTU Cottbus-Senftenberg (inoffiziell)",
    app_description: "Alle Module und Studiengänge der BTU Cottbus-Senftenberg: durchsuchen, filtern, Studium planen. Inoffiziell.",
    gate_title: "Geschlossener Test",
    gate_text: "Betula wird gerade in kleinem Kreis getestet. Mit dem Passwort aus deiner Einladung geht es weiter.",
    gate_password: "Passwort",
    gate_open: "Öffnen",
    gate_wrong: "Das Passwort stimmt nicht.",
    gate_closed: "Zu viele falsche Versuche. Bitte versuche es in einer Minute noch einmal.",
    no_calendar: "Kein Kalender unter dieser Adresse.\n",
};

pub const EN: Texts = Texts {
    card_module: |id| format!("Module {id}"),
    card_program: "Degree programme",
    teaches_both: "German and English",
    teaches_german: "German",
    teaches_english: "English",
    regulations: |version| format!("Examination regulations {version}"),
    curricular_modules: |n| format!("{n} modules in the curriculum"),
    with_plan: "with a standard study plan",
    bookmarks_eyebrow: "Saved modules",
    bookmarks_title: "Save modules and find them again",
    bookmarks_facts: ["No account", "to another device by a link"],
    bookmarks_note: "The saved modules stay in your own browser.",
    studyplan_eyebrow: "Timetable",
    studyplan_of: |semester| format!("Timetable · {semester}"),
    studyplan_title: "The week of your modules",
    studyplan_facts: ["Dates", "Exams", "Calendar subscription"],
    studyplan_note: "The timetable stays in your own browser.",
    app_name: "Betula: module catalogue for BTU Cottbus-Senftenberg (unofficial)",
    app_description: "Every module and degree programme of BTU Cottbus-Senftenberg: search, filter, plan your studies. Unofficial.",
    gate_title: "Closed testing",
    gate_text: "Betula is being tested by a small circle right now. The password from your invitation lets you in.",
    gate_password: "Password",
    gate_open: "Open",
    gate_wrong: "The password is not right.",
    gate_closed: "Too many wrong attempts. Please try again in a minute.",
    no_calendar: "No calendar at this address.\n",
};

/// The server's texts in a language.
pub fn texts(locale: Locale) -> &'static Texts {
    match locale {
        Locale::De => &DE,
        Locale::En => &EN,
    }
}
