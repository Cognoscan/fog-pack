use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use crate::*;
use educe::Educe;
use serde::{Deserialize, Serialize};

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}

#[inline]
fn u64_is_zero(v: &u64) -> bool {
    *v == 0
}

#[inline]
fn int_is_max(v: &Integer) -> bool {
    v.as_u64().map(|v| v == u64::MAX).unwrap_or(false)
}

#[inline]
fn int_is_min(v: &Integer) -> bool {
    v.as_i64().map(|v| v == i64::MIN).unwrap_or(false)
}

/// Validator for integer values.
///
/// This validator type will only pass integers. Validation passes if:
///
/// - The bits set in `bits_clr` are cleared in the integer
/// - The bits set in `bits_set` are set in the integer
/// - The integer is less than the maximum in `max`, or equal to it if `ex_max` is not set to true.
/// - The integer is greater than the minimum in `min`, or equal to it if `ex_min` is not set to true.
/// - If the `in` list is not empty, the integer must be among the integers in it.
/// - The integer must not be among the integers in the `nin` list.
///
/// # Defaults
///
/// Fields that aren't specified for the validator use their defaults instead. The defaults for
/// each field are:
///
/// - comment: ""
/// - bits_clr: 0
/// - bits_set: 0
/// - max: u64::MAX
/// - min: i64::MIN
/// - ex_max: false
/// - ex_min: false
/// - in_list: empty
/// - nin_list: empty
/// - query: false
/// - bit: false
/// - ord: false
///
#[derive(Educe, Clone, Debug, Serialize, Deserialize)]
#[educe(PartialEq, Default)]
#[serde(deny_unknown_fields, default)]
pub struct IntValidator {
    /// An optional comment explaining the validator.
    #[educe(PartialEq(ignore))]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    /// An unsigned 64-bit integers used as a bit field. Any bits set in it must be cleared in an
    /// allowed value.
    #[serde(skip_serializing_if = "u64_is_zero")]
    pub bits_clr: u64,
    /// An unsigned 64-bit integers used as a bit field. Any bits set in it must be set in an
    /// allowed value.
    #[serde(skip_serializing_if = "u64_is_zero")]
    pub bits_set: u64,
    /// The maximum allowed integer value.
    #[educe(Default(expression = Integer::max_value()))]
    #[serde(skip_serializing_if = "int_is_max")]
    pub max: Integer,
    /// The minimum allowed integer value.
    #[educe(Default(expression = Integer::min_value()))]
    #[serde(skip_serializing_if = "int_is_min")]
    pub min: Integer,
    /// Changes `max` into an exclusive maximum.
    #[serde(skip_serializing_if = "is_false")]
    pub ex_max: bool,
    /// Changes `min` into an exclusive maximum.
    #[serde(skip_serializing_if = "is_false")]
    pub ex_min: bool,
    /// A vector of specific allowed values, stored under the `in` field. If empty, this vector is not checked against.
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<Integer>,
    /// A vector of specific unallowed values, stored under the `nin` field.
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<Integer>,
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
}

impl IntValidator {
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
    pub fn bits_set(mut self, bits_set: u64) -> Self {
        self.bits_set = bits_set;
        self
    }

    /// Choose which bits must be cleared.
    pub fn bits_clr(mut self, bits_clr: u64) -> Self {
        self.bits_clr = bits_clr;
        self
    }

    /// Set the maximum allowed value.
    pub fn max(mut self, max: impl Into<Integer>) -> Self {
        self.max = max.into();
        self
    }

    /// Set the minimum allowed value.
    pub fn min(mut self, min: impl Into<Integer>) -> Self {
        self.min = min.into();
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

    /// Add a value to the `in` list.
    pub fn in_add(mut self, add: impl Into<Integer>) -> Self {
        self.in_list.push(add.into());
        self
    }

    /// Add a value to the `nin` list.
    pub fn nin_add(mut self, add: impl Into<Integer>) -> Self {
        self.nin_list.push(add.into());
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

    /// Build this into a [`Validator`] enum.
    pub fn build(self) -> Validator {
        Validator::Int(Box::new(self))
    }

    pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
        let elem = parser
            .next()
            .ok_or_else(|| Error::FailValidate("Expected a integer".to_string()))??;
        let int = if let Element::Int(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "Expected Int, got {}",
                elem.name()
            )));
        };
        let bits = int.as_bits();
        if !self.in_list.is_empty() && !self.in_list.iter().any(|v| *v == int) {
            return Err(Error::FailValidate(
                "Integer is not on `in` list".to_string(),
            ));
        }
        if self.nin_list.iter().any(|v| *v == int) {
            return Err(Error::FailValidate("Integer is on `nin` list".to_string()));
        }
        if (bits & self.bits_clr) != 0 {
            return Err(Error::FailValidate(
                "Integer does not have all required bits cleared".to_string(),
            ));
        }
        if (bits & self.bits_set) != self.bits_set {
            return Err(Error::FailValidate(
                "Integer does not have all required bits set".to_string(),
            ));
        }
        match int.cmp(&self.max) {
            std::cmp::Ordering::Equal if self.ex_max => {
                return Err(Error::FailValidate(
                    "Integer greater than maximum allowed".to_string(),
                ))
            }
            std::cmp::Ordering::Greater => {
                return Err(Error::FailValidate(
                    "Integer greater than maximum allowed".to_string(),
                ))
            }
            _ => (),
        }
        match int.cmp(&self.min) {
            std::cmp::Ordering::Equal if self.ex_min => {
                return Err(Error::FailValidate(
                    "Integer less than minimum allowed".to_string(),
                ))
            }
            std::cmp::Ordering::Less => {
                return Err(Error::FailValidate(
                    "Integer less than minimum allowed".to_string(),
                ))
            }
            _ => (),
        }
        Ok(())
    }

    fn query_check_int(&self, other: &Self) -> bool {
        (self.query || (other.in_list.is_empty() && other.nin_list.is_empty()))
            && (self.bit || (other.bits_clr == 0 && other.bits_set == 0))
            && (self.ord
                || (!other.ex_min
                    && !other.ex_max
                    && int_is_max(&other.max)
                    && int_is_min(&other.min)))
    }

    pub(crate) fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::Int(other) => self.query_check_int(other),
            Validator::Multi(list) => list.iter().all(|other| match other {
                Validator::Int(other) => self.query_check_int(other),
                _ => false,
            }),
            Validator::Any => true,
            _ => false,
        }
    }
}
