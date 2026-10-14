//! Texts of the profile page (`profile.rs`).

pub struct Texts {
    /// The page's title in the browser's tab and its name: „Profil".
    pub title: &'static str,
    /// What search engines and link previews read of the page (the same for everybody: what a
    /// profile keeps lives in the browser).
    pub description: &'static str,
    /// The server's page, which cannot know any profile.
    pub server_title: &'static str,
    pub server_hint: &'static str,
    /// The sidebar's heading, and the name of the list of profiles: „Profile".
    pub profiles: &'static str,
    /// The answers of a question, and of a note.
    pub ok: &'static str,
    pub cancel: &'static str,

    // ---- the active profile, at the head of the page ----
    /// The name of the head, for screen readers: „Aktives Profil".
    pub active: &'static str,
    /// What a profile is, under its name.
    pub lead: &'static str,
    /// „Angelegt am 14.10.2026"; the value is the date.
    pub made_on: fn(&str) -> String,
    pub rename: &'static str,
    /// The name field, for screen readers, and its button.
    pub name: &'static str,
    pub save: &'static str,
    pub delete: &'static str,
    /// Why the only profile cannot be deleted (the tooltip of its greyed-out button).
    pub only_profile: &'static str,
    /// „„Zweitstudium“ mit allem, was es enthält, löschen?"; the value is the profile's name.
    pub delete_question: fn(&str) -> String,
    pub delete_yes: &'static str,

    // ---- notes at the head of the page ----
    /// „„Zweitstudium“ angelegt …", „„Zweitstudium“ aus der Datei angelegt.", „„Zweitstudium“
    /// gelöscht."; the value is the profile's name.
    pub created: fn(&str) -> String,
    pub opened: fn(&str) -> String,
    pub deleted: fn(&str) -> String,
    pub cleared: &'static str,

    // ---- „Mein Studiengang" ----
    pub program_heading: &'static str,
    pub no_program_hint: &'static str,
    /// The ways to set one: „Mein Studium" and the program overview.
    pub set_up: &'static str,
    pub all_programs: &'static str,
    pub change_program: &'static str,
    pub program: &'static str,
    /// Beside a program the snapshot no longer has.
    pub program_gone: &'static str,
    pub direction: &'static str,
    pub start: &'static str,
    /// „3. Fachsemester".
    pub fs: fn(u8) -> String,
    /// „Urlaubssemester" and „1 Semester".
    pub leave_heading: &'static str,
    pub leave: fn(usize) -> String,
    pub town: &'static str,
    /// The Standort where none is chosen (the Stundenplan's modules decide), and where both are.
    pub town_derived: &'static str,
    pub town_both: &'static str,

    // ---- what the profile keeps ----
    pub kept_heading: &'static str,
    pub bookmarks: &'static str,
    pub studyplan: &'static str,
    pub study: &'static str,
    pub saved_plans: &'static str,
    /// „8 Module in 2 Semestern": the modules planned, each once, and the semesters they are in.
    pub planned: fn(usize, usize) -> String,
    /// „15 Module bestanden".
    pub passed: fn(usize) -> String,
    /// „2 Pläne".
    pub plans: fn(usize) -> String,
    /// „Profil leeren", the line under it, its question and its answer, and why it is greyed out.
    pub clear: &'static str,
    pub clear_hint: &'static str,
    pub clear_question: &'static str,
    pub clear_yes: &'static str,
    pub nothing_to_clear: &'static str,

    // ---- the settings of the browser ----
    pub settings_heading: &'static str,
    pub settings_hint: &'static str,
    pub theme: &'static str,
    pub theme_system: &'static str,
    pub theme_light: &'static str,
    pub theme_dark: &'static str,
    pub language: &'static str,

    // ---- the sidebar ----
    /// The line under a profile without „Mein Studiengang".
    pub no_program: &'static str,
    pub new_profile: &'static str,
    /// The name field of a new profile, for screen readers, and its button.
    pub new_name: &'static str,
    pub create: &'static str,
    /// Why no profile can be made any more (the tooltip of the greyed-out button); the value is
    /// how many there may be.
    pub full: fn(usize) -> String,
    /// „Sichern & übertragen": the group of the file.
    pub backup: &'static str,
    pub save_file: &'static str,
    pub save_file_hint: &'static str,
    pub open_file: &'static str,
    pub open_file_hint: &'static str,
    /// A file picked: „„Zweitstudium“:", what it holds, the question and its answer.
    pub file_named: fn(&str) -> String,
    pub file_empty: &'static str,
    /// „12 gemerkte Module", „8 Module geplant", „2 gespeicherte Pläne": what a file holds.
    pub n_marked: fn(usize) -> String,
    pub n_planned: fn(usize) -> String,
    pub n_plans: fn(usize) -> String,
    pub file_question: &'static str,
    pub file_take: &'static str,
    /// A file that makes no profile, and why.
    pub file_unreadable: &'static str,
    pub file_newer: &'static str,
    pub file_too_large: &'static str,
    pub file_full: fn(usize) -> String,
    /// Where all of it lives.
    pub storage_hint: &'static str,
}

