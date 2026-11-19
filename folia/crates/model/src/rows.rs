//! Structs that mirror what the queries select from the read views.
//!
//! `Option` means the source does not say: unknown stays unknown all the way to the page.

use serde::{Deserialize, Serialize};

use crate::db::{Database, DbError, FromRow, Row, Value};
use crate::labels::{
    ChoiceBasis, ChoiceKind, Code, DegreeLevel, ExamForm, KindBasis, KindSource, ModuleKind, OfferStatus, PlanStatus,
    PrerequisiteKind, Relation, Season, StudySection, StudyVariant, TurnusParity, TurnusSeason,
};

/// `v_meta`, the keys the UI shows.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub built_at: Option<String>,
    pub data_changed_at: Option<String>,
    pub current_semester: Option<String>,
    pub content_digest: Option<String>,
    /// The Radix that built the snapshot (`radix/internal/version`); older snapshots do not say.
    pub radix_version: Option<String>,
}

/// „Datenstand" and the current semester.
pub fn meta(db: &dyn Database) -> Result<Meta, DbError> {
    let rows = db.query("meta", "SELECT key, value FROM v_meta", &[])?;
    let mut meta = Meta::default();
    for row in &rows.rows {
        if let [Value::Text(key), Value::Text(value)] = row.as_slice() {
            let value = Some(value.clone());
            match key.as_str() {
                "built_at" => meta.built_at = value,
                "data_changed_at" => meta.data_changed_at = value,
                "current_semester" => meta.current_semester = value,
                "content_digest" => meta.content_digest = value,
                "radix_version" => meta.radix_version = value,
                _ => {}
            }
        }
    }
    Ok(meta)
}

/// `v_semester`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Semester {
    pub key: String,
    pub season: Code<Season>,
    pub year: i64,
    pub label: String,
    pub starts_on: String,
    pub ends_on: String,
    pub is_current: bool,
    pub teaching_events: i64,
    pub exam_events: i64,
}

impl FromRow for Semester {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            key: row.text("key")?,
            season: Code::parse(&row.text("season")?),
            year: row.int("year")?,
            label: row.text("label")?,
            starts_on: row.text("starts_on")?,
            ends_on: row.text("ends_on")?,
            is_current: row.flag("is_current")?,
            teaching_events: row.int("teaching_events")?,
            exam_events: row.int("exam_events")?,
        })
    }
}

/// A module's titles and credits (`v_module`): what a row of a Notenübersicht is matched with
/// (`folia_transcript::matching`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleTitle {
    pub id: String,
    pub title: String,
    pub title_de: Option<String>,
    pub title_en: Option<String>,
    pub credits: Option<f64>,
    pub offer_status: Code<OfferStatus>,
}

impl FromRow for ModuleTitle {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            id: row.text("id")?,
            title: row.text("title")?,
            title_de: row.opt_text("title_de")?,
            title_en: row.opt_text("title_en")?,
            credits: row.opt_real("credits")?,
            offer_status: Code::parse(&row.text("offer_status")?),
        })
    }
}

/// `v_department`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Department {
    pub id: i64,
    pub code: String,
    pub label: String,
    pub name_de: String,
    pub name_en: Option<String>,
    pub modules: i64,
}

impl FromRow for Department {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            id: row.int("id")?,
            code: row.text("code")?,
            label: row.text("label")?,
            name_de: row.text("name_de")?,
            name_en: row.opt_text("name_en")?,
            modules: row.int("modules")?,
        })
    }
}

/// `v_program`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub degree_level: Code<DegreeLevel>,
    pub study_variant: Option<Code<StudyVariant>>,
    /// „B.Sc." only where a source states it.
    pub degree_label: Option<String>,
    pub degree_raw: String,
    /// `degree_label`, else „Bachelor" / „Master"; `None` where even that is unknown.
    pub degree_display: Option<String>,
    pub po_version: String,
    pub po_year: Option<i64>,
    pub family_key: String,
    /// The same subject across degree levels (Bachelor and Master of one name).
    pub name_key: String,
    pub is_latest_po: bool,
    pub source_url: String,
    pub has_plan: bool,
    pub plan_status: Option<Code<PlanStatus>>,
    pub curricular_modules: i64,
    pub fues_modules: i64,
    pub documents: i64,
}

