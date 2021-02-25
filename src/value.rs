
use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;
use crate::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Value {
    Null,
    Boolean(bool),
    Integer(Integer),
    String(String),
    F32(f32),
    F64(f64),
    Binary(Vec<u8>),
    Array(Vec<Value>),
    Map(BTreeMap<String, Value>),
    Hash(Hash),
    Identity(Identity),
    StreamId(StreamId),
    LockId(LockId),
    Timestamp(Timestamp),
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
            Value::Boolean(v) => ValueRef::Boolean(v),
            Value::Integer(v) => ValueRef::Integer(v),
            Value::String(ref v) => ValueRef::String(v.as_ref()),
            Value::F32(v) => ValueRef::F32(v),
            Value::F64(v) => ValueRef::F64(v),
            Value::Binary(ref v) => ValueRef::Binary(v.as_slice()),
            Value::Array(ref v) => {
                ValueRef::Array(v.iter().map(|i| i.as_ref()).collect())
            },
            Value::Map(ref v) => {
                ValueRef::Map(v.iter().map(
                    |(f, ref i)| (f.as_ref(), i.as_ref())).collect())
            },
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
        if let Value::Integer(ref v) = *self {
            v.is_i64()
        } else {
            false
        }
    }

    pub fn is_u64(&self) -> bool {
        if let Value::Integer(ref v) = *self {
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

    pub fn is_hash(&self) -> bool {
        self.as_hash().is_some()
    }

    pub fn is_id(&self) -> bool {
        self.as_id().is_some()
    }

    pub fn is_timestamp(&self) -> bool {
        self.as_timestamp().is_some()
    }

    pub fn as_bool(&self) -> Option<bool> {
        if let Value::Boolean(val) = *self {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_int(&self) -> Option<Integer> {
        if let Value::Integer(val) = *self {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Value::Integer(ref n) => n.as_i64(),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match *self {
            Value::Integer(ref n) => n.as_u64(),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Value::Integer(ref n) => n.as_f64(),
            Value::F32(n) => Some(From::from(n)),
            Value::F64(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let Value::String(ref val) = *self {
            Some(val.as_str())
        } else {
            None
        }
    }

    pub fn as_slice(&self) -> Option<&[u8]> {
        if let Value::Binary(ref val) = *self {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        if let Value::Array(ref array) = *self {
            Some(&*array)
        } else {
            None
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>> {
        match *self {
            Value::Array(ref mut array) => Some(array),
            _ => None
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
            _ => None
        }
    }

    pub fn as_hash(&self) -> Option<&Hash> {
        if let Value::Hash(ref hash) = *self {
            Some(hash)
        } else {
            None
        }
    }

    pub fn as_id(&self) -> Option<&Identity> {
        if let Value::Identity(ref id) = *self {
            Some(id)
        } else {
            None
        }
    }

    pub fn as_timestamp(&self) -> Option<Timestamp> {
        if let Value::Timestamp(time) = *self {
            Some(time)
        } else {
            None
        }
    }

    pub fn as_string(&self) -> Option<&String> {
        if let Value::String(ref val) = *self {
            Some(val)
        } else {
            None
        }
    }

    //pub fn get<I: Index>(&self, index: I) -> Option<&Value> {
    //    index.index_into(self)
    //}

    //pub fn get_mut<I: Index>(&mut self, index: I) -> Option<&mut Value> {
    //    index.index_into_mut(self)
    //}
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ValueRef<'a> {
    Null,
    Boolean(bool),
    Integer(Integer),
    String(&'a str),
    F32(f32),
    F64(f64),
    Binary(&'a [u8]),
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
