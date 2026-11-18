//! Texts of the Termine of one's own (`studyplan/own.rs`; folia/design/kalender/termine.html):
//! „+ Termin", the form that makes and changes one, the list under a calendar's modules, a module
//! without Termine, and the notes with „Rückgängig".

pub struct Texts {
    // ---- the ways in ----
    /// „+ Termin" in the week's head, under a calendar's modules and on a phone's round button,
    /// with its tooltip.
    pub add: &'static str,
    pub add_title: &'static str,
    /// „Termin eintragen" under a module without a Termin of QIS (whose row says „keine Termine in
    /// QIS", `studyplan_modules::no_dates`), with its tooltip.
    pub enter: &'static str,
    pub enter_title: fn(&str) -> String,
    /// The name of a module's „+ Termin" in the list: „Termin für EvS eintragen"; of a calendar's
    /// own, „Termin in „Lena“ eintragen".
    pub add_for: fn(&str) -> String,
    pub add_in: fn(&str) -> String,
    /// „Löschen" in the form's foot of a Termin of a calendar of one's own (the bin).
    pub delete: &'static str,

    // ---- the form ----
    /// Its title: „Neuer Termin", or „„Arbeit“ bearbeiten" („Termin bearbeiten" without a name).
    pub form_new: &'static str,
    pub form_edit: fn(&str) -> String,
    pub form_edit_unnamed: &'static str,
    /// The line under it: of which calendar and semester („In „Mein Studiengang“, WiSe 2026/27").
    pub form_sub: fn(&str, &str) -> String,
    /// „Was", and its placeholder.
    pub what: &'static str,
    pub what_placeholder: &'static str,
    /// „Wann": every week, in the A or the B weeks, or on dates of its own.
    pub when: &'static str,
    pub every_week: &'static str,
    pub a_week: &'static str,
    pub b_week: &'static str,
    pub on_dates: &'static str,
    /// „Tag" of one that recurs; „Datum" of one on dates, „+ Datum" and the × of one of them.
    pub day: &'static str,
    pub date: &'static str,
    pub add_date: &'static str,
    pub remove_date: &'static str,
    /// „Zeit", „Beginn", „Ende" and „bis" between them.
    pub time: &'static str,
    pub begin: &'static str,
    pub end: &'static str,
    pub until: &'static str,
    /// „Zeitraum": the lecture period („05.10.–31.01., die Vorlesungszeit"), „ändern", its own „von"
    /// and „bis", and the way back to the lecture period („Vorlesungszeit").
    pub period: &'static str,
    pub lectures: fn(&str) -> String,
    pub lectures_unknown: &'static str,
    pub change: &'static str,
    pub from: &'static str,
    pub to: &'static str,
    pub back_to_lectures: &'static str,
    /// „Wo", and its placeholder.
    pub place: &'static str,
    pub place_placeholder: &'static str,
    /// „Kalender" and „Mein Studiengang" in its list.
    pub calendar: &'static str,
    pub mine: &'static str,
    /// „Gehört zu", and „kein Modul".
    pub module: &'static str,
    pub no_module: &'static str,
    /// „Art", the field of its kind, and „Termin", its option for none (the kinds are the
    /// timetable's labels: „Vorlesung", „Übung" …).
    pub kind: &'static str,
    pub no_kind: &'static str,
    /// What it meets of the week: „Passt in deine Woche", „Trifft Ü MIT1 (Mi 09:15)." with
    /// „Anlegen geht trotzdem." / „Speichern geht trotzdem.", and of another calendar „Zählt für
    /// „Gemeinsam frei“ mit Lena".
    pub fits: &'static str,
    pub meets: fn(&str) -> String,
    pub meets_anyway_new: &'static str,
    pub meets_anyway_edit: &'static str,
    pub in_calendar: fn(&str) -> String,
    /// What cannot be a Termin: no time, an end before its beginning, no date, a period that ends
    /// before it begins.
    pub no_time: &'static str,
    pub end_before: &'static str,
    pub no_date: &'static str,
    pub bad_period: &'static str,
    /// The buttons.
    pub create: &'static str,
    pub save: &'static str,
    pub cancel: &'static str,
    /// The store holds no more („100 eigene Termine sind da das Höchste").
    pub full_up: &'static str,

