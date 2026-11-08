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

/// How far a module another asks for is: in place, planned only later or not passed there, not
/// planned. Of modules of which one is enough, the one furthest counts.
fn reach(need: &Need) -> u8 {
    match need.stand {
        _ if in_place(need) => 2,
        Stand::After(_) | Stand::Failed(_) => 1,
        _ => 0,
    }
}

/// Modules of which one is enough, as one entry of what a module asks for.
struct Named {
    /// Their names (`short_title`), each once.
    names: Vec<String>,
    /// Whether any of them is asked for, not only recommended.
    mandatory: bool,
    /// Their choices (`Need::mandatory`, `Need::choice`).
    choices: Vec<(bool, i64)>,
    /// The one furthest (`reach`), the first of those.
    need: Need,
}

impl Named {
    fn take(&mut self, other: Named) {
        for name in other.names {
            if !self.names.contains(&name) {
                self.names.push(name);
            }
        }
        for choice in other.choices {
            if !self.choices.contains(&choice) {
                self.choices.push(choice);
            }
        }
        self.mandatory |= other.mandatory;
        if reach(&other.need) > reach(&self.need) {
            self.need = other.need;
        }
    }
}

/// What module `id` asks for, seen from semester `s`: one entry for the modules of one choice
/// („Programmierpraktikum oder Programming Laboratory (IMT)", 12202) and of one name
/// (`short_title`), as either is enough. One of the name of the module itself (`own`) is left out:
/// it is a version of it, no module before it.
fn named_needs(doc: &PlanDoc, prerequisites: &[Prerequisite], id: &str, own: &str, s: SemesterKey, now: SemesterKey) -> Vec<Named> {
    let mut named: Vec<Named> = Vec::new();
    for need in study::needs(doc, prerequisites, id, s, now) {
        let title = short_title(&need.title).to_string();
        if title == short_title(own) {
            continue;
        }
        let choice = (need.mandatory, need.choice);
        let joins: Vec<usize> = named.iter().enumerate().filter(|(_, known)| known.names.contains(&title) || known.choices.contains(&choice)).map(|(at, _)| at).collect();
        let mut entry = Named { names: vec![title], mandatory: need.mandatory, choices: vec![choice], need };
        // One module may join two entries, of the name of one and the choice of the other: they
        // become one, where the first of them stood.
        for &at in joins.iter().rev() {
            let mut known = named.remove(at);
            known.take(entry);
            entry = known;
        }
        named.insert(joins.first().copied().unwrap_or(named.len()), entry);
    }
    named
}

/// Something module `id` asks for („setzt voraus") is not in place seen from semester `s`: what
/// `needs_text` warns of, and why „Aus dem 4. FS vorziehen" does not suggest it. Of the modules of
/// a choice one in place is enough.
pub(super) fn lacks_required(doc: &PlanDoc, prerequisites: &[Prerequisite], id: &str, own: &str, s: SemesterKey, now: SemesterKey) -> bool {
    named_needs(doc, prerequisites, id, own, s, now).iter().any(|entry| entry.mandatory && !in_place(&entry.need))
}

