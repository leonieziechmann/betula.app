//! A calendar handed on that stays current (Betula Sync, step 3; folia/design/teilen/teilen.html,
//! owner 2026-11-08): what its part holds. The part lies at Phloem sealed with a key of its own
//! (docs/phloem/protocol.md §5), every device of the sharer's profile renews it when the calendar
//! changes, and whoever has its link — a friend's Betula that follows it, or Folia for a calendar
//! app's subscription — opens it and shows this.
//!
//! It holds one semester of one calendar (the plan's, „Mein Studiengang", or one of one's own):
//! the modules of its week in their order, the program whose abbreviations name them, the groups
//! ticked and the changes of Termine of QIS. For a calendar app also what is hidden and the town,
//! so that the subscribed calendar is the week the student sees, as a subscription's code carries
//! it (`subscription`); for a person not, as the link of a fixed plan leaves it out (`share`).
//! The Termine of one's own and the notes at Termine only where the share takes them along. No
//! name, no profile, nothing that leads back to the sharer.
//!
//! A part outlives releases as a code does (a friend's Betula of last month reads it): the layout
//! is named in the code (`VERSION`), and the struct keeps pack's rule for lasting codes within it
//! (fields only appended, zero meaning absent). Unlike a link's code a part may hold no module at
//! all: a plan emptied is what the friend then sees.

use folia_model::ids::{is_module_id, is_program_id};
use serde::{Deserialize, Serialize};

use crate::change::{note_text, Change, MAX_NOTE, MAX_ROOM};
use crate::day::Day;
use crate::kind::KindSet;
use crate::own::{OwnRhythm, OwnTermin, MAX_OWN_DATES, MAX_OWN_TEXT};
use crate::rowkey::RowKey;
use crate::select::{Selection, TownChoice, MAX_HIDDEN, MAX_MODULES};
use crate::semester::SemesterKey;
use crate::subscription::{date_of, date_value, CodeChange};

/// The kind of every part's code. Frozen: it is part of the check characters.
pub const KIND: &str = "live";

/// The layout of `LivePart` that codes are written in.
pub const VERSION: u8 = 1;

/// The most characters of a part's code: sealed (a nonce and a tag more) it stays below what
/// Phloem keeps of a part, 16 KiB. A part is a few hundred bytes; notes make it larger.
pub const MAX_CODE: usize = 16 * 1024 - 64;

/// The most Termine of one's own and the most notes a part carries: a semester's caps
/// (`folia_plans::studyplan::MAX_OWN`, `MAX_NOTES`).
pub const MAX_OWN: usize = 100;
pub const MAX_NOTES: usize = 100;

/// One semester of a calendar as its part holds it.
///
/// FROZEN LAYOUT `VERSION` (folia/crates/pack/src/lib.rs): fields only appended, zero = absent;
/// never reorder, retype, remove.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LivePart {
    /// `SemesterKey::index()`. 0 is refused.
    pub semester: u16,
    /// The modules of its week by their numeric ids, planned and visited, in their order.
    #[serde(with = "folia_pack::list")]
    pub modules: Vec<u32>,
    /// The program whose abbreviations name the modules; `None` names each by its own.
    pub program: Option<String>,
    /// The groups ticked (`RowKey::packed`).
    #[serde(with = "folia_pack::set")]
    pub chosen: Vec<u64>,
    /// The changes of Termine of QIS, as a subscription's code carries them.
    pub changes: Vec<CodeChange>,
    /// What is hidden, for a calendar app (`Subscription`'s fields of the same names); empty for a
    /// person.
    pub hidden_kinds: u16,
    #[serde(with = "folia_pack::set")]
    pub hidden_events: Vec<u32>,
    #[serde(with = "folia_pack::set")]
    pub hidden_rows: Vec<u64>,
    #[serde(with = "folia_pack::set")]
    pub hidden_dates: Vec<u64>,
    /// `TownChoice::code`; 0 derives it from the modules.
    pub town: u8,
    /// The modules of `modules` that are only visited: in the week, without their exams.
    #[serde(with = "folia_pack::set")]
    pub visits: Vec<u32>,
    /// The Termine of one's own, where the share takes them along.
    pub own: Vec<CodeOwn>,
    /// The notes at Termine, where the share takes them along.
    pub notes: Vec<CodeNote>,
}

