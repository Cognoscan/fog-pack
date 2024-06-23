use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}

/// Validator for boolean values.
///
/// This validator type will only pass booleans. Validation only passes if the value also
/// matches the value in `val`, if one is present.
///
/// # Defaults
///
/// Fields that aren't specified for the validator use their defaults instead. The defaults for
/// each field are:
/// - comment: ""
/// - in_list: empty
/// - nin_list: empty
/// - query: false
///
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BoolValidator {
    /// An optional comment explaining the validator.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    /// An optional boolean this must match.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub val: Option<bool>,
    /// If true, queries against matching spots may have the `val` field set.
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
}

impl BoolValidator {
    /// Make a new validator with the default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a comment for the validator.
    pub fn comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = comment.into();
        self
    }

    /// Set a required value
    pub fn set_val(mut self, val: bool) -> Self {
        self.val = Some(val);
        self
    }

    /// Set whether or not queries can use the `in` and `nin` lists.
    pub fn query(mut self, query: bool) -> Self {
        self.query = query;
        self
    }

    /// Build this into a [`Validator`] enum.
    pub fn build(self) -> Validator {
        Validator::Bool(Box::new(self))
    }

    pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
        let elem = parser
            .next()
            .ok_or_else(|| Error::FailValidate("Expected a boolean".to_string()))??;
        let elem = if let Element::Bool(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "Expected Bool, got {}",
                elem.name()
            )));
        };
        if let Some(val) = self.val {
            if val != elem {
                return Err(Error::FailValidate(
                    "Boolean does not match the required value".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn query_check_bool(&self, other: &Self) -> bool {
        self.query || other.val.is_none()
    }

    pub(crate) fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::Bool(other) => self.query_check_bool(other),
            Validator::Multi(list) => list.iter().all(|other| match other {
                Validator::Bool(other) => self.query_check_bool(other),
                _ => false,
            }),
            Validator::Any => true,
            _ => false,
        }
    }
}