impl Program {
    /// What to print as the degree: never invented, falls back to the raw QIS text.
    pub fn degree(&self) -> &str {
        self.degree_display.as_deref().unwrap_or(&self.degree_raw)
    }
}

impl FromRow for Program {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            id: row.text("id")?,
            slug: row.text("slug")?,
            name: row.text("name")?,
            degree_level: Code::parse(&row.text("degree_level")?),
            study_variant: Code::parse_opt(row.opt_text("study_variant")?),
            degree_label: row.opt_text("degree_label")?,
            degree_raw: row.text("degree_raw")?,
            degree_display: row.opt_text("degree_display")?,
            po_version: row.text("po_version")?,
            po_year: row.opt_int("po_year")?,
            family_key: row.text("family_key")?,
            name_key: row.text("name_key")?,
            is_latest_po: row.flag("is_latest_po")?,
            source_url: row.text("source_url")?,
            has_plan: row.flag("has_plan")?,
            plan_status: Code::parse_opt(row.opt_text("plan_status")?),
            curricular_modules: row.int("curricular_modules")?,
            fues_modules: row.int("fues_modules")?,
            documents: row.int("documents")?,
        })
    }
}

/// `v_program_module`: one module of one program.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramModule {
    pub module_id: String,
    pub relation: Code<Relation>,
    /// `None`: no source states the kind. Never shown as „Pflicht".
    pub kind: Option<Code<ModuleKind>>,
    pub kind_source: Option<Code<KindSource>>,
    pub kind_basis: Option<Code<KindBasis>>,
    pub area: Option<String>,
    pub section: Option<Code<StudySection>>,
    pub module_title: String,
    pub module_credits: Option<f64>,
    pub offer_status: Code<OfferStatus>,
    pub turnus_season: Option<Code<TurnusSeason>>,
    /// Only from a validated study plan.
    pub plan_semester: Option<i64>,
}

impl FromRow for ProgramModule {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            module_id: row.text("module_id")?,
            relation: Code::parse(&row.text("relation")?),
            kind: Code::parse_opt(row.opt_text("kind")?),
            kind_source: Code::parse_opt(row.opt_text("kind_source")?),
            kind_basis: Code::parse_opt(row.opt_text("kind_basis")?),
            area: row.opt_text("area")?,
            section: Code::parse_opt(row.opt_text("section")?),
            module_title: row.text("module_title")?,
            module_credits: row.opt_real("module_credits")?,
            offer_status: Code::parse(&row.text("offer_status")?),
            turnus_season: Code::parse_opt(row.opt_text("turnus_season")?),
            plan_semester: row.opt_int("plan_semester")?,
        })
    }
}

