//! The semesters of „Einplanen" (owner, 2026-10-17: „wenn man es lange gedrückt hält (auf dem
//! handy) soll dabei sich die border aufladen und wrnn das voll ist soll ein dropdown mit so 3 bis 5
//! semestern kommen in die man das einplanen kann. Also das aktuelle und dann die nächsten … wenn man
//! das Semester noch nicht angelegt hat soll das trotzdem gehen"): the menu of its switch
//! (`folia_design::holdmenu`, which opens it the same way at every switch that has one) with the
//! current semester and the four after it (`semesters`), each planning the module there or taking
//! it out. A semester nothing is planned into yet takes it all the same: the plan has a semester
//! once something is in it, and „Mein Studium" and the Stundenplan show it from then on. Beside
//! „Merken" and among the actions of a module page's sidebar alike (owner, the same day, of
//! „Anderes Semester", which the sidebar had until then: „ich finde das un intuitiv und eigentlich
//! will ich, dass man es machen kann ohne vollbild. Und es soll halt einheitlich sein").

use std::collections::BTreeSet;

use folia_calendar::semester::SemesterKey;
use folia_design::holdmenu::{HoldItem, MenuOf};
use folia_model::labels::TurnusSeason;
use folia_plans::study::Offer;
use folia_plans::studyplan::PlanDoc;
use leptos::prelude::*;

use super::{aim_of, press, semester_entry, Aim, PlanHint, Studyplan};
use crate::i18n::{self, Texts};
use folia_data::use_data;

/// The semesters after the current one the menu offers: with it, five.
const AHEAD: i32 = 4;

/// The module a menu plans, as its switch was given it (`PlanButton`).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Planning {
    pub id: String,
    pub title: String,
    pub current: SemesterKey,
    pub newest: Option<SemesterKey>,
    pub turnus: Option<TurnusSeason>,
    pub offer: Offer,
    /// The page's hint as it was when the menu opened.
    pub hint: Option<PlanHint>,
}

/// Whose menu it is: the switch of a module where it stands (`hero`, beside „Merken"; `side`, in
/// the sidebar), so that the focus goes back to the one it was opened at.
pub(super) fn owner_of(id: &str, place: &str) -> String {
    format!("plan:{place}:{id}")
}

/// The menu of the switches of `what`, as the plan is when it opens: its semesters, the one the
/// switch plans into the entry its click means.
pub(super) fn menu_of(what: Planning, owner: String, plan: Studyplan, start: Option<SemesterKey>, t: &'static Texts) -> MenuOf {
    let (semesters, target) = plan.with_untracked(|doc| (semesters(&what, doc), aim_of(&what.id, what.current, what.newest, what.turnus, what.hint.as_ref(), doc).target));
    let (label, rows) = ((t.planner.semesters_of)(&what.title), semesters.len());
    let entries = ViewFn::from(move || {
        semesters
            .iter()
            .enumerate()
            .map(|(i, semester)| {
                let label = semester_entry(*semester, start, t);
                view! { <Entry plan what=what.clone() semester=*semester n=i + 1 label default=*semester == target/> }
            })
            .collect_view()
    });
    MenuOf { owner, label, head: t.planner.plan_into.to_string(), rows, entries }
}

/// The semesters the menu offers: the current one and the four after it, and the one the switch
/// plans into should it lie beyond them (the finder's, a placeholder's).
pub(super) fn semesters(what: &Planning, doc: &PlanDoc) -> Vec<SemesterKey> {
    let target = aim_of(&what.id, what.current, what.newest, what.turnus, what.hint.as_ref(), doc).target;
    let mut all: BTreeSet<SemesterKey> = (0..=AHEAD).filter_map(|n| what.current.plus(n)).collect();
    all.insert(target);
    all.into_iter().collect()
}

/// What taking a semester of the menu does: in the semester the switch aims at, what a click on
/// the switch does (for its placeholder, too); in any other, planning the module there or taking
/// it out of it.
pub(super) fn aim_in(what: &Planning, semester: SemesterKey, doc: &PlanDoc) -> Aim {
    let aim = aim_of(&what.id, what.current, what.newest, what.turnus, what.hint.as_ref(), doc);
    if aim.target == semester {
        return aim;
    }
    Aim { target: semester, pressed: doc.is_planned(semester, &what.id), fill: None, elsewhere: Vec::new() }
}

/// What a semester says at its end: „geplant" where the module is planned, else „nicht
/// angeboten" where its turnus leaves the semester out (R12: what the catalog does not state does
/// not restrict).
fn note_of(what: &Planning, semester: SemesterKey, planned: bool, t: &'static Texts) -> Option<(&'static str, &'static str)> {
    if planned {
        Some((t.planner.planned_mark, "on"))
    } else if !what.offer.offered(semester) {
        Some((t.planner.not_offered, "warn"))
    } else {
        None
    }
}

/// One semester of the menu: planned there or not, flipped at once by a click (R21) and written
/// after the next frame as the switch's click is (`press`).
#[component]
fn Entry(plan: Studyplan, what: Planning, semester: SemesterKey, n: usize, label: String, default: bool) -> impl IntoView {
    let t = i18n::t();
    let source = use_data().ok();
    let planned = {
        let what = what.clone();
        Memo::new(move |_| plan.with(|doc| aim_in(&what, semester, doc).pressed))
    };
    let said = RwSignal::new(None::<bool>);
    let pressed = Memo::new(move |_| said.get().unwrap_or_else(|| planned.get()));
    let note = {
        let what = what.clone();
        Signal::derive(move || note_of(&what, semester, pressed.get(), t))
    };
    let take = move || {
        if said.get_untracked().is_some() {
            return;
        }
        let aim = plan.with_untracked(|doc| aim_in(&what, semester, doc));
        let was = aim.pressed;
        said.set(Some(!was));
        press(plan, source.clone(), what.id.clone(), aim, was, move || {
            said.try_set(None);
        });
    };
    view! { <HoldItem n label checked=pressed on="calendar-check-2" off="calendar-plus" note default take/> }
}

