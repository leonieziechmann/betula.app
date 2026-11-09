//! Two devices change the same thing at the same time, case by case, with the stores' own
//! methods: in the end both hold the same, and nothing either did is lost without a reason.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

mod common;

use common::*;
use folia_calendar::own::{OwnRhythm, OwnTermin};
use folia_plans::studyplan::PlanDoc;
use folia_sync::{Op, Store};

fn pair() -> (Device, Device) {
    (Device::new(1), Device::new(2))
}

fn termin(name: &str) -> OwnTermin {
    OwnTermin { number: 0, rhythm: OwnRhythm::Week, weekday: 3, dates: Vec::new(), from: 10 * 60, to: 11 * 60 + 30, period: None, module: None, name: name.to_string(), place: "Bibliothek".to_string(), skip: Vec::new() }
}

fn planned(device: &Device, s: &str) -> Vec<String> {
    device.plan_doc().modules_in(key(s))
}

#[test]
fn a_module_moved_on_both_devices_is_lost_on_neither() {
    let (mut a, mut b) = pair();
    let first = a.plan(1_000, |d| assert!(d.plan(key("2026W"), "11787", 100, None)));
    b.receive(&first);
    let on_a = a.plan(2_000, |d| d.move_to(key("2026W"), key("2027S"), "11787"));
    let on_b = b.plan(2_500, |d| d.move_to(key("2026W"), key("2027W"), "11787"));
    meet(&mut a, &on_a, &mut b, &on_b);
    same(&a, &b);
    assert_eq!((planned(&a, "2026W"), planned(&a, "2027S"), planned(&a, "2027W")), (vec![], vec!["11787".to_string()], vec!["11787".to_string()]));
    // Both devices know it was moved twice, from where.
    for device in [&a, &b] {
        assert_eq!(device.doc.conflicts.iter().map(|c| (c.key.as_str(), c.other.as_str())).collect::<Vec<_>>(), [("P|mv|2026W|11787", "")]);
    }
    assert_eq!(a.doc.conflicts, b.doc.conflicts);
}

#[test]
fn planned_and_visited_never_both_the_planned_one_counts() {
    let (mut a, mut b) = pair();
    let on_a = a.plan(1_000, |d| assert!(d.visit(key("2026W"), "12101", 1)));
    let on_b = b.plan(1_200, |d| assert!(d.plan(key("2026W"), "12101", 1, None)));
    meet(&mut a, &on_a, &mut b, &on_b);
    same(&a, &b);
    let doc = a.plan_doc();
    assert!(doc.is_planned(key("2026W"), "12101") && !doc.is_visited(key("2026W"), "12101"));
}

#[test]
fn kinds_hidden_on_two_devices_are_both_hidden() {
    use folia_calendar::kind::EventKind;
    let (mut a, mut b) = pair();
    let on_a = a.plan(1_000, |d| d.set_kind(key("2026W"), EventKind::Lecture, true));
    let on_b = b.plan(1_000, |d| d.set_kind(key("2026W"), EventKind::Tutorial, true));
    meet(&mut a, &on_a, &mut b, &on_b);
    same(&a, &b);
    let kinds = a.plan_doc().hidden.get(&key("2026W")).map(|h| h.kinds.codes()).unwrap_or_default();
    assert!(kinds.contains("lecture") && kinds.contains("tutorial"), "{kinds}");
}

#[test]
fn calendars_made_at_once_get_two_numbers_and_keep_their_modules() {
    let (mut a, mut b) = pair();
    let mut id_a = 0;
    let mut id_b = 0;
    let on_a = a.plan(1_000, |d| id_a = d.add_calendar("Lena", None, key("2026W"), &["11109".to_string()], &[], &[]).unwrap());
    let on_b = b.plan(1_050, |d| id_b = d.add_calendar("Max", None, key("2026W"), &["13977".to_string()], &[], &[]).unwrap());
    assert_ne!(id_a, id_b);
    meet(&mut a, &on_a, &mut b, &on_b);
    same(&a, &b);
    let doc = a.plan_doc();
    assert_eq!(doc.calendar(id_a).map(|c| (c.name.clone(), c.modules_in(key("2026W")))), Some(("Lena".to_string(), vec!["11109".to_string()])));
    assert_eq!(doc.calendar(id_b).map(|c| (c.name.clone(), c.modules_in(key("2026W")))), Some(("Max".to_string(), vec!["13977".to_string()])));
}

