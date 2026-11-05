//! Devices of one profile that change everything at once, go offline and come back, over a
//! network that delays, reorders and doubles: in the end every device holds the same profile.
//!
//! `cargo test --release --test simulation -- --nocapture` prints what the writes cost.

mod common;

use common::*;
use sync_merge_proto::codec::{batch_bytes, snapshot_bytes};
use sync_merge_proto::doc::Op;
use sync_merge_proto::replica::Replica;
use sync_merge_proto::schema::{self, Store, MAX_CALENDARS};

struct World {
    devices: Vec<Replica>,
    online: Vec<bool>,
    /// The service's log: every batch any device sent while online.
    service: Vec<Op>,
    /// Batches on their way to a device that was online when they left (directly or through the
    /// service), in no particular order.
    flight: Vec<(usize, Vec<Op>)>,
    /// What a device wrote while offline.
    outbox: Vec<Vec<Op>>,
    wall: u64,
    skew: Vec<i64>,
    rng: Rng,
    batches: Vec<usize>,
    writes: usize,
}

impl World {
    fn new(seed: u64, n: usize) -> Self {
        let mut rng = Rng::new(seed);
        let skew = (0..n).map(|_| rng.below(60_000) as i64 - 30_000).collect();
        World {
            devices: (0..n).map(|i| Replica::new(i as u16 + 1, i as u32 + 1)).collect(),
            online: vec![true; n],
            service: Vec::new(),
            flight: Vec::new(),
            outbox: vec![Vec::new(); n],
            wall: 1_767_225_600_000 + 26_000_000_000,
            skew,
            rng,
            batches: Vec::new(),
            writes: 0,
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
            let (to, ops) = if self.rng.below(10) == 0 { self.flight[i].clone() } else { self.flight.swap_remove(i) };
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
        let ops = act(&mut self.devices[d], wall, &mut self.rng);
        self.writes += ops.len();
        if !ops.is_empty() {
            self.batches.push(batch_bytes(&ops));
        }
        self.send(d, ops);
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

/// One thing a person does, on one device, with what is there on that device.
fn act(device: &mut Replica, wall: u64, rng: &mut Rng) -> Vec<Op> {
    let secs = wall / 1_000;
    let s = *rng.pick(&SEMESTERS);
    let m = *rng.pick(&MODULES);
    let on = rng.below(3) != 0;
    match rng.below(22) {
        0..=2 => device.change(Store::Plan, wall, |r| plan(r, s, m, secs)),
        3 => device.change(Store::Plan, wall, |r| visit(r, s, m, secs)),
        4 => device.change(Store::Plan, wall, |r| unplan(r, s, m)),
        5 | 6 => {
            let to = *rng.pick(&SEMESTERS);
            let view = device.view(Store::Plan);
            let Some((from, module)) = planned(&view).first().cloned() else { return Vec::new() };
            device.change(Store::Plan, wall, |r| move_to(r, &from, to, &module))
        }
        7 => {
            let kind = *rng.pick(&KINDS);
            device.change(Store::Plan, wall, |r| toggle(r, format!("P|k|{s}|{kind}"), on))
        }
        8 => {
            let (row, day) = (*rng.pick(&ROWS), *rng.pick(&DAYS));
            device.change(Store::Plan, wall, |r| toggle(r, format!("P|h|{s}|{row}|{day}"), on))
        }
        9 => {
            let row = *rng.pick(&ROWS);
            device.change(Store::Plan, wall, |r| toggle(r, format!("P|c|{s}|{row}"), on))
        }
        10 => device.change(Store::Plan, wall, |r| if on { pass(r, s, m) } else { r.remove(&format!("P|d|{m}")); }),
        11 => {
            let Some(pid) = device.free_id("P|p|", schema::MAX_PID, false) else { return Vec::new() };
            let ord = rng.below(300) as u32;
            let ops = device.change(Store::Plan, wall, |r| new_placeholder(r, pid, s, ord));
            let mut more = device.change(Store::Plan, wall, |r| fill(r, s, m, pid));
            more.splice(0..0, ops);
            more
        }
        12 => {
            let name = *rng.pick(&NAMES);
            let view = device.view(Store::Plan);
            let have = calendars(&view);
            if on && have.len() < MAX_CALENDARS {
                let Some(id) = device.free_id("P|l|", schema::MAX_CALENDAR_ID, true) else { return Vec::new() };
                device.change(Store::Plan, wall, |r| {
                    new_calendar(r, id, name);
                })
            } else if let Some(id) = have.first().copied() {
                if rng.below(2) == 0 {
                    device.change(Store::Plan, wall, |r| delete_calendar(r, id))
                } else {
                    device.change(Store::Plan, wall, |r| rename_calendar(r, id, name))
                }
            } else {
                Vec::new()
            }
        }
        13 => {
            let have = calendars(&device.view(Store::Plan));
            let Some(id) = have.get(rng.below(have.len().max(1))).copied() else { return Vec::new() };
            device.change(Store::Plan, wall, |r| calendar_module(r, id, s, m, on))
        }
        14 => {
            let view = device.view(Store::Plan);
            let have = owns(&view);
            if on || have.is_empty() {
                let Some(number) = device.free_id("P|t|", schema::MAX_OWN_NUMBER, false) else { return Vec::new() };
                let calendar = calendars(&view).first().copied().filter(|_| rng.below(2) == 0).unwrap_or(0);
                let name = *rng.pick(&WORDS);
                device.change(Store::Plan, wall, |r| own(r, number, calendar, s, name, "10:00-11:30"))
            } else {
                let number = have[rng.below(have.len())];
                device.change(Store::Plan, wall, |r| {
                    r.remove(&format!("P|t|{number}"));
                })
            }
        }
        15 | 16 => device.change(Store::Marks, wall, |r| if on { mark(r, m, secs) } else { unmark(r, m) }),
        17 => {
            let name = *rng.pick(&SHELVES);
            let view = device.view(Store::Shelf);
            let have = shelves(&view);
            if on || have.is_empty() {
                let Some(id) = device.free_id("C|c|", u32::MAX, false) else { return Vec::new() };
                device.change(Store::Shelf, wall, |r| {
                    new_shelf(r, id, name);
                })
            } else {
                let (id, _) = have[rng.below(have.len())];
                device.change(Store::Shelf, wall, |r| delete_shelf(r, id))
            }
        }
        18 => {
            let have = shelves(&device.view(Store::Shelf));
            let Some((id, _)) = have.get(rng.below(have.len().max(1))).cloned() else { return Vec::new() };
            device.change(Store::Shelf, wall, |r| member(r, m, id, on))
        }
        19 => {
            let words: Vec<&str> = (0..1 + rng.below(6)).map(|_| *rng.pick(&WORDS)).collect();
            let text = words.join(" ");
            device.change(Store::Shelf, wall, |r| note(r, m, if on { Some(&text) } else { None }))
        }
        20 => {
            let (i, j) = (rng.below(4), rng.below(4));
            device.change(Store::Shelf, wall, |r| swap_shelves(r, i, j))
        }
        _ => {
            let key = *rng.pick(&["program", "start", "fachsemester", "standort"]);
            let value = format!("{}", rng.below(5));
            device.change(Store::Mine, wall, |r| mine(r, key, &value))
        }
    }
}

/// What a store promises about its text, whatever came together in it.
fn check_text(world: &World) {
    let plan = world.devices[0].text(Store::Plan);
    let l: Vec<&str> = plan.lines().filter(|x| x.starts_with("l\t")).collect();
    assert!(l.len() <= MAX_CALENDARS);
    let ids: Vec<&str> = l.iter().filter_map(|x| x.split('\t').nth(1)).collect();
    for n in plan.lines().filter(|x| x.starts_with("n\t")) {
        assert!(ids.contains(&n.split('\t').nth(1).unwrap()), "a module of a calendar that is not there: {n}");
    }
    let shelf = world.devices[0].text(Store::Shelf);
    let names: Vec<String> = shelf.lines().filter(|x| x.starts_with("c\t")).map(|x| x.split('\t').nth(2).unwrap().trim().to_lowercase()).collect();
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), names.len(), "two collections of one name");
    // Read back, every store text gives the same records again.
    for store in Store::ALL {
        let text = world.devices[0].text(store);
        let again = schema::records(store, &text);
        assert_eq!(again, world.devices[0].view(store));
    }
}

fn run(seed: u64, devices: usize, steps: usize) -> World {
    let mut world = World::new(seed, devices);
    for _ in 0..steps {
        match world.rng.below(100) {
            0..=59 => world.act(),
            60..=91 => world.deliver(),
            _ => world.toggle(),
        }
    }
    world.settle();
    let first = &world.devices[0];
    for other in &world.devices[1..] {
        assert_eq!(other.doc.entries, first.doc.entries, "seed {seed}: the records differ");
        for store in Store::ALL {
            assert_eq!(other.text(store), first.text(store), "seed {seed}: {store:?} differs");
        }
    }
    check_text(&world);
    world
}

#[test]
fn three_devices_end_with_one_profile() {
    for seed in 0..200 {
        run(seed, 3, 1_500);
    }
}

#[test]
fn four_devices_offline_for_long_end_with_one_profile() {
    for seed in 1_000..1_040 {
        run(seed, 4, 4_000);
    }
}

fn percentile(sorted: &[usize], p: f64) -> usize {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

#[test]
fn what_the_writes_cost() {
    let mut all = Vec::new();
    let mut writes = 0;
    let mut actions = 0;
    let (mut stale, mut doubled, mut conflicts) = (0, 0, 0);
    for seed in 0..50 {
        let world = run(seed, 3, 1_500);
        writes += world.writes;
        actions += world.batches.len();
        stale += world.devices.iter().map(|d| d.doc.stale).sum::<usize>();
        doubled += world.devices.iter().map(|d| d.doc.doubled).sum::<usize>();
        conflicts += world.devices.iter().map(|d| d.doc.conflicts.len()).sum::<usize>();
        all.extend(world.batches);
    }
    all.sort_unstable();
    let mean = all.iter().sum::<usize>() as f64 / all.len() as f64;
    println!("50 runs of 3 devices: changes {actions}, writes {writes} ({:.2} per change)", writes as f64 / actions as f64);
    println!("arrived after a later write of their record: {stale}; arrived twice: {doubled}; notes typed at once on two devices: {conflicts}");
    println!("sealed batch of one change: mean {mean:.0} B, median {} B, p95 {} B, max {} B", percentile(&all, 0.5), percentile(&all, 0.95), all[all.len() - 1]);

    // A typical profile, made the way a person makes it: what a new device downloads first.
    let mut d = Replica::new(1, 1);
    let mut wall = 1_767_225_600_000 + 26_000_000_000;
    let mut tick = || {
        wall += 1_000;
        wall
    };
    for (i, s) in ["2025W", "2026S", "2026W", "2027S"].iter().enumerate() {
        for m in MODULES.iter().cycle().skip(i * 3).take(6) {
            let t = tick();
            d.change(Store::Plan, t, |r| plan(r, s, m, t / 1_000));
        }
    }
    for m in &MODULES[..10] {
        let t = tick();
        d.change(Store::Plan, t, |r| pass(r, "2025W", m));
    }
    for i in 0..6 {
        let pid = d.free_id("P|p|", schema::MAX_PID, false).unwrap();
        let t = tick();
        d.change(Store::Plan, t, |r| new_placeholder(r, pid, SEMESTERS[i % 3], 10 + i as u32));
    }
    for kind in ["vl", "ue"] {
        let t = tick();
        d.change(Store::Plan, t, |r| toggle(r, format!("P|k|2026W|{kind}"), true));
    }
    for row in ROWS {
        let t = tick();
        d.change(Store::Plan, t, |r| toggle(r, format!("P|c|2026W|{row}"), true));
    }
    let lena = d.free_id("P|l|", 99, true).unwrap();
    let t = tick();
    d.change(Store::Plan, t, |r| {
        new_calendar(r, lena, "Lena");
    });
    for m in &MODULES[..5] {
        let t = tick();
        d.change(Store::Plan, t, |r| calendar_module(r, lena, "2026W", m, true));
    }
    for i in 0..4 {
        let n = d.free_id("P|t|", schema::MAX_OWN_NUMBER, false).unwrap();
        let t = tick();
        d.change(Store::Plan, t, |r| own(r, n, 0, "2026W", WORDS[i], "10:00-11:30"));
    }
    for (i, m) in MODULES.iter().cycle().take(40).enumerate() {
        let t = tick();
        d.change(Store::Marks, t, |r| mark(r, &format!("{m}{i}"), t / 1_000));
    }
    for name in ["Für Lena", "Später", "Wichtig"] {
        let id = d.free_id("C|c|", u32::MAX, false).unwrap();
        let t = tick();
        d.change(Store::Shelf, t, |r| {
            new_shelf(r, id, name);
        });
    }
    for m in &MODULES[..6] {
        let t = tick();
        d.change(Store::Shelf, t, |r| note(r, m, Some("Altklausur rechnen, mit Lena lernen, Folien von Woche 3")));
    }
    let t = tick();
    d.change(Store::Mine, t, |r| {
        mine(r, "program", "079-82-2008");
        mine(r, "name", "Informatik B.Sc. · PO 2008");
        mine(r, "start", "2025W");
    });
    let texts: usize = Store::ALL.iter().map(|s| d.text(*s).len()).sum();
    println!("a typical profile: {} records, texts {texts} B, sealed snapshot {} B (records with their clocks)", d.doc.entries.len(), snapshot_bytes(&d.doc));

    // What a change costs the device: read the store's records, write the difference, merge.
    let started = std::time::Instant::now();
    let rounds = 2_000;
    for i in 0..rounds {
        let t = tick();
        let to = if i % 2 == 0 { "2027W" } else { "2027S" };
        let from = if i % 2 == 0 { "2027S" } else { "2027W" };
        d.change(Store::Plan, t, |r| move_to(r, from, to, MODULES[9]));
    }
    println!("a change in the Stundenplan of that profile, on this machine: {:.0} µs", started.elapsed().as_secs_f64() * 1e6 / f64::from(rounds));
}
