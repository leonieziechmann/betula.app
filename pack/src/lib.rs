//! Values packed into the characters a link carries as they are: a bit format for serde, written in
//! base 66, with two check characters at the end.
//!
//! For what has to travel in a link and nowhere else: the marked modules on their way to another
//! device (`/bookmarks#m=…`, `app/src/bookmarks.rs`), later a timetable that a calendar subscribes to.
//! A value becomes a code of the unreserved characters of an address (`A–Z a–z 0–9 - . _ ~`, RFC
//! 3986), which no browser, server or chat program escapes or cuts, and comes back as the same value:
//!
//! ```
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Serialize, Deserialize, PartialEq, Debug)]
//! struct Plan {
//!     semester: u16,
//!     #[serde(with = "pack::set")]
//!     events: Vec<u32>,
//! }
//!
//! let plan = Plan { semester: 2026, events: vec![301_512, 301_517, 301_530, 302_048] };
//! let code = pack::to_code("plan", &plan)?;
//! assert_eq!(code, "2_w9p_YZ_8713UL");
//! assert_eq!(pack::from_code::<Plan>("plan", &code)?, plan);
//! # Ok::<(), pack::Error>(())
//! ```
//!
//! **Small.** A value is written as bits, not bytes: no field names, a `bool` in one bit, a number
//! in as many bits as its size needs, an id-like string in six bits a character. What serde cannot
//! know, a field can say: [`set`] writes integers as the gaps between them, [`list`] as the steps
//! from one to the next. So module numbers in the order they were marked take about 13 bits each,
//! two or three characters of a code instead of the six of `11101,`, and as a set fewer still. A
//! general-purpose compressor (deflate, zstd) would make codes like these longer: they are too
//! short for it to find anything to take out, and it would add what it needs to describe itself.
//!
//! **Checked.** The last two characters check the rest: a character typed wrong, or two next to
//! each other swapped, is caught always, anything else 4090 times out of 4091. What is checked
//! includes the kind of code, the first argument, so a code of one kind is not taken for another.
//!
//! **Lasting.** A code outlives the version of the type that wrote it (a calendar keeps its link for
//! months). What keeps old codes readable: *add fields only at the end of a struct, and only ones
//! whose zero is what they mean when absent* (`Option`, `Vec`, `bool`, a number that is 0 when
//! unset, an enum whose first variant is the default). A reader takes the bits after the end of a
//! code as zeros, and zero bits read as those values, so a code written before the field existed
//! reads with it absent. Never reorder, remove or retype a field, and never insert one; a field
//! that is left out (`skip_serializing_if`) is refused, since fields are known by their place. A
//! code with a field its reader does not know yet is refused ([`Error::Trailing`]) unless that
//! field is zero.
//!
//! # The format
//!
//! Frozen: every code written so far must keep its meaning.
//!
//! **Characters.** `ALPHABET` spells the digits 0 to 65. A code is its payload and two check
//! characters.
//!
//! **Payload.** The bits, read as one number whose lowest bit is the first one written, in base 66,
//! the lowest digit first, without zeros at the high end (so it ends in its last bit that is set,
//! and zero is the empty payload).
//!
//! **Check.** h = 0; for each byte of the kind, then each digit of the payload:
//! h = (h · 66 + value + 1) mod 4091. Written as `ALPHABET[h / 62]` and `ALPHABET[h % 62]`, so a
//! code ends in a letter or a digit (never in a `.` that a program making a link of it could take
//! for the end of a sentence). How far this checks: `text.rs`.
//!
//! **Bits.** First the version of this format, 0, in the gamma code (the bit `0`); a reader refuses
//! any other ([`Error::Format`]). Then the value, as serde describes it, in these codes:
//!
//! - *gamma* of n: as many ones as n + 1 has bits below its highest, a zero, those bits, lowest
//!   first: 0 → `0`, 1 → `100`, 2 → `101`, 3 → `11000`.
//! - *delta* of n: gamma of the number of bits of n + 1 below its highest, then those bits.
//! - *Exp-Golomb of order k*: gamma of n >> k, then the lowest k bits of n.
//!
//! | Value | Bits |
//! |---|---|
//! | `bool` | 1 |
//! | `u8`…`u64`, `char` | delta |
//! | `i8`…`i64` | delta of the zigzag (0, -1, 1, -2 … → 0, 1, 2, 3 …) |
//! | `f32`, `f64` | IEEE 754, 32 or 64 bits |
//! | `str`, `String` | delta of the length in bytes, then as `strings.rs` spells it |
//! | bytes ([`bytes`]) | delta of the length, 8 bits each |
//! | `Option` | `0` for none, `1` and the value for some |
//! | unit, unit struct | nothing |
//! | newtype struct | its value |
//! | tuple, struct | the fields in their order |
//! | sequence, map | delta of the length, the items (a map's as key, value) |
//! | enum | gamma of the index of the variant, then what it holds |
//! | [`set`] | delta of the count; if any: delta of the first; if more: delta of the order k, then each gap to the one before, less one, in Exp-Golomb of order k |
//! | [`list`] | the same with each step from the one before, taken modulo 2^64 as a signed number, zigzagged |
//!
//! A writer picks the k that takes the fewest bits. 128-bit integers are not written.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

