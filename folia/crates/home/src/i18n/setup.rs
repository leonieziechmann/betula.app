//! Texts of the start page's setup (`home/setup.rs`, concept A „Einrichtung"): three steps on one
//! card, the program, the semester the study began in, and the modules of the Fachsemester that
//! follows from it which are taken now; and where the site offers Betula Sync a fourth, the profile
//! on the phone as well (`home/devices.rs`). What a step says follows the usual case and refuses nothing
//! (CLAUDE.md, „Betula does not say no"): an unusual choice is said beside it.

pub struct Texts {
    /// The steps, as the chips at the top of the card name them.
    pub step_program: &'static str,
    pub step_start: &'static str,
    pub step_semester: &'static str,
    pub step_devices: &'static str,
    /// „Studium einrichten: Schritt 2 von 3", for screen readers.
    pub steps_label: &'static str,
    /// A step done, for screen readers before what it holds.
    pub done: &'static str,

    // Step 1
    pub program_title: &'static str,
    pub program_lead: &'static str,
    /// „148 Studiengänge in 6 Fakultäten, jeder ein Punkt".
    pub drawing: fn(usize, usize) -> String,
    /// The faculties of the drawing's key, by their number: a short name each.
    pub faculty: fn(&str) -> String,
    pub across_faculties: &'static str,
    pub look_around: &'static str,
    pub look_around_title: &'static str,
    pub take_profile: &'static str,
    pub take_profile_title: &'static str,
    /// The same where the site offers Betula Sync: from the phone by its code (`/pair`).
    pub take_profile_title_sync: &'static str,

    // Step 2
    pub start_title: &'static str,
    pub start_lead: &'static str,
    /// The group of the Studienbeginne, for screen readers.
    pub start_label: &'static str,
    /// „Ich fange gerade an", „jetzt im 3. Fachsemester".
    pub starting: &'static str,
    pub now_in: fn(u8) -> String,
    /// „3." and „FS" of the number at the right.
    pub fs_short: &'static str,
    /// What is unusual about a semester: „über der Regelstudienzeit (6 Sem.)", „vor der PO 2008".
    pub beyond_regular: fn(u8) -> String,
    pub before_po: fn(&str) -> String,
    /// The fifth box, its − and +, and what it takes.
    pub other_fs: &'static str,
    pub fs_less: &'static str,
    pub fs_more: &'static str,
    pub go_on: &'static str,
    pub fs_range: &'static str,
    /// The fifth box's semester, said under its name: „Beginn im WiSe 2023/24".
    pub began_in: fn(&str) -> String,
    /// What „Befehle" says of the semester picked (R25).
    pub picked: &'static str,

    // Step 3
    /// „Was belegst du im WiSe 2026/27?"
    pub semester_title: fn(&str) -> String,
    /// „Vorausgewählt nach Regelstudienplan (3. FS). Lass nur angehakt, was du wirklich belegst:
    /// 5 Module, 32 LP kommen in deinen Stundenplan." (the Fachsemester, what is ticked).
    pub semester_lead: fn(u8, usize, &str) -> String,
    pub semester_lead_empty: fn(u8) -> String,
    /// A row whose kind no source states.
    pub module: &'static str,
    pub placeholder: &'static str,
    /// „FS 3–4": the Fachsemester a row spans.
    pub span: fn(u8, u8) -> String,
    pub direction: &'static str,
    pub no_plan: &'static str,
    pub beyond_plan: fn(u8) -> String,
    pub past_switch: &'static str,
    /// What „Befehle" says of the switch while it is on (R25).
    pub switch_on: &'static str,
    /// „4 Module, 28 LP aus FS 1", „aus FS 1–2".
    pub past_detail: fn(usize, &str, u8) -> String,
    pub past_none: &'static str,
    pub without_modules: &'static str,
    pub create: &'static str,
    pub back: &'static str,
    /// The chip of step 3 once the study is set up: „4 Module".
    pub semester_done: fn(usize) -> String,