/// One row of the catalog table: `v_module_facets` joined with `v_module`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogRow {
    pub id: String,
    pub title: String,
    pub title_de: Option<String>,
    pub title_en: Option<String>,
    pub credits: Option<f64>,
    pub turnus_season: Option<Code<TurnusSeason>>,
    pub turnus_parity: Option<Code<TurnusParity>>,
    pub offer_status: Code<OfferStatus>,
    pub teaches_german: Option<bool>,
    pub teaches_english: Option<bool>,
    pub is_fues: bool,
    pub is_limited: Option<bool>,
    pub department: Option<String>,
    /// Teaching events in the module's newest semester.
    pub teaching_events: i64,
    pub exam_form: Option<Code<ExamForm>>,
    /// The responsible persons as the module page names them.
    pub responsible: Option<String>,
    /// Present when a program is selected.
    pub kind: Option<Code<ModuleKind>>,
    /// Where the kind comes from and whether a source states it or Betula infers it: the badge
    /// says „abgeleitet" (`folia_design::ui::KindBadge`).
    #[serde(default)]
    pub kind_source: Option<Code<KindSource>>,
    #[serde(default)]
    pub kind_basis: Option<Code<KindBasis>>,
    pub plan_semester: Option<i64>,
    pub area: Option<String>,
    /// In the module's newest semester with teaching: how many courses at campuses it is held as
    /// (docs/radix/schema-v2.md, „Tracks"; 0 below two), and its choices („Choices"). Not of the
    /// list's query: `catalog_offers` reads them for the rows of a page.
    #[serde(default)]
    pub courses: u32,
    #[serde(default)]
    pub choices: Vec<ChoiceSummary>,
    /// While the list is a search's: what of the module the search found, for the row's marks
    /// (`folia_search::marks`). Filled by the query of the list, never read from a row of SQL.
    #[serde(default)]
    pub marks: Option<SearchMarks>,
    /// While the list is a program's FÜS list: what it says of the module besides that it is on it
    /// (`FuesNote`). Not of the list's query: `catalog_fues_notes` reads them for the rows of a page.
    #[serde(default)]
    pub fues_note: Option<FuesNote>,
}

/// What a program's FÜS list says of a module besides that it is on it, by the BTU's FÜS
/// catalogs (docs/radix/schema-v2.md, „FÜS catalogs"; owner, 2026-11-17: „mit Hinweis statt
/// ausblenden"): the choices of the program its negative list excludes the module for
/// („Anwendungsfach Physik"), and the seasons whose catalog holds the module and excludes it for
/// every student of the program, while the other admits it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FuesNote {
    pub excluded_for: Vec<String>,
    pub not_in: Vec<Code<TurnusSeason>>,
}

impl FuesNote {
    /// The note of a module from its rows of `catalog_fues_notes`; `None` where they say nothing.
    /// With the student's `choice` (`ProgramScope::choice`) it is that student's: a season that
    /// excludes the module for the choice is a season it is not admitted in, and what the list
    /// excludes for other choices is not said (it is not theirs).
    pub fn of<'a>(rows: impl IntoIterator<Item = &'a FuesNoteRow>, choice: Option<&str>) -> Option<Self> {
        let mut note = Self::default();
        for row in rows {
            if !row.admitted || choice.is_some_and(|choice| row.excludes(choice)) {
                note.not_in.push(row.season.clone());
            }
            if choice.is_some() {
                continue;
            }
            for scope in row.choices() {
                if !note.excluded_for.iter().any(|known| known == scope) {
                    note.excluded_for.push(scope.to_string());
                }
            }
        }
        (!note.excluded_for.is_empty() || !note.not_in.is_empty()).then_some(note)
    }

    /// Whether the FÜS list leaves the module of `rows` (its rows of one program) out for
    /// `choice`: a season's catalog excludes it for the choice, and no season's admits it for it.
    /// A season without a row admits the module for everyone, or its catalog does not hold it;
    /// the rows cannot tell which, and the module then counts as left out (on canary's data of
    /// 2026-11-19, 17 of the 214 pairs of a module and a choice another season's catalog admits).
    /// `folia_query::sql` asks the same of `v_program_fues_note`.
    pub fn leaves_out<'a>(rows: impl IntoIterator<Item = &'a FuesNoteRow> + Clone, choice: &str) -> bool {
        rows.clone().into_iter().any(|row| row.excludes(choice)) && !rows.into_iter().any(|row| row.admits(choice))
    }
}

/// A choice of a program that its negative list in the BTU's FÜS catalogs tells apart, as the
/// catalog names it („Anwendungsfach Physik"), and how many modules of the program's FÜS list it
/// leaves out (`FuesNote::leaves_out`): what the FÜS list lets a student choose
/// (`ProgramScope::choice`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FuesChoice {
    pub name: String,
    pub leaves_out: u32,
}

