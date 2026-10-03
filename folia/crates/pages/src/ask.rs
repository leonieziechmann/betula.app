//! The questions a page asks of the catalog (docs/folia/folia-refactor.md §6.3): one type per loader,
//! with the type of its answer and the loader it runs. The app asks them through `DataClient`
//! (`folia_app::data`), which answers on the page's thread for now and from the data worker later;
//! either side runs the same `Ask::run`, so what a page gets does not depend on where it was worked
//! out.
//!
//! What a question asks is all in the question (and the snapshot): two equal questions have equal
//! answers, so an answer can be kept by its question (`Ask::key`) until the snapshot changes.

use std::fmt::Debug;

use folia_calendar::select::Selection;
use folia_calendar::semester::SemesterKey;
use folia_calendar::share::SharedPlan;
use folia_locale::Locale;
use folia_model::db::{Database, DbError};
use folia_model::rows::{CatalogRow, Meta, Program, Semester};
use folia_model::rows_detail::DateRow;
use folia_routes::filter::{CatalogQuery, FitsFilter};
use folia_routes::url::{BookmarkSort, CatalogUrl};
use folia_timetable::fit::CandidateSet;
use serde::{Deserialize, Serialize};

use crate::{
    BookmarksData, CatalogChoices, CatalogData, CatalogSummary, FitResult, Ground, HomeData, ModuleData, MyProgramInfo,
    Overlay, PlanSource, ProgramData, ProgramsData, SharedPlanData, StudyplanData,
};

/// What a page shows instead of data. Serializable: it crosses to the page from wherever the
/// question was answered.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DataError {
    /// No snapshot yet: worth retrying. Otherwise the query itself failed.
    pub unavailable: bool,
    pub message: String,
}

impl From<DbError> for DataError {
    fn from(error: DbError) -> Self {
        Self { unavailable: matches!(error, DbError::Unavailable(_)), message: error.to_string() }
    }
}

/// How questions of one kind wait (§6.3): one of a lane replaces another of the same lane that
/// has not been answered yet, so typing never piles up work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// What a page shows: every question is answered.
    Page,
    /// What follows the keys or a slider: only the newest of its kind counts.
    Typing,
}

/// What the side that answers keeps between questions besides their answers: the finder's
/// candidates, which do not change with the plan (`folia_timetable::fit::CandidateSet`).
#[derive(Default)]
pub struct Kept {
    pub candidates: Option<CandidateSet>,
}

/// A question of a page, its answer, and the loader that answers it.
pub trait Ask: Clone + Debug + PartialEq + Serialize + 'static {
    type Answer: Clone + Debug + PartialEq + 'static;
    /// Its name in the messages to the data worker and in the keys of kept answers.
    const NAME: &'static str;
    const LANE: Lane = Lane::Page;
    /// The answer, from `db`, with what is kept besides (`Kept`).
    fn run(&self, db: &dyn Database, kept: &mut Kept) -> Result<Self::Answer, DbError>;
    /// The question as a key of its answer: its name and its fields.
    fn key(&self) -> String {
        format!("{}:{}", Self::NAME, serde_json::to_string(self).unwrap_or_default())
    }
}

