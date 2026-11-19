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
//! were planned (their tones, on the page as on the card), the program whose abbreviations name
//! them (the plan's, else „Mein Studiengang") and, since 2026-10-26, the groups ticked, so that a
//! plan handed on can stand as a calendar of one's own with its owner's groups (owner, 2026-10-26:
//! Lena's calendar „aus Lenas Link · mit ihren Gruppen"); since 2026-11-03 the changes of Termine of
//! QIS too, as far as the code holds them, for the same calendar (folia/design/kalender/
//! termine.html: „sonst stimmt der Kalender bei Lena nicht"); since 2026-11-19 what is hidden and
//! the Standort, as far as the code holds them beside the rest (owner: „wenn ich bei mir ein
//! modul/veranstaltung ausblende dann sollte das auch bei der person, die das ding aboniert hat
//! ausgeblendet werden. Beide sollen den gleichen kalender sehen."). No note, no Termin of one's
//! own, no placeholders. Folia's own log writes the page's path without its query, and the
//! card's path as one fixed text (`redacted_path`); the edge's access log keeps the address like
//! every address. The privacy notice says so („Stundenplan teilen" in folia/crates/home/src/legal.rs): a
//! field added here is a word added there.
//!
//! A code outlives releases like a calendar's (a link in a chat is opened weeks later): it names
//! the layout of its fields in four bits (`VERSION`), and the struct keeps pack's rule for lasting
//! codes within its layout (fields only appended, zero meaning absent).

use folia_model::ids::is_program_id;
use serde::{Deserialize, Serialize};

use crate::change::{Change, MAX_ROOM};
use crate::kind::KindSet;
use crate::rowkey::RowKey;
use crate::select::{Selection, TownChoice, MAX_HIDDEN, MAX_MODULES};
use crate::semester::SemesterKey;
use crate::subscription::{date_of, date_value, CodeChange};

/// The kind of every code of a shared plan. Frozen: it is part of every code's check characters,
/// so a code of another kind (a calendar's, the Merkliste's) is never read as a shared plan.
pub const KIND: &str = "studyplan";

/// The layout of `SharedPlan` that codes are written in.
pub const VERSION: u8 = 1;

/// The most characters of a code: `MAX_MODULES` modules in any order and a program id take about
/// 150. The reader checks it before decoding, and the writer refuses a longer code.
pub const MAX_CODE: usize = 256;

/// The most groups a code carries; fewer where the code would be longer than `MAX_CODE`
/// (`SharedPlan::with_chosen`).
pub const MAX_CHOSEN: usize = 60;

/// The most changes of Termine of QIS a code carries; fewer where the code would be longer than
/// `MAX_CODE` (`SharedPlan::with_changes`).
pub const MAX_CHANGES: usize = 20;

/// The Stundenplan's parameter that carries a code.
pub const PARAM: &str = "share";

/// Where the picture of a shared plan's link preview lives: `/cards/studyplan/<code>.png`.
pub const CARD_PREFIX: &str = "/cards/studyplan/";

const CARD_SUFFIX: &str = ".png";

/// What Folia's access log writes for every path under `CARD_PREFIX`, valid or not.
const REDACTED: &str = "/cards/studyplan/….png";