#[test]
fn what_one_device_adds_to_a_calendar_the_other_deletes_does_not_come_back_with_its_number() {
    let (mut a, mut b) = pair();
    let mut id = 0;
    let made = a.plan(1_000, |d| id = d.add_calendar("Lena", None, key("2026W"), &[], &[], &[]).unwrap());
    b.receive(&made);
    let gone = a.plan(2_000, |d| assert!(d.remove_calendar(id).is_some()));
    let added = b.plan(2_100, |d| assert!(d.add_to_calendar(id, key("2026W"), "12160")));
    meet(&mut a, &gone, &mut b, &added);
    same(&a, &b);
    assert!(a.plan_doc().calendars.is_empty());
    // Its number is not taken again (`folia_plans::ids`): Lena's module stays out of the next one.
    let mut again = 0;
    let ops = a.plan(3_000, |d| again = d.add_calendar("Sport", None, key("2026W"), &[], &[], &[]).unwrap());
    assert_ne!(again, id);
    b.receive(&ops);
    same(&a, &b);
    assert_eq!(a.plan_doc().calendar(again).map(|c| c.modules_in(key("2026W"))), Some(vec![]));
    assert!(a.plan_doc().calendar(id).is_none());
}

/// A device that did not see a calendar deleted writes for it later by its clock than another
/// device makes the next calendar: with a number of its own, the new one inherits nothing (a share
/// of the old one would otherwise show the new one, folia/design/teilen/teilen.html).
#[test]
fn a_calendar_made_after_one_was_deleted_never_inherits_what_is_written_for_the_old_one() {
    let (mut a, mut b) = pair();
    let mut old = 0;
    let made = a.plan(1_000, |d| old = d.add_calendar("Lena", None, key("2026W"), &["11109".to_string()], &[], &[]).unwrap());
    b.receive(&made);
    let mut on_a = a.plan(2_000, |d| assert!(d.remove_calendar(old).is_some()));
    let mut new = 0;
    on_a.extend(a.plan(3_000, |d| new = d.add_calendar("Sport", None, key("2026W"), &[], &[], &[]).unwrap()));
    // b's clock runs ahead: what it writes for the old calendar is later than the new one's birth.
    let late = b.plan(9_000, |d| {
        assert!(d.add_to_calendar(old, key("2026W"), "12160"));
        d.tick_in_calendar(old, key("2026W"), row(1), true);
    });
    meet(&mut a, &on_a, &mut b, &late);
    same(&a, &b);
    let doc = a.plan_doc();
    assert_ne!(new, old);
    assert_eq!(doc.calendars.iter().map(|c| (c.id, c.modules_in(key("2026W")), c.chosen.len())).collect::<Vec<_>>(), [(new, vec![], 0)]);
    // Every device knows the old number as given up, also the one that wrote late.
    assert!(folia_sync::retired(&b.doc).calendars.contains(&u32::from(old)));
}

