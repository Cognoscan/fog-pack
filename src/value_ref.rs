use crate::value::Value;
use crate::*;
use std::ops::Index;
use std::{collections::BTreeMap, fmt::Debug};

#[derive(Clone, Debug, PartialEq)]
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

impl<'a> ValueRef<'a> {
    pub fn to_owned(&self) -> Value {
        match *self {
            ValueRef::Null => Value::Null,
            ValueRef::Bool(v) => Value::Bool(v),
            ValueRef::Int(v) => Value::Int(v),
            ValueRef::Str(v) => Value::Str(v.into()),
            ValueRef::F32(v) => Value::F32(v),
            ValueRef::F64(v) => Value::F64(v),
            ValueRef::Bin(v) => Value::Bin(v.into()),
            ValueRef::Array(ref v) => Value::Array(v.iter().map(|i| i.to_owned()).collect()),
            ValueRef::Map(ref v) => Value::Map(
                v.iter()
                    .map(|(f, i)| (String::from(*f), i.to_owned()))
                    .collect(),
            ),
            ValueRef::Timestamp(v) => Value::Timestamp(v),
            ValueRef::Hash(ref v) => Value::Hash(v.clone()),
            ValueRef::Identity(ref v) => Value::Identity(v.clone()),
            ValueRef::StreamId(ref v) => Value::StreamId(v.clone()),
            ValueRef::LockId(ref v) => Value::LockId(v.clone()),
            ValueRef::DataLockbox(v) => Value::DataLockbox(v.to_owned()),
            ValueRef::IdentityLockbox(v) => Value::IdentityLockbox(v.to_owned()),
            ValueRef::StreamLockbox(v) => Value::StreamLockbox(v.to_owned()),
            ValueRef::LockLockbox(v) => Value::LockLockbox(v.to_owned()),
        }
    }

