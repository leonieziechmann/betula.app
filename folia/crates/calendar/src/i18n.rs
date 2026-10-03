//! The calendar's words: Brandenburg's public holidays (`day::Holiday`), as the agenda and the
//! calendar feed name them.

use folia_locale::Locale;

pub struct Texts {
    /// „Neujahr"
    pub new_year: &'static str,
    /// „Karfreitag"
    pub good_friday: &'static str,
    /// „Ostersonntag"
    pub easter_sunday: &'static str,
    /// „Ostermontag"
    pub easter_monday: &'static str,
    /// „Tag der Arbeit"
    pub labour_day: &'static str,
    /// „Christi Himmelfahrt"
    pub ascension_day: &'static str,
    /// „Pfingstsonntag"
    pub whit_sunday: &'static str,
    /// „Pfingstmontag"
    pub whit_monday: &'static str,
    /// „Tag der Deutschen Einheit"
    pub german_unity_day: &'static str,
    /// „Reformationstag"
    pub reformation_day: &'static str,
    /// „1. Weihnachtstag"
    pub christmas_day: &'static str,
    /// „2. Weihnachtstag"
    pub boxing_day: &'static str,
}

pub const DE: Texts = Texts {
    new_year: "Neujahr",
    good_friday: "Karfreitag",
    easter_sunday: "Ostersonntag",
    easter_monday: "Ostermontag",
    labour_day: "Tag der Arbeit",
    ascension_day: "Christi Himmelfahrt",
    whit_sunday: "Pfingstsonntag",
    whit_monday: "Pfingstmontag",
    german_unity_day: "Tag der Deutschen Einheit",
    reformation_day: "Reformationstag",
    christmas_day: "1. Weihnachtstag",
    boxing_day: "2. Weihnachtstag",
};

pub const EN: Texts = Texts {
    new_year: "New Year's Day",
    good_friday: "Good Friday",
    easter_sunday: "Easter Sunday",
    easter_monday: "Easter Monday",
    labour_day: "Labour Day",
    ascension_day: "Ascension Day",
    whit_sunday: "Whit Sunday",
    whit_monday: "Whit Monday",
    german_unity_day: "Day of German Unity",
    reformation_day: "Reformation Day",
    christmas_day: "Christmas Day",
    boxing_day: "Boxing Day",
};

/// The calendar's words in `locale`.
pub fn texts(locale: Locale) -> &'static Texts {
    match locale {
        Locale::De => &DE,
        Locale::En => &EN,
    }
}