#[test]
fn a_calendar_deleted_and_put_back_comes_back_with_what_it_had() {
    let (mut a, mut b) = pair();
    let mut ids = Vec::new();
    let made = a.plan(1_000, |d| {
        for name in ["Lena", "Max", "Sport"] {
            ids.push(d.add_calendar(name, None, key("2026W"), &["11109".to_string(), "12160".to_string()], &[row(0)], &[]).unwrap());
        }
    });
    b.receive(&made);
    let max = ids[1];
    let mut kept = None;
    let gone = a.plan(2_000, |d| kept = d.remove_calendar(max));
    // Meanwhile the other device adds a module to it: added to what was deleted, it stays out.
    let added = b.plan(2_050, |d| assert!(d.add_to_calendar(max, key("2026W"), "13977")));
    let (at, calendar) = kept.unwrap();
    let back = a.plan(2_100, |d| assert!(d.put_calendar(at, calendar)));
    let mut on_a = gone;
    on_a.extend(back);
    meet(&mut a, &on_a, &mut b, &added);
    same(&a, &b);
    let doc = a.plan_doc();
    // A calendar put back begins a new life, at the end of the calendars, with what it had.
    assert_eq!(doc.calendars.iter().map(|c| c.id).collect::<Vec<_>>(), [ids[0], ids[2], max]);
    assert_eq!(doc.calendar(max).map(|c| (c.modules_in(key("2026W")), c.chosen.len())), Some((vec!["11109".to_string(), "12160".to_string()], 1)));
}

#[test]
fn a_module_taken_out_and_put_back_stands_where_it_stood_and_one_planned_anew_at_the_end() {
    let (mut a, mut b) = pair();
    let w = key("2026W");
    let made = a.plan(1_000, |d| {
        for id in ["12104", "12107", "12102", "11112"] {
            assert!(d.plan(w, id, 1, None));
        }
    });
    b.receive(&made);
    // „Rückgängig": the plan as it was before.
    let before: PlanDoc = a.plan_doc();
    let out = a.plan(2_000, |d| d.unplan(w, "12107", &[]));
    b.receive(&out);
    let undo = a.plan(2_100, |d| *d = before.clone());
    b.receive(&undo);
    same(&a, &b);
    assert_eq!(planned(&b, "2026W"), ["12104", "12107", "12102", "11112"]);
    // Taken out and planned again later: at the end, as the store adds it.
    let out = a.plan(3_000, |d| d.unplan(w, "12104", &[]));
    let anew = a.plan(3_100, |d| assert!(d.plan(w, "12104", 5, None)));
    b.receive(&out);
    b.receive(&anew);
    same(&a, &b);
    assert_eq!(planned(&b, "2026W"), ["12107", "12102", "11112", "12104"]);
}

#[test]
fn a_termin_moved_into_another_calendar_stays_there() {
    let (mut a, mut b) = pair();
    let w = key("2026W");
    let mut number = 0;
    let first = a.plan(1_000, |d| number = d.add_own(0, w, termin("Arbeit")).unwrap());
    b.receive(&first);
    let mut lena = 0;
    let made = b.plan(2_000, |d| lena = d.add_calendar("Lena", None, w, &[], &[], &[]).unwrap());
    a.receive(&made);
    let moved = a.plan(3_000, |d| assert!(d.set_own(number, lena, termin("Arbeit"))));
    b.receive(&moved);
    same(&a, &b);
    assert_eq!(b.plan_doc().own_termin(number).map(|(calendar, _)| calendar), Some(lena));
}

#[test]
fn a_note_typed_on_two_devices_keeps_the_later_and_shows_the_other() {
    let (mut a, mut b) = pair();
    let first = a.shelf(1_000, |s| s.note("11787", Some("Altklausur")));
    b.receive(&first);
    let on_a = a.shelf(2_000, |s| s.note("11787", Some("Altklausur 2024 rechnen")));
    let on_b = b.shelf(2_300, |s| s.note("11787", Some("Altklausur mit Lena")));
    meet(&mut a, &on_a, &mut b, &on_b);
    same(&a, &b);
    assert!(a.text(Store::Shelf).contains("n\t11787\tAltklausur mit Lena"));
    for device in [&a, &b] {
        let conflict = device.doc.conflicts.last().expect("a conflict");
        assert_eq!((conflict.key.as_str(), conflict.other.as_str()), ("C|n|11787", "n\t11787\tAltklausur 2024 rechnen"));
    }
}

