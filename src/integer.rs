use std::fmt::{self, Debug, Display};
use std::cmp;
use std::cmp::Ordering;
use std::ops;

use num_traits::NumCast;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IntPriv {
    /// Always non-less than zero.
    PosInt(u64),
    /// Always less than zero.
    NegInt(i64),
}

/// Represents a MessagePack integer, whether signed or unsigned.
///
/// A `Value` or `ValueRef` that contains integer can be constructed using `From` trait.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Integer {
    n: IntPriv,
}

impl Integer {
    /// Minimum possible integer that can be represented. Equivalent to `i64::min_value()`.
    pub fn min_value() -> Integer {
        Integer { n: IntPriv::NegInt(i64::min_value()) }
    }

    /// Maximum possible integer that can be represented. Equivalent to `u64::max_value()`.
    pub fn max_value() -> Integer {
        Integer { n: IntPriv::PosInt(u64::max_value()) }
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

pub fn get_int_internal(val: &Integer) -> IntPriv {
    val.n
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
                }
                else {
                    if lhs >= (1u64 << 63) {
                        Integer::from(lhs.wrapping_add(other as u64))
                    }
                    else {
                        Integer::from((lhs as i64) + other)
                    }
                }
            },
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
                }
                else {
                    if lhs >= (1u64 << 63) {
                        Integer::from(lhs - (other as u64))
                    }
                    else {
                        Integer::from((lhs as i64) - other)
                    }
                }
            },
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add() {
        let x = Integer::min_value();
        let y = i64::max_value();
        assert_eq!(x + y, Integer::from(-1));
        let y = 1i64;
        assert_eq!(x + y, Integer::from(i64::min_value()+1));
        let x = Integer::from(1u64 << 63);
        assert_eq!(x + y, Integer::from((1u64 << 63)+1));
        let x = Integer::from((1u64 << 63)-1);
        assert_eq!(x + y, Integer::from(1u64 << 63));

        let x = Integer::max_value();
        let y = i64::min_value();
        assert_eq!(x + y, Integer::from(u64::max_value() >> 1));
        let y = -1i64;
        assert_eq!(x + y, Integer::from(u64::max_value()-1));
        let x = Integer::from(1u64 << 63);
        assert_eq!(x + y, Integer::from((1u64 << 63)-1));
        let x = Integer::from((1u64 << 63)-1);
        assert_eq!(x + y, Integer::from((1u64 << 63)-2));
    }

    #[test]
    fn sub() {
        let x = Integer::min_value();
        let y = i64::min_value();
        assert_eq!(x - y, Integer::from(0));
        let y = -1i64;
        assert_eq!(x - y, Integer::from(i64::min_value()+1));
        let x = Integer::from(1u64 << 63);
        assert_eq!(x - y, Integer::from((1u64 << 63)+1));
        let x = Integer::from((1u64 << 63)-1);
        assert_eq!(x - y, Integer::from(1u64 << 63));


        let x = Integer::max_value();
        let y = i64::max_value();
        assert_eq!(x - y, Integer::from(1u64 << 63));
        let y = 1i64;
        assert_eq!(x - y, Integer::from(u64::max_value()-1));
        let x = Integer::from(1u64 << 63);
        assert_eq!(x - y, Integer::from((1u64 << 63)-1));
        let x = Integer::from((1u64 << 63)-1);
        assert_eq!(x - y, Integer::from((1u64 << 63)-2));
    }
}

