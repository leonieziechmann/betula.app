//! Fields that know more about their values than serde can say: integers that make a set or a list,
//! and bytes. Put on a field with `#[serde(with = "folia_pack::set")]` (or `list`, `bytes`).

use std::fmt;

use serde::de::value::SeqDeserializer;
use serde::de::{self, Deserialize, Deserializer, SeqAccess, Visitor};
use serde::ser::Serialize;

use crate::bits::{delta_len, exp_golomb_len, unzigzag, zigzag, Bits, Reader};
use crate::Error;

/// The names under which the helpers hand their integers to serde. A writer of codes knows them;
/// any other format sees a newtype around a sequence of integers, and writes that.
const SET: &str = "$folia_pack::set";
const LIST: &str = "$folia_pack::list";

#[derive(Clone, Copy)]
pub(crate) enum Shape {
    Set,
    List,
}

impl Shape {
    pub(crate) fn named(name: &str) -> Option<Self> {
        match name {
            SET => Some(Shape::Set),
            LIST => Some(Shape::List),
            _ => None,
        }
    }
}

/// Writes the integers a helper hands over. They come as a plain sequence, which is what any other
/// format should get: written the plain way, read back, and written as they are meant to be.
pub(crate) fn write<T: ?Sized + Serialize>(bits: &mut Bits, shape: Shape, value: &T) -> Result<(), Error> {
    let mut plain = crate::ser::Serializer::default();
    value.serialize(&mut plain)?;
    let mut reader = crate::de::Deserializer::new(Reader::new(plain.bits.words(), plain.bits.len()));
    let values = Vec::<u64>::deserialize(&mut reader)?;
    match shape {
        Shape::Set => write_set(bits, &values),
        Shape::List => {
            write_list(bits, &values);
            Ok(())
        }
    }
}

/// Reads the integers of a helper, and hands them over as the plain sequence it expects.
pub(crate) fn read<'de, V: Visitor<'de>>(bits: &mut Reader, shape: Shape, visitor: V) -> Result<V::Value, Error> {
    let values = match shape {
        Shape::Set => read_set(bits)?,
        Shape::List => read_list(bits)?,
    };
    visitor.visit_newtype_struct(SeqDeserializer::<_, Error>::new(values.into_iter()))
}

/// The order of the Exp-Golomb code that writes these numbers in the fewest bits (itself included).
fn best_order(numbers: &[u64]) -> u32 {
    (0..64).min_by_key(|k| delta_len(u64::from(*k)) + numbers.iter().map(|n| exp_golomb_len(*n, *k)).sum::<u64>()).unwrap_or(0)
}

fn read_order(bits: &mut Reader) -> Result<u32, Error> {
    u32::try_from(bits.delta()?).ok().filter(|k| *k < 64).ok_or(Error::Malformed)
}

/// How many, the first, then the order of the code and each gap to the one before (less one: they
/// all differ).
fn write_set(bits: &mut Bits, values: &[u64]) -> Result<(), Error> {
    bits.delta(values.len() as u64);
    let Some((first, rest)) = values.split_first() else { return Ok(()) };
    bits.delta(*first);
    if rest.is_empty() {
        return Ok(());
    }
    let gaps = values.iter().zip(rest).map(|(before, value)| value.checked_sub(*before)?.checked_sub(1)).collect::<Option<Vec<u64>>>().ok_or(Error::Unsupported("a set whose numbers do not ascend"))?;
    let k = best_order(&gaps);
    bits.delta(k.into());
    for gap in gaps {
        bits.exp_golomb(gap, k);
    }
    Ok(())
}

fn read_set(bits: &mut Reader) -> Result<Vec<u64>, Error> {
    let len = bits.length()?;
    let mut values = Vec::with_capacity(len.min(1 << 12));
    if len == 0 {
        return Ok(values);
    }
    let mut value = bits.delta()?;
    values.push(value);
    if len == 1 {
        return Ok(values);
    }
    let k = read_order(bits)?;
    for _ in 1..len {
        value = value.checked_add(bits.exp_golomb(k)?).and_then(|value| value.checked_add(1)).ok_or(Error::Malformed)?;
        values.push(value);
    }
    Ok(values)
}

/// How many, the first, then the order of the code and each step from the one before, up or down.
fn write_list(bits: &mut Bits, values: &[u64]) {
    bits.delta(values.len() as u64);
    let Some((first, rest)) = values.split_first() else { return };
    bits.delta(*first);
    if rest.is_empty() {
        return;
    }
    // A step is taken modulo 2^64, so every one there is fits into 64 bits, and a short one is small.
    let steps: Vec<u64> = values.iter().zip(rest).map(|(before, value)| zigzag(value.wrapping_sub(*before) as i64)).collect();
    let k = best_order(&steps);
    bits.delta(k.into());
    for step in steps {
        bits.exp_golomb(step, k);
    }
}

fn read_list(bits: &mut Reader) -> Result<Vec<u64>, Error> {
    let len = bits.length()?;
    let mut values = Vec::with_capacity(len.min(1 << 12));
    if len == 0 {
        return Ok(values);
    }
    let mut value = bits.delta()?;
    values.push(value);
    if len == 1 {
        return Ok(values);
    }
    let k = read_order(bits)?;
    for _ in 1..len {
        value = value.wrapping_add(unzigzag(bits.exp_golomb(k)?) as u64);
        values.push(value);
    }
    Ok(values)
}

