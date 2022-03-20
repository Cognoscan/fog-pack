use crate::value_ref::ValueRef;
use crate::*;
use std::borrow::Cow;
use std::ops::Index;
use std::{collections::BTreeMap, fmt::Debug};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(Integer),
    Str(String),
    F32(f32),
    F64(f64),
    Bin(Vec<u8>),
    Array(Vec<Value>),
    Map(BTreeMap<String, Value>),
    Timestamp(Timestamp),
    Hash(Hash),
    Identity(Identity),
    LockId(LockId),
    StreamId(StreamId),
    DataLockbox(DataLockbox),
    IdentityLockbox(IdentityLockbox),
    StreamLockbox(StreamLockbox),
    LockLockbox(LockLockbox),
}

impl Value {
    pub fn as_ref(&self) -> ValueRef {
        use std::ops::Deref;
        match *self {
            Value::Null => ValueRef::Null,
            Value::Bool(v) => ValueRef::Bool(v),
            Value::Int(v) => ValueRef::Int(v),
            Value::Str(ref v) => ValueRef::Str(v.as_ref()),
            Value::F32(v) => ValueRef::F32(v),
            Value::F64(v) => ValueRef::F64(v),
            Value::Bin(ref v) => ValueRef::Bin(v.as_slice()),
            Value::Array(ref v) => ValueRef::Array(v.iter().map(|i| i.as_ref()).collect()),
            Value::Map(ref v) => ValueRef::Map(
                v.iter()
                    .map(|(f, i)| (f.as_ref(), i.as_ref()))
                    .collect(),
            ),
            Value::Timestamp(v) => ValueRef::Timestamp(v),
            Value::Hash(ref v) => ValueRef::Hash(v.clone()),
            Value::Identity(ref v) => ValueRef::Identity(v.clone()),
            Value::StreamId(ref v) => ValueRef::StreamId(v.clone()),
            Value::LockId(ref v) => ValueRef::LockId(v.clone()),
            Value::DataLockbox(ref v) => ValueRef::DataLockbox(v.deref()),
            Value::IdentityLockbox(ref v) => ValueRef::IdentityLockbox(v.deref()),
            Value::StreamLockbox(ref v) => ValueRef::StreamLockbox(v.deref()),
            Value::LockLockbox(ref v) => ValueRef::LockLockbox(v.deref()),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Value::Bool(_))
    }

    pub fn is_int(&self) -> bool {
        matches!(self, Value::Int(_))
    }

    pub fn is_i64(&self) -> bool {
        if let Value::Int(ref v) = *self {
            v.is_i64()
        } else {
            false
        }
    }

    pub fn is_u64(&self) -> bool {
        if let Value::Int(ref v) = *self {
            v.is_u64()
        } else {
            false
        }
    }

    pub fn is_f32(&self) -> bool {
        matches!(self, Value::F32(_))
    }

    pub fn is_f64(&self) -> bool {
        matches!(self, Value::F64(_))
    }

    pub fn is_str(&self) -> bool {
        matches!(self, Value::Str(_))
    }