macro_rules! ask {
    ($(#[$doc:meta])* $name:ident { $($field:ident: $ty:ty),* $(,)? } -> $answer:ty, $lane:expr, |$this:ident, $db:ident, $kept:ident| $body:expr) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct $name { $(pub $field: $ty),* }

        impl Ask for $name {
            type Answer = $answer;
            const NAME: &'static str = stringify!($name);
            const LANE: Lane = $lane;
            #[allow(unused_variables)]
            fn run(&self, $db: &dyn Database, $kept: &mut Kept) -> Result<$answer, DbError> {
                let $this = self;
                $body
            }
        }
    };
}

ask!(
    /// The ground at the end of every page: the data's dates and the current semester.
    GroundAsk {} -> Ground, Lane::Page, |q, db, kept| crate::ground(db)
);
ask!(
    /// The start page with the counts of the catalogs its entries lead to.
    HomeAsk { entries: Vec<CatalogQuery> } -> HomeData, Lane::Page, |q, db, kept| crate::home(db, &q.entries)
);
ask!(
    /// What the catalog's pickers offer.
    CatalogChoicesAsk {} -> CatalogChoices, Lane::Page, |q, db, kept| crate::catalog_choices(db)
);
ask!(
    /// A page of the catalog's list with what its panel says about it.
    CatalogAsk { url: CatalogUrl, locale: Locale } -> CatalogData, Lane::Page, |q, db, kept| crate::catalog(db, &q.url, q.locale)
);
ask!(
    /// What the filter of the phone's sheet holds before the list follows it.
    CatalogSummaryAsk { query: CatalogQuery } -> CatalogSummary, Lane::Typing, |q, db, kept| crate::catalog_summary(db, &q.query)
);
ask!(
    /// How many modules a filter lists.
    CatalogCountAsk { query: CatalogQuery } -> u64, Lane::Typing, |q, db, kept| folia_query::catalog_count(db, &q.query)
);
ask!(
    /// „Ähnliche Module": the semantic search's hits that the filters hold besides the results.
    SimilarAsk { query: CatalogQuery, hits: Vec<String>, limit: usize } -> Vec<CatalogRow>, Lane::Typing, |q, db, kept| crate::similar(db, &q.query, &q.hits, q.limit)
);
ask!(
    /// The program overview.
    ProgramsOverviewAsk {} -> ProgramsData, Lane::Page, |q, db, kept| crate::programs_overview(db)
);
ask!(
    /// Every program, newest PO first, as the pickers offer them.
    ProgramsAsk {} -> Vec<Program>, Lane::Page, |q, db, kept| folia_query::programs(db)
);
ask!(
    /// A module's page; `None` for a module the catalog lacks.
    ModuleAsk { id: String } -> Option<ModuleData>, Lane::Page, |q, db, kept| crate::module(db, &q.id)
);
ask!(
    /// A program's page; `None` for a program the catalog lacks.
    ProgramAsk { slug: String } -> Option<ProgramData>, Lane::Page, |q, db, kept| crate::program(db, &q.slug)
);
ask!(
    /// The Merkliste.
    BookmarksAsk { ids: Vec<String>, sort: BookmarkSort, descending: bool } -> BookmarksData, Lane::Page, |q, db, kept| crate::bookmarks(db, &q.ids, q.sort, q.descending)
);
ask!(
    /// A semester of the Stundenplan, its modules named by `program`'s abbreviations where given.
    StudyplanAsk { key: SemesterKey, ids: Vec<String>, program: Option<String>, locale: Locale } -> StudyplanData, Lane::Page, |q, db, kept| crate::studyplan_in(db, q.key, &q.ids, q.program.as_deref(), q.locale)
);
ask!(
    /// The rows of planned modules, and the ids the catalog lacks.
    StudyplanModulesAsk { ids: Vec<String> } -> (Vec<CatalogRow>, Vec<String>), Lane::Page, |q, db, kept| crate::studyplan_modules(db, &q.ids)
);
ask!(
    /// The plans of a program, for „Mein Studiengang" and the import.
    PlanSourceAsk { program_id: String, locale: Locale } -> Option<PlanSource>, Lane::Page, |q, db, kept| crate::plan_source(db, &q.program_id, q.locale)
);
ask!(
    /// The program of a stored id, or of its family.
    MyProgramAsk { program_id: String } -> Option<MyProgramInfo>, Lane::Page, |q, db, kept| crate::my_program(db, &q.program_id)
);
ask!(
    /// „Passt in meinen Stundenplan" for a plan.
    FitAsk { filter: FitsFilter, plan: Vec<String>, selection: Selection, locale: Locale } -> FitResult, Lane::Typing, |q, db, kept| crate::fit(db, &q.filter, &q.plan, &q.selection, &mut kept.candidates, q.locale)
);
ask!(
    /// The overlay of a module on the plan of its semester (`crate::overlay`): the plan of `key`
    /// with `others` (the module left out), named by `program`.
    OverlayAsk { key: SemesterKey, others: Vec<String>, program: Option<String>, module_id: String, selection: Selection, locale: Locale } -> Overlay, Lane::Page, |q, db, kept| {
        let plan = crate::studyplan_in(db, q.key, &q.others, q.program.as_deref(), q.locale)?;
        crate::overlay(db, &plan, &q.module_id, &q.selection)
    }
);
ask!(
    /// What a shared plan's code names.
    SharedPlanAsk { plan: SharedPlan, locale: Locale } -> Option<SharedPlanData>, Lane::Page, |q, db, kept| crate::shared_plan(db, &q.plan, q.locale)
);
ask!(
    /// The snapshot's facts: its dates, its current semester.
    MetaAsk {} -> Meta, Lane::Page, |q, db, kept| folia_query::meta(db)
);
ask!(
    /// The snapshot's semesters and a module's Termine, for „Einplanen" from a list (`swipe`).
    ModuleSemestersAsk { id: String } -> (Vec<Semester>, Vec<folia_model::rows_detail::EventDate>), Lane::Page, |q, db, kept| Ok((folia_query::semesters(db)?, folia_query::module_schedule(db, &q.id)?))
);
ask!(
    /// A page of the catalog's list as the virtual list loads it: `limit` rows from `offset`.
    CatalogRowsAsk { query: CatalogQuery, offset: u64, limit: u64 } -> folia_model::rows::CatalogPage, Lane::Page, |q, db, kept| folia_query::catalog_page(db, &q.query, q.offset, q.limit)
);
ask!(
    /// Where a module stands in a list (0-based), `None` where the list does not hold it.
    CatalogPositionAsk { query: CatalogQuery, id: String } -> Option<u64>, Lane::Page, |q, db, kept| folia_query::catalog_position(db, &q.query, &q.id)
);
ask!(
    /// The dated rows (teaching and exams) in a semester of `own` and of `others`: what a module
    /// brings into a plan that the plan's other modules do not.
    PlanRowsAsk { key: SemesterKey, own: Vec<String>, others: Vec<String> } -> (Vec<DateRow>, Vec<DateRow>), Lane::Page, |q, db, kept| {
        let key = q.key.key();
        let rows = |ids: &[String]| -> Result<Vec<DateRow>, DbError> { Ok([folia_query::modules_schedule(db, ids, &key)?, folia_query::modules_exams(db, ids, &key)?].concat()) };
        Ok((rows(&q.own)?, rows(&q.others)?))
    }
);