/// One semester of a Studienplan as a link hands it on.
///
/// FROZEN LAYOUT `VERSION` (folia/crates/pack/src/lib.rs): fields only appended, zero = absent; never reorder,
/// retype, remove.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedPlan {
    /// `SemesterKey::index()`: year * 2 + 1 for a winter (2026W = 4053). 0 is refused.
    pub semester: u16,
    /// The planned modules by their numeric ids, in the order they were planned.
    #[serde(with = "folia_pack::list")]
    pub modules: Vec<u32>,
    /// The program whose abbreviations name the modules (`program.id`, `079-82-2008`). `None`, and
    /// an id of another shape (`url::is_program_id`), name each module by its own.
    pub program: Option<String>,
    /// The groups ticked in the semester (`RowKey::packed`), at most `MAX_CHOSEN`. Empty in a code
    /// written before 2026-10-26.
    #[serde(with = "folia_pack::set")]
    pub chosen: Vec<u64>,
    /// The changes of Termine of QIS in the semester, as a calendar's code carries them
    /// (`CodeChange`), at most `MAX_CHANGES`. Empty in a code written before 2026-11-03; a build
    /// before it refuses a code with changes, which only a rollback meets.
    pub changes: Vec<CodeChange>,
    /// What is hidden in the semester, as a calendar's code carries it (`Subscription`'s fields of
    /// the same names), as far as the code holds it beside the rest (`SharedPlan::with_hidden`), at
    /// most `MAX_HIDDEN` of each list. Empty in a code written before 2026-11-19; a build before it
    /// refuses a code with any of them, which only a rollback meets.
    pub hidden_kinds: u16,
    #[serde(with = "folia_pack::set")]
    pub hidden_events: Vec<u32>,
    #[serde(with = "folia_pack::set")]
    pub hidden_rows: Vec<u64>,
    #[serde(with = "folia_pack::set")]
    pub hidden_dates: Vec<u64>,
    /// The Standort (`TownChoice::code`); 0 derives it from the modules, as before 2026-11-19.
    pub town: u8,
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
        (!modules.is_empty()).then(|| SharedPlan { semester: key.index(), modules, program: program.filter(|id| is_program_id(id)).map(str::to_string), ..SharedPlan::default() })
    }

    /// The plan with the groups `rows` ticked, as many of them as a code carries: the first
    /// `MAX_CHOSEN` by their keys, and of those fewer where the code would grow longer than
    /// `MAX_CODE` — a plan of many modules hands on fewer of its ticks rather than nothing.
    pub fn with_chosen(self, rows: impl IntoIterator<Item = RowKey>) -> SharedPlan {
        let mut chosen: Vec<u64> = rows.into_iter().filter(|row| row.event > 0).map(RowKey::packed).collect();
        chosen.sort_unstable();
        chosen.dedup();
        chosen.truncate(MAX_CHOSEN);
        while !chosen.is_empty() {
            let plan = SharedPlan { chosen: chosen.clone(), ..self.clone() };
            if plan.code().is_ok() {
                return plan;
            }
            chosen.pop();
        }
        SharedPlan { chosen, ..self }
    }

    /// The groups ticked, by their keys.
    pub fn chosen_rows(&self) -> Vec<RowKey> {
        self.chosen.iter().filter_map(|value| RowKey::unpack(*value)).collect()
    }

    /// The plan with the changes `changes` of its semester's Termine of QIS, as many as the code
    /// carries beside its groups: the first `MAX_CHANGES` by Termin and scope, each that still
    /// fits in `MAX_CODE` (one with a long room may not, a later one may).
    pub fn with_changes<'a>(self, changes: impl IntoIterator<Item = &'a Change>) -> SharedPlan {
        let Some(key) = self.key() else { return self };
        let mut list: Vec<CodeChange> = changes.into_iter().filter_map(|change| CodeChange::of(key, change)).collect();
        list.sort_unstable();
        list.dedup();
        list.truncate(MAX_CHANGES);
        let mut plan = SharedPlan { changes: Vec::new(), ..self };
        for change in list {
            let mut more = plan.clone();
            more.changes.push(change);
            if more.code().is_ok() {
                plan = more;
            }
        }
        plan
    }

    /// The changes it carries, made to hold whatever QIS says (`CodeChange::change`); one a reader
    /// does not know is left out.
    pub fn change_list(&self) -> Vec<Change> {
        self.key().map(|key| self.changes.iter().filter_map(|change| change.change(key)).collect()).unwrap_or_default()
    }

    /// The plan with what `selection` hides of its semester and its Standort, as far as the code
    /// carries them beside its modules, groups and changes (owner, 2026-11-19: „Beide sollen den
    /// gleichen kalender sehen"): the kinds and the Standort, then of the events, the Termine and
    /// the dates each as many as still fit, by their keys; with the number of what was left out. A
    /// date outside the semester's half-year is none of its dates and counts for nothing. The
    /// caller hands what its timetable still knows (`folia_timetable::export::subscription_of`), so
    /// that the code spends nothing on a Termin QIS has taken back.
    pub fn with_hidden(self, selection: &Selection) -> (SharedPlan, usize) {
        let Some(key) = self.key() else { return (self, 0) };
        let events: Vec<u32> = selection.hidden_events.iter().copied().take(MAX_HIDDEN).collect();
        let rows: Vec<u64> = selection.hidden_rows.iter().map(|row| row.packed()).take(MAX_HIDDEN).collect();
        let dates: Vec<u64> = selection.hidden_dates.iter().filter_map(|(row, day)| date_value(key, *row, *day)).take(MAX_HIDDEN).collect();
        let whole = events.len() + rows.len() + dates.len();
        let mut plan = SharedPlan { hidden_kinds: selection.hidden_kinds.known().0, town: selection.town.code(), hidden_events: Vec::new(), hidden_rows: Vec::new(), hidden_dates: Vec::new(), ..self };
        if plan.code().is_err() {
            plan.hidden_kinds = 0;
            plan.town = 0;
        }
        let events = fitting(&plan, &events, |plan, list| plan.hidden_events = list);
        plan.hidden_events = events;
        let rows = fitting(&plan, &rows, |plan, list| plan.hidden_rows = list);
        plan.hidden_rows = rows;
        let dates = fitting(&plan, &dates, |plan, list| plan.hidden_dates = list);
        plan.hidden_dates = dates;
        let left = whole - plan.hidden_events.len() - plan.hidden_rows.len() - plan.hidden_dates.len();
        let lost = usize::from(plan.hidden_kinds != selection.hidden_kinds.known().0) + usize::from(plan.town != selection.town.code());
        (plan, left + lost)
    }

    /// What it hides and its Standort, as the timetable applies it; what this build does not know
    /// is left out.
    pub fn hidden(&self) -> Selection {
        let rows = |packed: &[u64]| packed.iter().filter_map(|value| RowKey::unpack(*value)).collect();
        Selection {
            hidden_kinds: KindSet(self.hidden_kinds).known(),
            hidden_events: self.hidden_events.iter().copied().collect(),
            hidden_rows: rows(&self.hidden_rows),
            hidden_dates: self.key().map(|key| self.hidden_dates.iter().filter_map(|value| date_of(key, *value)).collect()).unwrap_or_default(),
            town: TownChoice::from_code(self.town),
            ..Selection::default()
        }
    }

    /// The code of the plan, at most `MAX_CODE` characters of `folia_pack::ALPHABET`; `Err` for a plan
    /// no reader takes (no module, a semester outside 2000–2099, too many modules), so no code
    /// comes out that `from_code` would refuse.
    pub fn code(&self) -> Result<String, folia_pack::Error> {
        let code = folia_pack::to_versioned_code(KIND, VERSION, self)?;
        if code.len() > MAX_CODE {
            return Err(folia_pack::Error::TooLong);
        }
        if Self::from_code(&code).is_none() {
            return Err(folia_pack::Error::Malformed);
        }
        Ok(code)
    }

    /// The plan of a code, or `None` for anything that is not one: the wrong length or
    /// characters, failed check characters, another kind or layout, a field this build does not
    /// know, a semester outside 2000–2099, no module, more than `MAX_MODULES`, one twice, more than
    /// `MAX_CHOSEN` groups or one that is no key, more than `MAX_CHANGES` changes or one with a room
    /// longer than a change's, more than `MAX_HIDDEN` in a list of what is hidden. A program that is
    /// no program id reads as none; kind bits no kind has, and a Standort this build does not know,
    /// as a subscription reads them (`Subscription::from_code`).
    pub fn from_code(code: &str) -> Option<SharedPlan> {
        // The cheap checks first: every page address with the parameter comes through here.
        let shaped = (1..=MAX_CODE).contains(&code.len()) && code.bytes().all(|byte| folia_pack::ALPHABET.as_bytes().contains(&byte));
        if !shaped {
            return None;
        }
        let mut plan: SharedPlan = folia_pack::from_versioned_code(KIND, VERSION, code).ok()?;
        SemesterKey::from_index(plan.semester)?;
        let mut seen = plan.modules.clone();
        seen.sort_unstable();
        seen.dedup();
        if !(1..=MAX_MODULES).contains(&plan.modules.len()) || seen.len() != plan.modules.len() {
            return None;
        }
        if plan.chosen.len() > MAX_CHOSEN || plan.chosen.iter().any(|value| RowKey::unpack(*value).is_none()) {
            return None;
        }
        if plan.changes.len() > MAX_CHANGES || plan.changes.iter().any(|change| change.room.as_ref().is_some_and(|room| room.chars().count() > MAX_ROOM)) {
            return None;
        }
        if [plan.hidden_events.len(), plan.hidden_rows.len(), plan.hidden_dates.len()].iter().any(|len| *len > MAX_HIDDEN) {
            return None;
        }
        plan.program = plan.program.filter(|id| is_program_id(id));
        plan.hidden_kinds = KindSet(plan.hidden_kinds).known().0;
        plan.town = TownChoice::from_code(plan.town).code();
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

/// How long a beginning of `items` `put` sets into `plan` while its code still fits in
/// `MAX_CODE`: the longest, found by halving (a longer beginning never makes a shorter code).
fn fitting<T: Clone>(plan: &SharedPlan, items: &[T], put: fn(&mut SharedPlan, Vec<T>)) -> Vec<T> {
    let fits = |n: usize| {
        let mut tried = plan.clone();
        put(&mut tried, items.iter().take(n).cloned().collect());
        tried.code().is_ok()
    };
    if fits(items.len()) {
        return items.to_vec();
    }
    let (mut fit, mut over) = (0, items.len());
    while over - fit > 1 {
        let middle = (fit + over) / 2;
        if fits(middle) {
            fit = middle;
        } else {
            over = middle;
        }
    }
    items.iter().take(fit).cloned().collect()
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
/// `/cards/studyplan/….png`, valid or not, because a code names the modules someone plans. So
/// does every path that has it further on: the card in English (`/en/cards/studyplan/…`), and
/// what is no address of the site but reaches the log all the same (`/de/cards/studyplan/…` is
/// redirected). Every other path is left as it is (a page's query, where the Stundenplan's code
/// travels, is never written).
pub fn redacted_path(path: &str) -> &str {
    if path.contains(CARD_PREFIX) {
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
        assert_eq!(card_path(&code), format!("/cards/studyplan/{code}.png"));
        assert_eq!(code_of_card(&format!("{code}.png")), Some(code.as_str()));
    }

    #[test]
    fn the_groups_ticked_travel_with_it() {
        let rows = [RowKey { event: 148_370, fp: 0xeb017 }, RowKey { event: 148_304, fp: 0x467cf }, RowKey { event: 148_370, fp: 0xeb017 }];
        let plan = SharedPlan::of(winter(), &ids(&["12104", "12102", "11112"]), Some("079-82-2008")).unwrap().with_chosen(rows);
        let code = plan.code().unwrap();
        let back = SharedPlan::from_code(&code).unwrap();
        assert_eq!(back.chosen_rows(), [RowKey { event: 148_304, fp: 0x467cf }, RowKey { event: 148_370, fp: 0xeb017 }], "each once, by key");
        assert!(code.len() < 60, "{code}");
        // A code written before the groups came reads with none; one with them the same plan.
        let before = SharedPlan::of(winter(), &ids(&["12104", "12102", "11112"]), Some("079-82-2008")).unwrap();
        let old = before.code().unwrap();
        assert_eq!(SharedPlan::from_code(&old).map(|plan| plan.chosen_rows()), Some(vec![]));
        assert_eq!(SharedPlan { chosen: vec![], ..back }, before);
        // A plan of every module there may be keeps as many of its groups as the code holds.
        let many: Vec<String> = (10_000..10_000 + MAX_MODULES as u32).map(|id| id.to_string()).collect();
        let rows: Vec<RowKey> = (0..80).map(|n| RowKey { event: 140_000 + n * 997, fp: (n * 7919) & 0xfffff }).collect();
        let full = SharedPlan::of(winter(), &many, Some("079-82-2008")).unwrap().with_chosen(rows.clone());
        assert!(full.code().is_ok_and(|code| code.len() <= MAX_CODE));
        assert!(full.chosen.len() < MAX_CHOSEN, "{} groups", full.chosen.len());
        let small = SharedPlan::of(winter(), &ids(&["12104"]), None).unwrap().with_chosen(rows);
        assert!(small.chosen.len() > full.chosen.len() && small.code().is_ok_and(|code| code.len() <= MAX_CODE), "{} groups", small.chosen.len());
        // A real plan's groups lie close together: a semester's events are numbered in a row.
        let near: Vec<RowKey> = (0..MAX_CHOSEN as u32).map(|n| RowKey { event: 148_300 + n * 3, fp: (n * 7919) & 0xfffff }).collect();
        let fs1 = SharedPlan::of(winter(), &ids(&["12104", "12107", "12102", "11112"]), Some("079-82-2008")).unwrap().with_chosen(near);
        assert!(fs1.chosen.len() >= 20 && fs1.code().is_ok(), "{} groups", fs1.chosen.len());
    }

    /// VL EvS in another room, and its Übung of 04.11. on the Thursday at 10:00: what a calendar
    /// made of the link shows as the plan's owner sees it.
    #[test]
    fn the_changes_travel_with_it() {
        use crate::change::{Scope, Values};
        use crate::day::Day;
        let room = Change {
            key: RowKey { event: 148_701, fp: 0xb7025 },
            scope: Scope::All,
            weekday: None,
            time: None,
            room: Some("ZHG/HS.A".into()),
            was: Values { weekday: 5, from: 555, to: 645, room: "ZHG/HS.C".into() },
            module: "12104".into(),
        };
        let day = Change { key: RowKey { event: 148_369, fp: 0xa4d12 }, scope: Scope::On(Day::parse("2026-11-04").unwrap()), weekday: Some(4), time: Some((600, 690)), room: None, ..room.clone() };
        let plain = SharedPlan::of(winter(), &ids(&["12104", "12102", "11112"]), Some("079-82-2008")).unwrap().with_chosen([RowKey { event: 148_304, fp: 0x467cf }]);
        let plan = plain.clone().with_changes([&day, &room, &day]);
        assert_eq!(plan.changes.len(), 2, "each once");
        let code = plan.code().unwrap();
        assert!(code.len() < 100, "{} characters: {code}", code.len());
        let back = SharedPlan::from_code(&code).unwrap();
        assert_eq!(back, plan);
        let changes = back.change_list();
        assert_eq!(changes.iter().map(|change| (change.key, change.scope, change.weekday, change.time, change.room.clone())).collect::<Vec<_>>(), [
            (day.key, day.scope, Some(4), Some((600, 690)), None),
            (room.key, room.scope, None, None, Some("ZHG/HS.A".to_string())),
        ]);
        // Without changes a code is what it was before them.
        assert_eq!(SharedPlan { changes: vec![], ..back }.code().unwrap(), plain.code().unwrap());
        // As many as the code holds beside every module there may be and their groups.
        let many: Vec<String> = (10_000..10_000 + MAX_MODULES as u32).map(|id| id.to_string()).collect();
        let rooms: Vec<Change> = (0..40).map(|n| Change { key: RowKey { event: 140_000 + n * 997, fp: (n * 7919) & 0xfffff }, room: Some(format!("Raum {n} mit langem Namen")), ..room.clone() }).collect();
        let full = SharedPlan::of(winter(), &many, Some("079-82-2008")).unwrap().with_changes(&rooms);
        assert!(full.code().is_ok_and(|code| code.len() <= MAX_CODE) && full.changes.len() < MAX_CHANGES, "{} changes", full.changes.len());
        let small = SharedPlan::of(winter(), &ids(&["12104"]), None).unwrap().with_changes(&rooms);
        assert!(small.changes.len() > full.changes.len() && small.code().is_ok_and(|code| code.len() <= MAX_CODE), "{} changes", small.changes.len());
        // More than the cap, or a room longer than a change's: no plan.
        let one = SharedPlan::of(winter(), &ids(&["12104"]), None).unwrap();
        let small_key = |n: u32| CodeChange { key: RowKey { event: 1, fp: n }.packed(), weekday: 1, ..CodeChange::default() };
        let at_cap = folia_pack::to_versioned_code(KIND, VERSION, &SharedPlan { changes: (0..MAX_CHANGES as u32).map(small_key).collect(), ..one.clone() }).unwrap();
        let over = folia_pack::to_versioned_code(KIND, VERSION, &SharedPlan { changes: (0..=MAX_CHANGES as u32).map(small_key).collect(), ..one.clone() }).unwrap();
        let long = folia_pack::to_versioned_code(KIND, VERSION, &SharedPlan { changes: vec![CodeChange { room: Some("x".repeat(MAX_ROOM + 1)), ..small_key(1) }], ..one }).unwrap();
        assert!(over.len() <= MAX_CODE && long.len() <= MAX_CODE, "{} and {} characters", over.len(), long.len());
        assert!(SharedPlan::from_code(&at_cap).is_some());
        assert!(SharedPlan::from_code(&over).is_none() && SharedPlan::from_code(&long).is_none());
    }

    /// Max's link hides what Max hides (owner, 2026-11-19: „Beide sollen den gleichen kalender
    /// sehen"): his Tutorien, the Sachsendorf lecture, an Übung and the Übung of 04.11., in Cottbus.
    #[test]
    fn what_is_hidden_travels_with_it() {
        use crate::day::Day;
        use crate::kind::EventKind;
        use crate::select::Town;
        let day = |iso: &str| Day::parse(iso).unwrap();
        let uebung = RowKey { event: 148_369, fp: 0xa4d12 };
        let hidden = Selection {
            hidden_kinds: KindSet::default().with(EventKind::Tutorial),
            hidden_events: [149_408].into(),
            hidden_rows: [RowKey { event: 148_369, fp: 0xaaf38 }].into(),
            // A day outside the half-year is none of its dates.
            hidden_dates: [(uebung, day("2026-11-04")), (uebung, day("2027-04-14"))].into(),
            town: TownChoice::Only(Town::Cottbus),
            ..Selection::default()
        };
        let plain = SharedPlan::of(winter(), &ids(&["12104", "12102", "11112"]), Some("079-82-2008")).unwrap().with_chosen([RowKey { event: 148_304, fp: 0x467cf }]);
        let (plan, left) = plain.clone().with_hidden(&hidden);
        assert_eq!(left, 0);
        let code = plan.code().unwrap();
        assert!(code.len() < 80, "{} characters: {code}", code.len());
        let back = SharedPlan::from_code(&code).unwrap();
        assert_eq!(back, plan);
        assert_eq!(back.hidden(), Selection { hidden_dates: [(uebung, day("2026-11-04"))].into(), ..hidden.clone() });
        assert_eq!(back.chosen_rows(), plain.chosen_rows());
        // Nothing hidden, the Standort derived: the code is what it was before the fields.
        assert_eq!(plain.clone().with_hidden(&Selection::default()).0.code().unwrap(), plain.code().unwrap());
        // As much as the code holds beside every module there may be: the events first, then the
        // Termine, then the dates, and what did not fit is counted.
        let many: Vec<String> = (10_000..10_000 + MAX_MODULES as u32).map(|id| id.to_string()).collect();
        let lots = Selection {
            hidden_events: (149_000..149_100).collect(),
            hidden_rows: (0..100).map(|n| RowKey { event: 148_000 + n, fp: (n * 7919) & 0xfffff }).collect(),
            hidden_dates: (0..100).map(|n| (RowKey { event: 147_000 + n, fp: n }, day("2026-11-04"))).collect(),
            ..hidden.clone()
        };
        let (full, left) = SharedPlan::of(winter(), &many, Some("079-82-2008")).unwrap().with_hidden(&lots);
        assert!(full.code().is_ok_and(|code| code.len() <= MAX_CODE));
        assert_eq!(left, 300 - full.hidden_events.len() - full.hidden_rows.len() - full.hidden_dates.len());
        assert!(left > 0 && !full.hidden_events.is_empty() && full.hidden_dates.is_empty(), "{} events, {} Termine", full.hidden_events.len(), full.hidden_rows.len());
        assert_eq!((full.hidden_kinds, full.town), (EventKind::Tutorial.bit(), 1));
        let (small, small_left) = SharedPlan::of(winter(), &ids(&["12104"]), None).unwrap().with_hidden(&lots);
        assert!(small_left < left && small.code().is_ok_and(|code| code.len() <= MAX_CODE), "{small_left} left out");
        // A reader takes kinds and a Standort a newer build may write as a subscription does, and
        // refuses a list past its cap.
        let one = SharedPlan::of(winter(), &ids(&["12104"]), None).unwrap();
        let newer = folia_pack::to_versioned_code(KIND, VERSION, &SharedPlan { hidden_kinds: 1 << 13 | EventKind::Tutorial.bit(), town: 9, ..one.clone() }).unwrap();
        assert_eq!(SharedPlan::from_code(&newer).map(|plan| (plan.hidden_kinds, plan.town)), Some((EventKind::Tutorial.bit(), 0)));
        let over = folia_pack::to_versioned_code(KIND, VERSION, &SharedPlan { hidden_events: (1..=MAX_HIDDEN as u32 + 1).collect(), ..one }).unwrap();
        assert!(over.len() <= MAX_CODE, "{} characters: the cap, not the length, must refuse it", over.len());
        assert!(SharedPlan::from_code(&over).is_none());
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
        let calendar = folia_pack::to_versioned_code("calendar", VERSION, &SharedPlan::of(winter(), &ids(&["12104"]), None).unwrap()).unwrap();
        let later = folia_pack::to_versioned_code(KIND, VERSION + 1, &SharedPlan::of(winter(), &ids(&["12104"]), None).unwrap()).unwrap();
        let twice = folia_pack::to_versioned_code(KIND, VERSION, &SharedPlan { semester: winter().index(), modules: vec![12104, 12104], program: None, chosen: vec![], changes: vec![], ..SharedPlan::default() }).unwrap();
        let none = folia_pack::to_versioned_code(KIND, VERSION, &SharedPlan { semester: winter().index(), modules: vec![], program: None, chosen: vec![], changes: vec![], ..SharedPlan::default() }).unwrap();
        let no_semester = folia_pack::to_versioned_code(KIND, VERSION, &SharedPlan { semester: 0, modules: vec![12104], program: None, chosen: vec![], changes: vec![], ..SharedPlan::default() }).unwrap();
        let no_key = folia_pack::to_versioned_code(KIND, VERSION, &SharedPlan { semester: winter().index(), modules: vec![12104], program: None, chosen: vec![0xfffff], changes: vec![], ..SharedPlan::default() }).unwrap();
        for code in [calendar, later, twice, none, no_semester, no_key] {
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

    #[test]
    fn the_log_never_writes_a_card_of_a_shared_plan_in_any_language() {
        let code = SharedPlan::of(winter(), &ids(&["12104"]), None).unwrap().code().unwrap();
        let english = folia_locale::Locale::En.path(&card_path(&code));
        assert_eq!(english, format!("/en/cards/studyplan/{code}.png"));
        for path in [english, "/en/cards/studyplan/anything".to_string(), format!("/de/cards/studyplan/{code}.png"), format!("/en/en/cards/studyplan/{code}.png")] {
            assert_eq!(redacted_path(&path), "/cards/studyplan/….png", "{path}");
        }
        for kept in ["/en/cards/module/12104.png", "/en/cards/studyplan.png", "/en/studyplan", "/en"] {
            assert_eq!(redacted_path(kept), kept);
        }
    }
}
