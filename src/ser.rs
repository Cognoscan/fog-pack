//! Serialization.
//!
//!
//! Enum variants, when mapped, are:
//! - Unit - Just the variant name as a string
//! - Newtype - Map with one pair. Key is variant name, content is the value
//! - Tuple - Map with one pair. Key is variant name, content is the tuple as an array
//! - Struct - Map with one pair. Key is variant name, content is the struct

use fog_crypto::serde::FOG_TYPE_ENUM;
use serde::ser::*;
use std::{collections::BTreeMap, convert::TryFrom, mem};

use crate::element::*;
use crate::marker::ExtType;

use crate::error::{Error, Result};

use crate::depth_tracking::DepthTracker;

struct FogSerializer {
    must_be_ordered: bool,
    depth_tracking: DepthTracker,
    buf: Vec<u8>,
}

impl Default for FogSerializer {
    fn default() -> Self {
        Self::with_params(false)
    }
}

impl FogSerializer {
    fn with_params(must_be_ordered: bool) -> Self {
        FogSerializer {
            must_be_ordered,
            depth_tracking: DepthTracker::new(),
            buf: Vec::new(),
        }
    }

    fn encode_element(&mut self, elem: Element) -> Result<()> {
        self.depth_tracking.update_elem(&elem)?;
        serialize_elem(&mut self.buf, elem);
        Ok(())
    }
}

