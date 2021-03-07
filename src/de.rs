//! Deserialization.
//!
//!

use std::fmt;

use fog_crypto::serde::FOG_TYPE_ENUM;
use serde::de::Error as DeError;
use serde::de::*;

use crate::{
    element::*,
    error::{Error, Result},
    get_int_internal,
    integer::IntPriv,
};

pub(crate) struct FogDeserializer<'a> {
    parser: Parser<'a>,
}

impl<'a> FogDeserializer<'a> {
    pub(crate) fn new(buf: &'a [u8]) -> Self {
        Self {
            parser: Parser::new(buf),
        }
    }

    pub(crate) fn from_parser(parser: Parser<'a>) -> Self {
        Self {
            parser
        }
    }

    pub(crate) fn with_debug(buf: &'a [u8], indent: String) -> Self {
        Self {
            parser: Parser::with_debug(buf, indent),
        }
    }

    pub(crate) fn get_debug(&self) -> Option<&str> {
        self.parser.get_debug()
    }

    pub(crate) fn finish(self) -> Result<()> {
        self.parser.finish()
    }

    fn next_elem(&mut self) -> Result<Element<'a>> {
        let elem = self
            .parser
            .next()
            .ok_or_else(|| Error::SerdeFail("missing next value".to_string()))??;
        Ok(elem)
    }

}

impl<'de, 'a> serde::Deserializer<'de> for &'a mut FogDeserializer<'de> {
    type Error = Error;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let elem = self.next_elem()?;
        match elem {
            Element::Null => visitor.visit_unit(),
            Element::Bool(v) => visitor.visit_bool(v),
            Element::Int(ref v) => match get_int_internal(v) {
                IntPriv::PosInt(v) => visitor.visit_u64(v),
                IntPriv::NegInt(v) => visitor.visit_i64(v),
            },
            Element::Str(v) => visitor.visit_borrowed_str(v),
            Element::F32(v) => visitor.visit_f32(v),
            Element::F64(v) => visitor.visit_f64(v),
            Element::Bin(v) => visitor.visit_borrowed_bytes(v),
            Element::Array(len) => visitor.visit_seq(SeqAccess::new(self, len)),
            Element::Map(len) => visitor.visit_map(MapAccess::new(self, len)),
            Element::Timestamp(v) => visitor.visit_enum(ExtAccess::new(Element::Timestamp(v))),
            Element::Hash(v) => visitor.visit_enum(ExtAccess::new(Element::Hash(v))),
            Element::Identity(v) => visitor.visit_enum(ExtAccess::new(Element::Identity(v))),
            Element::LockId(v) => visitor.visit_enum(ExtAccess::new(Element::LockId(v))),
            Element::StreamId(v) => visitor.visit_enum(ExtAccess::new(Element::StreamId(v))),
            Element::DataLockbox(v) => visitor.visit_enum(ExtAccess::new(Element::DataLockbox(v))),
            Element::IdentityLockbox(v) => {
                visitor.visit_enum(ExtAccess::new(Element::IdentityLockbox(v)))
            }
            Element::StreamLockbox(v) => {
                visitor.visit_enum(ExtAccess::new(Element::StreamLockbox(v)))
            }
            Element::LockLockbox(v) => visitor.visit_enum(ExtAccess::new(Element::LockLockbox(v))),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        use crate::marker::Marker;
        let marker = self
            .parser
            .peek_marker()
            .ok_or_else(|| Error::SerdeFail("missing next value".to_string()))?;
        if marker == Marker::Null {
            self.next_elem()?;
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        if name == FOG_TYPE_ENUM {
            let elem = self.next_elem()?;
            match elem {
                Element::Timestamp(v) => visitor.visit_enum(ExtAccess::new(Element::Timestamp(v))),
                Element::Hash(v) => visitor.visit_enum(ExtAccess::new(Element::Hash(v))),
                Element::Identity(v) => visitor.visit_enum(ExtAccess::new(Element::Identity(v))),
                Element::LockId(v) => visitor.visit_enum(ExtAccess::new(Element::LockId(v))),
                Element::StreamId(v) => visitor.visit_enum(ExtAccess::new(Element::StreamId(v))),
                Element::DataLockbox(v) => {
                    visitor.visit_enum(ExtAccess::new(Element::DataLockbox(v)))
                }
                Element::IdentityLockbox(v) => {
                    visitor.visit_enum(ExtAccess::new(Element::IdentityLockbox(v)))
                }
                Element::StreamLockbox(v) => {
                    visitor.visit_enum(ExtAccess::new(Element::StreamLockbox(v)))
                }
                Element::LockLockbox(v) => {
                    visitor.visit_enum(ExtAccess::new(Element::LockLockbox(v)))
                }
                _ => Err(Error::invalid_type(
                    elem.unexpected(),
                    &"known fogpack specialized type",
                )),
            }
        } else {
            visitor.visit_enum(EnumAccess::new(self))
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str
        string bytes byte_buf unit unit_struct newtype_struct
        seq tuple tuple_struct map struct identifier ignored_any
    }
}

struct ExtAccess<'de> {
    element: Element<'de>,
    tag_was_read: bool,
}

