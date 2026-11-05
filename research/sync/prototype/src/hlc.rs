//! A hybrid logical clock: the device's wall time in milliseconds, a counter for what happens
//! within one millisecond or while the wall clock lags behind what was seen, and the device as
//! the last word. Totally ordered: of two changes of one record, the later one wins, and a
//! change made after seeing another is always later than it, whatever the two wall clocks say.

/// The time of one change.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hlc {
    pub ms: u64,
    pub n: u32,
    pub dev: u16,
}

/// One device's clock.
#[derive(Clone, Debug)]
pub struct Clock {
    dev: u16,
    ms: u64,
    n: u32,
}

impl Clock {
    pub fn new(dev: u16) -> Self {
        Clock { dev, ms: 0, n: 0 }
    }

    /// The time of a change made now, `wall` being the device's own clock: later than every
    /// change this device made or saw.
    pub fn tick(&mut self, wall: u64) -> Hlc {
        if wall > self.ms {
            self.ms = wall;
            self.n = 0;
        } else {
            self.n += 1;
        }
        Hlc { ms: self.ms, n: self.n, dev: self.dev }
    }

    /// A change of another device was seen: the next tick comes after it.
    pub fn observe(&mut self, h: Hlc) {
        if (h.ms, h.n) > (self.ms, self.n) {
            self.ms = h.ms;
            self.n = h.n;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_after_seeing_another_is_later_even_with_a_slow_clock() {
        let mut fast = Clock::new(1);
        let mut slow = Clock::new(2);
        let seen = fast.tick(10_000);
        slow.observe(seen);
        let after = slow.tick(4_000);
        assert!(after > seen);
    }
}
