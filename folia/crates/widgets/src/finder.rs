//! „Passt in meinen Stundenplan": what this browser remembers the finder compares (`finder_on`),
//! for the catalog's switch and the Stundenplan's „+ Modul".

use std::collections::{BTreeMap, BTreeSet};

use folia_calendar::semester::SemesterKey;
use folia_routes::filter::{
    CatalogQuery, FitsFilter,
};
use folia_routes::url::CatalogUrl;
use leptos::prelude::*;

use folia_design::nav;
use folia_stores::studyplan::PlanHint;


/// Where this browser remembers what „Passt in meinen Stundenplan" compares, for the next time it
/// is switched on (owner, 2026-09-26: the choice below it is kept, not reset with every switch):
/// the part of the address that says it (`fits-skip=exam&fits-undated=1`), nothing while it
/// compares everything. A view setting like the width of the panel (R13): never in server HTML.
pub const FINDER_KEY: &str = "betula.finder";

/// The finder switched on for `semester` (the catalog's switch, „+ Modul" and „Modul finden" of the
/// Stundenplan): comparing what it compared when this browser had it on last, everything the
/// first time. The server's page knows nothing of it (R9).
pub fn finder_on(semester: SemesterKey) -> FitsFilter {
    finder_kept(nav::local_get(FINDER_KEY).as_deref(), semester)
}

/// `finder_on` with what is stored. Read as the address is read (what comes from storage is
/// checked like what comes from a URL, R20), and a choice that compares no class at all is none.
pub fn finder_kept(stored: Option<&str>, semester: SemesterKey) -> FitsFilter {
    let all = FitsFilter::all(&semester.key());
    let Some(stored) = stored.filter(|stored| !stored.is_empty()) else { return all };
    CatalogUrl::parse(&format!("fits={}&{stored}", all.semester)).query.fits.filter(|kept| kept.lectures || kept.exercises || kept.exams).unwrap_or(all)
}

/// What `FINDER_KEY` keeps of the finder: its pairs of the address without the semester, written
/// by the address's own codec. Empty while it compares everything (`nav::local_set` then takes
/// the key out).
pub fn finder_text(fits: &FitsFilter) -> String {
    let only_finder = CatalogUrl { query: CatalogQuery { fits: Some(fits.clone()), ..CatalogQuery::default() }, ..CatalogUrl::default() };
    only_finder.to_query_string().split('&').filter(|pair| !pair.starts_with("fits=")).collect::<Vec<_>>().join("&")
}
/// What the finder says beside the list: a small note at a row that fits only in part or could
/// not be checked („Übung 1 von 3 frei", „keine festen Termine"), and a line under the tags when
/// the semester has no dates yet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FitView {
    /// By module id, as the finder words them.
    pub notes: BTreeMap<String, String>,
    /// The modules whose note says why they could not be checked (`FitResult::unknown`): a quiet
    /// note, where a partial fit's warns.
    pub quiet: BTreeSet<String>,
    /// The modules checked that do not clash.
    pub fitting: BTreeSet<String>,
    /// „keine Termine im WiSe 2026/27": with „auch ohne Termine", what a listed module that was
    /// not checked says.
    pub undated_note: Option<String>,
    /// „SoSe 2027: noch keine Termine veröffentlicht.": nothing could be checked.
    pub line: Option<String>,
}

impl FitView {
    /// The note at a module's row, and whether it is a quiet one (not checked, rather than
    /// fitting only in part). `no_termine`: the row says „noch keine Termine" itself, so a note
    /// that would only say the same again („keine festen Termine", „keine Termine im WiSe
    /// 2026/27") is left out (owner, 2026-09-23: „Viel Redundanz").
    pub fn note_of(&self, id: &str, no_termine: bool) -> Option<(String, bool)> {
        match self.notes.get(id) {
            Some(note) if no_termine && note.starts_with(folia_timetable::i18n::texts(crate::i18n::locale()).no_fixed_dates) => None,
            Some(note) => Some((note.clone(), self.quiet.contains(id))),
            None => self.undated_note.clone().filter(|_| !no_termine && !self.fitting.contains(id)).map(|note| (note, true)),
        }
    }
}

/// What the catalog's list tells its rows and its head of the Studienplan (`CatalogPage` provides
/// it; other lists of modules have none): the finder's view, and what „Einplanen" aims at, which
/// a row takes along to the module's page on a phone.
#[derive(Clone, Copy)]
pub struct Finder {
    pub view: Memo<FitView>,
    pub hint: Memo<Option<PlanHint>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switched_on_again_the_finder_compares_what_it_compared_last() {
        let summer = SemesterKey::parse("2027S").unwrap();
        // The first time: every class compared, modules without dates left out.
        assert_eq!(finder_kept(None, summer), FitsFilter::all("2027S"));
        // What it compared is kept as the address says it, and holds for any semester.
        let chosen = FitsFilter { exams: false, undated: true, ..FitsFilter::all("2026W") };
        assert_eq!(finder_text(&chosen), "fits-skip=exam&fits-undated=1");
        assert_eq!(finder_kept(Some(&finder_text(&chosen)), summer), FitsFilter { semester: "2027S".to_string(), ..chosen });
        let lectures_only = FitsFilter { exercises: false, exams: false, ..FitsFilter::all("2026W") };
        assert_eq!(finder_kept(Some(&finder_text(&lectures_only)), summer), FitsFilter { semester: "2027S".to_string(), ..lectures_only });
        // Everything compared keeps nothing: the key goes.
        assert_eq!(finder_text(&FitsFilter::all("2026W")), "");
        assert_eq!(finder_kept(Some(""), summer), FitsFilter::all("2027S"));
        // Read as an address is read: what it does not know is left out, another semester or
        // another filter changes nothing, and a choice that compares nothing is none.
        let odd = finder_kept(Some("fits-skip=EXAM,yoga&fits=1999W&marked=only&fits-undated=yes"), summer);
        assert_eq!(odd, FitsFilter { exams: false, ..FitsFilter::all("2027S") });
        assert_eq!(finder_kept(Some("fits-skip=lecture,exercise,exam"), summer), FitsFilter::all("2027S"));
        assert_eq!(finder_kept(Some("&&=#?"), summer), FitsFilter::all("2027S"));
    }
}