impl<'de> ExtAccess<'de> {
    fn new(element: Element<'de>) -> Self {
        Self {
            element,
            tag_was_read: false,
        }
    }
}

impl<'de> serde::de::EnumAccess<'de> for ExtAccess<'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(mut self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: DeserializeSeed<'de>,
    {
        let val = seed.deserialize(&mut self)?;
        Ok((val, self))
    }
}

impl<'de> serde::de::VariantAccess<'de> for ExtAccess<'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        let unexp = Unexpected::NewtypeVariant;
        Err(Error::invalid_type(unexp, &"unit variant"))
    }

    fn newtype_variant_seed<T>(mut self, seed: T) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        println!("You picked the newtype, good jorb");
        seed.deserialize(&mut self)
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let unexp = Unexpected::NewtypeVariant;
        Err(Error::invalid_type(unexp, &"struct variant"))
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let unexp = Unexpected::TupleVariant;
        Err(Error::invalid_type(unexp, &"tuple variant"))
    }
}

impl<'de> Deserializer<'de> for &mut ExtAccess<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if !self.tag_was_read {
            use fog_crypto::serde::*;
            let variant = match self.element {
                Element::Timestamp(_) => FOG_TYPE_ENUM_TIME_INDEX,
                Element::Hash(_) => FOG_TYPE_ENUM_HASH_INDEX,
                Element::Identity(_) => FOG_TYPE_ENUM_IDENTITY_INDEX,
                Element::LockId(_) => FOG_TYPE_ENUM_LOCK_ID_INDEX,
                Element::StreamId(_) => FOG_TYPE_ENUM_STREAM_ID_INDEX,
                Element::DataLockbox(_) => FOG_TYPE_ENUM_DATA_LOCKBOX_INDEX,
                Element::IdentityLockbox(_) => FOG_TYPE_ENUM_IDENTITY_LOCKBOX_INDEX,
                Element::StreamLockbox(_) => FOG_TYPE_ENUM_STREAM_LOCKBOX_INDEX,
                Element::LockLockbox(_) => FOG_TYPE_ENUM_LOCK_LOCKBOX_INDEX,
                _ => unreachable!("ExtAccess should never see any other Element type"),
            };
            self.tag_was_read = true;
            visitor.visit_u64(variant)
        } else {
            match self.element {
                Element::Timestamp(ref v) => visitor.visit_byte_buf(v.as_vec()),
                Element::Hash(ref v) => visitor.visit_bytes(v.as_ref()),
                Element::Identity(ref v) => visitor.visit_byte_buf(v.as_vec()),
                Element::LockId(ref v) => visitor.visit_byte_buf(v.as_vec()),
                Element::StreamId(ref v) => visitor.visit_byte_buf(v.as_vec()),
                Element::DataLockbox(v) => visitor.visit_borrowed_bytes(v.as_bytes()),
                Element::IdentityLockbox(v) => visitor.visit_borrowed_bytes(v.as_bytes()),
                Element::StreamLockbox(v) => visitor.visit_borrowed_bytes(v.as_bytes()),
                Element::LockLockbox(v) => visitor.visit_borrowed_bytes(v.as_bytes()),
                _ => unreachable!("ExtAccess should never see any other Element type"),
            }
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str
        string bytes byte_buf option unit unit_struct newtype_struct
        seq tuple tuple_struct map struct enum identifier ignored_any
    }
}

struct EnumAccess<'a, 'de> {
    de: &'a mut FogDeserializer<'de>,
    has_value: bool,
}