impl<'a> Serializer for &'a mut FogSerializer {
    type Ok = ();
    type Error = crate::error::Error;
    type SerializeSeq = SeqSerializer<'a>;
    type SerializeTuple = TupleSerializer<'a>;
    type SerializeTupleStruct = TupleSerializer<'a>;
    type SerializeTupleVariant = TupleSerializer<'a>;
    type SerializeMap = MapSerializer<'a>;
    type SerializeStruct = StructSerializer<'a>;
    type SerializeStructVariant = StructSerializer<'a>;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.encode_element(Element::Bool(v))
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        self.encode_element(Element::Int(crate::Integer::from(v)))
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        self.encode_element(Element::Int(crate::Integer::from(v)))
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        self.encode_element(Element::F32(v))
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        self.encode_element(Element::F64(v))
    }

    fn serialize_char(self, v: char) -> Result<()> {
        self.encode_element(Element::Str(&v.to_string()))
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        self.encode_element(Element::Str(v))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.encode_element(Element::Bin(v))
    }

    fn serialize_none(self) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_some<T: Serialize + ?Sized>(self, v: &T) -> Result<()> {
        v.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        self.encode_element(Element::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        v: &T,
    ) -> Result<()> {
        v.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        mut self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<()> {
        if name == FOG_TYPE_ENUM {
            let index = u8::try_from(variant_index)
                .map_err(|_| Error::SerdeFail("unrecognized FogPack variant".to_string()))?;
            let ext = ExtType::from_u8(index)
                .ok_or_else(|| Error::SerdeFail("unrecognized FogPack variant".to_string()))?;
            let mut ext_se = ExtSerializer::new(ext, &mut self);
            value.serialize(&mut ext_se)
        } else {
            self.encode_element(Element::Map(1))?;
            self.encode_element(Element::Str(variant))?;
            value.serialize(self)
        }
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        SeqSerializer::new(self, len)
    }

    fn serialize_tuple(self, len: usize) -> Result<TupleSerializer<'a>> {
        self.encode_element(Element::Array(len))?;
        Ok(TupleSerializer::new(self))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<TupleSerializer<'a>> {
        // Tuple structs usually just discard the name
        self.encode_element(Element::Array(len))?;
        Ok(TupleSerializer::new(self))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        self.encode_element(Element::Map(1))?;
        self.encode_element(Element::Str(variant))?;
        self.encode_element(Element::Array(len))?;
        Ok(TupleSerializer::new(self))
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        MapSerializer::new(self, len)
    }

    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        self.encode_element(Element::Map(len))?;
        Ok(StructSerializer::new(self))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.encode_element(Element::Map(1))?;
        self.encode_element(Element::Str(variant))?;
        self.encode_element(Element::Map(len))?;
        Ok(StructSerializer::new(self))
    }

    fn collect_seq<I>(self, iter: I) -> Result<()>
    where
        I: IntoIterator,
        <I as IntoIterator>::Item: Serialize,
    {
        let iter = iter.into_iter();
        match iter.size_hint() {
            (lo, Some(hi)) if lo == hi => {
                let mut tuple_ser = self.serialize_tuple(lo)?;
                for item in iter {
                    tuple_ser.serialize_element(&item)?;
                }
                Ok(())
            }
            (lo, _) => {
                let mut v: Vec<I::Item> = Vec::with_capacity(lo);
                for item in iter {
                    v.push(item);
                }
                let mut tuple_ser = self.serialize_tuple(v.len())?;
                for item in v.iter() {
                    tuple_ser.serialize_element(&item)?;
                }
                Ok(())
            }
        }
    }

    fn collect_map<K, V, I>(self, iter: I) -> Result<()>
    where
        K: Serialize,
        V: Serialize,
        I: IntoIterator<Item = (K, V)>,
    {
        let iter = iter.into_iter();
        let len = match iter.size_hint() {
            (lo, Some(hi)) if lo == hi => Some(lo),
            _ => None,
        };
        if let Some(len) = len {
            self.encode_element(Element::Map(len))?;
            if self.must_be_ordered {
                // Sized & Ordered
                let mut last_key = None;
                let mut new_key = String::new();
                for (k, v) in iter {
                    k.serialize(KeySerializer::new(&mut new_key))?;
                    self.encode_element(Element::Str(&new_key))?;
                    if let Some(ref mut last_key) = last_key {
                        if new_key <= *last_key {
                            return Err(Error::SerdeFail(format!(
                                "map keys are unordered: {} follows {}",
                                new_key, last_key
                            )));
                        }
                        mem::swap(&mut new_key, &mut *last_key);
                    } else {
                        last_key = Some(mem::replace(&mut new_key, String::new()));
                    }
                    v.serialize(&mut *self)?;
                }
            } else {
                // Sized & Unordered
                let mut map = BTreeMap::new();
                // Collect into ordered map
                for (k, v) in iter {
                    let mut key = String::new();
                    k.serialize(KeySerializer::new(&mut key))?;
                    map.insert(key, v);
                }
                // Serialize in order
                for (k, v) in map.iter() {
                    self.encode_element(Element::Str(k))?;
                    v.serialize(&mut *self)?;
                }
            }
        } else {
            if self.must_be_ordered {
                // Unsized & Ordered
                let mut map = Vec::with_capacity(iter.size_hint().0);
                for (k, v) in iter {
                    let mut key = String::new();
                    k.serialize(KeySerializer::new(&mut key))?;
                    if let Some((last_key, _)) = map.last() {
                        if key <= *last_key {
                            return Err(Error::SerdeFail(format!(
                                "map keys are unordered: {} follows {}",
                                key, last_key
                            )));
                        }
                    }
                    map.push((key, v));
                }
                self.encode_element(Element::Map(map.len()))?;
                for (k, v) in map.iter() {
                    self.encode_element(Element::Str(k))?;
                    v.serialize(&mut *self)?;
                }
            } else {
                // Unsized & Unordered
                let mut map = BTreeMap::new();
                // Collect into ordered map
                for (k, v) in iter {
                    let mut key = String::new();
                    k.serialize(KeySerializer::new(&mut key))?;
                    map.insert(key, v);
                }
                // Serialize in order
                self.encode_element(Element::Map(map.len()))?;
                for (k, v) in map.iter() {
                    self.encode_element(Element::Str(k))?;
                    v.serialize(&mut *self)?;
                }
            }
        }
        Ok(())
    }
}

