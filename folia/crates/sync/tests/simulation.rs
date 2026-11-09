//! Devices of one profile change everything at once with the stores' own methods, go offline and
//! come back, over a network that delays, reorders and doubles, and a new device joins from a
//! snapshot: in the end every device holds the same profile, and every store reads all of it.
//!
//! `cargo test --release -p folia-sync --test simulation -- --nocapture` prints what it cost.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

mod common;

use common::*;
use folia_calendar::day::Day;
use folia_calendar::kind::EventKind;
use folia_calendar::own::{OwnRhythm, OwnTermin};
use folia_calendar::select::{Town, TownChoice};
use folia_plans::studyplan::{Import, PlanDoc, Placeholder, MAX_CALENDARS};
use folia_sync::{codec, project, records, Doc, Op, Store, Writer};

struct World {
    devices: Vec<Device>,
    online: Vec<bool>,
    /// The service's log: every write any device sent while online.
    service: Vec<Op>,
    /// Batches on their way to a device that was online when they left, in no order.
    flight: Vec<(usize, Vec<Op>)>,
    /// What a device wrote while offline.
    outbox: Vec<Vec<Op>>,
    /// A plan a device can go back to („Rückgängig").
    undo: Vec<Option<PlanDoc>>,
    wall: u64,
    skew: Vec<i64>,
    rng: Rng,
    writes: usize,
    changes: usize,
    batch_bytes: Vec<usize>,
}

impl World {
    fn new(seed: u64, n: usize) -> Self {
        let mut rng = Rng::new(seed);
        let skew = (0..n).map(|_| rng.below(60_000) as i64 - 30_000).collect();
        World {
            devices: (0..n).map(|i| Device::new(i as u16 + 1)).collect(),
            online: vec![true; n],
            service: Vec::new(),
            flight: Vec::new(),
            outbox: vec![Vec::new(); n],
            undo: vec![None; n],
            wall: 1_790_000_000_000,
            skew,
            rng,
            writes: 0,
            changes: 0,
            batch_bytes: Vec::new(),
        }
    }

    fn send(&mut self, from: usize, ops: Vec<Op>) {
        if ops.is_empty() {
            return;
        }
        if self.online[from] {
            self.service.extend(ops.iter().cloned());
            for to in 0..self.devices.len() {
                if to != from && self.online[to] {
                    self.flight.push((to, ops.clone()));
                }
            }
        } else {
            self.outbox[from].extend(ops);
        }
    }

    fn deliver(&mut self) {
        for _ in 0..1 + self.rng.below(4) {
            if self.flight.is_empty() {
                return;
            }
            let i = self.rng.below(self.flight.len());
            let (to, ops) = if self.rng.chance(10) { self.flight[i].clone() } else { self.flight.swap_remove(i) };
            if self.online[to] {
                self.devices[to].receive(&ops);
            }
        }
    }

    fn catch_up(&mut self, d: usize) {
        let missing: Vec<Op> = self.service.iter().filter(|op| !self.devices[d].doc.has(op)).cloned().collect();
        self.devices[d].receive(&missing);
    }

    fn toggle(&mut self) {
        let d = self.rng.below(self.devices.len());
        if self.online[d] {
            self.online[d] = false;
        } else {
            self.online[d] = true;
            let out = std::mem::take(&mut self.outbox[d]);
            self.send(d, out);
            self.catch_up(d);
        }
    }

    fn act(&mut self) {
        let d = self.rng.below(self.devices.len());
        self.wall += 1 + self.rng.below(3_000) as u64;
        let wall = (self.wall as i64 + self.skew[d]) as u64;
        let mut undo = self.undo[d].take();
        let ops = act(&mut self.devices[d], wall, &mut self.rng, &mut undo);
        self.undo[d] = undo;
        self.writes += ops.len();
        if !ops.is_empty() {
            self.changes += 1;
            self.batch_bytes.push(codec::batch(&ops).len() + 28);
        }
        self.send(d, ops);
    }

    /// A device joins from what the service holds: a snapshot of one device and the writes after.
    fn join_from_snapshot(&mut self) {
        let from = self.rng.below(self.devices.len());
        let (entries, seen) = codec::read_snapshot(&codec::snapshot(&self.devices[from].doc)).unwrap();
        let mut doc = Doc::default();
        doc.join(entries, &seen);
        let slot = self.devices.len() as u16 + 1;
        let writer = Writer::new(slot, &doc);
        self.devices.push(Device { doc, writer });
        self.online.push(true);
        self.outbox.push(Vec::new());
        self.undo.push(None);
        self.skew.push(0);
        self.catch_up(self.devices.len() - 1);
    }

