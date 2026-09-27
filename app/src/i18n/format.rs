//! Texts of how values read on a page (`format.rs`): credits, counts, semesters, the turnus.

pub struct Texts {
    /// The form of study in a word or two, where a program is one line.
    pub dual_practice: &'static str,
    pub dual_training: &'static str,
    pub extended: &'static str,
    pub reduced: &'static str,
    /// „6 LP"; the number is written already.
    pub credits: fn(&str) -> String,
    pub credits_unknown: &'static str,
    /// „1 Modul", „17 Module"; the number is written already (thousands grouped).
    pub modules: fn(i64, &str) -> String,
    /// Where a validated study plan places a module: „1.", „5.–6." for one semester or a span …
    pub semester_one: fn(i64) -> String,
    pub semester_span: fn(i64, i64) -> String,
    /// … and the whole line: „5.–6. Semester", „4. oder 5. Semester" (the alternatives but the
    /// last joined with „, ").
    pub semesters: fn(&str) -> String,
    pub semesters_or: fn(&str, &str) -> String,
    /// The halves of the year in the turnus: „WiSe", „SoSe".
    pub winter_short: &'static str,
    pub summer_short: &'static str,
    pub turnus_unknown: &'static str,
    /// The short names of the exam forms, for table columns.
    pub prereq_map: &'static str,
    pub prereq_mca: &'static str,
    pub other_exam: &'static str,
}

pub const DE: Texts = Texts {
    dual_practice: "dual, Praxis",
    dual_training: "dual, Ausbildung",
    extended: "erweitert",
    reduced: "verkürzt",
    credits: |n| format!("{n} LP"),
    credits_unknown: "LP nicht angegeben",
    modules: |n, written| if n == 1 { "1 Modul".to_string() } else { format!("{written} Module") },
    semester_one: |n| format!("{n}."),
    semester_span: |from, to| format!("{from}.–{to}."),
    semesters: |named| format!("{named} Semester"),
    semesters_or: |rest, last| format!("{rest} oder {last} Semester"),
    winter_short: "WiSe",
    summer_short: "SoSe",
    turnus_unknown: "Turnus nicht angegeben",
    prereq_map: "Vorleistung + MAP",
    prereq_mca: "Vorleistung + MCA",
    other_exam: "andere Form",
};

pub const EN: Texts = Texts {
    dual_practice: "dual, practice",
    dual_training: "dual, training",
    extended: "extended",
    reduced: "shortened",
    credits: |n| format!("{n} CP"),
    credits_unknown: "CP not stated",
    modules: |n, written| if n == 1 { "1 module".to_string() } else { format!("{written} modules") },
    semester_one: ordinal,
    semester_span: |from, to| format!("{}–{}", ordinal(from), ordinal(to)),
    semesters: |named| format!("{named} semester"),
    semesters_or: |rest, last| format!("{rest} or {last} semester"),
    winter_short: "Winter",
    summer_short: "Summer",
    turnus_unknown: "Offering not stated",
    prereq_map: "Prerequisite + MAP",
    prereq_mca: "Prerequisite + MCA",
    other_exam: "other form",
};

/// 1 → "1st", 2 → "2nd", 11 → "11th", 23 → "23rd".
pub fn ordinal(n: i64) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}
