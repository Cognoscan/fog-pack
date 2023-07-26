use super::*;
use crate::error::{Error, Result};
use crate::{de::FogDeserializer, element::*, value::Value, value_ref::ValueRef};
use serde::{Deserialize, Deserializer, Serialize};
use std::default::Default;

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}

#[inline]
fn u32_is_zero(v: &u32) -> bool {
    *v == 0
}

#[inline]
fn u32_is_max(v: &u32) -> bool {
    *v == u32::MAX
}

#[inline]
fn normalize_is_none(v: &Normalize) -> bool {
    matches!(v, Normalize::None)
}

fn get_str_validator<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Box<StrValidator>>, D::Error> {
    // Decode the validator. If this function is called, there should be an actual validator
    // present. Otherwise we fail. In other words, no `null` allowed.
    Ok(Some(Box::new(StrValidator::deserialize(deserializer)?)))
}


fn get_validator<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Box<Validator>>, D::Error> {
    // Decode the validator. If this function is called, there should be an actual validator
    // present. Otherwise we fail. In other words, no `null` allowed.
    Ok(Some(Box::new(Validator::deserialize(deserializer)?)))
}

/// Validator for maps.
///
/// This validator will only pass maps, whose keys are strings and values are any valid fog-pack
/// value. Validation passes if:
///
/// - If the `in` list is not empty, the array must be among the arrays in the list.
/// - The array must not be among the arrays in the `nin` list.
/// - The number of key-value pairs in the map is less than or equal to the value in `max_len`.
/// - The number of key-value pairs in the map is greater than or equal to the value in `min_len`.
/// - There must be a matching key-value in the map for each key-validator pair in `req` .
/// - For each key-value pair in the map:
///     1. If the key is in `req`, the corresponding validator is used to validate the value.
///     2. If the key is not in `req` but is in `opt`, the corresponding validator is used to
///        validate the value.
///     3. if the key is not in `req` or `opt`, the validator for `values` is used to validate the
///        value, and the validator for `keys` (if present) is used to validate the key.
///         1. If no validator is present for `keys`, the key passes.
///         2. If there is no validator for `values`, validation does not pass.
///
/// Note how each key-value pair must be validated, so an unlimited collection of key-value pairs
/// isn't allowed unless there is a validator present in `values`.
///
/// # Defaults
///
/// Fields that aren't specified for the validator use their defaults instead. The defaults for
/// each field are:
///
/// - comment: ""
/// - max_len: u32::MAX
/// - min_len: u32::MIN
/// - keys: None
/// - values: None
/// - req: empty
/// - opt: empty
/// - in_list: empty
/// - nin_list: empty
/// - query: false
/// - size: false
/// - map_ok: false
/// - match_keys: false
/// - len_keys: false
///
/// # Query Checking
///
/// Queries for maps are only allowed to use non-default values for each field if the
/// corresponding query permission is set in the schema's validator:
///
/// - query: `in` and `nin` lists
/// - size: `max_len` and `min_len`
/// - map_ok: `req`, `opt`, `keys`, and `values`
///
/// In addition, sub-validators in the query are matched against the schema's sub-validators:
///
/// - The `values` validator is checked against the schema's `values` validator. If no schema 
///     validator is present, the query is invalid.
/// - The `keys` string validator is checked against the schema's `keys` string validator. If no 
///     schema validator is present, the query is invalid.
/// - The `req` validators are checked against the schema's `req`/`opt`/`values` validators,
///     choosing whichever validator is found first. If no validator is found, the check fails.
/// - The `opt` validators are checked against the schema's `req`/`opt`/`values` validators,
///     choosing whichever validator is found first. If no validator is found, the check fails.
///
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct MapValidator {
    /// An optional comment explaining the validator.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    /// The maximum allowed number of key-value pairs in the map.
    #[serde(skip_serializing_if = "u32_is_max")]
    pub max_len: u32,
    /// The minimum allowed number of key-value pairs in the map.
    #[serde(skip_serializing_if = "u32_is_zero")]
    pub min_len: u32,
    /// The optional sub-validator for unknown keys in the map.
    #[serde(
        skip_serializing_if = "Option::is_none",
        deserialize_with = "get_str_validator"
    )]
    pub keys: Option<Box<StrValidator>>,
    /// An optional validator that each value in the map must pass, unless it is instead checked by
    /// a validator in `req` or `opt`. Unchecked values cause the map to fail validation.
    #[serde(
        skip_serializing_if = "Option::is_none",
        deserialize_with = "get_validator"
    )]
    pub values: Option<Box<Validator>>,
    /// A map whose keys must all be present in a passing map, and whose validators are used to
    /// check the value held by a matching key in the map.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub req: BTreeMap<String, Validator>,
    /// A map whose keys may be present in a map, and whose validators are used to
    /// check the value held by a matching key in the map, unless it is first checked by a
    /// validator in `req`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub opt: BTreeMap<String, Validator>,
    /// A vector of specific allowed values, stored under the `in` field. If empty, this vector is not checked against.
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<BTreeMap<String, Value>>,
    /// A vector of specific unallowed values, stored under the `nin` field.
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<BTreeMap<String, Value>>,
    /// If true, queries against matching spots may have values in the `in` or `nin` lists.
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
    /// If true, queries against matching spots may use `max_len` and `min_len`.
    #[serde(skip_serializing_if = "is_false")]
    pub size: bool,
    /// If true, queries against matching spots may use `req`, `opt`, `keys`, and `values`.
    #[serde(skip_serializing_if = "is_false")]
    pub map_ok: bool,
}

