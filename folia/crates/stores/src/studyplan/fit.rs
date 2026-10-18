//! What the dialog for a study not set up fills in (`folia_widgets::plansetup`; owner, 2026-10-18,
//! of concept E: „Also so, dass man ein modal bekommt und dann da zwar schon die defaults drin sind.
//! Aber man trotzdem alles änderen kann … so dass man im best case nur auf ok drücken muss"): the
//! Fachsemester now and the semester the module goes into, made to fit the Regelstudienplan of the
//! program chosen (the same day: „nachdem der Studiengang gewählt wurde können wir im
//! regelstudienplan gucken, in welchem Semester das Modul steht und dann das aktuelle fs und das fs
//! in das das modul so wählen, dass das logisch zusammenpasst. Aber nur, wenn der User noch nichts an
//! den werten geändert hat").
//!
//! What the student set stays. Without a Fachsemester, Betula takes the one that puts the module into
//! its semester of the plan in the semester it goes into; with one (set, or the catalog's filter),
//! the semester that does. A place behind the student is not refused (the same day: „Betula sollte
//! bei sowas nicht nein sagen"): the module goes where „Einplanen" aimed, and the dialog says why.
//! Where the season of the program's odd Fachsemester is known (a Bachelor's), the Fachsemester fit
//! it.

use folia_calendar::semester::SemesterKey;
use folia_model::labels::Season;
use folia_plans::study::Offer;

/// Where the Fachsemester the dialog shows comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FsFrom {
    /// The student set it, with − and +.
    Student,
    /// The catalog's filter, read by the time (`filter_fs`).
    Filter,
    /// Made to fit where the Regelstudienplan places the module.
    Plan,
    /// The program's plan does not place the module: the first, to be checked.
    Assumed,
    /// No program is chosen yet.
    Unknown,
}

/// What the dialog knows when it fills in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Known {
    /// The snapshot's current semester: the Fachsemester is the one of it, and what is before it is
    /// behind the student.
    pub now: SemesterKey,
    /// Where „Einplanen" aimed (`target_semester`).
    pub aim: SemesterKey,
    pub offer: Offer,
    /// A program is chosen.
    pub program: bool,
    /// The semester its Regelstudienplan places the module in.
    pub place: Option<u8>,
    /// The season its odd Fachsemester fall in (`intake_of`).
    pub intake: Option<Season>,
    /// The Fachsemester the catalog's filter speaks for (`filter_fs`).
    pub filter_fs: Option<u8>,
    /// What the student set: the Fachsemester, the module's semester.
    pub fs: Option<u8>,
    pub semester: Option<SemesterKey>,
}

/// What is filled in: the Fachsemester now (none while no program is chosen and none is set),
/// where it comes from, and the module's semester.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Filled {
    pub fs: Option<u8>,
    pub from: FsFrom,
    pub semester: SemesterKey,
}

/// What the dialog fills in from what it knows.
pub fn fill(k: &Known) -> Filled {
    let given = k.fs.map(|fs| (fs, FsFrom::Student)).or_else(|| k.filter_fs.filter(|_| k.program).map(|fs| (fs, FsFrom::Filter)));
    let place = k.place.filter(|_| k.program);
    let Some(n) = place else {
        // Nothing to fit to: what is set, else the first Fachsemester once a program is chosen.
        let (fs, from) = match given {
            Some((fs, from)) => (Some(fs), from),
            None if k.program => (Some(1), FsFrom::Assumed),
            None => (None, FsFrom::Unknown),
        };
        return Filled { fs, from, semester: k.semester.unwrap_or(k.aim) };
    };
    if let Some((fs, from)) = given {
        // The Fachsemester is known: the module goes where the plan places it, unless that is behind
        // the student (then where „Einplanen" aimed) or the student chose its semester.
        let semester = k.semester.unwrap_or_else(|| match plus(k.now, i32::from(n) - i32::from(fs)) {
            Some(at) if at >= k.now => k.offer.next(at),
            _ => k.aim,
        });
        return Filled { fs: Some(fs), from, semester };
    }
    match k.intake.map(|intake| fs_in_winter(n, intake)) {
        // The season of its Fachsemester is known: the semester of the plan's place is one of that
        // season, the next from where „Einplanen" aimed (from the semester the student chose: the
        // nearer one, the earlier at a tie), or a year before where it would make the Fachsemester
        // now less than the first.
        Some(winter) => {
            let mut at = match k.semester {
                Some(chosen) if chosen.winter == winter => chosen,
                Some(chosen) => plus(chosen, -1).unwrap_or(chosen),
                None => k.aim,
            };
            while at.winter != winter {
                at = plus(at, 1).unwrap_or(at);
            }
            while i32::from(n) - between(k.now, at) < 1 {
                match plus(at, -2) {
                    Some(earlier) => at = earlier,
                    None => break,
                }
            }
            let fs = fs_of(i32::from(n) - between(k.now, at));
            let semester = k.semester.unwrap_or_else(|| k.offer.next(at.max(k.aim)));
            Filled { fs: Some(fs), from: FsFrom::Plan, semester }
        }
        // Not known (a Master's): the module goes where „Einplanen" aimed, and the Fachsemester puts
        // it into the plan's semester there.
        None => {
            let semester = k.semester.unwrap_or(k.aim);
            Filled { fs: Some(fs_of(i32::from(n) - between(k.now, semester))), from: FsFrom::Plan, semester }
        }
    }
}

