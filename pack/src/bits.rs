//! The bits of a code, and the codes for numbers they are made of.
//!
//! The bits of a code are one number: the first bit written is its lowest. A field of several bits
//! is written the same way round, its lowest bit first. After the last bit that is set, a reader
//! finds zeros, which is why every code for numbers here writes zero as `0`: whatever stands at the
//! end of a value and is zero costs nothing.

use crate::Error;

/// How many zero bits a reader takes after the last bit of a code, as the values that were zero at
/// its end. It bounds what a code that claims a long list of nothing can make a reader do. A writer
/// refuses a value that ends in more zeros than that (it would not be read back).
pub(crate) const ZERO_TAIL: usize = 1 << 16;

/// Bits as they are written.
#[derive(Clone, Debug, Default)]
pub(crate) struct Bits {
    words: Vec<u32>,
    len: usize,
}

impl Bits {
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// The bits as a number, the lowest word first.
    pub(crate) fn words(&self) -> &[u32] {
        &self.words
    }

    pub(crate) fn push(&mut self, bit: bool) {
        if self.len.is_multiple_of(32) {
            self.words.push(0);
        }
        if bit {
            if let Some(word) = self.words.last_mut() {
                *word |= 1 << (self.len % 32);
            }
        }
        self.len += 1;
    }

    /// The lowest `count` bits of `value` (at most 64), the lowest first.
    pub(crate) fn push_bits(&mut self, value: u64, count: u32) {
        for at in 0..count.min(64) {
            self.push((value >> at) & 1 == 1);
        }
    }

    pub(crate) fn append(&mut self, other: &Bits) {
        for at in 0..other.len {
            self.push(bit(&other.words, at));
        }
    }

    /// `n` in the Elias gamma code of `n + 1`, its width in ones: as many ones as `n + 1` has
    /// bits below its highest, a zero, then those bits. `0` is `0`, `1` is `100`, `2` is `101`,
    /// `3` is `11000`: 2 w + 1 bits for a number of w + 1 bits.
    pub(crate) fn gamma(&mut self, n: u64) {
        let (width, low) = split(n);
        for _ in 0..width {
            self.push(true);
        }
        self.push(false);
        self.push_bits(low, width);
    }

    /// `n` in the Elias delta code of `n + 1`: the width of `n + 1` below its highest bit in the
    /// gamma code, then those bits. `0` is `0`, a number of five digits takes 20 to 25 bits, one of
    /// seven 28 to 32: shorter than gamma from 31 on.
    pub(crate) fn delta(&mut self, n: u64) {
        let (width, low) = split(n);
        self.gamma(u64::from(width));
        self.push_bits(low, width);
    }

    /// `n` in the Exp-Golomb code of order `k` (at most 63): `n >> k` in the gamma code, then the
    /// lowest `k` bits of `n`. Numbers around 2^k take k + 1 to k + 3 bits, and one far off costs
    /// twice its width, not its size.
    pub(crate) fn exp_golomb(&mut self, n: u64, k: u32) {
        let k = k.min(63);
        self.gamma(n >> k);
        self.push_bits(n, k);
    }
}

/// `n + 1` as the width of its bits below the highest, and those bits.
fn split(n: u64) -> (u32, u64) {
    let x = u128::from(n) + 1;
    let width = 127 - x.leading_zeros();
    // The bits below the highest: at most 64 of them, all of them in the lower word.
    (width, (x ^ (1 << width)) as u64)
}

/// The bit at `at` of a number.
fn bit(words: &[u32], at: usize) -> bool {
    words.get(at / 32).is_some_and(|word| (word >> (at % 32)) & 1 == 1)
}

/// A signed number as an unsigned one that is small when it is: 0, -1, 1, -2, 2 … are 0, 1, 2, 3, 4 …
pub(crate) fn zigzag(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}

pub(crate) fn unzigzag(n: u64) -> i64 {
    ((n >> 1) as i64) ^ -((n & 1) as i64)
}

/// How many bits [`Bits::gamma`] writes for `n`.
pub(crate) fn gamma_len(n: u64) -> u64 {
    2 * u64::from(split(n).0) + 1
}

