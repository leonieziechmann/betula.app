//! Every enum code of the contract and its label in each language of the site, in one place.
//!
//! The codes are defined by the CHECK constraints of the snapshot
//! (`radix/internal/catalogdb/migrations/`); their meaning by `radix/internal/normalize/`.
//! `tests::every_enum_code_has_a_label` reads the constraints from a real snapshot
//! and fails for any code that has no variant here.

use folia_locale::Locale;
use serde::{Deserialize, Serialize};

/// A code from the database: one this build has a label for, or one it has not seen yet.
///
/// A new code in the data must not break the page (a new kind of study program or event
/// has to work without a release), so it is carried along and shown as it is.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Code<E> {
    Known(E),
    Unlabelled(String),
}

pub trait Labelled: Sized + Copy + 'static {
    const ALL: &'static [Self];
    fn code(self) -> &'static str;
    fn label(self, locale: Locale) -> &'static str;

    fn from_code(code: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|v| v.code() == code)
    }
}

impl<E: Labelled> Code<E> {
    pub fn parse(code: &str) -> Self {
        match E::from_code(code) {
            Some(known) => Code::Known(known),
            None => Code::Unlabelled(code.to_string()),
        }
    }

    pub fn parse_opt(code: Option<String>) -> Option<Self> {
        code.map(|c| Self::parse(&c))
    }

    pub fn known(&self) -> Option<E> {
        match self {
            Code::Known(e) => Some(*e),
            Code::Unlabelled(_) => None,
        }
    }

    pub fn is(&self, value: E) -> bool
    where
        E: PartialEq,
    {
        self.known() == Some(value)
    }

    pub fn code(&self) -> &str {
        match self {
            Code::Known(e) => e.code(),
            Code::Unlabelled(code) => code,
        }
    }

    /// The label in `locale`; a code this build has none for, as it is.
    pub fn label(&self, locale: Locale) -> &str {
        match self {
            Code::Known(e) => e.label(locale),
            Code::Unlabelled(code) => code,
        }
    }
}

macro_rules! code_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $($variant:ident = $code:literal => { de: $de:literal, en: $en:literal }),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name { $($variant),+ }

        impl Labelled for $name {
            const ALL: &'static [Self] = &[$(Self::$variant),+];
            fn code(self) -> &'static str { match self { $(Self::$variant => $code),+ } }
            fn label(self, locale: Locale) -> &'static str {
                match locale {
                    Locale::De => match self { $(Self::$variant => $de),+ },
                    Locale::En => match self { $(Self::$variant => $en),+ },
                }
            }
        }
    };
}

code_enum! {
    /// In which semesters a module is offered.
    TurnusSeason {
        Winter = "winter" => { de: "Wintersemester", en: "Winter semester" },
        Summer = "summer" => { de: "Sommersemester", en: "Summer semester" },
        Both = "both" => { de: "jedes Semester", en: "every semester" },
        Irregular = "irregular" => { de: "unregelmäßig", en: "irregularly" },
    }
}

code_enum! {
    /// Offered only in even or odd years.
    TurnusParity {
        Even = "even" => { de: "gerade Jahre", en: "even years" },
        Odd = "odd" => { de: "ungerade Jahre", en: "odd years" },
    }
}

code_enum! {
    OfferStatus {
        Active = "active" => { de: "im Angebot", en: "offered" },
        PhaseOut = "phase_out" => { de: "Auslaufmodul", en: "Being phased out" },
        NotOffered = "not_offered" => { de: "wird nicht mehr angeboten", en: "no longer offered" },
    }
}

code_enum! {
    ExamForm {
        Map = "map" => { de: "Modulabschlussprüfung (MAP)", en: "Final module examination (MAP)" },
        PrereqMap = "prereq_map" => { de: "Prüfungsvorleistung + Modulabschlussprüfung (MAP)", en: "Exam prerequisite + final module examination (MAP)" },
        Mca = "mca" => { de: "Continuous Assessment (MCA)", en: "Continuous Assessment (MCA)" },
        PrereqMca = "prereq_mca" => { de: "Prüfungsvorleistung + Continuous Assessment (MCA)", en: "Exam prerequisite + Continuous Assessment (MCA)" },
        Other = "other" => { de: "andere Prüfungsform", en: "other form of assessment" },
    }
}

code_enum! {
    /// Teaching forms of modules and of events.
    TeachingForm {
        Lecture = "lecture" => { de: "Vorlesung", en: "Lecture" },
        Exercise = "exercise" => { de: "Übung", en: "Exercise" },
        Seminar = "seminar" => { de: "Seminar", en: "Seminar" },
        Practical = "practical" => { de: "Praktikum", en: "Practical" },
        Project = "project" => { de: "Projekt", en: "Project" },
        Tutorial = "tutorial" => { de: "Tutorium", en: "Tutorial" },
        Consultation = "consultation" => { de: "Konsultation", en: "Consultation" },
        Excursion = "excursion" => { de: "Exkursion", en: "Excursion" },
        SelfStudy = "self_study" => { de: "Selbststudium", en: "Self-study" },
        Paper = "paper" => { de: "Hausarbeit", en: "Term paper" },
        Other = "other" => { de: "andere Lehrform", en: "other teaching form" },
    }
}