/// Encode a sequence of possibly unknown length.
///
/// If the length is known, this is easy and looks pretty much like the TupleSerializer. However,
/// if the length is unknown, we can't encode the array marker ahead of time. So instead, we:
///
/// 1. Swap in a temporary buffer into the FogSerializer
/// 2. Update the depth tracker with a placeholder Array element
/// 3. Serialize elements using the FogSerializer instance, which also updates the depth tracker
/// 4. Repeat 3 until finishing with end()
/// 5. Swap the original buffer back into FogSerializer
/// 6. Directly encode the actual array element, skipping the depth tracker
/// 7. Copy over the entire temporary buffer
/// 8. Update the depth tracker by dropping the placeholder element
///
/// This is about the best we can do for unknown length sequences, unless you can call collect_seq
/// instead, in which case we can avoid temporarily encoding to a buffer.
struct SeqSerializer<'a> {
    se: &'a mut FogSerializer,
    unknown_len: Option<(usize, Vec<u8>)>,
}

impl<'a> SeqSerializer<'a> {
    fn new(se: &'a mut FogSerializer, len: Option<usize>) -> Result<Self> {
        if let Some(len) = len {
            se.encode_element(Element::Array(len))?;
            Ok(Self {
                se,
                unknown_len: None,
            })
        } else {
            se.depth_tracking
                .update_elem(&Element::Array(u32::MAX as usize))?;
            let enc = mem::replace(&mut se.buf, Vec::new());
            Ok(Self {
                se,
                unknown_len: Some((0, enc)),
            })
        }
    }
}

impl<'a> SerializeSeq for SeqSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        if let Some((ref mut len, _)) = self.unknown_len {
            *len += 1;
        }
        value.serialize(&mut *self.se)
    }

    fn end(self) -> Result<()> {
        if let Some((len, enc)) = self.unknown_len {
            let enc = mem::replace(&mut self.se.buf, enc);
            serialize_elem(&mut self.se.buf, Element::Array(len));
            self.se.buf.extend_from_slice(&enc);
            self.se.depth_tracking.early_end();
            Ok(())
        } else {
            Ok(())
        }
    }
}

struct TupleSerializer<'a> {
    se: &'a mut FogSerializer,
}

impl<'a> TupleSerializer<'a> {
    fn new(se: &'a mut FogSerializer) -> Self {
        Self { se }
    }
}

impl<'a> SerializeTuple for TupleSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        value.serialize(&mut *self.se)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a> SerializeTupleStruct for TupleSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        value.serialize(&mut *self.se)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a> SerializeTupleVariant for TupleSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        value.serialize(&mut *self.se)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

enum MapSerializer<'a> {
    SizedOrdered {
        se: &'a mut FogSerializer,
        last_key: Option<String>,
        new_key: String,
    },
    SizedUnordered {
        se: &'a mut FogSerializer,
        map: BTreeMap<String, Vec<u8>>,
        pending_key: String,
    },
    UnsizedOrdered {
        se: &'a mut FogSerializer,
        last_key: Option<String>,
        new_key: String,
        len: usize,
        buf: Vec<u8>,
    },
    UnsizedUnordered {
        se: &'a mut FogSerializer,
        map: BTreeMap<String, Vec<u8>>,
        pending_key: String,
    },
}

impl<'a> MapSerializer<'a> {
    fn new(se: &'a mut FogSerializer, len: Option<usize>) -> Result<Self> {
        if let Some(len) = len {
            se.encode_element(Element::Map(len))?;
            Ok(if se.must_be_ordered {
                MapSerializer::SizedOrdered {
                    se,
                    last_key: None,
                    new_key: String::new(),
                }
            } else {
                MapSerializer::SizedUnordered {
                    se,
                    map: BTreeMap::new(),
                    pending_key: String::new(),
                }
            })
        } else {
            se.depth_tracking
                .update_elem(&Element::Map(u32::MAX as usize))?;
            if se.must_be_ordered {
                let buf = mem::replace(&mut se.buf, Vec::new());
                Ok(MapSerializer::UnsizedOrdered {
                    se,
                    last_key: None,
                    new_key: String::new(),
                    len: 0,
                    buf,
                })
            } else {
                Ok(MapSerializer::UnsizedUnordered {
                    se,
                    map: BTreeMap::new(),
                    pending_key: String::new(),
                })
            }
        }
    }
}

