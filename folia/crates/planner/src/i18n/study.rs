//! Texts of „Mein Studium" (`study/`): the Studium tab's page, the whole study semester by
//! semester.

pub struct Texts {
    /// The page's title and the sidebar's heading.
    pub title: &'static str,
    /// What the server writes in the page's place (the page is the app's): its title and hint.
    pub server_title: &'static str,
    pub server_hint: &'static str,
    /// The description of the page for link previews.
    pub seo_description: &'static str,

    // Without a program.
    /// „Plane dein Studium", the lead under it and the picker's placeholder.
    pub welcome_title: &'static str,
    pub welcome_lead: &'static str,
    pub choose_program: &'static str,
    pub search_program: &'static str,
    /// „Alle Studiengänge ansehen": the way to the overview.
    pub all_programs: &'static str,
    /// The program kept is in the catalog no longer (its stored name).
    pub gone: fn(&str) -> String,

    // The sidebar.
    /// „Studiengang", „Studienrichtung", „Studienbeginn": the groups' labels.
    pub program: &'static str,
    pub direction: &'static str,
    pub start: &'static str,
    /// „Angenommen: Studienbeginn im aktuellen Semester. Stimmt das?" and its button, where no
    /// Studienbeginn is stored.
    pub start_assumed: &'static str,
    pub start_confirm: &'static str,
    /// „1. FS" in the entries of the Studienbeginn's select: the semester it is the start of makes
    /// the current one this Fachsemester („WiSe 2025/26 · jetzt 3. FS").
    pub start_entry: fn(&str, u8) -> String,
    /// The program's pages: its Regelstudienplan, its electives and areas.
    pub to_plan: &'static str,
    pub to_areas: &'static str,
    /// „Zum Stundenplan": the timetable of the current semester.
    pub to_timetable: &'static str,
    /// The legend: what the marks of a row say.
    pub legend: &'static str,
    pub legend_overdue: &'static str,
    pub legend_moved: &'static str,
    pub legend_turnus: &'static str,
    pub legend_done: &'static str,
    /// Where all of it lives.
    pub storage_hint: &'static str,

    // The head.
    /// „Mein Studiengang", the small line over the program's name.
    pub mine: &'static str,
    /// „3. Fachsemester · WiSe 2026/27"; before the Studienbeginn „Studienbeginn WiSe 2026/27".
    pub fachsemester_now: fn(u8, &str) -> String,
    pub starts_in: fn(&str) -> String,
    /// „48 von 180 LP": what is passed of what the plan comes to (both written already).
    pub progress: fn(&str, &str) -> String,
    /// „48 LP bestanden": without a plan to measure against.
    pub progress_alone: fn(&str) -> String,
    /// The progress bar's name, for screen readers.
    pub progress_label: &'static str,
    /// „Regelstudienzeit bis SoSe 2028" …
    pub regular_end: fn(&str) -> String,
    /// … „voraussichtlich fertig: WiSe 2028/29" …
    pub expected_end: fn(&str) -> String,
    /// … and with nothing left open.
    pub all_done: &'static str,
    /// The program has no checked Regelstudienplan: what the page shows then.
    pub no_plan: &'static str,

    // The semesters before the current one.
    /// „Bisher", the summary of the semesters before the current one: „1.–2. Fachsemester" …
    pub past: &'static str,
    pub past_span: fn(u8, u8) -> String,
    /// … „8 von 10 abgehakt" …
    pub past_count: fn(usize, usize) -> String,
    /// … and what it asks while something there is open.
    pub past_prompt: &'static str,
    /// „Alles bestanden": ticks off what a semester before the current one holds.
    pub all_passed: &'static str,

    // A semester.
    /// „3. FS", the badge „jetzt", and „nach der Regelstudienzeit".
    pub fs: fn(u8) -> String,
    pub now: &'static str,
    pub beyond: &'static str,
    /// „34 LP", „≥ 34 LP" (some rows say no number); „Plan: 30 LP" beside it.
    pub credits: fn(&str, bool) -> String,
    pub planned: fn(&str) -> String,
    /// More than the Regelstudienplan puts into the semester: the tooltip of the warning.
    pub heavy: &'static str,
    /// A semester with nothing in it.
    pub empty: &'static str,

