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
use serde::de::DeserializeOwned;
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

/// What `DataError::pending` says: no error, the answer is on its way.
const PENDING: &str = "the answer is on its way";

impl DataError {
    /// The answer is on its way (the data worker has not answered yet): a page shows what it
    /// showed before, or nothing, never an error.
    pub fn pending() -> Self {
        Self { unavailable: true, message: PENDING.to_string() }
    }

    pub fn is_pending(&self) -> bool {
        self.unavailable && self.message == PENDING
    }

    /// `now`, or while it is pending the answer before it, where there was one: what is shown stays
    /// until the new answer is there (docs/folia/folia-refactor.md §6.4).
    pub fn or_before<T: Clone>(now: Result<T, DataError>, before: Option<&Result<T, DataError>>) -> Result<T, DataError> {
        match (&now, before) {
            (Err(error), Some(before)) if error.is_pending() => before.clone(),
            _ => now,
        }
    }
}

/// What a memo makes of an answer, `make`, unless the answer is on its way and the memo had a
/// value before: then that value stays (what is shown stays until the new answer is there).
pub fn unless_pending<A, T: Clone>(now: Result<A, DataError>, before: Option<&T>, make: impl FnOnce(Result<A, DataError>) -> T) -> T {
    match (&now, before) {
        (Err(error), Some(before)) if error.is_pending() => before.clone(),
        _ => make(now),
    }
}

impl From<DbError> for DataError {
    fn from(error: DbError) -> Self {
        Self { unavailable: matches!(error, DbError::Unavailable(_)), message: error.to_string() }
    }
}

/// What kind of wait a question is (§6.3). Every question is answered, in turn: „newest wins" by
/// kind would drop the question of another part of the page that asks the same kind at the same
/// time, and the steps of what is typed are dropped before they ask (`Pending::typed`). The lane
/// says which questions follow the keys, for a worker that answers them differently one day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// What a page shows.
    Page,
    /// What follows the keys or a slider.
    Typing,
}

/// What the side that answers keeps between questions besides their answers: the finder's
/// candidates, which do not change with the plan (`folia_timetable::fit::CandidateSet`).
#[derive(Default)]
pub struct Kept {
    pub candidates: Option<CandidateSet>,
}

/// A question of a page, its answer, and the loader that answers it.
pub trait Ask: Clone + Debug + PartialEq + Serialize + DeserializeOwned + Send + Sync + 'static {
    type Answer: Clone + Debug + PartialEq + Serialize + DeserializeOwned + Send + Sync + 'static;
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

/// Answers the question named `name` (an `Ask::NAME`) whose fields `question` holds in JSON, with
/// the JSON of its `Result<Answer, DataError>`: what the data worker does with every message.
/// `None` for a name no question has.
pub fn answer_json(name: &str, question: &str, db: &dyn Database, kept: &mut Kept) -> Option<String> {
    fn answer<A: Ask>(question: &str, db: &dyn Database, kept: &mut Kept) -> String {
        let answer: Result<A::Answer, DataError> = serde_json::from_str::<A>(question)
            .map_err(|error| DataError { unavailable: false, message: format!("{}: {error}", A::NAME) })
            .and_then(|ask| ask.run(db, kept).map_err(DataError::from));
        serde_json::to_string(&answer).unwrap_or_default()
    }
    macro_rules! dispatch {
        ($($ask:ident),* $(,)?) => {
            match name {
                $(n if n == $ask::NAME => Some(answer::<$ask>(question, db, kept)),)*
                _ => None,
            }
        };
    }
    dispatch!(
        GroundAsk, HomeAsk, CatalogChoicesAsk, CatalogAsk, CatalogSummaryAsk, CatalogCountAsk, SimilarAsk, ProgramsOverviewAsk,
        ProgramsAsk, ModuleAsk, ProgramAsk, BookmarksAsk, StudyplanAsk, StudyplanModulesAsk, PlanSourceAsk, MyProgramAsk, FitAsk,
        OverlayAsk, SharedPlanAsk, MetaAsk, ModuleSemestersAsk, CatalogRowsAsk, CatalogPositionAsk, PlanRowsAsk,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use folia_test_support::open;

    /// What the data worker answers (`answer_json`) is what the question answers where it is asked
    /// (`Ask::run`), read back as the page reads it: every question crosses in JSON and comes back
    /// whole.
    fn same<A: Ask>(ask: A) {
        let db = open();
        let here = ask.run(&db, &mut Kept::default()).map_err(DataError::from);
        let json = answer_json(A::NAME, &serde_json::to_string(&ask).unwrap(), &db, &mut Kept::default()).unwrap_or_else(|| panic!("{} is not answered", A::NAME));
        let there: Result<A::Answer, DataError> = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{}: {e}", A::NAME));
        assert_eq!(here, there, "{}", A::NAME);
    }

    #[test]
    fn the_worker_answers_what_the_page_would() {
        let url = CatalogUrl { query: CatalogQuery { text: "Analysis".into(), ..Default::default() }, page: 1, open: None, fill: None };
        same(CatalogAsk { url, locale: Locale::En });
        same(GroundAsk {});
        same(ModuleAsk { id: "11103".into() });
        same(ModuleAsk { id: "00000".into() });
        same(ProgramAsk { slug: "bachelor-informatik-2008".into() });
        same(BookmarksAsk { ids: vec!["11103".into(), "12104".into()], sort: BookmarkSort::Added, descending: false });
        let key = SemesterKey::parse("2026W").unwrap();
        same(StudyplanAsk { key, ids: vec!["12104".into(), "12107".into()], program: Some("079-82-2008".into()), locale: Locale::De });
        same(PlanRowsAsk { key, own: vec!["12104".into()], others: vec!["12107".into()] });
        same(MetaAsk {});
        assert_eq!(answer_json("NoSuchAsk", "{}", &open(), &mut Kept::default()), None);
    }
}