    fn settle(&mut self) {
        for d in 0..self.devices.len() {
            if !self.online[d] {
                self.online[d] = true;
                let out = std::mem::take(&mut self.outbox[d]);
                self.send(d, out);
            }
        }
        for (to, ops) in std::mem::take(&mut self.flight) {
            self.devices[to].receive(&ops);
        }
        for d in 0..self.devices.len() {
            self.catch_up(d);
        }
    }
}

fn own(name: &str, weekday: u8) -> OwnTermin {
    OwnTermin { number: 0, rhythm: OwnRhythm::Week, weekday, dates: Vec::new(), from: 10 * 60, to: 11 * 60 + 30, period: None, module: None, name: name.to_string(), place: String::new(), skip: Vec::new() }
}

/// One thing a person does on one device, with what that device shows.
fn act(device: &mut Device, wall: u64, rng: &mut Rng, undo: &mut Option<PlanDoc>) -> Vec<Op> {
    let secs = wall / 1_000;
    let s = key(rng.pick(&SEMESTERS));
    let m = *rng.pick(&MODULES);
    let on = !rng.chance(3);
    let r = row(rng.below(3));
    let kept = device.plan_doc();
    let kind = *rng.pick(&[EventKind::Lecture, EventKind::Exercise, EventKind::Tutorial, EventKind::Seminar]);
    let pick = rng.below(100);
    let ops = match pick {
        0..=9 => device.plan(wall, |d| {
            d.plan(s, m, secs, None);
        }),
        10..=12 => device.plan(wall, |d| {
            d.visit(s, m, secs);
        }),
        13..=16 => device.plan(wall, |d| d.unplan(s, m, &[])),
        17..=21 => {
            let to = key(rng.pick(&SEMESTERS));
            let Some((from, module)) = kept.modules.first().map(|p| (p.semester, p.module_id.clone())) else { return Vec::new() };
            device.plan(wall, |d| d.move_to(from, to, &module))
        }
        22..=24 => device.plan(wall, |d| d.set_kind(s, kind, on)),
        25..=27 => {
            let day = Day::parse(rng.pick(&DAYS)).unwrap();
            device.plan(wall, |d| d.set_date(s, r, day, on))
        }
        28..=29 => device.plan(wall, |d| d.choose(s, r.event, on.then_some(r))),
        30..=32 => device.plan(wall, |d| if on { d.pass(s, m); } else { d.unpass(m); }),
        33..=35 => {
            let ord = rng.below(300) as i64;
            device.plan(wall, |d| {
                let p = Placeholder { pid: 0, semester: s, program_id: "079-82-2008".to_string(), ord, span: (1, 1), credits: Some("6".to_string()), kind: None, caption: String::new(), name: "Wahlpflicht".to_string(), size: None };
                d.apply(&Import { placeholders: vec![p], ..Default::default() }, secs);
                let pid = d.placeholders.last().map(|p| p.pid);
                d.set_fills(s, m, pid);
            })
        }
        36..=40 => {
            let name = *rng.pick(&NAMES);
            if on && kept.calendars.len() < MAX_CALENDARS {
                device.plan(wall, |d| {
                    d.add_calendar(name, None, s, &[m.to_string()], &[], &[]);
                })
            } else if let Some(id) = kept.calendars.first().map(|c| c.id) {
                if rng.chance(2) {
                    device.plan(wall, |d| {
                        d.remove_calendar(id);
                    })
                } else {
                    device.plan(wall, |d| {
                        d.rename_calendar(id, name);
                    })
                }
            } else {
                Vec::new()
            }
        }
        41..=45 => {
            let Some(id) = kept.calendars.get(rng.below(kept.calendars.len())).map(|c| c.id) else { return Vec::new() };
            device.plan(wall, |d| {
                if on {
                    d.add_to_calendar(id, s, m);
                } else {
                    d.remove_from_calendar(id, s, m, &[]);
                }
            })
        }
        46..=47 => {
            let Some(id) = kept.calendars.get(rng.below(kept.calendars.len())).map(|c| c.id) else { return Vec::new() };
            device.plan(wall, |d| d.tick_in_calendar(id, s, r, on))
        }
        48..=52 => {
            let calendars: Vec<u8> = std::iter::once(0).chain(kept.calendars.iter().map(|c| c.id)).collect();
            let calendar = *rng.pick(&calendars);
            let name = *rng.pick(&WORDS);
            let weekday = 1 + rng.below(5) as u8;
            let numbers: Vec<u32> = kept.own.iter().map(|o| o.termin.number).chain(kept.calendars.iter().flat_map(|c| c.own.iter().map(|o| o.termin.number))).collect();
            match (on, numbers.get(rng.below(numbers.len())).copied()) {
                (true, _) | (false, None) => device.plan(wall, |d| {
                    d.add_own(calendar, s, own(name, weekday));
                }),
                (false, Some(number)) if rng.chance(2) => device.plan(wall, |d| {
                    d.remove_own(number);
                }),
                (false, Some(number)) => device.plan(wall, |d| {
                    d.set_own(number, calendar, own(name, weekday));
                }),
            }
        }
        53..=54 => {
            let words: Vec<&str> = (0..1 + rng.below(5)).map(|_| *rng.pick(&WORDS)).collect();
            let text = if on { words.join(" ") } else { String::new() };
            device.plan(wall, |d| {
                d.set_note(s, r, &text);
            })
        }
        55 => {
            // What is shared and followed: a share made or taken back, a calendar that follows,
            // takes a part with notes, is not shared any more, or keeps what it has.
            use folia_plans::shares::{encode, Share, ShareTo, ID_BYTES, KEY_BYTES, LINK_BYTES};
            let calendars: Vec<u8> = std::iter::once(0).chain(kept.calendars.iter().map(|c| c.id)).collect();
            let calendar = *rng.pick(&calendars);
            let n = rng.below(256) as u8;
            let own_calendar = kept.calendars.get(rng.below(kept.calendars.len())).map(|c| c.id);
            let words: Vec<&str> = (0..1 + rng.below(4)).map(|_| *rng.pick(&WORDS)).collect();
            match rng.below(5) {
                0 | 1 if on => device.plan(wall, |d| {
                    let to = if calendar == 0 { ShareTo::Person } else { ShareTo::App };
                    d.add_share(Share {
                        id: encode(&[n; ID_BYTES]),
                        calendar,
                        to,
                        semester: s,
                        made: secs,
                        live: true,
                        once: to == ShareTo::Person,
                        own: n.is_multiple_of(2),
                        notes: n.is_multiple_of(3),
                        key: encode(&[n ^ 0x5a; KEY_BYTES]),
                        parcel: if to == ShareTo::Person { encode(&[n; LINK_BYTES]) } else { String::new() },
                        program: None,
                        name: words.join(" "),
                    });
                }),
                0 | 1 => {
                    let Some(id) = kept.shares.get(rng.below(kept.shares.len())).map(|share| share.id.clone()) else { return Vec::new() };
                    if rng.chance(2) {
                        device.plan(wall, |d| {
                            d.remove_share(&id);
                        })
                    } else {
                        device.plan(wall, |d| {
                            d.update_share(&id, |share| share.parcel.clear());
                        })
                    }
                }
                2 => {
                    let Some(id) = own_calendar else { return Vec::new() };
                    device.plan(wall, |d| {
                        if on {
                            d.follow(id, &encode(&[n; LINK_BYTES]), secs);
                        } else {
                            d.unfollow(id);
                        }
                    })
                }
                3 => {
                    let Some(id) = own_calendar else { return Vec::new() };
                    device.plan(wall, |d| {
                        d.end_follow(id, secs);
                    })
                }
                _ => {
                    let Some(id) = own_calendar else { return Vec::new() };
                    let part = folia_calendar::live::LivePart {
                        semester: s.index(),
                        modules: vec![m.parse().unwrap()],
                        own: vec![folia_calendar::live::CodeOwn { number: 1 + u32::from(n), rhythm: 0, weekday: 1 + n % 5, from: 600, to: 660, name: words.join(" "), ..Default::default() }],
                        notes: vec![folia_calendar::live::CodeNote { key: r.packed(), text: words.join(" ") }],
                        ..Default::default()
                    };
                    device.plan(wall, |d| {
                        d.take_part(id, &part);
                    })
                }
            }
        }
        56..=58 => {
            // „Rückgängig": the plan as it was before the last change of this device.
            let Some(back) = undo.take() else { return Vec::new() };
            return device.plan(wall, |d| *d = back);
        }
        59..=66 => device.marks(wall, |k| {
            if on {
                k.mark(m, secs);
            } else {
                k.unmark(m);
            }
        }),
        67..=71 => {
            let name = *rng.pick(&SHELVES);
            let shelf = shelf::Shelf::restored(&device.text(Store::Shelf));
            if on || shelf.collections.is_empty() {
                device.shelf(wall, |k| {
                    k.create(name);
                })
            } else {
                let (id, _) = shelf.collections[rng.below(shelf.collections.len())].clone();
                if rng.chance(2) {
                    device.shelf(wall, |k| k.delete(id))
                } else {
                    device.shelf(wall, |k| k.rename(id, name))
                }
            }
        }
        72..=77 => {
            let shelf = shelf::Shelf::restored(&device.text(Store::Shelf));
            let Some((id, _)) = shelf.collections.get(rng.below(shelf.collections.len())).cloned() else { return Vec::new() };
            if rng.chance(4) {
                device.shelf(wall, |k| k.standard(id, on))
            } else {
                device.shelf(wall, |k| k.member(m, id, on))
            }
        }
        78..=83 => {
            let words: Vec<&str> = (0..1 + rng.below(6)).map(|_| *rng.pick(&WORDS)).collect();
            let text = words.join(" ");
            device.shelf(wall, |k| k.note(m, on.then_some(text.as_str())))
        }
        84..=86 => {
            let (i, j) = (rng.below(4), rng.below(4));
            device.shelf(wall, |k| k.swap(i, j))
        }
        87..=91 => {
            let start = key(rng.pick(&["2025W", "2026S", "2026W"]));
            let town = *rng.pick(&[TownChoice::Derive, TownChoice::Both, TownChoice::Only(Town::Cottbus)]);
            device.mine(wall, |k| match rng.below(3) {
                0 => k.start = Some(start),
                1 => k.town = town,
                _ => {
                    if !k.leave.insert(start) {
                        k.leave.remove(&start);
                    }
                }
            })
        }
        _ => {
            let name = *rng.pick(&NAMES);
            let plan = kept.semester_only(s);
            device.saved(wall, |k| {
                if on {
                    k.save(name, TownChoice::Derive, plan);
                } else {
                    k.remove(name);
                }
            })
        }
    };
    if (0..56).contains(&pick) && !ops.is_empty() {
        *undo = Some(kept);
    }
    ops
}

