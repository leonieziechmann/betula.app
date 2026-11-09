//! Texts of the workbench of „Mein Studium" (`study/bench.rs`): one semester planned by the areas
//! of the plan and their subgroups.

pub struct Texts {
    // The bar of the mode.
    /// „Planen": what the bar is.
    pub mode: &'static str,
    /// „Fertig", and what it does as its tooltip.
    pub done: &'static str,
    pub done_hint: &'static str,
    /// „3. Fachsemester · jetzt" under the semester; the Fachsemester's words, whether it is now.
    pub semester_sub: fn(&str, bool) -> String,
    /// „30 von 30 LP": what the semester holds against what the plan puts into it; „30 LP" where
    /// the plan says nothing.
    pub of_planned: fn(&str, &str) -> String,
    pub credits: fn(&str) -> String,
    /// „+6": what it holds beyond the plan.
    pub over: fn(&str) -> String,
    /// The semester before and after, for screen readers.
    pub previous: &'static str,
    pub next: &'static str,

    // The areas.
    /// „Bereiche", „des Plans".
    pub areas: &'static str,
    pub areas_sub: &'static str,
    /// „Laut Plan im 3. FS"; „Laut Plan" for a semester without a Fachsemester.
    pub by_plan: fn(&str) -> String,
    pub by_plan_plain: &'static str,
    /// „Gemerkt", „Im Katalog suchen".
    pub marked: &'static str,
    pub in_catalog: &'static str,
    /// „Pflichtmodule": the Pflicht of an area.
    pub pflicht: &'static str,
    /// „2 offen": the Pflicht's rows that stand nowhere yet.
    pub open_n: fn(usize) -> String,
    /// „Erledigt", for screen readers beside a check.
    pub complete: &'static str,

    // „Laut Plan im 3. FS".
    /// „Laut Regelstudienplan im 3. FS", „Laut Regelstudienplan".
    pub plan_title: fn(&str) -> String,
    pub plan_title_plain: &'static str,
    pub plan_rule: &'static str,
    /// „Plan übernehmen · 3 Pflichtmodule", the line beside it, and what stands there once the
    /// Pflicht of the Fachsemester stands in the semester („3. FS").
    pub take_over: fn(usize) -> String,
    pub take_over_hint: &'static str,
    pub taken_over: fn(&str) -> String,
    /// The note: „3 Pflichtmodule in WiSe 2026/27 eingeplant".
    pub took_over: fn(usize, &str) -> String,
    /// The groups of the list: „Pflicht", „Wahlpflicht", „Vorgemerkt", „Noch offen aus früheren
    /// Semestern", „Aus dem 4. FS vorziehen".
    pub g_pflicht: &'static str,
    pub g_choices: &'static str,
    pub g_reserved: &'static str,
    pub g_earlier: &'static str,
    pub g_next: fn(&str) -> String,
    /// A choice's button: „Wählen", and „Ändern" once it is complete.
    pub choose: &'static str,
    pub change: &'static str,
    /// What a line says of a row: „Pflicht · Informatik", „laut Plan 4. FS", „bestanden · 1. FS",
    /// „eingeplant: SoSe 27", „nicht bestanden im WiSe 25/26".
    pub of_area: fn(&str) -> String,
    pub plan_fs: fn(&str) -> String,
    pub passed_fs: fn(&str) -> String,
    pub planned_in: fn(&str) -> String,
    pub failed_in: fn(&str) -> String,
    /// A choice's line: „6 von 12 LP · laut Plan 3.–4. FS", what is taken of it, its span.
    pub choice_line: fn(&str, &str, &str) -> String,
    /// A placeholder of it standing in the semester: „vorgemerkt, noch ohne Modul".
    pub reserved_line: &'static str,

