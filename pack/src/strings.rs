//! Strings, in the fewest bits per character their characters allow.
//!
//! A string is its length in bytes (delta code) and, unless it is empty, how it is spelled:
//!
//! | Spelling | Bits | Characters | Per character |
//! |---|---|---|---|
//! | digits | `0` | `0`–`9` | 10 bits for three, 7 for two, 4 for one |
//! | name | `10` | `A`–`Z`, `a`–`z`, `0`–`9`, `-`, `_` (in this order) | 6 bits |
//! | ASCII | `110` | ASCII | 7 bits |
//! | UTF-8 | `111` | anything | 8 bits a byte |
//!
//! A writer takes the first that fits. Ids and names of things are the common strings of a link:
//! a module number as a string takes 23 bits instead of the 40 of its bytes.

use crate::bits::{Bits, Reader};
use crate::Error;

/// The six-bit value of a character of a name.
fn name_value(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

/// The character of a name with this six-bit value.
fn name_char(value: u64) -> u8 {
    match value {
        0..=25 => b'A' + value as u8,
        26..=51 => b'a' + (value - 26) as u8,
        52..=61 => b'0' + (value - 52) as u8,
        62 => b'-',
        _ => b'_',
    }
}

/// How many bits a group of up to three decimal digits takes.
fn digit_bits(digits: usize) -> u32 {
    match digits {
        1 => 4,
        2 => 7,
        _ => 10,
    }
}

pub(crate) fn write(bits: &mut Bits, text: &str) {
    let bytes = text.as_bytes();
    bits.delta(bytes.len() as u64);
    if bytes.is_empty() {
        return;
    }
    if bytes.iter().all(u8::is_ascii_digit) {
        bits.push(false);
        for group in bytes.chunks(3) {
            let value = group.iter().fold(0, |value, digit| value * 10 + u64::from(digit - b'0'));
            bits.push_bits(value, digit_bits(group.len()));
        }
    } else if bytes.iter().all(|c| name_value(*c).is_some()) {
        bits.push_bits(0b01, 2);
        for c in bytes {
            bits.push_bits(name_value(*c).map_or(0, u64::from), 6);
        }
    } else if text.is_ascii() {
        bits.push_bits(0b011, 3);
        for c in bytes {
            bits.push_bits(u64::from(*c), 7);
        }
    } else {
        bits.push_bits(0b111, 3);
        for c in bytes {
            bits.push_bits(u64::from(*c), 8);
        }
    }
}

pub(crate) fn read(bits: &mut Reader) -> Result<String, Error> {
    let len = bits.length()?;
    if len == 0 {
        return Ok(String::new());
    }
    let mut bytes: Vec<u8> = Vec::with_capacity(len);
    if !bits.bit()? {
        let mut left = len;
        while left > 0 {
            let group = left.min(3);
            let value = bits.bits(digit_bits(group))?;
            if value >= 10u64.pow(group as u32) {
                return Err(Error::Malformed);
            }
            for place in (0..group as u32).rev() {
                bytes.push(b'0' + (value / 10u64.pow(place) % 10) as u8);
            }
            left -= group;
        }
    } else if !bits.bit()? {
        for _ in 0..len {
            bytes.push(name_char(bits.bits(6)?));
        }
    } else {
        let width = if bits.bit()? { 8 } else { 7 };
        for _ in 0..len {
            bytes.push(bits.bits(width)? as u8);
        }
    }
    String::from_utf8(bytes).map_err(|_| Error::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(text: &str) -> Bits {
        let mut bits = Bits::default();
        write(&mut bits, text);
        bits
    }

    #[test]
    fn every_string_reads_back_in_the_fewest_bits_its_characters_allow() {
        // Length, spelling, characters.
        let cases = [
            ("", 1),
            ("0", 4 + 1 + 4),
            ("12204", 5 + 1 + 17),
            ("0012204", 8 + 1 + 24),
            ("FUES-7", 5 + 2 + 36),
            ("Lineare_Algebra", 9 + 2 + 90),
            ("Analysis I", 8 + 3 + 70),
            ("Übung ~ 3.", 8 + 3 + 88),
            ("Grüße 😀", 8 + 3 + 96),
        ];
        for (text, len) in cases {
            let bits = written(text);
            assert_eq!(bits.len(), len, "{text}");
            let mut reader = Reader::new(bits.words(), bits.len());
            assert_eq!(read(&mut reader).as_deref(), Ok(text));
            assert_eq!(reader.finish(), Ok(()));
        }
        for value in 0..64 {
            assert_eq!(name_value(name_char(value)), Some(value as u8));
        }
        let every_digit_string: Vec<String> = (0..1000).map(|n| format!("{n:03}")).chain((0..100).map(|n| format!("{n:02}"))).chain((0..10).map(|n| n.to_string())).collect();
        for text in &every_digit_string {
            let bits = written(text);
            assert_eq!(read(&mut Reader::new(bits.words(), bits.len())).as_deref(), Ok(text.as_str()));
        }
    }

    #[test]
    fn what_no_writer_writes_is_refused() {
        // Three digits in ten bits go up to 999, not to 1023.
        let mut bits = Bits::default();
        bits.delta(3);
        bits.push(false);
        bits.push_bits(1000, 10);
        assert_eq!(read(&mut Reader::new(bits.words(), bits.len())), Err(Error::Malformed));
        // Bytes that are no UTF-8.
        let mut bits = Bits::default();
        bits.delta(2);
        bits.push_bits(0b111, 3);
        bits.push_bits(0xc3, 8);
        bits.push_bits(0x28, 8);
        assert_eq!(read(&mut Reader::new(bits.words(), bits.len())), Err(Error::Malformed));
        // Longer than a code can be.
        let mut bits = Bits::default();
        bits.delta(1 << 40);
        assert_eq!(read(&mut Reader::new(bits.words(), bits.len())), Err(Error::Malformed));
    }
}
