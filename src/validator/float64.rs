use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}
#[inline]
fn is_nan(v: &f64) -> bool {
    v.is_nan()
}

/// Validator for 64-bit floating-point values.
///
/// This validator will only pass f64 values. Validation passes if:
///
/// - If `max` is a number, that the value is less than the maximum in `max`, or equal to it if
///     `ex_max` is not set to true.
/// - If `min` is a number, that the value is greater than the minimum in `min`, or equal to it if
///     `ex_min` is not set to true.
/// - If the `in` list is not empty, the value must be among the values in it. This performs an
///     exact bit-wise match.
/// - The value must not be among the values in the `nin` list. This performas an exact bit-wise
///     match.
///
/// # Defaults
///
/// Fields that aren't specified for the validator use their defaults instead. The defaults for
/// each field are:
///
/// - comment: ""
/// - max: NaN
/// - min: NaN
/// - ex_max: false
/// - ex_min: false
/// - in_list: empty
/// - nin_list: empty
/// - query: false
/// - ord: false
///
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct F64Validator {
    /// An optional comment explaining the validator.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    /// The maximum allowed f64 value. If NaN, it is ignored.
    #[serde(skip_serializing_if = "is_nan")]
    pub max: f64,
    /// The minimum allowed f64 value. If NaN, it is ignored.
    #[serde(skip_serializing_if = "is_nan")]
    pub min: f64,
    /// Changes `max` into an exclusive maximum.
    #[serde(skip_serializing_if = "is_false")]
    pub ex_max: bool,
    /// Changes `min` into an exclusive maximum.
    #[serde(skip_serializing_if = "is_false")]
    pub ex_min: bool,
    /// A vector of specific allowed values, stored under the `in` field. If empty, this vector is not checked against.
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<f64>,
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    /// A vector of specific unallowed values, stored under the `nin` field.
    pub nin_list: Vec<f64>,
    /// If true, queries against matching spots may have values in the `in` or `nin` lists.
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
    /// If true, queries against matching spots may set the `max`, `min`, `ex_max`, and `ex_min`
    /// values to non-defaults.
    #[serde(skip_serializing_if = "is_false")]
    pub ord: bool,
}

impl std::default::Default for F64Validator {
    fn default() -> Self {
        Self {
            comment: String::new(),
            max: f64::NAN,
            min: f64::NAN,
            ex_max: false,
            ex_min: false,
            in_list: Vec::new(),
            nin_list: Vec::new(),
            query: false,
            ord: false,
        }
    }
}

impl F64Validator {
    /// Make a new validator with the default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a comment for the validator.
    pub fn comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = comment.into();
        self
    }

    /// Set the maximum allowed value.
    pub fn max(mut self, max: f64) -> Self {
        self.max = max;
        self
    }

    /// Set the minimum allowed value.
    pub fn min(mut self, min: f64) -> Self {
        self.min = min;
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
    pub fn in_add(mut self, add: f64) -> Self {
        self.in_list.push(add);
        self
    }

    /// Add a value to the `nin` list.
    pub fn nin_add(mut self, add: f64) -> Self {
        self.nin_list.push(add);
        self
    }

    /// Set whether or not queries can use the `in` and `nin` lists.
    pub fn query(mut self, query: bool) -> Self {
        self.query = query;
        self
    }

    /// Set whether or not queries can use the `max`, `min`, `ex_max`, and `ex_min` values.
    pub fn ord(mut self, ord: bool) -> Self {
        self.ord = ord;
        self
    }

    pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
        let elem = parser
            .next()
            .ok_or_else(|| Error::FailValidate("Expected a f64".to_string()))??;
        let elem = if let Element::F64(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "Expected F64, got {}",
                elem.name()
            )));
        };
        let bytes = elem.to_ne_bytes();
        if !self.in_list.is_empty() && !self.in_list.iter().any(|v| v.to_ne_bytes() == bytes) {
            return Err(Error::FailValidate("F64 is not on `in` list".to_string()));
        }
        if self.nin_list.iter().any(|v| v.to_ne_bytes() == bytes) {
            return Err(Error::FailValidate("F64 is on `nin` list".to_string()));
        }
        if !self.max.is_nan() && ((self.ex_max && elem >= self.max) || (elem > self.max)) {
            return Err(Error::FailValidate(
                "F64 greater than maximum allowed".to_string(),
            ));
        }
        if !self.min.is_nan() && ((self.ex_min && elem <= self.min) || (elem < self.min)) {
            return Err(Error::FailValidate(
                "F64 less than maximum allowed".to_string(),
            ));
        }
        Ok(())
    }

    fn query_check_f64(&self, other: &Self) -> bool {
        (self.query || (other.in_list.is_empty() && other.nin_list.is_empty()))
            && (self.ord
                || (!other.ex_min && !other.ex_max && other.min.is_nan() && other.max.is_nan()))
    }

    pub(crate) fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::F64(other) => self.query_check_f64(other),
            Validator::Multi(list) => list.iter().all(|other| match other {
                Validator::F64(other) => self.query_check_f64(other),
                _ => false,
            }),
            Validator::Any => true,
            _ => false,
        }
    }
}
