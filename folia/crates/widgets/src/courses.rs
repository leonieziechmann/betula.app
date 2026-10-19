//! The courses of a module held at two campuses or more on its page (owner, 2026-10-19; the
//! courses are Radix's, `folia_timetable::courses`): which Termine the page shows for whom.
//!
//! - **Mein Studiengang**, where the page names no other program (the catalog filtered by it,
//!   the module's own page): only the courses it takes, the others behind „Auch zeigen"
//!   („wenn ein Studiengang eingerichtet [ist], dann die Termine für den Standort vom Studiengang
//!   Anzeigen").
//! - **Another program** the page stands for (the catalog filtered by it, its program page): its
//!   courses as they are, the others faint („die Veranstaltungen hervorheben die dem Studiengang
//!   entsprechend würden und die anderen etwas leichter anzeigen").
//! - **Neither**, or a program Radix decides nothing for, and always on the server (R9): every
//!   course, each in its colour and under its own heading („mit klarer Erkennbarkeit, dass die
//!   einen in Cottbus sind und die anderen in Senftenberg").

use std::collections::BTreeSet;

use folia_model::labels::{Campus, Labelled};
use folia_model::rows::Program;
use folia_timetable::courses::Courses;

use crate::i18n::Texts;

/// The marks of the courses, in the order of their campuses (app.css `.course-1` …): the colour
/// of a course's slots and of its heading.
const MARKS: [&str; 4] = ["course-1", "course-2", "course-3", "course-4"];

/// How a module's page shows its courses.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CourseView {
    /// Every course, each marked; `about` says why where the page stands for a program.
    All { about: Option<Undecided> },
    /// The courses `own` that Mein Studiengang (`program`) takes; the others behind „Auch zeigen".
    Mine { program: String, own: BTreeSet<Campus>, qis: bool },
    /// The courses `own` that `program` takes, the others faint.
    Highlight { program: String, own: BTreeSet<Campus>, qis: bool },
}

/// Why a page that stands for a program shows every course.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Undecided {
    /// The program takes none: QIS names it at none, and its campus decides nothing.
    None(String),
    /// It takes every one (a family QIS lists on each: Elektrotechnik B.Sc.'s study directions).
    Every(String),
}

impl CourseView {
    /// The view for the page's program (`page`: the program of the list or page beside which the
    /// module stands) and Mein Studiengang (`mine`, the browser app's alone). The page's program
    /// is the visitor's own where it is of the family of Mein Studiengang.
    pub(crate) fn of(courses: &Courses, page: Option<&Program>, mine: Option<&Program>) -> CourseView {
        let (program, is_mine) = match (page, mine) {
            (Some(page), Some(mine)) if page.family_key == mine.family_key => (mine, true),
            (Some(page), _) => (page, false),
            (None, Some(mine)) => (mine, true),
            (None, None) => return CourseView::All { about: None },
        };
        let name = program_name(program);
        let Some(own) = courses.taken_by(&program.id, &program.family_key) else {
            return CourseView::All { about: Some(Undecided::None(name)) };
        };
        let qis = own.iter().all(|campus| courses.lists(*campus, &program.family_key));
        if own.len() == courses.campuses.len() {
            // A program that takes every course has nothing to hide.
            return CourseView::All { about: Some(Undecided::Every(name)) };
        }
        match is_mine {
            true => CourseView::Mine { program: name, own, qis },
            false => CourseView::Highlight { program: name, own, qis },
        }
    }

    /// The courses taken, `None` for all of them.
    pub(crate) fn own(&self) -> Option<&BTreeSet<Campus>> {
        match self {
            CourseView::All { .. } => None,
            CourseView::Mine { own, .. } | CourseView::Highlight { own, .. } => Some(own),
        }
    }
}

/// „Informatik · B.Sc.": a program as a course's line names it.
pub(crate) fn program_name(program: &Program) -> String {
    format!("{} · {}", program.name, program.degree())
}

/// The mark of the course at `campus` among `courses` (app.css `.course-1` …).
pub(crate) fn mark(courses: &Courses, campus: Campus) -> Option<&'static str> {
    courses.campuses.iter().position(|held| *held == campus).and_then(|at| MARKS.get(at).copied())
}

/// „Zentralcampus Cottbus und Cottbus-Sachsendorf": the campuses of courses, as a sentence lists
/// them.
pub(crate) fn campuses_text(campuses: &[Campus], t: &'static Texts) -> String {
    let names: Vec<&str> = campuses.iter().map(|campus| campus.label(t.locale)).collect();
    match names.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{}{}{last}", rest.join(", "), t.module.courses_and),
        Some((last, _)) => (*last).to_string(),
        None => String::new(),
    }
}

/// The heading of a course in the list of Termine: „Cottbus-Sachsendorf · für Medizininformatik ·
/// B.Sc."; at most `SHOWN` programs, then how many more.
pub(crate) fn course_heading(courses: &Courses, campus: Campus, t: &'static Texts) -> String {
    const SHOWN: usize = 3;
    let place = campus.label(t.locale);
    let listed = courses.listed.get(&campus).map(Vec::as_slice).unwrap_or_default();
    if listed.is_empty() {
        return place.to_string();
    }
    let named: Vec<&str> = listed.iter().take(SHOWN).map(String::as_str).collect();
    let programs = match listed.len().saturating_sub(SHOWN) {
        0 => named.join(", "),
        more => (t.module.courses_more)(&named.join(", "), more),
    };
    (t.module.course_for)(place, &programs)
}

