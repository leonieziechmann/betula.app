//! The kinds of an event: Radix's teaching forms and the exam.
//!
//! No view of the snapshot carries `event_form` or `event.category`, only `type_raw` („Vorlesung",
//! „Vorlesung/Übung", „Prüfung"). So this is a port of Radix's own mapping,
//! `normalize.TeachingForm` (`radix/internal/normalize/normalize.go`) and the category rule of
//! `radix/internal/catalogbuild/events.go`, and a test holds it to `event_form` for every event of the
//! snapshot. A visitor hides kinds („Übungen aus"); an event is hidden by kind only when all its
//! kinds are, so „Übung" off hides the pure Übungen and keeps a „Vorlesung/Übung".

use serde::{Deserialize, Serialize};

use crate::i18n::Locale;
use crate::labels::{Labelled, TeachingForm};

/// FROZEN ORDER: the index is the bit in `Subscription::hidden_kinds`. Append only.
///
/// The eleven teaching forms of Radix, then the exam: nothing Radix tells apart is merged (a
/// Konsultation is often optional, a Tutorium too).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EventKind {
    Lecture,
    Exercise,
    Seminar,
    Practical,
    Project,
    Tutorial,
    Consultation,
    Excursion,
    SelfStudy,
    Paper,
    Other,
    Exam,
}

impl EventKind {
    /// Every kind, in the frozen order of the bits.
    pub const ALL: [EventKind; 12] = [
        EventKind::Lecture,
        EventKind::Exercise,
        EventKind::Seminar,
        EventKind::Practical,
        EventKind::Project,
        EventKind::Tutorial,
        EventKind::Consultation,
        EventKind::Excursion,
        EventKind::SelfStudy,
        EventKind::Paper,
        EventKind::Other,
        EventKind::Exam,
    ];

    /// `1 << index`, the kind's bit in a `KindSet` and in a subscription code.
    pub fn bit(self) -> u16 {
        1 << (self as u16)
    }

