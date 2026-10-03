//! How values read on a page. Unknown values get an honest text, never a guess.

use folia_locale::Locale;
use folia_model::labels::{Code, StudyVariant, TurnusParity, TurnusSeason};

use crate::i18n;

/// The form of study in a word or two, for places where a program is one line.
pub fn variant_short(variant: &Code<StudyVariant>, locale: Locale) -> String {
    let t = &i18n::texts(locale).format;
    match variant.known() {
        Some(StudyVariant::DualPractice) => t.dual_practice.to_string(),
        Some(StudyVariant::DualTraining) => t.dual_training.to_string(),
        Some(StudyVariant::Extended) => t.extended.to_string(),
        Some(StudyVariant::Reduced) => t.reduced.to_string(),
        _ => variant.label(locale).to_string(),
    }
}

/// `6` → „6", `7.5` → „7,5" (German), "7.5" (English).
pub fn number(value: f64, locale: Locale) -> String {
    locale.decimal(value)
}

/// „6 LP", „LP nicht angegeben".
pub fn credits(value: Option<f64>, locale: Locale) -> String {
    let t = &i18n::texts(locale).format;
    match value {
        Some(value) => (t.credits)(&number(value, locale)),
        None => t.credits_unknown.to_string(),
    }
}

/// „1 Modul", „17 Module", „1.204 Module".
pub fn modules(count: impl Into<i64>, locale: Locale) -> String {
    let n = count.into();
    let written = if n < 0 { n.to_string() } else { self::count(n.unsigned_abs(), locale) };
    (i18n::texts(locale).format.modules)(n, &written)
}

/// `1234` → „1.234" (German), "1,234" (English).
pub fn count(value: u64, locale: Locale) -> String {
    locale.thousands(value)
}

/// `2026-09-19T18:06:32Z` or `2026-09-19` → „19.09.2026", "19 Sep 2026"; anything else is
/// returned as it is.
pub fn date(iso: &str, locale: Locale) -> String {
    let day = iso.split('T').next().unwrap_or(iso);
    match folia_calendar::day::Day::parse(day) {
        Some(day) => day.date(locale),
        None => iso.to_string(),
    }
}

/// „1. Semester", „5.–6. Semester", „4. oder 5. Semester": where a validated study plan places a
/// module (`ModuleData::plan_semesters`), several where its study directions differ. `None` for
/// no semester.
pub fn plan_semesters(spans: &[(i64, i64)], locale: Locale) -> Option<String> {
    let t = &i18n::texts(locale).format;
    let named: Vec<String> = spans.iter().map(|(from, to)| if from == to { (t.semester_one)(*from) } else { (t.semester_span)(*from, *to) }).collect();
    let (last, rest) = named.split_last()?;
    Some(match rest.is_empty() {
        true => (t.semesters)(last),
        false => (t.semesters_or)(&rest.join(", "), last),
    })
}

/// „WiSe", „SoSe (gerade Jahre)", „jedes Semester", „unregelmäßig"
pub fn turnus(season: Option<&Code<TurnusSeason>>, parity: Option<&Code<TurnusParity>>, locale: Locale) -> String {
    let t = &i18n::texts(locale).format;
    let Some(season) = season else { return t.turnus_unknown.to_string() };
    let short = match season.known() {
        Some(TurnusSeason::Winter) => t.winter_short,
        Some(TurnusSeason::Summer) => t.summer_short,
        _ => season.label(locale),
    };
    match parity {
        Some(parity) => format!("{short} ({})", parity.label(locale)),
        None => short.to_string(),
    }
}

/// „DE", „EN", „DE / EN"; `None` when the module page does not say.
pub fn languages(german: Option<bool>, english: Option<bool>) -> Option<&'static str> {
    match (german, english) {
        (Some(true), Some(true)) => Some("DE / EN"),
        (Some(true), _) => Some("DE"),
        (_, Some(true)) => Some("EN"),
        _ => None,
    }
}

/// „Mo 09:15–10:45", "Mon 09:15–10:45"
pub fn time_slot(weekday: Option<i64>, start: Option<&str>, end: Option<&str>, locale: Locale) -> Option<String> {
    let day = weekday.and_then(|weekday| locale.texts().weekday_short(weekday)).map(str::to_string);
    let time = match (start, end) {
        (Some(start), Some(end)) => Some(format!("{start}–{end}")),
        (Some(start), None) => Some(start.to_string()),
        _ => None,
    };
    match (day, time) {
        (Some(day), Some(time)) => Some(format!("{day} {time}")),
        (Some(day), None) => Some(day),
        (None, Some(time)) => Some(time),
        (None, None) => None,
    }
}

