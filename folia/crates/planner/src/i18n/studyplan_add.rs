//! Texts of „+ Modul" (`studyplan/add.rs`): the choice of modules for one calendar, „Merkliste" and
//! „Katalog", each module with how it sits in the calendar's week, „Aufnehmen", and the note after
//! it.

pub struct Texts {
    /// The heading: „Modul in „Mein Studiengang“" (the calendar's name).
    pub title: fn(&str) -> String,
    /// What a module taken in there is: in „Mein Studiengang" a visit („Zählt in „Mein Studium“ als
    /// Besuch, ohne LP."), in a calendar of one's own nothing of the study („Module in „Lena“ zählen
    /// nirgends.").
    pub mine_says: &'static str,
    pub calendar_says: fn(&str) -> String,
    /// The × of the panel.
    pub close: &'static str,
    /// The two ways to a module: the Merkliste (with how many it offers) and the catalog.
    pub ways: &'static str,
    pub marked: fn(usize) -> String,
    pub catalog: &'static str,
    /// The catalog's search field and what stands under it while it is empty.
    pub search: &'static str,
    pub search_hint: &'static str,
    /// Nothing to offer: the Merkliste holds nothing more for the calendar, the search finds
    /// nothing.
    pub nothing_marked: &'static str,
    pub nothing_found: &'static str,
    /// A module's button, and its name for screen readers („„Denkmalpflege“ aufnehmen"); while it
    /// lands, „Aufgenommen".
    pub take: &'static str,
    pub take_named: fn(&str) -> String,
    pub taken: &'static str,
    /// A module without a Termin of the semester to compare: „keine Termine im WiSe 2026/27".
    pub no_dates: fn(&str) -> String,
    /// The note after „Aufnehmen", with „Rückgängig": „„Denkmalpflege“ in „Mein Studiengang“
    /// aufgenommen"; of several at once „3 gemerkte Module in „Mein Studiengang“ aufgenommen",
    /// after „3 gemerkte Module aufnehmen".
    pub added: fn(&str, &str) -> String,
    pub all_added: fn(usize, &str) -> String,
    pub take_all: fn(usize) -> String,
    /// The way to the whole catalog at the end: what fits the week.
    pub to_catalog: &'static str,
}

pub const DE: Texts = Texts {
    title: |calendar| format!("Modul in „{calendar}“"),
    mine_says: "Zählt in „Mein Studium“ als Besuch, ohne LP.",
    calendar_says: |calendar| format!("Module in „{calendar}“ zählen nirgends."),
    close: "Schließen",
    ways: "Woher",
    marked: |n| match n {
        0 => "Merkliste".to_string(),
        n => format!("Merkliste ({n})"),
    },
    catalog: "Katalog",
    search: "Modul, Nummer oder Thema",
    search_hint: "Ein Name oder eine Nummer, und die Module dazu stehen hier.",
    nothing_marked: "Die Merkliste hat kein Modul mehr dafür.",
    nothing_found: "Nichts gefunden.",
    take: "Aufnehmen",
    take_named: |title| format!("„{title}“ aufnehmen"),
    taken: "Aufgenommen",
    no_dates: |semester| format!("keine Termine im {semester}"),
    added: |title, calendar| format!("„{title}“ in „{calendar}“ aufgenommen"),
    all_added: |n, calendar| format!("{n} gemerkte Module in „{calendar}“ aufgenommen"),
    take_all: |n| format!("{n} gemerkte Module aufnehmen"),
    to_catalog: "Im Katalog suchen, mit „Passt in meinen Stundenplan“",
};

pub const EN: Texts = Texts {
    title: |calendar| format!("Module into “{calendar}”"),
    mine_says: "Counts in “My studies” as a visit, without credits.",
    calendar_says: |calendar| format!("Modules in “{calendar}” count nowhere."),
    close: "Close",
    ways: "From",
    marked: |n| match n {
        0 => "Saved".to_string(),
        n => format!("Saved ({n})"),
    },
    catalog: "Catalogue",
    search: "Module, number or topic",
    search_hint: "A name or a number, and its modules show here.",
    nothing_marked: "Your saved modules hold no more for it.",
    nothing_found: "Nothing found.",
    take: "Add",
    take_named: |title| format!("Add “{title}”"),
    taken: "Added",
    no_dates: |semester| format!("no dates in {semester}"),
    added: |title, calendar| format!("“{title}” added to “{calendar}”"),
    all_added: |n, calendar| format!("{n} saved modules added to “{calendar}”"),
    take_all: |n| format!("Add {n} saved modules"),
    to_catalog: "Search the catalogue, with “Fits my timetable”",
};
