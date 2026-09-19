use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgramOption {
    pub id: String,
    pub program_name: String,
    pub degree: Option<String>,
    pub po_version: Option<String>,
}

pub fn format_degree_short(degree: Option<&str>) -> String {
    match degree {
        Some(d) => {
            let d_trim = d.trim();
            let d_lower = d_trim.to_lowercase();
            if d_lower.contains("keine abschlussprüfung")
                || d_lower.contains("ohne abschluss")
                || d_lower.contains("kein abschluss")
                || d_lower.contains("abschluss im ausland")
            {
                String::new()
            } else if d_trim.contains("Bachelor of Science") || d_trim.eq_ignore_ascii_case("B.Sc.") || d_trim.eq_ignore_ascii_case("B.Sc") {
                "B.Sc.".to_string()
            } else if d_trim.contains("Master of Science") || d_trim.eq_ignore_ascii_case("M.Sc.") || d_trim.eq_ignore_ascii_case("M.Sc") {
                "M.Sc.".to_string()
            } else if d_trim.contains("Bachelor of Arts") || d_trim.eq_ignore_ascii_case("B.A.") || d_trim.eq_ignore_ascii_case("B.A") {
                "B.A.".to_string()
            } else if d_trim.contains("Master of Arts") || d_trim.eq_ignore_ascii_case("M.A.") || d_trim.eq_ignore_ascii_case("M.A") {
                "M.A.".to_string()
            } else if d_trim.contains("Bachelor of Engineering") || d_trim.eq_ignore_ascii_case("B.Eng.") || d_trim.eq_ignore_ascii_case("B.Eng") {
                "B.Eng.".to_string()
            } else if d_trim.contains("Master of Engineering") || d_trim.eq_ignore_ascii_case("M.Eng.") || d_trim.eq_ignore_ascii_case("M.Eng") {
                "M.Eng.".to_string()
            } else if d_trim.contains("Bachelor of Education") || d_trim.eq_ignore_ascii_case("B.Ed.") {
                "B.Ed.".to_string()
            } else if d_trim.contains("Master of Education") || d_trim.eq_ignore_ascii_case("M.Ed.") {
                "M.Ed.".to_string()
            } else if d_lower.contains("bachelor") {
                "B.Sc.".to_string()
            } else if d_lower.contains("master") {
                "M.Sc.".to_string()
            } else if d_lower.contains("diplom") {
                "Dipl.".to_string()
            } else if d_lower.contains("staatsexamen") {
                "StEx".to_string()
            } else if d_lower.contains("zertifikat") {
                "Zertifikat".to_string()
            } else if !d_trim.is_empty() && d_trim.chars().count() <= 12 {
                d_trim.to_string()
            } else {
                String::new()
            }
        }
        None => String::new(),
    }
}

pub fn slugify(s: &str) -> String {
    let transliterated = s
        .to_lowercase()
        .replace('ä', "ae")
        .replace('ö', "oe")
        .replace('ü', "ue")
        .replace('ß', "ss");
    transliterated
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn base_slug(p: &ProgramOption) -> String {
    let degree = format_degree_short(p.degree.as_deref()).replace('.', "");
    let year = p
        .po_version
        .as_deref()
        .unwrap_or("")
        .split(|c: char| !c.is_ascii_digit())
        .find(|s| s.len() == 4)
        .unwrap_or("ohne-po");
    let mut base = format!("{}-{}-{}", slugify(&degree), slugify(&p.program_name), year)
        .trim_start_matches('-')
        .to_string();
    let d = p.degree.as_deref().unwrap_or("").to_lowercase();
    for (needle, label) in [
        ("ausbildungsintegrierend", "ausbildung"),
        ("praxisintegrierend", "praxis"),
        ("fern", "fernstudium"),
        ("teilzeit", "teilzeit"),
        ("erweiterte", "erweitert"),
        ("verringerte", "verkuerzt"),
        ("doppelabschluss", "doppelabschluss"),
    ] {
        if d.contains(needle) {
            base.push('-');
            base.push_str(label);
        }
    }
    base
}

pub fn program_slug(id: &str, programs: &[ProgramOption]) -> String {
    let Some(p) = programs.iter().find(|p| p.id == id) else {
        return id.to_string();
    };
    let base = base_slug(p);
    let collisions: Vec<_> = programs.iter().filter(|other| base_slug(other) == base).collect();
    if collisions.len() == 1 {
        return base;
    }
    let suffix = format!(
        "{}-{}",
        slugify(p.po_version.as_deref().unwrap_or("")),
        slugify(p.degree.as_deref().unwrap_or(""))
    );
    let candidate = format!("{base}-{suffix}");
    if collisions
        .iter()
        .filter(|other| other.po_version == p.po_version && other.degree == p.degree)
        .count()
        == 1
    {
        return candidate;
    }
    let hash = id
        .bytes()
        .fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    format!("{candidate}-{hash:08x}")
}

pub fn resolve_program(token: &str, programs: &[ProgramOption]) -> String {
    if programs.iter().any(|p| p.id == token) {
        return token.to_string();
    }
    programs
        .iter()
        .find(|p| program_slug(&p.id, programs) == token)
        .map(|p| p.id.clone())
        .unwrap_or_else(|| token.to_string())
}

#[cfg(test)]
#[path = "slug_tests.rs"]
mod slug_tests;
