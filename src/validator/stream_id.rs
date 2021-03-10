use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use crate::StreamId;
use serde::{Deserialize, Serialize};

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct StreamIdValidator {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<StreamId>,
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<StreamId>,
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
}

impl std::default::Default for StreamIdValidator {
    fn default() -> Self {
        Self {
            comment: String::new(),
            in_list: Vec::new(),
            nin_list: Vec::new(),
            query: false,
        }
    }
}

impl StreamIdValidator {
    pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
        let elem = parser
            .next()
            .ok_or_else(|| Error::FailValidate("Expected a StreamId".to_string()))??;
        let elem = if let Element::StreamId(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "Expected StreamId, got {}",
                elem.name()
            )));
        };
        if !self.in_list.is_empty() && !self.in_list.iter().any(|v| *v == elem) {
            return Err(Error::FailValidate(
                "StreamId is not on `in` list".to_string(),
            ));
        }
        if self.nin_list.iter().any(|v| *v == elem) {
            return Err(Error::FailValidate("StreamId is on `nin` list".to_string()));
        }
        Ok(())
    }

    fn query_check_self(&self, other: &Self) -> bool {
        self.query || (other.in_list.is_empty() && other.nin_list.is_empty())
    }

    pub(crate) fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::StreamId(other) => self.query_check_self(other),
            Validator::Multi(list) => list.iter().all(|other| match other {
                Validator::StreamId(other) => self.query_check_self(other),
                _ => false,
            }),
            Validator::Any => true,
            _ => false,
        }
    }
}
