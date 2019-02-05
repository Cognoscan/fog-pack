use std::ops;
use std::borrow::Cow;
use std::fmt::{self, Display};

use crypto::integer::Integer;
use crypto::utf8string::{Utf8String, Utf8StringRef};
use crypto::timestamp::Timestamp;
use crypto::lock::{Lockbox,LockboxRef};
use crypto::hash::Hash;
use crypto::key::{Identity,Signature};

/// Represents any valid MessagePack value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    Boolean(bool),
    Integer(Integer),
    F32(f32),
    F64(f64),
    String(Utf8String),
    Binary(Vec<u8>),
    Array(Vec<Value>),
    Map(Vec<(Value, Value)>),
    Timestamp(Timestamp),
    Hash(Hash),
    Identity(Identity),
    Lockbox(Lockbox),
    Ext(i8, Vec<u8>),
}

impl Value {
    pub fn as_ref(&self) -> ValueRef {
        match self {
            &Value::Nil => ValueRef::Nil,
            &Value::Boolean(val) => ValueRef::Boolean(val),
            &Value::Integer(val) => ValueRef::Integer(val),
            &Value::F32(val) => ValueRef::F32(val),
            &Value::F64(val) => ValueRef::F64(val),
            &Value::String(ref val) => ValueRef::String(val.as_ref()),
            &Value::Binary(ref val) => ValueRef::Binary(val.as_slice()),
            &Value::Array(ref val) => {
                ValueRef::Array(val.iter().map(|v| v.as_ref()).collect())
            }
            &Value::Map(ref val) => {
                ValueRef::Map(val.iter().map(|&(ref k, ref v)| (k.as_ref(), v.as_ref())).collect())
            }
            &Value::Timestamp(val) => ValueRef::Timestamp(val),
            &Value::Hash(val) => ValueRef::Hash(val),
            &Value::Identity(val) => ValueRef::Identity(val),
            &Value::Signature(val) => ValueRef::Signature(val),
            &Value::Lockbox(ref val) => ValueRef::Lockbox(val.as_ref()),
            &Value::Ext(ty, ref buf) => ValueRef::Ext(ty, buf.as_slice()),
        }
    }

    pub fn is_nil(&self) -> bool {
        if let Value::Nil = *self {
            true
        } else {
            false
        }
    }