    // A subgroup.
    /// What the plan asks of a choice: „12 LP · laut Plan 3.–4. FS".
    pub rule_choice: fn(&str, &str) -> String,
    /// „aus der FÜS-Liste des Studiengangs", „aus einem von 5 Fächern".
    pub rule_fues: &'static str,
    pub rule_lists: fn(usize) -> String,
    pub rule_pflicht: &'static str,
    pub rule_marked: &'static str,
    /// „gemerkt": what marks a module of the Merkliste in a list, its collections after it.
    pub marked_mark: &'static str,
    /// „Sammlung", „Alle" of „Gemerkt".
    pub collection: &'static str,
    pub all_collections: &'static str,
    /// „Fach", „Alle 5 Fächer".
    pub fach: &'static str,
    pub all_lists: fn(usize) -> String,
    /// „Im WiSe angeboten", „Passt in meine Woche", the search.
    pub offered_in: fn(&str) -> String,
    pub fits_week: &'static str,
    pub search: &'static str,
    pub clear_search: &'static str,
    /// The groups of a list: „passen in deine Woche", „treffen deine Woche", „ohne feste Termine",
    /// „nicht im WiSe angeboten"; „im SoSe angeboten" where the semester has no dates yet.
    pub group_fit: &'static str,
    pub group_clash: &'static str,
    pub group_undated: &'static str,
    pub group_unoffered: fn(&str) -> String,
    pub group_offered: fn(&str) -> String,
    /// What a row says of the week: „passt", „trifft deine Woche", „noch keine Termine".
    pub fit_ok: &'static str,
    pub fit_clash: &'static str,
    pub fit_none: &'static str,
    /// The semester has no dates yet (its label).
    pub no_dates: fn(&str) -> String,
    pub nothing: &'static str,
    /// The search finds nothing: „Nichts gefunden."
    pub no_hits: &'static str,
    /// A choice whose lists the tree leaves empty.
    pub no_list: &'static str,
    /// „Einplanen", „eingeplant", „bestanden", and what a click does (module, semester).
    pub plan: &'static str,
    pub planned: &'static str,
    pub passed: &'static str,
    pub plan_aria: fn(&str, &str) -> String,
    pub unplan_aria: fn(&str, &str) -> String,
    /// „Mehr Filter:" and the FÜS's „Sprache, Prüfungsform, LP, Lehrform:" before „Im Katalog
    /// suchen".
    pub more_filters: &'static str,
    /// Under „Im Katalog suchen" at the end of a list on a phone.
    pub catalog_sub: &'static str,
    pub fues_filters: &'static str,
    /// „Vormerken": the subgroup put into the semester without a module, for later.
    pub reserve: &'static str,
    pub reserve_hint: &'static str,
    /// The notes: „Anwendungsfach in WiSe 2026/27 vorgemerkt", „Softwarepraktikum in WiSe 2026/27
    /// eingeplant", „… eingeplant · zählt für Anwendungsfach".
    pub reserved_note: fn(&str, &str) -> String,
    pub planned_note: fn(&str, &str) -> String,
    pub planned_for_note: fn(&str, &str) -> String,
    /// „zählt für Anwendungsfach": the subgroup a module counts for.
    pub counts_for: fn(&str) -> String,

    // The semester.
    /// „3. Fachsemester · was du belegst", „was du belegst".
    pub sem_sub: fn(&str) -> String,
    pub sem_sub_plain: &'static str,
    pub sem_empty: &'static str,
    /// „für Anwendungsfach", „Pflicht", „nur besuchen".
    pub for_group: fn(&str) -> String,
    pub pflicht_tag: &'static str,
    pub visit: &'static str,
    /// „Softwarepraktikum herausnehmen".
    pub remove: fn(&str) -> String,
    /// „6 LP mehr als im Plan".
    pub over_note: fn(&str) -> String,

    // The sheet of a phone.
    /// „Hinzufügen zu WiSe 2026/27": what it plans into; „Alle Bereiche": the page of the areas.
    pub sheet_title: fn(&str) -> String,
    /// „Hinzufügen zu" before the semester with ‹ › in the head of a desktop's workbench.
    pub add_to: &'static str,
    pub all_areas: &'static str,
    /// The way to a page of it: its root („Übersicht"), ‹ and what it is for a screen reader.
    pub overview: &'static str,
    pub back: &'static str,
    pub path: &'static str,
    /// „vorgemerkt" at a choice reserved in the semester.
    pub reserved_mark: &'static str,

