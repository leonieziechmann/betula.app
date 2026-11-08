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
    /// „Profil seit 14.10.2026"; the value is the date it was made.
    pub made_on: fn(&str) -> String,
    /// The pencil beside the name, the name field (for screen readers) and its button.
    pub rename: &'static str,
    pub name: &'static str,
    pub save: &'static str,
    /// „Mein Studiengang" as a line under the name: none yet; „3. Fachsemester"; both Standorte.
    pub no_program: &'static str,
    pub fs: fn(u8) -> String,
    pub town_both: &'static str,
    /// The numbers under the name, each a value over its label: saved; planned (the value: in
    /// how many semesters); passed (the value: how many modules); collections and notes; saved
    /// plans. An empty profile says so instead.
    pub stat_marked: &'static str,
    pub stat_planned: fn(usize) -> String,
    pub stat_passed: fn(usize) -> String,
    pub stat_kept: &'static str,
    pub stat_plans: &'static str,
    pub stats_empty: &'static str,

    // ---- notes at the head of the page ----
    /// „„Zweitstudium“ angelegt …", „„Zweitstudium“ aus der Datei angelegt.", „„Zweitstudium“
    /// gelöscht."; the value is the profile's name.
    pub created: fn(&str) -> String,
    pub opened: fn(&str) -> String,
    pub deleted: fn(&str) -> String,
    pub cleared: &'static str,

    // ---- „Daten" ----
    pub data_heading: &'static str,
    /// „Als Datei sichern", the sentence under it and its button.
    pub save_file: &'static str,
    pub save_file_hint: &'static str,
    pub save_button: &'static str,

    // ---- „Gefahrenzone" ----
    pub danger_heading: &'static str,
    pub danger_hint: &'static str,
    /// „Profil leeren": the sentence under it (the value is the profile's name), its button, its
    /// question and its answer, and why it is greyed out.
    pub clear: &'static str,
    pub clear_hint: fn(&str) -> String,
    pub clear_button: &'static str,
    pub clear_question: &'static str,
    pub clear_yes: &'static str,
    pub nothing_to_clear: &'static str,
    /// „Profil löschen": the same; the only profile is emptied instead, and says so.
    pub delete: &'static str,
    pub delete_hint: fn(&str) -> String,
    pub delete_only: &'static str,
    pub delete_button: &'static str,
    pub delete_question: fn(&str) -> String,
    pub delete_yes: &'static str,

    // ---- the sidebar: the profiles and „Profil hinzufügen" ----
    pub add_profile: &'static str,
    /// The ways: an empty one, one from another device, one from a file.
    pub add_empty: &'static str,
    pub add_other: &'static str,
    /// The name field of a new profile, for screen readers, and its button.
    pub new_name: &'static str,
    pub create: &'static str,
    /// Why no profile can be made any more (the tooltip of the greyed-out button); the value is
    /// how many there may be.
    pub full: fn(usize) -> String,
    pub open_file: &'static str,
    /// A file picked: „„Zweitstudium“:", what it holds, the question and its answer.
    pub file_named: fn(&str) -> String,
    pub file_empty: &'static str,
    /// „12 gemerkte Module", „8 Module geplant", „2 gespeicherte Pläne": what a file holds.
    pub n_marked: fn(usize) -> String,
    pub n_planned: fn(usize) -> String,
    pub n_plans: fn(usize) -> String,
    /// „3 Sammlungen", „2 Notizen": the Merkliste's collections and notes a file holds.
    pub n_collections: fn(usize) -> String,
    pub n_notes: fn(usize) -> String,
    /// „1 Modul bestanden", what a file holds.
    pub passed: fn(usize) -> String,
    pub file_question: &'static str,
    pub file_take: &'static str,
    /// A file that makes no profile, and why.
    pub file_unreadable: &'static str,
    pub file_newer: &'static str,
    pub file_too_large: &'static str,
    pub file_full: fn(usize) -> String,
    /// Where all of it lives; where the site offers Betula Sync, with it.
    pub storage_hint: &'static str,
    pub storage_hint_sync: &'static str,
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
    description: "Die Profile von Betula: Merkliste, Stundenplan und Studium getrennt für mehrere Profile, alles nur im eigenen Browser, ohne Konto.",
    server_title: "Deine Profile liegen in deinem Browser",
    server_hint: "Welche Profile es gibt und was sie enthalten, weiß nur dein Browser. Mit JavaScript siehst du sie hier und kannst sie verwalten.",
    profiles: "Profile",
    ok: "OK",
    cancel: "Abbrechen",

    active: "Aktives Profil",
    made_on: |date| format!("Profil seit {date}"),
    rename: "Umbenennen",
    name: "Name des Profils",
    save: "Speichern",
    no_program: "Noch kein Studiengang",
    fs: |fs| format!("{fs}. Fachsemester"),
    town_both: "Cottbus und Senftenberg",
    stat_marked: "gemerkt",
    stat_planned: |semesters| match semesters {
        0 => "geplant".to_string(),
        1 => "geplant, in 1 Semester".to_string(),
        n => format!("geplant, in {n} Semestern"),
    },
    stat_passed: |modules| match modules {
        0 => "bestanden".to_string(),
        1 => "bestanden, 1 Modul".to_string(),
        _ => format!("bestanden, {modules} Module"),
    },
    stat_kept: "Sammlungen · Notizen",
    stat_plans: "gespeicherte Pläne",
    stats_empty: "Noch leer: Was du merkst, planst und bestehst, zählt hier.",

    created: |name| format!("„{name}“ angelegt: Merkliste, Stundenplan und Studium sind noch leer."),
    opened: |name| format!("„{name}“ aus der Datei angelegt."),
    deleted: |name| format!("„{name}“ gelöscht."),
    cleared: "Profil geleert.",

    data_heading: "Daten",
    save_file: "Als Datei sichern",
    save_file_hint: "Für ein Archiv, oder ohne Sync auf ein anderes Gerät",
    save_button: "Speichern",

    danger_heading: "Gefahrenzone",
    danger_hint: "Direkt danach lässt sich beides rückgängig machen, später nicht mehr.",
    clear: "Profil leeren",
    clear_hint: |name| format!("Merkliste, Pläne und Studium gehen, „{name}“ bleibt"),
    clear_button: "Leeren",
    clear_question: "Merkliste, Stundenplan, „Mein Studium“, gespeicherte Pläne und „Mein Studiengang“ dieses Profils löschen?",
    clear_yes: "Leeren",
    nothing_to_clear: "Das Profil ist leer.",
    delete: "Profil löschen",
    delete_hint: |name| format!("„{name}“ verschwindet aus diesem Browser"),
    delete_only: "Das einzige Profil wird stattdessen geleert",
    delete_button: "Löschen",
    delete_question: |name| format!("„{name}“ mit allem, was es enthält, löschen?"),
    delete_yes: "Löschen",

    add_profile: "Profil hinzufügen",
    add_empty: "Leer anlegen",
    add_other: "Von einem anderen Gerät",
    new_name: "Name des neuen Profils",
    create: "Anlegen",
    full: |max| format!("Mehr als {max} Profile gehen nicht: Lösche erst eines."),
    open_file: "Aus Datei laden",
    file_named: |name| format!("„{name}“:"),
    file_empty: "Ein leeres Profil.",
    n_marked: |n| if n == 1 { "1 gemerktes Modul".to_string() } else { format!("{n} gemerkte Module") },
    n_planned: |n| format!("{n} {} geplant", module_de(n)),
    n_plans: |n| if n == 1 { "1 gespeicherter Plan".to_string() } else { format!("{n} gespeicherte Pläne") },
    n_collections: |n| if n == 1 { "1 Sammlung".to_string() } else { format!("{n} Sammlungen") },
    n_notes: |n| if n == 1 { "1 Notiz".to_string() } else { format!("{n} Notizen") },
    passed: |n| format!("{n} {} bestanden", module_de(n)),
    file_question: "Als neues Profil anlegen?",
    file_take: "Anlegen",
    file_unreadable: "Diese Datei ist keine Sicherung eines Profils von Betula.",
    file_newer: "Diese Sicherung stammt aus einer neueren Version von Betula. Lade die Seite neu und versuche es noch einmal.",
    file_too_large: "Diese Datei ist zu groß für eine Sicherung.",
    file_full: |max| format!("Es gibt schon {max} Profile. Lösche eines, dann lässt sich die Datei laden."),
    storage_hint: "Profile liegen nur in diesem Browser: kein Konto, nichts auf dem Server. Ein anderes Gerät hat seine eigenen; eine Sicherung als Datei bringt ein Profil hinüber.",
    storage_hint_sync: "Profile liegen in diesem Browser, ohne Konto. Auf den Server kommt ein Profil nur mit Betula Sync, verschlüsselt mit einem Schlüssel, den nur deine Geräte haben.",
};

