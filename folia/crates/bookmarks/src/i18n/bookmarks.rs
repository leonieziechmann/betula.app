//! Texts of the Merkliste (`pages/bookmarks.rs`). Its list is one like the catalog's: the heads
//! of its columns, its keys and its tags are the catalog's (`i18n::catalog`).

pub struct Texts {
    /// The page's title, heading and name: „Merkliste".
    pub title: &'static str,
    /// What search engines and link previews read of the page (the same for everybody: what is
    /// marked lives in the browser).
    pub description: &'static str,

    // ---- a list brought over from another device (`#m=…`) ----
    /// A link whose list cannot be read: its heading and why.
    pub broken_link: &'static str,
    pub broken_link_hint: &'static str,
    pub ok: &'static str,
    /// „3 Module aus einem Link.": the number of modules the link brings.
    pub from_link: fn(usize) -> String,
    /// All of them marked already, none of them, or some („2 davon fehlen auf deiner Merkliste.
    /// Hinzufügen?"; the value is how many are missing).
    pub all_there: &'static str,
    pub add_question: &'static str,
    pub some_missing: fn(usize) -> String,
    /// More than the Merkliste has room for: how many of them fit, or that none does (the value is
    /// the limit).
    pub room_for: fn(usize) -> String,
    pub none_fit: fn(usize) -> String,
    pub add: &'static str,
    pub discard: &'static str,