impl<'a> SerializeMap for MapSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        match self {
            MapSerializer::SizedOrdered {
                se,
                new_key,
                last_key,
            } => {
                // Turn the key into a String or fail (this clears out the string before
                // serializing)
                value.serialize(KeySerializer::new(new_key))?;
                // Immediately serialize, while our string is unwrapped
                se.encode_element(Element::Str(new_key))?;
                // Verify the Strings are correctly ordered & move to last_key
                if let Some(last_key) = last_key {
                    if new_key <= last_key {
                        return Err(Error::SerdeFail(format!(
                            "map keys are unordered: {} follows {}",
                            new_key, last_key
                        )));
                    }
                    mem::swap(new_key, last_key);
                } else {
                    // Replace new_key with a new string, and load the last key into memory
                    *last_key = Some(mem::replace(new_key, String::new()));
                }
            }
            MapSerializer::SizedUnordered { pending_key, .. } => {
                value.serialize(KeySerializer::new(pending_key))?;
            }
            MapSerializer::UnsizedOrdered {
                se,
                last_key,
                new_key,
                len,
                ..
            } => {
                *len += 1;
                value.serialize(KeySerializer::new(new_key))?;
                se.encode_element(Element::Str(new_key))?;
                if let Some(last_key) = last_key {
                    if new_key <= last_key {
                        return Err(Error::SerdeFail(format!(
                            "map keys are unordered: {} follows {}",
                            new_key, last_key
                        )));
                    }
                    mem::swap(new_key, last_key);
                } else {
                    // Replace new_key with a new string, and load the last key into memory
                    *last_key = Some(mem::replace(new_key, String::new()));
                }
            }
            MapSerializer::UnsizedUnordered { pending_key, .. } => {
                value.serialize(KeySerializer::new(pending_key))?;
            }
        }
        Ok(())
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        match self {
            MapSerializer::SizedOrdered { se, .. } => {
                value.serialize(&mut **se)?;
            }
            MapSerializer::SizedUnordered {
                se,
                map,
                pending_key,
            } => {
                // Slot in buffer, fill it like we're writing to the actual buffer, then store it
                // off for later reordering
                let buf = mem::replace(&mut se.buf, Vec::new());
                se.encode_element(Element::Str(pending_key))?;
                value.serialize(&mut **se)?;
                // Replace buffers & store off in BTreeMap
                let buf = mem::replace(&mut se.buf, buf);
                let key = mem::replace(pending_key, String::new());
                map.insert(key, buf);
            }
            MapSerializer::UnsizedOrdered { se, .. } => {
                value.serialize(&mut **se)?;
            }
            MapSerializer::UnsizedUnordered {
                se,
                map,
                pending_key,
            } => {
                // Slot in buffer, fill it like we're writing to the actual buffer, then store it
                // off for later reordering
                let buf = mem::replace(&mut se.buf, Vec::new());
                se.encode_element(Element::Str(pending_key))?;
                value.serialize(&mut **se)?;
                // Replace buffers & store off in BTreeMap
                let buf = mem::replace(&mut se.buf, buf);
                let key = mem::replace(pending_key, String::new());
                map.insert(key, buf);
            }
        }
        Ok(())
    }

    fn end(self) -> Result<()> {
        match self {
            MapSerializer::SizedOrdered { .. } => (),
            MapSerializer::SizedUnordered { se, map, .. } => {
                // Flush all buffers, in order, out to the main one
                for (_, vec) in map.iter() {
                    se.buf.extend_from_slice(&vec);
                }
            }
            MapSerializer::UnsizedOrdered { se, len, buf, .. } => {
                // The serializer has our temporary buffer. Swap back, put in the real Map marker,
                // and extend
                let enc = mem::replace(&mut se.buf, buf);
                serialize_elem(&mut se.buf, Element::Map(len));
                se.buf.extend_from_slice(&enc);
                se.depth_tracking.early_end();
            }
            MapSerializer::UnsizedUnordered { se, map, .. } => {
                // Fill in the real map marker, update depth tracking, and
                // flush all buffers, in order, out to the main one
                serialize_elem(&mut se.buf, Element::Map(map.len()));
                for (_, vec) in map.iter() {
                    se.buf.extend_from_slice(&vec);
                }
                se.depth_tracking.early_end();
            }
        }
        Ok(())
    }
}

