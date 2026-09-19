//! How values read on a page. Unknown values get an honest text, never a guess.

use catalog::labels::{Code, TurnusParity, TurnusSeason};

/// `6` → „6", `7.5` → „7,5"
pub fn number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value}").replace('.', ",")
    }
}

pub fn credits(value: Option<f64>) -> String {
    match value {
        Some(value) => format!("{} LP", number(value)),
        None => "LP nicht angegeben".to_string(),
    }
}

/// `1234` → „1.234"
pub fn count(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(digit);
    }
    out
}

/// `2026-09-19T18:06:32Z` or `2026-09-19` → „19.09.2026"; anything else is returned as it is.
pub fn date(iso: &str) -> String {
    let day = iso.split('T').next().unwrap_or(iso);
    let parts: Vec<&str> = day.split('-').collect();
    match parts.as_slice() {
        [year, month, day] if year.len() == 4 => format!("{day}.{month}.{year}"),
        _ => iso.to_string(),
    }
}

/// „WiSe", „SoSe (gerade Jahre)", „jedes Semester", „unregelmäßig"
pub fn turnus(season: Option<&Code<TurnusSeason>>, parity: Option<&Code<TurnusParity>>) -> String {
    let Some(season) = season else { return "Turnus nicht angegeben".to_string() };
    let short = match season.known() {
        Some(TurnusSeason::Winter) => "WiSe",
        Some(TurnusSeason::Summer) => "SoSe",
        _ => season.label(),
    };
    match parity {
        Some(parity) => format!("{short} ({})", parity.label()),
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

/// „Mo 09:15–10:45"
pub fn time_slot(weekday: Option<i64>, start: Option<&str>, end: Option<&str>) -> Option<String> {
    let day = weekday.and_then(catalog::labels::weekday_label).map(|label| label.chars().take(2).collect::<String>());
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
pub fn exam_short(form: &Code<catalog::labels::ExamForm>) -> String {
    use catalog::labels::ExamForm;
    match form.known() {
        Some(ExamForm::Map) => "MAP".to_string(),
        Some(ExamForm::PrereqMap) => "Vorleistung + MAP".to_string(),
        Some(ExamForm::Mca) => "MCA".to_string(),
        Some(ExamForm::PrereqMca) => "Vorleistung + MCA".to_string(),
        Some(ExamForm::Other) => "andere Form".to_string(),
        None => form.label().to_string(),
    }
}

/// `09:15` → 18.5 half hours since midnight; `None` for anything that is not a time.
pub fn half_hours(time: &str) -> Option<f64> {
    let (h, m) = time.split_once(':')?;
    let (h, m) = (h.trim().parse::<u32>().ok()?, m.get(..2)?.parse::<u32>().ok()?);
    (h < 24 && m < 60).then(|| f64::from(h) * 2.0 + f64::from(m) / 30.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(number(6.0), "6");
        assert_eq!(number(7.5), "7,5");
        assert_eq!(count(3204), "3.204");
        assert_eq!(count(12), "12");
        assert_eq!(count(1_000_000), "1.000.000");
        assert_eq!(date("2026-09-19T18:06:32Z"), "19.09.2026");
        assert_eq!(date("soon"), "soon");
        assert_eq!(credits(None), "LP nicht angegeben");
        assert_eq!(languages(Some(false), None), None);
        assert_eq!(time_slot(Some(1), Some("09:15"), Some("10:45")).as_deref(), Some("Mo 09:15–10:45"));
        let winter = Code::Known(TurnusSeason::Winter);
        let odd = Code::Known(TurnusParity::Odd);
        assert_eq!(turnus(Some(&winter), Some(&odd)), "WiSe (ungerade Jahre)");
        assert_eq!(turnus(None, None), "Turnus nicht angegeben");
    }
}
