//! Texts of what „Ordnungen & Dokumente" of „Mein Studium" shows of the BTU as a whole
//! (`study/btu.rs`): its regulations for every program, and its pages around the study. The
//! regulations are named by their topic (`folia_model::labels::GeneralTopic`), as the BTU's own
//! pages name them, in each language its pages have.

pub struct Texts {
    // „Ordnungen der BTU".
    /// The panel's title, and what stands under it: for whom, what counts, and how old the list is,
    /// the day Radix read the BTU's page (day, month 1–12, year).
    pub statutes: &'static str,
    pub statutes_lead: &'static str,
    pub as_of: fn(u32, u32, i32) -> String,
    /// What the row of a regulation the data does not have says; it leads to the BTU's list.
    pub missing: &'static str,
    /// An issue of the Amtsblatt: „ABl. 13/2016".
    pub issue: fn(&str) -> String,
    /// The reading version in an amendment: „Lesefassung, Stand der 5. Änderungssatzung".
    pub reading: fn(usize) -> String,
    /// The BTU's English translation, and that it does not count.
    pub english: &'static str,
    pub not_binding: &'static str,
    /// The amendments, folded: „Alle 6 Änderungssatzungen"; one of them: „6. Änderungssatzung".
    pub amendments: fn(usize) -> String,
    pub amendment: fn(usize) -> String,
    /// Where the law of the state the BTU's regulations rest on is published.
    pub law_line: &'static str,
    /// „Bei der BTU": the BTU's own lists, each as a page of it.
    pub at_btu: &'static str,
    pub all_statutes: &'static str,
    pub all_program_statutes: &'static str,
    pub forms: &'static str,
    pub btu_site: &'static str,

    // „Rund ums Studium": the BTU's pages, by what they are about.
    pub around: &'static str,
    pub around_lead: &'static str,
    pub exams: &'static str,
    pub exams_register: &'static str,
    pub exams_deadlines: &'static str,
    pub exams_illness: &'static str,
    pub exams_work: &'static str,
    pub semester: &'static str,
    pub dates: &'static str,
    pub reregister: &'static str,
    pub fee: &'static str,
    pub leave: &'static str,
    pub change: &'static str,
    pub exmatriculation: &'static str,
    pub card: &'static str,
    pub help: &'static str,
    pub service: &'static str,
    pub advice: &'static str,
    pub portals: &'static str,
}

pub const DE: Texts = Texts {
    statutes: "Ordnungen der BTU",
    statutes_lead: "Für alle Studiengänge; welche für dich gilt, sagt deine Prüfungsordnung. Verbindlich ist, was das Amtsblatt der BTU veröffentlicht.",
    as_of: |day, month, year| format!("Stand der Liste: {day}. {} {year}.", month_name(&MONTHS_DE, month)),
    missing: "Gerade nicht im Datenstand · bei der BTU",
    issue: |issue| format!("ABl. {issue}"),
    reading: |n| format!("Lesefassung, Stand der {n}. Änderungssatzung"),
    english: "Englische Übersetzung",
    not_binding: "nicht verbindlich",
    amendments: |n| if n == 1 { "Die Änderungssatzung".to_string() } else { format!("Alle {n} Änderungssatzungen") },
    amendment: |n| format!("{n}. Änderungssatzung"),
    law_line: "Landesrecht, bei BRAVORS",
    at_btu: "Bei der BTU",
    all_statutes: "Alle Grundsatzordnungen",
    all_program_statutes: "Prüfungs- und Studienordnungen aller Studiengänge",
    forms: "Formulare und Anträge",
    btu_site: "b-tu.de",

    around: "Rund ums Studium",
    around_lead: "Die Seiten der BTU dazu.",
    exams: "Prüfungen",
    exams_register: "An- und Abmeldung von Modulprüfungen",
    exams_deadlines: "Prüfungs- und Studienfristen",
    exams_illness: "Krankheit und Verhinderung",
    exams_work: "Prüfungsleistungen",
    semester: "Semester und Formalitäten",
    dates: "Termine und Fristen",
    reregister: "Rückmeldung",
    fee: "Semestergebühr",
    leave: "Beurlaubung",
    change: "Studiengangwechsel",
    exmatriculation: "Exmatrikulation",
    card: "Studierendenausweis",
    help: "Service und Beratung",
    service: "Studierendenservice",
    advice: "Beratung und Hilfe",
    portals: "Portale",
};

pub const EN: Texts = Texts {
    statutes: "BTU regulations",
    statutes_lead: "For every programme; your examination regulations say which apply to you. What the BTU's Official Gazette publishes is binding.",
    as_of: |day, month, year| format!("List as of {day} {} {year}.", month_name(&MONTHS_EN, month)),
    missing: "Not in the data right now · at the BTU",
    issue: |issue| format!("Gazette {issue}"),
    reading: |n| format!("Reading version as of the {}", ordinal_amendment(n)),
    english: "English translation",
    not_binding: "not legally binding",
    amendments: |n| if n == 1 { "The amendment".to_string() } else { format!("All {n} amendments") },
    amendment: |n| {
        let word = ordinal_amendment(n);
        let mut chars = word.chars();
        chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or(word)
    },
    law_line: "State law, on BRAVORS",
    at_btu: "At the BTU",
    all_statutes: "All basic regulations",
    all_program_statutes: "Examination and study regulations of every programme",
    forms: "Forms and applications",
    btu_site: "b-tu.de",

    around: "Around your studies",
    around_lead: "The BTU's pages on it.",
    exams: "Examinations",
    exams_register: "Registering for examinations",
    exams_deadlines: "Examination deadlines and study time limits",
    exams_illness: "Illness and incapability",
    exams_work: "Academic work",
    semester: "Semester and formalities",
    dates: "Dates and deadlines",
    reregister: "Registering for the following semester",
    fee: "Semester fee",
    leave: "Leave of absence",
    change: "Change of degree programme",
    exmatriculation: "Removal from the register of students",
    card: "Identification card",
    help: "Services and advice",
    service: "Student services",
    advice: "Advice and support",
    portals: "Portals",
};

const MONTHS_DE: [&str; 12] = ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"];
const MONTHS_EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// The month (1–12) by its name; nothing for what is no month.
fn month_name(months: &[&'static str; 12], month: u32) -> &'static str {
    usize::try_from(month).ok().and_then(|month| month.checked_sub(1)).and_then(|at| months.get(at)).copied().unwrap_or_default()
}

/// „fifth amendment": an amendment by its number, as the BTU's English translations write it.
fn ordinal_amendment(n: usize) -> String {
    let word = match n {
        1 => "first",
        2 => "second",
        3 => "third",
        4 => "fourth",
        5 => "fifth",
        6 => "sixth",
        7 => "seventh",
        8 => "eighth",
        9 => "ninth",
        _ => return format!("amendment no. {n}"),
    };
    format!("{word} amendment")
}
