//! The cases where two devices change the same thing at the same time, one by one.

mod common;

use common::*;
use sync_merge_proto::doc::Op;
use sync_merge_proto::replica::Replica;
use sync_merge_proto::schema::Store;

/// Two devices of one profile, each with its own slot.
fn pair() -> (Replica, Replica) {
    (Replica::new(1, 1), Replica::new(2, 2))
}

/// Both see everything the other wrote: in different orders, once more for good measure.
fn meet(a: &mut Replica, a_ops: &[Op], b: &mut Replica, b_ops: &[Op]) {
    b.receive(a_ops);
    let mut reversed = b_ops.to_vec();
    reversed.reverse();
    a.receive(&reversed);
    a.receive(b_ops);
    b.receive(a_ops);
}

fn same(a: &Replica, b: &Replica) {
    assert_eq!(a.doc.entries, b.doc.entries);
    for store in Store::ALL {
        assert_eq!(a.text(store), b.text(store), "{store:?}");
    }
}

#[test]
fn a_module_moved_on_both_devices_is_lost_on_neither() {
    let (mut a, mut b) = pair();
    let first = a.change(Store::Plan, 1_000, |r| plan(r, "2026W", "11787", 100));
    b.receive(&first);
    let on_a = a.change(Store::Plan, 2_000, |r| move_to(r, "2026W", "2027S", "11787"));
    let on_b = b.change(Store::Plan, 2_500, |r| move_to(r, "2026W", "2027W", "11787"));
    meet(&mut a, &on_a, &mut b, &on_b);
    same(&a, &b);
    let text = a.text(Store::Plan);
    assert!(text.contains("m\t2027S\t11787\t100") && text.contains("m\t2027W\t11787\t100"), "{text}");
    assert!(!text.contains("2026W\t11787"), "{text}");
}

#[test]
fn planned_and_visited_never_both_the_later_one_counts() {
    let (mut a, mut b) = pair();
    let on_a = a.change(Store::Plan, 1_000, |r| visit(r, "2026W", "12101", 1));
    let on_b = b.change(Store::Plan, 1_200, |r| plan(r, "2026W", "12101", 1));
    meet(&mut a, &on_a, &mut b, &on_b);
    same(&a, &b);
    let text = a.text(Store::Plan);
    assert!(text.contains("m\t2026W\t12101") && !text.contains("v\t2026W\t12101"), "{text}");
}

#[test]
fn kinds_hidden_on_two_devices_are_both_hidden() {
    let (mut a, mut b) = pair();
    let on_a = a.change(Store::Plan, 1_000, |r| toggle(r, "P|k|2026W|vl".into(), true));
    let on_b = b.change(Store::Plan, 1_000, |r| toggle(r, "P|k|2026W|ue".into(), true));
    meet(&mut a, &on_a, &mut b, &on_b);
    same(&a, &b);
    assert!(a.text(Store::Plan).contains("k\t2026W\tue,vl"));
}

#[test]
fn calendars_made_at_once_on_two_devices_get_two_numbers_and_keep_their_modules() {
    let (mut a, mut b) = pair();
    let id_a = a.free_id("P|l|", 99, true).unwrap();
    let id_b = b.free_id("P|l|", 99, true).unwrap();
    assert_ne!(id_a, id_b);
    let mut on_a = a.change(Store::Plan, 1_000, |r| assert!(new_calendar(r, id_a, "Lena")));
    on_a.extend(a.change(Store::Plan, 1_100, |r| calendar_module(r, id_a, "2026W", "11109", true)));
    let mut on_b = b.change(Store::Plan, 1_050, |r| assert!(new_calendar(r, id_b, "Max")));
    on_b.extend(b.change(Store::Plan, 1_150, |r| calendar_module(r, id_b, "2026W", "13977", true)));
    meet(&mut a, &on_a, &mut b, &on_b);
    same(&a, &b);
    let text = a.text(Store::Plan);
    assert!(text.contains(&format!("l\t{id_a}\tstrip\t\tLena\nn\t{id_a}\t2026W\t11109")), "{text}");
    assert!(text.contains(&format!("l\t{id_b}\tstrip\t\tMax\nn\t{id_b}\t2026W\t13977")), "{text}");
}

#[test]
fn what_one_device_adds_to_a_calendar_the_other_deletes_does_not_come_back_with_its_number() {
    let (mut a, mut b) = pair();
    let id = a.free_id("P|l|", 99, true).unwrap();
    let made = a.change(Store::Plan, 1_000, |r| assert!(new_calendar(r, id, "Lena")));
    b.receive(&made);
    let gone = a.change(Store::Plan, 2_000, |r| delete_calendar(r, id));
    let added = b.change(Store::Plan, 2_100, |r| calendar_module(r, id, "2026W", "12160", true));
    meet(&mut a, &gone, &mut b, &added);
    same(&a, &b);
    assert!(!a.text(Store::Plan).contains("12160"));
    // The number is taken again, for a calendar of another life: Lena's module stays out of it.
    assert_eq!(a.free_id("P|l|", 99, true), Some(id));
    let again = a.change(Store::Plan, 3_000, |r| assert!(new_calendar(r, id, "Sport")));
    b.receive(&again);
    same(&a, &b);
    let text = a.text(Store::Plan);
    assert!(text.contains("Sport") && !text.contains("12160"), "{text}");
}

