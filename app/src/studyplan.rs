//! The Studienplan in this browser: its store, and where „Einplanen" plans a module to.
//!
//! What a visitor plans is theirs alone (R20, owner decision 2026-09-24): the plan lives in this
//! browser's `localStorage` (`betula.studyplan.v1`, the text of `catalog::studyplan::PlanDoc`) and
//! nowhere else. It is empty on the server and in a browser that refuses storage (R9), never part
//! of a URL, and it reaches no request: the pages ask the local copy of the catalog about it. Its
//! text is read like a URL (`PlanDoc::restored`): a line that fails is dropped alone, lines of a
//! newer build are kept and written back.
//!
//! Every control reads what it shows through a memo of its own (R5), so a change of the plan
//! changes those controls and what is computed from the plan, not the page around them. A change
//! whose consequences query the catalog or re-run the finder (planning, removing, moving a module,
//! taking a Regelstudienplan over) is written after the next frame (`update_after_paint`): the
//! control that was clicked flips its own state first, so the click answers in the next frame
//! (R21) whatever the work then costs.

use std::collections::BTreeSet;

use catalog::labels::TurnusSeason;
use catalog::studyplan::PlanDoc;
use catalog::timetable::select::{Selection, TownChoice};
use catalog::timetable::semester::SemesterKey;
use catalog::url::{self, LocalView, StudyplanUrl};
use leptos::prelude::*;

use crate::nav;

const STORAGE_KEY: &str = "betula.studyplan.v1";

/// The visitor's Studienplan, shared through context. Empty on the server, always.
#[derive(Clone, Copy)]
pub struct Studyplan(RwSignal<PlanDoc>);

impl Studyplan {
    /// Reads what this browser has stored and provides the plan. Another tab of the same browser
    /// may change it; this one follows.
    pub fn provide() -> Self {
        let plan = Studyplan(RwSignal::new(load()));
        provide_context(plan);
        // Effects run in the browser only.
        Effect::new(move |_| {
            let handle = window_event_listener_untyped("storage", move |_| {
                let stored = load();
                if plan.0.with_untracked(|doc| *doc != stored) {
                    plan.0.set(stored);
                }
            });
            on_cleanup(move || handle.remove());
        });
        plan
    }

    pub fn expect() -> Option<Self> {
        use_context::<Studyplan>()
    }

    /// Reads the plan. Tracked: read it in a memo that keeps only what it needs, never in a list.
    pub fn with<R>(self, f: impl FnOnce(&PlanDoc) -> R) -> R {
        self.0.with(f)
    }

    /// The same without subscribing: for handlers that build the next state on top of it.
    pub fn with_untracked<R>(self, f: impl FnOnce(&PlanDoc) -> R) -> R {
        self.0.with_untracked(f)
    }

    /// Changes the plan and stores it. What depends on the plan hears of it only when something
    /// changed; an empty plan takes the key out of the storage (`nav::local_set`).
    pub fn update<R>(self, f: impl FnOnce(&mut PlanDoc) -> R) -> R {
        let mut doc = self.0.try_with_untracked(Clone::clone).unwrap_or_default();
        let result = f(&mut doc);
        if self.0.try_with_untracked(|now| *now != doc).unwrap_or(false) {
            nav::local_set(STORAGE_KEY, &stored_text(&doc));
            self.0.set(doc);
        }
        result
    }

    /// For a change whose consequences query the catalog or re-run the finder: `f` runs once
    /// after the next frame (`nav::after_paint`). The control that calls it flips its own state
    /// first, so it answers in the next frame (R21) and the pages follow after.
    pub fn update_after_paint(self, f: impl FnOnce(&mut PlanDoc) + 'static) {
        nav::after_paint(move || self.update(f));
    }

    /// Whether nothing is planned (no module, no placeholder). Tracked.
    pub fn is_empty(self) -> bool {
        self.0.with(PlanDoc::is_empty)
    }

    /// How many modules are planned (`planned_modules`). Tracked: read it in a memo.
    pub fn count(self) -> usize {
        self.0.with(planned_modules)
    }

