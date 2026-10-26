//! The visitor's day as the start page tells it (concept A; folia/design/startseite/konzepte.html,
//! „Heute"): the Termine of today in their order with what is over, what runs and what comes next,
//! the next day with Termine after it („Morgen", „Am Montag"), and the exams still to come. Worked
//! out from the Stundenplan's timetable of the current semester (`Timetable::agenda`: what the
//! visitor's selection shows, a cancelled date as cancelled, the break and the holidays left out),
//! so the start page and the Stundenplan never disagree. Pure: the page hands in the day and the
//! minute.

use std::collections::BTreeMap;

use folia_calendar::day::Day;
use folia_calendar::rowkey::RowKey;
use folia_locale::Locale;
use folia_timetable::model::Timetable;
use folia_timetable::views::{self, AgendaItem, AgendaWeek};

/// How far ahead the next day with Termine is looked for: past a break of two weeks, not into
/// the next semester.
const AHEAD_DAYS: i32 = 40;

/// Where a Termin of today stands at the minute asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Past,
    /// It runs.
    Now,
    /// The first one to come.
    Next,
    Later,
}

/// One Termin or exam of a day.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub day: Day,
    /// Minutes since midnight; `None` for a date without a clear time.
    pub from: Option<u16>,
    pub to: Option<u16>,
    /// What QIS calls the event („Vorlesung", „Übung"), and its few letters („VL", „Ü"); an exam's
    /// are the page's word for it.
    pub kind: String,
    pub kind_short: String,
    /// The module's title as the catalog has it, else the event's.
    pub title: String,
    /// The module in the few letters a line has room for: its abbreviation („EvS"), else its title
    /// cut short.
    pub short: String,
    pub module: Option<String>,
    /// The Termin a click opens in its own view (`url::home_termin_path`); `None` for an exam.
    pub key: Option<RowKey>,
    /// The rooms, as short as Radix writes them („ZHG/HS.C"), joined by „ / ".
    pub room: Option<String>,
    /// QIS cancels the date, with its reason (empty when it gives none).
    pub cancelled: Option<String>,
    /// A room note of the day („Raumwechsel").
    pub note: Option<String>,
    /// An option of a choice the visitor has not made yet (a group of an Übung, one of three
    /// exercises): one of several, the Stundenplan picks it.
    pub open: bool,
    pub exam: bool,
    /// The tone of the module in the plan, 1–8, as the Stundenplan colours it.
    pub tone: u8,
    pub state: State,
}

impl Entry {
    /// A Termin that is held and taken: neither cancelled nor one option of an open choice.
    pub fn firm(&self) -> bool {
        self.cancelled.is_none() && !self.open
    }
}

/// The day of the start page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Today {
    /// The Termine and exams of today, by time (those without one first).
    pub entries: Vec<Entry>,
    /// The next day after today with a Termin held, and its Termine.
    pub next_day: Option<(Day, Vec<Entry>)>,
    /// The exams from now on, by day and time.
    pub exams: Vec<Entry>,
    /// The days of every exam of the semester, for the clock.
    pub exam_days: Vec<Day>,
}

impl Today {
    /// The Termin that runs now, the first of them where two do.
    pub fn running(&self) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.state == State::Now && entry.firm() && !entry.exam)
    }

    pub fn next(&self) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.state == State::Next)
    }

    /// The Termine held today, the options of a choice not counted.
    pub fn firm(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|entry| entry.firm() && !entry.exam)
    }

    /// Those still to come today.
    pub fn coming(&self) -> impl Iterator<Item = &Entry> {
        self.firm().filter(|entry| matches!(entry.state, State::Next | State::Later))
    }

    /// Two firm Termine of today at once: the other one of `entry`, where there is one.
    pub fn at_once(&self, entry: &Entry) -> Option<&Entry> {
        let (Some(from), Some(to)) = (entry.from, entry.to) else { return None };
        self.firm().find(|other| *other != entry && other.from.zip(other.to).is_some_and(|(a, b)| a < to && from < b))
    }
}

/// How `today` says what it writes: the page's language, and what an exam is called in it.
pub struct Words<'a> {
    pub locale: Locale,
    pub exam: &'a str,
}