#[cfg(test)]
mod tests {
    use folia_model::labels::TurnusParity;

    use super::*;
    use crate::i18n::{DE, EN};

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    fn planning(turnus: Option<TurnusSeason>, hint: Option<PlanHint>) -> Planning {
        Planning {
            id: "12330".into(),
            title: "Datenbanken".into(),
            current: key("2026W"),
            newest: Some(key("2026W")),
            turnus,
            offer: Offer { season: turnus, parity: None },
            hint,
        }
    }

    #[test]
    fn the_menu_offers_the_current_semester_and_the_four_after_it() {
        let doc = PlanDoc::default();
        let what = planning(None, None);
        assert_eq!(semesters(&what, &doc), ["2026W", "2027S", "2027W", "2028S", "2028W"].map(key));
        // The semester the switch plans into stands among them where it lies beyond them.
        let far = planning(None, Some(PlanHint { semester: Some(key("2030S")), fill: None }));
        assert_eq!(semesters(&far, &doc), ["2026W", "2027S", "2027W", "2028S", "2028W", "2030S"].map(key));
        // What the plan holds elsewhere does not lengthen it: the switch's small line says that.
        let mut held = PlanDoc::default();
        assert!(held.plan(key("2025W"), "12330", 1, None));
        assert_eq!(semesters(&what, &held).len(), 5);
    }

    /// The owner, 2026-10-17: „wenn man das Semester noch nicht angelegt hat soll das trotzdem
    /// gehen". A semester the plan has nothing in takes the module, and is the plan's from then on.
    #[test]
    fn a_semester_nothing_is_planned_into_yet_takes_the_module() {
        let what = planning(None, None);
        let mut doc = PlanDoc::default();
        let later = key("2028S");
        let aim = aim_in(&what, later, &doc);
        assert_eq!((aim.target, aim.pressed, aim.fill.clone()), (later, false, None));
        super::super::toggle_in(&mut doc, &what.id, &aim, aim.pressed, &[], 5);
        assert!(doc.is_planned(later, "12330"));
        assert_eq!(doc.semesters(), [later]);
        // Taken again: out of it, and the semester with it.
        let aim = aim_in(&what, later, &doc);
        assert!(aim.pressed);
        super::super::toggle_in(&mut doc, &what.id, &aim, aim.pressed, &[], 6);
        assert!(doc.semesters().is_empty());
    }

    #[test]
    fn the_semester_the_switch_aims_at_does_what_the_switch_does() {
        let now = key("2026W");
        let mut doc = PlanDoc::default();
        doc.placeholders.push(folia_plans::studyplan::Placeholder {
            pid: 3,
            semester: now,
            program_id: "079-82-2008".into(),
            ord: 17,
            span: (3, 3),
            credits: Some("6".into()),
            kind: None,
            caption: String::new(),
            name: "Fachübergreifendes Studium".into(),
        });
        assert!(doc.plan(now, "12330", 1, None));
        let what = planning(None, Some(PlanHint { semester: None, fill: Some(3) }));
        // Planned there, but not for the placeholder: as on the switch, taking it counts it.
        let aim = aim_in(&what, now, &doc);
        assert_eq!((aim.pressed, aim.fill.as_ref().map(|(pid, _)| *pid)), (false, Some(3)));
        super::super::toggle_in(&mut doc, &what.id, &aim, aim.pressed, &[], 2);
        assert_eq!(doc.fillers(3).len(), 1);
        // Another semester: planned there plainly, for no placeholder.
        let next = aim_in(&what, key("2027S"), &doc);
        assert_eq!((next.pressed, next.fill.clone()), (false, None));
        super::super::toggle_in(&mut doc, &what.id, &next, next.pressed, &[], 3);
        assert!(doc.is_planned(key("2027S"), "12330") && doc.fillers(3).len() == 1);
    }

    #[test]
    fn a_semester_says_whether_the_module_is_planned_or_offered_there() {
        let winter = planning(Some(TurnusSeason::Winter), None);
        assert_eq!(note_of(&winter, key("2027S"), false, &DE), Some(("nicht angeboten", "warn")));
        assert_eq!(note_of(&winter, key("2027W"), false, &DE), None);
        assert_eq!(note_of(&winter, key("2027S"), true, &DE), Some(("geplant", "on")));
        assert_eq!(note_of(&winter, key("2027S"), false, &EN), Some(("not offered", "warn")));
        // Every semester, or nothing stated: nothing to say.
        assert_eq!(note_of(&planning(Some(TurnusSeason::Both), None), key("2027S"), false, &DE), None);
        assert_eq!(note_of(&planning(None, None), key("2027S"), false, &DE), None);
        // Odd years only: the winter of 2027 is one, that of 2028 is not.
        let odd = Planning { offer: Offer { season: Some(TurnusSeason::Winter), parity: Some(TurnusParity::Odd) }, ..winter };
        assert_eq!((note_of(&odd, key("2027W"), false, &DE), note_of(&odd, key("2028W"), false, &DE)), (None, Some(("nicht angeboten", "warn"))));
    }
}
