use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use crate::*;
use serde::{Deserialize, Serialize};
use std::default::Default;

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}
#[inline]
fn u64_is_zero(v: &u64) -> bool {
    *v == 0
}
#[inline]
fn int_is_zero(v: &Integer) -> bool {
    v.as_u64().and_then(|v| Some(v == 0)).unwrap_or(false)
}
#[inline]
fn int_is_max(v: &Integer) -> bool {
    v.as_u64()
        .and_then(|v| Some(v == u64::MAX))
        .unwrap_or(false)
}
#[inline]
fn int_is_min(v: &Integer) -> bool {
    v.as_i64()
        .and_then(|v| Some(v == i64::MIN))
        .unwrap_or(false)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct IntValidator {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    #[serde(skip_serializing_if = "u64_is_zero")]
    pub bits_clr: u64,
    #[serde(skip_serializing_if = "u64_is_zero")]
    pub bits_set: u64,
    #[serde(skip_serializing_if = "int_is_zero")]
    pub default: Integer,
    #[serde(skip_serializing_if = "int_is_max")]
    pub max: Integer,
    #[serde(skip_serializing_if = "int_is_min")]
    pub min: Integer,
    #[serde(skip_serializing_if = "is_false")]
    pub ex_max: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub ex_min: bool,
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<Integer>,
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<Integer>,
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub bit: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub ord: bool,
}

impl Default for IntValidator {
    fn default() -> Self {
        Self {
            comment: String::new(),
            bits_clr: 0,
            bits_set: 0,
            default: Integer::default(),
            max: Integer::max_value(),
            min: Integer::min_value(),
            ex_max: false,
            ex_min: false,
            in_list: Vec::new(),
            nin_list: Vec::new(),
            query: false,
            bit: false,
            ord: false,
        }
    }
}

impl IntValidator {
    pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
        let elem = parser
            .next()
            .ok_or(Error::FailValidate("Expected a integer".to_string()))??;
        let int = if let Element::Int(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "Expected Int, got {}",
                elem.name()
            )));
        };
        let bits = int.as_bits();
        if self.in_list.len() > 0 {
            if !self.in_list.iter().any(|v| *v == int) {
                return Err(Error::FailValidate(
                    "Integer is not on `in` list".to_string(),
                ));
            }
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

    pub(crate) fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::Int(other) => {
                (self.query || (other.in_list.is_empty() && other.nin_list.is_empty()))
                    && (self.bit || (other.bits_clr == 0 && other.bits_set == 0))
                    && (self.ord
                        || (!other.ex_min
                            && !other.ex_max
                            && int_is_max(&other.max)
                            && int_is_min(&other.min)))
            }
            Validator::Any => true,
            _ => false,
        }
    }
}