/// Where the Regelstudienplan places the module for a student in Fachsemester `fs` now: the
/// semester of its Fachsemester `n`.
pub fn place_semester(now: SemesterKey, fs: u8, n: u8) -> Option<SemesterKey> {
    plus(now, i32::from(n) - i32::from(fs))
}

/// The Fachsemester a filter of the catalog speaks for, and the semester it means (owner,
/// 2026-10-18: „wenn man semester 3 auswählt im filter ist man probably erst im zweiten. Das muss
/// also auch Kontextanhängig von der zeit inferred werden"): its semester `n` is one of the plan, the
/// one the student looks at, not always the one they are in. Where the season of the program's odd
/// Fachsemester is known, it is the one of the current semester and the next that has that season;
/// else the semester the session plans for. The Fachsemester now is `n` less the semesters until
/// then, the first at least.
pub fn filter_fs(n: u8, now: SemesterKey, session: SemesterKey, intake: Option<Season>) -> (u8, SemesterKey) {
    let at = match intake.map(|intake| fs_in_winter(n, intake)) {
        Some(winter) if now.winter == winter => now,
        Some(_) => plus(now, 1).unwrap_or(now),
        None => session.max(now),
    };
    (fs_of(i32::from(n) - between(now, at)), at)
}

/// Whether Fachsemester `n` of a study begun in `intake` is a winter: the odd ones are the
/// intake's season.
pub fn fs_in_winter(n: u8, intake: Season) -> bool {
    (n % 2 == 1) == (intake == Season::Winter)
}

/// The season a program's odd Fachsemester fall in (owner, 2026-10-18: „bei bachelor Studiengängen
/// man ziemlich sicher sagen, dass ungerade zahlen im Winter sind und gerade im Sommer. Das kann man
/// mit dem regelstudienplan auch nochmal bestätigen … Master hingegen haben diese "Einschränkung"
/// nicht"): a Bachelor's begins in the winter unless its Regelstudienplan says otherwise (`plan`,
/// `folia_plans::studyplan::intake_season`: a start its caption names, or the turnus of its
/// modules); any other in either.
pub fn intake_of(bachelor: bool, plan: Option<Season>) -> Option<Season> {
    bachelor.then(|| plan.unwrap_or(Season::Winter))
}

/// How many semesters `to` is after `from` (before it: less than 0).
fn between(from: SemesterKey, to: SemesterKey) -> i32 {
    i32::from(to.index()) - i32::from(from.index())
}

fn plus(at: SemesterKey, n: i32) -> Option<SemesterKey> {
    at.plus(n)
}