/// Integers, as a helper expects them back: from a code, or as a sequence from any other format.
struct Integers;

impl<'de> Visitor<'de> for Integers {
    type Value = Vec<u64>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("unsigned integers")
    }

    fn visit_newtype_struct<D: Deserializer<'de>>(self, deserializer: D) -> Result<Vec<u64>, D::Error> {
        Vec::deserialize(deserializer)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<u64>, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = seq.next_element()? {
            values.push(value);
        }
        Ok(values)
    }
}

fn collect<C: FromIterator<T>, T: TryFrom<u64>, E: de::Error>(values: Vec<u64>) -> Result<C, E> {
    values.into_iter().map(|value| T::try_from(value).map_err(|_| E::custom(format!("{value} is too large for its field")))).collect()
}

/// A set of unsigned integers (`u8` to `u64`): `#[serde(with = "folia_pack::set")]` on a field that holds
/// them, a `Vec<u32>`, a `BTreeSet<u64>` or anything else that iterates over them and collects them.
///
/// Written in ascending order, each number as its distance from the one before, in the Exp-Golomb
/// code whose order suits these distances best: ids that lie close together take a few bits each,
/// and one far off costs twice the bits of its distance, not the distance. The order of the field is
/// not kept, and a number that is there twice is there once. For a list whose order counts, see
/// [`list`](crate::list).
pub mod set {
    use serde::{Deserializer, Serializer};

    pub fn serialize<'a, C, T, S>(set: &'a C, serializer: S) -> Result<S::Ok, S::Error>
    where
        &'a C: IntoIterator<Item = &'a T>,
        T: Copy + Into<u64> + 'a,
        S: Serializer,
    {
        let mut values: Vec<u64> = set.into_iter().map(|value| (*value).into()).collect();
        values.sort_unstable();
        values.dedup();
        serializer.serialize_newtype_struct(super::SET, &values)
    }

    pub fn deserialize<'de, C, T, D>(deserializer: D) -> Result<C, D::Error>
    where
        C: FromIterator<T>,
        T: TryFrom<u64>,
        D: Deserializer<'de>,
    {
        super::collect(deserializer.deserialize_newtype_struct(super::SET, super::Integers)?)
    }
}

/// A list of unsigned integers (`u8` to `u64`) in its order: `#[serde(with = "folia_pack::list")]` on a
/// field that holds them.
///
/// Written as the first number, then each step from the one before, up or down, in the Exp-Golomb
/// code whose order suits these steps best: numbers that follow each other closely take a few bits
/// each. Where the order does not count, [`set`](crate::set) takes fewer.
pub mod list {
    use serde::{Deserializer, Serializer};

    pub fn serialize<'a, C, T, S>(list: &'a C, serializer: S) -> Result<S::Ok, S::Error>
    where
        &'a C: IntoIterator<Item = &'a T>,
        T: Copy + Into<u64> + 'a,
        S: Serializer,
    {
        let values: Vec<u64> = list.into_iter().map(|value| (*value).into()).collect();
        serializer.serialize_newtype_struct(super::LIST, &values)
    }

    pub fn deserialize<'de, C, T, D>(deserializer: D) -> Result<C, D::Error>
    where
        C: FromIterator<T>,
        T: TryFrom<u64>,
        D: Deserializer<'de>,
    {
        super::collect(deserializer.deserialize_newtype_struct(super::LIST, super::Integers)?)
    }
}

/// Bytes as they are, eight bits each: `#[serde(with = "folia_pack::bytes")]` on a `Vec<u8>` (or anything
/// that holds bytes and is made from a `Vec<u8>`). Without it, serde hands over a `Vec<u8>` as a
/// list of numbers, and each takes the bits of its size: 1 for a zero, 14 for 200.
pub mod bytes {
    use std::fmt;

    use serde::de::{SeqAccess, Visitor};
    use serde::{Deserializer, Serializer};

    pub fn serialize<T: ?Sized + AsRef<[u8]>, S: Serializer>(bytes: &T, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(bytes.as_ref())
    }

    pub fn deserialize<'de, T: From<Vec<u8>>, D: Deserializer<'de>>(deserializer: D) -> Result<T, D::Error> {
        deserializer.deserialize_byte_buf(Bytes).map(T::from)
    }

    struct Bytes;

    impl<'de> Visitor<'de> for Bytes {
        type Value = Vec<u8>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("bytes")
        }

        fn visit_bytes<E>(self, bytes: &[u8]) -> Result<Vec<u8>, E> {
            Ok(bytes.to_vec())
        }

        fn visit_byte_buf<E>(self, bytes: Vec<u8>) -> Result<Vec<u8>, E> {
            Ok(bytes)
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<u8>, A::Error> {
            let mut bytes = Vec::new();
            while let Some(byte) = seq.next_element()? {
                bytes.push(byte);
            }
            Ok(bytes)
        }
    }
}
