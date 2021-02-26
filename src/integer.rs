use std::cmp;
use std::cmp::Ordering;
use std::fmt::{self, Debug, Display, LowerHex, UpperHex};
use std::ops;

use num_traits::NumCast;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum IntPriv {
    /// Always non-less than zero.
    PosInt(u64),
    /// Always less than zero.
    NegInt(i64),
}

/// Represents a fog-pack integer, whether signed or unsigned.
///
/// A `Value` or `ValueRef` that contains integer can be constructed using `From` trait.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Integer {
    n: IntPriv,
}

impl Integer {
    /// Minimum possible integer that can be represented. Equivalent to `i64::min_value()`.
    pub fn min_value() -> Integer {
        Integer {
            n: IntPriv::NegInt(i64::min_value()),
        }
    }

    /// Maximum possible integer that can be represented. Equivalent to `u64::max_value()`.
    pub fn max_value() -> Integer {
        Integer {
            n: IntPriv::PosInt(u64::max_value()),
        }
    }

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

    /// Forcibly casts the value to u64 without modification.
    #[inline]
    pub fn as_bits(&self) -> u64 {
        match self.n {
            IntPriv::PosInt(n) => n,
            IntPriv::NegInt(n) => n as u64,
        }
    }
}

pub(crate) fn get_int_internal(val: &Integer) -> IntPriv {
    val.n
}

impl std::default::Default for Integer {
    fn default() -> Self {
        Self {
            n: IntPriv::PosInt(0),
        }
    }
}

impl cmp::Ord for Integer {
    fn cmp(&self, other: &Integer) -> Ordering {
        match (self.n, other.n) {
            (IntPriv::NegInt(lhs), IntPriv::NegInt(ref rhs)) => lhs.cmp(rhs),
            (IntPriv::NegInt(_), IntPriv::PosInt(_)) => Ordering::Less,
            (IntPriv::PosInt(_), IntPriv::NegInt(_)) => Ordering::Greater,
            (IntPriv::PosInt(lhs), IntPriv::PosInt(ref rhs)) => lhs.cmp(rhs),
        }
    }
}

impl cmp::PartialOrd for Integer {
    fn partial_cmp(&self, other: &Integer) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl ops::Add<i64> for Integer {
    type Output = Integer;

    fn add(self, other: i64) -> Integer {
        match self.n {
            IntPriv::PosInt(lhs) => {
                if other >= 0 {
                    Integer::from(lhs + (other as u64))
                } else if lhs >= (1u64 << 63) {
                    Integer::from(lhs.wrapping_add(other as u64))
                } else {
                    Integer::from((lhs as i64) + other)
                }
            }
            IntPriv::NegInt(lhs) => Integer::from(lhs + other),
        }
    }
}

impl ops::Sub<i64> for Integer {
    type Output = Integer;

