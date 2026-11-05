//! What „Mein Studium" says beside a module it offers (until the workbench, `bench.rs`, the
//! dialog „Module hinzufügen" with three tabs and the Gesamtplan's cell were here; owner,
//! 2026-11-03: „Passt so, bau das jetzt in Folia"): where a semester offers it („jedes WiSe", „im
//! SoSe nicht angeboten · nur WiSe"), its name as a short note has room for, and what it asks for
//! of others, seen from a semester.

use folia_calendar::semester::SemesterKey;
use folia_model::rows::Prerequisite;
use folia_plans::study::{self, Need, Offer, Stand};
use folia_plans::studyplan::PlanDoc;

use crate::i18n::Texts;

/// „jedes SoSe", „jedes Semester", „im SoSe nicht angeboten · nur WiSe".
pub(super) fn offer_note(offer: Offer, s: SemesterKey, t: &Texts) -> (&'static str, String) {
    let st = &t.study;
    let season = |winter: bool| if winter { t.format.winter_short } else { t.format.summer_short };
    if !offer.offered(s) {
        let only = offer.only().map(|only| format!(" · {}", if only == folia_model::labels::Season::Winter { st.only_winter } else { st.only_summer })).unwrap_or_default();
        return ("warn", format!("{}{only}", (st.unoffered)(season(s.winter))));
    }
    match offer.only() {
        Some(only) => ("", (st.every)(season(only == folia_model::labels::Season::Winter))),
        None => ("", st.every_semester.to_string()),
    }
}

/// The name of a module as a short note has room for: what stands before a bracket
/// („Mathematik IT-1 (Diskrete Mathematik)" → „Mathematik IT-1"). Two modules of one name with
/// different brackets („Programmierpraktikum (IMT)") are alternatives of each other.
pub(super) fn short_title(title: &str) -> &str {
    match title.split_once(" (") {
        Some((before, _)) if before.chars().count() >= 4 => before,
        _ => title,
    }
}

/// A module another asks for is in place seen from a semester: passed before it, planned before
/// it or in it.
fn in_place(need: &Need) -> bool {
    matches!(need.stand, Stand::Passed(_) | Stand::Before(_) | Stand::Same)
}

/// What module `id` asks for, seen from semester `s`, one entry per name (`short_title`): the
/// name, whether any module of it is asked for, not only recommended, and the one most in place
/// (a name is in place where any of its modules is). One of the name of the module itself
/// (`own`) is left out: it is a version of it, no module before it.
fn named_needs(doc: &PlanDoc, prerequisites: &[Prerequisite], id: &str, own: &str, s: SemesterKey, now: SemesterKey) -> Vec<(String, bool, Need)> {
    let mut named: Vec<(String, bool, Need)> = Vec::new();
    for need in study::needs(doc, prerequisites, id, s, now) {
        let title = short_title(&need.title);
        if title == short_title(own) {
            continue;
        }
        match named.iter_mut().find(|(known, ..)| known == title) {
            Some(entry) => {
                entry.1 |= need.mandatory;
                if in_place(&need) && !in_place(&entry.2) {
                    entry.2 = need;
                }
            }
            None => named.push((title.to_string(), need.mandatory, need)),
        }
    }
    named
}

/// Something module `id` asks for („setzt voraus") is not in place seen from semester `s`: what
/// `needs_text` warns of, and why „Aus dem 4. FS vorziehen" does not suggest it.
pub(super) fn lacks_required(doc: &PlanDoc, prerequisites: &[Prerequisite], id: &str, own: &str, s: SemesterKey, now: SemesterKey) -> bool {
    named_needs(doc, prerequisites, id, own, s, now).iter().any(|(_, mandatory, need)| *mandatory && !in_place(need))
}

/// What module `id` asks for, seen from semester `s`: „empfiehlt Mathematik IT-2 ✓, Algorithmen
/// und Datenstrukturen im WiSe 26/27" while everything is in place; where something is not, only
/// that („setzt voraus Programmierpraktikum – noch nicht eingeplant"), and whether that is
/// something asked for (`true`), not only recommended. Modules of one name count as one; one of
/// the name of the module itself (`own`) is left out.
pub(super) fn needs_text(doc: &PlanDoc, prerequisites: &[Prerequisite], id: &str, own: &str, s: SemesterKey, now: SemesterKey, t: &Texts) -> Option<(String, bool)> {
    let st = &t.study;
    let named = named_needs(doc, prerequisites, id, own, s, now);
    if named.is_empty() {
        return None;
    }
    let word = |title: &str, need: &Need| match &need.stand {
        Stand::Passed(_) => format!("{title} ✓"),
        Stand::Before(at) => format!("{title} {}", (st.need_before)(&at.short(t.locale))),
        Stand::Same => format!("{title} {}", st.need_same),
        Stand::After(at) => format!("{title} – {}", (st.need_after)(&at.short(t.locale))),
        Stand::Failed(at) => format!("{title} – {}", (st.failed_in)(&at.short(t.locale))),
        Stand::Open => format!("{title} – {}", st.need_open),
    };
    let missing: Vec<&(String, bool, Need)> = named.iter().filter(|(.., need)| !in_place(need)).collect();
    let shown: Vec<&(String, bool, Need)> = if missing.is_empty() { named.iter().collect() } else { missing };
    let mut parts = Vec::new();
    for mandatory in [true, false] {
        let list: Vec<String> = shown.iter().filter(|(_, m, _)| *m == mandatory).map(|(title, _, need)| word(title, need)).collect();
        if !list.is_empty() {
            parts.push((if mandatory { st.requires } else { st.recommends })(&list.join(", ")));
        }
    }
    let warn = shown.iter().any(|(_, mandatory, need)| *mandatory && !in_place(need));
    Some((parts.join(" · "), warn))
}
