//! What a visitor chose to see: town, hidden kinds, events and Termine, choices made.
//!
//! The Studienplan shows everything its modules link unless the visitor says otherwise, because a
//! lecture the app hid is missed while one that does not happen is easy to hide. A `Selection` is
//! what the visitor said for one semester: kinds switched off, whole events, single Termine and
//! single dates of a Termin hidden, the options of a choice ticked (as many as the visitor likes,
//! all alike: owner, 2026-10-26), and the town whose course a module with two city tracks is taken
//! in, and the Termine of one's own the visitor added (`own`). The store (`studyplan`) and the
//! subscription code carry it (the code not the Termine of one's own); the
//! timetable (`model`) applies it and says per event and row which rule hid it (`HiddenBy`).

use std::collections::BTreeSet;

use folia_model::labels::{Campus, Code};
use serde::{Deserialize, Serialize};

use crate::day::Day;
use crate::kind::KindSet;
use crate::own::OwnTermin;
use crate::rowkey::RowKey;

/// The most modules of one semester: the store's cap and the subscription code's.
pub const MAX_MODULES: usize = 60;

/// The most entries of each list of hidden events, hidden Termine, chosen Termine and hidden dates
/// of one semester: the store's cap and the subscription code's.
pub const MAX_HIDDEN: usize = 500;

/// One of the BTU's two towns: what a visitor chooses („Standort"). The courses of a module are
/// campuses (`HiddenBy::Course`, docs/radix/schema-v2.md „Tracks"): Radix tells a module's two
/// courses at the Zentralcampus and in Sachsendorf (12102) from its two required parts there
/// (11760) by their SWS; a town chosen decides between courses in two towns.
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
    campus.known().map(town_of_campus)
}

/// The town of a known campus.
pub fn town_of_campus(campus: Campus) -> Town {
    match campus {
        Campus::Zentralcampus | Campus::Sachsendorf | Campus::Nord => Town::Cottbus,
        Campus::Senftenberg => Town::Senftenberg,
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
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Selection {
    /// Kinds switched off; an event is hidden only when all its kinds are.
    pub hidden_kinds: KindSet,
    /// Events hidden as a whole, by `veranstid`.
    pub hidden_events: BTreeSet<u32>,
    /// Single Termine hidden by an eye button.
    pub hidden_rows: BTreeSet<RowKey>,
    /// Ticked options of choices: of a choice holding the rows, the options with them are shown,
    /// as many as are ticked, and each counts as a Termin of the plan like any other (owner,
    /// 2026-10-26: „Ja zu allen drei Vorschlägen", the ticked groups alike).
    pub chosen_rows: BTreeSet<RowKey>,
    /// Single dates hidden („Nur am 04.11. ausblenden"): the Termin's key and the day. The Termin
    /// stays; that day is in neither its week, its day nor the feed (owner, 2026-10-26: a hidden
    /// date is missing in the calendar too).
    pub hidden_dates: BTreeSet<(RowKey, Day)>,
    pub town: TownChoice,
    /// The planned modules a derived town is taken from: those taken over from the
    /// Regelstudienplan, so that an elective added later does not turn the town (owner review
    /// 2026-09-25); `None` for every planned module (a plan without an import, a subscription).
    pub town_from: Option<BTreeSet<String>>,
    /// The modules of the timetable that are only visited („nur besuchen", owner 2026-10-19: „in
    /// der Woche ja, in den LP nicht"; 2026-10-26: „+ Modul" in „Mein Studiengang"): their Termine
    /// stand in the week and meet nothing in a clash, a visit gives way (`clash::yield_pairs` of
    /// the timetable), and they bring no exam. Empty for a subscription, whose code names none.
    #[serde(default)]
    pub visits: BTreeSet<String>,
    /// The Termine of one's own of the semester's calendar (`own`; folia/design/kalender/termine.html,
    /// owner 2026-10-27): the timetable takes each as a Termin of the plan, firm as a module's.
    /// Empty for a subscription, whose code names none (the feed leaves them out until their own
    /// step, „Eigene Termine mitnehmen").
    #[serde(default)]
    pub own: Vec<OwnTermin>,
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
    /// The event is the course of a module held at two campuses or more that the plan does not
    /// take (docs/radix/schema-v2.md, „Tracks"): its program takes another, or the town shown
    /// holds another. The campus is the event's own, where it is held.
    Course(Campus),
    /// Other options of the event's choice were ticked, not this one.
    Choice,
    /// Another option of the choice among its module's events was chosen (docs/radix/schema-v2.md,
    /// „Choices": one exercise of three, one of 14 projects).
    Option,
    /// QIS lists the plan's program at another option of the choice among its module's events,
    /// whose options are for other programs: the program takes that one.
    Program,
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
        assert!(empty.hidden_events.is_empty() && empty.hidden_rows.is_empty() && empty.chosen_rows.is_empty() && empty.hidden_dates.is_empty());
        assert_eq!(empty.town, TownChoice::Derive);
    }
}
