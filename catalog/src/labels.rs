//! Every enum code of the contract and its German label, in one place.
//!
//! The codes are defined by the CHECK constraints of the snapshot
//! (`internal/catalogdb/migrations/`); their meaning by `internal/normalize/`.
//! `tests::every_enum_code_has_a_label` reads the constraints from a real snapshot
//! and fails for any code that has no variant here.

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
    fn label(self) -> &'static str;

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

    pub fn label(&self) -> &str {
        match self {
            Code::Known(e) => e.label(),
            Code::Unlabelled(code) => code,
        }
    }
}

macro_rules! code_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $($variant:ident = $code:literal => $label:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name { $($variant),+ }

        impl Labelled for $name {
            const ALL: &'static [Self] = &[$(Self::$variant),+];
            fn code(self) -> &'static str { match self { $(Self::$variant => $code),+ } }
            fn label(self) -> &'static str { match self { $(Self::$variant => $label),+ } }
        }
    };
}

code_enum! {
    /// In which semesters a module is offered.
    TurnusSeason {
        Winter = "winter" => "Wintersemester",
        Summer = "summer" => "Sommersemester",
        Both = "both" => "jedes Semester",
        Irregular = "irregular" => "unregelmäßig",
    }
}

code_enum! {
    /// Offered only in even or odd years.
    TurnusParity {
        Even = "even" => "gerade Jahre",
        Odd = "odd" => "ungerade Jahre",
    }
}

code_enum! {
    OfferStatus {
        Active = "active" => "im Angebot",
        PhaseOut = "phase_out" => "Auslaufmodul",
        NotOffered = "not_offered" => "wird nicht mehr angeboten",
    }
}

code_enum! {
    ExamForm {
        Map = "map" => "Modulabschlussprüfung (MAP)",
        PrereqMap = "prereq_map" => "Prüfungsvorleistung + Modulabschlussprüfung (MAP)",
        Mca = "mca" => "Continuous Assessment (MCA)",
        PrereqMca = "prereq_mca" => "Prüfungsvorleistung + Continuous Assessment (MCA)",
        Other = "other" => "andere Prüfungsform",
    }
}

code_enum! {
    /// Teaching forms of modules and of events.
    TeachingForm {
        Lecture = "lecture" => "Vorlesung",
        Exercise = "exercise" => "Übung",
        Seminar = "seminar" => "Seminar",
        Practical = "practical" => "Praktikum",
        Project = "project" => "Projekt",
        Tutorial = "tutorial" => "Tutorium",
        Consultation = "consultation" => "Konsultation",
        Excursion = "excursion" => "Exkursion",
        SelfStudy = "self_study" => "Selbststudium",
        Paper = "paper" => "Hausarbeit",
        Other = "other" => "andere Lehrform",
    }
}

code_enum! {
    /// What a module is within a program. NULL in the data means: no source says it.
    ModuleKind {
        Compulsory = "compulsory" => "Pflicht",
        Elective = "elective" => "Wahlpflicht",
        Thesis = "thesis" => "Abschlussarbeit",
        Internship = "internship" => "Praktikum",
        Fues = "fues" => "FÜS",
    }
}

code_enum! {
    /// How a module belongs to a program.
    Relation {
        Curricular = "curricular" => "Curriculum",
        Fues = "fues" => "Fachübergreifendes Studium",
    }
}

code_enum! {
    /// Which source states the kind of a module in a program.
    KindSource {
        PdfPlan = "pdf_plan" => "Regelstudienplan der Prüfungsordnung",
        ModulePage = "module_page" => "Modulbeschreibung",
        QisTree = "qis_tree" => "Modulbaum im Vorlesungsverzeichnis",
    }
}

code_enum! {
    KindBasis {
        Stated = "stated" => "ausdrücklich angegeben",
        Inferred = "inferred" => "aus der Gliederung abgeleitet",
    }
}

code_enum! {
    PrerequisiteKind {
        Mandatory = "mandatory" => "zwingend",
        Recommended = "recommended" => "empfohlen",
    }
}

code_enum! {
    TextItemKind {
        Literature = "literature" => "Literatur",
        Course = "course" => "Lehrveranstaltungen",
    }
}

