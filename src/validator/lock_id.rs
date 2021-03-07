use crate::LockId;
use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct LockIdValidator {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<LockId>,
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<LockId>,
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
}

impl std::default::Default for LockIdValidator {
    fn default() -> Self {
        Self {
            comment: String::new(),
            in_list: Vec::new(),
            nin_list: Vec::new(),
            query: false,
        }
    }
}

impl LockIdValidator {
    pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
        let elem = parser
            .next()
            .ok_or(Error::FailValidate("Expected a LockId".to_string()))??;
        let elem = if let Element::LockId(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "Expected LockId, got {}",
                elem.name()
            )));
        };
        if self.in_list.len() > 0 {
            if !self.in_list.iter().any(|v| *v == elem) {
                return Err(Error::FailValidate(
                    "LockId is not on `in` list".to_string(),
                ));
            }
        }
        if self.nin_list.iter().any(|v| *v == elem) {
            return Err(Error::FailValidate("LockId is on `nin` list".to_string()));
        }
        Ok(())
    }

    fn query_check_self(&self, other: &Self) -> bool {
        self.query || (other.in_list.is_empty() && other.nin_list.is_empty())
    }

    pub(crate) fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::LockId(other) => self.query_check_self(other),
            Validator::Multi(list) => list.iter().all(|other| match other {
                Validator::LockId(other) => self.query_check_self(other),
                _ => false,
            }),
            Validator::Any => true,
            _ => false,
        }
    }
}