    pub fn is_bin(&self) -> bool {
        matches!(self, Value::Bin(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Value::Array(_))
    }

    pub fn is_map(&self) -> bool {
        matches!(self, Value::Map(_))
    }

    pub fn is_timestamp(&self) -> bool {
        matches!(self, Value::Timestamp(_))
    }

    pub fn is_hash(&self) -> bool {
        matches!(self, Value::Hash(_))
    }

    pub fn is_identity(&self) -> bool {
        matches!(self, Value::Identity(_))
    }

    pub fn is_stream_id(&self) -> bool {
        matches!(self, Value::StreamId(_))
    }

    pub fn is_lock_id(&self) -> bool {
        matches!(self, Value::LockId(_))
    }

    pub fn is_lockbox(&self) -> bool {
        matches!(
            self,
            Value::DataLockbox(_)
                | Value::IdentityLockbox(_)
                | Value::StreamLockbox(_)
                | Value::LockLockbox(_)
        )
    }

    pub fn is_data_lockbox(&self) -> bool {
        matches!(self, Value::DataLockbox(_))
    }

    pub fn is_identity_lockbox(&self) -> bool {
        matches!(self, Value::IdentityLockbox(_))
    }

    pub fn is_stream_lockbox(&self) -> bool {
        matches!(self, Value::StreamLockbox(_))
    }

    pub fn is_lock_lockbox(&self) -> bool {
        matches!(self, Value::LockLockbox(_))
    }

    pub fn as_bool(&self) -> Option<bool> {
        if let Value::Bool(val) = *self {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_int(&self) -> Option<Integer> {
        if let Value::Int(val) = *self {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Value::Int(ref n) => n.as_i64(),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match *self {
            Value::Int(ref n) => n.as_u64(),
            _ => None,
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        match *self {
            Value::F32(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Value::F64(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_floating(&self) -> Option<f64> {
        match *self {
            Value::F32(n) => Some(n.into()),
            Value::F64(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let Value::Str(ref val) = *self {
            Some(val.as_str())
        } else {
            None
        }
    }

    pub fn as_string(&self) -> Option<&String> {
        if let Value::Str(ref val) = *self {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_slice(&self) -> Option<&[u8]> {
        if let Value::Bin(ref val) = *self {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        if let Value::Array(ref array) = *self {
            Some(&*array)
        } else {
            None
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut [Value]> {
        match *self {
            Value::Array(ref mut array) => Some(array),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&BTreeMap<String, Value>> {
        if let Value::Map(ref map) = *self {
            Some(map)
        } else {
            None
        }
    }

    pub fn as_map_mut(&mut self) -> Option<&mut BTreeMap<String, Value>> {
        match *self {
            Value::Map(ref mut map) => Some(map),
            _ => None,
        }
    }

    pub fn as_timestamp(&self) -> Option<Timestamp> {
        if let Value::Timestamp(time) = *self {
            Some(time)
        } else {
            None
        }
    }

    pub fn as_hash(&self) -> Option<&Hash> {
        if let Value::Hash(ref hash) = *self {
            Some(hash)
        } else {
            None
        }
    }

    pub fn as_identity(&self) -> Option<&Identity> {
        if let Value::Identity(ref id) = *self {
            Some(id)
        } else {
            None
        }
    }

    pub fn as_stream_id(&self) -> Option<&StreamId> {
        if let Value::StreamId(ref id) = *self {
            Some(id)
        } else {
            None
        }
    }

    pub fn as_lock_id(&self) -> Option<&LockId> {
        if let Value::LockId(ref id) = *self {
            Some(id)
        } else {
            None
        }
    }

    pub fn as_data_lockbox(&self) -> Option<&DataLockboxRef> {
        if let Value::DataLockbox(ref lockbox) = *self {
            Some(lockbox)
        } else {
            None
        }
    }

    pub fn as_identity_lockbox(&self) -> Option<&IdentityLockboxRef> {
        if let Value::IdentityLockbox(ref lockbox) = *self {
            Some(lockbox)
        } else {
            None
        }
    }

    pub fn as_stream_lockbox(&self) -> Option<&StreamLockboxRef> {
        if let Value::StreamLockbox(ref lockbox) = *self {
            Some(lockbox)
        } else {
            None
        }
    }

    pub fn as_lock_lockbox(&self) -> Option<&LockLockboxRef> {
        if let Value::LockLockbox(ref lockbox) = *self {
            Some(lockbox)
        } else {
            None
        }
    }
}

impl std::default::Default for Value {
    fn default() -> Self {
        Value::Null
    }
}

static NULL: Value = Value::Null;

impl Index<usize> for Value {
    type Output = Value;

    fn index(&self, index: usize) -> &Self::Output {
        self.as_array().and_then(|v| v.get(index)).unwrap_or(&NULL)
    }
}

impl Index<&str> for Value {
    type Output = Value;

    fn index(&self, index: &str) -> &Self::Output {
        self.as_map().and_then(|v| v.get(index)).unwrap_or(&NULL)
    }
}

impl<'a> PartialEq<ValueRef<'a>> for Value {
    fn eq(&self, other: &ValueRef) -> bool {
        use std::ops::Deref;
        match self {
            Value::Null => other == &ValueRef::Null,
            Value::Bool(s) => {
                if let ValueRef::Bool(o) = other {
                    s == o
                } else {
                    false
                }
            }
            Value::Int(s) => {
                if let ValueRef::Int(o) = other {
                    s == o
                } else {
                    false
                }
            }
            Value::Str(s) => {
                if let ValueRef::Str(o) = other {
                    s == o
                } else {
                    false
                }
            }
            Value::F32(s) => {
                if let ValueRef::F32(o) = other {
                    s == o
                } else {
                    false
                }
            }
            Value::F64(s) => {
                if let ValueRef::F64(o) = other {
                    s == o
                } else {
                    false
                }
            }
            Value::Bin(s) => {
                if let ValueRef::Bin(o) = other {
                    s == o
                } else {
                    false
                }
            }
            Value::Array(s) => {
                if let ValueRef::Array(o) = other {
                    s == o
                } else {
                    false
                }
            }
            Value::Map(s) => {
                if let ValueRef::Map(o) = other {
                    s.len() == o.len()
                        && s.iter()
                            .zip(o)
                            .all(|((ks, vs), (ko, vo))| (ks == ko) && (vs == vo))
                } else {
                    false
                }
            }
            Value::Hash(s) => {
                if let ValueRef::Hash(o) = other {
                    s == o
                } else {
                    false
                }
            }
            Value::Identity(s) => {
                if let ValueRef::Identity(o) = other {
                    s == o
                } else {
                    false
                }
            }
            Value::StreamId(s) => {
                if let ValueRef::StreamId(o) = other {
                    s == o
                } else {
                    false
                }
            }
            Value::LockId(s) => {
                if let ValueRef::LockId(o) = other {
                    s == o
                } else {
                    false
                }
            }
            Value::Timestamp(s) => {
                if let ValueRef::Timestamp(o) = other {
                    s == o
                } else {
                    false
                }
            }
            Value::DataLockbox(s) => {
                if let ValueRef::DataLockbox(o) = other {
                    o == &s.deref()
                } else {
                    false
                }
            }
            Value::IdentityLockbox(s) => {
                if let ValueRef::IdentityLockbox(o) = other {
                    o == &s.deref()
                } else {
                    false
                }
            }
            Value::StreamLockbox(s) => {
                if let ValueRef::StreamLockbox(o) = other {
                    o == &s.deref()
                } else {
                    false
                }
            }
            Value::LockLockbox(s) => {
                if let ValueRef::LockLockbox(o) = other {
                    o == &s.deref()
                } else {
                    false
                }
            }
        }
    }
}

macro_rules! impl_value_from_integer {
    ($t: ty) => {
        impl From<$t> for Value {
            fn from(v: $t) -> Self {
                Value::Int(From::from(v))
            }
        }
    };
}

macro_rules! impl_value_from {
    ($t: ty, $p: ident) => {
        impl From<$t> for Value {
            fn from(v: $t) -> Self {
                Value::$p(v)
            }
        }
    };
}

impl_value_from!(bool, Bool);
impl_value_from!(Integer, Int);
impl_value_from!(f32, F32);
impl_value_from!(f64, F64);
impl_value_from!(String, Str);
impl_value_from!(Vec<u8>, Bin);
impl_value_from!(Vec<Value>, Array);
impl_value_from!(BTreeMap<String, Value>, Map);
impl_value_from!(Timestamp, Timestamp);
impl_value_from!(Hash, Hash);
impl_value_from!(Identity, Identity);
impl_value_from!(StreamId, StreamId);
impl_value_from!(LockId, LockId);
impl_value_from!(DataLockbox, DataLockbox);
impl_value_from!(IdentityLockbox, IdentityLockbox);
impl_value_from!(StreamLockbox, StreamLockbox);
impl_value_from!(LockLockbox, LockLockbox);
impl_value_from_integer!(u8);
impl_value_from_integer!(u16);
impl_value_from_integer!(u32);
impl_value_from_integer!(u64);
impl_value_from_integer!(usize);
impl_value_from_integer!(i8);
impl_value_from_integer!(i16);
impl_value_from_integer!(i32);
impl_value_from_integer!(i64);
impl_value_from_integer!(isize);

impl From<()> for Value {
    fn from((): ()) -> Self {
        Value::Null
    }
}

impl<'a> From<&'a str> for Value {
    fn from(v: &str) -> Self {
        Value::Str(v.to_string())
    }
}

impl<'a> From<Cow<'a, str>> for Value {
    fn from(v: Cow<'a, str>) -> Self {
        Value::Str(v.to_string())
    }
}

impl<'a> From<&'a [u8]> for Value {
    fn from(v: &[u8]) -> Self {
        Value::Bin(v.into())
    }
}

impl<'a> From<Cow<'a, [u8]>> for Value {
    fn from(v: Cow<'a, [u8]>) -> Self {
        Value::Bin(v.into_owned())
    }
}

impl<V: Into<Value>> std::iter::FromIterator<V> for Value {
    fn from_iter<T: IntoIterator<Item = V>>(iter: T) -> Self {
        let v: Vec<Value> = iter.into_iter().map(Into::into).collect();
        Value::Array(v)
    }
}

use std::convert::TryFrom;

macro_rules! impl_try_from_value {
    ($t: ty, $p: ident) => {
        impl TryFrom<Value> for $t {
            type Error = Value;
            fn try_from(v: Value) -> Result<Self, Self::Error> {
                match v {
                    Value::$p(v) => Ok(v),
                    _ => Err(v),
                }
            }
        }
    };
}

macro_rules! impl_try_from_value_integer {
    ($t: ty) => {
        impl TryFrom<Value> for $t {
            type Error = Value;
            fn try_from(v: Value) -> Result<Self, Self::Error> {
                match v {
                    Value::Int(i) => TryFrom::try_from(i).map_err(|_| v),
                    _ => Err(v),
                }
            }
        }
    };
}

impl_try_from_value!(bool, Bool);
impl_try_from_value!(String, Str);
impl_try_from_value!(f32, F32);
impl_try_from_value!(f64, F64);
impl_try_from_value!(Vec<u8>, Bin);
impl_try_from_value!(Vec<Value>, Array);
impl_try_from_value!(BTreeMap<String, Value>, Map);
impl_try_from_value!(Timestamp, Timestamp);
impl_try_from_value!(Hash, Hash);
impl_try_from_value!(Identity, Identity);
impl_try_from_value!(StreamId, StreamId);
impl_try_from_value!(LockId, LockId);
impl_try_from_value!(DataLockbox, DataLockbox);
impl_try_from_value!(IdentityLockbox, IdentityLockbox);
impl_try_from_value!(StreamLockbox, StreamLockbox);
impl_try_from_value!(LockLockbox, LockLockbox);
impl_try_from_value_integer!(u8);
impl_try_from_value_integer!(u16);
impl_try_from_value_integer!(u32);
impl_try_from_value_integer!(u64);
impl_try_from_value_integer!(usize);
impl_try_from_value_integer!(i8);
impl_try_from_value_integer!(i16);
impl_try_from_value_integer!(i32);
impl_try_from_value_integer!(i64);
impl_try_from_value_integer!(isize);

impl serde::Serialize for Value {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Value::Null => serializer.serialize_unit(),
            Value::Bool(v) => serializer.serialize_bool(*v),
            Value::Int(v) => v.serialize(serializer),
            Value::Str(v) => serializer.serialize_str(v),
            Value::F32(v) => serializer.serialize_f32(*v),
            Value::F64(v) => serializer.serialize_f64(*v),
            Value::Bin(v) => serializer.serialize_bytes(v),
            Value::Array(v) => v.serialize(serializer),
            Value::Map(v) => v.serialize(serializer),
            Value::Timestamp(v) => v.serialize(serializer),
            Value::Hash(v) => v.serialize(serializer),
            Value::Identity(v) => v.serialize(serializer),
            Value::LockId(v) => v.serialize(serializer),
            Value::StreamId(v) => v.serialize(serializer),
            Value::DataLockbox(v) => v.serialize(serializer),
            Value::IdentityLockbox(v) => v.serialize(serializer),
            Value::StreamLockbox(v) => v.serialize(serializer),
            Value::LockLockbox(v) => v.serialize(serializer),
        }
    }
}

impl<'de> serde::Deserialize<'de> for Value {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::*;
        use std::fmt;

        struct ValueVisitor;
        impl<'de> Visitor<'de> for ValueVisitor {
            type Value = Value;

            fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                fmt.write_str("any valid fogpack Value")
            }

            fn visit_bool<E: Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(Value::Bool(v))
            }

            fn visit_i8<E: Error>(self, v: i8) -> Result<Self::Value, E> {
                Ok(Value::Int(Integer::from(v)))
            }

            fn visit_i16<E: Error>(self, v: i16) -> Result<Self::Value, E> {
                Ok(Value::Int(Integer::from(v)))
            }

            fn visit_i32<E: Error>(self, v: i32) -> Result<Self::Value, E> {
                Ok(Value::Int(Integer::from(v)))
            }

            fn visit_i64<E: Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(Value::Int(Integer::from(v)))
            }

            fn visit_u8<E: Error>(self, v: u8) -> Result<Self::Value, E> {
                Ok(Value::Int(Integer::from(v)))
            }

            fn visit_u16<E: Error>(self, v: u16) -> Result<Self::Value, E> {
                Ok(Value::Int(Integer::from(v)))
            }

            fn visit_u32<E: Error>(self, v: u32) -> Result<Self::Value, E> {
                Ok(Value::Int(Integer::from(v)))
            }

            fn visit_u64<E: Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(Value::Int(Integer::from(v)))
            }

            fn visit_f32<E: Error>(self, v: f32) -> Result<Self::Value, E> {
                Ok(Value::F32(v))
            }

            fn visit_f64<E: Error>(self, v: f64) -> Result<Self::Value, E> {
                Ok(Value::F64(v))
            }

            fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(Value::Str(v.into()))
            }

            fn visit_string<E: Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(Value::Str(v))
            }

            fn visit_bytes<E: Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                Ok(Value::Bin(v.into()))
            }

            fn visit_byte_buf<E: Error>(self, v: Vec<u8>) -> Result<Self::Value, E> {
                Ok(Value::Bin(v))
            }

            fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
                Ok(Value::Null)
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                // Allocate with the size hint, but be conservative. 4096 is what serde uses
                // internally for collections, so we'll do likewise.
                let mut seq = match access.size_hint() {
                    Some(size) => Vec::with_capacity(size.min(4096)),
                    None => Vec::new(),
                };
                while let Some(elem) = access.next_element()? {
                    seq.push(elem);
                }
                Ok(Value::Array(seq))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut map = BTreeMap::new();
                while let Some((key, val)) = access.next_entry()? {
                    map.insert(key, val);
                }
                Ok(Value::Map(map))
            }

            /// Should only be called when deserializing our special types.
            /// Fogpack's deserializer will always turn the variant into a u64
            fn visit_enum<A: EnumAccess<'de>>(self, access: A) -> Result<Self::Value, A::Error> {
                let (variant, access) = access.variant()?;
                use fog_crypto::serde::*;
                use serde_bytes::{Bytes, ByteBuf};
                match variant {
                    FOG_TYPE_ENUM_TIME_INDEX => {
                        let bytes: ByteBuf = access.newtype_variant()?;
                        let val = Timestamp::try_from(bytes.as_ref()).map_err(A::Error::custom)?;
                        Ok(Value::Timestamp(val))
                    }
                    FOG_TYPE_ENUM_HASH_INDEX => {
                        let bytes: ByteBuf = access.newtype_variant()?;
                        let val = Hash::try_from(bytes.as_ref())
                            .map_err(|e| A::Error::custom(e.serde_err()))?;
                        Ok(Value::Hash(val))
                    }
                    FOG_TYPE_ENUM_IDENTITY_INDEX => {
                        let bytes: ByteBuf = access.newtype_variant()?;
                        let val = Identity::try_from(bytes.as_ref())
                            .map_err(|e| A::Error::custom(e.serde_err()))?;
                        Ok(Value::Identity(val))
                    }
                    FOG_TYPE_ENUM_LOCK_ID_INDEX => {
                        let bytes: ByteBuf = access.newtype_variant()?;
                        let val = LockId::try_from(bytes.as_ref())
                            .map_err(|e| A::Error::custom(e.serde_err()))?;
                        Ok(Value::LockId(val))
                    }
                    FOG_TYPE_ENUM_STREAM_ID_INDEX => {
                        let bytes: ByteBuf = access.newtype_variant()?;
                        let val = StreamId::try_from(bytes.as_ref())
                            .map_err(|e| A::Error::custom(e.serde_err()))?;
                        Ok(Value::StreamId(val))
                    }
                    FOG_TYPE_ENUM_DATA_LOCKBOX_INDEX => {
                        let bytes: &Bytes = access.newtype_variant()?;
                        let val = DataLockboxRef::from_bytes(bytes)
                            .map_err(|e| A::Error::custom(e.serde_err()))?
                            .to_owned();
                        Ok(Value::DataLockbox(val))
                    }
                    FOG_TYPE_ENUM_IDENTITY_LOCKBOX_INDEX => {
                        let bytes: &Bytes = access.newtype_variant()?;
                        let val = IdentityLockboxRef::from_bytes(bytes)
                            .map_err(|e| A::Error::custom(e.serde_err()))?
                            .to_owned();
                        Ok(Value::IdentityLockbox(val))
                    }
                    FOG_TYPE_ENUM_STREAM_LOCKBOX_INDEX => {
                        let bytes: &Bytes = access.newtype_variant()?;
                        let val = StreamLockboxRef::from_bytes(bytes)
                            .map_err(|e| A::Error::custom(e.serde_err()))?
                            .to_owned();
                        Ok(Value::StreamLockbox(val))
                    }
                    FOG_TYPE_ENUM_LOCK_LOCKBOX_INDEX => {
                        let bytes: &Bytes = access.newtype_variant()?;
                        let val = LockLockboxRef::from_bytes(bytes)
                            .map_err(|e| A::Error::custom(e.serde_err()))?
                            .to_owned();
                        Ok(Value::LockLockbox(val))
                    }
                    _ => Err(A::Error::custom("unrecognized fogpack extension type")),
                }
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}
