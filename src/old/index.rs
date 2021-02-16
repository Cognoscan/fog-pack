use std::fmt;
use std::ops;
use std::collections::BTreeMap;

use super::Value;

pub trait Index: private::Sealed {
    /// Return None if the key is not already in the array or object.
    #[doc(hidden)]
    fn index_into<'v>(&self, v: &'v Value) -> Option<&'v Value>;

    /// Return None if the key is not already in the array or object.
    #[doc(hidden)]
    fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value>;

    /// Panic if array index is out of bounds. If the key is not yet in the object, insert it with 
    /// a null value. Panic if Value is a type that cannot be indexed, except if Value is null, in 
    /// which case it will be treated as an empty object.
    #[doc(hidden)]
    fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value;
}

impl Index for usize {
    fn index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        match *v {
            Value::Array(ref vec) => vec.get(*self),
            _ => None,
        }
    }
    fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
        match *v {
            Value::Array(ref mut vec) => vec.get_mut(*self),
            _ => None,
        }
    }
    fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value {
        match *v {
            Value::Array(ref mut vec) => {
                let len = vec.len();
                vec.get_mut(*self).unwrap_or_else(|| {
                    panic!(
                        "cannot access index {} of fogpack of length {}",
                        self, len
                    )
                })
            }
            _ => panic!("cannot access index {} of fogpack {}", self, Type(v)),
        }
    }
}

impl Index for str {
    fn index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        match *v {
            Value::Object(ref obj) => obj.get(self),
            _ => None,
        }
    }
    fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
        match *v {
            Value::Object(ref mut obj) => obj.get_mut(self),
            _ => None,
        }
    }
    fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value {
        if let Value::Null = *v {
            *v = Value::Object(BTreeMap::new());
        }
        match *v {
            Value::Object(ref mut obj) => {
                obj.entry(self.to_owned()).or_insert(Value::Null)
            }
            _ => panic!("cannot access field {:?} of fogpack {}", self, Type(v)),
        }
    }
}

impl Index for String {
    fn index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        self[..].index_into(v)
    }
    fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
        self[..].index_into_mut(v)
    }
    fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value {
        self[..].index_or_insert(v)
    }
}

impl<'a, T: ?Sized> Index for &'a T
where
    T: Index
{
    fn index_into<'v>(&self, v: &'v Value) -> Option<&'v Value> {
        (**self).index_into(v)
    }
    fn index_into_mut<'v>(&self, v: &'v mut Value) -> Option<&'v mut Value> {
        (**self).index_into_mut(v)
    }
    fn index_or_insert<'v>(&self, v: &'v mut Value) -> &'v mut Value {
        (**self).index_or_insert(v)
    }
}

mod private {
    pub trait Sealed {}
    impl Sealed for usize {}
    impl Sealed for str {}
    impl Sealed for String {}
    impl<'a, T: ?Sized> Sealed for &'a T where T: Sealed {}
}

struct Type<'a>(&'a Value);

impl<'a> fmt::Display for Type<'a> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match *self.0 {
            Value::Null         => formatter.write_str("Null"),
            Value::Boolean(_)   => formatter.write_str("Boolean"),
            Value::Integer(_)   => formatter.write_str("Integer"),
            Value::String(_)    => formatter.write_str("String"),
            Value::F32(_)       => formatter.write_str("F32"),
            Value::F64(_)       => formatter.write_str("F64"),
            Value::Binary(_)    => formatter.write_str("Binary"),
            Value::Array(_)     => formatter.write_str("Array"),
            Value::Object(_)    => formatter.write_str("Object"),
            Value::Hash(_)      => formatter.write_str("Hash"),
            Value::Identity(_)  => formatter.write_str("Identity"),
            Value::Lockbox(_)   => formatter.write_str("Lockbox"),
            Value::Timestamp(_) => formatter.write_str("Timestamp"),
        }
    }
}

impl<I> ops::Index<I> for Value where I: Index
{
    type Output = Value;
    fn index(&self, index: I) -> &Value {
        static NULL: Value = Value::Null;
        index.index_into(self).unwrap_or(&NULL)
    }
}

impl<I> ops::IndexMut<I> for Value where I: Index
{
    fn index_mut(&mut self, index: I) -> &mut Value {
        index.index_or_insert(self)
    }
}





