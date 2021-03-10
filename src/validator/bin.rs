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
fn usize_is_zero(v: &usize) -> bool {
    *v == 0
}

#[inline]
fn usize_is_max(v: &usize) -> bool {
    *v == usize::MAX
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BinValidator {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    #[serde(skip_serializing_if = "bytes_empty")]
    pub default: ByteBuf,
    #[serde(skip_serializing_if = "bytes_empty")]
    pub bits_clr: ByteBuf,
    #[serde(skip_serializing_if = "bytes_empty")]
    pub bits_set: ByteBuf,
    #[serde(skip_serializing_if = "is_false")]
    pub ex_max: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub ex_min: bool,
    #[serde(skip_serializing_if = "bytes_empty")]
    pub max: ByteBuf,
    #[serde(skip_serializing_if = "bytes_empty")]
    pub min: ByteBuf,
    #[serde(skip_serializing_if = "usize_is_max")]
    pub max_len: usize,
    #[serde(skip_serializing_if = "usize_is_zero")]
    pub min_len: usize,
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<ByteBuf>,
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<ByteBuf>,
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub bit: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub ord: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub size: bool,
}

impl Default for BinValidator {
    fn default() -> Self {
        Self {
            comment: String::new(),
            default: ByteBuf::new(),
            bits_clr: ByteBuf::new(),
            bits_set: ByteBuf::new(),
            ex_max: false,
            ex_min: false,
            max: ByteBuf::new(),
            min: ByteBuf::new(),
            max_len: usize::MAX,
            min_len: usize::MIN,
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
    pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
        use std::iter::repeat;

        // Get element
        let elem = parser
            .next()
            .ok_or(Error::FailValidate("expected binary data".to_string()))??;
        let val = if let Element::Bin(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "expected Bin, got {}",
                elem.name()
            )));
        };

        // Length checks
        if val.len() > self.max_len {
            return Err(Error::FailValidate(
                "Bin is longer than max_len".to_string(),
            ));
        }
        if val.len() < self.min_len {
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
        fn trim<'a>(val: &'a [u8]) -> &'a [u8] {
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
        if self.in_list.len() > 0 {
            if !self.in_list.iter().any(|v| *v == val) {
                return Err(Error::FailValidate("Bin is not on `in` list".to_string()));
            }
        }
        if self.nin_list.iter().any(|v| *v == val) {
            return Err(Error::FailValidate("Bin is on `nin` list".to_string()));
        }

        todo!()
    }

    fn query_check_self(&self, other: &Self) -> bool {
        (self.query || (other.in_list.is_empty() && other.nin_list.is_empty()))
            && (self.bit || (other.bits_set.is_empty() && other.bits_clr.is_empty()))
            && (self.ord
                || (!other.ex_min && !other.ex_max && other.min.is_empty() && other.max.is_empty()))
            && (self.size || (usize_is_max(&other.max_len) && usize_is_zero(&other.min_len)))
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
