//! A hybrid logical clock: the device's wall time in milliseconds, a counter for what happens
//! within one millisecond or while the wall clock lags behind what was seen, and the device's
//! slot as the last word. Totally ordered: of two writes of one record the later one wins, and a
//! write made after seeing another is always later than it, whatever the two wall clocks say.

/// The time of one write.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hlc {
    pub ms: u64,
    pub n: u32,
    pub dev: u16,
}

/// One device's clock.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Clock {
    dev: u16,
    ms: u64,
    n: u32,
}

impl Clock {
    pub fn new(dev: u16) -> Self {
        Clock { dev, ms: 0, n: 0 }
    }

    /// The device whose clock this is.
    pub fn dev(self) -> u16 {
        self.dev
    }

    /// The time of a write made now, `wall` being the device's clock (in the browser corrected by
    /// what the service says its clock is): later than every write this device made or saw.
    pub fn tick(&mut self, wall: u64) -> Hlc {
        if wall > self.ms {
            self.ms = wall;
            self.n = 0;
        } else {
            self.n = self.n.saturating_add(1);
        }
        Hlc { ms: self.ms, n: self.n, dev: self.dev }
    }

    /// A write of another device was seen: the next tick comes after it.
    pub fn observe(&mut self, h: Hlc) {
        if (h.ms, h.n) > (self.ms, self.n) {
            self.ms = h.ms;
            self.n = h.n;
        }
    }

    /// The latest time this clock gave or saw.
    pub fn last(self) -> Hlc {
        Hlc { ms: self.ms, n: self.n, dev: self.dev }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_after_seeing_another_is_later_even_with_a_slow_clock() {
        let mut fast = Clock::new(1);
        let mut slow = Clock::new(2);
        let seen = fast.tick(10_000);
        slow.observe(seen);
        let after = slow.tick(4_000);
        assert!(after > seen);
        assert!(slow.tick(4_000) > after);
    }

    #[test]
    fn two_devices_at_the_same_moment_are_told_apart_by_their_slot() {
        let a = Clock::new(1).tick(5_000);
        let b = Clock::new(2).tick(5_000);
        assert!(a < b && a != b);
    }
}