enum StructSerializer<'a> {
    Ordered {
        se: &'a mut FogSerializer,
        last_key: Option<&'static str>,
    },
    Unordered {
        se: &'a mut FogSerializer,
        map: BTreeMap<&'static str, Vec<u8>>,
    },
}

impl<'a> StructSerializer<'a> {
    fn new(se: &'a mut FogSerializer) -> Self {
        if se.must_be_ordered {
            StructSerializer::Ordered { se, last_key: None }
        } else {
            StructSerializer::Unordered {
                se,
                map: BTreeMap::new(),
            }
        }
    }

    fn serialize_field_inner<T: Serialize + ?Sized>(
        &mut self,
        field: &'static str,
        value: &T,
    ) -> Result<()> {
        match self {
            StructSerializer::Ordered { se, last_key } => {
                if let Some(last_key) = last_key {
                    if field <= last_key {
                        return Err(Error::SerdeFail(format!(
                            "map keys are unordered: {} follows {}",
                            field, last_key
                        )));
                    }
                    *last_key = field;
                } else {
                    *last_key = Some(field);
                }
                se.encode_element(Element::Str(field))?;
                value.serialize(&mut **se)?;
            }
            StructSerializer::Unordered { se, map } => {
                // Slot in buffer, fill it like we're writing to the actual buffer, then store it
                // off for later reordering
                let buf = mem::replace(&mut se.buf, Vec::new());
                se.encode_element(Element::Str(field))?;
                value.serialize(&mut **se)?;
                // Replace buffers & store off in BTreeMap
                let buf = mem::replace(&mut se.buf, buf);
                map.insert(field, buf);
            }
        }
        Ok(())
    }

    fn end_inner(self) -> Result<()> {
        match self {
            StructSerializer::Ordered { .. } => (),
            StructSerializer::Unordered { se, map } => {
                for (_, vec) in map.iter() {
                    se.buf.extend_from_slice(&vec);
                }
            }
        }
        Ok(())
    }
}

impl<'a> SerializeStruct for StructSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        field: &'static str,
        value: &T,
    ) -> Result<()> {
        self.serialize_field_inner(field, value)
    }

    fn end(self) -> Result<()> {
        self.end_inner()
    }
}

impl<'a> SerializeStructVariant for StructSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        field: &'static str,
        value: &T,
    ) -> Result<()> {
        self.serialize_field_inner(field, value)
    }

    fn end(self) -> Result<()> {
        self.end_inner()
    }
}

struct ExtSerializer<'a> {
    ext: ExtType,
    received: bool,
    se: &'a mut FogSerializer,
}

impl<'a> ExtSerializer<'a> {
    fn new(ext: ExtType, se: &'a mut FogSerializer) -> Self {
        Self {
            ext,
            received: false,
            se,
        }
    }

    fn ser_fail(&self, received: &'static str) -> Error {
        let s = format!("expected bytes, received {}", received);
        Error::SerdeFail(s)
    }
}