#[test]
fn a_note_of_a_termin_typed_on_two_devices_keeps_both() {
    let (mut a, mut b) = pair();
    let w = key("2026W");
    let on_a = a.plan(1_000, |d| assert!(d.set_note(w, row(0), "Folien lesen")));
    let on_b = b.plan(1_100, |d| assert!(d.set_note(w, row(0), "Raum wechselt")));
    meet(&mut a, &on_a, &mut b, &on_b);
    same(&a, &b);
    assert_eq!(a.plan_doc().note_of(w, row(0)), Some("Raum wechselt"));
    assert_eq!(a.doc.conflicts.len(), 1);
    assert!(a.doc.conflicts[0].other.ends_with("Folien lesen"));
}

#[test]
fn a_note_typed_after_seeing_the_other_is_no_conflict_even_with_a_slow_clock() {
    let (mut a, mut b) = pair();
    let first = a.shelf(1_000, |s| s.note("11787", Some("Altklausur")));
    b.receive(&first);
    let second = b.shelf(1_500, |s| s.note("11787", Some("Altklausur, dann Folien")));
    a.receive(&second);
    let third = a.shelf(1_400, |s| s.note("11787", Some("Altklausur, dann Folien, dann Lena")));
    b.receive(&third);
    same(&a, &b);
    assert!(a.doc.conflicts.is_empty() && b.doc.conflicts.is_empty());
    assert!(a.text(Store::Shelf).contains("dann Lena"));
}

#[test]
fn two_collections_of_one_name_both_stand() {
    let (mut a, mut b) = pair();
    let mut id_a = 0;
    let mut id_b = 0;
    let on_a = a.shelf(1_000, |s| {
        id_a = s.create("Für Lena").unwrap();
        s.member("11109", id_a, true);
    });
    let on_b = b.shelf(1_050, |s| {
        id_b = s.create("für  lena").unwrap();
        s.member("13977", id_b, true);
    });
    assert_ne!(id_a, id_b);
    meet(&mut a, &on_a, &mut b, &on_b);
    same(&a, &b);
    let text = a.text(Store::Shelf);
    assert!(text.contains(&format!("c\t{id_a}\tFür Lena\n")) && text.contains(&format!("c\t{id_b}\tfür lena (2)\n")), "{text}");
    assert!(text.contains(&format!("m\t11109\t{id_a}")) && text.contains(&format!("m\t13977\t{id_b}")), "{text}");
}

#[test]
fn collections_that_change_places_on_one_device_and_grow_on_another_keep_both() {
    let (mut a, mut b) = pair();
    let made = a.shelf(1_000, |s| {
        for name in ["Eins", "Zwei", "Drei"] {
            s.create(name).unwrap();
        }
    });
    b.receive(&made);
    let swapped = a.shelf(2_000, |s| s.swap(0, 2));
    let grown = b.shelf(2_000, |s| {
        s.create("Vier").unwrap();
    });
    meet(&mut a, &swapped, &mut b, &grown);
    same(&a, &b);
    let names: Vec<String> = shelf::Shelf::restored(&a.text(Store::Shelf)).collections.into_iter().map(|(_, n)| n).collect();
    assert_eq!(names, ["Drei", "Zwei", "Eins", "Vier"]);
}

#[test]
fn an_offline_edit_older_than_an_online_one_loses_though_it_arrives_later() {
    let (mut a, mut b) = pair();
    let offline = b.mine(1_000, |m| m.start = Some(key("2025W")));
    let online = a.mine(2_000, |m| m.start = Some(key("2026W")));
    a.receive(&offline);
    b.receive(&online);
    same(&a, &b);
    assert_eq!(a.text(Store::Mine), "start\t2026W\n");
}

#[test]
fn a_mark_taken_away_on_one_device_and_marked_on_the_other_follows_the_later() {
    let (mut a, mut b) = pair();
    let first = a.marks(1_000, |m| m.mark("12101", 10));
    b.receive(&first);
    let away = a.marks(2_000, |m| {
        m.unmark("12101");
    });
    let again = b.marks(2_100, |m| m.mark("11787", 11));
    meet(&mut a, &away, &mut b, &again);
    same(&a, &b);
    assert_eq!(a.text(Store::Marks), "11787\t11\n");
}

