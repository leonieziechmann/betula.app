//! A semester of the Studienplan handed on as a link: `/studyplan?share=<code>`.
//!
//! What a visitor plans lives in their browser (R20). Handing it on is the visitor's own act
//! („Link zum Teilen kopieren" in the Stundenplan's sidebar), and the link has two readers: whoever
//! opens it, whose Stundenplan offers to take the modules over, and the server of the messenger it
//! is sent through, which fetches the page's link preview without a cookie and without the
//! browser's storage. The owner asked on 2026-09-26 for that preview to show the plan's modules
//! („MIT-1, AuP, EEG"): so the address carries them, as a calendar subscription's does
//! (`subscription`), and the server resolves them from the active snapshot for the page's tags and
//! its card (`/cards/studyplan/<code>.png`) and keeps nothing.
//!
//! A code carries as little as that takes: the semester, the planned modules in the order they
//! were planned (their tones, on the page as on the card) and the program whose abbreviations name
//! them (the plan's, else „Mein Studiengang"). Nothing of what is hidden or chosen, no
//! placeholders, no Standort. Folia's own log writes the page's path without its query, and the
//! card's path as one fixed text (`redacted_path`); the edge's access log keeps the address like
//! every address. The privacy notice says so („Stundenplan teilen" in app/src/pages/legal.rs): a
//! field added here is a word added there.
//!
//! A code outlives releases like a calendar's (a link in a chat is opened weeks later): it names
//! the layout of its fields in four bits (`VERSION`), and the struct keeps pack's rule for lasting
//! codes within its layout (fields only appended, zero meaning absent).

use serde::{Deserialize, Serialize};

use super::select::MAX_MODULES;
use super::semester::SemesterKey;
use crate::url::{is_program_id, STUDYPLAN};

/// The kind of every code of a shared plan. Frozen: it is part of every code's check characters,
/// so a code of another kind (a calendar's, the Merkliste's) is never read as a shared plan.
pub const KIND: &str = "studyplan";

/// The layout of `SharedPlan` that codes are written in.
pub const VERSION: u8 = 1;

/// The most characters of a code: `MAX_MODULES` modules in any order and a program id take about
/// 150. The reader checks it before decoding, and the writer refuses a longer code.
pub const MAX_CODE: usize = 256;

/// The Stundenplan's parameter that carries a code.
pub const PARAM: &str = "share";

/// Where the picture of a shared plan's link preview lives: `/cards/studyplan/<code>.png`.
pub const CARD_PREFIX: &str = "/cards/studyplan/";

const CARD_SUFFIX: &str = ".png";

/// What Folia's access log writes for every path under `CARD_PREFIX`, valid or not.
const REDACTED: &str = "/cards/studyplan/….png";

/// One semester of a Studienplan as a link hands it on.
///
/// FROZEN LAYOUT `VERSION` (pack/src/lib.rs): fields only appended, zero = absent; never reorder,
/// retype, remove.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedPlan {
    /// `SemesterKey::index()`: year * 2 + 1 for a winter (2026W = 4053). 0 is refused.
    pub semester: u16,
    /// The planned modules by their numeric ids, in the order they were planned.
    #[serde(with = "pack::list")]
    pub modules: Vec<u32>,
    /// The program whose abbreviations name the modules (`program.id`, `079-82-2008`). `None`, and
    /// an id of another shape (`url::is_program_id`), name each module by its own.
    pub program: Option<String>,
}

impl SharedPlan {
    /// The plan of the semester `key` with the modules `ids` in their order: those with a numeric
    /// id (every module of the catalog has one), each once, at most `MAX_MODULES`. `None` when not
    /// one is left: there is nothing to hand on.
    pub fn of(key: SemesterKey, ids: &[String], program: Option<&str>) -> Option<SharedPlan> {
        let mut modules: Vec<u32> = Vec::new();
        for id in ids {
            let Some(number) = id.parse::<u32>().ok().filter(|number| number.to_string() == *id) else { continue };
            if !modules.contains(&number) && modules.len() < MAX_MODULES {
                modules.push(number);
            }
        }
        (!modules.is_empty()).then(|| SharedPlan { semester: key.index(), modules, program: program.filter(|id| is_program_id(id)).map(str::to_string) })
    }

    /// The code of the plan, at most `MAX_CODE` characters of `pack::ALPHABET`; `Err` for a plan
    /// no reader takes (no module, a semester outside 2000–2099, too many modules), so no code
    /// comes out that `from_code` would refuse.
    pub fn code(&self) -> Result<String, pack::Error> {
        let code = pack::to_versioned_code(KIND, VERSION, self)?;
        if code.len() > MAX_CODE {
            return Err(pack::Error::TooLong);
        }
        if Self::from_code(&code).is_none() {
            return Err(pack::Error::Malformed);
        }
        Ok(code)
    }

    /// The plan of a code, or `None` for anything that is not one: the wrong length or
    /// characters, failed check characters, another kind or layout, a field this build does not
    /// know, a semester outside 2000–2099, no module, more than `MAX_MODULES`, one twice. A program
    /// that is no program id reads as none.
    pub fn from_code(code: &str) -> Option<SharedPlan> {
        // The cheap checks first: every page address with the parameter comes through here.
        let shaped = (1..=MAX_CODE).contains(&code.len()) && code.bytes().all(|byte| pack::ALPHABET.as_bytes().contains(&byte));
        if !shaped {
            return None;
        }
        let mut plan: SharedPlan = pack::from_versioned_code(KIND, VERSION, code).ok()?;
        SemesterKey::from_index(plan.semester)?;
        let mut seen = plan.modules.clone();
        seen.sort_unstable();
        seen.dedup();
        if !(1..=MAX_MODULES).contains(&plan.modules.len()) || seen.len() != plan.modules.len() {
            return None;
        }
        plan.program = plan.program.filter(|id| is_program_id(id));
        Some(plan)
    }