    pub fn is_nil(&self) -> bool {
        matches!(self, ValueRef::Null)
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, ValueRef::Bool(_))
    }

    pub fn is_int(&self) -> bool {
        matches!(self, ValueRef::Int(_))
    }

    pub fn is_i64(&self) -> bool {
        if let ValueRef::Int(ref v) = *self {
            v.is_i64()
        } else {
            false
        }
    }

    pub fn is_u64(&self) -> bool {
        if let ValueRef::Int(ref v) = *self {
            v.is_u64()
        } else {
            false
        }
    }

    pub fn is_f32(&self) -> bool {
        matches!(self, ValueRef::F32(_))
    }

    pub fn is_f64(&self) -> bool {
        matches!(self, ValueRef::F64(_))
    }

    pub fn is_str(&self) -> bool {
        matches!(self, ValueRef::Str(_))
    }

    pub fn is_bin(&self) -> bool {
        matches!(self, ValueRef::Bin(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, ValueRef::Array(_))
    }

    pub fn is_map(&self) -> bool {
        matches!(self, ValueRef::Map(_))
    }

    pub fn is_timestamp(&self) -> bool {
        matches!(self, ValueRef::Timestamp(_))
    }

    pub fn is_hash(&self) -> bool {
        matches!(self, ValueRef::Hash(_))
    }

    pub fn is_identity(&self) -> bool {
        matches!(self, ValueRef::Identity(_))
    }

    pub fn is_stream_id(&self) -> bool {
        matches!(self, ValueRef::StreamId(_))
    }

    pub fn is_lock_id(&self) -> bool {
        matches!(self, ValueRef::LockId(_))
    }

    pub fn is_lockbox(&self) -> bool {
        matches!(
            self,
            ValueRef::DataLockbox(_)
                | ValueRef::IdentityLockbox(_)
                | ValueRef::StreamLockbox(_)
                | ValueRef::LockLockbox(_)
        )
    }

    pub fn is_data_lockbox(&self) -> bool {
        matches!(self, ValueRef::DataLockbox(_))
    }

    pub fn is_identity_lockbox(&self) -> bool {
        matches!(self, ValueRef::IdentityLockbox(_))
    }

    pub fn is_stream_lockbox(&self) -> bool {
        matches!(self, ValueRef::StreamLockbox(_))
    }

    pub fn is_lock_lockbox(&self) -> bool {
        matches!(self, ValueRef::LockLockbox(_))
    }

    pub fn as_bool(&self) -> Option<bool> {
        if let ValueRef::Bool(val) = *self {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_int(&self) -> Option<Integer> {
        if let ValueRef::Int(val) = *self {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            ValueRef::Int(ref n) => n.as_i64(),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match *self {
            ValueRef::Int(ref n) => n.as_u64(),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            ValueRef::F32(n) => Some(From::from(n)),
            ValueRef::F64(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let ValueRef::Str(val) = *self {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_slice(&self) -> Option<&[u8]> {
        if let ValueRef::Bin(val) = *self {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_array(&self) -> Option<&[ValueRef<'a>]> {
        if let ValueRef::Array(ref array) = *self {
            Some(&*array)
        } else {
            None
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut [ValueRef<'a>]> {
        match *self {
            ValueRef::Array(ref mut array) => Some(array),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&BTreeMap<&'a str, ValueRef<'a>>> {
        match *self {
            ValueRef::Map(ref map) => Some(map),
            _ => None,
        }
    }

    pub fn as_map_mut(&mut self) -> Option<&mut BTreeMap<&'a str, ValueRef<'a>>> {
        match *self {
            ValueRef::Map(ref mut map) => Some(map),
            _ => None,
        }
    }

    pub fn as_timestamp(&self) -> Option<Timestamp> {
        if let ValueRef::Timestamp(time) = *self {
            Some(time)
        } else {
            None
        }
    }

    pub fn as_hash(&self) -> Option<&Hash> {
        if let ValueRef::Hash(ref hash) = *self {
            Some(hash)
        } else {
            None
        }
    }

    pub fn as_identity(&self) -> Option<&Identity> {
        if let ValueRef::Identity(ref id) = *self {
            Some(id)
        } else {
            None
        }
    }

    pub fn as_stream_id(&self) -> Option<&StreamId> {
        if let ValueRef::StreamId(ref id) = *self {
            Some(id)
        } else {
            None
        }
    }

    pub fn as_lock_id(&self) -> Option<&LockId> {
        if let ValueRef::LockId(ref id) = *self {
            Some(id)
        } else {
            None
        }
    }

    pub fn as_data_lockbox(&self) -> Option<&DataLockboxRef> {
        if let ValueRef::DataLockbox(lockbox) = *self {
            Some(lockbox)
        } else {
            None
        }
    }

    pub fn as_identity_lockbox(&self) -> Option<&IdentityLockboxRef> {
        if let ValueRef::IdentityLockbox(lockbox) = *self {
            Some(lockbox)
        } else {
            None
        }
    }

    pub fn as_stream_lockbox(&self) -> Option<&StreamLockboxRef> {
        if let ValueRef::StreamLockbox(lockbox) = *self {
            Some(lockbox)
        } else {
            None
        }
    }

    pub fn as_lock_lockbox(&self) -> Option<&LockLockboxRef> {
        if let ValueRef::LockLockbox(lockbox) = *self {
            Some(lockbox)
        } else {
            None
        }
    }
}

impl<'a> std::default::Default for ValueRef<'a> {
    fn default() -> Self {
        ValueRef::Null
    }
}

static NULL_REF: ValueRef<'static> = ValueRef::Null;

impl<'a> Index<usize> for ValueRef<'a> {
    type Output = ValueRef<'a>;

    fn index(&self, index: usize) -> &Self::Output {
        self.as_array()
            .and_then(|v| v.get(index))
            .unwrap_or(&NULL_REF)
    }
}

impl<'a> Index<&str> for ValueRef<'a> {
    type Output = ValueRef<'a>;

    fn index(&self, index: &str) -> &Self::Output {
        self.as_map()
            .and_then(|v| v.get(index))
            .unwrap_or(&NULL_REF)
    }
}

impl<'a> PartialEq<Value> for ValueRef<'a> {
    fn eq(&self, other: &Value) -> bool {
        use std::ops::Deref;
        match self {
            ValueRef::Null => other == &Value::Null,
            ValueRef::Bool(s) => {
                if let Value::Bool(o) = other {
                    s == o
                } else {
                    false
                }
            }
            ValueRef::Int(s) => {
                if let Value::Int(o) = other {
                    s == o
                } else {
                    false
                }
            }
            ValueRef::Str(s) => {
                if let Value::Str(o) = other {
                    s == o
                } else {
                    false
                }
            }
            ValueRef::F32(s) => {
                if let Value::F32(o) = other {
                    s == o
                } else {
                    false
                }
            }
            ValueRef::F64(s) => {
                if let Value::F64(o) = other {
                    s == o
                } else {
                    false
                }
            }
            ValueRef::Bin(s) => {
                if let Value::Bin(o) = other {
                    s == o
                } else {
                    false
                }
            }
            ValueRef::Array(s) => {
                if let Value::Array(o) = other {
                    s == o
                } else {
                    false
                }
            }
            ValueRef::Map(s) => {
                if let Value::Map(o) = other {
                    s.len() == o.len()
                        && s.iter()
                            .zip(o)
                            .all(|((ks, vs), (ko, vo))| (ks == ko) && (vs == vo))
                } else {
                    false
                }
            }
            ValueRef::Hash(s) => {
                if let Value::Hash(o) = other {
                    s == o
                } else {
                    false
                }
            }
            ValueRef::Identity(s) => {
                if let Value::Identity(o) = other {
                    s == o
                } else {
                    false
                }
            }
            ValueRef::StreamId(s) => {
                if let Value::StreamId(o) = other {
                    s == o
                } else {
                    false
                }
            }
            ValueRef::LockId(s) => {
                if let Value::LockId(o) = other {
                    s == o
                } else {
                    false
                }
            }
            ValueRef::Timestamp(s) => {
                if let Value::Timestamp(o) = other {
                    s == o
                } else {
                    false
                }
            }
            ValueRef::DataLockbox(s) => {
                if let Value::DataLockbox(o) = other {
                    s == &o.deref()
                } else {
                    false
                }
            }
            ValueRef::IdentityLockbox(s) => {
                if let Value::IdentityLockbox(o) = other {
                    s == &o.deref()
                } else {
                    false
                }
            }
            ValueRef::StreamLockbox(s) => {
                if let Value::StreamLockbox(o) = other {
                    s == &o.deref()
                } else {
                    false
                }
            }
            ValueRef::LockLockbox(s) => {
                if let Value::LockLockbox(o) = other {
                    s == &o.deref()
                } else {
                    false
                }
            }
        }
    }
}

macro_rules! impl_value_from_integer {
    ($t: ty) => {
        impl<'a> From<$t> for ValueRef<'a> {
            fn from(v: $t) -> Self {
                ValueRef::Int(From::from(v))
            }
        }
    };
}

macro_rules! impl_value_from {
    ($t: ty, $p: ident) => {
        impl<'a> From<$t> for ValueRef<'a> {
            fn from(v: $t) -> Self {
                ValueRef::$p(v)
            }
        }
    };
}

impl_value_from!(bool, Bool);
impl_value_from!(Integer, Int);
impl_value_from!(f32, F32);
impl_value_from!(f64, F64);
impl_value_from!(&'a str, Str);
impl_value_from!(&'a [u8], Bin);
impl_value_from!(Vec<ValueRef<'a>>, Array);
impl_value_from!(BTreeMap<&'a str, ValueRef<'a>>, Map);
impl_value_from!(Timestamp, Timestamp);
impl_value_from!(Hash, Hash);
impl_value_from!(Identity, Identity);
impl_value_from!(StreamId, StreamId);
impl_value_from!(LockId, LockId);
impl_value_from!(&'a DataLockboxRef, DataLockbox);
impl_value_from!(&'a IdentityLockboxRef, IdentityLockbox);
impl_value_from!(&'a StreamLockboxRef, StreamLockbox);
impl_value_from!(&'a LockLockboxRef, LockLockbox);
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

impl<'a> From<()> for ValueRef<'a> {
    fn from((): ()) -> Self {
        ValueRef::Null
    }
}

impl<'a, V: Into<ValueRef<'a>>> std::iter::FromIterator<V> for ValueRef<'a> {
    fn from_iter<T: IntoIterator<Item = V>>(iter: T) -> Self {
        let v: Vec<ValueRef> = iter.into_iter().map(Into::into).collect();
        ValueRef::Array(v)
    }
}

use std::convert::TryFrom;

macro_rules! impl_try_from_value {
    ($t: ty, $p: ident) => {
        impl<'a> TryFrom<ValueRef<'a>> for $t {
            type Error = ValueRef<'a>;
            fn try_from(v: ValueRef<'a>) -> Result<Self, Self::Error> {
                match v {
                    ValueRef::$p(v) => Ok(v),
                    _ => Err(v),
                }
            }
        }
    };
}

macro_rules! impl_try_from_value_integer {
    ($t: ty) => {
        impl<'a> TryFrom<ValueRef<'a>> for $t {
            type Error = ValueRef<'a>;
            fn try_from(v: ValueRef<'a>) -> Result<Self, Self::Error> {
                match v {
                    ValueRef::Int(i) => TryFrom::try_from(i).map_err(|_| v),
                    _ => Err(v),
                }
            }
        }
    };
}

impl_try_from_value!(bool, Bool);
impl_try_from_value!(&'a str, Str);
impl_try_from_value!(f32, F32);
impl_try_from_value!(f64, F64);
impl_try_from_value!(&'a [u8], Bin);
impl_try_from_value!(Vec<ValueRef<'a>>, Array);
impl_try_from_value!(BTreeMap<&'a str, ValueRef<'a>>, Map);
impl_try_from_value!(Timestamp, Timestamp);
impl_try_from_value!(Hash, Hash);
impl_try_from_value!(Identity, Identity);
impl_try_from_value!(StreamId, StreamId);
impl_try_from_value!(LockId, LockId);
impl_try_from_value!(&'a DataLockboxRef, DataLockbox);
impl_try_from_value!(&'a IdentityLockboxRef, IdentityLockbox);
impl_try_from_value!(&'a StreamLockboxRef, StreamLockbox);
impl_try_from_value!(&'a LockLockboxRef, LockLockbox);
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

impl<'a> serde::Serialize for ValueRef<'a> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            ValueRef::Null => serializer.serialize_unit(),
            ValueRef::Bool(v) => serializer.serialize_bool(*v),
            ValueRef::Int(v) => v.serialize(serializer),
            ValueRef::Str(v) => serializer.serialize_str(v),
            ValueRef::F32(v) => serializer.serialize_f32(*v),
            ValueRef::F64(v) => serializer.serialize_f64(*v),
            ValueRef::Bin(v) => serializer.serialize_bytes(v),
            ValueRef::Array(v) => v.serialize(serializer),
            ValueRef::Map(v) => v.serialize(serializer),
            ValueRef::Timestamp(v) => v.serialize(serializer),
            ValueRef::Hash(v) => v.serialize(serializer),
            ValueRef::Identity(v) => v.serialize(serializer),
            ValueRef::LockId(v) => v.serialize(serializer),
            ValueRef::StreamId(v) => v.serialize(serializer),
            ValueRef::DataLockbox(v) => v.serialize(serializer),
            ValueRef::IdentityLockbox(v) => v.serialize(serializer),
            ValueRef::StreamLockbox(v) => v.serialize(serializer),
            ValueRef::LockLockbox(v) => v.serialize(serializer),
        }
    }
}

impl<'de> serde::Deserialize<'de> for ValueRef<'de> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::*;
        use std::fmt;

        struct ValueVisitor;
        impl<'de> Visitor<'de> for ValueVisitor {
            type Value = ValueRef<'de>;

            fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                fmt.write_str("any valid fogpack Value")
            }

            fn visit_bool<E: Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(ValueRef::Bool(v))
            }

            fn visit_i8<E: Error>(self, v: i8) -> Result<Self::Value, E> {
                Ok(ValueRef::Int(Integer::from(v)))
            }

            fn visit_i16<E: Error>(self, v: i16) -> Result<Self::Value, E> {
                Ok(ValueRef::Int(Integer::from(v)))
            }

            fn visit_i32<E: Error>(self, v: i32) -> Result<Self::Value, E> {
                Ok(ValueRef::Int(Integer::from(v)))
            }

            fn visit_i64<E: Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(ValueRef::Int(Integer::from(v)))
            }

            fn visit_u8<E: Error>(self, v: u8) -> Result<Self::Value, E> {
                Ok(ValueRef::Int(Integer::from(v)))
            }

            fn visit_u16<E: Error>(self, v: u16) -> Result<Self::Value, E> {
                Ok(ValueRef::Int(Integer::from(v)))
            }

            fn visit_u32<E: Error>(self, v: u32) -> Result<Self::Value, E> {
                Ok(ValueRef::Int(Integer::from(v)))
            }

            fn visit_u64<E: Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(ValueRef::Int(Integer::from(v)))
            }

            fn visit_f32<E: Error>(self, v: f32) -> Result<Self::Value, E> {
                Ok(ValueRef::F32(v))
            }

            fn visit_f64<E: Error>(self, v: f64) -> Result<Self::Value, E> {
                Ok(ValueRef::F64(v))
            }

            fn visit_borrowed_str<E: Error>(self, v: &'de str) -> Result<Self::Value, E> {
                Ok(ValueRef::Str(v))
            }

            fn visit_borrowed_bytes<E: Error>(self, v: &'de [u8]) -> Result<Self::Value, E> {
                Ok(ValueRef::Bin(v))
            }

            fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
                Ok(ValueRef::Null)
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
                Ok(ValueRef::Array(seq))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut map = BTreeMap::new();
                while let Some((key, val)) = access.next_entry()? {
                    map.insert(key, val);
                }
                Ok(ValueRef::Map(map))
            }

            /// Should only be called when deserializing our special types.
            /// Fogpack's deserializer will always turn the variant into a u64
            fn visit_enum<A: EnumAccess<'de>>(self, access: A) -> Result<Self::Value, A::Error> {
                let (variant, access) = access.variant()?;
                use fog_crypto::serde::*;
                use serde_bytes::Bytes;
                let bytes: &Bytes = access.newtype_variant()?;
                match variant {
                    FOG_TYPE_ENUM_TIME_INDEX => {
                        let val = Timestamp::try_from(bytes.as_ref()).map_err(A::Error::custom)?;
                        Ok(ValueRef::Timestamp(val))
                    }
                    FOG_TYPE_ENUM_HASH_INDEX => {
                        let val = Hash::try_from(bytes.as_ref())
                            .map_err(|e| A::Error::custom(e.serde_err()))?;
                        Ok(ValueRef::Hash(val))
                    }
                    FOG_TYPE_ENUM_IDENTITY_INDEX => {
                        let val = Identity::try_from(bytes.as_ref())
                            .map_err(|e| A::Error::custom(e.serde_err()))?;
                        Ok(ValueRef::Identity(val))
                    }
                    FOG_TYPE_ENUM_LOCK_ID_INDEX => {
                        let val = LockId::try_from(bytes.as_ref())
                            .map_err(|e| A::Error::custom(e.serde_err()))?;
                        Ok(ValueRef::LockId(val))
                    }
                    FOG_TYPE_ENUM_STREAM_ID_INDEX => {
                        let val = StreamId::try_from(bytes.as_ref())
                            .map_err(|e| A::Error::custom(e.serde_err()))?;
                        Ok(ValueRef::StreamId(val))
                    }
                    FOG_TYPE_ENUM_DATA_LOCKBOX_INDEX => {
                        let val = DataLockboxRef::from_bytes(bytes)
                            .map_err(|e| A::Error::custom(e.serde_err()))?;
                        Ok(ValueRef::DataLockbox(val))
                    }
                    FOG_TYPE_ENUM_IDENTITY_LOCKBOX_INDEX => {
                        let val = IdentityLockboxRef::from_bytes(bytes)
                            .map_err(|e| A::Error::custom(e.serde_err()))?;
                        Ok(ValueRef::IdentityLockbox(val))
                    }
                    FOG_TYPE_ENUM_STREAM_LOCKBOX_INDEX => {
                        let val = StreamLockboxRef::from_bytes(bytes)
                            .map_err(|e| A::Error::custom(e.serde_err()))?;
                        Ok(ValueRef::StreamLockbox(val))
                    }
                    FOG_TYPE_ENUM_LOCK_LOCKBOX_INDEX => {
                        let val = LockLockboxRef::from_bytes(bytes)
                            .map_err(|e| A::Error::custom(e.serde_err()))?;
                        Ok(ValueRef::LockLockbox(val))
                    }
                    _ => Err(A::Error::custom("unrecognized fogpack extension type")),
                }
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}
