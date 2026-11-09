//! What the stores write comes back from the records as they wrote it: a plan with every tag of
//! `PlanDoc` (its shares and what its calendars follow too), saved plans, „Mein Studiengang", the
//! Merkliste and the Sammlungen.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

mod common;

use common::*;
use folia_calendar::change::{Change, Scope, Values};
use folia_calendar::day::Day;
use folia_calendar::kind::EventKind;
use folia_calendar::own::{OwnRhythm, OwnTermin};
use folia_calendar::select::{Town, TownChoice};
use folia_plans::studyplan::{DoneRow, Import, MineDoc, PlanDoc, Placeholder, SavedPlans};
use folia_sync::{codec, project, records, Doc, Store, Writer};

fn day(iso: &str) -> Day {
    Day::parse(iso).unwrap()
}

fn termin(name: &str, weekday: u8) -> OwnTermin {
    OwnTermin { number: 0, rhythm: OwnRhythm::Week, weekday, dates: Vec::new(), from: 13 * 60, to: 17 * 60, period: None, module: None, name: name.to_string(), place: "Café Mosaik".to_string(), skip: Vec::new() }
}

fn placeholder(s: &str, ord: i64, name: &str) -> Placeholder {
    Placeholder { pid: 0, semester: key(s), program_id: "079-82-2008".to_string(), ord, span: (1, 1), credits: Some("6".to_string()), kind: Some("elective".to_string()), caption: String::new(), name: name.to_string(), size: None }
}

fn change(event: u32, fp: u32, scope: Scope, room: &str) -> Change {
    Change {
        key: folia_calendar::rowkey::RowKey { event, fp },
        scope,
        weekday: None,
        time: None,
        room: Some(room.to_string()),
        was: Values { weekday: 5, from: 9 * 60 + 15, to: 10 * 60 + 45, room: "ZHG/HS.C".to_string() },
        module: "12104".to_string(),
    }
}

/// A plan with every tag the store writes, and a line of a newer build.
pub fn rich_plan() -> PlanDoc {
    let (w, s) = (key("2026W"), key("2027S"));
    let mut doc = PlanDoc::default();
    for (sem, id) in [(w, "12104"), (w, "12107"), (s, "11113"), (w, "12102"), (s, "12229"), (w, "11112")] {
        assert!(doc.plan(sem, id, 1_790_000_000, None));
    }
    assert!(doc.visit(s, "13977", 1_790_000_100));
    assert_eq!(doc.apply(&Import { placeholders: vec![placeholder("2026W", 17, "Fachübergreifendes Studium"), placeholder("2027S", 18, "Anwendungsfach")], ..Default::default() }, 0), (0, 2));
    doc.set_fills(w, "12102", Some(1));
    doc.set_kind(w, EventKind::Tutorial, true);
    doc.set_kind(w, EventKind::Exercise, true);
    doc.set_event(w, 149_408, true);
    doc.choose(w, 148_369, Some(row(0)));
    doc.set_row(s, row(1), true);
    doc.set_date(w, row(2), day("2026-11-04"), true);
    doc.set_date(w, row(2), day("2026-11-11"), true);
    assert!(doc.set_change(w, change(148_701, 0xb7025, Scope::All, "ZHG/HS.A")));
    assert!(doc.set_change(w, change(148_701, 0xb7025, Scope::From(day("2026-11-06")), "HG 1")));
    assert!(doc.set_note(w, row(0), "Folien vorher lesen"));
    doc.remember(s, "b3MOclrbw-CLbf8P0dlhmewCCVwqAX4Y41XPC~Uv0");
    assert!(doc.pass(w, "11109"));
    assert!(doc.pass(w, "11787"));
    assert!(doc.finish_row(DoneRow { semester: w, program_id: "079-82-2008".to_string(), ord: 21, caption: String::new(), name: "Praktikum".to_string() }));
    assert_eq!(doc.add_own(0, w, termin("Arbeit", 5)), Some(1));
    assert_eq!(doc.add_own(0, w, termin("Lerngruppe", 2)), Some(2));
    let lena = doc.add_calendar("Lena", Some("013-82-2018"), w, &["11109".to_string(), "13977".to_string()], &[row(1)], &[]).unwrap();
    let max = doc.add_calendar("Max", None, s, &["12160".to_string()], &[], &[]).unwrap();
    assert!(doc.add_to_calendar(lena, w, "12229"));
    assert_eq!(doc.add_own(lena, w, termin("Volleyball", 4)), Some(3));
    assert!(doc.set_change(w, change(148_701, 0xb7025, Scope::On(day("2026-11-13")), "C 7")));
    assert!(doc.add_to_calendar(max, s, "12101"));
    // What is shared, and Max's calendar following a share with its notes.
    use folia_plans::shares::{encode, Share, ShareTo};
    let shared = |n: u8, calendar: u8, to: ShareTo| Share {
        id: encode(&[n; 16]),
        calendar,
        to,
        semester: w,
        made: 1_794_000_000 + u64::from(n),
        live: true,
        once: to == ShareTo::Person,
        own: true,
        notes: n.is_multiple_of(2),
        key: encode(&[n + 1; 32]),
        parcel: if to == ShareTo::Person { encode(&[n + 2; 48]) } else { String::new() },
        program: Some("079-82-2008".into()),
        name: "Lena".into(),
    };
    assert!(doc.add_share(shared(1, 0, ShareTo::Person)));
    assert!(doc.add_share(shared(4, lena, ShareTo::App)));
    assert!(doc.add_share(Share { parcel: String::new(), name: String::new(), ..shared(7, 0, ShareTo::Person) }));
    assert!(doc.follow(max, &encode(&[9; 48]), 1_794_000_100));
    let part = folia_calendar::live::LivePart {
        semester: s.index(),
        modules: vec![12160, 12101],
        notes: vec![
            folia_calendar::live::CodeNote { key: row(1).packed(), text: "Altklausur".into() },
            folia_calendar::live::CodeNote { key: folia_calendar::rowkey::RowKey { event: 99_999, fp: 2 }.packed(), text: "vorher".into() },
        ],
        ..Default::default()
    };
    assert!(doc.take_part(max, &part));
    doc
}

