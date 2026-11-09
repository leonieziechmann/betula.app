//! Texts of the workbench of „Mein Studium" (`study/bench.rs`): one semester planned by the areas
//! of the plan and their subgroups.

pub struct Texts {
    // The head of the workbench.
    /// × and Esc: what they do, as the tooltip of ×.
    pub done_hint: &'static str,
    /// „30 von 30 LP": what the semester holds against what the plan puts into it; „30 LP" where
    /// the plan says nothing.
    pub of_planned: fn(&str, &str) -> String,
    pub credits: fn(&str) -> String,

    // The areas.
    /// „Bereiche", for screen readers at the ways of the root.
    pub areas: &'static str,
    /// „Laut Plan im 3. FS"; „Laut Plan" for a semester without a Fachsemester.
    pub by_plan: fn(&str) -> String,
    pub by_plan_plain: &'static str,
    /// „Gemerkt", „Im Katalog suchen".
    pub marked: &'static str,
    pub in_catalog: &'static str,
    /// „Pflichtmodule": the Pflicht of an area.
    pub pflicht: &'static str,

    // „Laut Plan im 3. FS".
    /// „Plan übernehmen · 3 Pflichtmodule", the line beside it, and what stands there once the
    /// Pflicht of the Fachsemester stands in the semester („3. FS").
    pub take_over: fn(usize) -> String,
    pub take_over_hint: &'static str,
    pub taken_over: fn(&str) -> String,
    /// The note: „3 Pflichtmodule in WiSe 2026/27 eingeplant".
    pub took_over: fn(usize, &str) -> String,
    /// „Aus dem 4. FS vorziehen".
    pub g_next: fn(&str) -> String,
    /// What a line says of a row: „Pflicht · Informatik", „laut Plan 4. FS", „bestanden · 1. FS",
    /// „eingeplant: SoSe 27", „nicht bestanden im WiSe 25/26".
    pub of_area: fn(&str) -> String,
    pub plan_fs: fn(&str) -> String,
    pub passed_fs: fn(&str) -> String,
    pub planned_in: fn(&str) -> String,
    pub failed_in: fn(&str) -> String,
    /// A placeholder of it standing in the semester: „vorgemerkt, noch ohne Modul".
    pub reserved_line: &'static str,

    // A subgroup.
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
    /// „Einplanen", „bestanden", and what a click does (module, semester).
    pub plan: &'static str,
    pub passed: &'static str,
    pub plan_aria: fn(&str, &str) -> String,
    pub unplan_aria: fn(&str, &str) -> String,
    /// Under „Im Katalog suchen" at the end of a list.
    pub catalog_sub: &'static str,
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
    /// „für Anwendungsfach": what a row of the semester counts for, beside its area, while the
    /// workbench is open over it on a desktop.
    pub for_group: fn(&str) -> String,

    // The sheet of a phone, the right half of a desktop's semester.
    /// „Hinzufügen zu WiSe 2026/27": what it plans into; „Alle Bereiche": the page of the areas.
    pub sheet_title: fn(&str) -> String,
    /// „Hinzufügen": the top of the right half of a desktop's semester while the workbench is open
    /// in it (its head names the semester).
    pub half_title: &'static str,
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
}

pub const DE: Texts = Texts {
    done_hint: "Planen beenden",
    of_planned: |have, plan| format!("{have} von {plan} LP"),
    credits: |have| format!("{have} LP"),

    areas: "Bereiche",
    by_plan: |fs| format!("Laut Plan im {fs}"),
    by_plan_plain: "Laut Plan",
    marked: "Gemerkt",
    in_catalog: "Im Katalog suchen",
    pflicht: "Pflichtmodule",

    take_over: |n| if n == 1 { "Plan übernehmen · 1 Pflichtmodul".to_string() } else { format!("Plan übernehmen · {n} Pflichtmodule") },
    take_over_hint: "plant sofort, mit „Rückgängig“",
    taken_over: |fs| format!("Die Pflicht des {fs} steht im Semester."),
    took_over: |n, semester| if n == 1 { format!("1 Pflichtmodul in {semester} eingeplant") } else { format!("{n} Pflichtmodule in {semester} eingeplant") },
    g_next: |fs| format!("Aus dem {fs} vorziehen"),
    of_area: |area| format!("Pflicht · {area}"),
    plan_fs: |fs| format!("laut Plan {fs}"),
    passed_fs: |fs| format!("bestanden · {fs}"),
    planned_in: |semester| format!("eingeplant: {semester}"),
    failed_in: |semester| format!("nicht bestanden im {semester}"),
    reserved_line: "vorgemerkt, noch ohne Modul",

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
    passed: "bestanden",
    plan_aria: |title, semester| format!("{title} in {semester} einplanen"),
    unplan_aria: |title, semester| format!("{title} aus {semester} herausnehmen"),
    catalog_sub: "mit allen Filtern des Katalogs",
    reserve: "Vormerken",
    reserve_hint: "ohne Modul, für später",
    reserved_note: |name, semester| format!("{name} in {semester} vorgemerkt"),
    planned_note: |title, semester| format!("{title} in {semester} eingeplant"),
    planned_for_note: |title, group| format!("{title} eingeplant · zählt für {group}"),
    counts_for: |group| format!("zählt für {group}"),
    for_group: |group| format!("für {group}"),

    sheet_title: |semester| format!("Hinzufügen zu {semester}"),
    half_title: "Hinzufügen",
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
};

pub const EN: Texts = Texts {
    done_hint: "Stop planning",
    of_planned: |have, plan| format!("{have} of {plan} ECTS"),
    credits: |have| format!("{have} ECTS"),

    areas: "Areas",
    by_plan: |fs| format!("By the plan in {fs}"),
    by_plan_plain: "By the plan",
    marked: "Saved",
    in_catalog: "Search the catalogue",
    pflicht: "Compulsory modules",

    take_over: |n| if n == 1 { "Take over the plan · 1 compulsory module".to_string() } else { format!("Take over the plan · {n} compulsory modules") },
    take_over_hint: "plans at once, with “Undo”",
    taken_over: |fs| format!("The compulsory modules of {fs} are in the semester."),
    took_over: |n, semester| if n == 1 { format!("1 compulsory module planned for {semester}") } else { format!("{n} compulsory modules planned for {semester}") },
    g_next: |fs| format!("Bring forward from {fs}"),
    of_area: |area| format!("Compulsory · {area}"),
    plan_fs: |fs| format!("plan: {fs}"),
    passed_fs: |fs| format!("passed · {fs}"),
    planned_in: |semester| format!("planned: {semester}"),
    failed_in: |semester| format!("not passed in {semester}"),
    reserved_line: "reserved, no module yet",

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
    passed: "passed",
    plan_aria: |title, semester| format!("Plan {title} for {semester}"),
    unplan_aria: |title, semester| format!("Take {title} out of {semester}"),
    catalog_sub: "with every filter of the catalogue",
    reserve: "Reserve",
    reserve_hint: "without a module, for later",
    reserved_note: |name, semester| format!("{name} reserved in {semester}"),
    planned_note: |title, semester| format!("{title} planned for {semester}"),
    planned_for_note: |title, group| format!("{title} planned · counts for {group}"),
    counts_for: |group| format!("counts for {group}"),
    for_group: |group| format!("for {group}"),

    sheet_title: |semester| format!("Add to {semester}"),
    half_title: "Add",
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
};
