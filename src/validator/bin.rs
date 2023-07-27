use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::default::Default;

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}

#[inline]
fn bytes_empty(v: &ByteBuf) -> bool {
    v.is_empty()
}

#[inline]
fn u32_is_zero(v: &u32) -> bool {
    *v == 0
}

#[inline]
fn u32_is_max(v: &u32) -> bool {
    *v == u32::MAX
}

/// Validator for byte sequences.
///
/// This validator type will only pass binary values (a sequence of bytes). A binary sequence can
/// also be treated as a little-endian arbitrary-length unsigned integer. Validation passes if:
///
/// - The bits set in `bits_clr` are cleared in the byte sequence.
/// - The bits set in `bits_set` are set in the byte sequence.
/// - If `max` has 1 or more bytes, the value is less than the maximum in `max`, or equal to it if
///     `ex_max` is not set to true.
/// - The value is greater than the minimum in `min`, or equal to it if `ex_min` is not set to true.
/// - The value's length in bytes is less than or equal to the value in `max_len`.
/// - The value's length in bytes is greater than or equal to the value in `min_len`.
/// - If the `in` list is not empty, the value must be among the values in the list.
/// - The value must not be among the values in the `nin` list.
///
/// # Defaults
///
/// Fields that aren't specified for the validator use their defaults instead. The defaults for
/// each field are:
///
/// - comment: ""
/// - bits_clr: empty
/// - bits_set: empty
/// - max: empty
/// - min: empty
/// - ex_max: false
/// - ex_min: false
/// - max_len: u32::MAX
/// - min_len: 0
/// - in_list: empty
/// - nin_list: empty
/// - query: false
/// - bit: false
/// - ord: false
/// - size: false
///
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BinValidator {
    /// An optional comment explaining the validator.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    /// A byte sequence used as a bit field. Any bits set in it must be cleared in an allowed
    /// value.
    #[serde(skip_serializing_if = "bytes_empty")]
    pub bits_clr: ByteBuf,
    /// A byte sequence used as a bit field. Any bits set in it must be set in an allowed
    /// value.
    #[serde(skip_serializing_if = "bytes_empty")]
    pub bits_set: ByteBuf,
    /// The maximum allowed value, as a little-endian arbitrary-length unsigned integer. If no
    /// bytes are present, there is no maximum.
    #[serde(skip_serializing_if = "bytes_empty")]
    pub max: ByteBuf,
    /// The minimum allowed value, as a little-endian arbitrary-length unsigned integer.
    #[serde(skip_serializing_if = "bytes_empty")]
    pub min: ByteBuf,
    /// Changes `max` into an exclusive maximum.
    #[serde(skip_serializing_if = "is_false")]
    pub ex_max: bool,
    /// Changes `min` into an exclusive maximum.
    #[serde(skip_serializing_if = "is_false")]
    pub ex_min: bool,
    /// Set the maximum allowed number of bytes.
    #[serde(skip_serializing_if = "u32_is_max")]
    pub max_len: u32,
    /// Set the minimum allowed number of bytes.
    #[serde(skip_serializing_if = "u32_is_zero")]
    pub min_len: u32,
    /// A vector of specific allowed values, stored under the `in` field. If empty, this vector is not checked against.
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<ByteBuf>,
    /// A vector of specific unallowed values, stored under the `nin` field.
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<ByteBuf>,
    /// If true, queries against matching spots may have values in the `in` or `nin` lists.
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
    /// If true, queries against matching spots may set the `bits_clr` and `bits_set` values to be
    /// non-zero.
    #[serde(skip_serializing_if = "is_false")]
    pub bit: bool,
    /// If true, queries against matching spots may set the `max`, `min`, `ex_max`, and `ex_min`
    /// values to non-defaults.
    #[serde(skip_serializing_if = "is_false")]
    pub ord: bool,
    /// If true, queries against matching spots may set the `min_len` and `max_len` values to
    /// non-defaults.
    #[serde(skip_serializing_if = "is_false")]
    pub size: bool,
}

impl Default for BinValidator {
    fn default() -> Self {
        Self {
            comment: String::new(),
            bits_clr: ByteBuf::new(),
            bits_set: ByteBuf::new(),
            ex_max: false,
            ex_min: false,
            max: ByteBuf::new(),
            min: ByteBuf::new(),
            max_len: u32::MAX,
            min_len: u32::MIN,
            in_list: Vec::new(),
            nin_list: Vec::new(),
            query: false,
            bit: false,
            ord: false,
            size: false,
        }
    }
}