code_enum! {
    /// What a module is within a program. NULL in the data means: no source says it.
    ModuleKind {
        Compulsory = "compulsory" => { de: "Pflicht", en: "Compulsory" },
        Elective = "elective" => { de: "Wahlpflicht", en: "Compulsory elective" },
        Thesis = "thesis" => { de: "Abschlussarbeit", en: "Thesis" },
        Internship = "internship" => { de: "Praktikum", en: "Internship" },
        Fues = "fues" => { de: "FÜS", en: "FÜS" },
    }
}

code_enum! {
    /// How a module belongs to a program.
    Relation {
        Curricular = "curricular" => { de: "Curriculum", en: "Curriculum" },
        Fues = "fues" => { de: "Fachübergreifendes Studium", en: "Interdisciplinary studies (FÜS)" },
    }
}

code_enum! {
    /// Which source states the kind of a module in a program.
    KindSource {
        PdfPlan = "pdf_plan" => { de: "Regelstudienplan der Prüfungsordnung", en: "Standard study plan of the examination regulations" },
        ModulePage = "module_page" => { de: "Modulbeschreibung", en: "Module description" },
        QisTree = "qis_tree" => { de: "Modulbaum im Vorlesungsverzeichnis", en: "Module tree in the course catalogue" },
    }
}

code_enum! {
    KindBasis {
        Stated = "stated" => { de: "ausdrücklich angegeben", en: "stated explicitly" },
        Inferred = "inferred" => { de: "aus der Gliederung abgeleitet", en: "inferred from the structure" },
    }
}

code_enum! {
    PrerequisiteKind {
        Mandatory = "mandatory" => { de: "zwingend", en: "mandatory" },
        Recommended = "recommended" => { de: "empfohlen", en: "recommended" },
    }
}

code_enum! {
    TextItemKind {
        Literature = "literature" => { de: "Literatur", en: "Literature" },
        Course = "course" => { de: "Lehrveranstaltungen", en: "Courses" },
    }
}

code_enum! {
    DegreeLevel {
        Bachelor = "bachelor" => { de: "Bachelor", en: "Bachelor" },
        Master = "master" => { de: "Master", en: "Master" },
        TeachingBachelor = "teaching_bachelor" => { de: "Lehramt Bachelor", en: "Teacher training Bachelor" },
        TeachingMaster = "teaching_master" => { de: "Lehramt Master", en: "Teacher training Master" },
        Doctoral = "doctoral" => { de: "Promotion", en: "Doctorate" },
        None = "none" => { de: "ohne Abschluss", en: "without a degree" },
        Other = "other" => { de: "anderer Abschluss", en: "other degree" },
    }
}

code_enum! {
    DegreeType {
        University = "university" => { de: "universitär", en: "university" },
        Applied = "applied" => { de: "anwendungsorientiert", en: "applied" },
    }
}

code_enum! {
    /// NULL in the data: the regular form of the program.
    StudyVariant {
        DualPractice = "dual_practice" => { de: "dual, praxisintegrierend", en: "dual, with integrated practice" },
        DualTraining = "dual_training" => { de: "dual, ausbildungsintegrierend", en: "dual, with integrated vocational training" },
        DoubleDegree = "double_degree" => { de: "Doppelabschluss", en: "Double degree" },
        Extended = "extended" => { de: "erweiterte Studienform", en: "extended form of study" },
        Reduced = "reduced" => { de: "verkürzte Studienform", en: "shortened form of study" },
        Distance = "distance" => { de: "Fernstudium", en: "Distance learning" },
        PartTime = "part_time" => { de: "Teilzeit", en: "Part-time" },
        Other = "other" => { de: "besondere Studienform", en: "special form of study" },
    }
}

code_enum! {
    DocumentType {
        Statute = "statute" => { de: "Prüfungs- und Studienordnung", en: "Examination and study regulations" },
        Amendment = "amendment" => { de: "Änderungssatzung", en: "Amending statute" },
        Other = "other" => { de: "Dokument", en: "Document" },
    }
}

code_enum! {
    StudySection {
        Basic = "basic" => { de: "Grundstudium", en: "Basic studies" },
        Main = "main" => { de: "Fachstudium", en: "Main studies" },
        Specialization = "specialization" => { de: "Vertiefungsstudium", en: "Specialisation studies" },
        Core = "core" => { de: "Kernstudium", en: "Core studies" },
    }
}

