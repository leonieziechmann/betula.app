//! What „Mein Studium" takes over of a Notenübersicht (`folia_transcript`; owner, 2026-11-18:
//! „Nur einmal die module extrahieren+status und dann datei vergessen"): the program its sentence
//! names, as the catalog has it, and the catalog's module of each of its rows. The data worker is
//! asked with the rows' titles and credits alone (`TranscriptAsk`), and with what the sentence names
//! of the study (the program, its degree and profile, the semester it began in), never the rows'
//! status or anything of the student, and matches them in Rust (`folia_transcript::matching`) over
//! every module's titles, which its statement reads without a parameter from the Notenübersicht: the
//! worker keeps the statements' answers (`folia-worker`), and none of them holds a part of it.

use std::collections::BTreeMap;

use folia_model::db::{Database, DbError};
use folia_model::labels::OfferStatus;
use folia_model::rows::Program;
use folia_query as queries;
use folia_routes::filter::ProgramRelation;
use folia_transcript::matching::{fold, Asked, Candidate, Matcher};
use folia_transcript::Term;
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

/// Whether the profile the sentence names („universitäres Profil", „anwendungsbezogenes Profil") is
/// the one of a program's degree as the catalog writes it („Bachelor (universitär)").
fn same_profile(profile: &str, degree: &str) -> bool {
    let Some(word) = degree.split_once('(').and_then(|(_, rest)| rest.split(')').next()).map(fold).filter(|word| !word.is_empty()) else { return false };
    fold(profile).split(' ').next().is_some_and(|first| first.starts_with(&word))
}

/// The program and the modules of a Notenübersicht: `mine` the program „Mein Studiengang" keeps,
/// `degree`, `name` and `profile` what the sentence above the table names, `since` the semester
/// the study began in („2022W").
///
/// The program: the one kept where it is the one named (another PO of it too); else of the POs of
/// that name, degree and profile the newest in force when the study began (a PO of the year Y from
/// the winter semester Y/Y+1 on: a study begun in 2019 is of the PO 2018, not of the one of 2022;
/// owner, 2026-11-19, the Notenübersicht of a friend), the oldest where none was yet, the newest
/// without a beginning. Its modules are the rows' first (its curriculum and its FÜS list), then
/// those of its other POs, profiles and variants.
pub fn transcript(db: &dyn Database, mine: Option<&str>, degree: Option<&str>, name: Option<&str>, profile: Option<&str>, since: Option<&str>, rows: &[TranscriptRow]) -> Result<TranscriptMatch, DbError> {
    let programs = queries::programs(db)?;
    let fits = |program: &Program| {
        let named = name.is_some_and(|name| fold(&program.name) == fold(name));
        let level = degree.map(levels).filter(|levels| !levels.is_empty()).is_none_or(|levels| levels.contains(&program.degree_level.code()));
        named && level
    };
    let fitting: Vec<&Program> = programs.iter().filter(|program| fits(program)).collect();
    let profiled: Vec<&Program> = fitting.iter().copied().filter(|program| profile.is_some_and(|profile| same_profile(profile, &program.degree_raw))).collect();
    let chosen = if profiled.is_empty() { fitting.clone() } else { profiled };
    let since = since.and_then(Term::from_code);
    let in_force = |program: &Program| since.is_some_and(|since| program.po_year.and_then(|year| u16::try_from(year).ok()).is_some_and(|year| Term { year, winter: true } <= since));
    let program = mine
        .and_then(|id| fitting.iter().find(|program| program.id == id))
        .or_else(|| match since {
            Some(_) => chosen
                .iter()
                .filter(|program| in_force(program))
                .max_by_key(|program| (program.po_year, program.study_variant.is_none(), program.is_latest_po, program.has_plan))
                .or_else(|| chosen.iter().min_by_key(|program| (program.po_year, program.study_variant.is_some()))),
            None => chosen.iter().max_by_key(|program| (program.is_latest_po, program.has_plan, program.po_year)),
        })
        .map(|program| (*program).clone());
    // The modules the program claims come first, then its other POs'; where the sentence names
    // none the catalog has, the kept program's.
    let own = program.as_ref().map(|program| program.id.clone()).or_else(|| mine.map(str::to_string));
    let mut claimed: BTreeMap<String, u8> = BTreeMap::new();
    for (id, rank) in fitting.iter().filter(|program| Some(&program.id) != own.as_ref()).map(|program| (program.id.as_str(), 1)).chain(own.as_deref().map(|id| (id, 2))) {
        for relation in [ProgramRelation::Curricular, ProgramRelation::Fues] {
            for module in queries::program_modules(db, id, relation)? {
                let entry = claimed.entry(module.module_id).or_insert(rank);
                *entry = (*entry).max(rank);
            }
        }
    }
    let candidates = queries::module_titles(db)?
        .into_iter()
        .map(|module| Candidate {
            claimed: claimed.get(&module.id).copied().unwrap_or(0),
            offered: module.offer_status.known() == Some(OfferStatus::Active),
            id: module.id,
            title: module.title,
            title_de: module.title_de,
            title_en: module.title_en,
            credits: module.credits,
        })
        .collect();
    let matcher = Matcher::new(candidates);
    let asked: Vec<Asked> = rows.iter().take(MAX_ROWS).map(|row| (row.title.as_str(), row.other.as_deref(), row.credits)).collect();
    let modules = matcher.assign(&asked).into_iter().map(|found| found.map(|found| MatchedModule { id: found.id.clone(), title: found.title.clone() })).collect();
    Ok(TranscriptMatch { program, modules })
}