    // The menu of a row.
    /// „Zählt für ›", its entries and the one that leaves it to the tree; the note („Analysis II
    /// zählt für Anwendungsfach").
    pub counts_menu: &'static str,
    pub counts_tree: &'static str,
    pub counts_tree_hint: &'static str,
    pub counted_note: fn(&str, &str) -> String,
    pub counted_tree_note: fn(&str) -> String,
    /// „Anderer Bereich …": the dialog that lets it count anywhere (owner, 2026-11-08: „jedes modul
    /// zu jedem möglichen bereich zuzuordnen … dass das man das nicht ausversehen macht"); the note
    /// of an exception.
    pub counts_other: &'static str,
    pub counted_exception_note: fn(&str, &str) -> String,
    /// The dialog: its title, what it is for, the way back to the study's own choice, the marks of
    /// a subgroup, the warning over its buttons, its buttons.
    pub count_title: &'static str,
    pub count_lead: &'static str,
    pub count_auto: &'static str,
    pub count_auto_hint: &'static str,
    pub count_fits: &'static str,
    pub count_exception: &'static str,
    pub count_now: &'static str,
    pub count_figure: fn(&str, &str) -> String,
    pub count_warn: fn(&str, &str) -> String,
    pub count_apply: &'static str,
    pub count_apply_exception: &'static str,

    // „Umfang …" of a placeholder (`size.rs`; owner, 2026-11-09: „wo man selbst die gewichtung
    // wählen kann, die man machen möchte. Also wie man das richtig darstellt").
    /// The menu's entry, the dialog's title and its question („Wie viele LP willst du hierfür im
    /// WiSe 2027/28 belegen?"), what the plan says („Laut Plan 10–24 LP"), what is in it already.
    pub size_menu: &'static str,
    pub size_title: &'static str,
    pub size_lead: fn(&str) -> String,
    pub size_plan: fn(&str) -> String,
    pub size_held: fn(&str) -> String,
    pub size_less: &'static str,
    pub size_more: &'static str,
    /// „Wie im Plan": what the row says again.
    pub size_reset: &'static str,
    /// The stepper's buttons and field.
    pub size_down: &'static str,
    pub size_up: &'static str,
    pub size_field: &'static str,
    /// Below what the row asks, and above it: said, not refused.
    pub size_below: fn(&str) -> String,
    pub size_above: fn(&str) -> String,
    /// The weighting of the group the row is part of: its heading, what it shows, the legend, a
    /// member's figure, the sum, what is still to share out or beyond, a member short of its least.
    pub weights_title: fn(&str) -> String,
    pub weights_lead: &'static str,
    pub weights_modules: &'static str,
    pub weights_kept: &'static str,
    pub weights_figure: fn(&str, &str) -> String,
    pub weights_sum: fn(&str, &str) -> String,
    pub weights_open: fn(&str) -> String,
    pub weights_over: fn(&str) -> String,
    pub weights_done: &'static str,
    pub weights_short: fn(&str) -> String,
    pub weights_extras: fn(&str) -> String,
    pub size_apply: &'static str,
    /// The notes: „Komplex Praktische Informatik: 14 LP im WiSe 2027/28", „… wie im Plan".
    pub size_note: fn(&str, &str, &str) -> String,
    pub size_note_plan: fn(&str, &str) -> String,
}