impl<'a, 'de> EnumAccess<'a, 'de> {
    fn new(de: &'a mut FogDeserializer<'de>) -> Self {
        Self {
            de,
            has_value: false,
        }
    }
}

impl<'a, 'de> serde::de::EnumAccess<'de> for EnumAccess<'a, 'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(mut self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: DeserializeSeed<'de>,
    {
        use crate::marker::Marker;
        let marker = self
            .de
            .parser
            .peek_marker()
            .ok_or_else(|| Error::SerdeFail("missing next value".to_string()))?;
        let val = match marker {
            Marker::FixMap(1) => {
                self.de.next_elem()?;
                self.has_value = true;
                seed.deserialize(&mut *self.de)?
            }
            Marker::FixStr(_) | Marker::Str8 | Marker::Str16 | Marker::Str24 => {
                self.has_value = false;
                seed.deserialize(&mut *self.de)?
            }
            _ => {
                return Err(Error::SerdeFail(
                    "expected a size-1 map or a string".to_string(),
                ))
            }
        };
        Ok((val, self))
    }
}

impl<'a, 'de> serde::de::VariantAccess<'de> for EnumAccess<'a, 'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        if self.has_value {
            Err(Error::SerdeFail(
                "invalid type: non-unit variant, expected unit variant".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        if self.has_value {
            seed.deserialize(&mut *self.de)
        } else {
            Err(Error::SerdeFail(
                "invalid type: unit variant, expected newtype variant".to_string(),
            ))
        }
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.has_value {
            self.de.deserialize_map(visitor)
        } else {
            Err(Error::SerdeFail(
                "invalid type: unit variant, expected newtype variant".to_string(),
            ))
        }
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.has_value {
            self.de.deserialize_tuple(len, visitor)
        } else {
            Err(Error::SerdeFail(
                "invalid type: unit variant, expected newtype variant".to_string(),
            ))
        }
    }
}

struct SeqAccess<'a, 'de> {
    de: &'a mut FogDeserializer<'de>,
    size_left: usize,
}

impl<'a, 'de> SeqAccess<'a, 'de> {
    fn new(de: &'a mut FogDeserializer<'de>, len: usize) -> Self {
        Self { de, size_left: len }
    }
}

impl<'a, 'de> serde::de::SeqAccess<'de> for SeqAccess<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        if self.size_left > 0 {
            self.size_left -= 1;
            let val = seed.deserialize(&mut *self.de)?;
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.size_left)
    }
}

#[derive(Clone, Copy)]
struct KeyStr<'de>(&'de str);

impl<'de> Deserialize<'de> for KeyStr<'de> {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct KeyVisitor;
        impl<'de> Visitor<'de> for KeyVisitor {
            type Value = KeyStr<'de>;

            fn expecting(
                &self,
                fmt: &mut fmt::Formatter<'_>,
            ) -> std::result::Result<(), fmt::Error> {
                write!(fmt, "a key string")
            }

            fn visit_borrowed_str<E: serde::de::Error>(
                self,
                v: &'de str,
            ) -> std::result::Result<Self::Value, E> {
                Ok(KeyStr(v))
            }
        }

        deserializer.deserialize_str(KeyVisitor)
    }
}

impl<'de> Deserializer<'de> for KeyStr<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_borrowed_str(self.0)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str
        string bytes byte_buf option unit unit_struct newtype_struct
        seq tuple tuple_struct map struct enum identifier ignored_any
    }
}

struct MapAccess<'a, 'de> {
    de: &'a mut FogDeserializer<'de>,
    size_left: usize,
    last_str: Option<KeyStr<'de>>,
}

impl<'a, 'de> MapAccess<'a, 'de> {
    fn new(de: &'a mut FogDeserializer<'de>, len: usize) -> Self {
        Self {
            de,
            size_left: len,
            last_str: None,
        }
    }
}