    // A row.
    /// The checkbox of a module and of a row, for screen readers: „Analysis I bestanden" …
    pub passed_label: fn(&str) -> String,
    pub done_label: fn(&str) -> String,
    /// … the marks: „nachholen · aus 1. FS" …
    pub overdue: fn(&str) -> String,
    /// … „verschoben · laut Plan 4. FS", „vorgezogen · laut Plan 5. FS" …
    pub moved_later: fn(&str) -> String,
    pub moved_earlier: fn(&str) -> String,
    /// … „nur im WiSe" / „nur im SoSe": the turnus put it here …
    pub only_winter: &'static str,
    pub only_summer: &'static str,
    /// … „eigenes Modul": planned besides the Regelstudienplan …
    pub own: &'static str,
    /// … „bestanden im SoSe 2026" where it was passed …
    pub passed_in: fn(&str) -> String,
    /// … „im Stundenplan": in the timetable of the current semester.
    pub in_timetable: &'static str,
    /// „Module finden": the catalog of a row's modules.
    pub find_modules: &'static str,
    /// The buttons of a row: „Früher: SoSe 2027", „Später: WiSe 2027/28" …
    pub earlier: fn(&str) -> String,
    pub later: fn(&str) -> String,
    /// … „Zurück an seinen Platz", „Aus dem Plan nehmen" …
    pub reset: &'static str,
    pub remove: &'static str,
    /// … and „Nachholen", the way a module of one's own that was not passed is planned again.
    pub again: fn(&str) -> String,

    // The current semester and the timetable.
    /// „In den Stundenplan (4)": takes the semester's modules and rows into its timetable …
    pub take: fn(usize) -> String,
    /// … „Alles im Stundenplan" …
    pub taken_all: &'static str,
    /// … „Übernommen: 4 Module, 1 Platzhalter" (the parts written already), and the way there.
    pub taken: fn(&str) -> String,
}

pub const DE: Texts = Texts {
    title: "Mein Studium",
    server_title: "Dein Studium, Semester für Semester",
    server_hint: "Hier planst du dein Studium: was du bestanden hast, was nachzuholen ist und was in den nächsten Semestern ansteht. Das braucht JavaScript, und alles bleibt in deinem Browser.",
    seo_description: "Dein Studium an der BTU, Semester für Semester: was bestanden ist, was nachzuholen ist und was ansteht. Inoffiziell, nur in deinem Browser.",

    welcome_title: "Plane dein Studium",
    welcome_lead: "Wähle deinen Studiengang. Betula legt dir den Regelstudienplan auf deine Semester und hält ihn aktuell: Was du nicht bestanden hast, kommt im nächsten passenden Semester als Erstes wieder.",
    choose_program: "Studiengang wählen",
    search_program: "Studiengang suchen",
    all_programs: "Alle Studiengänge ansehen",
    gone: |name| format!("„{name}“ ist nicht mehr im Katalog. Wähle deinen Studiengang neu."),

    program: "Studiengang",
    direction: "Studienrichtung",
    start: "Studienbeginn",
    start_assumed: "Angenommen: Studienbeginn wie oben. Stimmt das?",
    start_confirm: "Stimmt",
    start_entry: |semester, fs| format!("{semester} · jetzt {fs}. FS"),
    to_plan: "Regelstudienplan",
    to_areas: "Wahlpflicht & Bereiche",
    to_timetable: "Zum Stundenplan",
    legend: "So liest du den Plan",
    legend_overdue: "Nachholen: aus einem früheren Semester, noch nicht bestanden. Steht immer zuerst.",
    legend_moved: "Verschoben oder vorgezogen: von dir mit ↑ ↓ an einen anderen Platz gelegt.",
    legend_turnus: "Nur im WiSe / SoSe: das Modul wird nur in einer Hälfte des Jahres angeboten.",
    legend_done: "Abgehakt heißt bestanden. Was offen bleibt, rückt von selbst nach.",
    storage_hint: "Was du abhakst und planst, bleibt in diesem Browser.",

    mine: "Mein Studiengang",
    fachsemester_now: |fs, semester| format!("{fs}. Fachsemester · {semester}"),
    starts_in: |semester| format!("Studienbeginn {semester}"),
    progress: |done, total| format!("{done} von {total} LP"),
    progress_alone: |done| format!("{done} LP bestanden"),
    progress_label: "Bestandene Leistungspunkte",
    regular_end: |semester| format!("Regelstudienzeit bis {semester}"),
    expected_end: |semester| format!("voraussichtlich fertig: {semester}"),
    all_done: "Alles abgehakt",
    no_plan: "Für diesen Studiengang gibt es keinen geprüften Regelstudienplan. Hier steht, was du selbst einplanst und bestanden hast.",

    past: "Bisher",
    past_span: |from, to| if from == to { format!("{from}. Fachsemester") } else { format!("{from}.–{to}. Fachsemester") },
    past_count: |done, all| format!("{done} von {all} abgehakt"),
    past_prompt: "Hake ab, was du bestanden hast. Was offen bleibt, steht in den nächsten Semestern als Erstes.",
    all_passed: "Alles bestanden",

    fs: |n| format!("{n}. FS"),
    now: "jetzt",
    beyond: "nach der Regelstudienzeit",
    credits: |n, partial| if partial { format!("≥\u{a0}{n}\u{a0}LP") } else { format!("{n}\u{a0}LP") },
    planned: |n| format!("Plan: {n}\u{a0}LP"),
    heavy: "Mehr als der Regelstudienplan in dieses Semester legt",
    empty: "Nichts geplant",

    passed_label: |name| format!("{name} bestanden"),
    done_label: |name| format!("{name} erledigt"),
    overdue: |fs| format!("nachholen · aus {fs}"),
    moved_later: |fs| format!("verschoben · laut Plan {fs}"),
    moved_earlier: |fs| format!("vorgezogen · laut Plan {fs}"),
    only_winter: "nur im WiSe",
    only_summer: "nur im SoSe",
    own: "eigenes Modul",
    passed_in: |semester| format!("bestanden im {semester}"),
    in_timetable: "im Stundenplan",
    find_modules: "Module finden",
    earlier: |semester| format!("Früher: {semester}"),
    later: |semester| format!("Später: {semester}"),
    reset: "Zurück an seinen Platz im Plan",
    remove: "Aus dem Plan nehmen",
    again: |semester| format!("Nachholen im {semester}"),

    take: |n| format!("In den Stundenplan ({n})"),
    taken_all: "Alles im Stundenplan",
    taken: |parts| format!("Übernommen: {parts}"),
};