/// The day `day` at `minute` of `table` (its agenda `agenda`, said in `words`), the modules named
/// by `titles` and, short, by `names` (`StudyplanData::titles`, `slot_names`).
pub fn today(table: &Timetable, agenda: &[AgendaWeek], titles: &BTreeMap<String, String>, names: &BTreeMap<String, String>, day: Day, minute: u16, words: &Words) -> Today {
    let entries_of = |on: Day, items: &[AgendaItem]| -> Vec<Entry> { items.iter().filter_map(|item| entry(table, item, on, titles, names, words)).collect() };
    let days: Vec<(Day, Vec<Entry>)> = agenda.iter().flat_map(|week| week.days.iter()).map(|agenda_day| (agenda_day.day, entries_of(agenda_day.day, &agenda_day.items))).collect();

    let mut entries = days.iter().find(|(on, _)| *on == day).map(|(_, entries)| entries.clone()).unwrap_or_default();
    for entry in &mut entries {
        entry.state = match (entry.from, entry.to) {
            (_, Some(to)) if to <= minute => State::Past,
            (Some(from), Some(_)) if from <= minute => State::Now,
            _ => State::Later,
        };
    }
    if let Some(next) = entries.iter_mut().find(|entry| entry.state == State::Later && entry.firm() && !entry.exam) {
        next.state = State::Next;
    }

    let next_day = days
        .iter()
        .filter(|(on, _)| *on > day && on.0 - day.0 <= AHEAD_DAYS)
        .find(|(_, entries)| entries.iter().any(|entry| entry.firm() && !entry.exam))
        .map(|(on, entries)| (*on, entries.iter().filter(|entry| !entry.exam).cloned().collect()));

    let mut exams: Vec<Entry> = days
        .iter()
        .flat_map(|(_, entries)| entries.iter())
        .filter(|entry| entry.exam && entry.cancelled.is_none())
        .filter(|entry| entry.day > day || (entry.day == day && entry.to.is_none_or(|to| to > minute)))
        .cloned()
        .collect();
    exams.sort_by_key(|entry| (entry.day, entry.from));
    let mut exam_days: Vec<Day> = days.iter().flat_map(|(_, entries)| entries.iter()).filter(|entry| entry.exam && entry.cancelled.is_none()).map(|entry| entry.day).collect();
    exam_days.sort_unstable();
    exam_days.dedup();
    Today { entries, next_day, exams, exam_days }
}

/// The entry of one item of the agenda on `day`.
fn entry(table: &Timetable, item: &AgendaItem, day: Day, titles: &BTreeMap<String, String>, names: &BTreeMap<String, String>, words: &Words) -> Option<Entry> {
    let join = |rooms: Vec<String>| {
        let mut unique: Vec<String> = Vec::new();
        for room in rooms.into_iter().map(|room| room.trim().to_string()).filter(|room| !room.is_empty()) {
            if !unique.contains(&room) {
                unique.push(room);
            }
        }
        (!unique.is_empty()).then(|| unique.join(" / "))
    };
    let named = |module: Option<&String>, fallback: &str| {
        let title = module.and_then(|id| titles.get(id)).cloned().unwrap_or_else(|| fallback.to_string());
        let short = module.and_then(|id| names.get(id)).cloned().unwrap_or_else(|| views::short_title(&title));
        (title, short)
    };
    let tone = |module: Option<&String>| -> u8 { module.and_then(|id| table.modules.iter().position(|planned| planned == id)).map_or(1, |at| u8::try_from(at % 8 + 1).unwrap_or(1)) };
    if let Some(e) = item.event {
        let event = table.events.get(e)?;
        let module = event.modules.first();
        let (title, short) = named(module, &event.title);
        let rooms = item.rows.iter().filter_map(|r| event.rows.get(*r)).filter_map(|row| row.date.room_short.clone().or_else(|| row.date.room.clone())).collect();
        let row = event.rows.get(item.row);
        let open = (event.unresolved() && row.is_some_and(|row| row.option.is_some())) || event.across.is_some_and(|alt| alt.open);
        return Some(Entry {
            day,
            from: item.from,
            to: item.to,
            kind: views::type_text(event, words.locale),
            kind_short: views::kind_short(event, words.locale),
            title,
            short,
            module: module.cloned(),
            key: row.and_then(|row| row.key),
            room: join(rooms),
            cancelled: item.cancelled.clone(),
            note: item.note.clone(),
            open,
            exam: false,
            tone: event.tone,
            state: State::Later,
        });
    }
    let exam = table.exams.get(item.exam?)?;
    let module = exam.modules.first();
    let (title, short) = named(module, &exam.title);
    let rooms = item.rows.iter().filter_map(|r| exam.rows.get(*r)).filter_map(|row| row.date.room_short.clone().or_else(|| row.date.room.clone())).collect();
    Some(Entry {
        day,
        from: item.from,
        to: item.to,
        kind: words.exam.to_string(),
        kind_short: words.exam.to_string(),
        title,
        short,
        module: module.cloned(),
        key: None,
        room: join(rooms),
        cancelled: item.cancelled.clone(),
        note: item.note.clone(),
        open: false,
        exam: true,
        tone: tone(module),
        state: State::Later,
    })
}