impl Default for MapValidator {
    fn default() -> Self {
        Self {
            comment: String::new(),
            max_len: u32::MAX,
            min_len: u32::MIN,
            keys: None,
            values: None,
            req: BTreeMap::new(),
            opt: BTreeMap::new(),
            in_list: Vec::new(),
            nin_list: Vec::new(),
            query: false,
            size: false,
            map_ok: false,
        }
    }
}

impl MapValidator {
    /// Make a new validator with the default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a comment for the validator.
    pub fn comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = comment.into();
        self
    }

    /// Set the `values` validator.
    pub fn values(mut self, values: Validator) -> Self {
        self.values = Some(Box::new(values));
        self
    }

    /// Add a new validator to the `req` map.
    pub fn req_add(mut self, key: impl Into<String>, req: Validator) -> Self {
        self.req.insert(key.into(), req);
        self
    }

    /// Add a new validator to the `opt` map.
    pub fn opt_add(mut self, key: impl Into<String>, opt: Validator) -> Self {
        self.opt.insert(key.into(), opt);
        self
    }

    /// Set the Key Validator.
    pub fn keys(mut self, keys: StrValidator) -> Self {
        self.keys = Some(Box::new(keys));
        self
    }

    /// Set the maximum number of allowed bytes.
    pub fn max_len(mut self, max_len: u32) -> Self {
        self.max_len = max_len;
        self
    }

    /// Set the minimum number of allowed bytes.
    pub fn min_len(mut self, min_len: u32) -> Self {
        self.min_len = min_len;
        self
    }

    /// Add a value to the `in` list.
    pub fn in_add(mut self, add: impl Into<BTreeMap<String, Value>>) -> Self {
        self.in_list.push(add.into());
        self
    }

    /// Add a value to the `nin` list.
    pub fn nin_add(mut self, add: impl Into<BTreeMap<String, Value>>) -> Self {
        self.nin_list.push(add.into());
        self
    }

    /// Set whether or not queries can use the `in` and `nin` lists.
    pub fn query(mut self, query: bool) -> Self {
        self.query = query;
        self
    }

    /// Set whether or not queries can use the `max_len` and `min_len` values.
    pub fn size(mut self, size: bool) -> Self {
        self.size = size;
        self
    }

    /// Set whether or not queries can use the `req`, `opt`, `ban`, and `values` values.
    pub fn map_ok(mut self, map_ok: bool) -> Self {
        self.map_ok = map_ok;
        self
    }

    /// Build this into a [`Validator`] enum.
    pub fn build(self) -> Validator {
        Validator::Map(self)
    }

    pub(crate) fn validate<'de, 'c>(
        &'c self,
        types: &'c BTreeMap<String, Validator>,
        mut parser: Parser<'de>,
        mut checklist: Option<Checklist<'c>>,
    ) -> Result<(Parser<'de>, Option<Checklist<'c>>)> {
        let val_parser = parser.clone();
        let elem = parser
            .next()
            .ok_or_else(|| Error::FailValidate("Expected a map".to_string()))??;
        let len = if let Element::Map(len) = elem {
            len
        } else {
            return Err(Error::FailValidate(format!(
                "Expected Map, got {}",
                elem.name()
            )));
        };

        if (len as u32) > self.max_len {
            return Err(Error::FailValidate(format!(
                "Map is {} pairs, longer than maximum allowed of {}",
                len, self.max_len
            )));
        }
        if (len as u32) < self.min_len {
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
                let in_pass = self.in_list.iter().any(|v| {
                    v.len() == map.len()
                        && v.iter()
                            .zip(map.iter())
                            .all(|((ks, vs), (ko, vo))| (ks == ko) && (vs == vo))
                });
                if !in_pass {
                    return Err(Error::FailValidate("Map is not on `in` list".to_string()));
                }
            }

            let nin_pass = !self.nin_list.iter().any(|v| {
                v.len() == map.len()
                    && v.iter()
                        .zip(map.iter())
                        .all(|((ks, vs), (ko, vo))| (ks == ko) && (vs == vo))
            });
            if !nin_pass {
                return Err(Error::FailValidate("Map is on `nin` list".to_string()));
            }
        }

        // Loop through each item, verifying it with the appropriate validator
        let mut reqs_found = 0;
        for _ in 0..len {
            // Extract the key
            let elem = parser
                .next()
                .ok_or_else(|| Error::FailValidate("expected a key string".to_string()))??;
            let key = if let Element::Str(v) = elem {
                v
            } else {
                return Err(Error::FailValidate(format!(
                    "expected Str, got {}",
                    elem.name()
                )));
            };

            // Look up the appropriate validator and use it
            let (p, c) = if let Some(validator) = self.req.get(key) {
                reqs_found += 1;
                validator.validate(types, parser, checklist)?
            }
            else if let Some(validator) = self.opt.get(key) {
                validator.validate(types, parser, checklist)?
            }
            else if let Some(validator) = &self.values {
                // Make sure the key is valid before proceeding
                if let Some(keys) = &self.keys {
                    keys.validate_str(key)?;
                }
                validator.validate(types, parser, checklist)?
            }
            else {
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
            && (self.size || (u32_is_max(&other.max_len) && u32_is_zero(&other.min_len)))
            && (self.map_ok
                || (other.req.is_empty()
                    && other.opt.is_empty()
                    && other.keys.is_none()
                    && other.values.is_none()));
        if !initial_check {
            return false;
        }
        if self.map_ok {
            // Make sure `keys` and `values` are OK, then check the req/opt pairs against matching 
            // validators

            let values_ok = match (&self.values, &other.values) {
                (None, None) => true,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (Some(s), Some(o)) => s.query_check(types, o.as_ref()),
            };
            if !values_ok {
                return false;
            }

            let keys_ok = match (&self.keys, &other.keys) {
                (None, None) => true,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (Some(s), Some(o)) => s.query_check_str(o.as_ref()),
            };
            if !keys_ok {
                return false;
            }

            let req_ok = other.req.iter().all(|(ko, kv)| {
                self.req
                    .get(ko)
                    .or_else(|| self.opt.get(ko))
                    .or(self.values.as_deref())
                    .map(|v| v.query_check(types, kv))
                    .unwrap_or(false)
            });
            if !req_ok {
                return false;
            }

            let opt_ok = other.opt.iter().all(|(ko, kv)| {
                self.req
                    .get(ko)
                    .or_else(|| self.opt.get(ko))
                    .or(self.values.as_deref())
                    .map(|v| v.query_check(types, kv))
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

        let mut de = FogDeserializer::with_debug(&actual, "    ");
        let decoded = MapValidator::deserialize(&mut de).unwrap();
        println!("{}", de.get_debug().unwrap());
        assert_eq!(schema, decoded);
    }
}