impl FuesChoice {
    /// The choices of a program's FÜS list from its rows of `v_program_fues_note`
    /// (`program_fues_notes`), in the order of their names (`folia_search::order_key` is the
    /// caller's to apply: the names shown may be shorter).
    pub fn of(rows: &[FuesNoteRow]) -> Vec<Self> {
        let mut names: Vec<&str> = rows.iter().flat_map(FuesNoteRow::choices).collect();
        names.sort_unstable();
        names.dedup();
        let mut modules: Vec<&str> = rows.iter().map(|row| row.module_id.as_str()).collect();
        modules.sort_unstable();
        modules.dedup();
        names
            .into_iter()
            .map(|name| {
                let left_out = modules.iter().filter(|id| FuesNote::leaves_out(rows.iter().filter(|row| row.module_id == **id), name)).count();
                Self { name: name.to_string(), leaves_out: u32::try_from(left_out).unwrap_or(u32::MAX) }
            })
            .collect()
    }
}

/// A row of `catalog_fues_notes` (`v_program_fues_note`): a season of a module of a program's FÜS
/// list that says more, the catalog it follows, whether that catalog admits the module for every
/// student of the program, and the choices it excludes it for (`; `-joined).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FuesNoteRow {
    pub module_id: String,
    pub season: Code<TurnusSeason>,
    pub semester: String,
    pub admitted: bool,
    pub excluded_for: Option<String>,
}

impl FuesNoteRow {
    /// The choices its season's negative list excludes the module for.
    pub fn choices(&self) -> impl Iterator<Item = &str> {
        self.excluded_for.iter().flat_map(|scopes| scopes.split("; ")).map(str::trim).filter(|scope| !scope.is_empty())
    }

    /// Whether its season's negative list excludes the module for `choice`.
    pub fn excludes(&self, choice: &str) -> bool {
        self.choices().any(|scope| scope == choice.trim())
    }

    /// Whether its season's catalog admits the module for `choice`: for every student of the
    /// program, and not excluded for the choice.
    pub fn admits(&self, choice: &str) -> bool {
        self.admitted && !self.excludes(choice)
    }
}

impl FromRow for FuesNoteRow {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            module_id: row.text("module_id")?,
            season: Code::parse(&row.text("season")?),
            semester: row.text("semester")?,
            admitted: row.flag("admitted")?,
            excluded_for: row.opt_text("excluded_for")?,
        })
    }
}

/// What a search found in a module, as the row of the list shows it: byte ranges of the title it
/// shows, and, where they were the reason, the title in the other language and a lecturer's name
/// with theirs, the number and the abbreviation (folia/design/suche/konzept.html, „Die Zeile zeigt
/// den Grund").
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchMarks {
    /// Byte ranges of `CatalogRow::title`.
    pub title: Vec<(u32, u32)>,
    /// The title in the other language, with the byte ranges found in it.
    pub other: Option<MarkedText>,
    /// A lecturer's name as QIS writes it („Hofstedt, Petra"), with the byte ranges found in it.
    pub lecturer: Option<MarkedText>,
    /// Whether that lecturer only teaches the module (is not responsible for it).
    pub teaching: bool,
    /// Byte ranges of `CatalogRow::id`: the number, or its start, as typed.
    pub id: Vec<(u32, u32)>,
    /// The abbreviations found („ThI", „BS1, BS2"), with the byte ranges of what the query names
    /// („BS").
    pub abbrev: Option<MarkedText>,
}

/// A text with byte ranges of it to mark.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkedText {
    pub text: String,
    pub spans: Vec<(u32, u32)>,
}