pub const DE: Texts = Texts {
    mode: "Planen",
    done: "Fertig",
    done_hint: "Planen beenden",
    semester_sub: |fs, now| if now { format!("{fs} · jetzt") } else { fs.to_string() },
    of_planned: |have, plan| format!("{have} von {plan} LP"),
    credits: |have| format!("{have} LP"),
    over: |n| format!("+{n}"),
    previous: "Semester davor",
    next: "Semester danach",

    areas: "Bereiche",
    areas_sub: "des Plans",
    by_plan: |fs| format!("Laut Plan im {fs}"),
    by_plan_plain: "Laut Plan",
    marked: "Gemerkt",
    in_catalog: "Im Katalog suchen",
    pflicht: "Pflichtmodule",
    open_n: |n| format!("{n} offen"),
    complete: "erledigt",

    plan_title: |fs| format!("Laut Regelstudienplan im {fs}"),
    plan_title_plain: "Laut Regelstudienplan",
    plan_rule: "Was der Plan für dieses Semester vorsieht, und was aus früheren noch offen ist.",
    take_over: |n| if n == 1 { "Plan übernehmen · 1 Pflichtmodul".to_string() } else { format!("Plan übernehmen · {n} Pflichtmodule") },
    take_over_hint: "plant sofort, mit „Rückgängig“",
    taken_over: |fs| format!("Die Pflicht des {fs} steht im Semester."),
    took_over: |n, semester| if n == 1 { format!("1 Pflichtmodul in {semester} eingeplant") } else { format!("{n} Pflichtmodule in {semester} eingeplant") },
    g_pflicht: "Pflicht",
    g_choices: "Wahlpflicht",
    g_reserved: "Vorgemerkt",
    g_earlier: "Noch offen aus früheren Semestern",
    g_next: |fs| format!("Aus dem {fs} vorziehen"),
    choose: "Wählen",
    change: "Ändern",
    of_area: |area| format!("Pflicht · {area}"),
    plan_fs: |fs| format!("laut Plan {fs}"),
    passed_fs: |fs| format!("bestanden · {fs}"),
    planned_in: |semester| format!("eingeplant: {semester}"),
    failed_in: |semester| format!("nicht bestanden im {semester}"),
    choice_line: |have, asked, span| format!("{have} von {asked} LP · {span}"),
    reserved_line: "vorgemerkt, noch ohne Modul",

    rule_choice: |credits, span| format!("{credits} LP · {span}"),
    rule_fues: "aus der FÜS-Liste des Studiengangs",
    rule_lists: |n| format!("aus einem von {n} Fächern"),
    rule_pflicht: "Alle Pflichtmodule des Bereichs, nach dem Fachsemester, in das der Plan sie legt.",
    rule_marked: "Was du gemerkt hast, mit der Untergruppe, für die es zählt.",
    marked_mark: "gemerkt",
    collection: "Sammlung",
    all_collections: "Alle",
    fach: "Fach",
    all_lists: |n| format!("Alle {n} Fächer"),
    offered_in: |season| format!("Im {season} angeboten"),
    fits_week: "Passt in meine Woche",
    search: "In der Liste suchen",
    clear_search: "Suche leeren",
    group_fit: "passen in deine Woche",
    group_clash: "treffen deine Woche",
    group_undated: "ohne feste Termine",
    group_unoffered: |season| format!("nicht im {season} angeboten"),
    group_offered: |season| format!("im {season} angeboten"),
    fit_ok: "passt",
    fit_clash: "trifft deine Woche",
    fit_none: "noch keine Termine",
    no_dates: |semester| format!("Für {semester} stehen noch keine Termine fest: ob etwas in deine Woche passt, sagt Betula, sobald sie da sind."),
    nothing: "Nichts, was alle Schalter erfüllt.",
    no_hits: "Nichts gefunden.",
    no_list: "Der Modulbaum nennt für diese Wahl keine Module. Im Katalog findest du sie.",
    plan: "Einplanen",
    planned: "eingeplant",
    passed: "bestanden",
    plan_aria: |title, semester| format!("{title} in {semester} einplanen"),
    unplan_aria: |title, semester| format!("{title} aus {semester} herausnehmen"),
    more_filters: "Mehr Filter:",
    catalog_sub: "mit allen Filtern des Katalogs",
    fues_filters: "Sprache, Prüfungsform, LP, Lehrform:",
    reserve: "Vormerken",
    reserve_hint: "ohne Modul, für später",
    reserved_note: |name, semester| format!("{name} in {semester} vorgemerkt"),
    planned_note: |title, semester| format!("{title} in {semester} eingeplant"),
    planned_for_note: |title, group| format!("{title} eingeplant · zählt für {group}"),
    counts_for: |group| format!("zählt für {group}"),

    sem_sub: |fs| format!("{fs} · was du belegst"),
    sem_sub_plain: "was du belegst",
    sem_empty: "Noch nichts belegt.",
    for_group: |group| format!("für {group}"),
    pflicht_tag: "Pflicht",
    visit: "nur besuchen",
    remove: |name| format!("{name} herausnehmen"),
    over_note: |n| format!("{n} LP mehr als im Plan"),

    sheet_title: |semester| format!("Hinzufügen zu {semester}"),
    add_to: "Hinzufügen zu",
    all_areas: "Alle Bereiche",
    overview: "Übersicht",
    back: "Zurück",
    path: "Wo du bist",
    reserved_mark: "vorgemerkt",

    counts_menu: "Zählt für",
    counts_tree: "Automatisch",
    counts_tree_hint: "wo es laut Plan am besten passt",
    counted_note: |title, group| format!("{title} zählt für {group}"),
    counted_tree_note: |title| format!("{title} zählt wieder automatisch"),
    counts_other: "Anderer Bereich …",
    counted_exception_note: |title, group| format!("{title} zählt als Ausnahme für {group}"),
    count_title: "Wofür zählt es?",
    count_lead: "Betula ordnet jedes Modul selbst dort zu, wo es laut Plan am besten passt. Hat dein Prüfungsausschuss eine Ausnahme gemacht, ordnest du es hier einem anderen Bereich zu.",
    count_auto: "Automatisch",
    count_auto_hint: "wo es laut Plan am besten passt",
    count_fits: "passt",
    count_exception: "Ausnahme",
    count_now: "jetzt",
    count_figure: |have, asked| format!("{have} / {asked} LP"),
    count_warn: |title, group| format!("{title} gehört laut Plan nicht zu „{group}“. Es zählt dort trotzdem und ist als Ausnahme markiert."),
    count_apply: "Zuordnen",
    count_apply_exception: "Als Ausnahme zuordnen",

    size_menu: "Umfang …",
    size_title: "Umfang",
    size_lead: |semester| format!("Wie viele LP willst du hierfür im {semester} belegen? Der Platzhalter verschwindet, sobald seine Module so viel haben."),
    size_plan: |credits| format!("Laut Plan {credits}\u{a0}LP"),
    size_held: |credits| format!("darin schon {credits}\u{a0}LP"),
    size_less: "Weniger",
    size_more: "Mehr",
    size_reset: "Wie im Plan",
    size_down: "1 LP weniger",
    size_up: "1 LP mehr",
    size_field: "Umfang in LP",
    size_below: |least| format!("Weniger, als der Plan verlangt (mindestens {least}\u{a0}LP). Was fehlt, bleibt im Bereich offen."),
    size_above: |most| format!("Mehr, als der Plan vorsieht (höchstens {most}\u{a0}LP). Was darüber liegt, zählt als über Bedarf."),
    weights_title: |label| format!("Gewichtung · {label}"),
    weights_lead: "Wie du die LP verteilst: was die Module jedes Bereichs haben und was seine Platzhalter noch vormerken.",
    weights_modules: "Module",
    weights_kept: "vorgemerkt",
    weights_figure: |total, asked| format!("{total} / {asked}\u{a0}LP"),
    weights_sum: |total, target| format!("Zusammen {total} von {target}\u{a0}LP"),
    weights_open: |n| format!("noch {n}\u{a0}LP zu verteilen"),
    weights_over: |n| format!("{n}\u{a0}LP über Bedarf"),
    weights_done: "verteilt",
    weights_short: |n| format!("noch {n}\u{a0}LP bis zum Minimum"),
    weights_extras: |n| format!("dazu {n}\u{a0}LP vorgemerkt, die einem der Bereiche zufallen"),
    size_apply: "Übernehmen",
    size_note: |name, credits, semester| format!("{name}: {credits}\u{a0}LP im {semester}"),
    size_note_plan: |name, semester| format!("{name} im {semester} wieder wie im Plan"),
};

