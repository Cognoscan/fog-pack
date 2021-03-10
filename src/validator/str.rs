use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}

#[inline]
fn usize_is_zero(v: &usize) -> bool {
    *v == 0
}

#[inline]
fn usize_is_max(v: &usize) -> bool {
    *v == usize::MAX
}

#[inline]
fn normalize_is_none(v: &Normalize) -> bool {
    match *v {
        Normalize::None => true,
        _ => false,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Normalize {
    None,
    NFC,
    NFKC,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct StrValidator {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub default: String,
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<String>,
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", with = "serde_regex")]
    pub matches: Option<Box<Regex>>,
    #[serde(skip_serializing_if = "usize_is_max")]
    pub max_len: usize,
    #[serde(skip_serializing_if = "usize_is_zero")]
    pub min_len: usize,
    #[serde(skip_serializing_if = "usize_is_max")]
    pub max_char: usize,
    #[serde(skip_serializing_if = "usize_is_zero")]
    pub min_char: usize,
    #[serde(skip_serializing_if = "normalize_is_none")]
    pub normalize: Normalize,
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub regex: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub size: bool,
}

impl PartialEq for StrValidator {
    fn eq(&self, rhs: &Self) -> bool {
        (self.comment == rhs.comment)
            && (self.default == rhs.default)
            && (self.in_list == rhs.in_list)
            && (self.nin_list == rhs.nin_list)
            && (self.max_len == rhs.max_len)
            && (self.min_len == rhs.min_len)
            && (self.max_char == rhs.max_char)
            && (self.min_char == rhs.min_char)
            && (self.normalize == rhs.normalize)
            && (self.query == rhs.query)
            && (self.regex == rhs.regex)
            && (self.size == rhs.size)
            && match (&self.matches, &rhs.matches) {
                (None, None) => true,
                (Some(_), None) => false,
                (None, Some(_)) => false,
                (Some(lhs), Some(rhs)) => lhs.as_str() == rhs.as_str(),
            }
    }
}

impl std::default::Default for StrValidator {
    fn default() -> Self {
        Self {
            comment: String::new(),
            default: String::new(),
            in_list: Vec::new(),
            nin_list: Vec::new(),
            matches: None,
            max_len: usize::MAX,
            min_len: usize::MIN,
            max_char: usize::MAX,
            min_char: usize::MIN,
            normalize: Normalize::None,
            query: false,
            regex: false,
            size: false,
        }
    }
}

impl StrValidator {
    pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
        // Get element
        let elem = parser
            .next()
            .ok_or(Error::FailValidate("expected a string".to_string()))??;
        let val = if let Element::Str(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "expected Str, got {}",
                elem.name()
            )));
        };

        // Length Checks
        if val.len() > self.max_len {
            return Err(Error::FailValidate(
                "String is longer than max_len".to_string(),
            ));
        }
        if val.len() < self.min_len {
            return Err(Error::FailValidate(
                "String is shorter than min_len".to_string(),
            ));
        }
        if self.max_char < usize::MAX || self.min_char > 0 {
            let len_char = bytecount::num_chars(val.as_bytes());
            if len_char > self.max_char {
                return Err(Error::FailValidate(
                    "String is longer than max_len".to_string(),
                ));
            }
            if len_char < self.min_char {
                return Err(Error::FailValidate(
                    "String is shorter than min_len".to_string(),
                ));
            }
        }

        // Content checks
        use unicode_normalization::{
            is_nfc_quick, is_nfkc_quick, IsNormalized, UnicodeNormalization,
        };
        match self.normalize {
            Normalize::None => {
                if self.in_list.len() > 0 {
                    if !self.in_list.iter().any(|v| *v == val) {
                        return Err(Error::FailValidate(
                            "String is not on `in` list".to_string(),
                        ));
                    }
                }
                if self.nin_list.iter().any(|v| *v == val) {
                    return Err(Error::FailValidate("String is on `nin` list".to_string()));
                }
                if let Some(ref regex) = self.matches {
                    if !regex.is_match(val) {
                        return Err(Error::FailValidate(
                            "String doesn't match regular expression".to_string(),
                        ));
                    }
                }
            }
            Normalize::NFC => {
                let temp_string: String;
                let val = match is_nfc_quick(val.chars()) {
                    IsNormalized::Yes => val,
                    _ => {
                        temp_string = val.nfc().collect::<String>();
                        temp_string.as_str()
                    }
                };

                if self.in_list.len() > 0 {
                    if !self.in_list.iter().any(|v| v.nfc().eq(val.chars())) {
                        return Err(Error::FailValidate(
                            "String is not on `in` list".to_string(),
                        ));
                    }
                }
                if self.nin_list.iter().any(|v| v.nfc().eq(val.chars())) {
                    return Err(Error::FailValidate("String is on `nin` list".to_string()));
                }
                if let Some(ref regex) = self.matches {
                    if !regex.is_match(val) {
                        return Err(Error::FailValidate(
                            "String doesn't match regular expression".to_string(),
                        ));
                    }
                }
            }
            Normalize::NFKC => {
                let temp_string: String;
                let val = match is_nfkc_quick(val.chars()) {
                    IsNormalized::Yes => val,
                    _ => {
                        temp_string = val.nfkc().collect::<String>();
                        temp_string.as_str()
                    }
                };

                if self.in_list.len() > 0 {
                    if !self.in_list.iter().any(|v| v.nfkc().eq(val.chars())) {
                        return Err(Error::FailValidate(
                            "String is not on `in` list".to_string(),
                        ));
                    }
                }
                if self.nin_list.iter().any(|v| v.nfkc().eq(val.chars())) {
                    return Err(Error::FailValidate("String is on `nin` list".to_string()));
                }
                if let Some(ref regex) = self.matches {
                    if !regex.is_match(val) {
                        return Err(Error::FailValidate(
                            "String doesn't match regular expression".to_string(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn query_check_str(&self, other: &Self) -> bool {
        (self.query || (other.in_list.is_empty() && other.nin_list.is_empty()))
            && (self.regex || other.matches.is_none())
            && (self.size
                || (usize_is_max(&other.max_len)
                    && usize_is_zero(&other.min_len)
                    && usize_is_max(&other.max_char)
                    && usize_is_zero(&other.min_char)))
    }

    pub(crate) fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::Str(other) => self.query_check_str(other),
            Validator::Multi(list) => list.iter().all(|other| match other {
                Validator::Str(other) => self.query_check_str(other),
                _ => false,
            }),
            Validator::Any => true,
            _ => false,
        }
    }
}