/// The short name students use for an exam form, for table columns.
pub fn exam_short(form: &Code<folia_model::labels::ExamForm>, locale: Locale) -> String {
    use folia_model::labels::ExamForm;
    let t = &i18n::texts(locale).format;
    match form.known() {
        Some(ExamForm::Map) => "MAP".to_string(),
        Some(ExamForm::PrereqMap) => t.prereq_map.to_string(),
        Some(ExamForm::Mca) => "MCA".to_string(),
        Some(ExamForm::PrereqMca) => t.prereq_mca.to_string(),
        Some(ExamForm::Other) => t.other_exam.to_string(),
        None => form.label(locale).to_string(),
    }
}

/// `09:15` → 18.5 half hours since midnight; `None` for anything that is not a time. `24:00` is
/// the end of the day (48): QIS ends late events and deadlines there.
pub fn half_hours(time: &str) -> Option<f64> {
    let (h, m) = time.split_once(':')?;
    let (h, m) = (h.trim().parse::<u32>().ok()?, m.get(..2)?.parse::<u32>().ok()?);
    ((h < 24 && m < 60) || (h, m) == (24, 0)).then(|| f64::from(h) * 2.0 + f64::from(m) / 30.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        let de = Locale::De;
        assert_eq!(number(6.0, de), "6");
        assert_eq!(number(7.5, de), "7,5");
        assert_eq!(number(Vec::<f64>::new().into_iter().sum(), de), "0");
        assert_eq!(count(3204, de), "3.204");
        assert_eq!(count(12, de), "12");
        assert_eq!(count(1_000_000, de), "1.000.000");
        assert_eq!(date("2026-09-19T18:06:32Z", de), "19.09.2026");
        assert_eq!(date("soon", de), "soon");
        assert_eq!(credits(None, de), "LP nicht angegeben");
        assert_eq!(languages(Some(false), None), None);
        assert_eq!(time_slot(Some(1), Some("09:15"), Some("10:45"), de).as_deref(), Some("Mo 09:15–10:45"));
        let winter = Code::Known(TurnusSeason::Winter);
        let odd = Code::Known(TurnusParity::Odd);
        assert_eq!(turnus(Some(&winter), Some(&odd), de), "WiSe (ungerade Jahre)");
        assert_eq!(turnus(None, None, de), "Turnus nicht angegeben");
        assert_eq!(half_hours("09:15"), Some(18.5));
        assert_eq!(half_hours("24:00"), Some(48.0));
        assert_eq!(half_hours("24:01"), None);
        assert_eq!(half_hours("offen"), None);
        assert_eq!(plan_semesters(&[(1, 1)], de).as_deref(), Some("1. Semester"));
        assert_eq!(plan_semesters(&[(5, 6)], de).as_deref(), Some("5.–6. Semester"));
        assert_eq!(plan_semesters(&[(4, 4), (5, 5)], de).as_deref(), Some("4. oder 5. Semester"));
        assert_eq!(plan_semesters(&[(1, 1), (2, 2), (3, 4)], de).as_deref(), Some("1., 2. oder 3.–4. Semester"));
        assert_eq!(plan_semesters(&[], de), None);
        assert_eq!(modules(1204, de), "1.204 Module");
    }

    #[test]
    fn formats_in_english() {
        let en = Locale::En;
        assert_eq!((number(7.5, en), count(3204, en), credits(Some(7.5), en)), ("7.5".to_string(), "3,204".to_string(), "7.5 CP".to_string()));
        assert_eq!((modules(1, en), modules(1204, en)), ("1 module".to_string(), "1,204 modules".to_string()));
        assert_eq!(date("2026-09-19T18:06:32Z", en), "19 Sep 2026");
        assert_eq!(time_slot(Some(1), Some("09:15"), Some("10:45"), en).as_deref(), Some("Mon 09:15–10:45"));
        let winter = Code::Known(TurnusSeason::Winter);
        let odd = Code::Known(TurnusParity::Odd);
        assert_eq!(turnus(Some(&winter), Some(&odd), en), "Winter (odd years)");
        assert_eq!(plan_semesters(&[(1, 1)], en).as_deref(), Some("1st semester"));
        assert_eq!(plan_semesters(&[(1, 1), (2, 2), (3, 4)], en).as_deref(), Some("1st, 2nd or 3rd–4th semester"));
        assert_eq!(crate::i18n::format::ordinal(12), "12th");
        assert_eq!(crate::i18n::format::ordinal(22), "22nd");
    }
}