    /// The modules planned into a semester, in the order they were planned. Tracked.
    pub fn modules_in(self, s: SemesterKey) -> Vec<String> {
        self.0.with(|doc| doc.modules_in(s))
    }

    /// Whether the module is planned into the semester. Tracked: read it in a memo per button (R5).
    pub fn is_planned(self, s: SemesterKey, id: &str) -> bool {
        self.0.with(|doc| doc.is_planned(s, id))
    }

    /// What the semester hides and has chosen, with the town. Tracked.
    pub fn selection(self, s: SemesterKey, town: TownChoice) -> Selection {
        self.0.with(|doc| doc.selection(s, town))
    }
}

/// What this browser has stored. Nothing on the server, and nothing in a browser that refuses
/// storage (a plan then lasts as long as the page).
fn load() -> PlanDoc {
    restored(nav::local_get(STORAGE_KEY).as_deref())
}

/// The plan of a stored text, read like anything from outside: garbage is dropped line by line,
/// never an error, and no text reads as more than the caps allow.
fn restored(stored: Option<&str>) -> PlanDoc {
    stored.map(PlanDoc::restored).unwrap_or_default()
}

/// How many modules a plan holds, each once however many semesters it is planned into (a retake):
/// the number the rail's „Plan" shows.
fn planned_modules(doc: &PlanDoc) -> usize {
    doc.modules.iter().map(|planned| planned.module_id.as_str()).collect::<BTreeSet<_>>().len()
}

/// The text a plan is stored as. Empty only when there is nothing at all to keep, and then the key
/// goes (`nav::local_set`): a plan without modules may still hold hidden Termine, a subscription's
/// code or a newer build's lines, which must survive (`PlanDoc::is_empty` is not the test).
fn stored_text(doc: &PlanDoc) -> String {
    doc.stored()
}

/// What a page asks „Einplanen" to plan into: the semester the catalog's „Passt in meinen Plan"
/// was checked against (`plan=`), and the placeholder a module found for it would fill (`fill=`),
/// whose semester wins (`target_semester`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanHint {
    pub semester: Option<SemesterKey>,
    pub fill: Option<u32>,
}

/// The semester „Einplanen" plans a module into (A.9), the first that applies: the page's hint
/// (the semester of the placeholder it fills while the plan still has it, else the semester the
/// finder checked); the semester of the module's newest events, when that is not over; the next
/// semester whose half of the year the module is offered in; the current one.
///
/// The placeholder wins over the finder's semester: a module planned for a placeholder fills it
/// (`fills=<pid>`, A.7), which means something only in the placeholder's own semester. The two
/// differ once the finder's switch was turned off and on again (it then checks the current
/// semester) while `fill` stayed in the address.
pub fn target_semester(current: SemesterKey, newest: Option<SemesterKey>, turnus: Option<TurnusSeason>, hint: Option<&PlanHint>, doc: &PlanDoc) -> SemesterKey {
    if let Some(hint) = hint {
        let filled = hint.fill.and_then(|pid| doc.placeholders.iter().find(|p| p.pid == pid)).map(|p| p.semester);
        if let Some(semester) = filled.or(hint.semester) {
            return semester;
        }
    }
    if let Some(newest) = newest.filter(|newest| *newest >= current) {
        return newest;
    }
    let winter = match turnus {
        Some(TurnusSeason::Winter) => Some(true),
        Some(TurnusSeason::Summer) => Some(false),
        _ => None,
    };
    match winter {
        Some(winter) if winter != current.winter => current.plus(1).unwrap_or(current),
        _ => current,
    }
}

/// The Studienplan's address as a local view (`url::LocalView`, `crate::local`), so that `open`
/// and `full` mean what they mean on every page that has them: `open=<id>` is the module beside
/// the plan, and `full=1` lets it fill the plan's place with the module's whole page, inside the
/// Studienplan's area (tab, history and „Zurück" stay the plan's, the catalog's tab never hears of
/// it). What stands beside the plan is the plan's own panel of the module (its Termine and what is
/// chosen of them), not the module's preview, so on a phone `open` alone shows that panel as the
/// page and only `full` fills it with the module: the plan asks `local::filling` as a desktop
/// does. `StudyplanUrl` says the rest; `full` is written last.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanAddress {
    pub url: StudyplanUrl,
    /// The module of `open` fills the page. Nothing without `open`.
    pub full: bool,
}