/// „1 Modul", „2 Module".
fn module_de(n: usize) -> &'static str {
    if n == 1 { "Modul" } else { "Module" }
}

fn module_en(n: usize) -> &'static str {
    if n == 1 { "module" } else { "modules" }
}

pub const DE: Texts = Texts {
    title: "Profil",
    description: "Profile und Einstellungen von Betula: Merkliste, Stundenplan und Studium getrennt für mehrere Profile, alles nur im eigenen Browser, ohne Konto.",
    server_title: "Deine Profile liegen in deinem Browser",
    server_hint: "Welche Profile es gibt und was sie enthalten, weiß nur dein Browser. Mit JavaScript siehst du sie hier und kannst sie verwalten.",
    profiles: "Profile",
    ok: "OK",
    cancel: "Abbrechen",

    active: "Aktives Profil",
    lead: "Merkliste, Stundenplan, „Mein Studium“ und „Mein Studiengang“ gehören zu diesem Profil. Jedes andere Profil hat seine eigenen.",
    made_on: |date| format!("Angelegt am {date}"),
    rename: "Umbenennen",
    name: "Name des Profils",
    save: "Speichern",
    delete: "Löschen",
    only_profile: "Das einzige Profil lässt sich nicht löschen, aber leeren.",
    delete_question: |name| format!("„{name}“ mit allem, was es enthält, löschen?"),
    delete_yes: "Löschen",

    created: |name| format!("„{name}“ angelegt: Merkliste, Stundenplan und Studium sind noch leer."),
    opened: |name| format!("„{name}“ aus der Datei angelegt."),
    deleted: |name| format!("„{name}“ gelöscht."),
    cleared: "Profil geleert.",

    program_heading: "Mein Studiengang",
    no_program_hint: "Noch kein Studiengang. Richte „Mein Studium“ ein oder setze einen Studiengang auf seiner Seite als deinen: Dann zeigt Betula dir seine Module zuerst.",
    set_up: "Studium einrichten",
    all_programs: "Alle Studiengänge",
    change_program: "In „Mein Studium“ ändern",
    program: "Studiengang",
    program_gone: "nicht mehr im Katalog",
    direction: "Studienrichtung",
    start: "Studienbeginn",
    fs: |fs| format!("{fs}. Fachsemester"),
    leave_heading: "Urlaubssemester",
    leave: |n| if n == 1 { "1 Semester".to_string() } else { format!("{n} Semester") },
    town: "Standort",
    town_derived: "nach dem Stundenplan",
    town_both: "Cottbus und Senftenberg",

    kept_heading: "In diesem Profil",
    bookmarks: "Merkliste",
    studyplan: "Stundenplan",
    study: "Mein Studium",
    saved_plans: "Gespeicherte Pläne",
    planned: |modules, semesters| match (modules, semesters) {
        (0, _) => "nichts geplant".to_string(),
        (n, 1) => format!("{n} {} in 1 Semester", module_de(n)),
        (n, s) => format!("{n} {} in {s} Semestern", module_de(n)),
    },
    passed: |n| if n == 0 { "nichts bestanden".to_string() } else { format!("{n} {} bestanden", module_de(n)) },
    plans: |n| match n {
        0 => "keine".to_string(),
        1 => "1 Plan".to_string(),
        n => format!("{n} Pläne"),
    },
    clear: "Profil leeren",
    clear_hint: "Das Profil bleibt, sein Inhalt geht",
    clear_question: "Merkliste, Stundenplan, „Mein Studium“, gespeicherte Pläne und „Mein Studiengang“ dieses Profils löschen?",
    clear_yes: "Leeren",
    nothing_to_clear: "Das Profil ist leer.",

    settings_heading: "Einstellungen",
    settings_hint: "Gelten in diesem Browser für alle Profile.",
    theme: "Darstellung",
    theme_system: "System",
    theme_light: "Hell",
    theme_dark: "Dunkel",
    language: "Sprache",

    no_program: "Kein Studiengang",
    new_profile: "Neues Profil",
    new_name: "Name des neuen Profils",
    create: "Anlegen",
    full: |max| format!("Mehr als {max} Profile gehen nicht: Lösche erst eines."),
    backup: "Sichern & übertragen",
    save_file: "Als Datei sichern",
    save_file_hint: "Für später oder ein anderes Gerät",
    open_file: "Aus Datei laden",
    open_file_hint: "Legt ein neues Profil an",
    file_named: |name| format!("„{name}“:"),
    file_empty: "Ein leeres Profil.",
    n_marked: |n| if n == 1 { "1 gemerktes Modul".to_string() } else { format!("{n} gemerkte Module") },
    n_planned: |n| format!("{n} {} geplant", module_de(n)),
    n_plans: |n| if n == 1 { "1 gespeicherter Plan".to_string() } else { format!("{n} gespeicherte Pläne") },
    file_question: "Als neues Profil anlegen?",
    file_take: "Anlegen",
    file_unreadable: "Diese Datei ist keine Sicherung eines Profils von Betula.",
    file_newer: "Diese Sicherung stammt aus einer neueren Version von Betula. Lade die Seite neu und versuche es noch einmal.",
    file_too_large: "Diese Datei ist zu groß für eine Sicherung.",
    file_full: |max| format!("Es gibt schon {max} Profile. Lösche eines, dann lässt sich die Datei laden."),
    storage_hint: "Profile liegen nur in diesem Browser: kein Konto, nichts auf dem Server. Ein anderes Gerät hat seine eigenen; eine Sicherung als Datei bringt ein Profil hinüber.",
};