/// What every store promises, whatever came together in it: it reads all the records give it,
/// and they give it what it holds.
fn check_stores(world: &World, seed: u64) {
    let device = &world.devices[0];
    for store in Store::ALL {
        let text = project(store, &device.doc);
        let held = held(store, &text);
        let (kept, given) = (records(store, &held), records(store, &text));
        let lost: Vec<(&str, &str)> = given.iter().filter(|(k, v)| kept.get(k) != Some(*v)).collect();
        let added: Vec<(&str, &str)> = kept.iter().filter(|(k, v)| given.get(k) != Some(*v)).collect();
        assert!(lost.is_empty() && added.is_empty(), "seed {seed}: {store:?} reads other records than it is given: lost {lost:?}, added {added:?}");
    }
    let plan = device.plan_doc();
    assert!(plan.calendars.len() <= MAX_CALENDARS);
}

fn run(seed: u64, devices: usize, steps: usize) -> World {
    let mut world = World::new(seed, devices);
    for step in 0..steps {
        match world.rng.below(100) {
            0..=59 => world.act(),
            60..=91 => world.deliver(),
            _ => world.toggle(),
        }
        if step == steps / 2 && seed.is_multiple_of(3) {
            world.join_from_snapshot();
        }
    }
    world.settle();
    let first = &world.devices[0];
    for (i, other) in world.devices.iter().enumerate().skip(1) {
        assert_eq!(other.doc.entries, first.doc.entries, "seed {seed}: the records of device {i} differ");
        for store in Store::ALL {
            assert_eq!(other.text(store), first.text(store), "seed {seed}: {store:?} of device {i} differs");
        }
    }
    check_stores(&world, seed);
    world
}

