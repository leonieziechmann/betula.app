//! „Mein Studium"'s study as the visitor keeps it: from „Mein Studiengang" (`MineDoc`), the plan
//! (`PlanDoc`), the program's plans (`PlanSource`) and the catalog's rows, what `study::study`
//! works it out from (`Setup`, `input_of`). Every page that speaks of the study takes it from here
//! — „Mein Studium" itself, the start page's „Mein Studium" panel and its card — and says its
//! numbers by `Study::totals`, so that two pages never disagree (owner, 2026-11-09: „Versuch da mal
//! ein system zu finden, dasss immer konsistent mit der berechnung von mein studium ist"). Until
//! Folia 1.85.1 the start page put the input together on its own and summed up by itself.

use std::collections::BTreeSet;

use folia_calendar::semester::SemesterKey;
use folia_model::rows::CatalogRow;
use folia_pages::PlanSource;
use folia_plans::study::Input;
use folia_plans::studyplan::{self as stored, MineDoc, PlanDoc};

use crate::myprogram::ProgramPlans;

/// What `study::study` is asked with, besides the plan and the rows.
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    /// The plan shown (an index into the program's plans): a core plan or a page of one.
    pub shown: Option<usize>,
    /// The core plan, and the page that fills a row of it with that row's `ord`.
    pub core: Option<usize>,
    pub page: Option<(usize, i64)>,
    pub start: SemesterKey,
    /// The Studienbeginn is stored; else it is assumed (`assumed_start`), and „Mein Studium" asks
    /// for it first.
    pub start_stored: bool,
    /// The current semester.
    pub now: SemesterKey,
    pub leave: BTreeSet<SemesterKey>,
    pub until: Option<SemesterKey>,
}

impl Setup {
    /// The setup of what „Mein Studiengang" keeps for `source`, in the semester `now`.
    pub fn of(source: &PlanSource, mine: &MineDoc, now: SemesterKey) -> Self {
        let (shown, core, page) = chosen(source, mine.caption.as_deref(), mine.direction.as_deref());
        Setup {
            shown,
            core,
            page,
            start: mine.start.unwrap_or_else(|| assumed_start(source, core, now)),
            start_stored: mine.start.is_some(),
            now,
            leave: mine.leave.clone(),
            until: mine.until,
        }
    }
}

/// What the study is worked out from: the program's plans as `setup` chose them, the plan, and the
/// catalog's rows of its modules (`rows_of`).
pub fn input_of<'a>(source: &'a PlanSource, setup: &'a Setup, doc: &'a PlanDoc, rows: &'a [CatalogRow]) -> Input<'a> {
    Input {
        program_id: &source.program.id,
        core: setup.core.and_then(|core| source.variants.get(core)),
        page: setup.page.and_then(|(page, ord)| Some((source.variants.get(page)?, ord))),
        start: setup.start,
        now: setup.now,
        leave: &setup.leave,
        until: setup.until,
        doc,
        rows,
        tree: &source.areas,
        placements: &source.placements,
        fues: &source.fues,
    }
}

/// The plan „Mein Studiengang" keeps among the program's plans (by its caption and direction, as
/// `ProgramPlans` reads them), else the first; with its core plan and the page of it.
pub fn chosen(source: &PlanSource, caption: Option<&str>, direction: Option<&str>) -> (Option<usize>, Option<usize>, Option<(usize, i64)>) {
    let Some(last) = source.variants.len().checked_sub(1) else { return (None, None, None) };
    let plans = ProgramPlans::new(&source.variants, source.supplements.clone());
    let shown = caption.and_then(|caption| plans.shown_index(caption, direction)).unwrap_or(0).min(last);
    match source.supplements.iter().find(|supplement| supplement.page == shown) {
        Some(supplement) => (Some(shown), Some(supplement.core), Some((supplement.page, supplement.ord))),
        None => (Some(shown), Some(shown), None),
    }
}

/// The Studienbeginn a study of `source` is assumed to have begun in: the current semester, or the
/// one before where the plan begins in the other half of the year.
pub fn assumed_start(source: &PlanSource, core: Option<usize>, now: SemesterKey) -> SemesterKey {
    let intake = core.and_then(|core| source.variants.get(core)).and_then(|core| stored::intake_season(core, &source.linked));
    stored::intake_start(now, intake)
}

/// The modules of the plan whose catalog rows the study needs beyond the program's own
/// (`PlanSource::linked`): what is planned, visited and passed, each once.
pub fn module_ids(doc: &PlanDoc) -> Vec<String> {
    let mut ids: Vec<String> = doc.modules.iter().map(|m| m.module_id.clone()).chain(doc.visits.iter().map(|v| v.module_id.clone())).chain(doc.passed.iter().map(|p| p.module_id.clone())).collect();
    ids.sort();
    ids.dedup();
    ids
}

/// The catalog's rows of `source`'s modules and of `extra` (the answer for `module_ids`), each
/// once: the program's own first.
pub fn rows_of(source: &PlanSource, extra: &[CatalogRow]) -> Vec<CatalogRow> {
    source.linked.iter().chain(extra.iter().filter(|row| !source.linked.iter().any(|linked| linked.id == row.id))).cloned().collect()
}
