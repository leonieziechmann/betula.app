//! Writing a value as bits, the way serde describes it.

use serde::ser::{self, Serialize};

use crate::bits::{zigzag, Bits};
use crate::{fields, strings, Error};

/// Writes values: fields in their order, no names, every number in a code of its own length.
#[derive(Default)]
pub(crate) struct Serializer {
    pub(crate) bits: Bits,
}

impl<'a> ser::Serializer for &'a mut Serializer {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Compound<'a>;
    type SerializeTuple = Compound<'a>;
    type SerializeTupleStruct = Compound<'a>;
    type SerializeTupleVariant = Compound<'a>;
    type SerializeMap = Compound<'a>;
    type SerializeStruct = Compound<'a>;
    type SerializeStructVariant = Compound<'a>;

    fn serialize_bool(self, v: bool) -> Result<(), Error> {
        self.bits.push(v);
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<(), Error> {
        self.serialize_i64(v.into())
    }

    fn serialize_i16(self, v: i16) -> Result<(), Error> {
        self.serialize_i64(v.into())
    }

    fn serialize_i32(self, v: i32) -> Result<(), Error> {
        self.serialize_i64(v.into())
    }

    fn serialize_i64(self, v: i64) -> Result<(), Error> {
        self.bits.delta(zigzag(v));
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<(), Error> {
        self.serialize_u64(v.into())
    }

    fn serialize_u16(self, v: u16) -> Result<(), Error> {
        self.serialize_u64(v.into())
    }

    fn serialize_u32(self, v: u32) -> Result<(), Error> {
        self.serialize_u64(v.into())
    }

    fn serialize_u64(self, v: u64) -> Result<(), Error> {
        self.bits.delta(v);
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<(), Error> {
        self.bits.push_bits(v.to_bits().into(), 32);
        Ok(())
    }

    fn serialize_f64(self, v: f64) -> Result<(), Error> {
        self.bits.push_bits(v.to_bits(), 64);
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<(), Error> {
        self.serialize_u32(v.into())
    }

    fn serialize_str(self, v: &str) -> Result<(), Error> {
        strings::write(&mut self.bits, v);
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<(), Error> {
        self.bits.delta(v.len() as u64);
        for byte in v {
            self.bits.push_bits((*byte).into(), 8);
        }
        Ok(())
    }

    fn serialize_none(self) -> Result<(), Error> {
        self.bits.push(false);
        Ok(())
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<(), Error> {
        self.bits.push(true);
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), Error> {
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Error> {
        Ok(())
    }

    fn serialize_unit_variant(self, _name: &'static str, variant_index: u32, _variant: &'static str) -> Result<(), Error> {
        self.bits.gamma(variant_index.into());
        Ok(())
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(self, name: &'static str, value: &T) -> Result<(), Error> {
        match fields::Shape::named(name) {
            Some(shape) => fields::write(&mut self.bits, shape, value),
            None => value.serialize(self),
        }
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(self, _name: &'static str, variant_index: u32, _variant: &'static str, value: &T) -> Result<(), Error> {
        self.bits.gamma(variant_index.into());
        value.serialize(self)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Compound<'a>, Error> {
        Ok(Compound::counted(self, len))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Compound<'a>, Error> {
        Ok(Compound::fixed(self))
    }

    fn serialize_tuple_struct(self, _name: &'static str, _len: usize) -> Result<Compound<'a>, Error> {
        Ok(Compound::fixed(self))
    }

    fn serialize_tuple_variant(self, _name: &'static str, variant_index: u32, _variant: &'static str, _len: usize) -> Result<Compound<'a>, Error> {
        self.bits.gamma(variant_index.into());
        Ok(Compound::fixed(self))
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Compound<'a>, Error> {
        Ok(Compound::counted(self, len))
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Compound<'a>, Error> {
        Ok(Compound::fixed(self))
    }

    fn serialize_struct_variant(self, _name: &'static str, variant_index: u32, _variant: &'static str, _len: usize) -> Result<Compound<'a>, Error> {
        self.bits.gamma(variant_index.into());
        Ok(Compound::fixed(self))
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

/// The items of a sequence, a map, a tuple or a struct.
pub(crate) struct Compound<'a> {
    serializer: &'a mut Serializer,
    /// The items of a sequence whose length was not known when it began: its length goes first, so
    /// they wait here until it is.
    waiting: Option<Serializer>,
    /// How many items (or entries of a map) there were, and how many were said to come.
    count: u64,
    announced: Option<u64>,
}

impl<'a> Compound<'a> {
    fn fixed(serializer: &'a mut Serializer) -> Self {
        Compound { serializer, waiting: None, count: 0, announced: None }
    }

    fn counted(serializer: &'a mut Serializer, len: Option<usize>) -> Self {
        match len {
            Some(len) => {
                serializer.bits.delta(len as u64);
                Compound { announced: Some(len as u64), ..Compound::fixed(serializer) }
            }
            None => Compound { waiting: Some(Serializer::default()), ..Compound::fixed(serializer) },
        }
    }

    fn item<T: ?Sized + Serialize>(&mut self, value: &T, counts: bool) -> Result<(), Error> {
        if counts {
            self.count += 1;
            if self.announced.is_some_and(|announced| self.count > announced) {
                return Err(Error::Unsupported("more items than the sequence said it has"));
            }
        }
        match &mut self.waiting {
            Some(waiting) => value.serialize(waiting),
            None => value.serialize(&mut *self.serializer),
        }
    }

    fn done(self) -> Result<(), Error> {
        if self.announced.is_some_and(|announced| self.count != announced) {
            return Err(Error::Unsupported("fewer items than the sequence said it has"));
        }
        if let Some(waiting) = self.waiting {
            self.serializer.bits.delta(self.count);
            self.serializer.bits.append(&waiting.bits);
        }
        Ok(())
    }
}

/// A field left out (`skip_serializing_if`) would shift every field after it: the fields of a code
/// are known by their place alone.
fn skipped() -> Result<(), Error> {
    Err(Error::Unsupported("a field that is left out: the fields of a code are known by their place"))
}

impl ser::SerializeSeq for Compound<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.item(value, true)
    }

    fn end(self) -> Result<(), Error> {
        self.done()
    }
}

impl ser::SerializeTuple for Compound<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.item(value, true)
    }

    fn end(self) -> Result<(), Error> {
        self.done()
    }
}

impl ser::SerializeTupleStruct for Compound<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.item(value, true)
    }

    fn end(self) -> Result<(), Error> {
        self.done()
    }
}

impl ser::SerializeTupleVariant for Compound<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.item(value, true)
    }

    fn end(self) -> Result<(), Error> {
        self.done()
    }
}

impl ser::SerializeMap for Compound<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Error> {
        self.item(key, true)
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.item(value, false)
    }

    fn end(self) -> Result<(), Error> {
        self.done()
    }
}

impl ser::SerializeStruct for Compound<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, _key: &'static str, value: &T) -> Result<(), Error> {
        self.item(value, true)
    }

    fn skip_field(&mut self, _key: &'static str) -> Result<(), Error> {
        skipped()
    }

    fn end(self) -> Result<(), Error> {
        self.done()
    }
}

impl ser::SerializeStructVariant for Compound<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, _key: &'static str, value: &T) -> Result<(), Error> {
        self.item(value, true)
    }

    fn skip_field(&mut self, _key: &'static str) -> Result<(), Error> {
        skipped()
    }

    fn end(self) -> Result<(), Error> {
        self.done()
    }
}