    // ---- the notes ----
    /// „„Chor“ angelegt", „„Chor“ gespeichert", „„Chor“ gelöscht" (each with „Rückgängig"); the
    /// name as `said` gives it: in quotes where the student wrote it, bare where it is made of the
    /// kind and the module („VL EvS gelöscht").
    pub created: fn(&str) -> String,
    pub saved: fn(&str) -> String,
    pub removed: fn(&str) -> String,
    pub said: fn(&str) -> String,

    // ---- the list and the week ----
    /// The list's heading under a calendar's modules: „Eigene Termine".
    pub heading: &'static str,
    /// A Termin of one's own without a name: „Eigener Termin".
    pub unnamed: &'static str,
    /// In the list and the tooltips: „A", „B", „3 Tage".
    pub weeks_a: &'static str,
    pub weeks_b: &'static str,
    pub days: fn(usize) -> String,
    /// The tooltip of a slot of one's own: „von dir, nicht in Datei und festem Abo" (a share that
    /// stays current takes them along).
    pub by_you: &'static str,
    /// The mark of a Termin of one's own in the list of modules: „von dir".
    pub mark: &'static str,
    /// The form's sub-line opened for a module: „Für EvS · WiSe 2026/27"; „Was" then needs no name
    /// (its placeholder „optional": the week calls it by its kind and module).
    pub form_for: fn(&str, &str) -> String,
    pub what_optional: &'static str,
    /// The draft in the week while the form is open: „Neuer Termin".
    pub draft: &'static str,
    /// The tooltip of a slot of a Termin of QIS changed by hand (step 2): „von dir geändert", and
    /// where QIS says something else now „… QIS sagt inzwischen etwas anderes".
    pub changed: &'static str,
    pub changed_differs: &'static str,
    /// The same of a slot of a calendar of one's own: its change came with its link.
    pub changed_elsewhere: &'static str,
}

pub const DE: Texts = Texts {
    add: "Termin",
    add_title: "Eigenen Termin anlegen: Arbeit, Sport, eine Lerngruppe",
    enter: "Termin eintragen",
    enter_title: |module| format!("Einen Termin für {module} eintragen, den QIS nicht kennt"),
    add_for: |module| format!("Termin für {module} eintragen"),
    add_in: |calendar| format!("Termin in „{calendar}“ eintragen"),
    delete: "Löschen",

    form_new: "Neuer Termin",
    form_edit: |name| format!("„{name}“ bearbeiten"),
    form_edit_unnamed: "Termin bearbeiten",
    form_sub: |calendar, semester| format!("In „{calendar}“, {semester}"),
    what: "Was",
    what_placeholder: "z. B. Arbeit, Sport, Lerngruppe",
    when: "Wann",
    every_week: "Jede Woche",
    a_week: "A-Woche",
    b_week: "B-Woche",
    on_dates: "Einmal",
    day: "Tag",
    date: "Datum",
    add_date: "Datum",
    remove_date: "Datum entfernen",
    time: "Zeit",
    begin: "Beginn",
    end: "Ende",
    until: "bis",
    period: "Zeitraum",
    lectures: |range| format!("{range}, die Vorlesungszeit"),
    lectures_unknown: "die Vorlesungszeit",
    change: "ändern",
    from: "von",
    to: "bis",
    back_to_lectures: "Vorlesungszeit",
    place: "Wo",
    place_placeholder: "optional",
    calendar: "Kalender",
    mine: "Mein Studiengang",
    module: "Gehört zu",
    no_module: "kein Modul",
    kind: "Art",
    no_kind: "Termin",
    fits: "Passt in deine Woche",
    meets: |what| format!("Trifft {what}."),
    meets_anyway_new: "Anlegen geht trotzdem.",
    meets_anyway_edit: "Speichern geht trotzdem.",
    in_calendar: |name| format!("Zählt für „Gemeinsam frei“ mit {name}"),
    no_time: "Beginn und Ende fehlen",
    end_before: "Das Ende liegt vor dem Beginn",
    no_date: "Ein Datum fehlt",
    bad_period: "Der Zeitraum endet, bevor er beginnt",
    create: "Anlegen",
    save: "Speichern",
    cancel: "Abbrechen",
    full_up: "100 eigene Termine im Semester sind das Höchste, das der Stundenplan hält.",

    created: |name| format!("{name} angelegt"),
    saved: |name| format!("{name} gespeichert"),
    removed: |name| format!("{name} gelöscht"),
    said: |name| format!("„{name}“"),

    heading: "Eigene Termine",
    unnamed: "Eigener Termin",
    weeks_a: "A",
    weeks_b: "B",
    days: |n| match n {
        1 => "1 Tag".to_string(),
        n => format!("{n} Tage"),
    },
    by_you: "von dir, nicht in Datei und festem Abo",
    mark: "von dir",
    form_for: |module, semester| format!("Für {module} · {semester}"),
    what_optional: "optional",
    draft: "Neuer Termin",
    changed: "von dir geändert",
    changed_differs: "von dir geändert, QIS sagt inzwischen etwas anderes",
    changed_elsewhere: "anders als in QIS",
};

