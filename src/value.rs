use std::borrow::Cow;
use std::fmt::{self, Display};
use std::collections::BTreeMap;

use super::{Integer, Hash, Identity, Lockbox, Timestamp};

use super::Index;

/// Value stores any type of MessagePack value, and can be encoded into a MessagePack byte sequence
#[derive(Debug)]
pub enum Value {
    Null,
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

impl Value {

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

    pub fn as_obj(&self) -> Option<&BTreeMap<String, Value>> {
        if let Value::Object(ref obj) = *self {
            Some(obj)
        } else {
            None
        }
    }

    pub fn as_obj_mut(&mut self) -> Option<&mut BTreeMap<String, Value>> {
        match *self {
            Value::Object(ref mut obj) => Some(obj),
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

    pub fn as_lockbox(&self) -> Option<&Lockbox> {
        if let Value::Lockbox(ref lock) = *self {
            Some(lock)
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

    pub fn to_string(self) -> Option<String> {
        if let Value::String(val) = self {
            Some(val)
        } else {
            None
        }
    }

    fn indent(f: &mut fmt::Formatter, n: usize) -> Result<(), fmt::Error> {
        for _ in 0..n {
            write!(f, "  ")?;
        }
        Ok(())
    }

    pub fn pretty_print(&self, cur_indent: usize, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        //let mut ids = Vec::new();
        //let mut hashes = Vec::new();
        //let mut owners = Vec::new();
        //let mut bin = Vec::new();

        match *self {
            Value::Null => Display::fmt("null", f),
            Value::Boolean(val) => write!(f, "{}", val),
            Value::Integer(ref val) => write!(f, "{}", val),
            Value::F32(val) => write!(f, "{}", val),
            Value::F64(val) => write!(f, "{}", val),
            Value::String(ref val) => {
                format_string(val, f)
            }
            Value::Binary(ref _val) => write!(f, "\"<Bin>\""),
            Value::Array(ref vec) => {
                write!(f, "[\n")?;
                match vec.iter().take(1).next() {
                    Some(ref v) => {
                        Value::indent(f, cur_indent+1)?;
                        v.pretty_print(cur_indent+1, f)?;
                    }
                    None => {
                        write!(f, "")?;
                    }
                }
                for val in vec.iter().skip(1) {
                    write!(f, ",\n")?;
                    Value::indent(f, cur_indent+1)?;
                    val.pretty_print(cur_indent+1, f)?;
                }
                write!(f, "\n")?;
                Value::indent(f, cur_indent)?;
                write!(f, "]")
            }
            Value::Object(ref obj) => {
                write!(f, "{{\n")?;

                match obj.iter().take(1).next() {
                    Some((ref k, ref v)) => {
                        Value::indent(f, cur_indent+1)?;
                        format_string(k, f)?;
                        write!(f, ": ")?;
                        v.pretty_print(cur_indent+1, f)?;
                    }
                    None => {
                        write!(f, "")?;
                    }
                }

                for (ref k, ref v) in obj.iter().skip(1) {
                    write!(f, ",\n")?;
                    Value::indent(f, cur_indent+1)?;
                    format_string(k, f)?;
                    write!(f, ": ")?;
                    v.pretty_print(cur_indent+1, f)?;
                }
                write!(f, "\n")?;
                Value::indent(f, cur_indent)?;
                write!(f, "}}")
            }
            Value::Timestamp(ref val) => write!(f, "{}", val),
            Value::Hash(ref val) => {
                if val.get_version() == 0 {
                    write!(f, "\"<Hash(Null)>\"")
                }
                else {
                    write!(f, "\"<Hash>\"")
                }
            }
            Value::Identity(_) => write!(f, "\"<Identity>\""),
            Value::Lockbox(_) => write!(f, "\"<Lockbox>\""),
        }
    }

    pub fn get<I: Index>(&self, index: I) -> Option<&Value> {
        index.index_into(self)
    }

    pub fn get_mut<I: Index>(&mut self, index: I) -> Option<&mut Value> {
        index.index_into_mut(self)
    }

}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Boolean(v)
    }
}

impl From<Integer> for Value {
    fn from(v: Integer) -> Self {
        Value::Integer(v)
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
        Value::String(v)
    }
}

impl<'a> From<&'a str> for Value {
    fn from(v: &str) -> Self {
        Value::String(v.to_string())
    }
}

impl<'a> From<Cow<'a, str>> for Value {
    fn from(v: Cow<'a, str>) -> Self {
        Value::String(v.to_string())
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

impl From<BTreeMap<String, Value>> for Value {
    fn from(v: BTreeMap<String, Value>) -> Self {
        Value::Object(v)
    }
}

impl From<Hash> for Value {
    fn from(v: Hash) -> Self {
        Value::Hash(v)
    }
}

impl From<Identity> for Value {
    fn from(v: Identity) -> Self {
        Value::Identity(v)
    }
}

impl From<Lockbox> for Value {
    fn from(v: Lockbox) -> Self {
        Value::Lockbox(v)
    }
}

impl From<Timestamp> for Value {
    fn from(v: Timestamp) -> Self {
        Value::Timestamp(v)
    }
}

fn format_string(val: &String, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    if val.starts_with("<") {
        write!(f, "\"<{}\"", val)
    }
    else {
        write!(f, "\"{}\"", val)
    }
}

fn format_str(val: &str, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    if val.starts_with("<") {
        write!(f, "\"<{}\"", val)
    }
    else {
        write!(f, "\"{}\"", val)
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.pretty_print(0, f)
    }
}

/// ValueRef stores a reference to a decoded MessagePack byte sequence
#[derive(Debug)]
pub enum ValueRef<'a> {
    Null,
    Boolean(bool),
    Integer(Integer),
    String(&'a str),
    F32(f32),
    F64(f64),
    Binary(&'a [u8]),
    Array(Vec<ValueRef<'a>>),
    Object(BTreeMap<&'a str, ValueRef<'a>>),
    Hash(Hash),
    Identity(Identity),
    Lockbox(Lockbox),
    Timestamp(Timestamp),
}

impl<'a> ValueRef<'a> {

    pub fn is_nil(&self) -> bool {
        if let ValueRef::Null = *self {
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
        if let ValueRef::Integer(ref v) = *self {
            v.is_i64()
        } else {
            false
        }
    }

    pub fn is_u64(&self) -> bool {
        if let ValueRef::Integer(ref v) = *self {
            v.is_u64()
        } else {
            false
        }
    }

    pub fn is_f32(&self) -> bool {
        if let ValueRef::F32(..) = *self {
            true
        } else {
            false
        }
    }

    pub fn is_f64(&self) -> bool {
        if let ValueRef::F64(..) = *self {
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
        if let ValueRef::Boolean(val) = *self {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_int(&self) -> Option<Integer> {
        if let ValueRef::Integer(val) = *self {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            ValueRef::Integer(ref n) => n.as_i64(),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match *self {
            ValueRef::Integer(ref n) => n.as_u64(),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            ValueRef::Integer(ref n) => n.as_f64(),
            ValueRef::F32(n) => Some(From::from(n)),
            ValueRef::F64(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let ValueRef::String(ref val) = *self {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_slice(&self) -> Option<&[u8]> {
        if let ValueRef::Binary(ref val) = *self {
            Some(val)
        } else {
            None
        }
    }

    pub fn as_array(&self) -> Option<&Vec<ValueRef>> {
        if let ValueRef::Array(ref array) = *self {
            Some(&*array)
        } else {
            None
        }
    }

    pub fn as_obj(&self) -> Option<&BTreeMap<&str, ValueRef>> {
        if let ValueRef::Object(ref obj) = *self {
            Some(obj)
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

    pub fn as_id(&self) -> Option<&Identity> {
        if let ValueRef::Identity(ref id) = *self {
            Some(id)
        } else {
            None
        }
    }

    pub fn as_lockbox(&self) -> Option<&Lockbox> {
        if let ValueRef::Lockbox(ref lock) = *self {
            Some(lock)
        } else {
            None
        }
    }

    pub fn as_timestamp(&self) -> Option<Timestamp> {
        if let ValueRef::Timestamp(time) = *self {
            Some(time)
        } else {
            None
        }
    }

    fn indent(f: &mut fmt::Formatter, n: usize) -> Result<(), fmt::Error> {
        for _ in 0..n {
            write!(f, "  ")?;
        }
        Ok(())
    }

    pub fn pretty_print(&self, cur_indent: usize, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        //let mut ids = Vec::new();
        //let mut hashes = Vec::new();
        //let mut owners = Vec::new();
        //let mut bin = Vec::new();

        match *self {
            ValueRef::Null => Display::fmt("null", f),
            ValueRef::Boolean(val) => write!(f, "{}", val),
            ValueRef::Integer(ref val) => write!(f, "{}", val),
            ValueRef::F32(val) => write!(f, "{}", val),
            ValueRef::F64(val) => write!(f, "{}", val),
            ValueRef::String(ref val) => {
                format_str(val, f)
            }
            ValueRef::Binary(ref _val) => write!(f, "\"<Bin>\""),
            ValueRef::Array(ref vec) => {
                write!(f, "[\n")?;
                match vec.iter().take(1).next() {
                    Some(ref v) => {
                        ValueRef::indent(f, cur_indent+1)?;
                        v.pretty_print(cur_indent+1, f)?;
                    }
                    None => {
                        write!(f, "")?;
                    }
                }
                for val in vec.iter().skip(1) {
                    write!(f, ",\n")?;
                    ValueRef::indent(f, cur_indent+1)?;
                    val.pretty_print(cur_indent+1, f)?;
                }
                write!(f, "\n")?;
                ValueRef::indent(f, cur_indent)?;
                write!(f, "]")
            }
            ValueRef::Object(ref obj) => {
                write!(f, "{{\n")?;

                match obj.iter().take(1).next() {
                    Some((ref k, ref v)) => {
                        ValueRef::indent(f, cur_indent+1)?;
                        format_str(k, f)?;
                        write!(f, ": ")?;
                        v.pretty_print(cur_indent+1, f)?;
                    }
                    None => {
                        write!(f, "")?;
                    }
                }

                for (ref k, ref v) in obj.iter().skip(1) {
                    write!(f, ",\n")?;
                    ValueRef::indent(f, cur_indent+1)?;
                    format_str(k, f)?;
                    write!(f, ": ")?;
                    v.pretty_print(cur_indent+1, f)?;
                }
                write!(f, "\n")?;
                ValueRef::indent(f, cur_indent)?;
                write!(f, "}}")
            }
            ValueRef::Timestamp(ref val) => write!(f, "{}", val),
            ValueRef::Hash(ref val) => {
                if val.get_version() == 0 {
                    write!(f, "\"<Hash(Null)>\"")
                }
                else {
                    write!(f, "\"<Hash>\"")
                }
            }
            ValueRef::Identity(_) => write!(f, "\"<Identity>\""),
            ValueRef::Lockbox(_) => write!(f, "\"<Lockbox>\""),
        }
    }

    //pub fn get<I: Index>(&self, index: I) -> Option<&ValueRef> {
    //    index.index_into(self)
    //}

    //pub fn get_mut<I: Index>(&mut self, index: I) -> Option<&mut ValueRef> {
    //    index.index_into_mut(self)
    //}

}

impl<'a> Display for ValueRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.pretty_print(0, f)
    }
}