    // ---- the list ----
    /// What the number above the list counts: „gemerkte Module", „gemerkte Module im Winter";
    /// the values are the number and the half of the year the list shows, if one.
    pub count_label: fn(u64, Option<&str>) -> String,
    /// While the search at the head searches the list: „von 9 gemerkten" after the number of what
    /// it found; the value is how many the list shows without the search.
    pub count_found: fn(u64) -> String,
    /// The head over the bookmarks, the order by the time of marking.
    pub col_marked: &'static str,
    /// The sections of „Zuletzt gemerkt": the days and weeks, then the months, a month of another
    /// year with its year („September 2025").
    pub today: &'static str,
    pub yesterday: &'static str,
    pub this_week: &'static str,
    pub last_week: &'static str,
    pub months: [&'static str; 12],
    pub month_year: fn(&str, i32) -> String,
    /// What was marked before the Merkliste kept the time of a mark.
    pub earlier: &'static str,
    /// The sections of „Nach Studium": a kind with its area („Wahlpflicht · Praktische
    /// Mathematik"; the values are the kind's word and the area's name), the program's modules no
    /// source gives a kind, and what lies outside the program.
    pub kind_in: fn(&str, &str) -> String,
    pub in_program: &'static str,
    pub not_in_program: &'static str,
    /// The sections of „Nach Angebot".
    pub offer_winter: &'static str,
    pub offer_summer: &'static str,
    pub offer_every: &'static str,
    pub offer_irregular: &'static str,
    pub offer_unstated: &'static str,
    /// What the mark of „Mein Studium" at a row stands for.
    pub status_planned: &'static str,
    pub status_running: &'static str,
    pub status_passed: &'static str,
    pub status_failed: &'static str,
    /// The search found nothing marked: „Nichts Gemerktes zu ‚Lena‘" and what it searches.
    pub nothing_found: fn(&str) -> String,
    pub nothing_found_hint: &'static str,
    /// The way from the search into the whole catalog, under what it found: „‚Philosophie‘ im ganzen
    /// Katalog", then „11 Module" (the number written already), or „suchen" for none known yet.
    pub in_catalog: fn(&str) -> String,
    pub catalog_modules: fn(u64, &str) -> String,
    pub catalog_search: &'static str,
    /// The filters leave nothing of the list: its title, its hint and the way back to all.
    pub nothing_filtered: &'static str,
    pub nothing_filtered_hint: &'static str,
    /// The server's page, which cannot know what is marked.
    pub server_title: &'static str,
    pub server_hint: &'static str,
    /// Nothing marked yet. The hint names the key M between its two parts.
    pub empty_title: &'static str,
    pub empty_hint_start: &'static str,
    pub empty_hint_end: &'static str,
    /// Nothing marked is offered in the half of the year chosen („Winter"); its title and hint.
    pub nothing_in_season: fn(&str) -> String,
    pub nothing_in_season_hint: fn(&str) -> String,
    pub show_all_saved: &'static str,
    /// The heading above the marked modules the snapshot does not know, and what such a row
    /// says under „Modul 12345".
    pub not_in_catalog: &'static str,
    pub module_numbered: fn(&str) -> String,
    pub not_in_catalog_row: &'static str,

    // ---- collections: the views of the list (folia/design/merken/oberflaeche.html, „Die Seite") ----
    /// „Sammlungen": the sidebar's first group, on a phone the chips over the list.
    pub collections: &'static str,
    /// „Neue Sammlung", what its field says while empty and what it is for screen readers.
    pub new_collection: &'static str,
    pub collection_name: &'static str,
    pub new_collection_name: &'static str,
    /// A name typed that a collection has: making it opens that one; renaming to it is refused.
    pub taken_open: &'static str,
    pub taken: &'static str,
    /// The menus: of a collection („Menü von ‚Für Lena‘"), of the whole list, the ⋯ that opens one.
    pub collection_menu: fn(&str) -> String,
    pub list_menu: &'static str,
    pub more: &'static str,
    pub rename: &'static str,
    pub move_up: &'static str,
    pub move_down: &'static str,
    pub delete: &'static str,
    /// What the head of the list says after the number in a view: „in Für Lena", „ohne Sammlung".
    pub in_collection: fn(&str) -> String,
    pub without_label: &'static str,
    /// An empty collection, „Ohne Sammlung" with nothing in it, a collection that is not there.
    pub empty_collection: fn(&str) -> String,
    pub empty_collection_hint: &'static str,
    pub nothing_without: &'static str,
    pub nothing_without_hint: &'static str,
    pub no_such_collection: &'static str,
    pub no_such_collection_hint: &'static str,

    // ---- the sidebar: on a phone the sheet behind „Filter" ----
    /// „Angeboten im": the choice between every half of the year, the winter and the summer.
    pub offered_in: &'static str,
    pub all: &'static str,
    /// „Stand": what „Mein Studium" says of a module (`url::Stand` names the choices).
    pub stand: &'static str,
    /// „Mein Studium": what counts in the visitor's study and what fits their Stundenplan.
    pub my_study: &'static str,
    pub counts: &'static str,
    /// „Ordnung": the orders, on a phone in the sheet (on a wide screen the heads of the columns).
    pub order: &'static str,

    // ---- the menu of the whole list (oberflaeche.html: „„Alle“ hat ein Menü wie eine Sammlung") ----
    /// The list as text, for a note or a message.
    pub copy_text: &'static str,
    /// The link for another device: its tooltip, its label and the line under it.
    pub transfer_title: &'static str,
    pub transfer: &'static str,
    pub transfer_hint: &'static str,
    /// Emptying the list: the question and its answers, the entry.
    pub clear_question: fn(usize) -> String,
    pub clear_yes: &'static str,
    pub cancel: &'static str,
    pub clear: &'static str,

    // ---- several at once (oberflaeche.html, „Mehrere auswählen"), as in „Mein Studium" ----
    /// The bar of the rows selected: its name for screen readers, „3 ausgewählt", „· ausgewählt"
    /// after a row's title for screen readers, „Alle auswählen", letting go of them.
    pub selection: &'static str,
    pub n_selected: fn(usize) -> String,
    pub is_selected: &'static str,
    pub select_all: &'static str,
    pub clear_selection: &'static str,
    /// Its actions „Sammlung" and „Teilen" (its menu's head „3 Module teilen"); the head of
    /// „Einplanen"'s menu („3 Module einplanen").
    pub act_collect: &'static str,
    pub act_share: &'static str,
    pub share_n: fn(usize) -> String,
    pub plan_n: fn(usize) -> String,
    /// „Einplanen"'s menu: each where its own „Einplanen" aims …
    pub each_aimed: &'static str,
    /// … or all into one semester, under „Alle in": whether the modules are offered there
    /// („angeboten", „nicht angeboten", „2 nicht angeboten"), „jetzt", and the semesters behind the
    /// student, folded.
    pub all_into: &'static str,
    pub offered: &'static str,
    pub not_offered: &'static str,
    pub n_not_offered: fn(usize) -> String,
    pub now: &'static str,
    pub earlier_semesters: &'static str,
    /// A row's menu (its right click): „Auswählen" or „Abwählen", „Aus dem Plan nehmen" where
    /// „Einplanen" would take the module out, „Von der Merkliste entfernen".
    pub select: &'static str,
    pub deselect: &'static str,
    /// What selects a row without its menu, beside „Auswählen" (R10).
    pub select_key: &'static str,
    pub unplan: &'static str,
    pub unmark: &'static str,
}

pub const DE: Texts = Texts {
    title: "Merkliste",
    description: "Module der BTU Cottbus-Senftenberg merken und wiederfinden. Die Merkliste liegt nur im eigenen Browser: kein Konto, keine Daten auf dem Server.",

    broken_link: "Der Link ist beschädigt.",
    broken_link_hint: "Die Merkliste darin lässt sich nicht lesen: Vielleicht fehlt beim Kopieren ein Stück, oder ein Zeichen ist falsch abgetippt.",
    ok: "In Ordnung",
    from_link: |n| if n == 1 { "1 Modul aus einem Link.".to_string() } else { format!("{n} Module aus einem Link.") },
    all_there: "Alles davon steht schon auf deiner Merkliste.",
    add_question: "Auf deine Merkliste setzen?",
    some_missing: |n| format!("{} davon fehlen auf deiner Merkliste. Hinzufügen?", if n == 1 { "Eins".to_string() } else { n.to_string() }),
    room_for: |n| if n == 1 { "Eins davon passt noch auf deine Merkliste. Hinzufügen?".to_string() } else { format!("{n} davon passen noch auf deine Merkliste. Hinzufügen?") },
    none_fit: |max| format!("Deine Merkliste ist voll: {max} Module."),
    add: "Hinzufügen",
    discard: "Verwerfen",

    count_label: |n, season| match (n, season) {
        (1, None) => "gemerktes Modul".to_string(),
        (_, None) => "gemerkte Module".to_string(),
        (1, Some(season)) => format!("gemerktes Modul im {season}"),
        (_, Some(season)) => format!("gemerkte Module im {season}"),
    },
    count_found: |n| format!("von {n} gemerkten"),
    col_marked: "Gemerkt",
    today: "Heute",
    yesterday: "Gestern",
    this_week: "Diese Woche",
    last_week: "Letzte Woche",
    months: ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"],
    month_year: |month, year| format!("{month} {year}"),
    earlier: "Früher",
    kind_in: |kind, area| format!("{kind} · {area}"),
    in_program: "Weitere im Studiengang",
    not_in_program: "Gehört nicht zu deinem Studiengang",
    offer_winter: "Winter",
    offer_summer: "Sommer",
    offer_every: "Jedes Semester",
    offer_irregular: "Unregelmäßig",
    offer_unstated: "Ohne Angabe",
    status_planned: "Geplant",
    status_running: "Läuft",
    status_passed: "Bestanden",
    status_failed: "Nicht bestanden",
    nothing_found: |text| format!("Nichts Gemerktes zu ‚{text}‘"),
    nothing_found_hint: "Die Suche findet Titel in beiden Sprachen, Nummern, Kürzel und Lehrende.",
    in_catalog: |text| format!("‚{text}‘ im ganzen Katalog"),
    catalog_modules: |n, written| if n == 1 { format!("{written} Modul") } else { format!("{written} Module") },
    catalog_search: "suchen",
    nothing_filtered: "Nichts Gemerktes passt zu diesen Filtern",
    nothing_filtered_hint: "Nimm einen Filter zurück oder sieh dir alle gemerkten an.",
    server_title: "Deine Merkliste",
    server_hint: "Sie liegt in deinem Browser, nicht auf dem Server, und erscheint, sobald die App geladen ist. Dafür braucht es JavaScript.",
    empty_title: "Noch nichts gemerkt",
    empty_hint_start: "Das Lesezeichen an einem Modul setzt es auf diese Liste, die Taste ",
    empty_hint_end: " ebenso. Sie bleibt in diesem Browser gespeichert.",
    nothing_in_season: |season| format!("Nichts davon im {season}"),
    nothing_in_season_hint: |season| format!("Keines der gemerkten Module wird laut Modulbeschreibung im {season} angeboten."),
    show_all_saved: "Alle gemerkten zeigen",
    not_in_catalog: "Nicht im Modulkatalog",
    module_numbered: |id| format!("Modul {id}"),
    not_in_catalog_row: "steht nicht (mehr) im Modulkatalog der BTU",

    collections: "Sammlungen",
    new_collection: "Neue Sammlung",
    collection_name: "Name der Sammlung",
    new_collection_name: "Name der neuen Sammlung",
    taken_open: "Gibt es schon · Enter öffnet sie",
    taken: "Gibt es schon",
    collection_menu: |name| format!("Menü von ‚{name}‘"),
    list_menu: "Menü der Merkliste",
    more: "Mehr",
    rename: "Umbenennen",
    move_up: "Nach oben",
    move_down: "Nach unten",
    delete: "Löschen",
    in_collection: |name| format!("in {name}"),
    without_label: "ohne Sammlung",
    empty_collection: |name| format!("‚{name}‘ ist leer"),
    empty_collection_hint: "Module kommen über „Sammlung“ im Hinweis nach dem Merken hierher, oder am Modul über den Pfeil neben „Gemerkt“.",
    nothing_without: "Alles steht in Sammlungen",
    nothing_without_hint: "Jedes gemerkte Modul steht in mindestens einer Sammlung.",
    no_such_collection: "Diese Sammlung gibt es nicht",
    no_such_collection_hint: "Vielleicht ist sie gelöscht, oder der Link kommt aus einem anderen Browser: Sammlungen liegen nur in dem Browser, in dem sie angelegt wurden.",

    offered_in: "Angeboten im",
    all: "Alle",
    stand: "Stand",
    my_study: "Mein Studium",
    counts: "Zählt in meinem Studium",
    order: "Ordnung",

    copy_text: "Als Text kopieren",
    transfer_title: "Der Link trägt die Merkliste hinter dem #: dieser Teil einer Adresse wird nie an einen Server gesendet",
    transfer: "Link für ein anderes Gerät",
    transfer_hint: "Kopieren und dort öffnen",
    clear_question: |n| if n == 1 { "Den Merker entfernen?".to_string() } else { format!("Alle {n} Merker entfernen?") },
    clear_yes: "Leeren",
    cancel: "Abbrechen",
    clear: "Merkliste leeren",

    selection: "Auswahl",
    n_selected: |n| format!("{n} ausgewählt"),
    is_selected: "ausgewählt",
    select_all: "Alle auswählen",
    clear_selection: "Auswahl aufheben",
    act_collect: "Sammlung",
    act_share: "Teilen",
    share_n: |n| if n == 1 { "1 Modul teilen".to_string() } else { format!("{n} Module teilen") },
    plan_n: |n| if n == 1 { "1 Modul einplanen".to_string() } else { format!("{n} Module einplanen") },
    each_aimed: "Jedes in sein Semester",
    all_into: "Alle in",
    offered: "angeboten",
    not_offered: "nicht angeboten",
    n_not_offered: |n| format!("{n} nicht angeboten"),
    now: "jetzt",
    earlier_semesters: "Frühere Semester",
    select: "Auswählen",
    deselect: "Abwählen",
    select_key: "Strg Klick",
    unplan: "Aus dem Plan nehmen",
    unmark: "Von der Merkliste entfernen",
};

pub const EN: Texts = Texts {
    title: "Saved modules",
    description: "Save modules of BTU Cottbus-Senftenberg and find them again. Your saved modules stay in your own browser: no account, no data on the server.",

    broken_link: "The link is damaged.",
    broken_link_hint: "The saved modules in it cannot be read: perhaps a piece went missing when it was copied, or a character was mistyped.",
    ok: "OK",
    from_link: |n| if n == 1 { "1 module from a link.".to_string() } else { format!("{n} modules from a link.") },
    all_there: "All of them are among your saved modules already.",
    add_question: "Add them to your saved modules?",
    some_missing: |n| if n == 1 { "One of them is missing from your saved modules. Add it?".to_string() } else { format!("{n} of them are missing from your saved modules. Add them?") },
    room_for: |n| if n == 1 { "One of them still fits among your saved modules. Add it?".to_string() } else { format!("{n} of them still fit among your saved modules. Add them?") },
    none_fit: |max| format!("Your saved modules are full: {max} modules."),
    add: "Add",
    discard: "Discard",

    count_label: |n, season| match (n, season) {
        (1, None) => "saved module".to_string(),
        (_, None) => "saved modules".to_string(),
        (1, Some(season)) => format!("saved module offered in {season}"),
        (_, Some(season)) => format!("saved modules offered in {season}"),
    },
    count_found: |n| format!("of {n} saved"),
    col_marked: "Saved",
    today: "Today",
    yesterday: "Yesterday",
    this_week: "This week",
    last_week: "Last week",
    months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
    month_year: |month, year| format!("{month} {year}"),
    earlier: "Earlier",
    kind_in: |kind, area| format!("{kind} · {area}"),
    in_program: "More in your programme",
    not_in_program: "Not part of your programme",
    offer_winter: "Winter",
    offer_summer: "Summer",
    offer_every: "Every semester",
    offer_irregular: "Irregularly",
    offer_unstated: "Not stated",
    status_planned: "Planned",
    status_running: "Under way",
    status_passed: "Passed",
    status_failed: "Not passed",
    nothing_found: |text| format!("Nothing saved matches ‘{text}’"),
    nothing_found_hint: "The search finds titles in both languages, numbers, abbreviations and lecturers.",
    in_catalog: |text| format!("‘{text}’ in the whole catalogue"),
    catalog_modules: |n, written| if n == 1 { format!("{written} module") } else { format!("{written} modules") },
    catalog_search: "search",
    nothing_filtered: "None of your saved modules matches these filters",
    nothing_filtered_hint: "Remove a filter, or look at all your saved modules.",
    server_title: "Your saved modules",
    server_hint: "They are kept in your browser, not on the server, and appear once the app has loaded. This needs JavaScript.",
    empty_title: "Nothing saved yet",
    empty_hint_start: "The bookmark on a module adds it to this list, and so does the key ",
    empty_hint_end: ". The list stays saved in this browser.",
    nothing_in_season: |season| format!("None of them in {season}"),
    nothing_in_season_hint: |season| format!("According to their module descriptions, none of the saved modules is offered in {season}."),
    show_all_saved: "Show all saved modules",
    not_in_catalog: "Not in the module catalogue",
    module_numbered: |id| format!("Module {id}"),
    not_in_catalog_row: "is not (or no longer) in BTU's module catalogue",

    collections: "Collections",
    new_collection: "New collection",
    collection_name: "Name of the collection",
    new_collection_name: "Name of the new collection",
    taken_open: "Exists already · Enter opens it",
    taken: "Exists already",
    collection_menu: |name| format!("Menu of ‘{name}’"),
    list_menu: "Menu of your saved modules",
    more: "More",
    rename: "Rename",
    move_up: "Move up",
    move_down: "Move down",
    delete: "Delete",
    in_collection: |name| format!("in {name}"),
    without_label: "in no collection",
    empty_collection: |name| format!("‘{name}’ is empty"),
    empty_collection_hint: "Modules come here by “Collection” in the note after saving, or at a module by the arrow beside “Saved”.",
    nothing_without: "Everything is in collections",
    nothing_without_hint: "Every saved module is in at least one collection.",
    no_such_collection: "This collection does not exist",
    no_such_collection_hint: "Perhaps it was deleted, or the link comes from another browser: collections live only in the browser they were made in.",

    offered_in: "Offered in",
    all: "All",
    stand: "Status",
    my_study: "My studies",
    counts: "Counts in my studies",
    order: "Order",

    copy_text: "Copy as text",
    transfer_title: "The link carries your saved modules after the #: this part of an address is never sent to a server",
    transfer: "Link for another device",
    transfer_hint: "Copy it and open it there",
    clear_question: |n| if n == 1 { "Remove the saved module?".to_string() } else { format!("Remove all {n} saved modules?") },
    clear_yes: "Clear",
    cancel: "Cancel",
    clear: "Clear saved modules",

    selection: "Selection",
    n_selected: |n| format!("{n} selected"),
    is_selected: "selected",
    select_all: "Select all",
    clear_selection: "Clear selection",
    act_collect: "Collection",
    act_share: "Share",
    share_n: |n| if n == 1 { "Share 1 module".to_string() } else { format!("Share {n} modules") },
    plan_n: |n| if n == 1 { "Plan 1 module".to_string() } else { format!("Plan {n} modules") },
    each_aimed: "Each into its semester",
    all_into: "All into",
    offered: "offered",
    not_offered: "not offered",
    n_not_offered: |n| format!("{n} not offered"),
    now: "now",
    earlier_semesters: "Earlier semesters",
    select: "Select",
    deselect: "Deselect",
    select_key: "Ctrl click",
    unplan: "Take out of the plan",
    unmark: "Remove from saved modules",
};