/// How many bits [`Bits::delta`] writes for `n`.
pub(crate) fn delta_len(n: u64) -> u64 {
    let width = split(n).0;
    gamma_len(u64::from(width)) + u64::from(width)
}

/// How many bits [`Bits::exp_golomb`] writes for `n`.
pub(crate) fn exp_golomb_len(n: u64, k: u32) -> u64 {
    let k = k.min(63);
    gamma_len(n >> k) + u64::from(k)
}

/// Where the bits of a number end: one past its highest bit that is set.
pub(crate) fn significant(words: &[u32]) -> usize {
    words.iter().enumerate().rev().find(|(_, word)| **word != 0).map_or(0, |(at, word)| at * 32 + 32 - word.leading_zeros() as usize)
}

/// Bits as they are read: those of a number, then zeros.
pub(crate) struct Reader<'a> {
    words: &'a [u32],
    /// Where the number ends: every bit from here on is zero.
    end: usize,
    at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(words: &'a [u32], end: usize) -> Self {
        Reader { words, end, at: 0 }
    }

    pub(crate) fn bit(&mut self) -> Result<bool, Error> {
        if self.left() == 0 {
            return Err(Error::Malformed);
        }
        let set = self.at < self.end && bit(self.words, self.at);
        self.at += 1;
        Ok(set)
    }

    /// `count` bits (at most 64), the lowest first.
    pub(crate) fn bits(&mut self, count: u32) -> Result<u64, Error> {
        let mut value = 0;
        for at in 0..count.min(64) {
            if self.bit()? {
                value |= 1 << at;
            }
        }
        Ok(value)
    }

    pub(crate) fn gamma(&mut self) -> Result<u64, Error> {
        let mut width = 0;
        while self.bit()? {
            width += 1;
            if width > 64 {
                return Err(Error::Malformed);
            }
        }
        let low = self.bits(width)?;
        joined(width, low)
    }

    pub(crate) fn delta(&mut self) -> Result<u64, Error> {
        let width = u32::try_from(self.gamma()?).ok().filter(|width| *width <= 64).ok_or(Error::Malformed)?;
        let low = self.bits(width)?;
        joined(width, low)
    }

    pub(crate) fn exp_golomb(&mut self, k: u32) -> Result<u64, Error> {
        let k = k.min(63);
        let high = self.gamma()?;
        let low = self.bits(k)?;
        u64::try_from((u128::from(high) << k) | u128::from(low)).map_err(|_| Error::Malformed)
    }

    /// The length of a list, a string or a set. No longer than what is left to read: every item
    /// takes a bit at least (all but `()`, of which a list is read up to 65536 long).
    pub(crate) fn length(&mut self) -> Result<usize, Error> {
        let length = self.delta()?;
        usize::try_from(length).ok().filter(|length| *length <= self.left()).ok_or(Error::Malformed)
    }

    /// How many bits can still be read, the zeros after the end included.
    pub(crate) fn left(&self) -> usize {
        self.end.saturating_add(ZERO_TAIL).saturating_sub(self.at)
    }

    /// Whether the value took every bit of the code: one that ends later carries more than the
    /// type it is read as knows of.
    pub(crate) fn finish(&self) -> Result<(), Error> {
        if self.at < self.end {
            return Err(Error::Trailing);
        }
        Ok(())
    }
}

