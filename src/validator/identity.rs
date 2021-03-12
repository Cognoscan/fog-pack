use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use crate::Identity;
use serde::{Deserialize, Serialize};

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}

/// Validator for a cryptographic [`Identity`][crate::Identity].
///
/// This validator will only pass an Identity value. Validation passes if:
///
/// - If the `in` list is not empty, the Identity must be among the ones in the list.
/// - The Identity must not be among the ones in the `nin` list.
///
/// # Defaults
///
/// Fields that aren't specified for the validator use their defaults instead. The defaults for
/// each field are:
///
/// - comment: ""
/// - in_list: empty
/// - nin_list: empty
/// - query: false
///
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct IdentityValidator {
    /// An optional comment explaining the validator.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    /// A vector of specific allowed values, stored under the `in` field. If empty, this vector is not checked against.
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<Identity>,
    /// A vector of specific unallowed values, stored under the `nin` field.
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<Identity>,
    /// If true, queries against matching spots may have values in the `in` or `nin` lists.
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
    /// Make a new validator with the default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a comment for the validator.
    pub fn comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = comment.into();
        self
    }

    /// Add a value to the `in` list.
    pub fn in_add(mut self, add: impl Into<Identity>) -> Self {
        self.in_list.push(add.into());
        self
    }

    /// Add a value to the `nin` list.
    pub fn nin_add(mut self, add: impl Into<Identity>) -> Self {
        self.nin_list.push(add.into());
        self
    }

    /// Set whether or not queries can use the `in` and `nin` lists.
    pub fn query(mut self, query: bool) -> Self {
        self.query = query;
        self
    }

    /// Build this into a [`Validator`] enum.
    pub fn build(self) -> Validator {
        Validator::Identity(self)
    }

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