impl BinValidator {
    /// Make a new validator with the default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a comment for the validator.
    pub fn comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = comment.into();
        self
    }

    /// Choose which bits must be set.
    pub fn bits_set(mut self, bits_set: impl Into<Vec<u8>>) -> Self {
        self.bits_set = ByteBuf::from(bits_set);
        self
    }

    /// Choose which bits must be cleared.
    pub fn bits_clr(mut self, bits_clr: impl Into<Vec<u8>>) -> Self {
        self.bits_clr = ByteBuf::from(bits_clr);
        self
    }

    /// Set the maximum allowed value.
    pub fn max(mut self, max: impl Into<Vec<u8>>) -> Self {
        self.max = ByteBuf::from(max);
        self
    }

    /// Set the minimum allowed value.
    pub fn min(mut self, min: impl Into<Vec<u8>>) -> Self {
        self.min = ByteBuf::from(min);
        self
    }

    /// Set whether or or not `max` is an exclusive maximum.
    pub fn ex_max(mut self, ex_max: bool) -> Self {
        self.ex_max = ex_max;
        self
    }

    /// Set whether or or not `min` is an exclusive maximum.
    pub fn ex_min(mut self, ex_min: bool) -> Self {
        self.ex_min = ex_min;
        self
    }

    /// Set the maximum number of allowed bytes.
    pub fn max_len(mut self, max_len: u32) -> Self {
        self.max_len = max_len;
        self
    }

    /// Set the minimum number of allowed bytes.
    pub fn min_len(mut self, min_len: u32) -> Self {
        self.min_len = min_len;
        self
    }

    /// Add a value to the `in` list.
    pub fn in_add(mut self, add: impl Into<Vec<u8>>) -> Self {
        self.in_list.push(ByteBuf::from(add));
        self
    }

    /// Add a value to the `nin` list.
    pub fn nin_add(mut self, add: impl Into<Vec<u8>>) -> Self {
        self.nin_list.push(ByteBuf::from(add));
        self
    }

    /// Set whether or not queries can use the `in` and `nin` lists.
    pub fn query(mut self, query: bool) -> Self {
        self.query = query;
        self
    }

    /// Set whether or not queries can use the `bits_clr` and `bits_set` values.
    pub fn bit(mut self, bit: bool) -> Self {
        self.bit = bit;
        self
    }

    /// Set whether or not queries can use the `max`, `min`, `ex_max`, and `ex_min` values.
    pub fn ord(mut self, ord: bool) -> Self {
        self.ord = ord;
        self
    }

    /// Set whether or not queries can use the `max_len` and `min_len` values.
    pub fn size(mut self, size: bool) -> Self {
        self.size = size;
        self
    }

    /// Build this into a [`Validator`] enum.
    pub fn build(self) -> Validator {
        Validator::Bin(Box::new(self))
    }

    pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
        use std::iter::repeat;

        // Get element
        let elem = parser
            .next()
            .ok_or_else(|| Error::FailValidate("expected binary data".to_string()))??;
        let val = if let Element::Bin(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "expected Bin, got {}",
                elem.name()
            )));
        };

        // Length checks
        if (val.len() as u32) > self.max_len {
            return Err(Error::FailValidate(
                "Bin is longer than max_len".to_string(),
            ));
        }
        if (val.len() as u32) < self.min_len {
            return Err(Error::FailValidate(
                "Bin is shorter than min_len".to_string(),
            ));
        }

        // Bit checks
        if self
            .bits_set
            .iter()
            .zip(val.iter().chain(repeat(&0u8)))
            .any(|(bit, val)| (bit & val) != *bit)
        {
            return Err(Error::FailValidate(
                "Bin does not have all required bits set".to_string(),
            ));
        }
        if self
            .bits_clr
            .iter()
            .zip(val.iter().chain(repeat(&0u8)))
            .any(|(bit, val)| (bit & val) != 0)
        {
            return Err(Error::FailValidate(
                "Bin does not have all required bits cleared".to_string(),
            ));
        }

        // Assist functions for comparison
        use std::cmp::Ordering;
        fn compare(lhs: &[u8], rhs: &[u8]) -> Ordering {
            match lhs.len().cmp(&rhs.len()) {
                Ordering::Equal => Iterator::cmp(lhs.iter().rev(), rhs.iter().rev()),
                other => other,
            }
        }
        fn trim(val: &[u8]) -> &[u8] {
            let trim_amount = val.iter().rev().take_while(|v| **v == 0).count();
            &val[0..(val.len() - trim_amount)]
        }

        // Range checks
        if !self.max.is_empty() || !self.min.is_empty() || self.ex_min {
            let trimmed_val = trim(val);
            let max_pass = match (self.max.is_empty(), self.ex_max) {
                (true, _) => true,
                (false, true) => compare(trimmed_val, trim(&self.max)) == Ordering::Less,
                (false, false) => compare(trimmed_val, trim(&self.max)) != Ordering::Greater,
            };

            let min_pass = match (self.min.is_empty(), self.ex_min) {
                (true, true) => !trimmed_val.is_empty(), // at least zero
                (true, false) => true,                   // Can be anything, 0 on up
                (false, true) => compare(trimmed_val, trim(&self.min)) == Ordering::Greater,
                (false, false) => compare(trimmed_val, trim(&self.min)) != Ordering::Less,
            };

            if !max_pass {
                return Err(Error::FailValidate(
                    "Bin greater than maximum allowed".to_string(),
                ));
            }
            if !min_pass {
                return Err(Error::FailValidate(
                    "Bin less than minimum allowed".to_string(),
                ));
            }
        }

        // in/nin checks
        if !self.in_list.is_empty() && !self.in_list.iter().any(|v| *v == val) {
            return Err(Error::FailValidate("Bin is not on `in` list".to_string()));
        }
        if self.nin_list.iter().any(|v| *v == val) {
            return Err(Error::FailValidate("Bin is on `nin` list".to_string()));
        }

        Ok(())
    }

    fn query_check_self(&self, other: &Self) -> bool {
        (self.query || (other.in_list.is_empty() && other.nin_list.is_empty()))
            && (self.bit || (other.bits_set.is_empty() && other.bits_clr.is_empty()))
            && (self.ord
                || (!other.ex_min && !other.ex_max && other.min.is_empty() && other.max.is_empty()))
            && (self.size || (u32_is_max(&other.max_len) && u32_is_zero(&other.min_len)))
    }

    pub(crate) fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::Bin(other) => self.query_check_self(other),
            Validator::Multi(list) => list.iter().all(|other| match other {
                Validator::Bin(other) => self.query_check_self(other),
                _ => false,
            }),
            Validator::Any => true,
            _ => false,
        }
    }
}