/// A choice among a module's events as its row in a list names it: „14 Projekte zur Wahl",
/// „Übung je Studiengang", „3 Gruppen zur Wahl".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChoiceSummary {
    pub kind: Code<ChoiceKind>,
    pub basis: Code<ChoiceBasis>,
    /// How many options there are, and how many one takes.
    pub options: u32,
    pub pick: u32,
    /// The type QIS gives every event of the options („Projekt"); `None` where they differ.
    pub event_type: Option<String>,
}

/// A row of `catalog_offers`: a module's courses and one of its choices (`None` for a module with
/// courses and no choice), in its newest semester with teaching.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogOffer {
    pub module_id: String,
    pub courses: u32,
    pub choice: Option<ChoiceSummary>,
}

impl FromRow for CatalogOffer {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        let count = |column: &str| -> Result<u32, DbError> { Ok(row.opt_int(column)?.and_then(|n| u32::try_from(n).ok()).unwrap_or(0)) };
        let choice = match row.opt_text("kind")? {
            Some(kind) => Some(ChoiceSummary {
                kind: Code::parse(&kind),
                basis: Code::parse(&row.text("basis")?),
                options: count("options")?,
                pick: count("pick")?.max(1),
                event_type: row.opt_text("event_type")?,
            }),
            None => None,
        };
        Ok(Self { module_id: row.text("module_id")?, courses: count("courses")?, choice })
    }
}

impl FromRow for CatalogRow {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            id: row.text("module_id")?,
            title: row.text("title")?,
            title_de: row.opt_text("title_de")?,
            title_en: row.opt_text("title_en")?,
            credits: row.opt_real("credits")?,
            turnus_season: Code::parse_opt(row.opt_text("turnus_season")?),
            turnus_parity: Code::parse_opt(row.opt_text("turnus_parity")?),
            offer_status: Code::parse(&row.text("offer_status")?),
            teaches_german: row.opt_flag("teaches_german")?,
            teaches_english: row.opt_flag("teaches_english")?,
            is_fues: row.flag("is_fues")?,
            is_limited: row.opt_flag("is_limited")?,
            department: row.opt_text("department")?,
            teaching_events: row.int("teaching_events")?,
            exam_form: Code::parse_opt(row.opt_text("exam_form")?),
            responsible: row.opt_text("responsible")?,
            kind: Code::parse_opt(row.opt_text("kind")?),
            kind_source: Code::parse_opt(row.opt_text("kind_source")?),
            kind_basis: Code::parse_opt(row.opt_text("kind_basis")?),
            plan_semester: row.opt_int("plan_semester")?,
            area: row.opt_text("area")?,
            courses: 0,
            choices: Vec::new(),
            marks: None,
            fues_note: None,
        })
    }
}

/// One page of the catalog with the exact number of matches.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogPage {
    pub total: u64,
    pub offset: u64,
    pub rows: Vec<CatalogRow>,
}

/// `v_module`: everything the module page states about the module itself.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Module {
    pub id: String,
    pub title: String,
    pub title_de: Option<String>,
    pub title_en: Option<String>,
    pub credits: Option<f64>,
    pub language_raw: Option<String>,
    pub teaches_german: Option<bool>,
    pub teaches_english: Option<bool>,
    pub duration_raw: Option<String>,
    pub duration_semesters: Option<i64>,
    pub turnus_raw: Option<String>,
    pub turnus_season: Option<Code<TurnusSeason>>,
    pub turnus_parity: Option<Code<TurnusParity>>,
    pub offer_status: Code<OfferStatus>,
    pub limitation_raw: Option<String>,
    pub is_limited: Option<bool>,
    pub participant_limit: Option<i64>,
    pub exam_form: Option<Code<ExamForm>>,
    pub exam_form_raw: Option<String>,
    pub exam_details: Option<String>,
    pub grading_raw: Option<String>,
    pub is_graded: Option<bool>,
    pub is_fues: bool,
    pub department: Option<String>,
    /// The language of the module's page and so of its texts („de", „en"): the BTU serves a
    /// module's page in the language it is taught in.
    pub page_lang: Option<String>,
    /// The free texts are Markdown (`crate::text`).
    pub learning_outcomes: Option<String>,
    pub contents: Option<String>,
    pub prerequisites_recommended: Option<String>,
    pub prerequisites_mandatory: Option<String>,
    pub remarks: Option<String>,
    pub source_url: Option<String>,
    pub fetched_at: Option<String>,
    pub at_zentralcampus: Option<bool>,
    pub at_sachsendorf: Option<bool>,
    pub at_senftenberg: Option<bool>,
    /// What the module is called for short without a program („EvS", schema 9,
    /// docs/radix/schema-v2.md „Short names"); a program may call it otherwise
    /// (`ProgramAbbrev`). Every module has one; `None` in an answer kept from before 1.33.1.
    #[serde(default)]
    pub abbrev: Option<String>,
}

