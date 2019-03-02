use std::ops;
use std::borrow::Cow;
use std::fmt::{self, Display};
use std::collections::BTreeMap;

use crypto::timestamp::Timestamp;
use crypto::integer::Integer;
use crypto::lock::{Lockbox,LockboxRef};
use crypto::hash::Hash;
use crypto::key::{Identity,Signature};

enum ValueType {
    Nil,
    Boolean(bool),
    Integer(Integer),
    String(String),
    F32(f32),
    F64(f64),
    Binary(Vec<u8>),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
    Hash(Hash),
    Identity(Identity),
    Lockbox(Lockbox),
    Timestamp(Timestamp),
}

pub struct Value {
    val: ValueType,
    sign: Vec<Signature>,
    id: Vec<Identity>,
}

impl Value {

    fn new_from_value_type(v: ValueType) -> Value {
        Value {
            val: v,
            sign: Vec::new(),
            id: Vec::new()
        }
    }

    pub fn new_nil() -> Value {
        Value::new_from_value_type(ValueType::Nil)
    }

    pub fn add_sign(&mut self, s: Signature) {
        self.sign.push(s);
    }

    pub fn add_id_for_signing(&mut self, id: Identity) {
        self.id.push(id);
    }

    pub fn is_signed(&self) -> bool {
        !self.sign.is_empty()
    }
    
    pub fn will_be_signed(&self) -> bool {
        !self.id.is_empty()
    }

    pub fn signed_by(&self) -> Vec<&Identity> {
        self.sign.iter().map(|s| s.signed_by()).collect()
    }

