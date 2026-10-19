//! The courses of a module held at two campuses or more, of which a student attends one
//! (docs/radix/schema-v2.md, „Tracks"; owner, 2026-10-19: „Aber es wird nur erwartet, dass die
//! Studies eine der beiden vls/praktika besucht."). Radix reads them: an event's course is its
//! campus (`EventDate::track`), and which course a program takes is what QIS lists the program on,
//! else the campus of its compulsory modules (`TrackProgram`). Here only what the pages make of
//! it: the courses of a semester, whom each is for, and which a program takes.

use std::collections::{BTreeMap, BTreeSet};

use folia_model::labels::{Campus, TrackBasis};
use folia_model::rows_detail::{EventDate, TrackProgram};

/// The courses of a module in one semester.
#[derive(Clone, Debug, PartialEq)]
pub struct Courses {
    /// The campuses of its courses, in the campuses' order (Zentralcampus, Sachsendorf,
    /// Senftenberg, Nord).
    pub campuses: Vec<Campus>,
    /// Per course, the programs QIS lists on its events, each once (one per family, by its newest
    /// PO's name): „Informatik · B.Sc.". Empty for a course QIS names nobody for.
    pub listed: BTreeMap<Campus, Vec<String>>,
    /// The semester's rows of `v_module_track_program`.
    rows: Vec<TrackProgram>,
}

impl Courses {
    /// The courses of the module in `semester`, from its dates (`EventDate::track`) and the rows
    /// of `v_module_track_program`; `None` where it has fewer than two.
    pub fn of(dates: &[EventDate], tracks: &[TrackProgram], semester: &str) -> Option<Courses> {
        let campuses: BTreeSet<Campus> = dates
            .iter()
            .filter(|date| date.semester_key == semester)
            .filter_map(|date| date.track.as_ref().and_then(|track| track.known()))
            .collect();
        if campuses.len() < 2 {
            return None;
        }
        let rows: Vec<TrackProgram> = tracks.iter().filter(|row| row.semester_key == semester).cloned().collect();
        let mut listed: BTreeMap<Campus, Vec<String>> = BTreeMap::new();
        for campus in &campuses {
            // One name per family: its newest PO's, else the first.
            let mut families: Vec<(&str, &TrackProgram)> = Vec::new();
            for row in rows.iter().filter(|row| row.basis.is(TrackBasis::Qis) && row.campus.known() == Some(*campus)) {
                match families.iter_mut().find(|(family, _)| *family == row.family_key) {
                    Some((_, kept)) if !kept.is_latest_po && row.is_latest_po => *kept = row,
                    Some(_) => {}
                    None => families.push((&row.family_key, row)),
                }
            }
            let mut names: Vec<String> = families.into_iter().map(|(_, row)| program_name(row)).collect();
            names.sort_by_key(|name| name.to_lowercase());
            names.dedup();
            listed.insert(*campus, names);
        }
        Some(Courses { campuses: campuses.into_iter().collect(), listed, rows })
    }

    /// The courses a program takes: by its id, else by its family (a PO of it the semester's rows
    /// name instead); `None` where it takes either, as Radix decided nothing for it.
    pub fn taken_by(&self, program_id: &str, family_key: &str) -> Option<BTreeSet<Campus>> {
        let of = |matches: &dyn Fn(&TrackProgram) -> bool| -> BTreeSet<Campus> {
            self.rows.iter().filter(|row| matches(row)).filter_map(|row| row.campus.known()).filter(|campus| self.campuses.contains(campus)).collect()
        };
        let by_id = of(&|row| row.program_id == program_id);
        let taken = if by_id.is_empty() { of(&|row| row.family_key == family_key) } else { by_id };
        (!taken.is_empty()).then_some(taken)
    }

    /// Whether QIS lists the program's family on the course at `campus` („laut QIS").
    pub fn lists(&self, campus: Campus, family_key: &str) -> bool {
        self.rows.iter().any(|row| row.campus.known() == Some(campus) && row.family_key == family_key && row.basis.is(TrackBasis::Qis))
    }
}

/// „Informatik · B.Sc.": a program as a course names whom it is for.
pub fn program_name(row: &TrackProgram) -> String {
    match row.degree_display.as_deref().map(str::trim).filter(|degree| !degree.is_empty()) {
        Some(degree) => format!("{} · {degree}", row.program_name),
        None => row.program_name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use folia_model::labels::Code;

    fn date(event: &str, semester: &str, track: Option<&str>) -> EventDate {
        EventDate {
            semester_key: semester.into(),
            semester_label: "WiSe 2026/27".into(),
            event_id: event.into(),
            event_number: None,
            event_title: "Programmierpraktikum".into(),
            event_type: None,
            group_name: None,
            weekday: None,
            start_time: None,
            end_time: None,
            rhythm: None,
            rhythm_raw: None,
            first_date: None,
            last_date: None,
            room: None,
            campus: None,
            instructor: None,
            comment: None,
            source_url: None,
            room_short: None,
            series: None,
            track: track.map(Code::parse),
        }
    }

    fn row(campus: &str, program: &str, basis: &str, name: &str, latest: bool) -> TrackProgram {
        TrackProgram {
            module_id: "12102".into(),
            semester_key: "2026W".into(),
            campus: Code::parse(campus),
            program_id: program.into(),
            basis: Code::parse(basis),
            family_key: program.get(..6).unwrap_or(program).into(),
            program_name: name.into(),
            degree_display: Some("B.Sc.".into()),
            is_latest_po: latest,
        }
    }

    /// The Programmierpraktikum of 2026W: its courses, whom QIS lists them for, which a program takes.
    #[test]
    fn the_courses_of_the_programmierpraktikum() {
        let dates = [
            date("148008", "2026W", Some("zentralcampus")),
            date("148370", "2026W", Some("zentralcampus")),
            date("149408", "2026W", Some("sachsendorf")),
            date("150000", "2026W", None),
            date("140000", "2026S", Some("senftenberg")),
        ];
        let rows = [
            row("zentralcampus", "079-82-2008", "qis", "Informatik", true),
            row("zentralcampus", "079-82-2004", "qis", "Informatik (alt)", false),
            row("zentralcampus", "105-82-2023", "qis", "Mathematik", true),
            row("zentralcampus", "794-82-2007", "campus", "eBusiness", true),
            row("sachsendorf", "247-82-2016", "qis", "Medizininformatik", true),
        ];
        let courses = Courses::of(&dates, &rows, "2026W").unwrap();
        assert_eq!(courses.campuses, [Campus::Zentralcampus, Campus::Sachsendorf]);
        assert_eq!(courses.listed[&Campus::Zentralcampus], ["Informatik · B.Sc.", "Mathematik · B.Sc."]);
        assert_eq!(courses.listed[&Campus::Sachsendorf], ["Medizininformatik · B.Sc."]);
        assert_eq!(courses.taken_by("247-82-2016", "247-82"), Some(BTreeSet::from([Campus::Sachsendorf])));
        assert_eq!(courses.taken_by("794-82-2007", "794-82"), Some(BTreeSet::from([Campus::Zentralcampus])));
        // A PO the rows do not name takes what its family takes.
        assert_eq!(courses.taken_by("079-82-2026", "079-82"), Some(BTreeSet::from([Campus::Zentralcampus])));
        assert_eq!(courses.taken_by("208-82-2026", "208-82"), None);
        assert!(courses.lists(Campus::Sachsendorf, "247-82") && !courses.lists(Campus::Zentralcampus, "794-82"));
        // One course in a semester is no choice.
        assert_eq!(Courses::of(&dates, &rows, "2026S"), None);
    }
}