/// A Fachsemester: the first at least, the 30th at most.
fn fs_of(n: i32) -> u8 {
    u8::try_from(n.clamp(1, 30)).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use folia_model::labels::TurnusSeason;

    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    /// 20 October 2026: the current semester and the one „Einplanen" aims at is the winter.
    fn known() -> Known {
        Known { now: key("2026W"), aim: key("2026W"), offer: Offer::default(), program: false, place: None, intake: None, filter_fs: None, fs: None, semester: None }
    }

    #[test]
    fn without_a_program_nothing_is_assumed() {
        assert_eq!(fill(&known()), Filled { fs: None, from: FsFrom::Unknown, semester: key("2026W") });
        // What the student set before choosing stays.
        let set = Known { fs: Some(4), semester: Some(key("2027S")), ..known() };
        assert_eq!(fill(&set), Filled { fs: Some(4), from: FsFrom::Student, semester: key("2027S") });
    }

    /// Datenbanken, in the 3rd semester of Informatik B.Sc., planned on 20 October: in the 3rd
    /// Fachsemester now, the module this winter.
    #[test]
    fn a_program_chosen_fits_the_fachsemester_to_the_plan() {
        let bachelor = Known { program: true, place: Some(3), intake: Some(Season::Winter), ..known() };
        assert_eq!(fill(&bachelor), Filled { fs: Some(3), from: FsFrom::Plan, semester: key("2026W") });
        // Its 4th semester (Mathematik B.Sc.) is a summer: the module goes into the summer, and
        // the student is in the 3rd now (not in a 4th in the winter, which a Bachelor has not).
        let fourth = Known { place: Some(4), ..bachelor };
        assert_eq!(fill(&fourth), Filled { fs: Some(3), from: FsFrom::Plan, semester: key("2027S") });
        // A Master's has no season: the module where „Einplanen" aimed, the Fachsemester to fit.
        let master = Known { intake: None, place: Some(4), ..bachelor };
        assert_eq!(fill(&master), Filled { fs: Some(4), from: FsFrom::Plan, semester: key("2026W") });
        // A module of the 1st semester, aimed at a summer (offered only then): the 1st was this
        // winter, which a Bachelor began in.
        let summer_only = Known { aim: key("2027S"), offer: Offer { season: Some(TurnusSeason::Summer), parity: None }, place: Some(1), ..bachelor };
        assert_eq!(fill(&summer_only), Filled { fs: Some(1), from: FsFrom::Plan, semester: key("2027S") });
        // Not placed by the program's plan: the first Fachsemester, assumed.
        let nowhere = Known { place: None, ..bachelor };
        assert_eq!(fill(&nowhere), Filled { fs: Some(1), from: FsFrom::Assumed, semester: key("2026W") });
    }

    /// The owner, 2026-10-18: „Aber nur, wenn der User noch nichts an den werten geändert hat".
    #[test]
    fn what_the_student_set_stays_and_the_rest_follows() {
        let bachelor = Known { program: true, place: Some(3), intake: Some(Season::Winter), ..known() };
        // The Fachsemester set: the module goes where the plan places it for them.
        let first = Known { fs: Some(1), ..bachelor };
        assert_eq!(fill(&first), Filled { fs: Some(1), from: FsFrom::Student, semester: key("2027W") });
        // Behind them (the 5th now, the module of the 3rd): where „Einplanen" aimed.
        let fifth = Known { fs: Some(5), ..bachelor };
        assert_eq!(fill(&fifth), Filled { fs: Some(5), from: FsFrom::Student, semester: key("2026W") });
        // The semester set: the Fachsemester fits it, by the season of the module's.
        let later = Known { semester: Some(key("2027S")), ..bachelor };
        assert_eq!(fill(&later), Filled { fs: Some(3), from: FsFrom::Plan, semester: key("2027S") });
        // In a past summer: the 3rd was the winter before it (taken a semester late rather than
        // early), so the 7th now.
        let past = Known { semester: Some(key("2025S")), ..bachelor };
        assert_eq!(fill(&past), Filled { fs: Some(7), from: FsFrom::Plan, semester: key("2025S") });
        // Both set: both stay.
        let both = Known { fs: Some(2), semester: Some(key("2025W")), ..bachelor };
        assert_eq!(fill(&both), Filled { fs: Some(2), from: FsFrom::Student, semester: key("2025W") });
        // The filter's Fachsemester counts as known, the student's wins over it.
        let filtered = Known { filter_fs: Some(1), ..bachelor };
        assert_eq!(fill(&filtered), Filled { fs: Some(1), from: FsFrom::Filter, semester: key("2027W") });
        assert_eq!(fill(&Known { fs: Some(3), ..filtered }).from, FsFrom::Student);
        // A semester of the plan's place that does not offer the module: the next that does.
        let winter_only = Known { fs: Some(2), offer: Offer { season: Some(TurnusSeason::Winter), parity: None }, ..bachelor };
        assert_eq!(fill(&winter_only).semester, key("2027W"));
        assert_eq!(place_semester(key("2026W"), 5, 3), Some(key("2025W")));
    }

    /// The owner, 2026-10-18: „wenn man semester 3 auswählt im filter ist man probably erst im
    /// zweiten".
    #[test]
    fn the_filters_semester_is_read_by_the_time() {
        let (october, june) = (key("2026W"), key("2027S"));
        // A Bachelor (odd ones in winter): in October „3. Semester" is now, „2. Semester" the summer
        // to come; in June „3. Semester" is the winter to come.
        assert_eq!(filter_fs(3, october, october, Some(Season::Winter)), (3, october));
        assert_eq!(filter_fs(2, october, october, Some(Season::Winter)), (1, key("2027S")));
        assert_eq!(filter_fs(3, june, key("2027W"), Some(Season::Winter)), (2, key("2027W")));
        // Without a season, the semester the session plans for: after 15 November the next.
        assert_eq!(filter_fs(3, october, key("2027S"), None), (2, key("2027S")));
        assert_eq!(filter_fs(3, october, october, None), (3, october));
        // Never before the first.
        assert_eq!(filter_fs(1, october, key("2027S"), None), (1, key("2027S")));
    }

    #[test]
    fn a_bachelor_begins_in_the_winter_unless_its_plan_says_otherwise() {
        assert_eq!(intake_of(true, None), Some(Season::Winter));
        assert_eq!(intake_of(true, Some(Season::Summer)), Some(Season::Summer));
        assert_eq!(intake_of(false, Some(Season::Winter)), None);
        assert!(fs_in_winter(1, Season::Winter) && !fs_in_winter(2, Season::Winter));
        assert!(fs_in_winter(2, Season::Summer) && !fs_in_winter(3, Season::Summer));
    }
}
