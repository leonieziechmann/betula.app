//! What a student meets in the Stundenplan when it holds a semester of the Regelstudienplan, on a
//! real snapshot: for every current program with a study plan, every plan of it (one per study
//! direction) and every Fachsemester that falls into the semester asked for (by the plan's intake
//! season, a winter start where the plan does not tell), the timetable that „Importieren ·
//! Regelstudienplan" gives before anything is chosen (`studyplan::import_fs`,
//! `pages::studyplan_in`, `Timetable::build`), and what is in it: the Termine of a week and their
//! days, the choices still open, the clashes, A and B weeks, dates that do not recur, what QIS
//! cancels or moves, the campuses of a day, the exams. It calls the crates' own functions, so it
//! counts what the app shows. The numbers of folia/design/kalender/analyse.html come from it.
//!
//! ```text
//! cargo run -p folia-pages --example timetable_survey -- <catalog-*.db> [2026W] [--weeks weeks.tsv] > survey.tsv
//! ```
//!
//! One line per semester of a plan, tab-separated with a header; how many on stderr. With
//! `--weeks`, a second file says for every week of the lecture period that differs from the
//! Regelwoche why it does: `once` (a date that does not recur), `partial` (a recurring Termin that
//! has not begun yet or is over), `cancelled`, `moved` (QIS names another date or a room note),
//! `holiday`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use folia_calendar::day::Day;
use folia_calendar::select::Selection;
use folia_calendar::semester::SemesterKey;
use folia_locale::Locale;
use folia_model::labels::Season;
use folia_model::native::NativeDatabase;
use folia_pages as pages;
use folia_plans::studyplan::{self, PlanDoc};
use folia_query as queries;
use folia_timetable::clash::{self, Weeks};
use folia_timetable::model::{TakenBy, Timetable};
use folia_timetable::occur::Skipped;
use folia_timetable::views::{WeekItem, WeekLabel};

/// A gap between two Termine of a day that leaves time for something else („Freistunde").
const GAP: u16 = 90;

const COLUMNS: &[&str] = &[
    "program", "name", "degree", "plan", "intake", "fs", "modules", "placeholders", "without_dates", "events",
    "required_a", "required_b", "open_groups", "open_across", "open_options", "preselected", "days", "earliest", "latest",
    "ab_items", "once_items", "partial_items", "extra_items", "clashes", "blocked", "option_rows_meeting", "hop_days",
    "gaps", "cancelled", "moved_or_room", "holidays", "changed_rows", "lecture_weeks", "irregular_weeks", "exams",
    "exams_dated", "exam_warnings", "exam_warnings_hard", "rows", "rows_without_room", "rows_time_unclear", "tracks",
    "max_per_day", "held_dates", "option_items", "max_per_day_open",
];

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: timetable_survey <catalog-*.db> [semester]");
        return std::process::ExitCode::FAILURE;
    };
    let semester = args.get(1).and_then(|key| SemesterKey::parse(key)).or_else(|| SemesterKey::parse("2026W"));
    let (Some(semester), Ok(db)) = (semester, NativeDatabase::open(&PathBuf::from(path))) else {
        eprintln!("cannot open {path} or read the semester");
        return std::process::ExitCode::FAILURE;
    };
    let mut weeks_out = match args.iter().position(|arg| arg == "--weeks").and_then(|at| args.get(at + 1)) {
        Some(path) => match std::fs::File::create(path) {
            Ok(file) => Some(std::io::BufWriter::new(file)),
            Err(error) => {
                eprintln!("cannot write {path}: {error}");
                return std::process::ExitCode::FAILURE;
            }
        },
        None => None,
    };

    println!("{}", COLUMNS.join("\t"));
    let mut written = 0usize;
    let programs = queries::programs(&db).unwrap_or_default();
    for program in programs.iter().filter(|program| program.is_latest_po && program.has_plan) {
        let Ok(Some(source)) = pages::plan_source(&db, &program.id, Locale::De) else { continue };
        for variant in &source.variants {
            let intake = studyplan::intake_season(variant, &source.linked);
            let starts_in_winter = intake != Some(Season::Summer);
            for fs in 1..=variant.semesters.clamp(0, 12) {
                // An odd Fachsemester lies in the season of the intake.
                let odd = fs % 2 == 1;
                let in_winter = odd == starts_in_winter;
                if in_winter != (semester.season() == Season::Winter) {
                    continue;
                }
                let Ok(fs) = u8::try_from(fs) else { continue };
                let import = studyplan::import_fs(&PlanDoc::default(), &program.id, variant, None, semester, fs);
                let ids: Vec<String> = import.modules.iter().map(|(_, id)| id.clone()).collect();
                if ids.is_empty() {
                    continue;
                }
                let Ok(data) = pages::studyplan_in(&db, semester, &ids, Some(&program.id), Locale::De) else { continue };
                let table = data.timetable(&Selection::default());
                let (counted, weeks) = count(&table);
                if let Some(out) = weeks_out.as_mut() {
                    use std::io::Write as _;
                    for (monday, why) in &weeks {
                        let why: Vec<&str> = why.iter().copied().collect();
                        let _ = writeln!(out, "{}\t{}\t{fs}\t{}\t{}", program.slug, variant.label.replace(['\t', '\n'], " "), monday.iso(), why.join(","));
                    }
                }
                let intake = match intake {
                    Some(Season::Winter) => "winter",
                    Some(Season::Summer) => "summer",
                    None => "unknown",
                };
                let head = [
                    program.slug.clone(),
                    program.name.clone(),
                    program.degree_label.clone().unwrap_or_default(),
                    variant.label.clone(),
                    intake.to_string(),
                    fs.to_string(),
                    ids.len().to_string(),
                    import.placeholders.len().to_string(),
                ];
                let line: Vec<String> = head.into_iter().chain(counted.into_iter().map(|n| n.to_string())).map(|cell| cell.replace(['\t', '\n'], " ")).collect();
                println!("{}", line.join("\t"));
                written += 1;
            }
        }
    }
    eprintln!("{written} semesters of plans in {}", semester.key());
    std::process::ExitCode::SUCCESS
}