#[test]
fn a_note_typed_on_two_devices_keeps_the_later_and_shows_the_other() {
    let (mut a, mut b) = pair();
    let first = a.change(Store::Shelf, 1_000, |r| note(r, "11787", Some("Altklausur")));
    b.receive(&first);
    let on_a = a.change(Store::Shelf, 2_000, |r| note(r, "11787", Some("Altklausur 2024 rechnen")));
    let on_b = b.change(Store::Shelf, 2_300, |r| note(r, "11787", Some("Altklausur mit Lena")));
    meet(&mut a, &on_a, &mut b, &on_b);
    same(&a, &b);
    assert!(a.text(Store::Shelf).contains("n\t11787\tAltklausur mit Lena"));
    for device in [&a, &b] {
        let conflict = device.doc.conflicts.last().expect("a conflict");
        assert_eq!(conflict.other, "Altklausur 2024 rechnen");
    }
}

#[test]
fn a_note_typed_after_seeing_the_other_is_no_conflict() {
    let (mut a, mut b) = pair();
    let first = a.change(Store::Shelf, 1_000, |r| note(r, "11787", Some("Altklausur")));
    b.receive(&first);
    let second = b.change(Store::Shelf, 1_500, |r| note(r, "11787", Some("Altklausur, dann Folien")));
    a.receive(&second);
    let third = a.change(Store::Shelf, 1_400, |r| note(r, "11787", Some("Altklausur, dann Folien, dann Lena")));
    b.receive(&third);
    same(&a, &b);
    assert!(a.doc.conflicts.is_empty() && b.doc.conflicts.is_empty());
    assert!(a.text(Store::Shelf).contains("dann Lena"), "a slow clock does not lose a later edit");
}

#[test]
fn two_collections_of_one_name_both_stand() {
    let (mut a, mut b) = pair();
    let id_a = a.free_id("C|c|", u32::MAX, false).unwrap();
    let id_b = b.free_id("C|c|", u32::MAX, false).unwrap();
    let mut on_a = a.change(Store::Shelf, 1_000, |r| assert!(new_shelf(r, id_a, "Für Lena")));
    on_a.extend(a.change(Store::Shelf, 1_100, |r| member(r, "11109", id_a, true)));
    let mut on_b = b.change(Store::Shelf, 1_050, |r| assert!(new_shelf(r, id_b, "für lena")));
    on_b.extend(b.change(Store::Shelf, 1_150, |r| member(r, "13977", id_b, true)));
    meet(&mut a, &on_a, &mut b, &on_b);
    same(&a, &b);
    let text = a.text(Store::Shelf);
    assert!(text.contains(&format!("c\t{id_a}\tFür Lena\n")) && text.contains(&format!("c\t{id_b}\tfür lena (2)\n")), "{text}");
    assert!(text.contains(&format!("m\t11109\t{id_a}")) && text.contains(&format!("m\t13977\t{id_b}")), "{text}");
}

#[test]
fn an_offline_edit_older_than_an_online_one_loses_though_it_arrives_later() {
    let (mut a, mut b) = pair();
    let offline = b.change(Store::Mine, 1_000, |r| mine(r, "start", "2025W"));
    let online = a.change(Store::Mine, 2_000, |r| mine(r, "start", "2026W"));
    a.receive(&offline);
    b.receive(&online);
    same(&a, &b);
    assert_eq!(a.text(Store::Mine), "start\t2026W\n");
}

#[test]
fn a_record_of_a_newer_build_survives_an_older_one() {
    let (mut newer, mut older) = pair();
    // A newer build knows a tag `z` and gives it keys of its own; this build does not show it.
    let at = newer.clock.tick(1_000);
    let z = Op { dev: 1, seq: 1, key: "P|z|1".into(), val: Some("z\t1\tsomething new".into()), hlc: at, created: at, base: None };
    newer.receive(&[z.clone()]);
    older.receive(&[z]);
    assert!(!older.text(Store::Plan).contains("something new"));
    let ops = older.change(Store::Plan, 2_000, |r| plan(r, "2026W", "11109", 5));
    assert!(ops.iter().all(|op| op.key != "P|z|1"));
    newer.receive(&ops);
    assert_eq!(newer.doc.live("P|z|1").and_then(|e| e.val.as_deref()), Some("z\t1\tsomething new"));
}

#[test]
fn numbers_of_two_devices_never_meet() {
    let (a, b) = pair();
    let pids: Vec<u32> = [&a, &b].iter().map(|d| d.free_id("P|p|", 9_999, false).unwrap()).collect();
    let owns: Vec<u32> = [&a, &b].iter().map(|d| d.free_id("P|t|", 99_999, false).unwrap()).collect();
    assert_ne!(pids[0], pids[1]);
    assert_ne!(owns[0], owns[1]);
}