fn with_newer_lines(text: &str) -> String {
    format!("{text}z\t2027S\twhat a newer build writes\nw\t1\t2026W\t150211-00001\tand more of it\n")
}

/// The records of a text, written by one device from nothing.
fn written(store: Store, text: &str) -> Device {
    let mut device = Device::new(1);
    let ops = device.edit(store, 1_000, |_| text.to_string());
    assert!(!ops.is_empty() || text.is_empty());
    device
}

#[test]
fn a_plan_comes_back_as_the_store_wrote_it() {
    let text = with_newer_lines(&rich_plan().stored_without_program());
    let held = PlanDoc::restored(&text).stored_without_program();
    assert_eq!(held, text, "the store keeps every line of it");
    let device = written(Store::Plan, &held);
    assert_eq!(project(Store::Plan, &device.doc), held);
    assert_eq!(device.text(Store::Plan), held);
}

#[test]
fn saved_plans_mine_and_the_rest_come_back_as_written() {
    let plan = rich_plan();
    let mut saved = SavedPlans::default();
    assert!(saved.save("Zweiter Versuch", TownChoice::Only(Town::Senftenberg), plan.semester_only(key("2026W"))));
    assert!(saved.save("Für Lena", TownChoice::Derive, plan.semester_only(key("2027S"))));
    let mut mine = MineDoc { program: Some("079-82-2008".to_string()), name: Some("Informatik B.Sc.".to_string()), start: Some(key("2025W")), town: TownChoice::Both, ..MineDoc::default() };
    mine.leave.insert(key("2026S"));
    mine.extra.push(("fachsemester".to_string(), "3".to_string()));
    let mut marks = marks::Marks::default();
    marks.mark("11787", 1_790_000_000);
    marks.mark("12101", 1_790_000_500);
    marks.mark("11109", 1_790_000_500);
    let mut shelf = shelf::Shelf::default();
    let lena = shelf.create("Für Lena").unwrap();
    let later = shelf.create("Später").unwrap();
    shelf.member("11787", lena, true);
    shelf.member("11787", later, true);
    shelf.member("12101", later, true);
    shelf.standard(lena, true);
    shelf.note("11787", Some("Altklausur\trechnen\nmit Lena \\ Max"));
    shelf.swap(0, 1);
    for (store, text) in [(Store::Saved, saved.stored()), (Store::Mine, mine.stored()), (Store::Marks, marks.stored()), (Store::Shelf, shelf.stored())] {
        assert_eq!(held(store, &text), text, "{store:?} keeps it");
        let device = written(store, &text);
        assert_eq!(project(store, &device.doc), text, "{store:?}");
    }
}

