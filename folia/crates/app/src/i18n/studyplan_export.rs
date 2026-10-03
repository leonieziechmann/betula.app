//! Texts of the Stundenplan as a calendar (`pages/studyplan/export.rs`): the file to download and
//! the calendar subscription. (The calendar's own words, and its name in Outlook, are the feed's:
//! `catalog::i18n::timetable`.)

pub struct Texts {
    /// „Kalender": the group's heading.
    pub calendar: &'static str,
    /// „.ics herunterladen" …
    pub download: &'static str,
    /// … and under it why there is no file: nothing of the semester is published yet, or nothing
    /// of the plan has a date that is shown.
    pub not_yet: &'static str,
    pub none: &'static str,
    /// The downloaded file's name, by the semester's key („2026W"): „studienplan-2026W.ics".
    pub file_name: fn(&str) -> String,
    /// „Abonnieren": opens the ways to subscribe.
    pub subscribe: &'static str,
    /// More is hidden or chosen than one address carries …
    pub too_much_hidden: &'static str,
    /// … and the way back to all Termine.
    pub show_all: &'static str,
    /// The calendar services (Outlook is a name).
    pub apple: &'static str,
    pub google: &'static str,
    /// „Adresse kopieren", and what the address is.
    pub copy_address: &'static str,
    pub subscription_hint: &'static str,
    /// The plan has moved on from the address last handed out: „Abo veraltet: …", „Neue Adresse
    /// kopieren", and after its click „Neue Adresse kopiert".
    pub stale: &'static str,
    pub copy_new: &'static str,
    pub new_copied: &'static str,
}

pub const DE: Texts = Texts {
    calendar: "Kalender",
    download: ".ics herunterladen",
    not_yet: "noch keine Termine",
    none: "keine Termine",
    file_name: |key| format!("studienplan-{key}.ics"),
    subscribe: "Abonnieren",
    too_much_hidden: "Zu viel ausgeblendet für ein Abo.",
    show_all: "Alle einblenden",
    apple: "Apple Kalender",
    google: "Google Kalender",
    copy_address: "Adresse kopieren",
    subscription_hint: "Die Adresse enthält Semester, Module und Ausgeblendetes; dein Kalender holt Änderungen selbst (Google etwa täglich).",
    stale: "Abo veraltet: Plan seitdem geändert",
    copy_new: "Neue Adresse kopieren",
    new_copied: "Neue Adresse kopiert",
};

pub const EN: Texts = Texts {
    calendar: "Calendar",
    download: "Download .ics",
    not_yet: "no dates yet",
    none: "no dates",
    file_name: |key| format!("timetable-{key}.ics"),
    subscribe: "Subscribe",
    too_much_hidden: "Too much hidden for a subscription.",
    show_all: "Show all",
    apple: "Apple Calendar",
    google: "Google Calendar",
    copy_address: "Copy address",
    subscription_hint: "The address contains the semester, the modules and what is hidden; your calendar fetches changes by itself (Google about once a day).",
    stale: "Subscription out of date: plan changed since",
    copy_new: "Copy new address",
    new_copied: "New address copied",
};
