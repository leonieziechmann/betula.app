//! Reading a value from bits, as the type it is read as describes it.
//!
//! The bits say nothing about what they hold: the type does (as with bincode or postcard). So there
//! is no `deserialize_any`, and with it nothing that needs it: untagged or internally tagged enums,
//! `flatten`, skipping unknown fields.

use serde::de::value::U32Deserializer;
use serde::de::{self, DeserializeSeed, IntoDeserializer, Visitor};

use crate::bits::{unzigzag, Reader};
use crate::{fields, strings, Error};

/// How deep values may be nested in one another: a reader follows a recursive type no further.
const MAX_DEPTH: usize = 64;

pub(crate) struct Deserializer<'a> {
    pub(crate) bits: Reader<'a>,
    depth: usize,
}

impl<'a> Deserializer<'a> {
    pub(crate) fn new(bits: Reader<'a>) -> Self {
        Deserializer { bits, depth: 0 }
    }

    /// Reads what lies one level deeper.
    fn nested<T>(&mut self, read: impl FnOnce(&mut Self) -> Result<T, Error>) -> Result<T, Error> {
        if self.depth == MAX_DEPTH {
            return Err(Error::Malformed);
        }
        self.depth += 1;
        let read = read(self);
        self.depth -= 1;
        read
    }

    fn unsigned<T: TryFrom<u64>>(&mut self) -> Result<T, Error> {
        T::try_from(self.bits.delta()?).map_err(|_| Error::Malformed)
    }

    fn signed<T: TryFrom<i64>>(&mut self) -> Result<T, Error> {
        T::try_from(unzigzag(self.bits.delta()?)).map_err(|_| Error::Malformed)
    }
}

fn not_self_describing() -> Error {
    Error::Unsupported("a type that does not say what it expects: the bits of a code do not say what they hold")
}

impl<'de> de::Deserializer<'de> for &mut Deserializer<'_> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Error> {
        Err(not_self_describing())
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_bool(self.bits.bit()?)
    }

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_i8(self.signed()?)
    }

    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_i16(self.signed()?)
    }

    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_i32(self.signed()?)
    }

    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_i64(self.signed()?)
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_u8(self.unsigned()?)
    }

    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_u16(self.unsigned()?)
    }

    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_u32(self.unsigned()?)
    }

    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_u64(self.unsigned()?)
    }

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_f32(f32::from_bits(self.bits.bits(32)? as u32))
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_f64(f64::from_bits(self.bits.bits(64)?))
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_char(char::from_u32(self.unsigned()?).ok_or(Error::Malformed)?)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_string(strings::read(&mut self.bits)?)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_string(strings::read(&mut self.bits)?)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_byte_buf(visitor)
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let len = self.bits.length()?;
        let bytes = (0..len).map(|_| self.bits.bits(8).map(|byte| byte as u8)).collect::<Result<Vec<u8>, Error>>()?;
        visitor.visit_byte_buf(bytes)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.bits.bit()? {
            false => visitor.visit_none(),
            true => self.nested(|de| visitor.visit_some(de)),
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(self, _name: &'static str, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(self, name: &'static str, visitor: V) -> Result<V::Value, Error> {
        match fields::Shape::named(name) {
            Some(shape) => fields::read(&mut self.bits, shape, visitor),
            None => self.nested(|de| visitor.visit_newtype_struct(de)),
        }
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let len = self.bits.length()?;
        self.nested(|de| visitor.visit_seq(Items { de, left: len }))
    }

    fn deserialize_tuple<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value, Error> {
        self.nested(|de| visitor.visit_seq(Items { de, left: len }))
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(self, _name: &'static str, len: usize, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_tuple(len, visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let len = self.bits.length()?;
        self.nested(|de| visitor.visit_map(Items { de, left: len }))
    }

    fn deserialize_struct<V: Visitor<'de>>(self, _name: &'static str, fields: &'static [&'static str], visitor: V) -> Result<V::Value, Error> {
        self.deserialize_tuple(fields.len(), visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(self, _name: &'static str, variants: &'static [&'static str], visitor: V) -> Result<V::Value, Error> {
        self.nested(|de| visitor.visit_enum(Variant { de, variants: variants.len() }))
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Error> {
        Err(not_self_describing())
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Error> {
        Err(not_self_describing())
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

/// The items of a sequence, a tuple or a struct, or the entries of a map: as many as it said.
struct Items<'a, 'b> {
    de: &'a mut Deserializer<'b>,
    left: usize,
}

impl<'de> de::SeqAccess<'de> for Items<'_, '_> {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>, Error> {
        if self.left == 0 {
            return Ok(None);
        }
        self.left -= 1;
        seed.deserialize(&mut *self.de).map(Some)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.left)
    }
}

impl<'de> de::MapAccess<'de> for Items<'_, '_> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>, Error> {
        if self.left == 0 {
            return Ok(None);
        }
        self.left -= 1;
        seed.deserialize(&mut *self.de).map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        seed.deserialize(&mut *self.de)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.left)
    }
}

/// A variant of an enum: its index in the gamma code, then what it holds.
struct Variant<'a, 'b> {
    de: &'a mut Deserializer<'b>,
    variants: usize,
}

impl<'de> de::EnumAccess<'de> for Variant<'_, '_> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self), Error> {
        let index = u32::try_from(self.de.bits.gamma()?).ok().filter(|index| (*index as usize) < self.variants).ok_or(Error::Malformed)?;
        let index: U32Deserializer<Error> = index.into_deserializer();
        Ok((seed.deserialize(index)?, self))
    }
}

impl<'de> de::VariantAccess<'de> for Variant<'_, '_> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        seed.deserialize(&mut *self.de)
    }

    fn tuple_variant<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_seq(Items { de: self.de, left: len })
    }

    fn struct_variant<V: Visitor<'de>>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value, Error> {
        visitor.visit_seq(Items { de: self.de, left: fields.len() })
    }
}
