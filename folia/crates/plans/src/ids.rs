//! New numbers while a profile syncs (Betula Sync, `folia_sync`): every device of a profile takes
//! them from a class of its own, the numbers that leave its slot when divided by 16, so that two
//! devices making a calendar, a Termin of one's own, a placeholder or a collection at the same
//! time never take one number. A device that does not sync takes them as before, and so does one
//! whose class is full.

use std::cell::Cell;

/// A profile has at most 16 devices.
pub const SLOTS: u32 = 16;

thread_local! {
    static SLOT: Cell<u16> = const { Cell::new(0) };
}

/// This device's slot in the profile it shows (1–16); 0 while that profile does not sync.
pub fn set_slot(slot: u16) {
    SLOT.with(|s| s.set(slot));
}

pub fn slot() -> u16 {
    SLOT.with(Cell::get)
}

/// Whether `n` is of this device's class; every number is while it does not sync.
fn ours(n: u32) -> bool {
    match u32::from(slot()) {
        0 => true,
        slot => n % SLOTS == slot % SLOTS,
    }
}

/// The smallest number in `1..=max` that is free, of this device's class while there is one.
pub fn smallest(max: u32, free: impl Fn(u32) -> bool) -> Option<u32> {
    (1..=max).find(|n| ours(*n) && free(*n)).or_else(|| (1..=max).find(|n| free(*n)))
}

/// A number after the largest one taken (`largest`), of this device's class; when there is none
/// up to `max`, the smallest one free (`smallest`).
pub fn after(largest: u32, max: u32, free: impl Fn(u32) -> bool) -> Option<u32> {
    let next = largest.checked_add(1).filter(|n| *n <= max).and_then(|from| (from..=max).take(SLOTS as usize).find(|n| ours(*n)));
    next.filter(|n| free(*n)).or_else(|| smallest(max, free))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_sync_numbers_are_taken_as_before() {
        set_slot(0);
        assert_eq!(after(7, 99, |_| true), Some(8));
        assert_eq!(after(99, 99, |n| n != 1), Some(2));
        assert_eq!(smallest(99, |n| n > 3), Some(4));
        assert_eq!(after(5, 5, |_| false), None);
    }

    #[test]
    fn two_devices_never_take_one_number() {
        let mut taken = Vec::new();
        for slot in 1..=16 {
            set_slot(slot);
            let a = after(17, 9_999, |_| true).unwrap();
            let b = smallest(99, |_| true).unwrap();
            assert!(a > 17 && u32::from(slot) % SLOTS == a % SLOTS && b % SLOTS == a % SLOTS);
            taken.push((a, b));
        }
        let firsts: std::collections::BTreeSet<u32> = taken.iter().map(|t| t.0).collect();
        assert_eq!(firsts.len(), 16);
        // A class that is full takes what is free.
        set_slot(3);
        assert_eq!(smallest(99, |n| n % 16 != 3 && n > 40), Some(41));
        set_slot(0);
    }
}