pub const EN: Texts = Texts {
    mode: "Planning",
    done: "Done",
    done_hint: "Stop planning",
    semester_sub: |fs, now| if now { format!("{fs} · now") } else { fs.to_string() },
    of_planned: |have, plan| format!("{have} of {plan} ECTS"),
    credits: |have| format!("{have} ECTS"),
    over: |n| format!("+{n}"),
    previous: "Previous semester",
    next: "Next semester",

    areas: "Areas",
    areas_sub: "of the plan",
    by_plan: |fs| format!("By the plan in {fs}"),
    by_plan_plain: "By the plan",
    marked: "Saved",
    in_catalog: "Search the catalogue",
    pflicht: "Compulsory modules",
    open_n: |n| format!("{n} open"),
    complete: "done",

    plan_title: |fs| format!("By the curriculum in {fs}"),
    plan_title_plain: "By the curriculum",
    plan_rule: "What the plan puts into this semester, and what is still open from earlier ones.",
    take_over: |n| if n == 1 { "Take over the plan · 1 compulsory module".to_string() } else { format!("Take over the plan · {n} compulsory modules") },
    take_over_hint: "plans at once, with “Undo”",
    taken_over: |fs| format!("The compulsory modules of {fs} are in the semester."),
    took_over: |n, semester| if n == 1 { format!("1 compulsory module planned for {semester}") } else { format!("{n} compulsory modules planned for {semester}") },
    g_pflicht: "Compulsory",
    g_choices: "Electives",
    g_reserved: "Reserved",
    g_earlier: "Still open from earlier semesters",
    g_next: |fs| format!("Bring forward from {fs}"),
    choose: "Choose",
    change: "Change",
    of_area: |area| format!("Compulsory · {area}"),
    plan_fs: |fs| format!("plan: {fs}"),
    passed_fs: |fs| format!("passed · {fs}"),
    planned_in: |semester| format!("planned: {semester}"),
    failed_in: |semester| format!("not passed in {semester}"),
    choice_line: |have, asked, span| format!("{have} of {asked} ECTS · {span}"),
    reserved_line: "reserved, no module yet",

    rule_choice: |credits, span| format!("{credits} ECTS · {span}"),
    rule_fues: "from the programme's list of interdisciplinary modules",
    rule_lists: |n| format!("from one of {n} subjects"),
    rule_pflicht: "Every compulsory module of the area, by the semester the plan puts it into.",
    rule_marked: "What you saved, with the group it counts for.",
    marked_mark: "saved",
    collection: "Collection",
    all_collections: "All",
    fach: "Subject",
    all_lists: |n| format!("All {n} subjects"),
    offered_in: |season| format!("Offered in {season}"),
    fits_week: "Fits my week",
    search: "Search the list",
    clear_search: "Clear the search",
    group_fit: "fit your week",
    group_clash: "meet your week",
    group_undated: "no fixed dates",
    group_unoffered: |season| format!("not offered in {season}"),
    group_offered: |season| format!("offered in {season}"),
    fit_ok: "fits",
    fit_clash: "meets your week",
    fit_none: "no dates yet",
    no_dates: |semester| format!("{semester} has no dates yet: Betula says what fits your week once they are out."),
    nothing: "Nothing meets every switch.",
    no_hits: "Nothing found.",
    no_list: "The module tree names no modules for this choice. The catalogue has them.",
    plan: "Plan",
    planned: "planned",
    passed: "passed",
    plan_aria: |title, semester| format!("Plan {title} for {semester}"),
    unplan_aria: |title, semester| format!("Take {title} out of {semester}"),
    more_filters: "More filters:",
    catalog_sub: "with every filter of the catalogue",
    fues_filters: "Language, exam, ECTS, teaching:",
    reserve: "Reserve",
    reserve_hint: "without a module, for later",
    reserved_note: |name, semester| format!("{name} reserved in {semester}"),
    planned_note: |title, semester| format!("{title} planned for {semester}"),
    planned_for_note: |title, group| format!("{title} planned · counts for {group}"),
    counts_for: |group| format!("counts for {group}"),

    sem_sub: |fs| format!("{fs} · what you take"),
    sem_sub_plain: "what you take",
    sem_empty: "Nothing taken yet.",
    for_group: |group| format!("for {group}"),
    pflicht_tag: "Compulsory",
    visit: "visit only",
    remove: |name| format!("Take {name} out"),
    over_note: |n| format!("{n} ECTS more than the plan"),

    sheet_title: |semester| format!("Add to {semester}"),
    add_to: "Add to",
    all_areas: "All areas",
    overview: "Overview",
    back: "Back",
    path: "Where you are",
    reserved_mark: "reserved",

    counts_menu: "Counts for",
    counts_tree: "Automatically",
    counts_tree_hint: "where it fits the plan best",
    counted_note: |title, group| format!("{title} counts for {group}"),
    counted_tree_note: |title| format!("{title} counts automatically again"),
    counts_other: "Another area …",
    counted_exception_note: |title, group| format!("{title} counts for {group} as an exception"),
    count_title: "What does it count for?",
    count_lead: "Betula puts every module where it fits the plan best. If your examination board made an exception, let it count for another area here.",
    count_auto: "Automatically",
    count_auto_hint: "where it fits the plan best",
    count_fits: "fits",
    count_exception: "Exception",
    count_now: "now",
    count_figure: |have, asked| format!("{have} / {asked} ECTS"),
    count_warn: |title, group| format!("By the plan, {title} does not belong to “{group}”. It counts there all the same, marked as an exception."),
    count_apply: "Assign",
    count_apply_exception: "Assign as an exception",

    size_menu: "Size …",
    size_title: "Size",
    size_lead: |semester| format!("How many CP do you want to take for this in {semester}? The placeholder goes once its modules hold that much."),
    size_plan: |credits| format!("By the plan {credits}\u{a0}CP"),
    size_held: |credits| format!("{credits}\u{a0}CP in it already"),
    size_less: "Less",
    size_more: "More",
    size_reset: "As in the plan",
    size_down: "1 CP less",
    size_up: "1 CP more",
    size_field: "Size in CP",
    size_below: |least| format!("Less than the plan asks (at least {least}\u{a0}CP). What is missing stays open in the area."),
    size_above: |most| format!("More than the plan has room for (at most {most}\u{a0}CP). What lies beyond counts as beyond need."),
    weights_title: |label| format!("Weighting · {label}"),
    weights_lead: "How you share the CP out: what the modules of each area hold and what its placeholders still keep.",
    weights_modules: "Modules",
    weights_kept: "kept",
    weights_figure: |total, asked| format!("{total} / {asked}\u{a0}CP"),
    weights_sum: |total, target| format!("Together {total} of {target}\u{a0}CP"),
    weights_open: |n| format!("{n}\u{a0}CP still to share out"),
    weights_over: |n| format!("{n}\u{a0}CP beyond need"),
    weights_done: "shared out",
    weights_short: |n| format!("{n}\u{a0}CP short of the least"),
    weights_extras: |n| format!("and {n}\u{a0}CP kept that fall to one of the areas"),
    size_apply: "Apply",
    size_note: |name, credits, semester| format!("{name}: {credits}\u{a0}CP in {semester}"),
    size_note_plan: |name, semester| format!("{name} in {semester} as in the plan again"),
};
