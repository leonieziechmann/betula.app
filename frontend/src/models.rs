use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleCardItem {
    pub id: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub title_de: String,
    #[serde(default)]
    pub title_en: Option<String>,
    #[serde(default)]
    pub department: Option<String>,
    #[serde(default)]
    pub credits: Option<f64>,
    #[serde(default)]
    pub credits_raw: Option<String>,
    #[serde(default)]
    pub turnus: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub is_fues: Option<i64>,
    #[serde(default)]
    pub cross_disciplinary: Option<i64>,
    #[serde(default)]
    pub is_phase_out: Option<i64>,
    #[serde(default)]
    pub is_not_offered: Option<i64>,
    #[serde(default)]
    pub limitation: Option<String>,
    #[serde(default)]
    pub exam_type: Option<String>,
    #[serde(default)]
    pub successor_modules: Option<String>,
    #[serde(default)]
    pub prerequisites_mandatory: Option<String>,
    #[serde(default)]
    pub prerequisites_recommended: Option<String>,
    #[serde(default)]
    pub events_count: Option<i64>,
    #[serde(default)]
    pub recommended_semester: Option<i64>,
    #[serde(default)]
    pub module_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurriculumModuleItem {
    pub program_id: String,
    pub program_name: String,
    #[serde(default)]
    pub degree: Option<String>,
    #[serde(default)]
    pub po_version: Option<String>,
    #[serde(default)]
    pub recommended_semester: Option<i64>,
    #[serde(default)]
    pub module_type: Option<String>,
    #[serde(default)]
    pub credits: Option<f64>,
    #[serde(default)]
    pub specialization: Option<String>,
}

impl ModuleCardItem {
    pub fn is_fues_module(&self) -> bool {
        self.is_fues.unwrap_or(0) == 1 || self.cross_disciplinary.unwrap_or(0) == 1
    }

    pub fn is_limited(&self) -> bool {
        if let Some(ref lim) = self.limitation {
            let l = lim.trim().to_lowercase();
            !l.is_empty() && l != "keine" && l != "nein" && l != "ohne" && l != "k.a."
        } else {
            false
        }
    }

    pub fn formatted_credits(&self) -> String {
        format_credits(self.credits_raw.as_deref(), self.credits.unwrap_or(0.0))
    }

    pub fn formatted_turnus(&self) -> String {
        format_turnus_short(self.turnus.as_deref().unwrap_or(""))
    }

    pub fn formatted_lang(&self) -> String {
        format_lang_badge(self.language.as_deref().unwrap_or(""))
    }

    pub fn successor_list(&self) -> Vec<String> {
        if let Some(ref succ) = self.successor_modules {
            if succ.starts_with('[') {
                if let Ok(vec) = serde_json::from_str::<Vec<String>>(succ) {
                    return vec;
                }
            }
            return succ.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        }
        Vec::new()
    }
}

pub fn format_credits(raw: Option<&str>, c: f64) -> String {
    if let Some(r) = raw {
        let trimmed = r.trim();
        if !trimmed.is_empty() {
            if trimmed.ends_with(".0") {
                return trimmed.trim_end_matches(".0").to_string();
            }
            return trimmed.to_string();
        }
    }
    if c.fract() == 0.0 {
        format!("{}", c as i64)
    } else {
        format!("{:.1}", c)
    }
}

pub fn format_turnus_short(t: &str) -> String {
    let low = t.trim().to_lowercase();
    if low.is_empty() {
        return "k. A.".to_string();
    }
    if low.contains("jedes semester") || low.contains("every semester") {
        return "🔄 Jedes Sem.".to_string();
    }
    if low.contains("winter") {
        if low.contains("gerad") || low.contains("even") {
            return "❄️ WiSe (ger.)".to_string();
        }
        if low.contains("ungerad") || low.contains("odd") {
            return "❄️ WiSe (ung.)".to_string();
        }
        return "❄️ WiSe".to_string();
    }
    if low.contains("sommer") || low.contains("summer") {
        if low.contains("gerad") || low.contains("even") {
            return "☀️ SoSe (ger.)".to_string();
        }
        if low.contains("ungerad") || low.contains("odd") {
            return "☀️ SoSe (ung.)".to_string();
        }
        return "☀️ SoSe".to_string();
    }
    if low.contains("sporadisch") || low.contains("ankündigung") || low.contains("announcement") {
        return "🎲 Sporadisch".to_string();
    }
    t.to_string()
}

pub fn format_lang_badge(l: &str) -> String {
    let low = l.trim().to_lowercase();
    if low.contains("deutsch") && low.contains("engl") {
        return "🇩🇪/🇬🇧".to_string();
    }
    if low.contains("engl") {
        return "🇬🇧".to_string();
    }
    if low.contains("deutsch") || low.is_empty() {
        return "🇩🇪".to_string();
    }
    l.to_string()
}

pub fn truncate(s: &str, max: usize) -> String {
    let char_count = s.chars().count();
    if char_count > max {
        let cut_len = max.saturating_sub(3);
        let truncated: String = s.chars().take(cut_len).collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleDetail {
    pub id: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub title_de: String,
    #[serde(default)]
    pub title_en: Option<String>,
    #[serde(default)]
    pub is_phase_out: Option<i64>,
    #[serde(default)]
    pub is_not_offered: Option<i64>,
    #[serde(default)]
    pub department: Option<String>,
    #[serde(default)]
    pub responsible_persons: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub turnus: Option<String>,
    #[serde(default)]
    pub credits: Option<f64>,
    #[serde(default)]
    pub credits_raw: Option<String>,
    #[serde(default)]
    pub learning_outcomes: Option<String>,
    #[serde(default)]
    pub contents: Option<String>,
    #[serde(default)]
    pub prerequisites_recommended: Option<String>,
    #[serde(default)]
    pub prerequisites_mandatory: Option<String>,
    #[serde(default)]
    pub teaching_forms: Option<String>,
    #[serde(default)]
    pub literature: Option<String>,
    #[serde(default)]
    pub exam_type: Option<String>,
    #[serde(default)]
    pub exam_details: Option<String>,
    #[serde(default)]
    pub grading: Option<String>,
    #[serde(default)]
    pub limitation: Option<String>,
    #[serde(default)]
    pub is_fues: Option<i64>,
    #[serde(default)]
    pub cross_disciplinary: Option<i64>,
    #[serde(default)]
    pub successor_modules: Option<String>,
    #[serde(default)]
    pub raw_url: Option<String>,
    #[serde(default)]
    pub study_programs: Option<String>,
    #[serde(default)]
    pub remarks: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TeachingFormItem {
    #[serde(rename = "type")]
    pub form_type: Option<String>,
    pub workload: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponsiblePersonItem {
    pub name: Option<String>,
    pub title: Option<String>,
}

impl ModuleDetail {
    pub fn parse_teaching_forms(&self) -> Vec<TeachingFormItem> {
        if let Some(ref tf) = self.teaching_forms {
            if let Ok(vec) = serde_json::from_str::<Vec<TeachingFormItem>>(tf) {
                return vec;
            }
        }
        Vec::new()
    }

    pub fn parse_literature(&self) -> Vec<String> {
        if let Some(ref lit) = self.literature {
            if let Ok(vec) = serde_json::from_str::<Vec<String>>(lit) {
                return vec;
            }
            return lit.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        }
        Vec::new()
    }

    pub fn parse_responsible_persons(&self) -> Vec<String> {
        if let Some(ref rp) = self.responsible_persons {
            if let Ok(vec) = serde_json::from_str::<Vec<ResponsiblePersonItem>>(rp) {
                return vec.into_iter().map(|p| {
                    if let Some(t) = p.title {
                        format!("{} {}", t, p.name.unwrap_or_default())
                    } else {
                        p.name.unwrap_or_default()
                    }
                }).filter(|s| !s.trim().is_empty()).collect();
            }
            if let Ok(vec) = serde_json::from_str::<Vec<String>>(rp) {
                return vec;
            }
            return rp.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        }
        Vec::new()
    }

    pub fn clean_department(&self) -> String {
        let d = self.department.as_deref().unwrap_or("BTU Cottbus-Senftenberg").trim();
        if let Some(idx) = d.rfind('/') {
            if idx + 1 < d.len() {
                return d[idx + 1..].trim().to_string();
            }
        }
        d.to_string()
    }

    pub fn successor_list(&self) -> Vec<String> {
        if let Some(ref succ) = self.successor_modules {
            let s = succ.trim();
            if s.starts_with('[') {
                if let Ok(vec) = serde_json::from_str::<Vec<String>>(s) {
                    return vec;
                }
            }
            return s.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
        }
        Vec::new()
    }

    pub fn formatted_credits(&self) -> String {
        format_credits(self.credits_raw.as_deref(), self.credits.unwrap_or(0.0))
    }

    pub fn formatted_lang(&self) -> String {
        format_lang_badge(self.language.as_deref().unwrap_or(""))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramOption {
    pub id: String,
    pub program_name: String,
    #[serde(default)]
    pub degree: Option<String>,
    #[serde(default)]
    pub po_version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatuteDocument {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(rename = "type", default)]
    pub doc_type: Option<String>,
    #[serde(default)]
    pub abl_number: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub local_path: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OfficialStudyProgramDetail {
    pub id: String,
    pub program_name: String,
    #[serde(default)]
    pub program_code: Option<String>,
    pub degree: String,
    #[serde(default)]
    pub degree_code: Option<String>,
    pub po_version: String,
    #[serde(default)]
    pub qis_node_id: Option<String>,
    #[serde(default)]
    pub qis_url: Option<String>,
    #[serde(default)]
    pub documents: Option<String>,
    #[serde(default)]
    pub scraped_at: Option<String>,
}

impl OfficialStudyProgramDetail {
    pub fn parse_documents(&self) -> Vec<StatuteDocument> {
        if let Some(ref doc_json) = self.documents {
            serde_json::from_str(doc_json).unwrap_or_default()
        } else {
            Vec::new()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventRow {
    pub id: String,
    pub event_number: Option<String>,
    pub title: String,
    pub event_type: Option<String>,
    pub semester: Option<String>,
    pub sws: Option<String>,
    pub raw_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScheduleItem {
    pub day_of_week: Option<String>,
    pub time_slot: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub rhythm: Option<String>,
    pub room: Option<String>,
    pub instructor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventItem {
    pub id: String,
    pub event_number: Option<String>,
    pub title: String,
    pub event_type: Option<String>,
    pub semester: Option<String>,
    pub sws: Option<String>,
    pub raw_url: Option<String>,
    pub schedules: Vec<ScheduleItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CalendarSchedule {
    pub event_id: String,
    pub title: String,
    pub event_type: String,
    pub day_of_week: String,
    pub day_index: i32,
    pub time_slot: String,
    pub start_time: String,
    pub end_time: String,
    pub top_percent: f64,
    pub height_percent: f64,
    pub room: String,
    pub rhythm: String,
    pub instructor: String,
}

pub fn parse_day_of_week(day: &str) -> i32 {
    let low = day.trim().to_lowercase();
    if low.starts_with("mo") { 1 }
    else if low.starts_with("di") { 2 }
    else if low.starts_with("mi") { 3 }
    else if low.starts_with("do") { 4 }
    else if low.starts_with("fr") { 5 }
    else { 0 }
}

pub fn format_day_name(idx: i32) -> &'static str {
    match idx {
        1 => "Mo",
        2 => "Di",
        3 => "Mi",
        4 => "Do",
        5 => "Fr",
        _ => "",
    }
}

pub fn parse_decimal_hour(t: &str) -> f64 {
    let parts: Vec<&str> = t.trim().split(':').collect();
    if parts.len() >= 2 {
        let h = parts[0].trim().parse::<f64>().unwrap_or(0.0);
        let m = parts[1].trim().parse::<f64>().unwrap_or(0.0);
        h + (m / 60.0)
    } else if let Ok(h) = t.trim().parse::<f64>() {
        h
    } else {
        0.0
    }
}

pub fn parse_time_range(start_str: &str, end_str: &str, slot_str: &str) -> (f64, f64) {
    if !start_str.is_empty() && !end_str.is_empty() {
        let s = parse_decimal_hour(start_str);
        let e = parse_decimal_hour(end_str);
        if s > 0.0 && e > 0.0 {
            return (s, e);
        }
    }
    if slot_str.contains("bis") {
        let parts: Vec<&str> = slot_str.split("bis").collect();
        if parts.len() == 2 {
            let s = parse_decimal_hour(parts[0]);
            let e = parse_decimal_hour(parts[1]);
            if s > 0.0 && e > 0.0 {
                return (s, e);
            }
        }
    }
    if slot_str.contains('-') {
        let parts: Vec<&str> = slot_str.split('-').collect();
        if parts.len() == 2 {
            let s = parse_decimal_hour(parts[0]);
            let e = parse_decimal_hour(parts[1]);
            if s > 0.0 && e > 0.0 {
                return (s, e);
            }
        }
    }
    (10.0, 11.5)
}

pub fn compute_calendar_schedules_from_events(events: &[EventItem]) -> Vec<CalendarSchedule> {
    let mut list = Vec::new();
    for evt in events {
        let title_low = evt.title.to_lowercase();
        let type_low = evt.event_type.as_deref().unwrap_or("").to_lowercase();
        if title_low.contains("prüfung") || type_low.contains("prüfung") {
            continue;
        }

        for sc in &evt.schedules {
            let day_str = sc.day_of_week.as_deref().unwrap_or("");
            let day_idx = parse_day_of_week(day_str);
            if day_idx < 1 || day_idx > 5 {
                continue;
            }

            let start_str = sc.start_time.as_deref().unwrap_or("");
            let end_str = sc.end_time.as_deref().unwrap_or("");
            let slot_str = sc.time_slot.as_deref().unwrap_or("");
            let (mut start_dec, mut end_dec) = parse_time_range(start_str, end_str, slot_str);

            if start_dec < 8.0 { start_dec = 8.0; }
            if end_dec > 20.0 { end_dec = 20.0; }
            if end_dec <= start_dec { end_dec = start_dec + 1.5; }

            let top_pct = (start_dec - 8.0) / 12.0 * 100.0;
            let height_pct = (end_dec - start_dec) / 12.0 * 100.0;

            list.push(CalendarSchedule {
                event_id: evt.id.clone(),
                title: evt.title.clone(),
                event_type: evt.event_type.clone().unwrap_or_default(),
                day_of_week: format_day_name(day_idx).to_string(),
                day_index: day_idx,
                time_slot: slot_str.to_string(),
                start_time: if start_str.is_empty() { format!("{:02.0}:{:02.0}", start_dec.floor(), (start_dec.fract()*60.0).round()) } else { start_str.to_string() },
                end_time: if end_str.is_empty() { format!("{:02.0}:{:02.0}", end_dec.floor(), (end_dec.fract()*60.0).round()) } else { end_str.to_string() },
                top_percent: top_pct,
                height_percent: height_pct,
                room: sc.room.clone().unwrap_or_default(),
                rhythm: sc.rhythm.clone().unwrap_or_default(),
                instructor: sc.instructor.clone().unwrap_or_default(),
            });
        }
    }
    list
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventSchedule {
    #[serde(default)]
    pub event_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub event_type: Option<String>,
    #[serde(default)]
    pub day_of_week: Option<String>,
    #[serde(default)]
    pub time_slot: Option<String>,
    #[serde(default)]
    pub start_time: Option<String>,
    #[serde(default)]
    pub end_time: Option<String>,
    #[serde(default)]
    pub room: Option<String>,
    #[serde(default)]
    pub instructor: Option<String>,
    #[serde(default)]
    pub rhythm: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PrereqStatus {
    Met,
    RecommendedMissing(Vec<String>),
    Missing(Vec<String>),
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FilterOptions {
    pub query: String,
    pub program_id: String,
    pub po_version: String,
    pub semester: Option<i64>,
    // Turnus filters
    pub turnus_all: bool,
    pub turnus_next: bool,
    pub turnus_accordion_open: bool,
    pub turnus_wise_even: bool,
    pub turnus_wise_odd: bool,
    pub turnus_sose_even: bool,
    pub turnus_sose_odd: bool,
    pub turnus_sporadic: bool,
    // Filter controls
    pub limitation: String, // "ja", "nein", "nur"
    pub fues: String,       // "inkl", "exkl", "nur"
    pub only_prereqs_met: bool,
    pub hide_phase_out: bool,
    pub min_credits: f64,
    pub max_credits: f64,
    // Campus filters
    pub campus_strict: bool,
    pub campus_hauptcampus: bool,
    pub campus_sachsendorf: bool,
    pub campus_senftenberg: bool,
    // Languages
    pub lang_de: bool,
    pub lang_en: bool,
    // Special views
    pub is_bookmarks_view: bool,
    pub is_completed_view: bool,
    // Sort
    pub sort_by: String, // "id", "title", "ects", "events"
    pub sort_asc: bool,
}

impl Default for FilterOptions {
    fn default() -> Self {
        Self {
            query: String::new(),
            program_id: String::new(),
            po_version: String::new(),
            semester: None,
            turnus_all: true,
            turnus_next: false,
            turnus_accordion_open: false,
            turnus_wise_even: false,
            turnus_wise_odd: false,
            turnus_sose_even: false,
            turnus_sose_odd: false,
            turnus_sporadic: false,
            limitation: "ja".to_string(),
            fues: "inkl".to_string(),
            only_prereqs_met: false,
            hide_phase_out: true,
            min_credits: 0.0,
            max_credits: 30.0,
            campus_strict: false,
            campus_hauptcampus: false,
            campus_sachsendorf: false,
            campus_senftenberg: false,
            lang_de: true,
            lang_en: false,
            is_bookmarks_view: false,
            is_completed_view: false,
            sort_by: "title".to_string(),
            sort_asc: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_unicode() {
        // String with German umlaut 'ö' at potential cut boundary
        let s = "Abschlussprüfung";
        assert_eq!(truncate(s, 15), "Abschlussprü...");
        assert_eq!(truncate(s, 10), "Abschlu...");
        assert_eq!(truncate(s, 20), "Abschlussprüfung");

        // String with emojis and umlauts
        let s2 = "🎓 Informatik & Künstliche Intelligenz";
        assert_eq!(truncate(s2, 10), "🎓 Infor...");
    }
}
