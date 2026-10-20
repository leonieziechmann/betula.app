//! Texts of „Merken" wherever a module can be marked (`bookmarks.rs`).

pub struct Texts {
    /// The switch's label: „Merken" while the module is not marked, „Gemerkt" once it is.
    pub save: &'static str,
    pub saved: &'static str,
    /// What the switch does, as its tooltip, with its shortcut: not marked yet …
    pub save_hint: &'static str,
    /// … and marked.
    pub saved_hint: &'static str,
    /// The switch as a command of „Befehle" while the module is marked: „Nicht mehr merken".
    pub unsave: &'static str,
    /// The icon alone at the end of a row, for screen readers: „Analysis I merken"; the value is
    /// the module's title.
    pub save_title: fn(&str) -> String,
    /// What a row swiped to the left uncovers on a marked module (`swipe.rs`): „Entfernen" …
    pub remove: &'static str,
    /// … with the line under it, where from …
    pub from_list: &'static str,
    /// … and „Entfernt" once it is done (a module marked by the swipe says `saved`).
    pub removed: &'static str,

    // ---- collections and notes (folia/design/merken/oberflaeche.html) ----
    /// The switch of a marked module, at its arrow: the menu of its collections and its note.
    pub collect: &'static str,
    pub collect_hint: &'static str,
    /// The arrow beside a row's bookmark, for screen readers: „Sammlung und Notiz zu Analysis I".
    pub collect_title: fn(&str) -> String,
    /// Under „Merken" where a default for new marks is set: where the next mark goes („in Für
    /// Lena"; the value is the collections' names).
    pub mark_into: fn(&str) -> String,
    /// A module in no collection; the field of the note after marking says so, and a module can be
    /// put into none.
    pub without_collection: &'static str,
    /// The field where no collection exists yet: it makes the first.
    pub collection: &'static str,
    /// What the field is, for screen readers: „Sammlung: Für Lena", and where the default put it.
    pub field_label: fn(&str) -> String,
    pub field_by_default: fn(&str) -> String,
    /// The note's „Entfernt" after the module's name on a wide screen.
    pub removed_after: &'static str,
    /// The Merkliste holds `MAX_MARKS` modules: nothing more is marked („Merkliste voll · 256
    /// Module"), and the way to it.
    pub full: &'static str,
    pub full_count: fn(usize) -> String,
    pub to_list: &'static str,
    /// A collection deleted: „‚Für Lena‘ gelöscht · 3 bleiben gemerkt", and the modules that were in
    /// it alone taken away too („Auch 2 entfernen").
    pub deleted: fn(&str) -> String,
    pub stay_marked: fn(usize) -> String,
    pub also_remove: fn(usize) -> String,
    /// „2 entfernt": how many were taken away with it.
    pub n_removed: fn(usize) -> String,
    /// The Merkliste emptied.
    pub cleared: &'static str,
    /// Copied: a link for another device, the list as text.
    pub copied_link: &'static str,
    pub copied_text: &'static str,
    pub copied_what: fn(usize) -> String,
    /// The rows selected on the Merkliste taken away: „3 Module entfernt".
    pub removed_many: fn(usize) -> String,
    /// The rows selected planned (folia/design/merken/konzept.html, „Ein Hinweis sagt, wohin"):
    /// „3 eingeplant", where („in WiSe 26/27", or „2 in WiSe 26/27" each), what was planned there
    /// already („1 war schon geplant"), and that all of them were.
    pub n_planned: fn(usize) -> String,
    pub planned_in: fn(&str) -> String,
    pub n_in: fn(usize, &str) -> String,
    pub planned_before: fn(usize) -> String,
    pub all_planned: &'static str,

    // ---- the menu of a module's collections and note ----
    /// The menu's name for screen readers, on a phone its sheet's head is the module.
    pub menu_title: &'static str,
    /// Its head for several modules: „3 Module".
    pub n_modules: fn(usize) -> String,
    /// The field at its top, which has the focus: it filters the collections and makes one.
    pub search_or_create: &'static str,
    /// „‚mathe‘ anlegen": a collection of the name typed, where none has it.
    pub create: fn(&str) -> String,
    /// No collection yet, nothing typed.
    pub no_collection_yet: &'static str,
    /// A collection of the name typed is there already; and at its limit.
    pub collections_full: &'static str,
    /// „Standard für neue Merker", what the switch replaces („bisher Für Lena"), and „Aufheben" for
    /// a default the module is not in.
    pub standard: &'static str,
    pub standard_before: fn(&str) -> String,
    pub standard_clear: &'static str,
    /// The note: its label, what an empty one says, what it is for screen readers, its tooltip at the
    /// module.
    pub note: &'static str,
    pub note_placeholder: &'static str,
    pub note_label: &'static str,
    pub edit_note: &'static str,
    /// The foot of the phone's sheet: it keeps what was chosen and written.
    pub done: &'static str,
}

