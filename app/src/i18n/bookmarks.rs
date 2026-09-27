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
    pub add: &'static str,
    pub discard: &'static str,

    // ---- the list ----
    /// What the number above the list counts: „gemerkte Module", „gemerkte Module im Winter";
    /// the values are the number and the half of the year the list shows, if one.
    pub count_label: fn(u64, Option<&str>) -> String,
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

    // ---- the sidebar ----
    pub overview: &'static str,
    /// „Gemerkt": how many modules are marked.
    pub saved: &'static str,
    pub credit_points: &'static str,
    /// „ · 2 ohne Angabe": how many marked modules state no credits.
    pub unstated: fn(u64) -> String,
    /// „Angeboten im": the choice between every half of the year, the winter and the summer.
    pub offered_in: &'static str,
    pub all: &'static str,
    pub order: &'static str,
    pub actions: &'static str,
    pub copy_list: &'static str,
    /// The link for another device: its tooltip, its label and the line under it.
    pub transfer_title: &'static str,
    pub transfer: &'static str,
    pub transfer_hint: &'static str,
    /// Emptying the list: done (with „Rückgängig" beside it), the question and its answers, the
    /// action.
    pub cleared: &'static str,
    pub clear_question: &'static str,
    pub clear_yes: &'static str,
    pub cancel: &'static str,
    pub clear: &'static str,
    /// Where the list lives.
    pub storage_hint: &'static str,
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
    add: "Hinzufügen",
    discard: "Verwerfen",

    count_label: |n, season| match (n, season) {
        (1, None) => "gemerktes Modul".to_string(),
        (_, None) => "gemerkte Module".to_string(),
        (1, Some(season)) => format!("gemerktes Modul im {season}"),
        (_, Some(season)) => format!("gemerkte Module im {season}"),
    },
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

    overview: "Übersicht",
    saved: "Gemerkt",
    credit_points: "Leistungspunkte",
    unstated: |n| format!("{n} ohne Angabe"),
    offered_in: "Angeboten im",
    all: "Alle",
    order: "Reihenfolge",
    actions: "Aktionen",
    copy_list: "Liste kopieren",
    transfer_title: "Der Link trägt die Merkliste hinter dem #: dieser Teil einer Adresse wird nie an einen Server gesendet",
    transfer: "Auf anderes Gerät übertragen",
    transfer_hint: "Link kopieren und dort öffnen",
    cleared: "Geleert",
    clear_question: "Alle Merker entfernen?",
    clear_yes: "Leeren",
    cancel: "Abbrechen",
    clear: "Merkliste leeren",
    storage_hint: "Die Merkliste liegt nur in diesem Browser: kein Konto, und nichts davon erreicht den Server. Ein anderes Gerät hat seine eigene; der Link zum Übertragen bringt sie dorthin.",
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
    add: "Add",
    discard: "Discard",

    count_label: |n, season| match (n, season) {
        (1, None) => "saved module".to_string(),
        (_, None) => "saved modules".to_string(),
        (1, Some(season)) => format!("saved module offered in {season}"),
        (_, Some(season)) => format!("saved modules offered in {season}"),
    },
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

    overview: "Overview",
    saved: "Saved",
    credit_points: "Credit points",
    unstated: |n| format!("{n} not stated"),
    offered_in: "Offered in",
    all: "All",
    order: "Order",
    actions: "Actions",
    copy_list: "Copy list",
    transfer_title: "The link carries your saved modules after the #: this part of an address is never sent to a server",
    transfer: "Move to another device",
    transfer_hint: "Copy the link and open it there",
    cleared: "Cleared",
    clear_question: "Remove all saved modules?",
    clear_yes: "Clear",
    cancel: "Cancel",
    clear: "Clear saved modules",
    storage_hint: "Your saved modules live in this browser only: no account, and none of it reaches the server. Another device has its own list; the transfer link brings yours there.",
};