#[test]
fn records_travel_as_batches_and_snapshots_and_give_the_same_texts() {
    let text = with_newer_lines(&rich_plan().stored_without_program());
    let mut device = Device::new(3);
    let ops = device.edit(Store::Plan, 5_000, |_| text.clone());
    // Over the wire, in two batches, read back on another device in the other order.
    let mut other = Device::new(1);
    let parts = codec::batches(&ops, 400);
    assert!(parts.len() > 1);
    for part in parts.iter().rev() {
        other.receive(&codec::read_batch(part).unwrap());
    }
    same(&device, &other);
    // A new device starts from a snapshot.
    let (entries, seen) = codec::read_snapshot(&codec::snapshot(&device.doc)).unwrap();
    let mut fresh = Doc::default();
    let changed = fresh.join(entries, &seen);
    assert_eq!(changed.len(), device.doc.entries.len());
    assert_eq!(project(Store::Plan, &fresh), project(Store::Plan, &device.doc));
    assert_eq!(fresh.seen, device.doc.seen);
    // Its writer goes on after the writes of its slot and after every clock of the records.
    let writer = Writer::new(3, &fresh);
    assert_eq!(writer.seq(), ops.len() as u32);
    assert!(writer.last() >= ops.iter().map(|op| op.hlc).max().unwrap());
}

#[test]
fn a_change_writes_only_what_it_changed() {
    let mut device = written(Store::Plan, &rich_plan().stored_without_program());
    let ops = device.plan(2_000, |doc| {
        assert!(doc.plan(key("2027W"), "12160", 1_790_100_000, None));
    });
    assert_eq!(ops.iter().map(|op| op.key.as_str()).collect::<Vec<_>>(), ["P|mv|2027W|12160"]);
    let ops = device.plan(3_000, |doc| doc.set_kind(key("2026W"), EventKind::Lecture, true));
    assert_eq!(ops.len(), 1, "one kind more is one record: {ops:?}");
    assert!(ops[0].key.starts_with("P|k|2026W|"));
    let ops = device.plan(4_000, |doc| doc.unplan(key("2026W"), "12107", &[]));
    assert_eq!((ops.len(), ops[0].key.as_str(), ops[0].val.as_deref()), (1, "P|mv|2026W|12107", None));
    // Reading and writing the same text again writes nothing.
    assert!(device.plan(5_000, |_| {}).is_empty());
    assert!(device.edit(Store::Plan, 5_000, |text| text.replace('\n', "\r\n")).is_empty());
}

#[test]
fn what_the_store_cannot_read_is_never_deleted() {
    // A newer build wrote a line this build keeps as it came; another device made calendars until
    // there are more than the store shows.
    let mut newer = Device::new(1);
    let text = with_newer_lines(&rich_plan().stored_without_program());
    let mut all = newer.edit(Store::Plan, 1_000, |_| text);
    for (i, name) in ["A", "B", "C", "D", "E", "F", "G"].iter().enumerate() {
        all.extend(newer.edit(Store::Plan, 10_000 + i as u64, |text| format!("{text}l\t{}\t\t\t{name}\n", 40 + i)));
    }
    let mut older = Device::new(2);
    older.receive(&all);
    let calendars = older.text(Store::Plan).lines().filter(|l| l.starts_with("l\t")).count();
    assert_eq!(calendars, 8, "the store shows eight");
    let before = older.doc.entries.len();
    let ops = older.plan(20_000, |doc| {
        assert!(doc.plan(key("2027W"), "13980", 1, None));
    });
    assert_eq!(ops.len(), 1, "{ops:?}");
    assert_eq!(older.doc.entries.len(), before + 1);
    assert!(older.doc.live("P|?|z\t2027S\twhat a newer build writes").is_some());
    assert!(older.doc.live("P|l|46").is_some(), "the ninth calendar stays");
}

#[test]
fn every_store_text_read_back_gives_its_records_again() {
    let text = with_newer_lines(&rich_plan().stored_without_program());
    let r = records(Store::Plan, &text);
    assert!(r.len() > 30, "{}", r.len());
    assert_eq!(records(Store::Plan, &PlanDoc::restored(&text).stored_without_program()), r);
}
