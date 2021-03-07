use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}
#[inline]
fn f64_is_zero(v: &f64) -> bool {
    *v == 0.0
}
#[inline]
fn is_nan(v: &f64) -> bool {
    v.is_nan()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct F64Validator {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    #[serde(skip_serializing_if = "f64_is_zero")]
    pub default: f64,
    #[serde(skip_serializing_if = "is_nan")]
    pub max: f64,
    #[serde(skip_serializing_if = "is_nan")]
    pub min: f64,
    #[serde(skip_serializing_if = "is_false")]
    pub ex_max: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub ex_min: bool,
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<f64>,
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<f64>,
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub ord: bool,
}

impl std::default::Default for F64Validator {
    fn default() -> Self {
        Self {
            comment: String::new(),
            default: f64::default(),
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
    pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
        let elem = parser
            .next()
            .ok_or(Error::FailValidate("Expected a f64".to_string()))??;
        let elem = if let Element::F64(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "Expected F64, got {}",
                elem.name()
            )));
        };
        let bytes = elem.to_ne_bytes();
        if self.in_list.len() > 0 {
            if !self.in_list.iter().any(|v| v.to_ne_bytes() == bytes) {
                return Err(Error::FailValidate("F64 is not on `in` list".to_string()));
            }
        }
        if self.nin_list.iter().any(|v| v.to_ne_bytes() == bytes) {
            return Err(Error::FailValidate("F64 is on `nin` list".to_string()));
        }
        if !self.max.is_nan() {
            if self.ex_max {
                if elem >= self.max {
                    return Err(Error::FailValidate(
                        "F64 greater than maximum allowed".to_string(),
                    ));
                }
            } else {
                if elem > self.max {
                    return Err(Error::FailValidate(
                        "F64 greater than maximum allowed".to_string(),
                    ));
                }
            }
        }
        if !self.min.is_nan() {
            if self.ex_min {
                if elem <= self.min {
                    return Err(Error::FailValidate(
                        "F64 less than maximum allowed".to_string(),
                    ));
                }
            } else {
                if elem < self.min {
                    return Err(Error::FailValidate(
                        "F64 less than maximum allowed".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    fn query_check_f64(&self, other: &Self) -> bool {
        (self.query || (other.in_list.is_empty() && other.nin_list.is_empty()))
            && (self.ord
                || (!other.ex_min
                    && !other.ex_max
                    && other.min.is_nan()
                    && other.max.is_nan()))
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
