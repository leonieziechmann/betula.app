//! Texts of the calendars of one's own (`studyplan/calendars.rs`; folia/design/kalender): their
//! list beside the week and on a phone over it, how each stands in the week, a new one from a link,
//! its menu, „Gemeinsam frei", and a calendar's part of the module list.

pub struct Texts {
    // ---- the list ----
    /// The group's heading and the phone's line: „Kalender".
    pub heading: &'static str,
    /// „Mein Studiengang", which is always there and always in full („immer voll").
    pub mine: &'static str,
    pub always_full: &'static str,
    /// Under „Mein Studiengang" without a program: „dein Studium".
    pub your_study: &'static str,
    /// A calendar without a name: „Kalender 2".
    pub unnamed: fn(u8) -> String,
    /// Under a calendar's name: „4 Module · MW1, GdW, ABWL1" …
    pub modules: fn(usize, &str) -> String,
    /// … while its Termine come, where its modules have none in the semester, and where the
    /// catalog could not answer.
    pub loading: &'static str,
    pub nothing_here: fn(&str) -> String,
    pub failed: &'static str,
    /// The three ways a calendar stands in the week, as the switch says them and as their tooltips.
    pub full: &'static str,
    pub strip: &'static str,
    pub off: &'static str,
    pub full_title: &'static str,
    pub strip_title: &'static str,
    pub off_title: &'static str,
    /// The switch's name for a screen reader: „Lena in der Woche".
    pub in_week: fn(&str) -> String,
    /// The phone's line: „Mein Studiengang, Lena als Spur" …
    pub line_full: fn(&str) -> String,
    pub line_strip: fn(&str) -> String,
    pub line_off: fn(&str) -> String,

    // ---- a calendar's menu ----
    pub menu: fn(&str) -> String,
    pub rename: &'static str,
    pub remove: &'static str,
    pub save: &'static str,
    /// After „Löschen": „„Lena" gelöscht" with „Rückgängig".
    pub removed: fn(&str) -> String,

    // ---- a new calendar ----
    pub new_calendar: &'static str,
    /// What it is made of: „Aus dem Link, den dir jemand aus dem eigenen Stundenplan geschickt hat
    /// („Link zum Teilen kopieren")."
    pub new_hint: &'static str,
    pub link: &'static str,
    pub link_placeholder: &'static str,
    pub name: &'static str,
    pub name_placeholder: &'static str,
    pub create: &'static str,
    pub cancel: &'static str,
    /// What the link holds, before „Anlegen": „4 Module: MW1, GdW, ABWL1, ABWL2, mit 2 Gruppen" …
    pub link_holds: fn(usize, usize) -> String,
    /// … of another semester than the one the page shows („aus dem SoSe 2027: er zeigt sich, sobald
    /// der Kalender dieses Semester zeigt") …
    pub link_elsewhere: fn(&str) -> String,
    /// … or no plan at all.
    pub link_wrong: &'static str,
    /// The eighth is the last (the store holds eight).
    pub full_up: &'static str,
    /// After „Anlegen" and „Als Kalender": „Als Kalender angelegt: Lena".
    pub created: fn(&str) -> String,

    // ---- the shared plan's offer ----
    /// „Als Kalender": the plan of the link beside one's own, not in it.
    pub as_calendar: &'static str,
    pub as_calendar_title: &'static str,

    // ---- the week ----
    /// The switch in the week's head once another calendar shows: „Gemeinsam frei", its tooltip,
    /// and what a phone's row of it says under it.
    pub together: &'static str,
    pub together_title: &'static str,
    pub together_small: &'static str,
    /// A stretch free in every calendar shown: „beide frei", „alle frei" …
    pub both_free: &'static str,
    pub all_free: &'static str,
    /// … its tooltip: „Mo 13:00–15:30 frei, in jedem Kalender, der in der Woche steht".
    pub free_title: fn(&str) -> String,

    // ---- the module list ----
    /// Under a calendar's modules: „aus einem Link · mit den Gruppen", „aus einem Link".
    pub from_link: &'static str,
    pub from_link_groups: &'static str,
    /// The menu's „Modul aufnehmen" (its „+ Modul"); a module's × in the calendar's part, and the
    /// note after it („„Denkmalpflege“ aus „Lena“ genommen"); a part without a module of the
    /// semester („Noch keine Module im WiSe 2026/27").
    pub add_module: &'static str,
    pub remove_module: fn(&str, &str) -> String,
    pub module_removed: fn(&str, &str) -> String,
    pub none_in: fn(&str) -> String,
}

