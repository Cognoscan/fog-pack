use super::*;
use crate::element::*;
use crate::Timestamp;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::default::Default;

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}

const ZERO_TIME: Timestamp = Timestamp::zero();
const MIN_TIME: Timestamp = Timestamp::min_value();
const MAX_TIME: Timestamp = Timestamp::max_value();

#[inline]
fn time_is_zero(v: &Timestamp) -> bool {
    *v == ZERO_TIME
}

#[inline]
fn time_is_min(v: &Timestamp) -> bool {
    *v == MIN_TIME
}

#[inline]
fn time_is_max(v: &Timestamp) -> bool {
    *v == MAX_TIME
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct TimeValidator {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    #[serde(skip_serializing_if = "time_is_zero")]
    pub default: Timestamp,
    #[serde(skip_serializing_if = "time_is_max")]
    pub max: Timestamp,
    #[serde(skip_serializing_if = "time_is_min")]
    pub min: Timestamp,
    #[serde(skip_serializing_if = "is_false")]
    pub ex_max: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub ex_min: bool,
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<Timestamp>,
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<Timestamp>,
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub ord: bool,
}

impl Default for TimeValidator {
    fn default() -> Self {
        Self {
            comment: String::new(),
            default: ZERO_TIME,
            max: MAX_TIME,
            min: MIN_TIME,
            ex_max: false,
            ex_min: false,
            in_list: Vec::new(),
            nin_list: Vec::new(),
            query: false,
            ord: false,
        }
    }
}

impl TimeValidator {
    pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
        let elem = parser
            .next()
            .ok_or(Error::FailValidate("Expected a timestamp".to_string()))??;
        let val = if let Element::Timestamp(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "Expected Time, got {}",
                elem.name()
            )));
        };

        // Range checks
        let max_pass = if self.ex_max {
            val < self.max
        }
        else {
            val <= self.max
        };
        let min_pass = if self.ex_min {
            val > self.min
        }
        else {
            val >= self.min
        };
        if !max_pass {
            return Err(Error::FailValidate("Timestamp greater than maximum allowed".to_string()));
        }
        if !min_pass {
            return Err(Error::FailValidate("Timestamp less than minimum allowed".to_string()));
        }

        // in/nin checks
        if self.in_list.len() > 0 {
            if !self.in_list.iter().any(|v| *v == val) {
                return Err(Error::FailValidate(
                        "Timestamp is not on `in` list".to_string()
                ));
            }
        }
        if self.nin_list.iter().any(|v| *v == val) {
            return Err(Error::FailValidate("Timestamp is on `nin` list".to_string()));
        }

        Ok(())
    }

    fn query_check_self(&self, other: &Self) -> bool {
        (self.query || (other.in_list.is_empty() && other.nin_list.is_empty()))
            && (self.ord
                || (!other.ex_min
                    && !other.ex_max
                    && time_is_min(&other.min)
                    && time_is_max(&other.max)))
    }

    pub(crate) fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::Time(other) => self.query_check_self(other),
            Validator::Multi(list) => list.iter().all(|other| match other {
                Validator::Time(other) => self.query_check_self(other),
                _ => false,
            }),
            Validator::Any => true,
            _ => false,
        }
    }
}