    pub fn is_bool(&self) -> bool {
        self.as_bool().is_some()
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

    pub fn is_number(&self) -> bool {
        match *self {
            Value::Integer(..) | Value::F32(..) | Value::F64(..) => true,
            _ => false,
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

    pub fn is_ext(&self) -> bool {
        self.as_ext().is_some()
    }

    pub fn as_bool(&self) -> Option<bool> {
        if let Value::Boolean(val) = *self {
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
            val.as_str()
        } else {
            None
        }
    }

    pub fn as_slice(&self) -> Option<&[u8]> {
        if let Value::Binary(ref val) = *self {
            Some(val)
        } else if let Value::String(ref val) = *self {
            Some(val.as_bytes())
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

    pub fn as_map(&self) -> Option<&Vec<(Value, Value)>> {
        if let Value::Map(ref map) = *self {
            Some(map)
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

    pub fn as_ext(&self) -> Option<(i8, &[u8])> {
        if let Value::Ext(ty, ref buf) = *self {
            Some((ty, buf))
        } else {
            None
        }
    }
}

static NIL: Value = Value::Nil;
static NIL_REF: ValueRef<'static> = ValueRef::Nil;

impl ops::Index<usize> for Value {
    type Output = Value;

    fn index(&self, index: usize) -> &Value {
        self.as_array().and_then(|v| v.get(index)).unwrap_or(&NIL)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Boolean(v)
    }
}

impl From<u8> for Value {
    fn from(v: u8) -> Self {
        Value::Integer(From::from(v))
    }
}

impl From<u16> for Value {
    fn from(v: u16) -> Self {
        Value::Integer(From::from(v))
    }
}

impl From<u32> for Value {
    fn from(v: u32) -> Self {
        Value::Integer(From::from(v))
    }
}

impl From<u64> for Value {
    fn from(v: u64) -> Self {
        Value::Integer(From::from(v))
    }
}

impl From<usize> for Value {
    fn from(v: usize) -> Self {
        Value::Integer(From::from(v))
    }
}

impl From<i8> for Value {
    fn from(v: i8) -> Self {
        Value::Integer(From::from(v))
    }
}

impl From<i16> for Value {
    fn from(v: i16) -> Self {
        Value::Integer(From::from(v))
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::Integer(From::from(v))
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Integer(From::from(v))
    }
}

impl From<isize> for Value {
    fn from(v: isize) -> Self {
        Value::Integer(From::from(v))
    }
}

impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Value::F32(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::F64(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::String(Utf8String::from(v))
    }
}

impl<'a> From<&'a str> for Value {
    fn from(v: &str) -> Self {
        Value::String(Utf8String::from(v))
    }
}

impl<'a> From<Cow<'a, str>> for Value {
    fn from(v: Cow<'a, str>) -> Self {
        Value::String(Utf8String::from(v))
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value::Binary(v)
    }
}

impl<'a> From<&'a [u8]> for Value {
    fn from(v: &[u8]) -> Self {
        Value::Binary(v.into())
    }
}

impl<'a> From<Cow<'a, [u8]>> for Value {
    fn from(v: Cow<'a, [u8]>) -> Self {
        Value::Binary(v.into_owned())
    }
}

impl From<Vec<Value>> for Value {
    fn from(v: Vec<Value>) -> Self {
        Value::Array(v)
    }
}

impl From<Vec<(Value, Value)>> for Value {
    fn from(v: Vec<(Value, Value)>) -> Self {
        Value::Map(v)
    }
}

impl From<Timestamp> for Value {
    fn from(v: Timestamp) -> Self {
        Value::Timestamp(v)
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            Value::Nil => Display::fmt("nil", f),
            Value::Boolean(val) => write!(f, "{}", val),
            Value::Integer(ref val) => write!(f, "{}", val),
            Value::F32(val) => write!(f, "{}", val),
            Value::F64(val) => write!(f, "{}", val),
            Value::String(ref val) => write!(f, "{}", val),
            Value::Binary(ref val) => write!(f, "{:?}", val),
            Value::Array(ref vec) => {
                // TODO: This can be slower than naive implementation. Need benchmarks for more
                // information.
                let res = vec.iter()
                    .map(|val| format!("{}", val))
                    .collect::<Vec<String>>()
                    .join(", ");

                write!(f, "[{}]", res)
            }
            Value::Map(ref vec) => {
                try!(write!(f, "{{"));

                match vec.iter().take(1).next() {
                    Some(&(ref k, ref v)) => {
                        try!(write!(f, "{}: {}", k, v));
                    }
                    None => {
                        try!(write!(f, ""));
                    }
                }

                for &(ref k, ref v) in vec.iter().skip(1) {
                    try!(write!(f, ", {}: {}", k, v));
                }

                write!(f, "}}")
            }
            Value::Timestamp(ref val) => write!(f, "{}", val),
            Value::Hash(ref val) => write!(f, "<Hash({})>", val.get_version()),
            Value::Identity(ref val) => write!(f, "<Identity({})>", val.get_version()),
            Value::Signature(ref val) => write!(f, "<Signature({},{})>", val.get_identity_version(), val.get_hash_version()),
            Value::Lockbox(ref val) => write!(f, "<Lockbox({})>", val.get_version()),
            Value::Ext(ty, ref data) => {
                write!(f, "<ext({},{:?})>", ty, data)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ValueRef<'a> {
    /// Nil represents nil.
    Nil,
    /// Boolean represents true or false.
    Boolean(bool),
    /// Integer represents an integer.
    ///
    /// A value of an `Integer` object is limited from `-(2^63)` upto `(2^64)-1`.
    Integer(Integer),
    /// A 32-bit floating point number.
    F32(f32),
    /// A 64-bit floating point number.
    F64(f64),
    /// String extending Raw type represents a UTF-8 string.
    String(Utf8StringRef<'a>),
    /// Binary extending Raw type represents a byte array.
    Binary(&'a [u8]),
    /// Array represents a sequence of objects.
    Array(Vec<ValueRef<'a>>),
    /// Map represents key-value pairs of objects.
    Map(Vec<(ValueRef<'a>, ValueRef<'a>)>),
    /// Timestamp represents a UNIX timestamp with optional nanoseconds field.
    Timestamp(Timestamp),
    /// A cryptographic hash of data, usually another encoded Value.
    Hash(Hash),
    /// An identity is a public key that can be used to verify signatures and encrypt data.
    Identity(Identity),
    /// A signature can be appended to an array to sign that array's contents with a particular key. 
    /// Others can use the associated Identity to verify the correctness of the signature.
    Signature(Signature),
    /// A lockbox contains encrypted data. It can contain a secret Key, StreamKey, or Value.
    Lockbox(LockboxRef<'a>),
    /// Extended implements Extension interface: represents a tuple of type information and a byte
    /// array where type information is an integer whose meaning is defined by applications.
    Ext(i8, &'a [u8]),
}

impl<'a> ValueRef<'a> {
    /// Converts the current non-owning value to an owned Value.
    ///
    /// This is achieved by deep copying all underlying structures and borrowed buffers.
    ///
    /// # Panics
    ///
    /// Panics in unable to allocate memory to keep all internal structures and buffers.
    ///
    /// # Examples
    /// ```
    /// use rmpv::{Value, ValueRef};
    ///
    /// let val = ValueRef::Array(vec![
    ///    ValueRef::Nil,
    ///    ValueRef::from(42),
    ///    ValueRef::Array(vec![
    ///        ValueRef::from("le message"),
    ///    ])
    /// ]);
    ///
    /// let expected = Value::Array(vec![
    ///     Value::Nil,
    ///     Value::from(42),
    ///     Value::Array(vec![
    ///         Value::String("le message".into())
    ///     ])
    /// ]);
    ///
    /// assert_eq!(expected, val.to_owned());
    /// ```
    pub fn to_owned(&self) -> Value {
        match self {
            &ValueRef::Nil => Value::Nil,
            &ValueRef::Boolean(val) => Value::Boolean(val),
            &ValueRef::Integer(val) => Value::Integer(val),
            &ValueRef::F32(val) => Value::F32(val),
            &ValueRef::F64(val) => Value::F64(val),
            &ValueRef::String(val) => Value::String(val.into()),
            &ValueRef::Binary(val) => Value::Binary(val.to_vec()),
            &ValueRef::Array(ref val) => {
                Value::Array(val.iter().map(|v| v.to_owned()).collect())
            }
            &ValueRef::Map(ref val) => {
                Value::Map(val.iter().map(|&(ref k, ref v)| (k.to_owned(), v.to_owned())).collect())
            }
            &ValueRef::Timestamp(val) => Value::Timestamp(val),
            &ValueRef::Hash(val) => Value::Hash(val),
            &ValueRef::Identity(val) => Value::Identity(val),
            &ValueRef::Signature(val) => Value::Signature(val),
            &ValueRef::Lockbox(val) => Value::Lockbox(val.into()),
            &ValueRef::Ext(ty, buf) => Value::Ext(ty, buf.to_vec()),
        }
    }

    pub fn index(&self, index: usize) -> &ValueRef {
        self.as_array().and_then(|v| v.get(index)).unwrap_or(&NIL_REF)
    }

    /// If the `ValueRef` is an integer, return or cast it to a u64.
    /// Returns None otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use rmpv::ValueRef;
    ///
    /// assert_eq!(Some(42), ValueRef::from(42).as_u64());
    /// ```
    pub fn as_u64(&self) -> Option<u64> {
        match *self {
            ValueRef::Integer(ref n) => n.as_u64(),
            _ => None,
        }
    }

    /// If the `ValueRef` is an Array, returns the associated vector.
    /// Returns None otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use rmpv::ValueRef;
    ///
    /// let val = ValueRef::Array(vec![ValueRef::Nil, ValueRef::Boolean(true)]);
    ///
    /// assert_eq!(Some(&vec![ValueRef::Nil, ValueRef::Boolean(true)]), val.as_array());
    /// assert_eq!(None, ValueRef::Nil.as_array());
    /// ```
    pub fn as_array(&self) -> Option<&Vec<ValueRef>> {
        if let ValueRef::Array(ref array) = *self {
            Some(&*array)
        } else {
            None
        }
    }

    pub fn into_array(self) -> Option<Vec<ValueRef<'a>>> {
        if let ValueRef::Array(array) = self {
            Some(array)
        } else {
            None
        }
    }
}

impl<'a> From<u8> for ValueRef<'a> {
    fn from(v: u8) -> Self {
        ValueRef::Integer(From::from(v))
    }
}

impl<'a> From<u16> for ValueRef<'a> {
    fn from(v: u16) -> Self {
        ValueRef::Integer(From::from(v))
    }
}

impl<'a> From<u32> for ValueRef<'a> {
    fn from(v: u32) -> Self {
        ValueRef::Integer(From::from(v))
    }
}

impl<'a> From<u64> for ValueRef<'a> {
    fn from(v: u64) -> Self {
        ValueRef::Integer(From::from(v))
    }
}

impl<'a> From<usize> for ValueRef<'a> {
    fn from(v: usize) -> Self {
        ValueRef::Integer(From::from(v))
    }
}

impl<'a> From<i8> for ValueRef<'a> {
    fn from(v: i8) -> Self {
        ValueRef::Integer(From::from(v))
    }
}

impl<'a> From<i16> for ValueRef<'a> {
    fn from(v: i16) -> Self {
        ValueRef::Integer(From::from(v))
    }
}

impl<'a> From<i32> for ValueRef<'a> {
    fn from(v: i32) -> Self {
        ValueRef::Integer(From::from(v))
    }
}

impl<'a> From<i64> for ValueRef<'a> {
    fn from(v: i64) -> Self {
        ValueRef::Integer(From::from(v))
    }
}

impl<'a> From<isize> for ValueRef<'a> {
    fn from(v: isize) -> Self {
        ValueRef::Integer(From::from(v))
    }
}

impl<'a> From<f32> for ValueRef<'a> {
    fn from(v: f32) -> Self {
        ValueRef::F32(v)
    }
}

impl<'a> From<f64> for ValueRef<'a> {
    fn from(v: f64) -> Self {
        ValueRef::F64(v)
    }
}

impl<'a> From<&'a str> for ValueRef<'a> {
    fn from(v: &'a str) -> Self {
        ValueRef::String(Utf8StringRef::from(v))
    }
}

impl<'a> From<&'a [u8]> for ValueRef<'a> {
    fn from(v: &'a [u8]) -> Self {
        ValueRef::Binary(v.into())
    }
}

impl<'a> From<Vec<ValueRef<'a>>> for ValueRef<'a> {
    fn from(v: Vec<ValueRef<'a>>) -> Self {
        ValueRef::Array(v)
    }
}

impl<'a> From<Vec<(ValueRef<'a>, ValueRef<'a>)>> for ValueRef<'a> {
    fn from(v: Vec<(ValueRef<'a>, ValueRef<'a>)>) -> Self {
        ValueRef::Map(v)
    }
}

impl<'a> From<Timestamp> for ValueRef<'a> {
    fn from(v: Timestamp) -> Self {
        ValueRef::Timestamp(v)
    }
}

impl<'a> Display for ValueRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            ValueRef::Nil => write!(f, "nil"),
            ValueRef::Boolean(val) => write!(f, "{}", val),
            ValueRef::Integer(ref val) => write!(f, "{}", val),
            ValueRef::F32(val) => write!(f, "{}", val),
            ValueRef::F64(val) => write!(f, "{}", val),
            ValueRef::String(ref val) => write!(f, "{}", val),
            ValueRef::Binary(ref val) => write!(f, "{:?}", val),
            ValueRef::Array(ref vec) => {
                let res = vec.iter()
                    .map(|val| format!("{}", val))
                    .collect::<Vec<String>>()
                    .join(", ");

                write!(f, "[{}]", res)
            }
            ValueRef::Map(ref vec) => {
                try!(write!(f, "{{"));

                match vec.iter().take(1).next() {
                    Some(&(ref k, ref v)) => {
                        try!(write!(f, "{}: {}", k, v));
                    }
                    None => {
                        try!(write!(f, ""));
                    }
                }

                for &(ref k, ref v) in vec.iter().skip(1) {
                    try!(write!(f, ", {}: {}", k, v));
                }

                write!(f, "}}")
            }
            ValueRef::Timestamp(ref val) => write!(f, "{}", val),
            ValueRef::Hash(ref val) => write!(f, "{{Hash V{}}}", val.get_version()),
            ValueRef::Identity(ref val) => write!(f, "{{Identity V{}}}", val.get_version()),
            ValueRef::Signature(ref val) => write!(f, "{{Signature ID=V{}, Hash=V{}}}", val.get_identity_version(), val.get_hash_version()),
            ValueRef::Lockbox(ref val) => write!(f, "{{Lockbox V{}}}", val.get_version()),
            ValueRef::Ext(ty, ref data) => {
                write!(f, "[{}, {:?}]", ty, data)
            }
        }
    }
}