pub const EN: Texts = Texts {
    title: "Profile",
    description: "Betula's profiles: saved modules, timetable and studies kept apart for several profiles, all only in your own browser, without an account.",
    server_title: "Your profiles are kept in your browser",
    server_hint: "Only your browser knows which profiles there are and what they hold. With JavaScript you see them here and can manage them.",
    profiles: "Profiles",
    ok: "OK",
    cancel: "Cancel",

    active: "Active profile",
    made_on: |date| format!("Profile since {date}"),
    rename: "Rename",
    name: "Name of the profile",
    save: "Save",
    no_program: "No degree programme yet",
    fs: |fs| format!("Semester {fs}"),
    town_both: "Cottbus and Senftenberg",
    stat_marked: "saved",
    stat_planned: |semesters| match semesters {
        0 => "planned".to_string(),
        1 => "planned, in 1 semester".to_string(),
        n => format!("planned, in {n} semesters"),
    },
    stat_passed: |modules| match modules {
        0 => "passed".to_string(),
        1 => "passed, 1 module".to_string(),
        _ => format!("passed, {modules} modules"),
    },
    stat_kept: "collections · notes",
    stat_plans: "saved plans",
    stats_empty: "Empty so far: what you save, plan and pass counts here.",

    created: |name| format!("“{name}” created: its saved modules, timetable and studies are empty for now."),
    opened: |name| format!("“{name}” created from the file."),
    deleted: |name| format!("“{name}” deleted."),
    cleared: "Profile emptied.",

    data_heading: "Data",
    save_file: "Save as a file",
    save_file_hint: "For an archive, or to another device without sync",
    save_button: "Save",

    danger_heading: "Danger zone",
    danger_hint: "Both can be undone right after, not later.",
    clear: "Empty profile",
    clear_hint: |name| format!("Saved modules, plans and studies go, “{name}” stays"),
    clear_button: "Empty",
    clear_question: "Delete the saved modules, timetable, “My studies”, saved plans and “My programme” of this profile?",
    clear_yes: "Empty",
    nothing_to_clear: "The profile is empty.",
    delete: "Delete profile",
    delete_hint: |name| format!("“{name}” leaves this browser"),
    delete_only: "The only profile is emptied instead",
    delete_button: "Delete",
    delete_question: |name| format!("Delete “{name}” with everything it holds?"),
    delete_yes: "Delete",

    add_profile: "Add profile",
    add_empty: "Create an empty one",
    add_other: "From another device",
    new_name: "Name of the new profile",
    create: "Create",
    full: |max| format!("There can be no more than {max} profiles: delete one first."),
    open_file: "Load from a file",
    file_named: |name| format!("“{name}”:"),
    file_empty: "An empty profile.",
    n_marked: |n| if n == 1 { "1 saved module".to_string() } else { format!("{n} saved modules") },
    n_planned: |n| format!("{n} {} planned", module_en(n)),
    n_plans: |n| if n == 1 { "1 saved plan".to_string() } else { format!("{n} saved plans") },
    n_collections: |n| if n == 1 { "1 collection".to_string() } else { format!("{n} collections") },
    n_notes: |n| if n == 1 { "1 note".to_string() } else { format!("{n} notes") },
    passed: |n| format!("{n} {} passed", module_en(n)),
    file_question: "Create it as a new profile?",
    file_take: "Create",
    file_unreadable: "This file is not a backup of a Betula profile.",
    file_newer: "This backup comes from a newer version of Betula. Reload the page and try again.",
    file_too_large: "This file is too large to be a backup.",
    file_full: |max| format!("There are {max} profiles already. Delete one, then the file can be loaded."),
    storage_hint: "Profiles are kept only in this browser: no account, nothing on the server. Another device has its own; a backup file takes a profile there.",
    storage_hint_sync: "Profiles are kept in this browser, without an account. A profile reaches the server only with Betula Sync, encrypted with a key only your devices have.",
};