    fn sub(self, other: i64) -> Integer {
        match self.n {
            IntPriv::PosInt(lhs) => {
                if other < 0 {
                    Integer::from(lhs.wrapping_sub(other as u64))
                } else if lhs >= (1u64 << 63) {
                    Integer::from(lhs - (other as u64))
                } else {
                    Integer::from((lhs as i64) - other)
                }
            }
            IntPriv::NegInt(lhs) => Integer::from(lhs - other),
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

impl UpperHex for Integer {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        UpperHex::fmt(&self.as_bits(), fmt)
    }
}

impl LowerHex for Integer {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        LowerHex::fmt(&self.as_bits(), fmt)
    }
}

macro_rules! impl_from_unsigned {
    ($t: ty) => {
        impl From<$t> for Integer {
            fn from(n: $t) -> Self {
                Integer {
                    n: IntPriv::PosInt(n as u64),
                }
            }
        }
    };
}

macro_rules! impl_from_signed {
    ($t: ty) => {
        impl From<$t> for Integer {
            fn from(n: $t) -> Self {
                if n < 0 {
                    Integer {
                        n: IntPriv::NegInt(n as i64),
                    }
                } else {
                    Integer {
                        n: IntPriv::PosInt(n as u64),
                    }
                }
            }
        }
    };
}

impl_from_unsigned!(u8);
impl_from_unsigned!(u16);
impl_from_unsigned!(u32);
impl_from_unsigned!(u64);
impl_from_unsigned!(usize);
impl_from_signed!(i8);
impl_from_signed!(i16);
impl_from_signed!(i32);
impl_from_signed!(i64);
impl_from_signed!(isize);

use std::convert::TryFrom;

macro_rules! impl_try_from {
    ($t: ty) => {
        impl TryFrom<Integer> for $t {
            type Error = Integer;
            fn try_from(v: Integer) -> Result<Self, Self::Error> {
                match v.n {
                    IntPriv::PosInt(n) => TryFrom::try_from(n).map_err(|_| v),
                    IntPriv::NegInt(n) => TryFrom::try_from(n).map_err(|_| v),
                }
            }
        }
    };
}

impl_try_from!(u8);
impl_try_from!(u16);
impl_try_from!(u32);
impl_try_from!(u64);
impl_try_from!(usize);
impl_try_from!(i8);
impl_try_from!(i16);
impl_try_from!(i32);
impl_try_from!(i64);
impl_try_from!(isize);

use serde::{
    de::{Deserialize, Deserializer},
    ser::{Serialize, Serializer},
};

impl Serialize for Integer {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.n {
            IntPriv::PosInt(v) => serializer.serialize_u64(v),
            IntPriv::NegInt(v) => serializer.serialize_i64(v),
        }
    }
}

impl<'de> Deserialize<'de> for Integer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct IntVisitor;
        impl<'de> serde::de::Visitor<'de> for IntVisitor {
            type Value = Integer;

            fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
                write!(fmt, "an integer")
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(Integer::from(v))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(Integer::from(v))
            }
        }

        deserializer.deserialize_any(IntVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add() {
        let x = Integer::min_value();
        let y = i64::max_value();
        assert_eq!(x + y, Integer::from(-1));
        let y = 1i64;
        assert_eq!(x + y, Integer::from(i64::min_value() + 1));
        let x = Integer::from(1u64 << 63);
        assert_eq!(x + y, Integer::from((1u64 << 63) + 1));
        let x = Integer::from((1u64 << 63) - 1);
        assert_eq!(x + y, Integer::from(1u64 << 63));

        let x = Integer::max_value();
        let y = i64::min_value();
        assert_eq!(x + y, Integer::from(u64::max_value() >> 1));
        let y = -1i64;
        assert_eq!(x + y, Integer::from(u64::max_value() - 1));
        let x = Integer::from(1u64 << 63);
        assert_eq!(x + y, Integer::from((1u64 << 63) - 1));
        let x = Integer::from((1u64 << 63) - 1);
        assert_eq!(x + y, Integer::from((1u64 << 63) - 2));
    }

    #[test]
    fn sub() {
        let x = Integer::min_value();
        let y = i64::min_value();
        assert_eq!(x - y, Integer::from(0));
        let y = -1i64;
        assert_eq!(x - y, Integer::from(i64::min_value() + 1));
        let x = Integer::from(1u64 << 63);
        assert_eq!(x - y, Integer::from((1u64 << 63) + 1));
        let x = Integer::from((1u64 << 63) - 1);
        assert_eq!(x - y, Integer::from(1u64 << 63));

        let x = Integer::max_value();
        let y = i64::max_value();
        assert_eq!(x - y, Integer::from(1u64 << 63));
        let y = 1i64;
        assert_eq!(x - y, Integer::from(u64::max_value() - 1));
        let x = Integer::from(1u64 << 63);
        assert_eq!(x - y, Integer::from((1u64 << 63) - 1));
        let x = Integer::from((1u64 << 63) - 1);
        assert_eq!(x - y, Integer::from((1u64 << 63) - 2));
    }
}
