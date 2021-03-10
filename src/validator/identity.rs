use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use crate::Identity;
use serde::{Deserialize, Serialize};

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct IdentityValidator {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<Identity>,
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<Identity>,
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
}

impl std::default::Default for IdentityValidator {
    fn default() -> Self {
        Self {
            comment: String::new(),
            in_list: Vec::new(),
            nin_list: Vec::new(),
            query: false,
        }
    }
}

impl IdentityValidator {
    pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
        let elem = parser
            .next()
            .ok_or_else(|| Error::FailValidate("Expected an Identity".to_string()))??;
        let elem = if let Element::Identity(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "Expected Identity, got {}",
                elem.name()
            )));
        };
        if !self.in_list.is_empty() && !self.in_list.iter().any(|v| *v == elem) {
            return Err(Error::FailValidate(
                "Identity is not on `in` list".to_string(),
            ));
        }
        if self.nin_list.iter().any(|v| *v == elem) {
            return Err(Error::FailValidate("Identity is on `nin` list".to_string()));
        }
        Ok(())
    }

    fn query_check_self(&self, other: &Self) -> bool {
        self.query || (other.in_list.is_empty() && other.nin_list.is_empty())
    }

    pub(crate) fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::Identity(other) => self.query_check_self(other),
            Validator::Multi(list) => list.iter().all(|other| match other {
                Validator::Identity(other) => self.query_check_self(other),
                _ => false,
            }),
            Validator::Any => true,
            _ => false,
        }
    }
}