mod bits;
mod de;
mod fields;
mod ser;
mod strings;
mod text;
#[cfg(test)]
mod tests;

use std::fmt;

use serde::de::DeserializeOwned;
use serde::Serialize;

pub use fields::{bytes, list, set};
pub use text::ALPHABET;

/// How long a code may be, check characters included. A code is read in time that grows with the
/// square of its length (a few milliseconds at this length), and a link longer than that would
/// not be one to type or paste anyway.
pub const MAX_LEN: usize = 16 * 1024;

/// The version of the format that codes are written in.
const FORMAT: u64 = 0;

/// The code for a value. `kind` says what it is for (`"bookmarks"`): a reader asks for the same.
pub fn to_code<T: ?Sized + Serialize>(kind: &str, value: &T) -> Result<String, Error> {
    let mut writer = ser::Serializer::default();
    writer.bits.gamma(FORMAT);
    value.serialize(&mut writer)?;
    text::encode(kind, &writer.bits)
}

/// The value a code of this kind carries.
pub fn from_code<T: DeserializeOwned>(kind: &str, code: &str) -> Result<T, Error> {
    let (number, end) = text::decode(kind, code)?;
    let mut reader = de::Deserializer::new(bits::Reader::new(&number, end));
    if reader.bits.gamma()? != FORMAT {
        return Err(Error::Format);
    }
    let value = T::deserialize(&mut reader)?;
    reader.bits.finish()?;
    Ok(value)
}

/// Why a value could not be written as a code, or a code not read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A character that no code has, at this byte of it.
    Character(usize),
    /// Longer than [`MAX_LEN`].
    TooLong,
    /// The check characters do not fit the rest: a character typed wrong, a code cut short or run
    /// into what follows it, or a code of another kind.
    Check,
    /// A code that checks out, but that no writer makes: bits that spell no value of the type, a
    /// number out of the range of its field, a variant that does not exist.
    Malformed,
    /// Bits left after the value: written by a newer version of the type, with a field this one
    /// does not know yet.
    Trailing,
    /// Written in a version of the format this reader does not know yet.
    Format,
    /// What this format does not write or read: see the message.
    Unsupported(&'static str),
    /// What the type itself has to say about a value (serde's `custom`).
    Message(String),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Character(at) => write!(formatter, "a character that no code has, at {at}"),
            Error::TooLong => write!(formatter, "longer than a code may be ({MAX_LEN} characters)"),
            Error::Check => formatter.write_str("the check characters do not fit: a character typed wrong, or not the whole code"),
            Error::Malformed => formatter.write_str("a code that no writer makes"),
            Error::Trailing => formatter.write_str("more than this version knows of: written by a newer one"),
            Error::Format => formatter.write_str("written in a version of the format this reader does not know"),
            Error::Unsupported(what) => write!(formatter, "not in this format: {what}"),
            Error::Message(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for Error {}

impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Error::Message(message.to_string())
    }
}

impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Error::Message(message.to_string())
    }
}