    // Step 4 (Betula Sync), the other device a phone; on a phone's layout a laptop (`_phone`).
    pub devices_title: &'static str,
    pub devices_title_phone: &'static str,
    pub devices_lead: &'static str,
    pub devices_lead_phone: &'static str,
    /// What the server then keeps, and that it ends: a point each, in the order of their icons
    /// (a lock, a person, a bin).
    pub devices_points: [&'static str; 3],
    /// „Später", and its title: where it goes later.
    pub later: &'static str,
    pub later_title: &'static str,
    pub activate: &'static str,
    pub activating: &'static str,
    /// What failed, before why (`folia_widgets::syncpair::problem_text`); „Erneut versuchen".
    pub failed: &'static str,
    pub retry: &'static str,
    /// Switched on: the link for the next device, while it is made.
    pub link_title: &'static str,
    pub link_title_phone: &'static str,
    pub link_lead: &'static str,
    pub link_lead_phone: &'static str,
    pub link_making: &'static str,
    /// On a phone: open the address on the laptop, then scan its code with this phone.
    pub elsewhere_open: &'static str,
    pub elsewhere_scan: &'static str,
    /// The other device has the profile: the title, what that means, the note, and „Noch ein Gerät
    /// verbinden".
    pub joined_title: &'static str,
    pub joined_lead: &'static str,
    pub joined: &'static str,
    pub pair_more: &'static str,
    pub finish: &'static str,
}

pub const DE: Texts = Texts {
    step_program: "Studiengang",
    step_start: "Beginn",
    step_semester: "Semester",
    step_devices: "Geräte",
    steps_label: "Schritte der Einrichtung",
    done: "Erledigt: ",

    program_title: "Was studierst du?",
    program_lead: "In drei Klicks zu deinem Stundenplan. Ohne Konto, alles bleibt in diesem Browser.",
    drawing: |programs, faculties| format!("{programs} Studiengänge in {faculties} Fakultäten, jeder ein Punkt"),
    faculty: |code| format!("Fakultät {code}"),
    across_faculties: "übergreifend",
    look_around: "Erst mal umschauen",
    look_around_title: "Die Einrichtung einklappen; sie bleibt oben erreichbar",
    take_profile: "Profil übernehmen",
    take_profile_title: "Schon eingerichtet? Ein Profil als Datei von einem anderen Gerät übernehmen",
    take_profile_title_sync: "Schon auf dem Handy eingerichtet? Scanne einen Code und hol dein Profil hierher",

    start_title: "Seit wann studierst du?",
    start_lead: "Daraus folgt dein Fachsemester.",
    start_label: "Studienbeginn",
    starting: "Ich fange gerade an",
    now_in: |fs| format!("jetzt im {fs}. Fachsemester"),
    fs_short: "FS",
    beyond_regular: |semesters| format!("über der Regelstudienzeit ({semesters}\u{a0}Sem.)"),
    before_po: |year| format!("vor der PO {year}"),
    other_fs: "Anderes Fachsemester",
    fs_less: "Ein Fachsemester weniger",
    fs_more: "Ein Fachsemester mehr",
    go_on: "Weiter",
    fs_range: "Von 1 bis 30: so weit zählt Betula Fachsemester",
    began_in: |semester| format!("Beginn im {semester}"),
    picked: "gewählt",

    semester_title: |semester| format!("Was belegst du im {semester}?"),
    semester_lead: |fs, modules, credits| match modules {
        0 => format!("Vorausgewählt nach Regelstudienplan ({fs}.\u{a0}FS). Nichts angehakt: Dein Stundenplan startet leer."),
        1 => format!("Vorausgewählt nach Regelstudienplan ({fs}.\u{a0}FS). Lass nur angehakt, was du wirklich belegst: 1 Modul, {credits}\u{a0}LP kommt in deinen Stundenplan."),
        n => format!("Vorausgewählt nach Regelstudienplan ({fs}.\u{a0}FS). Lass nur angehakt, was du wirklich belegst: {n} Module, {credits}\u{a0}LP kommen in deinen Stundenplan."),
    },
    semester_lead_empty: |fs| format!("Dein {fs}.\u{a0}Fachsemester."),
    module: "Modul",
    placeholder: "als Platzhalter, später wählen",
    span: |from, to| if from == to { format!("FS\u{a0}{from}") } else { format!("FS\u{a0}{from}–{to}") },
    direction: "Studienrichtung",
    no_plan: "Für diesen Studiengang hat Betula keinen geprüften Regelstudienplan. Dein Stundenplan startet leer; Module planst du aus dem Katalog.",
    beyond_plan: |last| format!("Der Regelstudienplan endet mit dem {last}. Fachsemester. Dein Stundenplan startet leer."),
    past_switch: "Bisherige Module als bestanden markieren",
    switch_on: "an",
    past_detail: |modules, credits, last| match last {
        1 => format!("{modules} {}, {credits}\u{a0}LP aus FS\u{a0}1", if modules == 1 { "Modul" } else { "Module" }),
        last => format!("{modules} {}, {credits}\u{a0}LP aus FS\u{a0}1–{last}", if modules == 1 { "Modul" } else { "Module" }),
    },
    past_none: "Im 1. Fachsemester gibt es noch nichts zu markieren.",
    without_modules: "Ohne Module weiter",
    create: "Studium anlegen",
    back: "Zurück",
    semester_done: |modules| match modules {
        0 => "Ohne Module".to_string(),
        1 => "1 Modul".to_string(),
        n => format!("{n} Module"),
    },

    devices_title: "Auch auf dem Handy?",
    devices_title_phone: "Auch am Laptop?",
    devices_lead: "Mit Betula Sync hast du deinen Stundenplan und alles andere auch auf dem Handy. Was du auf einem Gerät änderst, ist sofort auf dem anderen.",
    devices_lead_phone: "Mit Betula Sync hast du deinen Stundenplan und alles andere auch am Laptop. Was du auf einem Gerät änderst, ist sofort auf dem anderen.",
    devices_points: [
        "Verschlüsselt: Den Schlüssel haben nur deine Geräte. Der Server bewahrt deine Änderungen auf, kann sie aber nicht öffnen.",
        "Ohne Konto: kein Name, keine E-Mail-Adresse. Der Server kennt nur zufällige Kennungen, Größen und Zeiten.",
        "Jederzeit im Profil zu beenden: Dann ist alles auf dem Server weg, deine Geräte behalten ihre Kopie. Ebenso nach 13 Monaten ohne Abruf.",
    ],
    later: "Später",
    later_title: "Geht jederzeit im Profil unter „Betula Sync“",
    activate: "Betula Sync aktivieren",
    activating: "Wird eingerichtet …",
    failed: "Das hat nicht geklappt.",
    retry: "Erneut versuchen",
    link_title: "Jetzt dein Handy verbinden",
    link_title_phone: "Jetzt deinen Laptop verbinden",
    link_lead: "Betula Sync ist an. Dein Handy bekommt dieses Profil, und von da an bleibt es auf beiden gleich.",
    link_lead_phone: "Betula Sync ist an. Hol dein Profil jetzt auf den Laptop, von da an bleibt es auf beiden gleich.",
    link_making: "Link wird vorbereitet …",
    elsewhere_open: "Öffne auf deinem Laptop diese Adresse:",
    elsewhere_scan: "Dort erscheint ein Code. Scanne ihn mit der Kamera dieses Handys und tippe auf „Verbinden“. Dann ist dein Laptop angemeldet, mit allem, was du hier eingerichtet hast.",
    joined_title: "Verbunden",
    joined_lead: "Was du auf einem Gerät änderst, ist auf dem anderen sofort da, auch nach einer Weile offline.",
    joined: "Dein Profil ist jetzt auch auf dem anderen Gerät.",
    pair_more: "Noch ein Gerät verbinden",
    finish: "Fertig",
};

pub const EN: Texts = Texts {
    step_program: "Programme",
    step_start: "Start",
    step_semester: "Semester",
    step_devices: "Devices",
    steps_label: "Steps of the set-up",
    done: "Done: ",

    program_title: "What do you study?",
    program_lead: "Three clicks to your timetable. No account, everything stays in this browser.",
    drawing: |programs, faculties| format!("{programs} degree programmes in {faculties} faculties, one dot each"),
    faculty: |code| format!("Faculty {code}"),
    across_faculties: "across faculties",
    look_around: "Look around first",
    look_around_title: "Fold the set-up away; it stays within reach at the top",
    take_profile: "Bring a profile",
    take_profile_title: "Set up elsewhere already? Bring a profile from another device as a file",
    take_profile_title_sync: "Set up on your phone already? Scan a code and bring your profile here",

    start_title: "When did you start?",
    start_lead: "That gives your semester of study.",
    start_label: "Start of studies",
    starting: "I'm just starting",
    now_in: |fs| format!("now in semester {fs}"),
    fs_short: "sem.",
    beyond_regular: |semesters| format!("beyond the standard period ({semesters}\u{a0}sem.)"),
    before_po: |year| format!("before the {year} regulations"),
    other_fs: "Another semester",
    fs_less: "One semester less",
    fs_more: "One semester more",
    go_on: "Continue",
    fs_range: "1 to 30: Betula counts semesters that far",
    began_in: |semester| format!("Started in {semester}"),
    picked: "picked",

    semester_title: |semester| format!("What are you taking in {semester}?"),
    semester_lead: |fs, modules, credits| match modules {
        0 => format!("Preselected from the standard study plan (semester\u{a0}{fs}). Nothing ticked: your timetable starts empty."),
        1 => format!("Preselected from the standard study plan (semester\u{a0}{fs}). Keep ticked only what you really take: 1 module, {credits}\u{a0}ECTS goes into your timetable."),
        n => format!("Preselected from the standard study plan (semester\u{a0}{fs}). Keep ticked only what you really take: {n} modules, {credits}\u{a0}ECTS go into your timetable."),
    },
    semester_lead_empty: |fs| format!("Your semester\u{a0}{fs}."),
    module: "Module",
    placeholder: "as a placeholder, choose later",
    span: |from, to| if from == to { format!("sem.\u{a0}{from}") } else { format!("sem.\u{a0}{from}–{to}") },
    direction: "Study direction",
    no_plan: "Betula has no checked standard study plan for this programme. Your timetable starts empty; you plan modules from the catalogue.",
    beyond_plan: |last| format!("The standard study plan ends with semester {last}. Your timetable starts empty."),
    past_switch: "Mark earlier modules as passed",
    switch_on: "on",
    past_detail: |modules, credits, last| match last {
        1 => format!("{modules} {}, {credits}\u{a0}ECTS of semester\u{a0}1", if modules == 1 { "module" } else { "modules" }),
        last => format!("{modules} {}, {credits}\u{a0}ECTS of semesters\u{a0}1–{last}", if modules == 1 { "module" } else { "modules" }),
    },
    past_none: "In the first semester there is nothing to mark yet.",
    without_modules: "Continue without modules",
    create: "Set up my studies",
    back: "Back",
    semester_done: |modules| match modules {
        0 => "No modules".to_string(),
        1 => "1 module".to_string(),
        n => format!("{n} modules"),
    },

    devices_title: "On your phone too?",
    devices_title_phone: "On your laptop too?",
    devices_lead: "With Betula Sync your timetable and everything else is on your phone as well. What you change on one device is on the other at once.",
    devices_lead_phone: "With Betula Sync your timetable and everything else is on your laptop as well. What you change on one device is on the other at once.",
    devices_points: [
        "Encrypted: only your devices have the key. The server keeps your changes but cannot open them.",
        "No account: no name, no e-mail address. The server knows only random ids, sizes and times.",
        "Can be ended at any time in the profile: then everything on the server is gone, and your devices keep their copy. The same after 13 months without a fetch.",
    ],
    later: "Later",
    later_title: "Possible at any time in the profile under “Betula Sync”",
    activate: "Turn on Betula Sync",
    activating: "Setting up …",
    failed: "That did not work.",
    retry: "Try again",
    link_title: "Now connect your phone",
    link_title_phone: "Now connect your laptop",
    link_lead: "Betula Sync is on. Your phone gets this profile, and from then on it stays the same on both.",
    link_lead_phone: "Betula Sync is on. Now get your profile onto your laptop; from then on it stays the same on both.",
    link_making: "Preparing the link …",
    elsewhere_open: "On your laptop, open this address:",
    elsewhere_scan: "A code appears there. Scan it with the camera of this phone and tap “Connect”. Then your laptop is signed in, with everything you set up here.",
    joined_title: "Connected",
    joined_lead: "What you change on one device is on the other at once, even after a while offline.",
    joined: "Your profile is now on the other device too.",
    pair_more: "Connect another device",
    finish: "Done",
};