#[cfg(test)]
mod tests {
    use super::*;
    use folia_test_support::open;
    use folia_transcript::matching::key;

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
        let since = read.since.map(|since| since.term().code());
        let found = transcript(&db, None, read.degree.as_deref(), read.program.as_deref(), read.profile.as_deref(), since.as_deref(), &rows(&read)).unwrap();
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
        let master = transcript(&db, None, Some("Master"), Some("Informatik"), None, None, &[]).unwrap();
        assert_eq!(master.program.as_ref().map(|program| program.id.as_str()), Some("079-88-2008"));
        assert_eq!(transcript(&db, None, Some("Bachelor"), Some("Kein Studiengang"), None, None, &[]).unwrap().program, None);
    }

    /// The PO by the profile and the beginning the sentence names: Elektrotechnik B.Sc. begun in
    /// 2020 is of the PO 2019, begun in 2023 of the PO 2022, before every PO of the oldest;
    /// „anwendungsbezogenes Profil" is the other program of that name.
    #[test]
    fn the_po_of_the_beginning_and_the_profile() {
        let db = open();
        let po = |profile: Option<&str>, since: Option<&str>| transcript(&db, None, Some("Bachelor"), Some("Elektrotechnik"), profile, since, &[]).unwrap().program.map(|program| program.id);
        let uni = Some("universitäres Profil");
        assert_eq!(po(uni, Some("2020W")).as_deref(), Some("048-82-2019"));
        assert_eq!(po(uni, Some("2023S")).as_deref(), Some("048-82-2022"));
        assert_eq!(po(uni, Some("2022S")).as_deref(), Some("048-82-2019"));
        assert_eq!(po(uni, Some("2010W")).as_deref(), Some("048-82-2019"));
        assert_eq!(po(Some("anwendungsbezogenes Profil"), Some("2023W")).as_deref(), Some("048-84-2018"));
        assert_eq!(po(uni, None).as_deref(), Some("048-82-2022"));
    }

    /// A module's row as the BTU writes it: its title in the language it is taught in, under it
    /// the other's, its credits.
    fn written(module: &folia_model::rows::ModuleTitle) -> (String, String, Option<f64>) {
        let german = module.title_de.clone().filter(|title| !title.trim().is_empty());
        let english = module.title_en.clone().filter(|title| !title.trim().is_empty());
        let other = if german.as_deref() == Some(module.title.as_str()) { english } else { german.or(english) };
        let other = other.filter(|other| fold(other) != fold(&module.title)).unwrap_or_default();
        (module.title.clone(), other, module.credits)
    }

    /// Every module of the catalog in a Notenübersicht as the BTU writes it (`fixture`: its
    /// column, its lines, its pages), read back as it was written (owner, 2026-11-19: „eine
    /// freundin meinte, dass zwei ihrer module nicht importiert wurden. Kannst du das mal mit allen
    /// modulen simulieren"): no row lost, no title changed („Kosten-" over „und Leistungsrechnung"
    /// was read „Kosten-und …"), the credits as they stand.
    #[test]
    fn every_module_through_the_pdf() {
        use folia_transcript::fixture::{self, Row};
        let db = open();
        let modules = queries::module_titles(&db).unwrap();
        let leak = |text: String| -> &'static str { Box::leak(text.into_boxed_str()) };
        let credits_text = |credits: Option<f64>| credits.map(|c| if c.fract() == 0.0 { format!("{c:.0}") } else { format!("{c}").replace('.', ",") }).unwrap_or_default();
        let mut wrong: Vec<String> = Vec::new();
        for chunk in modules.chunks(folia_transcript::table::MAX_ENTRIES - 100) {
            let rows: Vec<Row> = chunk
                .iter()
                .map(|module| {
                    let (title, other, credits) = written(module);
                    Row::Module { title: leak(title), other: leak(other), values: ["", "BE", leak(credits_text(credits)), "1", "WS 2024/25", "", ""] }
                })
                .collect();
            let read = folia_transcript::read(&fixture::pdf(&fixture::paginate(rows), fixture::Options::default())).unwrap();
            assert_eq!(read.entries.len(), chunk.len(), "rows lost in the PDF of the modules from {}", chunk[0].id);
            for (module, entry) in chunk.iter().zip(&read.entries) {
                let (title, other, credits) = written(module);
                if fold(&entry.title) != fold(&title) || fold(entry.other.as_deref().unwrap_or_default()) != fold(&other) || entry.credits != credits {
                    wrong.push(format!("{}: „{title}“ / „{other}“ ({credits:?}) read „{}“ / „{}“ ({:?})", module.id, entry.title, entry.other.as_deref().unwrap_or_default(), entry.credits));
                }
            }
        }
        assert!(wrong.is_empty(), "{} of {} modules read otherwise than written:\n{}", wrong.len(), modules.len(), wrong.join("\n"));
    }

    /// Every program's modules (its curriculum and its FÜS list) in a Notenübersicht that names the
    /// program as the BTU's sentence does, its profile and the winter its PO began with: each row a
    /// module of its own, every one found; with „Mein Studiengang" the program, each as itself or as
    /// its twin, which no row tells apart from it (the same titles and credits: „Technische
    /// Thermodynamik" twice in G30-P2-2025); until 2026-11-19 the six „Seminar" of the Informatik
    /// Master were all the first, and taking them over kept one. Without it the program by its PO
    /// year and profile, or a variant of it of that year (a double degree, a dual or an extended
    /// one, which the sentence does not tell apart), each as itself or as the module of its titles
    /// that PO has.
    #[test]
    fn every_module_of_every_program_found() {
        let db = open();
        let modules = queries::module_titles(&db).unwrap();
        let by_id: BTreeMap<&str, &folia_model::rows::ModuleTitle> = modules.iter().map(|module| (module.id.as_str(), module)).collect();
        let (mut rows_asked, mut elsewhere) = (0, 0);
        let mut wrong: Vec<String> = Vec::new();
        for program in queries::programs(&db).unwrap() {
            let mut ids: Vec<String> = Vec::new();
            for relation in [ProgramRelation::Curricular, ProgramRelation::Fues] {
                ids.extend(queries::program_modules(&db, &program.id, relation).unwrap().into_iter().map(|module| module.module_id));
            }
            ids.sort();
            ids.dedup();
            ids.retain(|id| by_id.contains_key(id.as_str()));
            let rows: Vec<TranscriptRow> = ids
                .iter()
                .filter_map(|id| by_id.get(id.as_str()))
                .map(|module| {
                    let (title, other, credits) = written(module);
                    TranscriptRow { title, other: Some(other).filter(|other| !other.is_empty()), credits }
                })
                .collect();
            let degree = match program.degree_level.code() {
                "bachelor" | "teaching_bachelor" => Some("Bachelor"),
                "master" | "teaching_master" => Some("Master"),
                _ => None,
            };
            // „Bachelor (universitär)": „universitäres Profil".
            let profile = program.degree_raw.split_once('(').and_then(|(_, rest)| rest.split(')').next()).map(|word| format!("{word}es Profil"));
            let since = program.po_year.map(|year| format!("{year}W"));
            for mine in [Some(program.id.as_str()), None] {
                let found = transcript(&db, mine, degree, Some(&program.name), profile.as_deref(), since.as_deref(), &rows).unwrap();
                let chosen = found.program.as_ref();
                let fits = chosen.is_some_and(|chosen| chosen.id == program.id || (mine.is_none() && fold(&chosen.name) == fold(&program.name) && chosen.po_year == program.po_year));
                if !fits {
                    wrong.push(format!("{} {} ({mine:?}): the program found is {:?}", program.id, program.name, chosen.map(|chosen| &chosen.id)));
                }
                let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
                for (id, module) in ids.iter().zip(&found.modules) {
                    rows_asked += 1;
                    let Some(module) = module else {
                        wrong.push(format!("{} ({mine:?}): {id} „{}“ found nothing", program.id, by_id.get(id.as_str()).map_or("", |module| module.title.as_str())));
                        continue;
                    };
                    if let Some(before) = seen.insert(&module.id, id) {
                        wrong.push(format!("{} ({mine:?}): {before} and {id} both {}", program.id, module.id));
                    }
                    let twin = |a: &str, b: &str| match (by_id.get(a), by_id.get(b)) {
                        (Some(a), Some(b)) => {
                            let ((ta, oa, ca), (tb, ob, cb)) = (written(a), written(b));
                            key(&ta) == key(&tb) && key(&oa) == key(&ob) && ca == cb
                        }
                        _ => false,
                    };
                    if &module.id != id && !twin(&module.id, id) {
                        if mine.is_some() {
                            wrong.push(format!("{} ({mine:?}): {id} „{}“ found as {} „{}“", program.id, by_id.get(id.as_str()).map_or("", |module| module.title.as_str()), module.id, module.title));
                        } else {
                            elsewhere += 1;
                        }
                    }
                }
            }
        }
        eprintln!("{rows_asked} rows of {} modules; without „Mein Studiengang“ {elsewhere} found as the module of their titles of the PO chosen", modules.len());
        assert!(wrong.is_empty(), "{} rows not found as they should be:\n{}", wrong.len(), wrong.join("\n"));
    }
}
