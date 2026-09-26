//! What a visitor chose to see: town, hidden kinds, events and Termine, choices made.
//!
//! The Studienplan shows everything its modules link unless the visitor says otherwise, because a
//! lecture the app hid is missed while one that does not happen is easy to hide. A `Selection` is
//! what the visitor said for one semester: kinds switched off, whole events and single Termine
//! hidden, one option of a choice taken („Nur diesen"), and the town whose course a module with
//! two city tracks is taken in. The store (`studyplan`) and the subscription code carry it; the
//! timetable (`model`) applies it and says per event and row which rule hid it (`HiddenBy`).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::kind::KindSet;
use super::rowkey::RowKey;
use crate::labels::{Campus, Code};

/// The most modules of one semester: the store's cap and the subscription code's.
pub const MAX_MODULES: usize = 60;

/// The most entries of each list of hidden events, hidden Termine and chosen Termine of one
/// semester: the store's cap and the subscription code's.
pub const MAX_HIDDEN: usize = 500;

/// One of the BTU's two towns. Tracks are towns only, never Cottbus sites: a module's events at
/// Zentralcampus and at Sachsendorf are just as often its two required parts (11760).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Town {
    Cottbus,
    Senftenberg,
}

impl Town {
    /// `cottbus` | `senftenberg`, as the store writes it.
    pub fn code(self) -> &'static str {
        match self {
            Town::Cottbus => "cottbus",
            Town::Senftenberg => "senftenberg",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Town::Cottbus => "Cottbus",
            Town::Senftenberg => "Senftenberg",
        }
    }
}

/// The town of a campus: Zentralcampus, Sachsendorf and Nord are Cottbus; a campus this build has
/// no label for is unknown, not guessed.
pub fn town_of(campus: &Code<Campus>) -> Option<Town> {
    match campus.known()? {
        Campus::Zentralcampus | Campus::Sachsendorf | Campus::Nord => Some(Town::Cottbus),
        Campus::Senftenberg => Some(Town::Senftenberg),
    }
}

/// Which town's course a module with city tracks is taken in. `Derive` takes the town most of the
/// plan's other modules are taught in; `Both` shows both tracks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TownChoice {
    #[default]
    Derive,
    Only(Town),
    Both,
}

impl TownChoice {
    /// The number a subscription code carries: 0 derive, 1 Cottbus, 2 Senftenberg, 3 both.
    pub fn code(self) -> u8 {
        match self {
            TownChoice::Derive => 0,
            TownChoice::Only(Town::Cottbus) => 1,
            TownChoice::Only(Town::Senftenberg) => 2,
            TownChoice::Both => 3,
        }
    }

    /// The choice of a code's number. A number this build does not know reads as `Derive`, so a
    /// code written by a newer build still resolves.
    pub fn from_code(code: u8) -> Self {
        match code {
            1 => TownChoice::Only(Town::Cottbus),
            2 => TownChoice::Only(Town::Senftenberg),
            3 => TownChoice::Both,
            _ => TownChoice::Derive,
        }
    }
}

/// What a visitor chose to see of one semester's timetable. Empty: everything shown, the town
/// derived.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Selection {
    /// Kinds switched off; an event is hidden only when all its kinds are.
    pub hidden_kinds: KindSet,
    /// Events hidden as a whole, by `veranstid`.
    pub hidden_events: BTreeSet<u32>,
    /// Single Termine hidden by an eye button.
    pub hidden_rows: BTreeSet<RowKey>,
    /// Made choices („Nur diesen"): of the open choice holding the row, only its option is shown.
    pub chosen_rows: BTreeSet<RowKey>,
    pub town: TownChoice,
    /// The planned modules a derived town is taken from: those taken over from the
    /// Regelstudienplan, so that an elective added later does not turn the town (owner review
    /// 2026-09-25); `None` for every planned module (a plan without an import, a subscription).
    pub town_from: Option<BTreeSet<String>>,
}

/// Why an event, an exam or a row is not shown: the first rule that holds, in this order (C.9).
/// Named apart from `studyplan::SemesterHides`, the store's lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HiddenBy {
    /// The whole event is hidden.
    Event,
    /// Every kind of the event is switched off.
    Kinds,
    /// The event belongs to a module's course in the other town; the town is the one shown.
    Town(Town),
    /// Another option of the event's choice was chosen.
    Choice,
    /// The Termin itself is hidden.
    Row,
}

/// Which classes „Passt in meinen Stundenplan" compares (the finder, `fit`). Here and not in
/// `fit`, so the catalog's filter can name it before the finder exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FitOptions {
    pub lectures: bool,
    pub exercises: bool,
    pub exams: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn towns_of_the_campuses() {
        assert_eq!(town_of(&Code::parse("zentralcampus")), Some(Town::Cottbus));
        assert_eq!(town_of(&Code::parse("sachsendorf")), Some(Town::Cottbus));
        assert_eq!(town_of(&Code::parse("nord")), Some(Town::Cottbus));
        assert_eq!(town_of(&Code::parse("senftenberg")), Some(Town::Senftenberg));
        assert_eq!(town_of(&Code::parse("mars")), None);
        assert_eq!(Town::Cottbus.code(), "cottbus");
        assert_eq!(Town::Senftenberg.label(), "Senftenberg");
    }

    #[test]
    fn town_choices_as_numbers() {
        for choice in [TownChoice::Derive, TownChoice::Only(Town::Cottbus), TownChoice::Only(Town::Senftenberg), TownChoice::Both] {
            assert_eq!(TownChoice::from_code(choice.code()), choice);
        }
        assert_eq!(TownChoice::Both.code(), 3);
        assert_eq!(TownChoice::from_code(7), TownChoice::Derive);
        assert_eq!(TownChoice::default(), TownChoice::Derive);
    }

    #[test]
    fn an_empty_selection_hides_nothing() {
        let empty = Selection::default();
        assert!(empty.hidden_kinds.is_empty());
        assert!(empty.hidden_events.is_empty() && empty.hidden_rows.is_empty() && empty.chosen_rows.is_empty());
        assert_eq!(empty.town, TownChoice::Derive);
    }
}