/// The numbers of one timetable, in the order of `COLUMNS` after the plan's own, and the weeks of
/// the lecture period that differ from the Regelwoche with why.
fn count(table: &Timetable) -> (Vec<u64>, Vec<(Day, BTreeSet<&'static str>)>) {
    let events = &table.events;
    let week = table.regular_week();
    let visible = |index: usize| events.get(index).is_some_and(|event| event.hidden.is_none());
    let required: Vec<&WeekItem> = week.iter().filter(|item| item.alt.is_none() && !item.extra && visible(item.event)).collect();
    let in_week = |shown: Weeks| required.iter().filter(|item| item.in_week(shown)).count();

    // Choices: an event's groups or parallel slots, and a choice across a module's events.
    let open_groups: Vec<usize> = events.iter().filter(|event| event.hidden.is_none() && event.unresolved()).map(|event| event.visible_options().len()).collect();
    let open_across: Vec<usize> = table.choices.iter().filter(|choice| choice.open(events)).map(|choice| choice.choosable(events).len()).collect();
    let preselected = table.choices.iter().filter(|choice| choice.by == Some(TakenBy::Program)).count();

    let days: BTreeSet<u8> = required.iter().map(|item| item.day).collect();
    let earliest = required.iter().map(|item| item.from).min().unwrap_or(0);
    let latest = required.iter().map(|item| item.to).max().unwrap_or(0);
    let ab_items = required.iter().filter(|item| matches!(item.weeks, Some(Weeks::A) | Some(Weeks::B))).count();
    let once_items = week.iter().filter(|item| matches!(item.label, WeekLabel::Once { .. })).count();
    let partial_items = week.iter().filter(|item| matches!(item.label, WeekLabel::Partial { .. })).count();
    let extra_items = week.iter().filter(|item| item.extra).count();
    // While choices are open: the slots of their options in the A week, and the fullest day with them.
    let option_items = week.iter().filter(|item| item.alt.is_some() && visible(item.event) && item.in_week(Weeks::A)).count();
    let mut open_by_day: BTreeMap<u8, usize> = BTreeMap::new();
    for item in week.iter().filter(|item| !item.extra && visible(item.event) && item.in_week(Weeks::A)) {
        *open_by_day.entry(item.day).or_default() += 1;
    }
    let max_per_day_open = open_by_day.values().copied().max().unwrap_or(0);

    // A day of the A week (or of every week) with Termine at two campuses, the gaps of a day, and
    // the most Termine of a day.
    let mut by_day: BTreeMap<u8, Vec<&WeekItem>> = BTreeMap::new();
    for item in required.iter().filter(|item| item.in_week(Weeks::A)) {
        by_day.entry(item.day).or_default().push(item);
    }
    let mut hop_days = 0u64;
    let mut gaps = 0u64;
    let mut max_per_day = 0u64;
    for items in by_day.values_mut() {
        items.sort_by_key(|item| (item.from, item.to));
        max_per_day = max_per_day.max(items.len() as u64);
        let campuses: BTreeSet<String> = items
            .iter()
            .filter_map(|item| events.get(item.event).and_then(|event| event.rows.get(item.row)))
            .filter_map(|row| row.date.campus.as_ref().map(|campus| format!("{campus:?}")))
            .collect();
        if campuses.len() > 1 {
            hop_days += 1;
        }
        let mut end: Option<u16> = None;
        for item in items.iter() {
            if let Some(end) = end {
                if item.from >= end.saturating_add(GAP) {
                    gaps += 1;
                }
            }
            end = Some(end.map_or(item.to, |end| end.max(item.to)));
        }
    }

    // What the dates hold: cancelled and moved dates, room notes, holidays, and the weeks of the
    // lecture period that differ from the Regelwoche.
    let lecture_weeks: Vec<Day> = match table.facts.lecture {
        Some((first, last)) => {
            let mut weeks = Vec::new();
            let mut monday = first.monday();
            while monday <= last {
                let in_break = table.facts.breaks.iter().any(|(from, to)| monday >= *from && monday <= *to);
                if !in_break {
                    weeks.push(monday);
                }
                monday = monday.plus(7);
            }
            weeks
        }
        None => Vec::new(),
    };
    let mut irregular: BTreeMap<Day, BTreeSet<&'static str>> = BTreeMap::new();
    let mut mark = |day: Day, why: &'static str| {
        irregular.entry(day.monday()).or_default().insert(why);
    };
    let (mut cancelled, mut moved, mut holidays, mut changed_rows, mut rows, mut without_room, mut unclear, mut held) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for event in events.iter().filter(|event| event.hidden.is_none()) {
        for row in event.rows.iter().filter(|row| row.hidden.is_none()) {
            rows += 1;
            if row.date.room.is_none() && row.date.room_short.is_none() {
                without_room += 1;
            }
            if row.from.is_none() {
                unclear += 1;
            }
            if row.option.is_none() && event.across.is_none() && !event.extra {
                held += row.occ.days.len() as u64;
            }
            cancelled += row.occ.cancelled.len() as u64;
            moved += row.occ.cancelled.iter().filter(|(_, _, instead)| instead.is_some()).count() as u64 + row.occ.notes.len() as u64;
            if !row.occ.cancelled.is_empty() || !row.occ.notes.is_empty() {
                changed_rows += 1;
            }
            for (day, why) in &row.occ.skipped {
                if matches!(why, Skipped::Holiday(_)) {
                    holidays += 1;
                    mark(*day, "holiday");
                }
            }
            for (day, _, instead) in &row.occ.cancelled {
                mark(*day, if instead.is_some() { "moved" } else { "cancelled" });
            }
            for (day, _) in &row.occ.notes {
                mark(*day, "moved");
            }
            match row.every {
                // Dates that do not recur make their weeks differ.
                None => {
                    for day in &row.occ.days {
                        mark(*day, "once");
                    }
                }
                // A recurring row that runs in a part of the period only: the weeks outside it.
                Some(_) => {
                    if let (Some(first), Some(last)) = (row.occ.days.first(), row.occ.days.last()) {
                        let (first, last) = (first.monday(), last.monday());
                        for monday in lecture_weeks.iter().filter(|monday| **monday < first || **monday > last) {
                            mark(*monday, "partial");
                        }
                    }
                }
            }
        }
    }
    let weeks: Vec<(Day, BTreeSet<&'static str>)> = lecture_weeks.iter().filter_map(|monday| irregular.get(monday).map(|why| (*monday, why.clone()))).collect();
    let irregular_weeks = weeks.len();

    let exams: Vec<_> = table.exams.iter().filter(|exam| exam.hidden.is_none()).collect();
    let exams_dated = exams.iter().filter(|exam| exam.rows.iter().any(|row| row.hidden.is_none() && row.date.first_date.is_some())).count();
    let option_rows_meeting = clash::option_meetings(events).len();

    let numbers = vec![
        table.without_dates.len() as u64,
        events.iter().filter(|event| event.hidden.is_none()).count() as u64,
        in_week(Weeks::A) as u64,
        in_week(Weeks::B) as u64,
        open_groups.len() as u64,
        open_across.len() as u64,
        (open_groups.iter().sum::<usize>() + open_across.iter().sum::<usize>()) as u64,
        preselected as u64,
        days.len() as u64,
        u64::from(earliest),
        u64::from(latest),
        ab_items as u64,
        once_items as u64,
        partial_items as u64,
        extra_items as u64,
        table.clashes.len() as u64,
        (table.blocked.len() + table.blocked_choices.len()) as u64,
        option_rows_meeting as u64,
        hop_days,
        gaps,
        cancelled,
        moved,
        holidays,
        changed_rows,
        lecture_weeks.len() as u64,
        irregular_weeks as u64,
        exams.len() as u64,
        exams_dated as u64,
        table.exam_warnings.len() as u64,
        table.exam_warnings.iter().filter(|warning| warning.hard).count() as u64,
        rows,
        without_room,
        unclear,
        table.tracks.len() as u64,
        max_per_day,
        held,
        option_items as u64,
        max_per_day_open as u64,
    ];
    (numbers, weeks)
}
