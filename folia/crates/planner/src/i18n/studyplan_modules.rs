//! Texts of the Stundenplan's modules (`pages/studyplan/modules.rs`): the list beside the week, a
//! line per module and under it a row per event that has a Termin (its kind's word and what it
//! holds), its eye while the plan is edited, the list's foot and its way to „Mein Studium", and the
//! empty week's ways. The word of an event is `folia_timetable::categories::word`: QIS's in German,
//! the kinds' labels in English.

pub struct Texts {
    /// The heading of the list, the calendar's name („Mein Studiengang", owner 2026-10-26: a part
    /// per calendar), and its name for screen readers („Module im Stundenplan").
    pub heading: &'static str,
    pub list: &'static str,
    /// A planned module the catalog has no title for, by its number: „Modul 12104".
    pub module_numbered: fn(&str) -> String,
    /// The semester has dates, the module none in QIS: „keine Termine in QIS", beside „Termin
    /// eintragen".
    pub no_dates: &'static str,
    /// „+ Modul" in the head of a calendar of one's own: its words after the „+", and its name with
    /// the calendar's („Modul in „Lena“ aufnehmen").
    pub add: &'static str,
    pub add_named: fn(&str) -> String,
    /// After the title of a module only visited: „Besuch".
    pub visit: &'static str,
    /// The last line of „Mein Studiengang"'s part: where the modules of the study are planned
    /// („Module planen in „Mein Studium“").
    pub to_study: &'static str,
    /// A row's summary of an event of groups: „4 Gruppen", then „offen" in the tone of what is to
    /// decide while none is ticked, else „1 von 4" after the times of the ticked.
    pub groups: fn(usize) -> String,
    pub open: &'static str,
    pub of: fn(usize, usize) -> String,
    /// The end of the summary of an extra: „freiwillig".
    pub optional: &'static str,
    /// An event whose Termine have no fixed time: „ohne feste Zeit".
    pub no_time: &'static str,
    /// An option of a choice among the module's events another was taken over: „nicht gewählt".
    pub not_chosen: &'static str,
    /// A row's link for a screen reader: „Vorlesung EvS, Di 11:30 · Fr 09:15: alle Termine"; of an
    /// event without any date, which leads to its module beside the plan, „…: im Modul".
    pub row_label: fn(&str, &str, &str) -> String,
    pub row_label_module: fn(&str, &str, &str) -> String,
    /// The eye of a row (`role="switch"`): the row's word, the labels of its kinds in the page's
    /// language („" where it has none Betula knows), what tells two of a kind apart („ · Mo"), the
    /// module — „Vorlesung EvS zeigen", "Show lecture · Mon EvS".
    pub show_event: fn(&str, &str, &str, &str) -> String,
    /// What a row says is hidden of an event that shows: „1 Gruppe", „2 Termine", „am Fr 20.11.",
    /// „an 3 Tagen", joined by „ · " and then „… ausgeblendet".
    pub hidden_on: fn(&str) -> String,
    pub hidden_days: fn(usize) -> String,
    pub hidden_end: fn(&str) -> String,
    /// The note after „Alle einblenden": „3 wieder eingeblendet".
    pub shown_again: fn(usize) -> String,
    /// The empty week: what it will hold, and its first way („Zu „Mein Studium“").
    pub empty_week: &'static str,
    pub to_my_study: &'static str,
}

pub const DE: Texts = Texts {
    heading: "Mein Studiengang",
    list: "Module im Stundenplan",
    module_numbered: |id| format!("Modul {id}"),
    no_dates: "keine Termine in QIS",
    add: "Modul",
    add_named: |calendar| format!("Modul in „{calendar}“ aufnehmen"),
    visit: "Besuch",
    to_study: "Module planen in „Mein Studium“",
    groups: |n| if n == 1 { "1 Gruppe".to_string() } else { format!("{n} Gruppen") },
    open: "offen",
    of: |ticked, of| format!("{ticked} von {of}"),
    optional: "freiwillig",
    no_time: "ohne feste Zeit",
    not_chosen: "nicht gewählt",
    row_label: |kind, module, said| format!("{kind} {module}, {said}: alle Termine"),
    row_label_module: |kind, module, said| format!("{kind} {module}, {said}: im Modul"),
    // The row's word as it stands, so that the name holds what the row shows.
    show_event: |word, _, _, module| format!("{word} {module} zeigen"),
    hidden_on: |day| format!("am {day}"),
    hidden_days: |n| format!("an {n} Tagen"),
    hidden_end: |parts| format!("{parts} ausgeblendet"),
    shown_again: |n| format!("{n} wieder eingeblendet"),
    empty_week: "Hier stehen die Termine der Module, die du in „Mein Studium“ planst.",
    to_my_study: "Zu „Mein Studium“",
};

pub const EN: Texts = Texts {
    heading: "My programme",
    list: "Modules in the timetable",
    module_numbered: |id| format!("Module {id}"),
    no_dates: "no dates in QIS",
    add: "Module",
    add_named: |calendar| format!("Add a module to “{calendar}”"),
    visit: "Visit",
    to_study: "Plan modules in “My studies”",
    groups: |n| if n == 1 { "1 group".to_string() } else { format!("{n} groups") },
    open: "open",
    of: |ticked, of| format!("{ticked} of {of}"),
    optional: "optional",
    no_time: "no fixed time",
    not_chosen: "not chosen",
    row_label: |kind, module, said| format!("{kind} {module}, {said}: all sessions"),
    row_label_module: |kind, module, said| format!("{kind} {module}, {said}: in the module"),
    // "Show lecture EvS", "Show exercise · Mon EvS": Betula's own label in the sentence; QIS's word
    // as it is written where it names no kind Betula knows — a name is never lower-cased.
    show_event: |word, kinds, apart, module| match kinds {
        "" => format!("Show {word} {module}"),
        kinds => format!("Show {}{apart} {module}", kinds.to_lowercase()),
    },
    hidden_on: |day| format!("on {day}"),
    hidden_days: |n| format!("on {n} days"),
    hidden_end: |parts| format!("{parts} hidden"),
    shown_again: |n| format!("{n} shown again"),
    empty_week: "The sessions of the modules you plan in “My studies” appear here.",
    to_my_study: "To “My studies”",
};