/// What module `id` asks for, seen from semester `s`: „empfiehlt Mathematik IT-2 ✓, Algorithmen
/// und Datenstrukturen im WiSe 26/27" while everything is in place; where something is not, only
/// that („setzt voraus Programmierpraktikum – noch nicht eingeplant"), and whether that is
/// something asked for (`true`), not only recommended. Modules of one choice or of one name count as
/// one, named by the one furthest, or all of them while none is planned („setzt voraus
/// Programmierpraktikum oder Programming Laboratory – noch nicht eingeplant"); one of the name of
/// the module itself (`own`) is left out.
pub(super) fn needs_text(doc: &PlanDoc, prerequisites: &[Prerequisite], id: &str, own: &str, s: SemesterKey, now: SemesterKey, t: &Texts) -> Option<(String, bool)> {
    let st = &t.study;
    let named = named_needs(doc, prerequisites, id, own, s, now);
    if named.is_empty() {
        return None;
    }
    let word = |entry: &Named| {
        let need = &entry.need;
        let title = match need.stand {
            Stand::Open => entry.names.join(&format!(" {} ", st.either)),
            _ => short_title(&need.title).to_string(),
        };
        match &need.stand {
            Stand::Passed(_) => format!("{title} ✓"),
            Stand::Before(at) => format!("{title} {}", (st.need_before)(&at.short(t.locale))),
            Stand::Same => format!("{title} {}", st.need_same),
            Stand::After(at) => format!("{title} – {}", (st.need_after)(&at.short(t.locale))),
            Stand::Failed(at) => format!("{title} – {}", (st.failed_in)(&at.short(t.locale))),
            Stand::Open => format!("{title} – {}", st.need_open),
        }
    };
    let missing: Vec<&Named> = named.iter().filter(|entry| !in_place(&entry.need)).collect();
    let shown: Vec<&Named> = if missing.is_empty() { named.iter().collect() } else { missing };
    let mut parts = Vec::new();
    for mandatory in [true, false] {
        let list: Vec<String> = shown.iter().filter(|entry| entry.mandatory == mandatory).map(|entry| word(entry)).collect();
        if !list.is_empty() {
            parts.push((if mandatory { st.requires } else { st.recommends })(&list.join(", ")));
        }
    }
    let warn = shown.iter().any(|entry| entry.mandatory && !in_place(&entry.need));
    Some((parts.join(" · "), warn))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use folia_model::labels::Code;

    use super::*;
    use crate::i18n;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    /// 12202 Softwarepraktikum asks for 12102 Programmierpraktikum or 11900 Programmierpraktikum
    /// (IMT), which is not offered any more, and for 12104 (owner, 2026-11-08: „Ansonsten vielleicht
    /// auch checken ob Modul überhaupt belegt werden kann"): with 12102 and 12104 in place it can be
    /// taken.
    #[test]
    fn of_a_choice_one_module_in_place_is_enough() {
        let asks = |required: &str, title: &str, choice: i64| Prerequisite {
            module_id: "12202".into(),
            required_module_id: required.into(),
            kind: Code::parse("mandatory"),
            choice,
            required_title: Some(title.into()),
            required_offer_status: None,
        };
        let prerequisites = [asks("12102", "Programmierpraktikum", 1), asks("11900", "Programming Laboratory (IMT)", 1), asks("12104", "Entwicklung von Softwaresystemen", 2)];
        let (now, summer) = (key("2025W"), key("2026S"));
        let lacks = |doc: &PlanDoc| lacks_required(doc, &prerequisites, "12202", "Softwarepraktikum", summer, now);
        let text = |doc: &PlanDoc| needs_text(doc, &prerequisites, "12202", "Softwarepraktikum", summer, now, &i18n::DE).unwrap();

        // Nothing planned: both alternatives are named, as either would do.
        let mut doc = PlanDoc::default();
        assert!(lacks(&doc));
        assert_eq!(
            text(&doc),
            ("setzt voraus Programmierpraktikum oder Programming Laboratory – noch nicht eingeplant, Entwicklung von Softwaresystemen – noch nicht eingeplant".to_string(), true)
        );

        // 12102 and 12104 passed: 11900 is not asked for any more.
        for id in ["12102", "12104"] {
            doc.plan(now, id, 1, None);
            assert!(doc.pass(now, id));
        }
        assert!(!lacks(&doc));
        assert_eq!(text(&doc), ("setzt voraus Programmierpraktikum ✓, Entwicklung von Softwaresystemen ✓".to_string(), false));

        // Only the alternative planned, and only later: it is named alone, with when.
        let mut later = PlanDoc::default();
        later.plan(now, "12104", 1, None);
        later.plan(key("2026W"), "11900", 1, None);
        assert!(lacks(&later));
        let (said, warn) = text(&later);
        assert!(said.starts_with("setzt voraus Programming Laboratory – erst im "), "{said}");
        assert!(warn);

        // Each a choice of its own, as before: all three are asked for.
        let apart: Vec<Prerequisite> = prerequisites.iter().enumerate().map(|(at, p)| Prerequisite { choice: at as i64 + 1, ..p.clone() }).collect();
        assert!(lacks_required(&doc, &apart, "12202", "Softwarepraktikum", summer, now));
    }

    /// Modules of one name are alternatives even where no choice says so, as before; one module that
    /// is of the name of one entry and of the choice of another joins the two.
    #[test]
    fn modules_of_one_name_or_one_choice_count_as_one() {
        let asks = |required: &str, title: &str, choice: i64| Prerequisite {
            module_id: "99999".into(),
            required_module_id: required.into(),
            kind: Code::parse("mandatory"),
            choice,
            required_title: Some(title.into()),
            required_offer_status: None,
        };
        let prerequisites = [asks("11001", "Mathematik (Teil A)", 1), asks("11002", "Statistik", 2), asks("11003", "Mathematik (Teil B)", 2)];
        let (now, summer) = (key("2025W"), key("2026S"));
        let mut doc = PlanDoc::default();
        doc.plan(now, "11002", 1, None);
        assert!(!lacks_required(&doc, &prerequisites, "99999", "Anderes", summer, now));
        let (said, warn) = needs_text(&doc, &prerequisites, "99999", "Anderes", summer, now, &i18n::DE).unwrap();
        assert!(said.starts_with("setzt voraus Statistik im "), "{said}");
        assert!(!warn);
    }
}