#[cfg(test)]
mod tests {
    use folia_model::labels::Code;
    use folia_model::rows_detail::{EventDate, TrackProgram};

    use super::*;
    use crate::i18n::{DE, EN};

    fn date(event: &str, track: &str) -> EventDate {
        EventDate {
            semester_key: "2026W".into(),
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
            track: Some(Code::parse(track)),
            choice: None,
            option: None,
            extra: false,
        }
    }

    fn row(campus: &str, program: &str, basis: &str, name: &str) -> TrackProgram {
        TrackProgram {
            module_id: "12102".into(),
            semester_key: "2026W".into(),
            campus: Code::parse(campus),
            program_id: program.into(),
            basis: Code::parse(basis),
            family_key: program.get(..6).unwrap_or(program).into(),
            program_name: name.into(),
            degree_display: Some("B.Sc.".into()),
            is_latest_po: true,
        }
    }

    fn program(id: &str, name: &str) -> Program {
        Program {
            id: id.into(),
            slug: name.to_lowercase(),
            name: name.into(),
            degree_level: Code::parse("bachelor"),
            study_variant: None,
            degree_label: Some("B.Sc.".into()),
            degree_raw: "Bachelor (universitär)".into(),
            degree_display: Some("B.Sc.".into()),
            po_version: "2016".into(),
            po_year: Some(2016),
            family_key: id.get(..6).unwrap_or(id).into(),
            name_key: name.to_lowercase(),
            is_latest_po: true,
            source_url: String::new(),
            has_plan: true,
            plan_status: None,
            curricular_modules: 0,
            fues_modules: 0,
            documents: 0,
        }
    }

    /// The Programmierpraktikum of 2026W as its page shows it to whom.
    #[test]
    fn the_page_shows_the_courses_of_whom_it_stands_for() {
        let dates = [date("148008", "zentralcampus"), date("148370", "zentralcampus"), date("149408", "sachsendorf"), date("148455", "sachsendorf")];
        let rows = [
            row("zentralcampus", "079-82-2008", "qis", "Informatik"),
            row("zentralcampus", "105-82-2023", "qis", "Mathematik"),
            row("zentralcampus", "128-82-2021", "qis", "Physik"),
            row("zentralcampus", "C38-82-2024", "qis", "Wirtschaftsinformatik"),
            row("zentralcampus", "794-82-2007", "campus", "eBusiness"),
            row("sachsendorf", "247-82-2016", "qis", "Medizininformatik"),
        ];
        let courses = Courses::of(&dates, &rows, "2026W").unwrap();
        let (medinf, informatik, ebusiness, soziale) =
            (program("247-82-2016", "Medizininformatik"), program("079-82-2008", "Informatik"), program("794-82-2007", "eBusiness"), program("208-82-2026", "Soziale Arbeit"));
        let sachsendorf = BTreeSet::from([Campus::Sachsendorf]);
        let zentral = BTreeSet::from([Campus::Zentralcampus]);

        // Nothing set: every course.
        assert_eq!(CourseView::of(&courses, None, None), CourseView::All { about: None });
        // Mein Studiengang alone, or the catalog filtered by it (another PO of the family too).
        let mine = CourseView::Mine { program: "Medizininformatik · B.Sc.".into(), own: sachsendorf.clone(), qis: true };
        assert_eq!(CourseView::of(&courses, None, Some(&medinf)), mine);
        assert_eq!(CourseView::of(&courses, Some(&program("247-82-2010", "Medizininformatik")), Some(&medinf)), mine);
        // Another program the page stands for: its course highlighted, whatever is set up.
        let highlight = CourseView::Highlight { program: "Informatik · B.Sc.".into(), own: zentral.clone(), qis: true };
        assert_eq!(CourseView::of(&courses, Some(&informatik), Some(&medinf)), highlight);
        assert_eq!(CourseView::of(&courses, Some(&informatik), None), highlight);
        // By the program's campus, not by QIS.
        assert_eq!(CourseView::of(&courses, None, Some(&ebusiness)), CourseView::Mine { program: "eBusiness · B.Sc.".into(), own: zentral, qis: false });
        // A program Radix decides nothing for: every course, and it says so.
        assert_eq!(CourseView::of(&courses, None, Some(&soziale)), CourseView::All { about: Some(Undecided::None("Soziale Arbeit · B.Sc.".into())) });

        assert_eq!((mark(&courses, Campus::Zentralcampus), mark(&courses, Campus::Sachsendorf), mark(&courses, Campus::Senftenberg)), (Some("course-1"), Some("course-2"), None));
        assert_eq!(campuses_text(&courses.campuses, &DE), "Zentralcampus Cottbus und Cottbus-Sachsendorf");
        assert_eq!(campuses_text(&[Campus::Zentralcampus, Campus::Sachsendorf, Campus::Senftenberg], &EN), "Central Campus Cottbus, Cottbus-Sachsendorf and Senftenberg");
        assert_eq!(course_heading(&courses, Campus::Sachsendorf, &DE), "Cottbus-Sachsendorf · für Medizininformatik · B.Sc.");
        assert_eq!(course_heading(&courses, Campus::Zentralcampus, &DE), "Zentralcampus Cottbus · für Informatik · B.Sc., Mathematik · B.Sc., Physik · B.Sc. und einen weiteren");
        assert_eq!(course_heading(&courses, Campus::Zentralcampus, &EN), "Central Campus Cottbus · for Informatik · B.Sc., Mathematik · B.Sc., Physik · B.Sc. and 1 more");
    }
}