code_enum! {
    DegreeLevel {
        Bachelor = "bachelor" => "Bachelor",
        Master = "master" => "Master",
        TeachingBachelor = "teaching_bachelor" => "Lehramt Bachelor",
        TeachingMaster = "teaching_master" => "Lehramt Master",
        Doctoral = "doctoral" => "Promotion",
        None = "none" => "ohne Abschluss",
        Other = "other" => "anderer Abschluss",
    }
}

code_enum! {
    DegreeType {
        University = "university" => "universitär",
        Applied = "applied" => "anwendungsorientiert",
    }
}

code_enum! {
    /// NULL in the data: the regular form of the program.
    StudyVariant {
        DualPractice = "dual_practice" => "dual, praxisintegrierend",
        DualTraining = "dual_training" => "dual, ausbildungsintegrierend",
        DoubleDegree = "double_degree" => "Doppelabschluss",
        Extended = "extended" => "erweiterte Studienform",
        Reduced = "reduced" => "verkürzte Studienform",
        Distance = "distance" => "Fernstudium",
        PartTime = "part_time" => "Teilzeit",
        Other = "other" => "besondere Studienform",
    }
}

code_enum! {
    DocumentType {
        Statute = "statute" => "Prüfungs- und Studienordnung",
        Amendment = "amendment" => "Änderungssatzung",
        Other = "other" => "Dokument",
    }
}

code_enum! {
    StudySection {
        Basic = "basic" => "Grundstudium",
        Main = "main" => "Fachstudium",
        Specialization = "specialization" => "Vertiefungsstudium",
        Core = "core" => "Kernstudium",
    }
}

code_enum! {
    PlanStatus {
        Saved = "saved" => "Regelstudienplan geprüft",
        SavedWithWarnings = "saved_with_warnings" => "Regelstudienplan geprüft, mit Hinweisen",
        NeedsReview = "needs_review" => "Regelstudienplan noch nicht geprüft",
        NoPlan = "no_plan" => "Prüfungsordnung enthält keinen Regelstudienplan",
        MissingSource = "missing_source" => "Prüfungsordnung liegt nicht vor",
    }
}

code_enum! {
    /// Whether a program named on a module page is one of the catalog.
    ResolveStatus {
        Resolved = "resolved" => "Studiengang im Katalog",
        Abroad = "abroad" => "Abschluss im Ausland",
        Unresolved = "unresolved" => "Studiengang nicht im Katalog",
    }
}

code_enum! {
    Season {
        Summer = "summer" => "Sommersemester",
        Winter = "winter" => "Wintersemester",
    }
}

code_enum! {
    EventCategory {
        Teaching = "teaching" => "Lehrveranstaltung",
        Exam = "exam" => "Prüfung",
        Other = "other" => "Sonstiges",
    }
}

code_enum! {
    Rhythm {
        Weekly = "weekly" => "wöchentlich",
        WeekA = "week_a" => "A-Woche",
        WeekB = "week_b" => "B-Woche",
        Single = "single" => "Einzeltermin",
        Block = "block" => "Blockveranstaltung",
        Other = "other" => "nach Absprache",
    }
}

code_enum! {
    Campus {
        Zentralcampus = "zentralcampus" => "Zentralcampus Cottbus",
        Sachsendorf = "sachsendorf" => "Cottbus-Sachsendorf",
        Senftenberg = "senftenberg" => "Senftenberg",
        Nord = "nord" => "Cottbus Nord",
    }
}

code_enum! {
    LecturerRole {
        Responsible = "responsible" => "Modulverantwortung",
        Instructor = "instructor" => "Lehrende",
    }
}

/// Weekday 1 = Monday … 7 = Sunday, as in the event tables.
pub fn weekday_label(weekday: i64) -> Option<&'static str> {
    match weekday {
        1 => Some("Montag"),
        2 => Some("Dienstag"),
        3 => Some("Mittwoch"),
        4 => Some("Donnerstag"),
        5 => Some("Freitag"),
        6 => Some("Samstag"),
        7 => Some("Sonntag"),
        _ => None,
    }
}

/// „Art nicht angegeben": what the UI says when no source states the kind of a module.
pub const KIND_UNKNOWN: &str = "Art nicht angegeben";

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
        ("ResolveStatus", codes::<ResolveStatus>()),
        ("Season", codes::<Season>()),
        ("EventCategory", codes::<EventCategory>()),
        ("Rhythm", codes::<Rhythm>()),
        ("Campus", codes::<Campus>()),
        ("LecturerRole", codes::<LecturerRole>()),
    ]
}
