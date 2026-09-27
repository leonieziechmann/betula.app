//! Words every part of the site shares: how numbers are written, weekdays and months, the
//! halves of the academic year.

pub struct Texts {
    /// The mark between the whole and the fraction of a number: „7,5", "7.5".
    pub decimal_separator: char,
    /// The mark between groups of thousands: „1.234", "1,234".
    pub thousands_separator: char,
    /// Monday … Sunday.
    pub weekdays: [&'static str; 7],
    /// Monday … Sunday in two or three letters, as a timetable's columns and a date name them.
    pub weekdays_short: [&'static str; 7],
    /// January … December in three letters, for dates whose month is spelled out.
    pub months_short: [&'static str; 12],
    /// A whole date: the day of the month, the month (1–12) and the year.
    pub date: fn(u32, u32, i32) -> String,
    /// A date whose year the page already says: day of the month, month (1–12).
    pub day_month: fn(u32, u32) -> String,
    /// The winter semester starting in the year, the summer semester of the year: „WiSe 2026/27",
    /// „SoSe 2027". The second number is the next year in two digits.
    pub winter_semester: fn(u16, u16) -> String,
    pub summer_semester: fn(u16) -> String,
    /// The same in the fewest letters, for narrow places: „WiSe 26/27", „SoSe 27".
    pub winter_semester_short: fn(u16, u16) -> String,
    pub summer_semester_short: fn(u16) -> String,
    /// What is not known, as the pages say it (R12).
    pub not_stated: &'static str,
}

pub const DE: Texts = Texts {
    decimal_separator: ',',
    thousands_separator: '.',
    weekdays: ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag"],
    weekdays_short: ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"],
    months_short: ["Jan.", "Feb.", "März", "Apr.", "Mai", "Juni", "Juli", "Aug.", "Sep.", "Okt.", "Nov.", "Dez."],
    date: |day, month, year| format!("{day:02}.{month:02}.{year:04}"),
    day_month: |day, month| format!("{day:02}.{month:02}."),
    winter_semester: |year, next| format!("WiSe {year}/{next:02}"),
    summer_semester: |year| format!("SoSe {year}"),
    winter_semester_short: |year, next| format!("WiSe {year:02}/{next:02}"),
    summer_semester_short: |year| format!("SoSe {year:02}"),
    not_stated: "nicht angegeben",
};

pub const EN: Texts = Texts {
    decimal_separator: '.',
    thousands_separator: ',',
    weekdays: ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"],
    weekdays_short: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
    months_short: MONTHS_EN,
    date: |day, month, year| format!("{day} {} {year}", month_of(&MONTHS_EN, month)),
    day_month: |day, month| format!("{day} {}", month_of(&MONTHS_EN, month)),
    winter_semester: |year, next| format!("Winter {year}/{next:02}"),
    summer_semester: |year| format!("Summer {year}"),
    winter_semester_short: |year, next| format!("WS {year:02}/{next:02}"),
    summer_semester_short: |year| format!("SS {year:02}"),
    not_stated: "not stated",
};

const MONTHS_EN: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// The month (1–12) of `months`; nothing for what is no month.
fn month_of(months: &[&'static str; 12], month: u32) -> &'static str {
    usize::try_from(month).ok().and_then(|month| month.checked_sub(1)).and_then(|i| months.get(i)).copied().unwrap_or_default()
}

impl Texts {
    /// The month (1–12) in three letters; nothing for what is no month.
    pub fn month_short(&self, month: u32) -> &'static str {
        month_of(&self.months_short, month)
    }

    /// Weekday 1 = Monday … 7 = Sunday, as in the event tables.
    pub fn weekday(&self, weekday: i64) -> Option<&'static str> {
        usize::try_from(weekday).ok().and_then(|day| day.checked_sub(1)).and_then(|i| self.weekdays.get(i)).copied()
    }

    /// The same in two or three letters.
    pub fn weekday_short(&self, weekday: i64) -> Option<&'static str> {
        usize::try_from(weekday).ok().and_then(|day| day.checked_sub(1)).and_then(|i| self.weekdays_short.get(i)).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_days_in_each_language() {
        assert_eq!(((DE.date)(5, 10, 2026), (EN.date)(5, 10, 2026)), ("05.10.2026".to_string(), "5 Oct 2026".to_string()));
        assert_eq!(((DE.day_month)(5, 10), (EN.day_month)(12, 2)), ("05.10.".to_string(), "12 Feb".to_string()));
        assert_eq!((DE.weekday(1), EN.weekday_short(7), EN.weekday(0), DE.weekday_short(8)), (Some("Montag"), Some("Sun"), None, None));
        assert_eq!(((DE.winter_semester)(2026, 27), (EN.summer_semester)(2027)), ("WiSe 2026/27".to_string(), "Summer 2027".to_string()));
        assert_eq!(((DE.winter_semester_short)(26, 27), (EN.summer_semester_short)(27)), ("WiSe 26/27".to_string(), "SS 27".to_string()));
    }
}