impl<'a, 'de> serde::de::MapAccess<'de> for MapAccess<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        if self.size_left > 0 {
            self.size_left -= 1;
            if let Some(last_str) = self.last_str {
                let new_str = KeyStr::deserialize(&mut *self.de)?;
                if new_str.0 <= last_str.0 {
                    return Err(Error::SerdeFail(format!(
                        "map keys are unordered: {} follows {}",
                        new_str.0, last_str.0
                    )));
                }
                self.last_str = Some(new_str);
            } else {
                self.last_str = Some(KeyStr::deserialize(&mut *self.de)?);
            }
            Ok(Some(seed.deserialize(self.last_str.unwrap())?))
        } else {
            Ok(None)
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        seed.deserialize(&mut *self.de)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.size_left)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde::Deserialize;

    #[test]
    fn de_unit() {
        let data = vec![0xc0];
        let mut de = FogDeserializer::new(&data);
        <()>::deserialize(&mut de).unwrap();
        de.finish().unwrap();
    }

    #[test]
    fn de_bool() {
        let data = vec![0xc3];
        let mut de = FogDeserializer::new(&data);
        let dec = bool::deserialize(&mut de).unwrap();
        de.finish().unwrap();
        assert_eq!(dec, true);

        let data = vec![0xc2];
        let mut de = FogDeserializer::new(&data);
        let dec = bool::deserialize(&mut de).unwrap();
        de.finish().unwrap();
        assert_eq!(dec, false);
    }

    #[test]
    fn de_u8() {
        let mut test_cases: Vec<(u8, Vec<u8>)> = Vec::new();
        test_cases.push((0x00, vec![0x00]));
        test_cases.push((0x01, vec![0x01]));
        test_cases.push((0x7f, vec![0x7f]));
        test_cases.push((0x80, vec![0xcc, 0x80]));
        test_cases.push((0xff, vec![0xcc, 0xff]));
        
        for (int, enc) in test_cases {
            let mut de = FogDeserializer::new(&enc);
            let dec = u8::deserialize(&mut de).unwrap();
            de.finish().unwrap();
            assert_eq!(dec, int);
        }
    }

    #[test]
    fn de_u16() {
        let mut test_cases: Vec<(u16, Vec<u8>)> = Vec::new();
        test_cases.push((0x0000, vec![0x00]));
        test_cases.push((0x0001, vec![0x01]));
        test_cases.push((0x007f, vec![0x7f]));
        test_cases.push((0x0080, vec![0xcc, 0x80]));
        test_cases.push((0x00ff, vec![0xcc, 0xff]));
        test_cases.push((0x0100, vec![0xcd, 0x00, 0x01]));
        test_cases.push((0xffff, vec![0xcd, 0xff, 0xff]));
        
        for (int, enc) in test_cases {
            let mut de = FogDeserializer::new(&enc);
            let dec = u16::deserialize(&mut de).unwrap();
            de.finish().unwrap();
            assert_eq!(dec, int);
        }
    }

    #[test]
    fn de_u32() {
        let mut test_cases: Vec<(u32, Vec<u8>)> = Vec::new();
        test_cases.push((0x0000_0000, vec![0x00]));
        test_cases.push((0x0000_0001, vec![0x01]));
        test_cases.push((0x0000_007f, vec![0x7f]));
        test_cases.push((0x0000_0080, vec![0xcc, 0x80]));
        test_cases.push((0x0000_00ff, vec![0xcc, 0xff]));
        test_cases.push((0x0000_0100, vec![0xcd, 0x00, 0x01]));
        test_cases.push((0x0000_ffff, vec![0xcd, 0xff, 0xff]));
        test_cases.push((0x0001_0000, vec![0xce, 0x00, 0x00, 0x01, 0x00]));
        test_cases.push((0xffff_ffff, vec![0xce, 0xff, 0xff, 0xff, 0xff]));
        
        for (int, enc) in test_cases {
            let mut de = FogDeserializer::new(&enc);
            let dec = u32::deserialize(&mut de).unwrap();
            de.finish().unwrap();
            assert_eq!(dec, int);
        }
    }

    #[test]
    fn de_u64() {
        let mut test_cases: Vec<(u64, Vec<u8>)> = Vec::new();
        test_cases.push((0x0000_0000, vec![0x00]));
        test_cases.push((0x0000_0001, vec![0x01]));
        test_cases.push((0x0000_007f, vec![0x7f]));
        test_cases.push((0x0000_0080, vec![0xcc, 0x80]));
        test_cases.push((0x0000_00ff, vec![0xcc, 0xff]));
        test_cases.push((0x0000_0100, vec![0xcd, 0x00, 0x01]));
        test_cases.push((0x0000_ffff, vec![0xcd, 0xff, 0xff]));
        test_cases.push((0x0001_0000, vec![0xce, 0x00, 0x00, 0x01, 0x00]));
        test_cases.push((0xffff_ffff, vec![0xce, 0xff, 0xff, 0xff, 0xff]));
        test_cases.push((
                u32::MAX as u64 + 1,
                vec![0xcf, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00],
        ));
        test_cases.push((
                u64::MAX,
                vec![0xcf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        ));
        
        for (int, enc) in test_cases {
            let mut de = FogDeserializer::new(&enc);
            let dec = u64::deserialize(&mut de).unwrap();
            de.finish().unwrap();
            assert_eq!(dec, int);
        }
    }

    #[test]
    fn de_i8() {
        let mut test_cases: Vec<(i8, Vec<u8>)> = Vec::new();
        test_cases.push((0x00, vec![0x00]));
        test_cases.push((0x01, vec![0x01]));
        test_cases.push((0x7f, vec![0x7f]));
        test_cases.push((-1, vec![0xff]));
        test_cases.push((-2, vec![0xfe]));
        test_cases.push((-32, vec![0xe0]));
        test_cases.push((-33, vec![0xd0, 0xdf]));
        test_cases.push((i8::MIN as i8, vec![0xd0, 0x80]));
        
        for (int, enc) in test_cases {
            let mut de = FogDeserializer::new(&enc);
            let dec = i8::deserialize(&mut de).unwrap();
            de.finish().unwrap();
            assert_eq!(dec, int);
        }
    }

    #[test]
    fn de_i16() {
        let mut test_cases: Vec<(i16, Vec<u8>)> = Vec::new();
        test_cases.push((0x0000, vec![0x00]));
        test_cases.push((0x0001, vec![0x01]));
        test_cases.push((0x007f, vec![0x7f]));
        test_cases.push((0x0080, vec![0xcc, 0x80]));
        test_cases.push((0x00ff, vec![0xcc, 0xff]));
        test_cases.push((0x0100, vec![0xcd, 0x00, 0x01]));
        test_cases.push((-1, vec![0xff]));
        test_cases.push((-2, vec![0xfe]));
        test_cases.push((-32, vec![0xe0]));
        test_cases.push((-33, vec![0xd0, 0xdf]));
        test_cases.push((i8::MIN as i16, vec![0xd0, 0x80]));
        test_cases.push((i8::MIN as i16 - 1, vec![0xd1, 0x7f, 0xff]));
        test_cases.push((i16::MIN as i16, vec![0xd1, 0x00, 0x80]));
        
        for (int, enc) in test_cases {
            let mut de = FogDeserializer::new(&enc);
            let dec = i16::deserialize(&mut de).unwrap();
            de.finish().unwrap();
            assert_eq!(dec, int);
        }
    }

    #[test]
    fn de_i32() {
        let mut test_cases: Vec<(i32, Vec<u8>)> = Vec::new();
        test_cases.push((0x0000_0000, vec![0x00]));
        test_cases.push((0x0000_0001, vec![0x01]));
        test_cases.push((0x0000_007f, vec![0x7f]));
        test_cases.push((0x0000_0080, vec![0xcc, 0x80]));
        test_cases.push((0x0000_00ff, vec![0xcc, 0xff]));
        test_cases.push((0x0000_0100, vec![0xcd, 0x00, 0x01]));
        test_cases.push((0x0000_ffff, vec![0xcd, 0xff, 0xff]));
        test_cases.push((0x0001_0000, vec![0xce, 0x00, 0x00, 0x01, 0x00]));
        test_cases.push((-1, vec![0xff]));
        test_cases.push((-2, vec![0xfe]));
        test_cases.push((-32, vec![0xe0]));
        test_cases.push((-33, vec![0xd0, 0xdf]));
        test_cases.push((i8::MIN as i32, vec![0xd0, 0x80]));
        test_cases.push((i8::MIN as i32 - 1, vec![0xd1, 0x7f, 0xff]));
        test_cases.push((i16::MIN as i32, vec![0xd1, 0x00, 0x80]));
        test_cases.push((i16::MIN as i32 - 1, vec![0xd2, 0xff, 0x7f, 0xff, 0xff]));
        test_cases.push((i32::MIN as i32, vec![0xd2, 0x00, 0x00, 0x00, 0x80]));
        
        for (int, enc) in test_cases {
            let mut de = FogDeserializer::new(&enc);
            let dec = i32::deserialize(&mut de).unwrap();
            de.finish().unwrap();
            assert_eq!(dec, int);
        }
    }

    #[test]
    fn de_i64() {
        let mut test_cases: Vec<(i64, Vec<u8>)> = Vec::new();
        test_cases.push((0x0000_0000, vec![0x00]));
        test_cases.push((0x0000_0001, vec![0x01]));
        test_cases.push((0x0000_007f, vec![0x7f]));
        test_cases.push((0x0000_0080, vec![0xcc, 0x80]));
        test_cases.push((0x0000_00ff, vec![0xcc, 0xff]));
        test_cases.push((0x0000_0100, vec![0xcd, 0x00, 0x01]));
        test_cases.push((0x0000_ffff, vec![0xcd, 0xff, 0xff]));
        test_cases.push((0x0001_0000, vec![0xce, 0x00, 0x00, 0x01, 0x00]));
        test_cases.push((0xffff_ffff, vec![0xce, 0xff, 0xff, 0xff, 0xff]));
        test_cases.push((
                u32::MAX as i64 + 1,
                vec![0xcf, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00],
        ));
        test_cases.push((-1, vec![0xff]));
        test_cases.push((-2, vec![0xfe]));
        test_cases.push((-32, vec![0xe0]));
        test_cases.push((-33, vec![0xd0, 0xdf]));
        test_cases.push((i8::MIN as i64, vec![0xd0, 0x80]));
        test_cases.push((i8::MIN as i64 - 1, vec![0xd1, 0x7f, 0xff]));
        test_cases.push((i16::MIN as i64, vec![0xd1, 0x00, 0x80]));
        test_cases.push((i16::MIN as i64 - 1, vec![0xd2, 0xff, 0x7f, 0xff, 0xff]));
        test_cases.push((i32::MIN as i64, vec![0xd2, 0x00, 0x00, 0x00, 0x80]));
        test_cases.push((
                i32::MIN as i64 - 1,
                vec![0xd3, 0xff, 0xff, 0xff, 0x7f, 0xff, 0xff, 0xff, 0xff],
        ));
        test_cases.push((
                i64::MIN,
                vec![0xd3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80],
        ));
        
        for (int, enc) in test_cases {
            let mut de = FogDeserializer::new(&enc);
            let dec = i64::deserialize(&mut de).unwrap();
            de.finish().unwrap();
            assert_eq!(dec, int);
        }
    }

    #[test]
    fn de_f32() {
        let mut test_cases: Vec<(f32, Vec<u8>)> = Vec::new();
        test_cases.push((0.0, vec![0xca, 0x00, 0x00, 0x00, 0x00]));
        test_cases.push((1.0, vec![0xca, 0x00, 0x00, 0x80, 0x3f]));
        test_cases.push((-1.0, vec![0xca, 0x00, 0x00, 0x80, 0xbf]));
        test_cases.push((f32::NEG_INFINITY, vec![0xca, 0x00, 0x00, 0x80, 0xff]));
        test_cases.push((f32::INFINITY, vec![0xca, 0x00, 0x00, 0x80, 0x7f]));
        for (float, enc) in test_cases {
            let mut de = FogDeserializer::new(&enc);
            let dec = f32::deserialize(&mut de).unwrap();
            de.finish().unwrap();
            assert_eq!(dec, float);
        }
    }

    #[test]
    fn de_f64() {
        let mut test_cases: Vec<(f64, Vec<u8>)> = Vec::new();
        test_cases.push((
                0.0,
                vec![0xcb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        ));
        test_cases.push((
                1.0,
                vec![0xcb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x3f],
        ));
        test_cases.push((
                -1.0,
                vec![0xcb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0xbf],
        ));
        test_cases.push((
                f64::NEG_INFINITY,
                vec![0xcb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0xff],
        ));
        test_cases.push((
                f64::INFINITY,
                vec![0xcb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x7f],
        ));
        for (float, enc) in test_cases {
            let mut de = FogDeserializer::new(&enc);
            let dec = f64::deserialize(&mut de).unwrap();
            de.finish().unwrap();
            assert_eq!(dec, float);
        }
    }

    #[test]
    fn de_bin() {
        let mut test_cases: Vec<(usize, Vec<u8>)> = Vec::new();
        test_cases.push((0, vec![0xc4, 0x00]));
        test_cases.push((1, vec![0xc4, 0x01, 0x00]));
        let mut case = vec![0xc4, 0xff];
        case.resize(255 + 2, 0u8);
        test_cases.push((255, case));
        let mut case = vec![0xc5, 0xff, 0xff];
        case.resize(65535 + 3, 0u8);
        test_cases.push((65535, case));
        let mut case = vec![0xc6, 0x00, 0x00, 0x01];
        case.resize(65536 + 4, 0u8);
        test_cases.push((65536, case));

        use serde_bytes::ByteBuf;
        for (len, enc) in test_cases {
            let mut de = FogDeserializer::new(&enc);
            let dec = ByteBuf::deserialize(&mut de).unwrap();
            de.finish().unwrap();
            assert_eq!(dec.len(), len);
            assert!(dec.iter().all(|byte| *byte == 0));
        }
    }

    #[test]
    fn de_str() {
        let mut test_cases: Vec<(usize, Vec<u8>)> = Vec::new();
        test_cases.push((0, vec![0xa0]));
        test_cases.push((1, vec![0xa1, 0x00]));
        let mut case = vec![0xbf];
        case.resize(32, 0u8);
        test_cases.push((31, case));
        let mut case = vec![0xd4, 0xff];
        case.resize(255 + 2, 0u8);
        test_cases.push((255, case));
        let mut case = vec![0xd5, 0xff, 0xff];
        case.resize(65535 + 3, 0u8);
        test_cases.push((65535, case));
        let mut case = vec![0xd6, 0x00, 0x00, 0x01];
        case.resize(65536 + 4, 0u8);
        test_cases.push((65536, case));

        for (len, enc) in test_cases {
            let mut de = FogDeserializer::new(&enc);
            let dec = String::deserialize(&mut de).unwrap();
            de.finish().unwrap();
            assert_eq!(dec.len(), len);
            assert!(dec.as_bytes().iter().all(|byte| *byte == 0));
        }
    }

    #[test]
    fn de_char() {
        let data = vec![0xa1, 'c' as u8];
        let mut de = FogDeserializer::new(&data);
        let dec = char::deserialize(&mut de).unwrap();
        de.finish().unwrap();
        assert_eq!(dec, 'c');

        let data = vec![0xa1, '0' as u8];
        let mut de = FogDeserializer::new(&data);
        let dec = char::deserialize(&mut de).unwrap();
        de.finish().unwrap();
        assert_eq!(dec, '0');
    }

    #[test]
    fn de_time() {
        use crate::Timestamp;
        let mut test_cases = Vec::new();
        // Zero
        let mut expected = vec![0xc7, 0x05, 0x00, 0x00];
        expected.extend_from_slice(&0u32.to_le_bytes());
        test_cases.push((Timestamp::zero(), expected));
        // Min
        let mut expected = vec![0xc7, 0x09, 0x00, 0x00];
        expected.extend_from_slice(&i64::MIN.to_le_bytes());
        test_cases.push((Timestamp::min_value(), expected));
        // Max
        let mut expected = vec![0xc7, 0x0d, 0x00, 0x00];
        expected.extend_from_slice(&i64::MAX.to_le_bytes());
        expected.extend_from_slice(&1_999_999_999u32.to_le_bytes());
        test_cases.push((Timestamp::max_value(), expected));
        // Start of year 2020
        let mut expected = vec![0xc7, 0x05, 0x00, 0x00];
        expected.extend_from_slice(&1577854800u32.to_le_bytes());
        test_cases.push((Timestamp::from_sec(1577854800), expected));

        for (time, enc) in test_cases {
            let mut de = FogDeserializer::new(&enc);
            let dec = Timestamp::deserialize(&mut de).unwrap();
            de.finish().unwrap();
            assert_eq!(dec, time);
        }
    }

    #[test]
    fn de_hash() {
        use crate::Hash;
        let hash = Hash::new("I am down with deserializing");
        let mut enc = vec![0xc7, 0x21, 0x01];
        enc.extend_from_slice(hash.as_ref());
        let mut de = FogDeserializer::new(&enc);
        let dec= Hash::deserialize(&mut de).unwrap();
        de.finish().unwrap();
        assert_eq!(dec, hash);
    }

}