/// A Termin of one's own as a part carries it (`OwnTermin`): its days as their distance from the
/// half-year's first day.
///
/// FROZEN LAYOUT (folia/crates/pack/src/lib.rs): fields only appended, zero = absent.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeOwn {
    /// Its number in the sharer's plan: the key its notes name it by (`OwnTermin::key`).
    pub number: u32,
    /// 0 every week, 1 the A weeks, 2 the B weeks, 3 on dates of its own; another value leaves it
    /// out.
    pub rhythm: u8,
    pub weekday: u8,
    pub dates: Vec<i32>,
    pub from: u16,
    pub to: u16,
    pub period: Option<(i32, i32)>,
    pub module: Option<String>,
    pub name: String,
    pub place: String,
    pub skip: Vec<i32>,
}

/// A note at a Termin (`RowKey::packed`, of QIS or of one's own).
///
/// FROZEN LAYOUT (folia/crates/pack/src/lib.rs): fields only appended, zero = absent.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeNote {
    pub key: u64,
    pub text: String,
}

/// What a share takes along beside the modules, groups and changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Along {
    /// What is hidden and the town: for a calendar app.
    pub hidden: bool,
    /// The Termine of one's own.
    pub own: bool,
    /// The notes at Termine.
    pub notes: bool,
}

impl CodeOwn {
    fn of(key: SemesterKey, termin: &OwnTermin) -> CodeOwn {
        let first = key.bounds().0;
        let offset = |day: &Day| day.0 - first.0;
        CodeOwn {
            number: termin.number,
            rhythm: match termin.rhythm {
                OwnRhythm::Week => 0,
                OwnRhythm::AWeek => 1,
                OwnRhythm::BWeek => 2,
                OwnRhythm::Dates => 3,
            },
            weekday: termin.weekday,
            dates: termin.dates.iter().map(offset).collect(),
            from: termin.from,
            to: termin.to,
            period: termin.period.map(|(a, b)| (offset(&a), offset(&b))),
            module: termin.module.clone(),
            name: termin.name.clone(),
            place: termin.place.clone(),
            skip: termin.skip.iter().map(offset).collect(),
        }
    }

    /// The Termin it carries, of semester `key`; `None` for one that is none here.
    pub fn termin(&self, key: SemesterKey) -> Option<OwnTermin> {
        let first = key.bounds().0;
        let day = |offset: &i32| first.plus(*offset);
        let rhythm = *OwnRhythm::ALL.get(usize::from(self.rhythm))?;
        let termin = OwnTermin {
            number: self.number,
            rhythm,
            weekday: self.weekday,
            dates: self.dates.iter().map(day).collect(),
            from: self.from,
            to: self.to,
            period: self.period.map(|(a, b)| (day(&a), day(&b))),
            module: self.module.clone().filter(|id| is_module_id(id)),
            name: self.name.chars().take(MAX_OWN_TEXT).collect(),
            place: self.place.chars().take(MAX_OWN_TEXT).collect(),
            skip: self.skip.iter().map(day).collect(),
        };
        termin.is_valid().then_some(termin)
    }
}