impl FromRow for Module {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            id: row.text("id")?,
            title: row.text("title")?,
            title_de: row.opt_text("title_de")?,
            title_en: row.opt_text("title_en")?,
            credits: row.opt_real("credits")?,
            language_raw: row.opt_text("language_raw")?,
            teaches_german: row.opt_flag("teaches_german")?,
            teaches_english: row.opt_flag("teaches_english")?,
            duration_raw: row.opt_text("duration_raw")?,
            duration_semesters: row.opt_int("duration_semesters")?,
            turnus_raw: row.opt_text("turnus_raw")?,
            turnus_season: Code::parse_opt(row.opt_text("turnus_season")?),
            turnus_parity: Code::parse_opt(row.opt_text("turnus_parity")?),
            offer_status: Code::parse(&row.text("offer_status")?),
            limitation_raw: row.opt_text("limitation_raw")?,
            is_limited: row.opt_flag("is_limited")?,
            participant_limit: row.opt_int("participant_limit")?,
            exam_form: Code::parse_opt(row.opt_text("exam_form")?),
            exam_form_raw: row.opt_text("exam_form_raw")?,
            exam_details: row.opt_text("exam_details")?,
            grading_raw: row.opt_text("grading_raw")?,
            is_graded: row.opt_flag("is_graded")?,
            is_fues: row.flag("is_fues")?,
            department: row.opt_text("department")?,
            page_lang: row.opt_text("page_lang")?,
            learning_outcomes: row.opt_text("learning_outcomes")?,
            contents: row.opt_text("contents")?,
            prerequisites_recommended: row.opt_text("prerequisites_recommended")?,
            prerequisites_mandatory: row.opt_text("prerequisites_mandatory")?,
            remarks: row.opt_text("remarks")?,
            source_url: row.opt_text("source_url")?,
            fetched_at: row.opt_text("fetched_at")?,
            at_zentralcampus: row.opt_flag("at_zentralcampus")?,
            at_sachsendorf: row.opt_flag("at_sachsendorf")?,
            at_senftenberg: row.opt_flag("at_senftenberg")?,
            abbrev: row.opt_text("abbrev")?,
        })
    }
}

/// `v_module_prerequisite`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prerequisite {
    pub module_id: String,
    pub required_module_id: String,
    pub kind: Code<PrerequisiteKind>,
    /// Of the rows of one module and kind with the same choice one is enough, and every choice is
    /// needed: „Modul 12102 Programmierpraktikum ODER Modul 11900 Programmierpraktikum (IMT) UND
    /// Modul 12104" (12202) is 12102 and 11900 in choice 1, 12104 in choice 2. Most choices hold
    /// one module.
    pub choice: i64,
    pub required_title: Option<String>,
    pub required_offer_status: Option<Code<OfferStatus>>,
}

impl Prerequisite {
    /// The choice the row is one of, as `prerequisite_choices` groups them.
    pub fn choice_of(&self) -> (&str, &str, i64) {
        (&self.module_id, self.kind.code(), self.choice)
    }
}

