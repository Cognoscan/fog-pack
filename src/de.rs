//! Deserialization.
//!
//!

use std::fmt;

use fog_crypto::serde::FOG_TYPE_ENUM;
use serde::de::Error as DeError;
use serde::de::*;

use crate::depth_tracking::DepthTracker;
use crate::{
    element::*,
    error::{Error, Result},
    get_int_internal,
    integer::IntPriv,
};

struct FogDeserializer<'a> {
    depth_tracking: DepthTracker,
    parser: Parser<'a>,
}

impl<'a> FogDeserializer<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self {
            depth_tracking: DepthTracker::new(),
            parser: Parser::new(buf),
        }
    }

    fn next_elem(&mut self) -> Result<Element<'a>> {
        let elem = self
            .parser
            .next()
            .ok_or_else(|| Error::SerdeFail("missing next value".to_string()))??;
        self.depth_tracking.update_elem(&elem)?;
        Ok(elem)
    }
}

impl<'de, 'a> serde::Deserializer<'de> for &'a mut FogDeserializer<'de> {
    type Error = Error;

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
            Marker::FixStr(_) | Marker::Str8 | Marker::Str16 | Marker::Str32 => {
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