#[test]
fn a_record_of_a_newer_build_survives_an_older_one() {
    let (mut newer, mut older) = pair();
    let ops = newer.edit(Store::Plan, 1_000, |text| format!("{text}z\t1\tsomething new\n"));
    older.receive(&ops);
    let more = older.plan(2_000, |d| assert!(d.plan(key("2026W"), "11109", 5, None)));
    assert!(more.iter().all(|op| !op.key.starts_with("P|?|")), "{more:?}");
    newer.receive(&more);
    same(&newer, &older);
    assert!(older.text(Store::Plan).contains("z\t1\tsomething new"));
}

#[test]
fn numbers_of_two_devices_never_meet() {
    let (mut a, mut b) = pair();
    let mut got: Vec<(u32, u32, u8, u32)> = Vec::new();
    for device in [&mut a, &mut b] {
        let mut numbers = (0, 0, 0, 0);
        device.plan(1_000, |d| {
            numbers.0 = d.next_pid();
            numbers.1 = d.add_own(0, key("2026W"), termin("x")).unwrap();
            numbers.2 = d.add_calendar("y", None, key("2026W"), &[], &[], &[]).unwrap();
        });
        device.shelf(1_000, |s| numbers.3 = s.create("z").unwrap());
        got.push(numbers);
    }
    let (x, y) = (got[0], got[1]);
    assert!(x.0 != y.0 && x.1 != y.1 && x.2 != y.2 && x.3 != y.3, "{got:?}");
}

#[test]
fn a_device_that_comes_back_after_long_gets_what_it_missed_in_any_order() {
    let (mut a, mut b) = pair();
    let mut all: Vec<Op> = Vec::new();
    for (i, id) in MODULES.iter().enumerate() {
        all.extend(a.plan(1_000 + i as u64, |d| {
            d.plan(key(SEMESTERS[i % 3]), id, i as u64, None);
        }));
    }
    let mut shuffled = all.clone();
    let mut rng = Rng::new(7);
    for i in (1..shuffled.len()).rev() {
        shuffled.swap(i, rng.below(i + 1));
    }
    b.receive(&shuffled);
    same(&a, &b);
    assert_eq!(b.receive(&all), 0, "what came twice counts once");
}

#[test]
fn a_row_imported_on_two_devices_at_once_has_one_placeholder_and_its_modules_count_for_it() {
    use folia_plans::studyplan::{Import, Placeholder};
    let (mut a, mut b) = pair();
    let w = key("2026W");
    let row = || Placeholder { pid: 0, semester: w, program_id: "079-82-2008".to_string(), ord: 17, span: (1, 1), credits: Some("6".to_string()), kind: None, caption: String::new(), name: "Wahlpflicht".to_string() };
    let import = |d: &mut PlanDoc, module: &str| {
        d.apply(&Import { placeholders: vec![row()], ..Default::default() }, 1);
        let pid = d.placeholders.last().map(|p| p.pid);
        assert!(d.plan(w, module, 1, pid));
    };
    let on_a = a.plan(1_000, |d| import(d, "12160"));
    let on_b = b.plan(1_000, |d| import(d, "13977"));
    meet(&mut a, &on_a, &mut b, &on_b);
    same(&a, &b);
    let doc = a.plan_doc();
    assert_eq!(doc.placeholders.len(), 1);
    let pid = doc.placeholders[0].pid;
    assert_eq!(doc.fillers(pid).len(), 2, "both modules count for the one that stands");
}