impl FromRow for Prerequisite {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            module_id: row.text("module_id")?,
            required_module_id: row.text("required_module_id")?,
            kind: Code::parse(&row.text("kind")?),
            choice: row.int("choice")?,
            required_title: row.opt_text("required_title")?,
            required_offer_status: Code::parse_opt(row.opt_text("required_offer_status")?),
        })
    }
}

/// Prerequisites in their choices (`Prerequisite::choice`), each choice where its first row
/// stands and its rows in their order: of the rows of one choice one is enough.
pub fn prerequisite_choices<'a>(rows: impl IntoIterator<Item = &'a Prerequisite>) -> Vec<Vec<&'a Prerequisite>> {
    let mut choices: Vec<Vec<&'a Prerequisite>> = Vec::new();
    for row in rows {
        match choices.iter_mut().find(|choice| choice.first().is_some_and(|first| first.choice_of() == row.choice_of())) {
            Some(choice) => choice.push(row),
            None => choices.push(vec![row]),
        }
    }
    choices
}

/// `v_module_vector`: a module's vector for the semantic search, computed by Radix and packed as
/// the crate `semantic` packs it (`folia_semantic::quantize`: values of 4 bits, two to a byte), which
/// takes it as it is (`folia_semantic::Index::push_codes`).
#[derive(Clone, Debug, PartialEq)]
pub struct ModuleVector {
    pub module_id: String,
    pub scale: f32,
    pub vector: Vec<u8>,
}

impl FromRow for ModuleVector {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        // The database seam carries no BLOBs; the query hands the vector over as hex.
        let hex = row.text("vector")?;
        let digit = |b: u8| match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'A'..=b'F' => Some(b - b'A' + 10),
            b'a'..=b'f' => Some(b - b'a' + 10),
            _ => None,
        };
        let vector = hex
            .as_bytes()
            .chunks(2)
            .map(|pair| match pair {
                [hi, lo] => Some(digit(*hi)? << 4 | digit(*lo)?),
                _ => None,
            })
            .collect::<Option<Vec<u8>>>()
            .ok_or_else(|| row.error("vector", "not hex"))?;
        #[allow(clippy::cast_possible_truncation)] // stored as REAL from an f32
        let scale = row.real("scale")? as f32;
        Ok(Self { module_id: row.text("module_id")?, scale, vector })
    }
}

/// What the search of a list finds outside its filters (`queries::search_elsewhere`): modules
/// that are offered, and modules that are no longer offered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchElsewhere {
    pub offered: u64,
    pub not_offered: u64,
}

impl FromRow for SearchElsewhere {
    fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        let count = |column: &str| row.int(column).map(|n| u64::try_from(n).unwrap_or(0));
        Ok(Self { offered: count("offered")?, not_offered: count("not_offered")? })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prerequisites_stand_in_their_choices() {
        let row = |module: &str, required: &str, kind: &str, choice: i64| Prerequisite {
            module_id: module.into(),
            required_module_id: required.into(),
            kind: Code::parse(kind),
            choice,
            required_title: None,
            required_offer_status: None,
        };
        let rows = [
            row("12202", "12102", "mandatory", 1),
            row("12202", "11900", "mandatory", 1),
            row("12202", "12104", "mandatory", 2),
            row("12202", "12101", "recommended", 1),
            row("12326", "12104", "mandatory", 1),
        ];
        let choices: Vec<Vec<&str>> = prerequisite_choices(&rows).into_iter().map(|choice| choice.into_iter().map(|p| p.required_module_id.as_str()).collect()).collect();
        assert_eq!(choices, [vec!["12102", "11900"], vec!["12104"], vec!["12101"], vec!["12104"]]);
    }
}

#[cfg(test)]
mod fues_note_tests {
    use super::*;

