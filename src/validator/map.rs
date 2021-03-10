use super::*;
use crate::error::{Error, Result};
use crate::{de::FogDeserializer, element::*, value::Value, value_ref::ValueRef};
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};
use std::default::Default;

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

#[inline]
fn key_validator_is_default(v: &KeyValidator) -> bool {
    v.matches.is_none()
        && normalize_is_none(&v.normalize)
        && usize_is_max(&v.max_len)
        && usize_is_zero(&v.min_len)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Normalize {
    None,
    NFC,
    NFKC,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct KeyValidator {
    #[serde(skip_serializing_if = "Option::is_none", with = "serde_regex")]
    pub matches: Option<Box<Regex>>,
    #[serde(skip_serializing_if = "normalize_is_none")]
    pub normalize: Normalize,
    #[serde(skip_serializing_if = "usize_is_max")]
    pub max_len: usize,
    #[serde(skip_serializing_if = "usize_is_zero")]
    pub min_len: usize,
}

impl KeyValidator {
    fn validate<'a>(&self, parser: &mut Parser<'a>) -> Result<&'a str> {
        // Get element
        let elem = parser
            .next()
            .ok_or(Error::FailValidate("expected a key string".to_string()))??;
        let val = if let Element::Str(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "expected Str key, got {}",
                elem.name()
            )));
        };

        // Length Checks
        if val.len() > self.max_len {
            return Err(Error::FailValidate(
                "Key is longer than max_len".to_string(),
            ));
        }
        if val.len() < self.min_len {
            return Err(Error::FailValidate(
                "Key is shorter than min_len".to_string(),
            ));
        }

        // Content checks
        use unicode_normalization::{
            is_nfc_quick, is_nfkc_quick, IsNormalized, UnicodeNormalization,
        };
        if let Some(ref regex) = self.matches {
            match self.normalize {
                Normalize::None => {
                    if !regex.is_match(val) {
                        return Err(Error::FailValidate(
                            "Key doesn't match regular expression".to_string(),
                        ));
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
                    if !regex.is_match(val) {
                        return Err(Error::FailValidate(
                            "Key doesn't match regular expression".to_string(),
                        ));
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
                    if !regex.is_match(val) {
                        return Err(Error::FailValidate(
                            "Key doesn't match regular expression".to_string(),
                        ));
                    }
                }
            }
        }
        Ok(val)
    }
}

impl PartialEq for KeyValidator {
    fn eq(&self, rhs: &Self) -> bool {
        (self.normalize == rhs.normalize)
            && (self.max_len == rhs.max_len)
            && (self.min_len == rhs.min_len)
            && match (&self.matches, &rhs.matches) {
                (None, None) => true,
                (Some(_), None) => false,
                (None, Some(_)) => false,
                (Some(lhs), Some(rhs)) => lhs.as_str() == rhs.as_str(),
            }
    }
}

impl Default for KeyValidator {
    fn default() -> Self {
        Self {
            matches: None,
            max_len: usize::MAX,
            min_len: usize::MIN,
            normalize: Normalize::None,
        }
    }
}

fn get_validator<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Box<Validator>>, D::Error> {
    // Decode the validator. If this function is called, there should be an actual validator
    // present. Otherwise we fail. In other words, no `null` allowed.
    Ok(Some(Box::new(Validator::deserialize(deserializer)?)))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct MapValidator {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub default: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "usize_is_max")]
    pub max_len: usize,
    #[serde(skip_serializing_if = "usize_is_zero")]
    pub min_len: usize,
    #[serde(skip_serializing_if = "key_validator_is_default")]
    pub keys: KeyValidator,
    #[serde(
        skip_serializing_if = "Option::is_none",
        deserialize_with = "get_validator"
    )]
    pub values: Option<Box<Validator>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub req: BTreeMap<String, Validator>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub opt: BTreeMap<String, Validator>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ban: Vec<String>,
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<BTreeMap<String, Value>>,
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<BTreeMap<String, Value>>,
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub size: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub map_ok: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub match_keys: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub len_keys: bool,
}

impl Default for MapValidator {
    fn default() -> Self {
        Self {
            comment: String::new(),
            default: BTreeMap::new(),
            max_len: usize::MAX,
            min_len: usize::MIN,
            keys: KeyValidator::default(),
            values: None,
            req: BTreeMap::new(),
            opt: BTreeMap::new(),
            ban: Vec::new(),
            in_list: Vec::new(),
            nin_list: Vec::new(),
            query: false,
            size: false,
            map_ok: false,
            match_keys: false,
            len_keys: false,
        }
    }
}