impl PlanAddress {
    /// Tolerant like every address: `full` only as `full=1` and only with a module in `open`.
    pub fn parse(raw_query: &str) -> Self {
        let url = StudyplanUrl::parse(raw_query);
        let (_, full) = url::local_from_pairs(&url::parse_pairs(raw_query));
        Self { full: full && url.open.is_some(), url }
    }

    pub fn path(&self) -> String {
        let path = self.url.path();
        match (self.full, &self.url.open) {
            (true, Some(_)) => format!("{path}&full=1"),
            _ => path,
        }
    }
}

impl LocalView for PlanAddress {
    fn open(&self) -> Option<&str> {
        self.url.open.as_deref()
    }

    fn full(&self) -> bool {
        self.full
    }

    /// The Termin the panel pointed at stays while the module does.
    fn with_module(&self, open: Option<&str>, full: bool) -> Self {
        let url = if open == self.url.open.as_deref() { self.url.clone() } else { self.url.with_open(open, None) };
        Self { full: full && url.open.is_some(), url }
    }

    fn path(&self) -> String {
        PlanAddress::path(self)
    }
}

#[cfg(test)]
mod tests {
    use catalog::url::PlanView;

    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    #[test]
    fn what_is_stored_is_read_like_anything_from_outside() {
        // Nothing stored, or garbage: an empty plan, never an error.
        assert!(restored(None).is_empty());
        let garbage = restored(Some("<script>alert(1)</script>\nm\t2026X\t12104\t0\t\nm\t2026W\t../../etc\t0\t\n\u{0}\u{7f}\n"));
        assert!(garbage.is_empty() && garbage.modules.is_empty() && garbage.placeholders.is_empty());
        // A good line among bad ones survives alone.
        let mixed = restored(Some("m\t2026W\t12104\t1790000000\t\nm\tWiSe\t12107\t0\t\ne\t2026W\t0\n"));
        assert_eq!(mixed.modules_in(key("2026W")), vec!["12104".to_string()]);
        assert!(!mixed.is_empty());
    }