impl<'a> Serializer for &mut ExtSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        if !self.received {
            self.received = true;
            let elem = match self.ext {
                ExtType::Timestamp => {
                    let v = crate::Timestamp::try_from(v).map_err(|_| {
                        Error::SerdeFail("Timestamp bytes weren't valid on encode".to_string())
                    })?;
                    Element::Timestamp(v)
                }
                ExtType::Hash => {
                    let v = fog_crypto::hash::Hash::try_from(v).map_err(|_| {
                        Error::SerdeFail("Hash bytes weren't valid on encode".to_string())
                    })?;
                    Element::Hash(v)
                }
                ExtType::Identity => {
                    let v = fog_crypto::identity::Identity::try_from(v).map_err(|_| {
                        Error::SerdeFail("Identity bytes weren't valid on encode".to_string())
                    })?;
                    Element::Identity(v)
                }
                ExtType::LockId => {
                    let v = fog_crypto::lock::LockId::try_from(v).map_err(|_| {
                        Error::SerdeFail("LockId bytes weren't valid on encode".to_string())
                    })?;
                    Element::LockId(v)
                }
                ExtType::StreamId => {
                    let v = fog_crypto::stream::StreamId::try_from(v).map_err(|_| {
                        Error::SerdeFail("StreamId bytes weren't valid on encode".to_string())
                    })?;
                    Element::StreamId(v)
                }
                ExtType::DataLockbox => {
                    let v = fog_crypto::lockbox::DataLockboxRef::from_bytes(v).map_err(|_| {
                        Error::SerdeFail("DataLockbox bytes weren't valid on encode".to_string())
                    })?;
                    Element::DataLockbox(v)
                }
                ExtType::IdentityLockbox => {
                    let v =
                        fog_crypto::lockbox::IdentityLockboxRef::from_bytes(v).map_err(|_| {
                            Error::SerdeFail(
                                "IdentityLockbox bytes weren't valid on encode".to_string(),
                            )
                        })?;
                    Element::IdentityLockbox(v)
                }
                ExtType::StreamLockbox => {
                    let v = fog_crypto::lockbox::StreamLockboxRef::from_bytes(v).map_err(|_| {
                        Error::SerdeFail("StreamLockbox bytes weren't valid on encode".to_string())
                    })?;
                    Element::StreamLockbox(v)
                }
                ExtType::LockLockbox => {
                    let v = fog_crypto::lockbox::LockLockboxRef::from_bytes(v).map_err(|_| {
                        Error::SerdeFail("LockLockbox bytes weren't valid on encode".to_string())
                    })?;
                    Element::LockLockbox(v)
                }
            };
            self.se.encode_element(elem)
        } else {
            Err(self.ser_fail("a second byte sequence"))
        }
    }

    type SerializeSeq = Impossible<(), Error>;
    type SerializeTuple = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Impossible<(), Error>;
    type SerializeStruct = Impossible<(), Error>;
    type SerializeStructVariant = Impossible<(), Error>;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn serialize_bool(self, _: bool) -> Result<()> {
        Err(self.ser_fail("bool"))
    }

    fn serialize_i8(self, _: i8) -> Result<()> {
        Err(self.ser_fail("i8"))
    }

    fn serialize_i16(self, _: i16) -> Result<()> {
        Err(self.ser_fail("i16"))
    }

    fn serialize_i32(self, _: i32) -> Result<()> {
        Err(self.ser_fail("i32"))
    }

    fn serialize_i64(self, _: i64) -> Result<()> {
        Err(self.ser_fail("i64"))
    }

    fn serialize_u8(self, _: u8) -> Result<()> {
        Err(self.ser_fail("u8"))
    }

    fn serialize_u16(self, _: u16) -> Result<()> {
        Err(self.ser_fail("u16"))
    }

    fn serialize_u32(self, _: u32) -> Result<()> {
        Err(self.ser_fail("u32"))
    }

    fn serialize_u64(self, _: u64) -> Result<()> {
        Err(self.ser_fail("u64"))
    }

    fn serialize_f32(self, _: f32) -> Result<()> {
        Err(self.ser_fail("f32"))
    }

    fn serialize_f64(self, _: f64) -> Result<()> {
        Err(self.ser_fail("f64"))
    }

    fn serialize_char(self, _: char) -> Result<()> {
        Err(self.ser_fail("char"))
    }

    fn serialize_str(self, _: &str) -> Result<()> {
        Err(self.ser_fail("str"))
    }

    fn serialize_none(self) -> Result<()> {
        Err(self.ser_fail("None"))
    }

    fn serialize_some<T: Serialize + ?Sized>(self, _: &T) -> Result<()> {
        Err(self.ser_fail("Some"))
    }

    fn serialize_unit(self) -> Result<()> {
        Err(self.ser_fail("unit"))
    }

    fn serialize_unit_struct(self, _: &'static str) -> Result<()> {
        Err(self.ser_fail("unit_struct"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<()> {
        Err(self.ser_fail("unit_variant"))
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _v: &T,
    ) -> Result<()> {
        Err(self.ser_fail("newtype_struct"))
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()> {
        Err(self.ser_fail("newtype_variant"))
    }

    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(self.ser_fail("seq"))
    }

    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple> {
        Err(self.ser_fail("tuple"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(self.ser_fail("tuple_struct"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(self.ser_fail("tuple_variant"))
    }

    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap> {
        Err(self.ser_fail("map"))
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Err(self.ser_fail("struct"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(self.ser_fail("struct_variant"))
    }
}

struct KeySerializer<'a> {
    s: &'a mut String,
}

impl<'a> KeySerializer<'a> {
    fn new(s: &'a mut String) -> Self {
        s.clear();
        Self { s }
    }

    fn ser_fail(&self, received: &'static str) -> Error {
        let s = format!("expected string, received {}", received);
        Error::SerdeFail(s)
    }
}

impl<'a> Serializer for KeySerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_char(self, v: char) -> Result<()> {
        self.s.push(v);
        Ok(())
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        self.s.push_str(v);
        Ok(())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.s.push_str(variant);
        Ok(())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        v: &T,
    ) -> Result<()> {
        v.serialize(self)
    }

    type SerializeSeq = Impossible<(), Error>;
    type SerializeTuple = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Impossible<(), Error>;
    type SerializeStruct = Impossible<(), Error>;
    type SerializeStructVariant = Impossible<(), Error>;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn serialize_bool(self, _: bool) -> Result<()> {
        Err(self.ser_fail("bool"))
    }

    fn serialize_i8(self, _: i8) -> Result<()> {
        Err(self.ser_fail("i8"))
    }

    fn serialize_i16(self, _: i16) -> Result<()> {
        Err(self.ser_fail("i16"))
    }

    fn serialize_i32(self, _: i32) -> Result<()> {
        Err(self.ser_fail("i32"))
    }

    fn serialize_i64(self, _: i64) -> Result<()> {
        Err(self.ser_fail("i64"))
    }

    fn serialize_u8(self, _: u8) -> Result<()> {
        Err(self.ser_fail("u8"))
    }

    fn serialize_u16(self, _: u16) -> Result<()> {
        Err(self.ser_fail("u16"))
    }

    fn serialize_u32(self, _: u32) -> Result<()> {
        Err(self.ser_fail("u32"))
    }

    fn serialize_u64(self, _: u64) -> Result<()> {
        Err(self.ser_fail("u64"))
    }

    fn serialize_f32(self, _: f32) -> Result<()> {
        Err(self.ser_fail("f32"))
    }

    fn serialize_f64(self, _: f64) -> Result<()> {
        Err(self.ser_fail("f64"))
    }

    fn serialize_bytes(self, _: &[u8]) -> Result<()> {
        Err(self.ser_fail("bytes"))
    }

    fn serialize_none(self) -> Result<()> {
        Err(self.ser_fail("None"))
    }

    fn serialize_some<T: Serialize + ?Sized>(self, _: &T) -> Result<()> {
        Err(self.ser_fail("Some"))
    }

    fn serialize_unit(self) -> Result<()> {
        Err(self.ser_fail("unit"))
    }

    fn serialize_unit_struct(self, _: &'static str) -> Result<()> {
        Err(self.ser_fail("unit_struct"))
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()> {
        Err(self.ser_fail("newtype_variant"))
    }

    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(self.ser_fail("seq"))
    }

    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple> {
        Err(self.ser_fail("tuple"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(self.ser_fail("tuple_struct"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(self.ser_fail("tuple_variant"))
    }

    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap> {
        Err(self.ser_fail("map"))
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Err(self.ser_fail("struct"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(self.ser_fail("struct_variant"))
    }
}