code_enum! {
    PlanStatus {
        Saved = "saved" => { de: "Regelstudienplan geprüft", en: "Standard study plan checked" },
        SavedWithWarnings = "saved_with_warnings" => { de: "Regelstudienplan geprüft, mit Hinweisen", en: "Standard study plan checked, with notes" },
        NeedsReview = "needs_review" => { de: "Regelstudienplan noch nicht geprüft", en: "Standard study plan not yet checked" },
        NoPlan = "no_plan" => { de: "Prüfungsordnung enthält keinen Regelstudienplan", en: "The examination regulations contain no standard study plan" },
        MissingSource = "missing_source" => { de: "Prüfungsordnung liegt nicht vor", en: "Examination regulations not available" },
    }
}

code_enum! {
    /// What a sum the regulation prints over rows of its plan covers.
    PlanTotalScope {
        Plan = "plan" => { de: "Summe des Studienplans", en: "Total of the study plan" },
        Section = "section" => { de: "Summe eines Abschnitts", en: "Total of a section" },
    }
}

code_enum! {
    /// Whether a program named on a module page is one of the catalog.
    ResolveStatus {
        Resolved = "resolved" => { de: "Studiengang im Katalog", en: "Degree programme in the catalogue" },
        Abroad = "abroad" => { de: "Abschluss im Ausland", en: "Degree abroad" },
        Unresolved = "unresolved" => { de: "Studiengang nicht im Katalog", en: "Degree programme not in the catalogue" },
    }
}

code_enum! {
    Season {
        Summer = "summer" => { de: "Sommersemester", en: "Summer semester" },
        Winter = "winter" => { de: "Wintersemester", en: "Winter semester" },
    }
}

code_enum! {
    EventCategory {
        Teaching = "teaching" => { de: "Lehrveranstaltung", en: "Course" },
        Exam = "exam" => { de: "Prüfung", en: "Exam" },
        Other = "other" => { de: "Sonstiges", en: "Other" },
    }
}

code_enum! {
    Rhythm {
        Weekly = "weekly" => { de: "wöchentlich", en: "weekly" },
        WeekA = "week_a" => { de: "A-Woche", en: "week A" },
        WeekB = "week_b" => { de: "B-Woche", en: "week B" },
        Single = "single" => { de: "Einzeltermin", en: "single date" },
        Block = "block" => { de: "Blockveranstaltung", en: "block course" },
        Other = "other" => { de: "nach Absprache", en: "by arrangement" },
    }
}

code_enum! {
    Campus {
        Zentralcampus = "zentralcampus" => { de: "Zentralcampus Cottbus", en: "Central Campus Cottbus" },
        Sachsendorf = "sachsendorf" => { de: "Cottbus-Sachsendorf", en: "Cottbus-Sachsendorf" },
        Senftenberg = "senftenberg" => { de: "Senftenberg", en: "Senftenberg" },
        Nord = "nord" => { de: "Cottbus Nord", en: "Cottbus North" },
    }
}

code_enum! {
    LecturerRole {
        Responsible = "responsible" => { de: "Modulverantwortung", en: "Module coordination" },
        Instructor = "instructor" => { de: "Lehrende", en: "Lecturers" },
    }
}

/// Weekday 1 = Monday … 7 = Sunday, as in the event tables.
pub fn weekday_label(weekday: i64, locale: Locale) -> Option<&'static str> {
    locale.texts().weekday(weekday)
}

/// The codes of every enum above, by enum name (for the label test).
pub fn code_sets() -> Vec<(&'static str, Vec<&'static str>)> {
    fn codes<E: Labelled>() -> Vec<&'static str> {
        E::ALL.iter().map(|v| v.code()).collect()
    }
    vec![
        ("TurnusSeason", codes::<TurnusSeason>()),
        ("TurnusParity", codes::<TurnusParity>()),
        ("OfferStatus", codes::<OfferStatus>()),
        ("ExamForm", codes::<ExamForm>()),
        ("TeachingForm", codes::<TeachingForm>()),
        ("ModuleKind", codes::<ModuleKind>()),
        ("Relation", codes::<Relation>()),
        ("KindSource", codes::<KindSource>()),
        ("KindBasis", codes::<KindBasis>()),
        ("PrerequisiteKind", codes::<PrerequisiteKind>()),
        ("TextItemKind", codes::<TextItemKind>()),
        ("DegreeLevel", codes::<DegreeLevel>()),
        ("DegreeType", codes::<DegreeType>()),
        ("StudyVariant", codes::<StudyVariant>()),
        ("DocumentType", codes::<DocumentType>()),
        ("StudySection", codes::<StudySection>()),
        ("PlanStatus", codes::<PlanStatus>()),
        ("PlanTotalScope", codes::<PlanTotalScope>()),
        ("ResolveStatus", codes::<ResolveStatus>()),
        ("Season", codes::<Season>()),
        ("EventCategory", codes::<EventCategory>()),
        ("Rhythm", codes::<Rhythm>()),
        ("Campus", codes::<Campus>()),
        ("LecturerRole", codes::<LecturerRole>()),
    ]
}
