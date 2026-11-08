//! New numbers while a profile syncs (Betula Sync, `folia_sync`): every device of a profile takes
//! them from a class of its own, the numbers that leave its slot when divided by 16, so that two
//! devices making a calendar, a Termin of one's own, a placeholder or a collection at the same
//! time never take one number. A device that does not sync takes them as before, and so does one
//! whose class is full.
//!
//! Nor does a synced profile give a number out again that it had in the last 13 months (the
//! records keep a deletion that long, `folia_sync::schema::retired`): a share hangs on its
//! calendar's number (folia/design/teilen/teilen.html, „Zuerst: jeder Kalender behält seine
//! Nummer"), and a calendar made anew under an old number would inherit what a device that did
//! not see the deletion yet still writes for the old one. Only where every free number was had
//! already does a new one take one of those, rather than none.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;

/// A profile has at most 16 devices.
pub const SLOTS: u32 = 16;

/// What a number numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// A calendar of one's own (`l`).
    Calendar,
    /// A Termin of one's own (`t`).
    Own,
    /// A placeholder (`p`).
    Placeholder,
    /// A collection of the Sammlungen.
    Collection,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Calendar, Kind::Own, Kind::Placeholder, Kind::Collection];

    fn index(self) -> usize {
        match self {
            Kind::Calendar => 0,
            Kind::Own => 1,
            Kind::Placeholder => 2,
            Kind::Collection => 3,
        }
    }
}

thread_local! {
    static SLOT: Cell<u16> = const { Cell::new(0) };
    static RETIRED: RefCell<[BTreeSet<u32>; 4]> = const { RefCell::new([BTreeSet::new(), BTreeSet::new(), BTreeSet::new(), BTreeSet::new()]) };
}

/// This device's slot in the profile it shows (1–16); 0 while that profile does not sync.
pub fn set_slot(slot: u16) {
    SLOT.with(|s| s.set(slot));
}

pub fn slot() -> u16 {
    SLOT.with(Cell::get)
}

/// The numbers of a kind the profile it shows had and gave up; none while it does not sync.
pub fn set_retired(kind: Kind, numbers: BTreeSet<u32>) {
    RETIRED.with(|r| {
        if let Some(set) = r.borrow_mut().get_mut(kind.index()) {
            *set = numbers;
        }
    });
}

/// Forgets every number given up: the profile shown does not sync (any more).
pub fn clear_retired() {
    for kind in Kind::ALL {
        set_retired(kind, BTreeSet::new());
    }
}

/// Whether the profile shown had the number `n` of `kind` and gave it up.
pub fn retired(kind: Kind, n: u32) -> bool {
    RETIRED.with(|r| r.borrow().get(kind.index()).is_some_and(|set| set.contains(&n)))
}

/// Whether `n` is of this device's class; every number is while it does not sync.
fn ours(n: u32) -> bool {
    match u32::from(slot()) {
        0 => true,
        slot => n % SLOTS == slot % SLOTS,
    }
}

/// The smallest number of `kind` in `1..=max` that is free and was never given up, of this
/// device's class while there is one; else the smallest one free.
pub fn smallest(kind: Kind, max: u32, free: impl Fn(u32) -> bool) -> Option<u32> {
    let fresh = |n: u32| free(n) && !retired(kind, n);
    (1..=max).find(|n| ours(*n) && fresh(*n)).or_else(|| (1..=max).find(|n| fresh(*n))).or_else(|| (1..=max).find(|n| free(*n)))
}

/// A number of `kind` after the largest one taken or given up (`largest`), of this device's
/// class; when there is none up to `max`, the smallest one free (`smallest`).
pub fn after(kind: Kind, largest: u32, max: u32, free: impl Fn(u32) -> bool) -> Option<u32> {
    let largest = RETIRED.with(|r| r.borrow().get(kind.index()).and_then(|set| set.range(..=max).next_back().copied())).map_or(largest, |gone| gone.max(largest));
    let next = largest.checked_add(1).filter(|n| *n <= max).and_then(|from| (from..=max).take(SLOTS as usize).find(|n| ours(*n)));
    next.filter(|n| free(*n) && !retired(kind, *n)).or_else(|| smallest(kind, max, free))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_sync_numbers_are_taken_as_before() {
        set_slot(0);
        clear_retired();
        assert_eq!(after(Kind::Own, 7, 99, |_| true), Some(8));
        assert_eq!(after(Kind::Own, 99, 99, |n| n != 1), Some(2));
        assert_eq!(smallest(Kind::Calendar, 99, |n| n > 3), Some(4));
        assert_eq!(after(Kind::Own, 5, 5, |_| false), None);
    }

    #[test]
    fn a_number_given_up_does_not_come_again() {
        set_slot(0);
        set_retired(Kind::Calendar, [1, 2, 4].into());
        set_retired(Kind::Own, [9].into());
        // Calendar 3 stands; 1, 2 and 4 were deleted: the next one is 5, not 1.
        assert_eq!(smallest(Kind::Calendar, 99, |n| n != 3), Some(5));
        // Another kind keeps its own numbers.
        assert_eq!(smallest(Kind::Placeholder, 99, |n| n != 3), Some(1));
        // After the largest taken or given up: Termin 9 was deleted, 8 is the largest standing.
        assert_eq!(after(Kind::Own, 8, 99, |n| n > 8), Some(10));
        // Only where every free number was had does one of them come again.
        assert_eq!(smallest(Kind::Calendar, 4, |n| n != 3), Some(1));
        // A synced device of slot 2 takes its class, never a number given up.
        set_slot(2);
        set_retired(Kind::Calendar, [2, 18].into());
        assert_eq!(smallest(Kind::Calendar, 99, |_| true), Some(34));
        set_slot(0);
        clear_retired();
        assert_eq!(smallest(Kind::Calendar, 99, |n| n != 3), Some(1));
    }

    #[test]
    fn two_devices_never_take_one_number() {
        let mut taken = Vec::new();
        for slot in 1..=16 {
            set_slot(slot);
            let a = after(Kind::Own, 17, 9_999, |_| true).unwrap();
            let b = smallest(Kind::Calendar, 99, |_| true).unwrap();
            assert!(a > 17 && u32::from(slot) % SLOTS == a % SLOTS && b % SLOTS == a % SLOTS);
            taken.push((a, b));
        }
        let firsts: std::collections::BTreeSet<u32> = taken.iter().map(|t| t.0).collect();
        assert_eq!(firsts.len(), 16);
        // A class that is full takes what is free.
        set_slot(3);
        assert_eq!(smallest(Kind::Calendar, 99, |n| n % 16 != 3 && n > 40), Some(41));
        set_slot(0);
    }
}