/// `n` from the bits of `n + 1` below its highest.
fn joined(width: u32, low: u64) -> Result<u64, Error> {
    let x = (1u128 << width) | u128::from(low);
    u64::try_from(x - 1).map_err(|_| Error::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(write: impl FnOnce(&mut Bits)) -> String {
        let mut bits = Bits::default();
        write(&mut bits);
        let mut reader = Reader::new(&bits.words, bits.len);
        (0..bits.len).map(|_| if reader.bit().unwrap() { '1' } else { '0' }).collect()
    }

    #[test]
    fn the_codes_for_numbers() {
        assert_eq!(written(|bits| bits.gamma(0)), "0");
        assert_eq!(written(|bits| bits.gamma(1)), "100");
        assert_eq!(written(|bits| bits.gamma(2)), "101");
        assert_eq!(written(|bits| bits.gamma(3)), "11000");
        // 6 + 1 = 0b111: the two bits below the highest, the lowest first.
        assert_eq!(written(|bits| bits.gamma(6)), "11011");
        assert_eq!(written(|bits| bits.delta(0)), "0");
        assert_eq!(written(|bits| bits.delta(1)), "1000");
        assert_eq!(written(|bits| bits.delta(3)), "10100");
        // 11101 + 1 has 14 bits: the gamma code of 13, then 13 bits.
        assert_eq!(written(|bits| bits.delta(11101)).len(), 7 + 13);
        assert_eq!(written(|bits| bits.exp_golomb(0, 3)), "0000");
        assert_eq!(written(|bits| bits.exp_golomb(13, 3)), "100101");
        assert_eq!(written(|bits| bits.push_bits(0b110, 3)), "011");
    }

    #[test]
    fn every_number_reads_back_and_takes_the_bits_it_says() {
        let mut numbers: Vec<u64> = (0..300).collect();
        for shift in 0..64 {
            let power = 1u64 << shift;
            numbers.extend([power - 1, power, power + 1, power.wrapping_mul(3)]);
        }
        numbers.push(u64::MAX);
        numbers.push(u64::MAX - 1);
        for k in [0, 1, 5, 13, 32, 63] {
            let mut bits = Bits::default();
            for &n in &numbers {
                let before = bits.len;
                bits.gamma(n);
                assert_eq!((bits.len - before) as u64, gamma_len(n), "gamma {n}");
                let before = bits.len;
                bits.delta(n);
                assert_eq!((bits.len - before) as u64, delta_len(n), "delta {n}");
                let before = bits.len;
                bits.exp_golomb(n, k);
                assert_eq!((bits.len - before) as u64, exp_golomb_len(n, k), "exp-golomb {n}, {k}");
            }
            let mut reader = Reader::new(&bits.words, bits.len);
            for &n in &numbers {
                assert_eq!(reader.gamma(), Ok(n));
                assert_eq!(reader.delta(), Ok(n));
                assert_eq!(reader.exp_golomb(k), Ok(n));
            }
            assert_eq!(reader.finish(), Ok(()));
        }
    }

    #[test]
    fn after_the_end_come_zeros_up_to_a_limit() {
        let mut bits = Bits::default();
        bits.gamma(5);
        let end = significant(&bits.words);
        assert_eq!(end, bits.len);
        let mut reader = Reader::new(&bits.words, end);
        assert_eq!(reader.gamma(), Ok(5));
        // Zero, however it is read.
        assert_eq!((reader.gamma(), reader.delta(), reader.exp_golomb(9), reader.bits(64)), (Ok(0), Ok(0), Ok(0), Ok(0)));
        while reader.left() > 0 {
            assert_eq!(reader.bit(), Ok(false));
        }
        assert_eq!(reader.bit(), Err(Error::Malformed));
    }

    #[test]
    fn what_no_writer_writes_is_refused() {
        // 65 ones: wider than any number of 64 bits.
        let mut bits = Bits::default();
        bits.push_bits(u64::MAX, 64);
        bits.push(true);
        assert_eq!(Reader::new(&bits.words, bits.len).gamma(), Err(Error::Malformed));
        // One more than the widest number there is (u64::MAX: 64 ones, a zero, 64 zeros).
        let mut bits = Bits::default();
        bits.push_bits(u64::MAX, 64);
        bits.push(false);
        bits.push_bits(1, 64);
        assert_eq!(Reader::new(&bits.words, bits.len).gamma(), Err(Error::Malformed));
        // A length longer than what is left.
        let mut bits = Bits::default();
        bits.delta(1_000_000);
        assert_eq!(Reader::new(&bits.words, bits.len).length(), Err(Error::Malformed));
        let mut bits = Bits::default();
        bits.delta(1000);
        assert_eq!(Reader::new(&bits.words, bits.len).length(), Ok(1000));
    }

    #[test]
    fn appended_bits_follow_on() {
        let mut first = Bits::default();
        first.push_bits(0b101, 3);
        let mut second = Bits::default();
        second.push_bits(u64::MAX, 40);
        second.push(false);
        first.append(&second);
        assert_eq!(first.len, 44);
        assert_eq!(significant(&first.words), 43);
    }
}
