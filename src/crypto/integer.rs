use std::fmt::{self, Debug, Display};

use num_traits::NumCast;

#[derive(Copy, Clone, Debug, PartialEq)]
enum IntPriv {
    /// Always non-less than zero.
    PosInt(u64),
    /// Always less than zero.
    NegInt(i64),
}

/// Represents a MessagePack integer, whether signed or unsigned.
///
/// A `Value` or `ValueRef` that contains integer can be constructed using `From` trait.
#[derive(Copy, Clone, PartialEq)]
pub struct Integer {
    n: IntPriv,
}

impl Integer {
    /// Returns `true` if the integer can be represented as `i64`.
    #[inline]
    pub fn is_i64(&self) -> bool {
        match self.n {
            IntPriv::PosInt(n) => n <= ::std::i64::MAX as u64,
            IntPriv::NegInt(..) => true,
        }
    }

    /// Returns `true` if the integer can be represented as `u64`.
    #[inline]
    pub fn is_u64(&self) -> bool {
        match self.n {
            IntPriv::PosInt(..) => true,
            IntPriv::NegInt(..) => false,
        }
    }

    /// Returns the integer represented as `i64` if possible, or else `None`.
    #[inline]
    pub fn as_i64(&self) -> Option<i64> {
        match self.n {
            IntPriv::PosInt(n) => NumCast::from(n),
            IntPriv::NegInt(n) => Some(n),
        }
    }

    /// Returns the integer represented as `u64` if possible, or else `None`.
    #[inline]
    pub fn as_u64(&self) -> Option<u64> {
        match self.n {
            IntPriv::PosInt(n) => Some(n),
            IntPriv::NegInt(n) => NumCast::from(n),
        }
    }

    /// Returns the integer represented as `f64` if possible, or else `None`.
    #[inline]
    pub fn as_f64(&self) -> Option<f64> {
        match self.n {
            IntPriv::PosInt(n) => NumCast::from(n),
            IntPriv::NegInt(n) => NumCast::from(n),
        }
    }
}

impl Debug for Integer {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        Debug::fmt(&self.n, fmt)
    }
}

impl Display for Integer {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self.n {
            IntPriv::PosInt(v) => Display::fmt(&v, fmt),
            IntPriv::NegInt(v) => Display::fmt(&v, fmt),
        }
    }
}

impl From<u8> for Integer {
    fn from(n: u8) -> Self {
        Integer { n: IntPriv::PosInt(n as u64) }
    }
}

impl From<u16> for Integer {
    fn from(n: u16) -> Self {
        Integer { n: IntPriv::PosInt(n as u64) }
    }
}

impl From<u32> for Integer {
    fn from(n: u32) -> Self {
        Integer { n: IntPriv::PosInt(n as u64) }
    }
}

impl From<u64> for Integer {
    fn from(n: u64) -> Self {
        Integer { n: IntPriv::PosInt(n as u64) }
    }
}

impl From<usize> for Integer {
    fn from(n: usize) -> Self {
        Integer { n: IntPriv::PosInt(n as u64) }
    }
}

impl From<i8> for Integer {
    fn from(n: i8) -> Self {
        if n < 0 {
            Integer { n: IntPriv::NegInt(n as i64) }
        } else {
            Integer { n: IntPriv::PosInt(n as u64) }
        }
    }
}

impl From<i16> for Integer {
    fn from(n: i16) -> Self {
        if n < 0 {
            Integer { n: IntPriv::NegInt(n as i64) }
        } else {
            Integer { n: IntPriv::PosInt(n as u64) }
        }
    }
}

impl From<i32> for Integer {
    fn from(n: i32) -> Self {
        if n < 0 {
            Integer { n: IntPriv::NegInt(n as i64) }
        } else {
            Integer { n: IntPriv::PosInt(n as u64) }
        }
    }
}

impl From<i64> for Integer {
    fn from(n: i64) -> Self {
        if n < 0 {
            Integer { n: IntPriv::NegInt(n as i64) }
        } else {
            Integer { n: IntPriv::PosInt(n as u64) }
        }
    }
}

impl From<isize> for Integer {
    fn from(n: isize) -> Self {
        if n < 0 {
            Integer { n: IntPriv::NegInt(n as i64) }
        } else {
            Integer { n: IntPriv::PosInt(n as u64) }
        }
    }
}
