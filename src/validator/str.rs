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

#[derive(Serialize, Deserialize)]
pub enum Normalize {
    None,
    NFC,
    NFKC,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct StrValidator {
    #[serde(skip_serializing_if = "String::is_empty")]
    comment: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    default: String,
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    in_list: Vec<String>,
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    nin_list: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", with = "serde_regex")]
    matches: Option<Regex>,
    #[serde(skip_serializing_if = "usize_is_max")]
    max_len: usize,
    #[serde(skip_serializing_if = "usize_is_zero")]
    min_len: usize,
    #[serde(skip_serializing_if = "usize_is_max")]
    max_char: usize,
    #[serde(skip_serializing_if = "usize_is_zero")]
    min_char: usize,
    #[serde(skip_serializing_if = "normalize_is_none")]
    normalize: Normalize,
    #[serde(skip_serializing_if = "is_false")]
    query: bool,
    #[serde(skip_serializing_if = "is_false")]
    regex: bool,
    #[serde(skip_serializing_if = "is_false")]
    size: bool,
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
        let val = if let Element::Str(v) = elem { v } else {
            return Err(Error::FailValidate(format!(
                "expected Str, got {}",
                elem.name()
            )));
        };

        // Length Checks
        if val.len() > self.max_len {
            return Err(Error::FailValidate("String is longer than max_len".to_string()));
        }
        if val.len() < self.min_len {
            return Err(Error::FailValidate("String is shorter than min_len".to_string()));
        }
        if self.max_char < usize::MAX || self.min_char > 0 {
            let len_char = bytecount::num_chars(val.as_bytes());
            if len_char > self.max_char {
                return Err(Error::FailValidate("String is longer than max_len".to_string()));
            }
            if len_char < self.min_char {
                return Err(Error::FailValidate("String is shorter than min_len".to_string()));
            }
        }

        // Content checks
        use unicode_normalization::{UnicodeNormalization, IsNormalized, is_nfc_quick, is_nfkc_quick};
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
                        return Err(Error::FailValidate("String doesn't match regular expression".to_string()));
                    }
                }
            },
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
                        return Err(Error::FailValidate("String doesn't match regular expression".to_string()));
                    }
                }
            },
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
                        return Err(Error::FailValidate("String doesn't match regular expression".to_string()));
                    }
                }
            },
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

pub(super) mod serde_regex {
    use super::*;
    use serde::{Serializer, Deserializer};

    pub(super) fn serialize<S: Serializer>(value: &Option<Regex>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            None => {
                serializer.serialize_none() // This should never actually happen, it should be skipped
            },
            Some(regex) => {
                serializer.serialize_str(regex.as_str())
            },
        }
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<Regex>, D::Error>
        where D: Deserializer<'de>
    {
        use serde::de::Error;
        // Note that this will not accept a null value - it *must* be a string, even though this is 
        // ends up as an Option. This is because we chose to have validators where the field is 
        // either defined, or it is absent.
        let regex: String = String::deserialize(deserializer)?;
        let regex = Regex::new(&regex).map_err(|e| D::Error::custom(e.to_string()))?;
        Ok(Some(regex))
    }
}