pub const DE: Texts = Texts {
    save: "Merken",
    saved: "Gemerkt",
    unsave: "Nicht mehr merken",
    save_hint: "Auf die Merkliste setzen (M)",
    saved_hint: "Gemerkt. Noch einmal nimmt das Modul von der Merkliste (M)",
    save_title: |title| format!("{title} merken"),
    remove: "Entfernen",
    from_list: "von der Merkliste",
    removed: "Entfernt",

    collect: "Sammlung und Notiz",
    collect_hint: "Sammlung und Notiz (⇧M)",
    collect_title: |title| format!("Sammlung und Notiz zu {title}"),
    mark_into: |names| format!("in {names}"),
    without_collection: "Ohne Sammlung",
    collection: "Sammlung",
    field_label: |names| format!("Sammlung: {names}"),
    field_by_default: |names| format!("Sammlung: {names}, über den Standard für neue Merker"),
    removed_after: "entfernt",
    full: "Merkliste voll",
    full_count: |n| format!("{n} Module"),
    to_list: "Zur Merkliste",
    deleted: |name| format!("‚{name}‘ gelöscht"),
    stay_marked: |n| if n == 1 { "1 bleibt gemerkt".to_string() } else { format!("{n} bleiben gemerkt") },
    also_remove: |n| format!("Auch {n} entfernen"),
    n_removed: |n| format!("{n} entfernt"),
    cleared: "Merkliste geleert",
    copied_link: "Link kopiert",
    copied_text: "Liste kopiert",
    copied_what: |n| if n == 1 { "1 Modul".to_string() } else { format!("{n} Module") },
    removed_many: |n| if n == 1 { "1 Modul entfernt".to_string() } else { format!("{n} Module entfernt") },
    n_planned: |n| format!("{n} eingeplant"),
    planned_in: |semester| format!("in {semester}"),
    n_in: |n, semester| format!("{n} in {semester}"),
    planned_before: |n| if n == 1 { "1 war schon geplant".to_string() } else { format!("{n} waren schon geplant") },
    all_planned: "Schon alle geplant",

    menu_title: "Sammlung und Notiz",
    n_modules: |n| if n == 1 { "1 Modul".to_string() } else { format!("{n} Module") },
    search_or_create: "Sammlung suchen oder anlegen",
    create: |name| format!("‚{name}‘ anlegen"),
    no_collection_yet: "Noch keine Sammlung. Tippe einen Namen.",
    collections_full: "Mehr Sammlungen gehen nicht",
    standard: "Standard für neue Merker",
    standard_before: |name| format!("bisher {name}"),
    standard_clear: "Aufheben",
    note: "Notiz",
    note_placeholder: "Notiz, gern mit Markdown",
    note_label: "Notiz (Markdown)",
    edit_note: "Notiz bearbeiten",
    done: "Fertig",
};

pub const EN: Texts = Texts {
    save: "Save",
    saved: "Saved",
    unsave: "Unsave",
    save_hint: "Add to your saved modules (M)",
    saved_hint: "Saved. Once more removes the module from your saved modules (M)",
    save_title: |title| format!("Save {title}"),
    remove: "Remove",
    from_list: "from saved modules",
    removed: "Removed",

    collect: "Collection and note",
    collect_hint: "Collection and note (⇧M)",
    collect_title: |title| format!("Collection and note for {title}"),
    mark_into: |names| format!("into {names}"),
    without_collection: "No collection",
    collection: "Collection",
    field_label: |names| format!("Collection: {names}"),
    field_by_default: |names| format!("Collection: {names}, by the default for new saves"),
    removed_after: "removed",
    full: "Saved modules full",
    full_count: |n| format!("{n} modules"),
    to_list: "To saved modules",
    deleted: |name| format!("‘{name}’ deleted"),
    stay_marked: |n| if n == 1 { "1 stays saved".to_string() } else { format!("{n} stay saved") },
    also_remove: |n| format!("Remove {n} as well"),
    n_removed: |n| format!("{n} removed"),
    cleared: "Saved modules cleared",
    copied_link: "Link copied",
    copied_text: "List copied",
    copied_what: |n| if n == 1 { "1 module".to_string() } else { format!("{n} modules") },
    removed_many: |n| if n == 1 { "1 module removed".to_string() } else { format!("{n} modules removed") },
    n_planned: |n| format!("{n} planned"),
    planned_in: |semester| format!("for {semester}"),
    n_in: |n, semester| format!("{n} for {semester}"),
    planned_before: |n| if n == 1 { "1 was planned already".to_string() } else { format!("{n} were planned already") },
    all_planned: "All planned already",

    menu_title: "Collection and note",
    n_modules: |n| if n == 1 { "1 module".to_string() } else { format!("{n} modules") },
    search_or_create: "Find or create a collection",
    create: |name| format!("Create ‘{name}’"),
    no_collection_yet: "No collection yet. Type a name.",
    collections_full: "No more collections",
    standard: "Default for new saves",
    standard_before: |name| format!("before: {name}"),
    standard_clear: "Clear",
    note: "Note",
    note_placeholder: "Note, Markdown welcome",
    note_label: "Note (Markdown)",
    edit_note: "Edit note",
    done: "Done",
};
