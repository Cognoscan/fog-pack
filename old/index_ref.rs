use std::fmt;
use std::ops;

use super::ValueRef;

pub trait IndexRef: private::Sealed {
    /// Return None if the key is not already in the array or object.
    #[doc(hidden)]
    fn index_into<'v>(&self, v: &'v ValueRef) -> Option<&'v ValueRef>;
}

impl IndexRef for usize {
    fn index_into<'v>(&self, v: &'v ValueRef) -> Option<&'v ValueRef> {
        match *v {
            ValueRef::Array(ref vec) => vec.get(*self),
            _ => None,
        }
    }
}

impl IndexRef for str {
    fn index_into<'v>(&self, v: &'v ValueRef) -> Option<&'v ValueRef> {
        match *v {
            ValueRef::Object(ref obj) => obj.get(self),
            _ => None,
        }
    }
}

impl IndexRef for String {
    fn index_into<'v>(&self, v: &'v ValueRef) -> Option<&'v ValueRef> {
        self[..].index_into(v)
    }
}

impl<'a, T: ?Sized> IndexRef for &'a T
where
    T: IndexRef
{
    fn index_into<'v>(&self, v: &'v ValueRef) -> Option<&'v ValueRef> {
        (**self).index_into(v)
    }
}

mod private {
    pub trait Sealed {}
    impl Sealed for usize {}
    impl Sealed for str {}
    impl Sealed for String {}
    impl<'a, T: ?Sized> Sealed for &'a T where T: Sealed {}
}

struct Type<'a, 'v>(&'a ValueRef<'v>);

impl<'a, 'v> fmt::Display for Type<'a, 'v> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match *self.0 {
            ValueRef::Null         => formatter.write_str("Null"),
            ValueRef::Boolean(_)   => formatter.write_str("Boolean"),
            ValueRef::Integer(_)   => formatter.write_str("Integer"),
            ValueRef::String(_)    => formatter.write_str("String"),
            ValueRef::F32(_)       => formatter.write_str("F32"),
            ValueRef::F64(_)       => formatter.write_str("F64"),
            ValueRef::Binary(_)    => formatter.write_str("Binary"),
            ValueRef::Array(_)     => formatter.write_str("Array"),
            ValueRef::Object(_)    => formatter.write_str("Object"),
            ValueRef::Hash(_)      => formatter.write_str("Hash"),
            ValueRef::Identity(_)  => formatter.write_str("Identity"),
            ValueRef::Lockbox(_)   => formatter.write_str("Lockbox"),
            ValueRef::Timestamp(_) => formatter.write_str("Timestamp"),
        }
    }
}

/*
impl<'v, 'a, I> ops::Index<I> for ValueRef<'a> where I: IndexRef
{
    type Output = ValueRef<'a>;
    fn index(&'v self, index: I) -> &ValueRef {
        static NULL: ValueRef = ValueRef::Null;
        index.index_into(self).unwrap_or(&NULL)
    }
}
*/