    /// The `TeachingForm` codes of the snapshot, and `exam`: what the store and the URL write.
    pub fn code(self) -> &'static str {
        match self.form() {
            Some(form) => form.code(),
            None => "exam",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.code() == code)
    }

    /// The chip's text.
    pub fn label(self, locale: Locale) -> &'static str {
        match (locale, self) {
            (Locale::De, EventKind::Lecture) => "Vorlesung",
            (Locale::De, EventKind::Exercise) => "Übung",
            (Locale::De, EventKind::Seminar) => "Seminar",
            (Locale::De, EventKind::Practical) => "Praktikum",
            (Locale::De, EventKind::Project) => "Projekt",
            (Locale::De, EventKind::Tutorial) => "Tutorium",
            (Locale::De, EventKind::Consultation) => "Konsultation",
            (Locale::De, EventKind::Excursion) => "Exkursion",
            (Locale::De, EventKind::SelfStudy) => "Selbststudium",
            (Locale::De, EventKind::Paper) => "Hausarbeit",
            (Locale::De, EventKind::Other) => "Sonstiges",
            (Locale::De, EventKind::Exam) => "Prüfung",
            (Locale::En, EventKind::Lecture) => "Lecture",
            (Locale::En, EventKind::Exercise) => "Exercise",
            (Locale::En, EventKind::Seminar) => "Seminar",
            (Locale::En, EventKind::Practical) => "Practical",
            (Locale::En, EventKind::Project) => "Project",
            (Locale::En, EventKind::Tutorial) => "Tutorial",
            (Locale::En, EventKind::Consultation) => "Consultation",
            (Locale::En, EventKind::Excursion) => "Excursion",
            (Locale::En, EventKind::SelfStudy) => "Self-study",
            (Locale::En, EventKind::Paper) => "Term paper",
            (Locale::En, EventKind::Other) => "Other",
            (Locale::En, EventKind::Exam) => "Exam",
        }
    }

    /// The label where a slot of the week grid has room for a few letters.
    pub fn short(self, locale: Locale) -> &'static str {
        match (locale, self) {
            (Locale::De, EventKind::Lecture) => "VL",
            (Locale::De, EventKind::Exercise) => "Ü",
            (Locale::De, EventKind::Seminar) => "Sem",
            (Locale::De, EventKind::Practical) => "Prak",
            (Locale::De, EventKind::Project) => "Proj",
            (Locale::De, EventKind::Tutorial) => "Tut",
            (Locale::De, EventKind::Consultation) => "Kons",
            (Locale::De, EventKind::Excursion) => "Exk",
            (Locale::De, EventKind::SelfStudy) => "Selbst",
            (Locale::De, EventKind::Paper) => "HA",
            (Locale::De, EventKind::Other) => "Sonst",
            (Locale::De, EventKind::Exam) => "Prüf",
            (Locale::En, EventKind::Lecture) => "Lec",
            (Locale::En, EventKind::Exercise) => "Ex",
            (Locale::En, EventKind::Seminar) => "Sem",
            (Locale::En, EventKind::Practical) => "Prac",
            (Locale::En, EventKind::Project) => "Proj",
            (Locale::En, EventKind::Tutorial) => "Tut",
            (Locale::En, EventKind::Consultation) => "Cons",
            (Locale::En, EventKind::Excursion) => "Exc",
            (Locale::En, EventKind::SelfStudy) => "Self",
            (Locale::En, EventKind::Paper) => "TP",
            (Locale::En, EventKind::Other) => "Other",
            (Locale::En, EventKind::Exam) => "Exam",
        }
    }

    pub fn of_form(form: TeachingForm) -> Self {
        match form {
            TeachingForm::Lecture => EventKind::Lecture,
            TeachingForm::Exercise => EventKind::Exercise,
            TeachingForm::Seminar => EventKind::Seminar,
            TeachingForm::Practical => EventKind::Practical,
            TeachingForm::Project => EventKind::Project,
            TeachingForm::Tutorial => EventKind::Tutorial,
            TeachingForm::Consultation => EventKind::Consultation,
            TeachingForm::Excursion => EventKind::Excursion,
            TeachingForm::SelfStudy => EventKind::SelfStudy,
            TeachingForm::Paper => EventKind::Paper,
            TeachingForm::Other => EventKind::Other,
        }
    }

    /// The teaching form of the kind; the exam is none.
    fn form(self) -> Option<TeachingForm> {
        match self {
            EventKind::Lecture => Some(TeachingForm::Lecture),
            EventKind::Exercise => Some(TeachingForm::Exercise),
            EventKind::Seminar => Some(TeachingForm::Seminar),
            EventKind::Practical => Some(TeachingForm::Practical),
            EventKind::Project => Some(TeachingForm::Project),
            EventKind::Tutorial => Some(TeachingForm::Tutorial),
            EventKind::Consultation => Some(TeachingForm::Consultation),
            EventKind::Excursion => Some(TeachingForm::Excursion),
            EventKind::SelfStudy => Some(TeachingForm::SelfStudy),
            EventKind::Paper => Some(TeachingForm::Paper),
            EventKind::Other => Some(TeachingForm::Other),
            EventKind::Exam => None,
        }
    }
}

/// A set of kinds as bits (`EventKind::bit`): the kinds of one event, or the kinds a visitor hid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KindSet(pub u16);

impl KindSet {
    pub fn with(self, k: EventKind) -> Self {
        KindSet(self.0 | k.bit())
    }

    pub fn without(self, k: EventKind) -> Self {
        KindSet(self.0 & !k.bit())
    }

    pub fn contains(self, k: EventKind) -> bool {
        self.0 & k.bit() != 0
    }

    /// Whether the set holds no kind this build knows (a bit of a newer build does not count).
    pub fn is_empty(self) -> bool {
        self.known().0 == 0
    }

    /// The kinds of the set, in the frozen order.
    pub fn iter(self) -> impl Iterator<Item = EventKind> {
        EventKind::ALL.into_iter().filter(move |k| self.contains(*k))
    }