impl MapValidator {
    pub(crate) fn validate<'de, 'c>(
        &'c self,
        types: &'c BTreeMap<String, Validator>,
        mut parser: Parser<'de>,
        mut checklist: Option<Checklist<'c>>,
    ) -> Result<(Parser<'de>, Option<Checklist<'c>>)> {
        let val_parser = parser.clone();
        let elem = parser
            .next()
            .ok_or(Error::FailValidate("Expected a map".to_string()))??;
        let len = if let Element::Map(len) = elem {
            len
        } else {
            return Err(Error::FailValidate(format!(
                "Expected Map, got {}",
                elem.name()
            )));
        };

        if len > self.max_len {
            return Err(Error::FailValidate(format!(
                "Map is {} pairs, longer than maximum allowed of {}",
                len, self.max_len
            )));
        }
        if len < self.min_len {
            return Err(Error::FailValidate(format!(
                "Map is {} pairs, shorter than minimum allowed of {}",
                len, self.min_len
            )));
        }

        // Check the requirements that require parsing the entire array
        if !self.in_list.is_empty() || !self.nin_list.is_empty() {
            let mut de = FogDeserializer::from_parser(val_parser);
            let map = BTreeMap::<&str, ValueRef>::deserialize(&mut de)?;

            if !self.in_list.is_empty() {
                if !self.in_list.iter().any(|v| {
                    v.len() == map.len()
                        && v.iter()
                            .zip(map.iter())
                            .all(|((ks, vs), (ko, vo))| (ks == ko) && (vs == vo))
                }) {
                    return Err(Error::FailValidate("Map is not on `in` list".to_string()));
                }
            }

            if self.nin_list.iter().any(|v| {
                v.len() == map.len()
                    && v.iter()
                        .zip(map.iter())
                        .all(|((ks, vs), (ko, vo))| (ks == ko) && (vs == vo))
            }) {
                return Err(Error::FailValidate("Map is on `nin` list".to_string()));
            }
        }

        // Loop through each item, verifying it with the appropriate validator
        let mut reqs_found = 0;
        for _ in 0..len {
            let key = self.keys.validate(&mut parser)?;
            if self.ban.iter().any(|k| k == key) {
                return Err(Error::FailValidate(format!(
                    "Map key {:?} is on the ban list",
                    key
                )));
            }
            let (p, c) = if let Some(validator) = self
                .req
                .get(key)
                .and_then(|v| {
                    reqs_found += 1;
                    Some(v)
                })
                .or_else(|| self.opt.get(key))
                .or_else(|| self.values.as_deref())
            {
                validator.validate(types, parser, checklist)?
            } else {
                return Err(Error::FailValidate(format!(
                    "Map key {:?} has no corresponding validator",
                    key
                )));
            };
            parser = p;
            checklist = c;
        }

        if reqs_found != self.req.len() {
            return Err(Error::FailValidate(format!(
                "Map did not have all required key-value pairs (missing {})",
                reqs_found
            )));
        }

        Ok((parser, checklist))
    }

    fn query_check_self(&self, types: &BTreeMap<String, Validator>, other: &MapValidator) -> bool {
        let initial_check = (self.query || (other.in_list.is_empty() && other.nin_list.is_empty()))
            && (self.size || (usize_is_max(&other.max_len) && usize_is_zero(&other.min_len)))
            && (self.map_ok
                || (other.req.is_empty()
                    && other.opt.is_empty()
                    && other.ban.is_empty()
                    && other.values.is_none()))
            && (self.match_keys || other.keys.matches.is_none())
            && (self.len_keys
                || (usize_is_max(&other.keys.max_len) && usize_is_zero(&other.keys.min_len)));
        if !initial_check {
            return false;
        }
        if self.map_ok {
            // Make sure `values` is OK, then check the req/opt pairs against matching validators
            let values_ok = match (&self.values, &other.values) {
                (None, None) => true,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (Some(s), Some(o)) => s.query_check(types, o.as_ref()),
            };
            if !values_ok {
                return false;
            }
            let req_ok = other.req.iter().all(|(ko, kv)| {
                self.req
                    .get(ko)
                    .or_else(|| self.opt.get(ko))
                    .or_else(|| self.values.as_deref())
                    .and_then(|v| Some(v.query_check(types, kv)))
                    .unwrap_or(false)
            });
            if !req_ok {
                return false;
            }
            let opt_ok = other.opt.iter().all(|(ko, kv)| {
                self.req
                    .get(ko)
                    .or_else(|| self.opt.get(ko))
                    .or_else(|| self.values.as_deref())
                    .and_then(|v| Some(v.query_check(types, kv)))
                    .unwrap_or(false)
            });
            opt_ok
        } else {
            true
        }
    }

    pub(crate) fn query_check(
        &self,
        types: &BTreeMap<String, Validator>,
        other: &Validator,
    ) -> bool {
        match other {
            Validator::Map(other) => self.query_check_self(types, other),
            Validator::Multi(list) => list.iter().all(|other| match other {
                Validator::Map(other) => self.query_check_self(types, other),
                _ => false,
            }),
            Validator::Any => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{de::FogDeserializer, ser::FogSerializer};

    #[test]
    fn ser_default() {
        // Should be an empty map if we use the defaults
        let schema = MapValidator::default();
        let mut ser = FogSerializer::default();
        schema.serialize(&mut ser).unwrap();
        let expected: Vec<u8> = vec![0x80];
        let actual = ser.finish();
        println!("expected: {:x?}", expected);
        println!("actual:   {:x?}", actual);
        assert_eq!(expected, actual);

        let mut de = FogDeserializer::with_debug(&actual, "    ".into());
        let decoded = MapValidator::deserialize(&mut de).unwrap();
        println!("{}", de.get_debug().unwrap());
        assert_eq!(schema, decoded);
    }
}
