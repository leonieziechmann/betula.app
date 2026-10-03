//! Texts the catalog's page loaders and study plans write (`pages.rs`, `plan.rs`, `studyplan.rs`,
//! `variants.rs`, `graph.rs`).

pub struct Texts {
    /// The word between the last two of several alternatives: „„A“, „B“ oder „C“", „Übung Do
    /// 14:30 oder Do 16:30".
    pub or: &'static str,
    /// A name in the quotation marks of the language: „„Physik“".
    pub quoted: fn(&str) -> String,

    // The line under a module's week beside the Stundenplan (`pages::overlay`).
    /// „Überschneidet sich mit: Analysis I (Di 09:15), Mathematik IT-1 (Di 13:45)": the planned
    /// modules the module meets, already listed.
    pub clashes_with: fn(&str) -> String,
    /// „Passt mit Übung Do 13:45": the free options of the module's open choices, already listed.
    pub fits_with: fn(&str) -> String,
    /// „Passt in deinen Stundenplan (WiSe 2026/27)": nothing meets; the semester's name.
    pub fits_plan: fn(&str) -> String,
    /// „Termin": what an event is called in that line when neither QIS's type nor a kind names it.
    pub session: &'static str,

    // The line under a module's exams beside the Stundenplan (`pages::overlay`).
    /// „Prüfung gleichzeitig mit Mathematik IT-1 (10.03.2027 11:00)": the other module and when.
    pub exam_overlap: fn(&str, &str) -> String,
    /// „45 min bis Senftenberg zu Kraftwerkstechnik I": the module's exam is the earlier one; the
    /// minutes between, where the other is held, the other module.
    pub exam_tight_before: fn(u16, &str, &str) -> String,
    /// „45 min bis Senftenberg nach Kraftwerkstechnik I": the module's exam is the later one.
    pub exam_tight_after: fn(u16, &str, &str) -> String,
    /// What avoids a soft warning, after „ · ": „Erstermin 25.02. passt", „Zweittermin 11.03.
    /// passt" (the module's own sitting on that day) …
    pub first_sitting_fits: fn(&str) -> String,
    pub second_sitting_fits: fn(&str) -> String,
    /// … „Mathematik IT-1 am 25.02. passt" (the other module's sitting) …
    pub other_sitting_fits: fn(&str, &str) -> String,
    /// … or „andere Termine passen" (only a change of both).
    pub other_dates_fit: &'static str,

    // A placeholder of the Stundenplan as the catalog's note of a semester says it
    // (`studyplan::placeholder_line`).
    /// „6 LP", „≥ 6 LP", „10–24 LP": the amount is written already, with a non-breaking space.
    pub credits: fn(&str) -> String,
    /// „aus dem Bereich" / „aus den Bereichen": in place of a name that only repeats its areas.
    pub from_area: &'static str,
    pub from_areas: &'static str,
    /// „Fachübergreifendes Studium": a row whose modules are the FÜS list.
    pub fues: &'static str,
    /// „unter diesem Namen nicht im Katalog": a row that names one module the catalog does not
    /// know under that name.
    pub not_in_catalog: &'static str,
    /// „alle Wahlpflichtmodule": a choice whose name points at no area.
    pub all_electives: &'static str,

    // The plans of a program (`variants.rs`).
    /// „Regelstudienplan": the chip of a plan the regulation prints without a caption.
    pub unnamed_plan: &'static str,
    /// What the link from a row of a plan into the catalog says: „FÜS-Liste des Studiengangs" …
    pub fues_list: &'static str,
    /// … „Wahlpflichtmodule des Studiengangs" …
    pub program_electives: &'static str,
    /// … „5 Bereiche".
    pub areas: fn(usize) -> String,

    // The map of the programs (`graph.rs`), written where the app draws it: the map itself is
    // the same for every language.
    /// „Fakultät 1".
    pub faculty: fn(&str) -> String,
    /// „Fakultät 1 · MINT".
    pub faculty_named: fn(&str, &str) -> String,
}

pub const DE: Texts = Texts {
    or: "oder",
    quoted: |name| format!("„{name}“"),
    clashes_with: |modules| format!("Überschneidet sich mit: {modules}"),
    fits_with: |options| format!("Passt mit {options}"),
    fits_plan: |semester| format!("Passt in deinen Stundenplan ({semester})"),
    session: "Termin",
    exam_overlap: |module, when| format!("Prüfung gleichzeitig mit {module} ({when})"),
    exam_tight_before: |gap, place, module| format!("{gap} min bis {place} zu {module}"),
    exam_tight_after: |gap, place, module| format!("{gap} min bis {place} nach {module}"),
    first_sitting_fits: |day| format!("Erstermin {day} passt"),
    second_sitting_fits: |day| format!("Zweittermin {day} passt"),
    other_sitting_fits: |module, day| format!("{module} am {day} passt"),
    other_dates_fit: "andere Termine passen",
    credits: |amount| format!("{amount}\u{a0}LP"),
    from_area: "aus dem Bereich",
    from_areas: "aus den Bereichen",
    fues: "Fachübergreifendes Studium",
    not_in_catalog: "unter diesem Namen nicht im Katalog",
    all_electives: "alle Wahlpflichtmodule",
    unnamed_plan: "Regelstudienplan",
    fues_list: "FÜS-Liste des Studiengangs",
    program_electives: "Wahlpflichtmodule des Studiengangs",
    areas: |n| format!("{n} Bereiche"),
    faculty: |code| format!("Fakultät {code}"),
    faculty_named: |code, name| format!("Fakultät {code} · {name}"),
};

pub const EN: Texts = Texts {
    or: "or",
    quoted: |name| format!("“{name}”"),
    clashes_with: |modules| format!("Clashes with: {modules}"),
    fits_with: |options| format!("Fits with {options}"),
    fits_plan: |semester| format!("Fits your timetable ({semester})"),
    session: "Session",
    exam_overlap: |module, when| format!("Exam at the same time as {module} ({when})"),
    exam_tight_before: |gap, place, module| format!("{gap} min to {place} for {module}"),
    exam_tight_after: |gap, place, module| format!("{gap} min to {place} after {module}"),
    first_sitting_fits: |day| format!("first sitting on {day} fits"),
    second_sitting_fits: |day| format!("second sitting on {day} fits"),
    other_sitting_fits: |module, day| format!("{module} on {day} fits"),
    other_dates_fit: "other dates fit",
    credits: |amount| format!("{amount}\u{a0}CP"),
    from_area: "from the area",
    from_areas: "from the areas",
    fues: "Interdisciplinary studies",
    not_in_catalog: "not in the catalogue under this name",
    all_electives: "all compulsory elective modules",
    unnamed_plan: "Standard study plan",
    fues_list: "FÜS list of the degree programme",
    program_electives: "Compulsory elective modules of the degree programme",
    areas: |n| if n == 1 { "1 area".to_string() } else { format!("{n} areas") },
    faculty: |code| format!("Faculty {code}"),
    faculty_named: |code, name| format!("Faculty {code} · {name}"),
};