/// „Guten Morgen" and the rest by the hour (`minute` since midnight): night before five and from
/// ten in the evening, morning before eleven, day before five in the afternoon, then evening.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Greeting {
    Morning,
    Day,
    Evening,
    Night,
}

pub fn greeting(minute: u16) -> Greeting {
    match minute / 60 {
        0..=4 | 22.. => Greeting::Night,
        5..=10 => Greeting::Morning,
        11..=16 => Greeting::Day,
        _ => Greeting::Evening,
    }
}

#[cfg(test)]
mod tests {
    use folia_calendar::select::Selection;
    use folia_timetable::model::tests::{d, table, teaching};

    use super::*;

    fn words() -> Words<'static> {
        Words { locale: Locale::De, exam: "Prüfung" }
    }

    /// A Tuesday of three lectures: at 10:00 the first is over, the second runs, the third comes;
    /// in the evening all are over and the next day with Termine is Thursday's.
    #[test]
    fn a_day_runs_its_course() {
        let rows = [
            teaching("11111", "1", 1, "Vorlesung", 2, "07:30", "09:00"),
            teaching("11111", "2", 1, "Übung", 2, "09:15", "10:45"),
            teaching("22222", "3", 1, "Vorlesung", 2, "11:30", "13:00"),
            teaching("22222", "4", 1, "Übung", 4, "13:45", "15:15"),
        ];
        let table = table(&rows, &["11111", "22222"], &[], &Selection::default());
        let agenda = table.agenda(Locale::De);
        let titles = BTreeMap::from([("11111".to_string(), "Mathematik IT-1".to_string())]);
        let names = BTreeMap::from([("11111".to_string(), "MIT1".to_string())]);
        let tuesday = d("2026-10-20");
        let at_ten = today(&table, &agenda, &titles, &names, tuesday, 600, &words());
        let states: Vec<State> = at_ten.entries.iter().map(|entry| entry.state).collect();
        assert_eq!(states, [State::Past, State::Now, State::Next]);
        let running = at_ten.running().unwrap();
        assert_eq!((running.title.as_str(), running.short.as_str(), running.kind_short.as_str()), ("Mathematik IT-1", "MIT1", "Ü"));
        assert_eq!(at_ten.next().map(|entry| entry.from), Some(Some(11 * 60 + 30)));
        assert_eq!(at_ten.coming().count(), 1);
        let evening = today(&table, &agenda, &titles, &names, tuesday, 20 * 60, &words());
        assert!(evening.running().is_none() && evening.next().is_none() && evening.coming().count() == 0);
        let (thursday, entries) = evening.next_day.clone().unwrap();
        assert_eq!((thursday, entries.len()), (d("2026-10-22"), 1));
    }

    #[test]
    fn the_greeting_follows_the_hour() {
        let at = |h: u16| greeting(h * 60);
        assert_eq!([at(3), at(7), at(12), at(18), at(23)], [Greeting::Night, Greeting::Morning, Greeting::Day, Greeting::Evening, Greeting::Night]);
    }
}