pub const EN: Texts = Texts {
    title: "Profile",
    description: "Profiles and settings of Betula: saved modules, timetable and studies kept apart for several profiles, all only in your own browser, without an account.",
    server_title: "Your profiles are kept in your browser",
    server_hint: "Only your browser knows which profiles there are and what they hold. With JavaScript you see them here and can manage them.",
    profiles: "Profiles",
    ok: "OK",
    cancel: "Cancel",

    active: "Active profile",
    lead: "Your saved modules, timetable, “My studies” and “My programme” belong to this profile. Every other profile has its own.",
    made_on: |date| format!("Created on {date}"),
    rename: "Rename",
    name: "Name of the profile",
    save: "Save",
    delete: "Delete",
    only_profile: "The only profile cannot be deleted, but it can be emptied.",
    delete_question: |name| format!("Delete “{name}” with everything it holds?"),
    delete_yes: "Delete",

    created: |name| format!("“{name}” created: its saved modules, timetable and studies are empty for now."),
    opened: |name| format!("“{name}” created from the file."),
    deleted: |name| format!("“{name}” deleted."),
    cleared: "Profile emptied.",

    program_heading: "My programme",
    no_program_hint: "No degree programme yet. Set up “My studies”, or set a programme as yours on its page: then Betula shows you its modules first.",
    set_up: "Set up my studies",
    all_programs: "All degree programmes",
    change_program: "Change in “My studies”",
    program: "Degree programme",
    program_gone: "no longer in the catalogue",
    direction: "Study track",
    start: "Start of studies",
    fs: |fs| format!("Semester {fs}"),
    leave_heading: "Semesters of leave",
    leave: |n| if n == 1 { "1 semester".to_string() } else { format!("{n} semesters") },
    town: "Location",
    town_derived: "from the timetable",
    town_both: "Cottbus and Senftenberg",

    kept_heading: "In this profile",
    bookmarks: "Saved modules",
    studyplan: "Timetable",
    study: "My studies",
    saved_plans: "Saved plans",
    planned: |modules, semesters| match (modules, semesters) {
        (0, _) => "nothing planned".to_string(),
        (n, 1) => format!("{n} {} in 1 semester", module_en(n)),
        (n, s) => format!("{n} {} in {s} semesters", module_en(n)),
    },
    passed: |n| if n == 0 { "nothing passed".to_string() } else { format!("{n} {} passed", module_en(n)) },
    plans: |n| match n {
        0 => "none".to_string(),
        1 => "1 plan".to_string(),
        n => format!("{n} plans"),
    },
    clear: "Empty profile",
    clear_hint: "The profile stays, what it holds goes",
    clear_question: "Delete the saved modules, timetable, “My studies”, saved plans and “My programme” of this profile?",
    clear_yes: "Empty",
    nothing_to_clear: "The profile is empty.",

    settings_heading: "Settings",
    settings_hint: "They apply to every profile in this browser.",
    theme: "Appearance",
    theme_system: "System",
    theme_light: "Light",
    theme_dark: "Dark",
    language: "Language",

    no_program: "No degree programme",
    new_profile: "New profile",
    new_name: "Name of the new profile",
    create: "Create",
    full: |max| format!("There can be no more than {max} profiles: delete one first."),
    backup: "Back up & transfer",
    save_file: "Save as a file",
    save_file_hint: "For later or another device",
    open_file: "Load from a file",
    open_file_hint: "Creates a new profile",
    file_named: |name| format!("“{name}”:"),
    file_empty: "An empty profile.",
    n_marked: |n| if n == 1 { "1 saved module".to_string() } else { format!("{n} saved modules") },
    n_planned: |n| format!("{n} {} planned", module_en(n)),
    n_plans: |n| if n == 1 { "1 saved plan".to_string() } else { format!("{n} saved plans") },
    file_question: "Create it as a new profile?",
    file_take: "Create",
    file_unreadable: "This file is not a backup of a Betula profile.",
    file_newer: "This backup comes from a newer version of Betula. Reload the page and try again.",
    file_too_large: "This file is too large to be a backup.",
    file_full: |max| format!("There are {max} profiles already. Delete one, then the file can be loaded."),
    storage_hint: "Profiles are kept only in this browser: no account, nothing on the server. Another device has its own; a backup file takes a profile there.",
};