pub const DE: Texts = Texts {
    heading: "Kalender",
    mine: "Mein Studiengang",
    always_full: "immer voll",
    your_study: "dein Studium",
    unnamed: |id| format!("Kalender {id}"),
    modules: |n, names| match (n, names.is_empty()) {
        (1, true) => "1 Modul".to_string(),
        (n, true) => format!("{n} Module"),
        (1, false) => format!("1 Modul · {names}"),
        (n, false) => format!("{n} Module · {names}"),
    },
    loading: "Termine kommen …",
    nothing_here: |semester| format!("keine Module im {semester}"),
    failed: "Termine gerade nicht lesbar",
    full: "Voll",
    strip: "Spur",
    off: "Aus",
    full_title: "voll in der Woche, neben deinen Terminen",
    strip_title: "als Spur am Rand jedes Tages",
    off_title: "nicht in der Woche",
    in_week: |name| format!("{name} in der Woche"),
    line_full: |name| name.to_string(),
    line_strip: |name| format!("{name} als Spur"),
    line_off: |name| format!("{name} aus"),

    menu: |name| format!("Mehr zu {name}"),
    rename: "Umbenennen",
    remove: "Löschen",
    save: "Sichern",
    removed: |name| format!("„{name}“ gelöscht"),

    new_calendar: "Neuer Kalender",
    new_hint: "Aus dem Link, den dir jemand aus dem eigenen Stundenplan geschickt hat („Link zum Teilen kopieren“).",
    link: "Link",
    link_placeholder: "betula.app/studyplan?share=…",
    name: "Name",
    name_placeholder: "z. B. Lena",
    create: "Anlegen",
    cancel: "Abbrechen",
    link_holds: |modules, groups| {
        let modules = match modules {
            1 => "1 Modul".to_string(),
            n => format!("{n} Module"),
        };
        match groups {
            0 => modules,
            1 => format!("{modules}, mit 1 Gruppe"),
            n => format!("{modules}, mit {n} Gruppen"),
        }
    },
    link_elsewhere: |semester| format!("aus dem {semester}: Er steht in der Woche, sobald der Stundenplan dieses Semester zeigt."),
    link_wrong: "Das ist kein Link zu einem Stundenplan.",
    full_up: "Acht eigene Kalender sind das Höchste.",
    created: |name| format!("Als Kalender angelegt: {name}"),

    as_calendar: "Als Kalender",
    as_calendar_title: "Den Plan als eigenen Kalender neben deinen legen, mit seinen Gruppen. Er zählt nicht für dein Studium.",

    together: "Gemeinsam frei",
    together_title: "Wo du und die anderen Kalender in der Woche frei seid, ab 90 Minuten zwischen 8 und 18 Uhr",
    together_small: "Wo alle frei sind, ab 90 Minuten",
    both_free: "beide frei",
    all_free: "alle frei",
    free_title: |when| format!("{when} frei, in jedem Kalender in der Woche"),

    from_link: "aus einem Link",
    from_link_groups: "aus einem Link · mit den Gruppen",
    add_module: "Modul aufnehmen",
    remove_module: |title, calendar| format!("„{title}“ aus „{calendar}“ nehmen"),
    module_removed: |title, calendar| format!("„{title}“ aus „{calendar}“ genommen"),
    none_in: |semester| format!("Noch keine Module im {semester}"),
};

pub const EN: Texts = Texts {
    heading: "Calendars",
    mine: "My programme",
    always_full: "always in full",
    your_study: "your study",
    unnamed: |id| format!("Calendar {id}"),
    modules: |n, names| match (n, names.is_empty()) {
        (1, true) => "1 module".to_string(),
        (n, true) => format!("{n} modules"),
        (1, false) => format!("1 module · {names}"),
        (n, false) => format!("{n} modules · {names}"),
    },
    loading: "sessions on their way …",
    nothing_here: |semester| format!("no modules in {semester}"),
    failed: "sessions cannot be read right now",
    full: "Full",
    strip: "Strip",
    off: "Off",
    full_title: "in full in the week, beside your sessions",
    strip_title: "as a strip at the edge of each day",
    off_title: "not in the week",
    in_week: |name| format!("{name} in the week"),
    line_full: |name| name.to_string(),
    line_strip: |name| format!("{name} as a strip"),
    line_off: |name| format!("{name} off"),

    menu: |name| format!("More on {name}"),
    rename: "Rename",
    remove: "Delete",
    save: "Save",
    removed: |name| format!("“{name}” deleted"),

    new_calendar: "New calendar",
    new_hint: "From the link someone sent you from their own timetable (“Copy link to share”).",
    link: "Link",
    link_placeholder: "betula.app/studyplan?share=…",
    name: "Name",
    name_placeholder: "e.g. Lena",
    create: "Create",
    cancel: "Cancel",
    link_holds: |modules, groups| {
        let modules = match modules {
            1 => "1 module".to_string(),
            n => format!("{n} modules"),
        };
        match groups {
            0 => modules,
            1 => format!("{modules}, with 1 group"),
            n => format!("{modules}, with {n} groups"),
        }
    },
    link_elsewhere: |semester| format!("from {semester}: it stands in the week once the timetable shows that semester."),
    link_wrong: "This is no link to a timetable.",
    full_up: "Eight calendars of your own are the most.",
    created: |name| format!("Created as a calendar: {name}"),

    as_calendar: "As a calendar",
    as_calendar_title: "Put the plan beside yours as a calendar of its own, with its groups. It does not count for your study.",

    together: "Free together",
    together_title: "Where you and the other calendars in the week are free, 90 minutes or more between 8 and 18",
    together_small: "Where all are free, 90 minutes or more",
    both_free: "both free",
    all_free: "all free",
    free_title: |when| format!("{when} free, in every calendar in the week"),

    from_link: "from a link",
    from_link_groups: "from a link · with the groups",
    add_module: "Add a module",
    remove_module: |title, calendar| format!("Remove “{title}” from “{calendar}”"),
    module_removed: |title, calendar| format!("“{title}” removed from “{calendar}”"),
    none_in: |semester| format!("No modules in {semester} yet"),
};
