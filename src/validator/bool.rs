use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::default::Default;

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BoolValidator {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    #[serde(skip_serializing_if = "is_false")]
    pub default: bool,
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<bool>,
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<bool>,
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
}

impl Default for BoolValidator {
    fn default() -> Self {
        Self {
            comment: String::new(),
            default: false,
            in_list: Vec::new(),
            nin_list: Vec::new(),
            query: false,
        }
    }
}

impl BoolValidator {
    pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
        let elem = parser
            .next()
            .ok_or(Error::FailValidate("Expected a boolean".to_string()))??;
        let elem = if let Element::Bool(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "Expected Bool, got {}",
                elem.name()
            )));
        };
        if self.in_list.len() > 0 {
            if !self.in_list.iter().any(|v| *v == elem) {
                return Err(Error::FailValidate(
                    "Boolean is not on `in` list".to_string(),
                ));
            }
        }
        if self.nin_list.iter().any(|v| *v == elem) {
            return Err(Error::FailValidate("Boolean is on `nin` list".to_string()));
        }
        Ok(())
    }

    pub(crate) fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::Bool(other) => {
                self.query || (other.in_list.is_empty() && other.nin_list.is_empty())
            }
            Validator::Any => true,
            _ => false,
        }
    }
}
