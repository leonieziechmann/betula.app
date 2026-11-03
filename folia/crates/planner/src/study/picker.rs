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

/// What module `id` asks for, seen from semester `s`: „empfiehlt Mathematik IT-2 ✓, Algorithmen
/// und Datenstrukturen im WiSe 26/27" while everything is in place; where something is not, only
/// that („setzt voraus Programmierpraktikum – noch nicht eingeplant"), and whether that is
/// something asked for (`true`), not only recommended. Modules of one name count as one; one of
/// the name of the module itself (`own`) is left out.
pub(super) fn needs_text(doc: &PlanDoc, prerequisites: &[Prerequisite], id: &str, own: &str, s: SemesterKey, now: SemesterKey, t: &Texts) -> Option<(String, bool)> {
    let st = &t.study;
    // A module of its own name is a version of it, no module before it.
    let needs: Vec<Need> = study::needs(doc, prerequisites, id, s, now).into_iter().filter(|need| short_title(&need.title) != short_title(own)).collect();
    if needs.is_empty() {
        return None;
    }
    let fine = |need: &Need| matches!(need.stand, Stand::Passed(_) | Stand::Before(_) | Stand::Same);
    // One entry per name: in place where any of its modules is.
    let mut named: Vec<(&str, bool, &Need)> = Vec::new();
    for need in &needs {
        let title = short_title(&need.title);
        match named.iter_mut().find(|(known, ..)| *known == title) {
            Some(entry) => {
                entry.1 |= need.mandatory;
                if fine(need) && !fine(entry.2) {
                    entry.2 = need;
                }
            }
            None => named.push((title, need.mandatory, need)),
        }
    }
    let word = |title: &str, need: &Need| match &need.stand {
        Stand::Passed(_) => format!("{title} ✓"),
        Stand::Before(at) => format!("{title} {}", (st.need_before)(&at.short(t.locale))),
        Stand::Same => format!("{title} {}", st.need_same),
        Stand::After(at) => format!("{title} – {}", (st.need_after)(&at.short(t.locale))),
        Stand::Failed(at) => format!("{title} – {}", (st.failed_in)(&at.short(t.locale))),
        Stand::Open => format!("{title} – {}", st.need_open),
    };
    let missing: Vec<&(&str, bool, &Need)> = named.iter().filter(|(.., need)| !fine(need)).collect();
    let shown: Vec<&(&str, bool, &Need)> = if missing.is_empty() { named.iter().collect() } else { missing };
    let mut parts = Vec::new();
    for mandatory in [true, false] {
        let list: Vec<String> = shown.iter().filter(|(_, m, _)| *m == mandatory).map(|(title, _, need)| word(title, need)).collect();
        if !list.is_empty() {
            parts.push((if mandatory { st.requires } else { st.recommends })(&list.join(", ")));
        }
    }
    let warn = shown.iter().any(|(_, mandatory, need)| *mandatory && !fine(need));
    Some((parts.join(" · "), warn))
}