/// How many runs: `FOLIA_SYNC_SEEDS` for a longer search; fewer unoptimised.
fn seeds(usual: u64) -> u64 {
    let usual = if cfg!(debug_assertions) { usual / 5 } else { usual };
    std::env::var("FOLIA_SYNC_SEEDS").ok().and_then(|n| n.parse().ok()).unwrap_or(usual)
}

#[test]
fn three_devices_end_with_one_profile() {
    for seed in 0..seeds(60) {
        run(seed, 3, 1_200);
    }
}

#[test]
fn four_devices_offline_for_long_end_with_one_profile() {
    for seed in 1_000..1_000 + seeds(60) / 5 {
        run(seed, 4, 3_000);
    }
}

#[test]
fn what_the_writes_cost() {
    let (mut writes, mut changes, mut conflicts) = (0, 0, 0);
    let mut sizes = Vec::new();
    let mut snapshot = 0;
    for seed in 0..seeds(20) {
        let world = run(seed, 3, 1_200);
        writes += world.writes;
        changes += world.changes;
        conflicts += world.devices.iter().map(|d| d.doc.conflicts.len()).sum::<usize>();
        sizes.extend(world.batch_bytes.iter().copied());
        snapshot = snapshot.max(codec::snapshot(&world.devices[0].doc).len());
    }
    sizes.sort_unstable();
    let at = |p: f64| sizes[((sizes.len() - 1) as f64 * p) as usize];
    println!("20 runs of 3 devices: {changes} changes, {writes} writes ({:.2} per change), {conflicts} conflict copies kept", writes as f64 / changes as f64);
    println!("sealed batch of one change: median {} B, p95 {} B, max {} B; largest snapshot {snapshot} B", at(0.5), at(0.95), at(1.0));
}