pub const EN: Texts = Texts {
    add: "Event",
    add_title: "Add an event of your own: work, sport, a study group",
    enter: "Add a session",
    enter_title: |module| format!("Add a session of {module} that QIS does not know"),
    add_for: |module| format!("Add a session of {module}"),
    add_in: |calendar| format!("Add an event to “{calendar}”"),
    delete: "Delete",

    form_new: "New event",
    form_edit: |name| format!("Edit “{name}”"),
    form_edit_unnamed: "Edit event",
    form_sub: |calendar, semester| format!("In “{calendar}”, {semester}"),
    what: "What",
    what_placeholder: "e.g. work, sport, study group",
    when: "When",
    every_week: "Every week",
    a_week: "Week A",
    b_week: "Week B",
    on_dates: "Once",
    day: "Day",
    date: "Date",
    add_date: "Date",
    remove_date: "Remove date",
    time: "Time",
    begin: "Start",
    end: "End",
    until: "to",
    period: "Period",
    lectures: |range| format!("{range}, the lecture period"),
    lectures_unknown: "the lecture period",
    change: "change",
    from: "from",
    to: "to",
    back_to_lectures: "Lecture period",
    place: "Where",
    place_placeholder: "optional",
    calendar: "Calendar",
    mine: "My programme",
    module: "Belongs to",
    no_module: "no module",
    kind: "Type",
    no_kind: "Event",
    fits: "Fits your week",
    meets: |what| format!("Meets {what}."),
    meets_anyway_new: "You can still add it.",
    meets_anyway_edit: "You can still save it.",
    in_calendar: |name| format!("Counts for “Free together” with {name}"),
    no_time: "Start and end are missing",
    end_before: "The end is before the start",
    no_date: "A date is missing",
    bad_period: "The period ends before it begins",
    create: "Add",
    save: "Save",
    cancel: "Cancel",
    full_up: "100 events of your own in a semester are the most the timetable holds.",

    created: |name| format!("{name} added"),
    saved: |name| format!("{name} saved"),
    removed: |name| format!("{name} deleted"),
    said: |name| format!("“{name}”"),

    heading: "Your own events",
    unnamed: "Own event",
    weeks_a: "A",
    weeks_b: "B",
    days: |n| match n {
        1 => "1 day".to_string(),
        n => format!("{n} days"),
    },
    by_you: "yours, not in the file or a fixed feed",
    mark: "by you",
    form_for: |module, semester| format!("For {module} · {semester}"),
    what_optional: "optional",
    draft: "New event",
    changed: "changed by you",
    changed_differs: "changed by you, QIS says something else now",
    changed_elsewhere: "not as in QIS",
};
