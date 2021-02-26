use crate::*;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::Index;
use std::{collections::BTreeMap, default, fmt::Debug};

#[derive(Clone, Debug)]
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
                    .map(|(f, ref i)| (f.as_ref(), i.as_ref()))
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

    pub fn is_nil(&self) -> bool {
        if let Value::Null = *self {
            true
        } else {
            false
        }
    }

    pub fn is_bool(&self) -> bool {
        self.as_bool().is_some()
    }

    pub fn is_int(&self) -> bool {
        self.as_int().is_some()
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
        if let Value::F32(..) = *self {
            true
        } else {
            false
        }
    }

    pub fn is_f64(&self) -> bool {
        if let Value::F64(..) = *self {
            true
        } else {
            false
        }
    }

    pub fn is_str(&self) -> bool {
        self.as_str().is_some()
    }

    pub fn is_bin(&self) -> bool {
        self.as_slice().is_some()
    }

    pub fn is_array(&self) -> bool {
        self.as_array().is_some()
    }

    pub fn is_map(&self) -> bool {
        self.as_map().is_some()
    }

    pub fn is_timestamp(&self) -> bool {
        self.as_timestamp().is_some()
    }

    pub fn is_hash(&self) -> bool {
        self.as_hash().is_some()
    }

    pub fn is_identity(&self) -> bool {
        self.as_identity().is_some()
    }

    pub fn is_stream_id(&self) -> bool {
        self.as_stream_id().is_some()
    }

    pub fn is_lock_id(&self) -> bool {
        self.as_lock_id().is_some()
    }

    pub fn is_lockbox(&self) -> bool {
        match self {
            Value::DataLockbox(_)
            | Value::IdentityLockbox(_)
            | Value::StreamLockbox(_)
            | Value::LockLockbox(_) => true,
            _ => false,
        }
    }

    pub fn is_data_lockbox(&self) -> bool {
        match self {
            Value::DataLockbox(_) => true,
            _ => false,
        }
    }

    pub fn is_identity_lockbox(&self) -> bool {
        match self {
            Value::IdentityLockbox(_) => true,
            _ => false,
        }
    }

    pub fn is_stream_lockbox(&self) -> bool {
        match self {
            Value::StreamLockbox(_) => true,
            _ => false,
        }
    }

    pub fn is_lock_lockbox(&self) -> bool {
        match self {
            Value::LockLockbox(_) => true,
            _ => false,
        }
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

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Value::Int(ref n) => n.as_f64(),
            Value::F32(n) => Some(From::from(n)),
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

            fn visit_char<E: Error>(self, v: char) -> Result<Self::Value, E> {
                Ok(Value::Str(v.into()))
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
                let mut seq = match access.size_hint() {
                    Some(size) => Vec::with_capacity(size),
                    None => Vec::new(),
                };
                while let Some(elem) = access.next_element()? {
                    seq.push(elem);
                }
                Ok(Value::Array(seq))
            }

            //fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
            //    let mut map = match access.size_hint() {
            //        Some(size) => Vec::with_capacity(size),
            //        None => Vec::new(),
            //    };

            //}
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ValueRef<'a> {
    Null,
    Bool(bool),
    Int(Integer),
    Str(&'a str),
    F32(f32),
    F64(f64),
    Bin(&'a [u8]),
    Array(Vec<ValueRef<'a>>),
    Map(BTreeMap<&'a str, ValueRef<'a>>),
    Hash(Hash),
    Identity(Identity),
    StreamId(StreamId),
    LockId(LockId),
    Timestamp(Timestamp),
    DataLockbox(&'a DataLockboxRef),
    IdentityLockbox(&'a IdentityLockboxRef),
    StreamLockbox(&'a StreamLockboxRef),
    LockLockbox(&'a LockLockboxRef),
}

static NULL_REF: ValueRef<'static> = ValueRef::Null;