pub const EN: Texts = Texts {
    title: "My studies",
    server_title: "Your studies, semester by semester",
    server_hint: "This is where you plan your studies: what you have passed, what you have to catch up on and what comes next. It needs JavaScript, and everything stays in your browser.",
    seo_description: "Your studies at BTU, semester by semester: what is passed, what is left to catch up on and what comes next. Unofficial, in your browser only.",

    welcome_title: "Plan your studies",
    welcome_lead: "Choose your degree programme. Betula lays its standard study plan over your semesters and keeps it current: what you have not passed comes first again in the next semester that fits.",
    choose_program: "Choose degree programme",
    search_program: "Search degree programmes",
    all_programs: "See all degree programmes",
    gone: |name| format!("\u{201c}{name}\u{201d} is no longer in the catalogue. Choose your degree programme again."),

    program: "Degree programme",
    direction: "Specialisation",
    start: "Start of studies",
    start_assumed: "Assumed: you started in the semester above. Is that right?",
    start_confirm: "Yes",
    start_entry: |semester, fs| format!("{semester} · now semester {fs}"),
    to_plan: "Standard study plan",
    to_areas: "Electives & areas",
    to_timetable: "To the timetable",
    legend: "How to read the plan",
    legend_overdue: "Catch up: from an earlier semester, not passed yet. Always comes first.",
    legend_moved: "Moved later or earlier: put somewhere else by you with ↑ ↓.",
    legend_turnus: "Winter / summer only: the module is offered in one half of the year only.",
    legend_done: "Ticked off means passed. What stays open moves on by itself.",
    storage_hint: "What you tick off and plan stays in this browser.",

    mine: "My programme",
    fachsemester_now: |fs, semester| format!("Semester {fs} · {semester}"),
    starts_in: |semester| format!("Starts {semester}"),
    progress: |done, total| format!("{done} of {total} CP"),
    progress_alone: |done| format!("{done} CP passed"),
    progress_label: "Credit points passed",
    regular_end: |semester| format!("Standard period until {semester}"),
    expected_end: |semester| format!("expected to finish: {semester}"),
    all_done: "All ticked off",
    no_plan: "There is no checked standard study plan for this degree programme. This shows what you plan and pass yourself.",

    past: "So far",
    past_span: |from, to| if from == to { format!("semester {from}") } else { format!("semesters {from}–{to}") },
    past_count: |done, all| format!("{done} of {all} ticked off"),
    past_prompt: "Tick off what you have passed. What stays open comes first in the next semesters.",
    all_passed: "All passed",

    fs: |n| format!("Semester {n}"),
    now: "now",
    beyond: "after the standard period",
    credits: |n, partial| if partial { format!("≥\u{a0}{n}\u{a0}CP") } else { format!("{n}\u{a0}CP") },
    planned: |n| format!("Plan: {n}\u{a0}CP"),
    heavy: "More than the standard study plan puts into this semester",
    empty: "Nothing planned",

    passed_label: |name| format!("{name} passed"),
    done_label: |name| format!("{name} done"),
    overdue: |fs| format!("catch up · from {fs}"),
    moved_later: |fs| format!("moved · plan says {fs}"),
    moved_earlier: |fs| format!("brought forward · plan says {fs}"),
    only_winter: "winter only",
    only_summer: "summer only",
    own: "your own module",
    passed_in: |semester| format!("passed in {semester}"),
    in_timetable: "in the timetable",
    find_modules: "Find modules",
    earlier: |semester| format!("Earlier: {semester}"),
    later: |semester| format!("Later: {semester}"),
    reset: "Back to its place in the plan",
    remove: "Take out of the plan",
    again: |semester| format!("Catch up in {semester}"),

    take: |n| format!("Into the timetable ({n})"),
    taken_all: "All in the timetable",
    taken: |parts| format!("Imported: {parts}"),
};