impl LivePart {
    /// The part of semester `key` of a calendar whose week holds `modules` (planned and visited,
    /// in their order; `visits` the visited ones), named as in `program`, as `selection` shows it,
    /// with what `along` takes along. Only numeric module ids travel, each once, at most
    /// `MAX_MODULES`.
    pub fn of(key: SemesterKey, modules: &[String], visits: &[String], program: Option<&str>, selection: &Selection, along: Along) -> LivePart {
        let numeric = |id: &String| id.parse::<u32>().ok().filter(|number| number.to_string() == *id);
        let mut ids: Vec<u32> = Vec::new();
        for number in modules.iter().filter_map(numeric) {
            if !ids.contains(&number) && ids.len() < MAX_MODULES {
                ids.push(number);
            }
        }
        let visits: Vec<u32> = {
            let mut list: Vec<u32> = visits.iter().filter_map(numeric).filter(|id| ids.contains(id)).collect();
            list.sort_unstable();
            list.dedup();
            list
        };
        let packed = |rows: &std::collections::BTreeSet<RowKey>| rows.iter().map(|row| row.packed()).take(MAX_HIDDEN).collect::<Vec<u64>>();
        let mut part = LivePart {
            semester: key.index(),
            modules: ids,
            program: program.filter(|id| is_program_id(id)).map(str::to_string),
            chosen: packed(&selection.chosen_rows),
            changes: {
                let mut list: Vec<CodeChange> = selection.changes.iter().filter_map(|change| CodeChange::of(key, change)).collect();
                list.sort_unstable();
                list.dedup();
                list.truncate(MAX_HIDDEN);
                list
            },
            visits,
            ..LivePart::default()
        };
        if along.hidden {
            part.hidden_kinds = selection.hidden_kinds.known().0;
            part.hidden_events = selection.hidden_events.iter().copied().take(MAX_HIDDEN).collect();
            part.hidden_rows = packed(&selection.hidden_rows);
            part.hidden_dates = selection.hidden_dates.iter().filter_map(|(row, day)| date_value(key, *row, *day)).take(MAX_HIDDEN).collect();
            part.town = selection.town.code();
        }
        if along.own {
            part.own = selection.own.iter().filter(|termin| termin.is_valid()).take(MAX_OWN).map(|termin| CodeOwn::of(key, termin)).collect();
        }
        if along.notes {
            part.notes = selection
                .notes
                .iter()
                .map(|(row, text)| CodeNote { key: row.packed(), text: note_text(text) })
                .filter(|note| !note.text.is_empty())
                .take(MAX_NOTES)
                .collect();
        }
        part
    }

    /// The part as its code holds it within `MAX_CODE`: the notes go first, from the last one,
    /// then the Termine of one's own; with them, the number of what was left out. A part of
    /// modules, groups and changes alone always fits (the caps keep it below).
    pub fn fitted(mut self) -> (LivePart, usize) {
        let mut left = 0;
        while self.code().is_err() {
            if self.notes.pop().is_some() || self.own.pop().is_some() {
                left += 1;
            } else {
                break;
            }
        }
        (self, left)
    }

    /// The code of the part, at most `MAX_CODE` characters; `Err` for one no reader takes.
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

    /// The part of a code, or `None` for anything that is not one: the wrong length, characters
    /// or check characters, another kind or layout, a field this build does not know, a semester
    /// outside 2000–2099, a list past its cap or a module twice. Within the known fields it reads
    /// what a newer build may write, as a subscription does: unknown kind bits are masked, an
    /// unknown town derives, a program of another shape is none, a Termin or a note that is none
    /// here is left out.
    pub fn from_code(code: &str) -> Option<LivePart> {
        let shaped = (1..=MAX_CODE).contains(&code.len()) && code.bytes().all(|byte| folia_pack::ALPHABET.as_bytes().contains(&byte));
        if !shaped {
            return None;
        }
        let mut part: LivePart = folia_pack::from_versioned_code(KIND, VERSION, code).ok()?;
        SemesterKey::from_index(part.semester)?;
        let mut seen = part.modules.clone();
        seen.sort_unstable();
        seen.dedup();
        let within = part.modules.len() <= MAX_MODULES
            && seen.len() == part.modules.len()
            && [part.chosen.len(), part.changes.len(), part.hidden_events.len(), part.hidden_rows.len(), part.hidden_dates.len()].iter().all(|len| *len <= MAX_HIDDEN)
            && part.own.len() <= MAX_OWN
            && part.notes.len() <= MAX_NOTES
            && part.changes.iter().all(|change| change.room.as_ref().is_none_or(|room| room.chars().count() <= MAX_ROOM))
            && part.own.iter().all(|own| own.dates.len() <= MAX_OWN_DATES && own.skip.len() <= MAX_OWN_DATES && own.name.chars().count() <= MAX_OWN_TEXT && own.place.chars().count() <= MAX_OWN_TEXT)
            && part.notes.iter().all(|note| note.text.chars().count() <= MAX_NOTE);
        if !within {
            return None;
        }
        part.hidden_kinds = KindSet(part.hidden_kinds).known().0;
        part.town = TownChoice::from_code(part.town).code();
        part.program = part.program.filter(|id| is_program_id(id));
        part.visits.retain(|id| seen.binary_search(id).is_ok());
        Some(part)
    }

