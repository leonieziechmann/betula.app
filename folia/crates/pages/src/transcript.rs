//! What „Mein Studium" takes over of a Notenübersicht (`folia_transcript`; owner, 2026-11-18:
//! „Nur einmal die module extrahieren+status und dann datei vergessen"): the program its sentence
//! names, as the catalog has it, and the catalog's module of each of its rows. The data worker is
//! asked with the rows' titles and credits alone (`TranscriptAsk`), never their status or anything
//! of the student, and matches them in Rust (`folia_transcript::matching`) over every module's
//! titles, which its statement reads without a parameter from the Notenübersicht: the worker keeps
//! the statements' answers (`folia-worker`), and none of them holds a part of it.

use std::collections::BTreeSet;

use folia_model::db::{Database, DbError};
use folia_model::labels::OfferStatus;
use folia_model::rows::Program;
use folia_query as queries;
use folia_routes::filter::ProgramRelation;
use folia_transcript::matching::{fold, Candidate, Matcher};
use serde::{Deserialize, Serialize};

/// The most rows matched at once: as many as a Notenübersicht is read with.
const MAX_ROWS: usize = folia_transcript::table::MAX_ENTRIES;

/// A row of the Notenübersicht as it is matched: its two titles and its credits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscriptRow {
    pub title: String,
    pub other: Option<String>,
    pub credits: Option<f64>,
}

/// The catalog's module of a row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatchedModule {
    pub id: String,
    /// Its title in the catalog.
    pub title: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TranscriptMatch {
    /// The program the sentence names, by its name and its degree: the one „Mein Studiengang" keeps
    /// where it is that program (another PO of it too), else its newest PO; `None` where the catalog
    /// has none of that name and degree, or the sentence names none.
    pub program: Option<Program>,
    /// The module of each row, in the order of the rows asked; `None` where the catalog has none
    /// of its titles.
    pub modules: Vec<Option<MatchedModule>>,
}

/// The degree levels the sentence's word for the degree („Bachelor-Studiengang") stands for.
fn levels(degree: &str) -> &'static [&'static str] {
    match fold(degree).as_str() {
        "bachelor" => &["bachelor", "teaching_bachelor"],
        "master" => &["master", "teaching_master"],
        _ => &[],
    }
}

/// The program and the modules of a Notenübersicht: `mine` the program „Mein Studiengang" keeps,
/// `degree` and `name` what the sentence above the table names.
pub fn transcript(db: &dyn Database, mine: Option<&str>, degree: Option<&str>, name: Option<&str>, rows: &[TranscriptRow]) -> Result<TranscriptMatch, DbError> {
    let programs = queries::programs(db)?;
    let fits = |program: &Program| {
        let named = name.is_some_and(|name| fold(&program.name) == fold(name));
        let level = degree.map(levels).filter(|levels| !levels.is_empty()).is_none_or(|levels| levels.contains(&program.degree_level.code()));
        named && level
    };
    let program = mine
        .and_then(|id| programs.iter().find(|program| program.id == id && fits(program)))
        .or_else(|| programs.iter().filter(|program| fits(program)).max_by_key(|program| (program.is_latest_po, program.has_plan, program.po_year)))
        .cloned();
    // The modules the program claims come first; where the sentence names none the catalog has,
    // the kept program's.
    let own = program.as_ref().map(|program| program.id.clone()).or_else(|| mine.map(str::to_string));
    let mut claimed: BTreeSet<String> = BTreeSet::new();
    if let Some(id) = &own {
        for relation in [ProgramRelation::Curricular, ProgramRelation::Fues] {
            claimed.extend(queries::program_modules(db, id, relation)?.into_iter().map(|module| module.module_id));
        }
    }
    let candidates = queries::module_titles(db)?
        .into_iter()
        .map(|module| Candidate {
            claimed: claimed.contains(&module.id),
            offered: module.offer_status.known() == Some(OfferStatus::Active),
            id: module.id,
            title: module.title,
            title_de: module.title_de,
            title_en: module.title_en,
            credits: module.credits,
        })
        .collect();
    let matcher = Matcher::new(candidates);
    let modules = rows
        .iter()
        .take(MAX_ROWS)
        .map(|row| matcher.find(&row.title, row.other.as_deref(), row.credits).map(|found| MatchedModule { id: found.id.clone(), title: found.title.clone() }))
        .collect();
    Ok(TranscriptMatch { program, modules })
}

#[cfg(test)]
mod tests {
    use super::*;
    use folia_test_support::open;

    fn rows(transcript: &folia_transcript::Transcript) -> Vec<TranscriptRow> {
        transcript.entries.iter().map(|entry| TranscriptRow { title: entry.title.clone(), other: entry.other.clone(), credits: entry.credits }).collect()
    }

    /// The made-up Notenübersicht (`folia_transcript::fixture`) against the real catalog: the
    /// program by its name and degree, every module by its titles, a title of many (the
    /// Bachelor-Arbeit, 68 of them) the program's own, the module taught in English by its first
    /// line.
    #[test]
    fn the_made_up_notenuebersicht_in_the_catalog() {
        let db = open();
        let read = folia_transcript::read(&folia_transcript::fixture::example()).unwrap();
        let found = transcript(&db, None, read.degree.as_deref(), read.program.as_deref(), &rows(&read)).unwrap();
        assert_eq!(found.program.as_ref().map(|program| program.id.as_str()), Some("079-82-2008"));
        let ids: Vec<Option<&str>> = found.modules.iter().map(|module| module.as_ref().map(|module| module.id.as_str())).collect();
        assert_eq!(
            ids,
            [
                Some("11787"),
                Some("12101"),
                Some("12102"),
                Some("12107"),
                Some("11112"),
                Some("11113"),
                Some("12333"),
                Some("12330"),
                Some("14085"),
                Some("14460"),
                Some("11684"),
                Some("12472")
            ]
        );
        // The Master of the same name is another program; a name the catalog lacks, none.
        let master = transcript(&db, None, Some("Master"), Some("Informatik"), &[]).unwrap();
        assert_eq!(master.program.as_ref().map(|program| program.id.as_str()), Some("079-88-2008"));
        assert_eq!(transcript(&db, None, Some("Bachelor"), Some("Kein Studiengang"), &[]).unwrap().program, None);
    }
}