    pub fn is_null(&self) -> bool {
        if let ValueType::Nil = self.val {
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
        if let ValueType::Integer(ref v) = self.val {
            v.is_i64()
        } else {
            false
        }
    }

    pub fn is_u64(&self) -> bool {
        if let ValueType::Integer(ref v) = self.val {
            v.is_u64()
        } else {
            false
        }
    }

    pub fn is_f32(&self) -> bool {
        if let ValueType::F32(..) = self.val {
            true
        } else {
            false
        }
    }

    pub fn is_f64(&self) -> bool {
        if let ValueType::F64(..) = self.val {
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

    pub fn is_obj(&self) -> bool {
        self.as_obj().is_some()
    }

    pub fn is_hash(&self) -> bool {
        self.as_hash().is_some()
    }

    pub fn is_id(&self) -> bool {
        self.as_id().is_some()
    }

    pub fn is_lockbox(&self) -> bool {
        self.as_lockbox().is_some()
    }

    pub fn is_timestamp(&self) -> bool {
        self.as_timestamp().is_some()
    }

    pub fn as_bool(&self) -> Option<bool> {
        if let ValueType::Boolean(val) = self.val {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_int(&self) -> Option<Integer> {
        if let ValueType::Integer(val) = self.val {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self.val {
            ValueType::Integer(ref n) => n.as_i64(),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self.val {
            ValueType::Integer(ref n) => n.as_u64(),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self.val {
            ValueType::Integer(ref n) => n.as_f64(),
            ValueType::F32(n) => Some(From::from(n)),
            ValueType::F64(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let ValueType::String(ref val) = self.val {
            Some(val.as_str())
        } else {
            None
        }
    }

    pub fn as_slice(&self) -> Option<&[u8]> {
        if let ValueType::Binary(ref val) = self.val {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        if let ValueType::Array(ref array) = self.val {
            Some(&*array)
        } else {
            None
        }
    }

    pub fn as_obj(&self) -> Option<&BTreeMap<String, Value>> {
        if let ValueType::Object(ref obj) = self.val {
            Some(obj)
        } else {
            None
        }
    }

    pub fn as_hash(&self) -> Option<&Hash> {
        if let ValueType::Hash(ref hash) = self.val {
            Some(hash)
        } else {
            None
        }
    }

    pub fn as_id(&self) -> Option<&Identity> {
        if let ValueType::Identity(ref id) = self.val {
            Some(id)
        } else {
            None
        }
    }

    pub fn as_lockbox(&self) -> Option<&Lockbox> {
        if let ValueType::Lockbox(ref lock) = self.val {
            Some(lock)
        } else {
            None
        }
    }

    pub fn as_timestamp(&self) -> Option<Timestamp> {
        if let ValueType::Timestamp(time) = self.val {
            Some(time)
        } else {
            None
        }
    }

}

impl ops::Index<usize> for Value {
    type Output = Value;

    /// Index into an array if Value is one. 
    ///
    /// # Panics
    ///
    /// Will panic if not an array or if index is out of bounds.
    fn index(&self, index: usize) -> &Value {
        self.as_array().and_then(|v| v.get(index)).expect("Index out of bounds")
    }
}

impl<'a>  ops::Index<&'a String> for Value {
    type Output = Value;

    /// Index into an object if Value is one. 
    ///
    /// # Panics
    ///
    /// Panics if key is not present, or if Value is not an object
    fn index(&self, key: &String) -> &Value {
        self.as_obj().and_then(|v| v.get(key)).expect("No entry for key")
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value {
            val: ValueType::Boolean(v),
            sign: Vec::new(),
            id: Vec::new(),
        }
    }
}

impl From<Integer> for Value {
    fn from(v: Integer) -> Self {
        Value::new_from_value_type(ValueType::Integer(v))
    }
}

impl From<u8> for Value {
    fn from(v: u8) -> Self {
        Value::new_from_value_type(ValueType::Integer(From::from(v)))
    }
}

impl From<u16> for Value {
    fn from(v: u16) -> Self {
        Value::new_from_value_type(ValueType::Integer(From::from(v)))
    }
}

impl From<u32> for Value {
    fn from(v: u32) -> Self {
        Value::new_from_value_type(ValueType::Integer(From::from(v)))
    }
}

impl From<u64> for Value {
    fn from(v: u64) -> Self {
        Value::new_from_value_type(ValueType::Integer(From::from(v)))
    }
}

impl From<usize> for Value {
    fn from(v: usize) -> Self {
        Value::new_from_value_type(ValueType::Integer(From::from(v)))
    }
}

impl From<i8> for Value {
    fn from(v: i8) -> Self {
        Value::new_from_value_type(ValueType::Integer(From::from(v)))
    }
}

impl From<i16> for Value {
    fn from(v: i16) -> Self {
        Value::new_from_value_type(ValueType::Integer(From::from(v)))
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::new_from_value_type(ValueType::Integer(From::from(v)))
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::new_from_value_type(ValueType::Integer(From::from(v)))
    }
}

impl From<isize> for Value {
    fn from(v: isize) -> Self {
        Value::new_from_value_type(ValueType::Integer(From::from(v)))
    }
}

impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Value::new_from_value_type(ValueType::F32(v))
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::new_from_value_type(ValueType::F64(v))
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::new_from_value_type(ValueType::String(v))
    }
}

impl<'a> From<&'a str> for Value {
    fn from(v: &str) -> Self {
        Value::new_from_value_type(ValueType::String(v.to_string()))
    }
}

impl<'a> From<Cow<'a, str>> for Value {
    fn from(v: Cow<'a, str>) -> Self {
        Value::new_from_value_type(ValueType::String(v.to_string()))
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value::new_from_value_type(ValueType::Binary(v))
    }
}

impl<'a> From<&'a [u8]> for Value {
    fn from(v: &[u8]) -> Self {
        Value::new_from_value_type(ValueType::Binary(v.into()))
    }
}

impl<'a> From<Cow<'a, [u8]>> for Value {
    fn from(v: Cow<'a, [u8]>) -> Self {
        Value::new_from_value_type(ValueType::Binary(v.into_owned()))
    }
}

impl From<Vec<Value>> for Value {
    fn from(v: Vec<Value>) -> Self {
        Value::new_from_value_type(ValueType::Array(v))
    }
}

impl From<BTreeMap<String, Value>> for Value {
    fn from(v: BTreeMap<String, Value>) -> Self {
        Value::new_from_value_type(ValueType::Object(v))
    }
}

impl From<Hash> for Value {
    fn from(v: Hash) -> Self {
        Value::new_from_value_type(ValueType::Hash(v))
    }
}

impl From<Identity> for Value {
    fn from(v: Identity) -> Self {
        Value::new_from_value_type(ValueType::Identity(v))
    }
}

impl From<Lockbox> for Value {
    fn from(v: Lockbox) -> Self {
        Value::new_from_value_type(ValueType::Lockbox(v))
    }
}

impl From<Timestamp> for Value {
    fn from(v: Timestamp) -> Self {
        Value::new_from_value_type(ValueType::Timestamp(v))
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        //let mut ids = Vec::new();
        //let mut hashes = Vec::new();
        //let mut owners = Vec::new();
        //let mut bin = Vec::new();

        match self.val {
            ValueType::Nil => Display::fmt("null", f),
            ValueType::Boolean(val) => write!(f, "{}", val),
            ValueType::Integer(ref val) => write!(f, "{}", val),
            ValueType::F32(val) => write!(f, "{}", val),
            ValueType::F64(val) => write!(f, "{}", val),
            ValueType::String(ref val) => {
                if val.starts_with("<") {
                    write!(f, "\"<{}\"", val)
                }
                else {
                    write!(f, "\"{}\"", val)
                }
            }
            ValueType::Binary(ref _val) => write!(f, "\"<Bin>\""),
            ValueType::Array(ref vec) => {
                let res = vec.iter()
                    .map(|val| format!("{}", val))
                    .collect::<Vec<String>>()
                    .join(", ");

                write!(f, "[{}]", res)
            }
            ValueType::Object(ref obj) => {
                write!(f, "{{")?;

                match obj.iter().take(1).next() {
                    Some((ref k, ref v)) => {
                        write!(f, "{}: {}", k, v)?;
                    }
                    None => {
                        write!(f, "")?;
                    }
                }

                for (ref k, ref v) in obj.iter().skip(1) {
                    write!(f, ", {}: {}", k, v)?;
                }

                write!(f, "}}")
            }
            ValueType::Timestamp(ref val) => write!(f, "{}", val),
            ValueType::Hash(ref val) => write!(f, "\"<Hash>\""),
            ValueType::Identity(ref val) => write!(f, "\"<Identity>\""),
            ValueType::Lockbox(ref val) => write!(f, "\"<Lockbox>\""),
        }
    }
}