    /// The semester, or `None` for an index no key has.
    pub fn key(&self) -> Option<SemesterKey> {
        SemesterKey::from_index(self.semester)
    }

    /// The modules' ids as the catalog writes them, in the order they were planned.
    pub fn module_ids(&self) -> Vec<String> {
        self.modules.iter().map(u32::to_string).collect()
    }
}

/// The Stundenplan's address that hands the plan of `code` on.
pub fn path(code: &str) -> String {
    format!("{STUDYPLAN}?{PARAM}={code}")
}

/// The address of the picture of a code's link preview.
pub fn card_path(code: &str) -> String {
    format!("{CARD_PREFIX}{code}{CARD_SUFFIX}")
}

/// The code of a card's file name (`<code>.png`, as axum's `Path` hands it over), when it has the
/// shape; whether it decodes is `SharedPlan::from_code`'s question.
pub fn code_of_card(file: &str) -> Option<&str> {
    file.strip_suffix(CARD_SUFFIX).filter(|code| (1..=MAX_CODE).contains(&code.len()))
}

/// The path as Folia's access log writes it: every path under `CARD_PREFIX` becomes
/// `/cards/studyplan/….png`, valid or not, because a code names the modules someone plans. Every
/// other path is left as it is (a page's query, where the Stundenplan's code travels, is never
/// written).
pub fn redacted_path(path: &str) -> &str {
    if path.starts_with(CARD_PREFIX) {
        REDACTED
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn winter() -> SemesterKey {
        SemesterKey::parse("2026W").unwrap()
    }

    fn ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| id.to_string()).collect()
    }

    #[test]
    fn a_plan_travels_in_its_order_with_its_program() {
        let plan = SharedPlan::of(winter(), &ids(&["12104", "11101", "13849", "12107"]), Some("079-82-2008")).unwrap();
        let code = plan.code().unwrap();
        assert!(code.len() < 40, "{code}");
        let back = SharedPlan::from_code(&code).unwrap();
        assert_eq!(back, plan);
        assert_eq!(back.module_ids(), ["12104", "11101", "13849", "12107"], "the order is the plan's");
        assert_eq!((back.key(), back.program.as_deref()), (Some(winter()), Some("079-82-2008")));
        assert_eq!(path(&code), format!("/studyplan?share={code}"));
        assert_eq!(card_path(&code), format!("/cards/studyplan/{code}.png"));
        assert_eq!(code_of_card(&format!("{code}.png")), Some(code.as_str()));
    }

    #[test]
    fn what_a_code_cannot_carry_is_left_out() {
        // Ids that are no numbers, twice or past the cap: left out; nothing left, no plan.
        let plan = SharedPlan::of(winter(), &ids(&["12104", "12104", "x-1", "0012104", "11101"]), Some("not a program")).unwrap();
        assert_eq!((plan.module_ids(), plan.program), (ids(&["12104", "11101"]), None));
        assert!(SharedPlan::of(winter(), &ids(&["x-1"]), None).is_none());
        let many: Vec<String> = (10_000..10_100).map(|id| id.to_string()).collect();
        let plan = SharedPlan::of(winter(), &many, None).unwrap();
        assert_eq!(plan.modules.len(), MAX_MODULES);
        assert!(plan.code().unwrap().len() <= MAX_CODE);
    }

    #[test]
    fn anything_else_is_no_plan() {
        let code = SharedPlan::of(winter(), &ids(&["12104", "11101"]), None).unwrap().code().unwrap();
        for wrong in [String::new(), "abc".to_string(), format!("{code}x"), code[1..].to_string(), format!("{code}/"), "~".repeat(MAX_CODE + 1)] {
            assert!(SharedPlan::from_code(&wrong).is_none(), "{wrong}");
        }
        // Another kind with the same fields, another layout, a module twice, no module.
        let calendar = pack::to_versioned_code("calendar", VERSION, &SharedPlan::of(winter(), &ids(&["12104"]), None).unwrap()).unwrap();
        let later = pack::to_versioned_code(KIND, VERSION + 1, &SharedPlan::of(winter(), &ids(&["12104"]), None).unwrap()).unwrap();
        let twice = pack::to_versioned_code(KIND, VERSION, &SharedPlan { semester: winter().index(), modules: vec![12104, 12104], program: None }).unwrap();
        let none = pack::to_versioned_code(KIND, VERSION, &SharedPlan { semester: winter().index(), modules: vec![], program: None }).unwrap();
        let no_semester = pack::to_versioned_code(KIND, VERSION, &SharedPlan { semester: 0, modules: vec![12104], program: None }).unwrap();
        for code in [calendar, later, twice, none, no_semester] {
            assert!(SharedPlan::from_code(&code).is_none(), "{code}");
        }
    }

    #[test]
    fn the_log_never_writes_a_card_of_a_shared_plan() {
        let code = SharedPlan::of(winter(), &ids(&["12104"]), None).unwrap().code().unwrap();
        assert_eq!(redacted_path(&card_path(&code)), "/cards/studyplan/….png");
        assert_eq!(redacted_path("/cards/studyplan/anything"), "/cards/studyplan/….png");
        assert_eq!(redacted_path("/cards/module/12104.png"), "/cards/module/12104.png");
        assert_eq!(redacted_path("/cards/studyplan.png"), "/cards/studyplan.png");
    }
}