    pub fn key(&self) -> Option<SemesterKey> {
        SemesterKey::from_index(self.semester)
    }

    /// The modules' ids as the catalog writes them, in their order.
    pub fn module_ids(&self) -> Vec<String> {
        self.modules.iter().map(u32::to_string).collect()
    }

    /// The groups ticked, by their keys.
    pub fn chosen_rows(&self) -> Vec<RowKey> {
        self.chosen.iter().filter_map(|value| RowKey::unpack(*value)).collect()
    }

    /// The changes it carries, made to hold whatever QIS says (`CodeChange::change`).
    pub fn change_list(&self) -> Vec<Change> {
        self.key().map(|key| self.changes.iter().filter_map(|change| change.change(key)).collect()).unwrap_or_default()
    }

    /// The Termine of one's own it carries, with the sharer's numbers.
    pub fn own_termine(&self) -> Vec<OwnTermin> {
        self.key().map(|key| self.own.iter().filter_map(|own| own.termin(key)).collect()).unwrap_or_default()
    }

    /// The notes it carries, by the Termin's key.
    pub fn note_list(&self) -> Vec<(RowKey, String)> {
        self.notes.iter().filter_map(|note| Some((RowKey::unpack(note.key)?, note_text(&note.text)))).filter(|(_, text)| !text.is_empty()).collect()
    }

    /// What its week shows, as the timetable applies it: for a calendar app the semester as the
    /// sharer sees it, for a person what the share takes along.
    pub fn selection(&self) -> Selection {
        let rows = |packed: &[u64]| packed.iter().filter_map(|value| RowKey::unpack(*value)).collect();
        Selection {
            hidden_kinds: KindSet(self.hidden_kinds).known(),
            hidden_events: self.hidden_events.iter().copied().collect(),
            hidden_rows: rows(&self.hidden_rows),
            chosen_rows: rows(&self.chosen),
            hidden_dates: self.key().map(|key| self.hidden_dates.iter().filter_map(|value| date_of(key, *value)).collect()).unwrap_or_default(),
            town: TownChoice::from_code(self.town),
            town_from: None,
            visits: self.visits.iter().map(u32::to_string).collect(),
            own: self.own_termine(),
            changes: self.change_list(),
            notes: self.note_list(),
        }
    }
}