#[test]
fn a_note_typed_key_by_key_on_two_devices_at_once_is_one_hint_with_the_other_text() {
    let (mut a, mut b) = pair();
    let first = a.shelf(1_000, |s| s.note("11787", Some("Altklausur")));
    b.receive(&first);
    // Each key is a write of the store, as the field saves it.
    let mut on_a = Vec::new();
    let mut on_b = Vec::new();
    for (i, text) in ["Altklausur 2", "Altklausur 20", "Altklausur 2024"].iter().enumerate() {
        on_b.extend(b.shelf(2_000 + i as u64 * 10, |s| s.note("11787", Some(&text.replace("2024", "mit Lena").replace("20", "mi").replace(" 2", " m")))));
    }
    for (i, text) in ["Altklausur 2", "Altklausur 20", "Altklausur 2024"].iter().enumerate() {
        on_a.extend(a.shelf(3_000 + i as u64 * 10, |s| s.note("11787", Some(text))));
    }
    meet(&mut a, &on_a, &mut b, &on_b);
    same(&a, &b);
    assert!(a.text(Store::Shelf).contains("n\t11787\tAltklausur 2024"));
    for device in [&a, &b] {
        let hints: Vec<_> = device.doc.conflicts.iter().filter(|c| c.key == "C|n|11787").collect();
        assert_eq!(hints.len(), 1, "one hint: {:?}", device.doc.conflicts);
        assert!(hints[0].other.ends_with("Altklausur mit Lena"), "the other device's last text: {:?}", hints[0]);
    }
    // Typed on after seeing it, nothing new is at once.
    let before = a.doc.conflicts.clone();
    let more = b.shelf(4_000, |s| s.note("11787", Some("Altklausur 2024, dann Lena")));
    a.receive(&more);
    assert_eq!(a.doc.conflicts, before);
}

/// A share's line (`folia_stores::collections::ListShare`) and a followed list's (`Followed`).
fn share_line(id: char, collection: u32) -> String {
    format!("s\t{}\t{collection}\t1790000000\tln\t{}\t-\tLena", id.to_string().repeat(22), "k".repeat(43))
}

fn follow_line(id: u32, link: char) -> String {
    format!("f\t{id}\t{}\t1790000000\t0\t-\tVon Max\tCODE", link.to_string().repeat(64))
}

#[test]
fn shares_and_followed_lists_travel_and_go_with_their_collection() {
    let (mut a, mut b) = pair();
    let mut lena = 0;
    let made = a.shelf(1_000, |s| {
        lena = s.create("Für Lena").unwrap();
        s.member("11109", lena, true);
    });
    b.receive(&made);
    // a shares the collection and the whole list; b follows a list of somebody else.
    let shared = a.edit(Store::Shelf, 2_000, |text| format!("{text}{}\n{}\n", share_line('A', lena), share_line('B', 0)));
    let followed = b.edit(Store::Shelf, 2_000, |text| format!("{text}{}\n", follow_line(500, 'L')));
    meet(&mut a, &shared, &mut b, &followed);
    same(&a, &b);
    let text = a.text(Store::Shelf);
    assert!(text.contains(&share_line('A', lena)) && text.contains(&share_line('B', 0)) && text.contains(&follow_line(500, 'L')), "{text}");
    assert_eq!(held(Store::Shelf, &text), text, "the store keeps every line of it");
    // b deletes the collection: its share goes with it on both, the whole list's stays.
    let gone = b.shelf(3_000, |s| s.delete(lena));
    a.receive(&gone);
    same(&a, &b);
    let text = a.text(Store::Shelf);
    assert!(!text.contains(&share_line('A', lena)) && text.contains(&share_line('B', 0)), "{text}");
}

#[test]
fn one_link_followed_on_two_devices_at_once_is_one_list() {
    let (mut a, mut b) = pair();
    let on_a = a.edit(Store::Shelf, 1_000, |text| format!("{text}{}\n", follow_line(7, 'L')));
    let on_b = b.edit(Store::Shelf, 1_050, |text| format!("{text}{}\n", follow_line(8, 'L')));
    meet(&mut a, &on_a, &mut b, &on_b);
    same(&a, &b);
    let text = a.text(Store::Shelf);
    assert!(text.contains(&follow_line(7, 'L')) && !text.contains(&follow_line(8, 'L')), "the first one followed stands: {text}");
    // A list unfollowed retires its number, as a collection deleted does.
    let unfollowed = a.edit(Store::Shelf, 2_000, |text| text.replace(&format!("{}\n", follow_line(7, 'L')), ""));
    b.receive(&unfollowed);
    assert!(folia_sync::schema::retired(&a.doc).collections.contains(&7));
}