    /// The set without bits no `EventKind` has: what a reader keeps of a code written by a newer
    /// build.
    pub fn known(self) -> Self {
        KindSet(self.0 & EventKind::ALL.iter().fold(0, |mask, k| mask | k.bit()))
    }

    /// Whether an event of these kinds is hidden when `hidden` are: only when it has kinds and
    /// every one of them is hidden.
    pub fn hidden_by(self, hidden: KindSet) -> bool {
        !self.is_empty() && self.iter().all(|k| hidden.contains(k))
    }

    /// The codes, comma-separated, in the frozen order: `exercise,tutorial`.
    pub fn codes(self) -> String {
        self.iter().map(EventKind::code).collect::<Vec<_>>().join(",")
    }

    /// The set of comma-separated codes; unknown codes are dropped.
    pub fn parse_codes(text: &str) -> KindSet {
        text.split(',')
            .filter_map(|code| EventKind::from_code(code.trim()))
            .fold(KindSet::default(), KindSet::with)
    }
}

/// Radix's `fold`: trimmed, lowercase, every run of ASCII whitespace one space (Go's `\s` is
/// ASCII only).
pub fn fold(text: &str) -> String {
    let lower = text.trim().to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut in_space = false;
    for c in lower.chars() {
        if c.is_ascii_whitespace() {
            if !in_space {
                out.push(' ');
            }
            in_space = true;
        } else {
            out.push(c);
            in_space = false;
        }
    }
    out
}

/// Radix's substrings per form, in its order (the order of the forms of a combined type).
const FORM_WORDS: [(TeachingForm, &[&str]); 10] = [
    (TeachingForm::Lecture, &["vorlesung", "lecture"]),
    (TeachingForm::Exercise, &["übung", "exercise"]),
    (TeachingForm::Seminar, &["seminar"]),
    (
        TeachingForm::Practical,
        &["praktikum", "praktische", "practical", "labor", "krankenbett", "schulpraktisch", "lernwerkst"],
    ),
    (TeachingForm::Project, &["projekt", "project", "entwurf", "stegreif"]),
    (TeachingForm::Tutorial, &["tutori"]),
    (TeachingForm::Consultation, &["konsultation", "consultation"]),
    (TeachingForm::Excursion, &["exkursion", "excursion"]),
    (TeachingForm::SelfStudy, &["selbststudium", "self organised", "self-organised", "self study"]),
    (TeachingForm::Paper, &["hausarbeit", "research paper"]),
];

/// The teaching forms of an event type, as `normalize.TeachingForm` finds them: fold; `""` or `•`
/// → none; every form one of whose substrings the type contains, in the order above; none matched
/// → `[Other]`. A combined type („Vorlesung/Übung") has several.
pub fn forms_of(type_raw: &str) -> Vec<TeachingForm> {
    let folded = fold(type_raw);
    if folded.is_empty() || folded == "•" {
        return Vec::new();
    }
    let forms: Vec<TeachingForm> = FORM_WORDS
        .iter()
        .filter(|(_, words)| words.iter().any(|word| folded.contains(word)))
        .map(|(form, _)| *form)
        .collect();
    if forms.is_empty() {
        vec![TeachingForm::Other]
    } else {
        forms
    }
}

/// Whether Radix files an event of this type as an exam (`category = 'exam'`): its lowercase type
/// contains „prüfung", „klausur" or „exam".
pub fn is_exam_type(type_raw: &str) -> bool {
    let lower = type_raw.to_lowercase();
    ["prüfung", "klausur", "exam"].iter().any(|word| lower.contains(word))
}

/// The kinds of a teaching event (a row of `v_module_schedule`): its forms; without a type, or
/// with one that names no form, `{Other}`. An exam is `EventKind::Exam` by where it comes from,
/// not by this.
pub fn kinds_of(type_raw: Option<&str>) -> KindSet {
    let kinds = forms_of(type_raw.unwrap_or(""))
        .into_iter()
        .fold(KindSet::default(), |set, form| set.with(EventKind::of_form(form)));
    if kinds.is_empty() {
        KindSet::default().with(EventKind::Other)
    } else {
        kinds
    }
}