    fn row(season: &str, admitted: bool, excluded_for: Option<&str>) -> FuesNoteRow {
        FuesNoteRow { module_id: "11812".into(), season: Code::parse(season), semester: "2026W".into(), admitted, excluded_for: excluded_for.map(String::from) }
    }

    #[test]
    fn a_note_gathers_the_choices_and_the_seasons_that_exclude() {
        let rows = [row("winter", true, Some("Anwendungsfach Maschinenbau / Elektrotechnik; Anwendungsfach Physik")), row("summer", false, Some("Anwendungsfach Physik"))];
        let note = FuesNote::of(rows.iter(), None).unwrap();
        assert_eq!(note.excluded_for, ["Anwendungsfach Maschinenbau / Elektrotechnik", "Anwendungsfach Physik"]);
        assert!(note.not_in.len() == 1 && note.not_in[0].is(crate::labels::TurnusSeason::Summer));
        assert_eq!(FuesNote::of([row("winter", true, None)].iter(), None), None);
        assert_eq!(FuesNote::of(std::iter::empty(), None), None);
    }

    /// With a choice the note is the student's: a season that excludes the module for it is one
    /// it is not admitted in, and other choices are not named.
    #[test]
    fn a_note_with_a_choice_says_what_holds_for_it() {
        let winter = row("winter", true, Some("Anwendungsfach Physik; Anwendungsfach Wirtschaftswissenschaften"));
        let summer = row("summer", true, Some("Anwendungsfach Wirtschaftswissenschaften"));
        let rows = [winter.clone(), summer.clone()];
        let physik = FuesNote::of(rows.iter(), Some("Anwendungsfach Physik")).unwrap();
        assert!(physik.excluded_for.is_empty());
        assert!(physik.not_in.len() == 1 && physik.not_in[0].is(crate::labels::TurnusSeason::Winter));
        // Another choice that neither excludes: nothing to say.
        assert_eq!(FuesNote::of(rows.iter(), Some("Anwendungsfach Bauingenieurwesen")), None);
        // Physik is admitted in the summer: the module stays. Wirtschaftswissenschaften nowhere.
        assert!(!FuesNote::leaves_out(rows.iter(), "Anwendungsfach Physik"));
        assert!(FuesNote::leaves_out(rows.iter(), "Anwendungsfach Wirtschaftswissenschaften"));
        assert!(!FuesNote::leaves_out(rows.iter(), "Anwendungsfach Bauingenieurwesen"));
        // A season excluded for everyone does not admit it for a choice either.
        let closed = [row("winter", false, None), row("summer", true, Some("Anwendungsfach Physik"))];
        assert!(FuesNote::leaves_out(closed.iter(), "Anwendungsfach Physik"));
        assert!(!FuesNote::leaves_out(closed.iter(), "Anwendungsfach Bauingenieurwesen"));
    }

    #[test]
    fn the_choices_of_a_list_count_what_they_leave_out() {
        let of = |module: &str, season: &str, admitted: bool, excluded_for: Option<&str>| FuesNoteRow { module_id: module.into(), ..row(season, admitted, excluded_for) };
        let rows = [
            of("13102", "winter", true, Some("Anwendungsfach Physik")),
            of("12488", "winter", true, Some("Anwendungsfach Maschinenbau / Elektrotechnik")),
            of("11812", "winter", true, Some("Anwendungsfach Maschinenbau / Elektrotechnik")),
            of("11812", "summer", true, Some("Anwendungsfach Maschinenbau / Elektrotechnik; Anwendungsfach Physik")),
            of("12045", "winter", false, None),
        ];
        assert_eq!(
            FuesChoice::of(&rows),
            [
                FuesChoice { name: "Anwendungsfach Maschinenbau / Elektrotechnik".into(), leaves_out: 2 },
                // 11812 is admitted for Physik in the winter.
                FuesChoice { name: "Anwendungsfach Physik".into(), leaves_out: 1 },
            ]
        );
        assert!(FuesChoice::of(&rows[4..]).is_empty());
    }
}