/// The semester a share that stays current shows on `today`, from the semester it was made in
/// (`from`): that one until its half-year is over, then the next, and so on (folia/design/teilen:
/// „bei „Bleibt aktuell“ geht die Freigabe mit, wenn das nächste Semester beginnt"). Never back.
pub fn following(from: SemesterKey, today: Day) -> SemesterKey {
    let mut key = from;
    while today > key.bounds().1 {
        match key.plus(1) {
            Some(next) => key = next,
            None => break,
        }
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::change::{Scope, Values};
    use crate::kind::EventKind;

    fn winter() -> SemesterKey {
        SemesterKey::parse("2026W").unwrap()
    }

    fn day(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|id| id.to_string()).collect()
    }

    fn arbeit() -> OwnTermin {
        OwnTermin { number: 7, rhythm: OwnRhythm::Week, weekday: 5, dates: vec![], from: 13 * 60, to: 17 * 60, period: None, module: None, name: "Arbeit".into(), place: "Café Mosaik".into(), skip: vec![day("2026-12-25")] }
    }

    fn block() -> OwnTermin {
        OwnTermin { number: 9, rhythm: OwnRhythm::Dates, weekday: 0, dates: vec![day("2026-09-28"), day("2026-11-14")], from: 9 * 60, to: 16 * 60, period: None, module: Some("12104".into()), name: "Blockseminar".into(), place: String::new(), skip: vec![] }
    }

    fn selection() -> Selection {
        let room = Change {
            key: RowKey { event: 148_701, fp: 0xb7025 },
            scope: Scope::From(day("2026-11-06")),
            weekday: None,
            time: None,
            room: Some("HG 1".into()),
            was: Values { weekday: 5, from: 555, to: 645, room: "ZHG/HS.C".into() },
            module: "12104".into(),
        };
        Selection {
            hidden_kinds: KindSet::default().with(EventKind::Tutorial),
            hidden_events: [149_408].into(),
            hidden_rows: [RowKey { event: 148_369, fp: 0xaaf38 }].into(),
            chosen_rows: [RowKey { event: 148_369, fp: 0xa4d12 }].into(),
            hidden_dates: [(RowKey { event: 148_369, fp: 0xa4d12 }, day("2026-11-04"))].into(),
            town: TownChoice::Only(crate::select::Town::Cottbus),
            visits: ["13977".to_string()].into(),
            own: vec![arbeit(), block()],
            changes: vec![room],
            notes: vec![(RowKey { event: 148_701, fp: 0xb7025 }, "Folien vorher lesen".into()), (arbeit().key(), "Schlüssel mitnehmen".into())],
            ..Selection::default()
        }
    }

    #[test]
    fn a_part_for_a_person_carries_the_week_without_what_is_hidden() {
        let part = LivePart::of(winter(), &ids(&["12104", "11101", "13977", "x-1", "12104"]), &ids(&["13977"]), Some("079-82-2008"), &selection(), Along::default());
        assert_eq!(part.module_ids(), ["12104", "11101", "13977"]);
        assert_eq!(part.visits, [13977]);
        assert_eq!(part.chosen_rows(), [RowKey { event: 148_369, fp: 0xa4d12 }]);
        assert_eq!(part.change_list().len(), 1);
        assert!(part.hidden_events.is_empty() && part.hidden_rows.is_empty() && part.hidden_dates.is_empty() && part.hidden_kinds == 0 && part.town == 0);
        assert!(part.own.is_empty() && part.notes.is_empty(), "nothing of one's own unless it comes along");
        let code = part.code().unwrap();
        assert!(code.len() < 120, "{} characters: {code}", code.len());
        assert_eq!(LivePart::from_code(&code), Some(part));
    }

    #[test]
    fn a_part_for_a_calendar_app_shows_the_week_as_the_student_sees_it() {
        let along = Along { hidden: true, own: true, notes: true };
        let part = LivePart::of(winter(), &ids(&["12104", "11101", "13977"]), &ids(&["13977"]), Some("079-82-2008"), &selection(), along);
        let back = LivePart::from_code(&part.code().unwrap()).unwrap();
        let shown = back.selection();
        let wanted = selection();
        assert_eq!((shown.hidden_kinds, &shown.hidden_events, &shown.hidden_rows, &shown.hidden_dates, shown.town), (wanted.hidden_kinds, &wanted.hidden_events, &wanted.hidden_rows, &wanted.hidden_dates, wanted.town));
        assert_eq!(shown.own, wanted.own, "a Termin before the half-year (a Vorkurs) comes along too");
        assert_eq!(shown.notes, wanted.notes);
        assert_eq!(shown.visits, wanted.visits);
        assert_eq!(shown.changes.iter().map(|c| (c.key, c.scope, c.room.clone())).collect::<Vec<_>>(), wanted.changes.iter().map(|c| (c.key, c.scope, c.room.clone())).collect::<Vec<_>>());
    }

    #[test]
    fn an_emptied_plan_is_a_part_too() {
        let part = LivePart::of(winter(), &[], &[], None, &Selection::default(), Along::default());
        let code = part.code().unwrap();
        assert_eq!(LivePart::from_code(&code).map(|p| p.modules.len()), Some(0));
    }

    #[test]
    fn notes_too_long_for_a_part_are_left_out_from_the_last() {
        let mut wanted = selection();
        wanted.notes = (0..100u32).map(|n| (RowKey { event: 140_000 + n, fp: n }, format!("{n} {}", "x".repeat(400)))).collect();
        let part = LivePart::of(winter(), &ids(&["12104"]), &[], None, &wanted, Along { hidden: false, own: true, notes: true });
        assert!(part.code().is_err(), "a hundred long notes are more than a part holds");
        let (fitted, left) = part.fitted();
        let code = fitted.code().unwrap();
        assert!(code.len() <= MAX_CODE && left > 0 && fitted.own.len() == 2, "{left} left out, {} kept", fitted.notes.len());
        assert_eq!(fitted.notes.len() + left, 100);
        assert_eq!(fitted.notes.first().map(|note| note.key), Some(RowKey { event: 140_000, fp: 0 }.packed()), "the first notes stay");
    }

    #[test]
    fn anything_else_is_no_part() {
        let part = LivePart::of(winter(), &ids(&["12104"]), &[], None, &Selection::default(), Along::default());
        let code = part.code().unwrap();
        for wrong in [String::new(), format!("{code}x"), code[1..].to_string(), "~".repeat(MAX_CODE + 1)] {
            assert!(LivePart::from_code(&wrong).is_none(), "{wrong}");
        }
        let other = folia_pack::to_versioned_code("calendar", VERSION, &part).unwrap();
        let later = folia_pack::to_versioned_code(KIND, VERSION + 1, &part).unwrap();
        let twice = folia_pack::to_versioned_code(KIND, VERSION, &LivePart { modules: vec![12104, 12104], ..part.clone() }).unwrap();
        let no_semester = folia_pack::to_versioned_code(KIND, VERSION, &LivePart { semester: 0, ..part.clone() }).unwrap();
        let long_note = folia_pack::to_versioned_code(KIND, VERSION, &LivePart { notes: vec![CodeNote { key: 1 << 20, text: "x".repeat(MAX_NOTE + 1) }], ..part }).unwrap();
        for code in [other, later, twice, no_semester, long_note] {
            assert!(LivePart::from_code(&code).is_none(), "{code}");
        }
    }

    #[test]
    fn a_termin_that_is_none_here_is_left_out() {
        let part = LivePart { semester: winter().index(), own: vec![CodeOwn { number: 1, rhythm: 9, weekday: 1, from: 60, to: 120, ..CodeOwn::default() }, CodeOwn::of(winter(), &arbeit())], ..LivePart::default() };
        let back = LivePart::from_code(&part.code().unwrap()).unwrap();
        assert_eq!(back.own_termine(), vec![arbeit()]);
    }

    #[test]
    fn a_share_goes_on_with_the_next_semester_never_back() {
        let summer = SemesterKey::parse("2027S").unwrap();
        assert_eq!(following(winter(), day("2026-11-08")), winter());
        assert_eq!(following(winter(), day("2027-03-31")), winter());
        assert_eq!(following(winter(), day("2027-04-01")), summer);
        assert_eq!(following(winter(), day("2027-10-02")), SemesterKey::parse("2027W").unwrap());
        // Made for the coming semester: it stays there, though today is still in the last one.
        assert_eq!(following(summer, day("2027-02-10")), summer);
    }
}