/// What the clash check and the finder compare an event as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Lecture,
    Other,
    Exam,
}

/// `Exam` if the set holds the exam; `Lecture` if it holds a lecture (a „Vorlesung/Übung" is
/// one); else `Other`.
pub fn class_of(kinds: KindSet) -> Class {
    if kinds.contains(EventKind::Exam) {
        Class::Exam
    } else if kinds.contains(EventKind::Lecture) {
        Class::Lecture
    } else {
        Class::Other
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::db::{Database, Value};

    fn set(kinds: &[EventKind]) -> KindSet {
        kinds.iter().copied().fold(KindSet::default(), KindSet::with)
    }

    #[test]
    fn the_bits_are_frozen() {
        let codes: Vec<&str> = EventKind::ALL.iter().map(|k| k.code()).collect();
        assert_eq!(
            codes,
            [
                "lecture",
                "exercise",
                "seminar",
                "practical",
                "project",
                "tutorial",
                "consultation",
                "excursion",
                "self_study",
                "paper",
                "other",
                "exam"
            ]
        );
        for (i, kind) in EventKind::ALL.iter().enumerate() {
            assert_eq!(kind.bit(), 1 << i, "{kind:?}");
            assert_eq!(EventKind::from_code(kind.code()), Some(*kind));
        }
        assert_eq!(EventKind::from_code("Lecture"), None);
        for form in TeachingForm::ALL {
            assert_eq!(EventKind::of_form(*form).code(), form.code());
        }
        let labels: Vec<&str> = EventKind::ALL.iter().map(|k| k.label(Locale::De)).collect();
        assert_eq!(labels.join(" "), "Vorlesung Übung Seminar Praktikum Projekt Tutorium Konsultation Exkursion Selbststudium Hausarbeit Sonstiges Prüfung");
        let shorts: Vec<&str> = EventKind::ALL.iter().map(|k| k.short(Locale::De)).collect();
        assert_eq!(shorts.join(" "), "VL Ü Sem Prak Proj Tut Kons Exk Selbst HA Sonst Prüf");
        let labels: Vec<&str> = EventKind::ALL.iter().map(|k| k.label(Locale::En)).collect();
        assert_eq!(labels.join(", "), "Lecture, Exercise, Seminar, Practical, Project, Tutorial, Consultation, Excursion, Self-study, Term paper, Other, Exam");
    }

    #[test]
    fn sets_of_kinds() {
        let s = set(&[EventKind::Tutorial, EventKind::Exercise]);
        assert!(s.contains(EventKind::Exercise) && !s.contains(EventKind::Lecture));
        assert_eq!(s.codes(), "exercise,tutorial");
        assert_eq!(KindSet::parse_codes("tutorial, exercise,nonsense,,"), s);
        assert_eq!(KindSet::parse_codes(""), KindSet::default());
        assert_eq!(s.without(EventKind::Tutorial), set(&[EventKind::Exercise]));
        assert_eq!(s.iter().collect::<Vec<_>>(), [EventKind::Exercise, EventKind::Tutorial]);
        assert!(KindSet::default().is_empty());
        // A bit of a newer build is no kind here.
        let newer = KindSet(1 << 13);
        assert!(newer.is_empty());
        assert_eq!(KindSet(s.0 | 1 << 13).known(), s);
        assert_eq!(KindSet(s.0 | 1 << 13).codes(), "exercise,tutorial");
    }

    #[test]
    fn hidden_only_when_every_kind_is() {
        let hidden = set(&[EventKind::Exercise]);
        assert!(kinds_of(Some("Übung")).hidden_by(hidden));
        assert!(!kinds_of(Some("Vorlesung/Übung")).hidden_by(hidden));
        assert!(kinds_of(Some("Vorlesung/Übung")).hidden_by(set(&[EventKind::Lecture, EventKind::Exercise])));
        assert!(!KindSet::default().hidden_by(hidden));
        assert!(!KindSet(1 << 13).hidden_by(KindSet(u16::MAX)));
    }

    #[test]
    fn radix_forms_of_the_types_in_the_data() {
        use TeachingForm::*;
        for (raw, forms) in [
            ("Vorlesung", vec![Lecture]),
            ("Vorlesung/Übung", vec![Lecture, Exercise]),
            ("Seminar/Übung", vec![Exercise, Seminar]),
            ("Laborausbildung", vec![Practical]),
            ("Unterricht am Krankenbett", vec![Practical]),
            ("Schulpraktische Studien (SPS)", vec![Practical]),
            ("Stegreif", vec![Project]),
            ("Projektseminare", vec![Seminar, Project]),
            ("Lernwerkstätten", vec![Practical]),
            ("Tutorium", vec![Tutorial]),
            ("  SELF   organised  study ", vec![SelfStudy]),
            ("Kolloquium", vec![Other]),
            ("Prüfung", vec![Other]),
            ("", vec![]),
            ("  •  ", vec![]),
        ] {
            assert_eq!(forms_of(raw), forms, "{raw:?}");
        }
        assert!(is_exam_type("Prüfung"));
        assert!(is_exam_type("Klausur"));
        assert!(is_exam_type("Online-Exam"));
        assert!(!is_exam_type("Vorlesung"));
        assert!(!is_exam_type(""));
        assert_eq!(kinds_of(None), set(&[EventKind::Other]));
        assert_eq!(kinds_of(Some("•")), set(&[EventKind::Other]));
        assert_eq!(kinds_of(Some("Vorlesung/Übung")), set(&[EventKind::Lecture, EventKind::Exercise]));
        assert_eq!(class_of(kinds_of(Some("Vorlesung/Übung"))), Class::Lecture);
        assert_eq!(class_of(kinds_of(Some("Übung"))), Class::Other);
        assert_eq!(class_of(set(&[EventKind::Exam])), Class::Exam);
        assert_eq!(class_of(KindSet::default()), Class::Other);
    }

    /// The port against Radix's own result: `event_form` and `event.category` of every event.
    /// Reads the two tables no view has, which only a test may.
    #[test]
    fn the_teaching_form_port_matches_radix() {
        let pinned = crate::tests::studyplan_db("the_teaching_form_port_matches_radix");
        let is_pinned = pinned.is_some();
        let db = pinned.unwrap_or_else(crate::tests::open);
        let rows = db
            .query(
                "test",
                "SELECT e.id, e.type_raw, e.category, \
                 (SELECT group_concat(form) FROM event_form f WHERE f.event_id = e.id) FROM event e",
                &[],
            )
            .unwrap();
        let text = |value: &Value| match value {
            Value::Text(s) => Some(s.clone()),
            Value::Null => None,
            other => panic!("expected text, got {other:?}"),
        };
        let mut mismatches = Vec::new();
        for row in &rows.rows {
            let id = text(&row[0]).unwrap();
            let type_raw = text(&row[1]);
            let category = text(&row[2]).unwrap();
            let stored: BTreeSet<String> =
                text(&row[3]).map(|forms| forms.split(',').map(str::to_string).collect()).unwrap_or_default();
            let raw = type_raw.as_deref().unwrap_or("");
            if is_exam_type(raw) != (category == "exam") {
                mismatches.push(format!("{id} {type_raw:?}: category {category}"));
            }
            // Radix stores no forms for an exam.
            let ported: BTreeSet<String> = if category == "exam" {
                BTreeSet::new()
            } else {
                forms_of(raw).into_iter().map(|form| form.code().to_string()).collect()
            };
            if ported != stored {
                mismatches.push(format!("{id} {type_raw:?}: ported {ported:?}, event_form {stored:?}"));
            }
        }
        assert!(mismatches.is_empty(), "{} mismatches: {mismatches:#?}", mismatches.len());
        assert!(!rows.rows.is_empty());
        if is_pinned {
            assert_eq!(rows.rows.len(), 4244);
        }
    }
}