    #[test]
    fn an_empty_plan_takes_its_key_out_and_a_plan_without_modules_keeps_what_it_hides() {
        // Nothing at all: the empty text, which `nav::local_set` turns into removing the key.
        assert_eq!(stored_text(&PlanDoc::default()), "");
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key("2026W"), "12104", 1, None));
        doc.set_event(key("2026W"), 149408, true);
        assert!(stored_text(&doc).contains("m\t2026W\t12104") && restored(Some(&stored_text(&doc))) == doc);
        // The last module goes; what the semester hides stays, so the key stays too.
        doc.unplan(key("2026W"), "12104", &[]);
        assert!(doc.is_empty() && !stored_text(&doc).is_empty());
        // Showing everything again leaves nothing to keep.
        doc.show_all(key("2026W"));
        assert_eq!(stored_text(&doc), "");
    }

    #[test]
    fn the_rail_counts_each_planned_module_once() {
        let mut doc = PlanDoc::default();
        assert_eq!(planned_modules(&doc), 0);
        assert!(doc.plan(key("2026W"), "12104", 1, None) && doc.plan(key("2026W"), "12107", 1, None));
        // A retake in the next winter is the same module.
        assert!(doc.plan(key("2027W"), "12104", 2, None));
        assert_eq!(planned_modules(&doc), 2);
    }

    #[test]
    fn einplanen_aims_at_the_semester_that_fits() {
        let now = key("2026W");
        let doc = PlanDoc::default();
        // The page's hint wins, over the module's own data.
        let hint = PlanHint { semester: Some(key("2027S")), fill: None };
        assert_eq!(target_semester(now, Some(now), Some(TurnusSeason::Winter), Some(&hint), &doc), key("2027S"));
        // Events of the current semester or a later one: that semester.
        assert_eq!(target_semester(now, Some(now), Some(TurnusSeason::Summer), None, &doc), now);
        assert_eq!(target_semester(now, Some(key("2027S")), None, None, &doc), key("2027S"));
        // Only past events: the next semester of its half of the year.
        assert_eq!(target_semester(now, Some(key("2026S")), Some(TurnusSeason::Summer), None, &doc), key("2027S"));
        assert_eq!(target_semester(now, Some(key("2026S")), Some(TurnusSeason::Winter), None, &doc), now);
        // Every semester, irregular or unknown: the current one.
        assert_eq!(target_semester(now, Some(key("2026S")), Some(TurnusSeason::Both), None, &doc), now);
        assert_eq!(target_semester(now, None, Some(TurnusSeason::Irregular), None, &doc), now);
        assert_eq!(target_semester(now, None, None, None, &doc), now);
        // No data, a winter module, in a winter: the current winter.
        assert_eq!(target_semester(now, None, Some(TurnusSeason::Winter), None, &doc), now);
        // A summer: the winter to come.
        assert_eq!(target_semester(key("2027S"), None, Some(TurnusSeason::Winter), None, &doc), key("2027W"));
        // A placeholder's hint without a semester plans into the placeholder's semester.
        let mut with_placeholder = PlanDoc::default();
        with_placeholder.placeholders.push(catalog::studyplan::Placeholder {
            pid: 3,
            semester: key("2027W"),
            program_id: "079-82-2008".into(),
            ord: 17,
            span: (3, 3),
            credits: Some("6".into()),
            kind: None,
            caption: String::new(),
            name: "Fachübergreifendes Studium".into(),
        });
        let fill = PlanHint { semester: None, fill: Some(3) };
        assert_eq!(target_semester(now, Some(now), None, Some(&fill), &with_placeholder), key("2027W"));
        // The finder checked another semester („Passt in meinen Plan" off and on again, `fill`
        // kept): the placeholder still decides, so the module lands where it fills it.
        let both = PlanHint { semester: Some(now), fill: Some(3) };
        assert_eq!(target_semester(now, Some(now), None, Some(&both), &with_placeholder), key("2027W"));
        // One the plan no longer has changes nothing: the finder's semester, else the module's.
        let gone = PlanHint { semester: None, fill: Some(9) };
        assert_eq!(target_semester(now, Some(now), None, Some(&gone), &with_placeholder), now);
        let gone_but_checked = PlanHint { semester: Some(key("2027S")), fill: Some(9) };
        assert_eq!(target_semester(now, Some(now), None, Some(&gone_but_checked), &with_placeholder), key("2027S"));
    }

    #[test]
    fn the_plan_is_a_local_view() {
        let beside = PlanAddress::parse("sem=2026W&open=12104&row=148369-aaf38");
        assert!(!beside.full && beside.url.row.as_deref() == Some("148369-aaf38"));
        // „Vollbild": the same page filled with the module; the Termin pointed at stays.
        let full = beside.with_full(true);
        assert_eq!(full.path(), "/studyplan?sem=2026W&open=12104&row=148369-aaf38&full=1");
        assert_eq!(PlanAddress::parse("sem=2026W&open=12104&row=148369-aaf38&full=1"), full);
        assert_eq!(full.with_full(false), beside);
        // Another module: its own Termine, no row of the one before.
        assert_eq!(beside.with_open(Some("12107")).path(), "/studyplan?sem=2026W&open=12107");
        assert_eq!(beside.with_open(None).path(), "/studyplan?sem=2026W");
        // `full` is nothing without a module, nor anything but `full=1`.
        assert_eq!(PlanAddress::parse("view=dates&full=1"), PlanAddress { url: StudyplanUrl { view: PlanView::Dates, ..Default::default() }, full: false });
        assert!(!PlanAddress::parse("open=12104&full=yes").full);
        assert!(!PlanAddress::parse("open=<x>&full=1").full);
        // Only „Vollbild" fills the plan's page, on a phone as well: `open` alone is the panel.
        assert_eq!(crate::local::filling(&full, false).as_deref(), Some("12104"));
        assert_eq!(crate::local::filling(&beside, false), None);
        assert_eq!(crate::local::back_href(&full, false), "/studyplan?sem=2026W&open=12104&row=148369-aaf38");
    }
}
