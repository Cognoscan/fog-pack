use super::*;
use crate::error::{Error, Result};
use crate::{de::FogDeserializer, element::*, value::Value, value_ref::ValueRef};
use serde::{Deserialize, Serialize};
use std::{default::Default, iter::repeat};

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}
#[inline]
fn validator_is_any(v: &Validator) -> bool {
    *v == Validator::Any
}

#[inline]
fn usize_is_zero(v: &usize) -> bool {
    *v == 0
}

#[inline]
fn usize_is_max(v: &usize) -> bool {
    *v == usize::MAX
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ArrayValidator {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub default: Vec<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub contains: Vec<Validator>,
    #[serde(skip_serializing_if = "validator_is_any")]
    pub items: Box<Validator>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub prefix: Vec<Validator>,
    #[serde(skip_serializing_if = "usize_is_max")]
    pub max_len: usize,
    #[serde(skip_serializing_if = "usize_is_zero")]
    pub min_len: usize,
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<Vec<Value>>,
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<Vec<Value>>,
    #[serde(skip_serializing_if = "is_false")]
    pub unique: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub array: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub contains_ok: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub unique_ok: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub size: bool,
}

impl Default for ArrayValidator {
    fn default() -> Self {
        Self {
            comment: String::new(),
            default: Vec::new(),
            contains: Vec::new(),
            items: Box::new(Validator::Any),
            prefix: Vec::new(),
            max_len: usize::MAX,
            min_len: usize::MIN,
            in_list: Vec::new(),
            nin_list: Vec::new(),
            unique: false,
            query: false,
            array: false,
            contains_ok: false,
            unique_ok: false,
            size: false,
        }
    }
}

impl ArrayValidator {
    pub(crate) fn validate<'de, 'c>(
        &'c self,
        types: &'c BTreeMap<String, Validator>,
        mut parser: Parser<'de>,
        mut checklist: Option<Checklist<'c>>,
    ) -> Result<(Parser<'de>, Option<Checklist<'c>>)> {
        let val_parser = parser.clone();
        let elem = parser
            .next()
            .ok_or_else(|| Error::FailValidate("Expected an array".to_string()))??;
        let len = if let Element::Array(len) = elem {
            len
        } else {
            return Err(Error::FailValidate(format!(
                "Expected Array, got {}",
                elem.name()
            )));
        };

        if len > self.max_len {
            return Err(Error::FailValidate(format!(
                "Array is {} elements, longer than maximum allowed of {}",
                len, self.max_len
            )));
        }
        if len < self.min_len {
            return Err(Error::FailValidate(format!(
                "Array is {} elements, shorter than minimum allowed of {}",
                len, self.min_len
            )));
        }

        // Check all the requirements that require parsing the entire array
        if self.unique || !self.in_list.is_empty() || !self.nin_list.is_empty() {
            let mut de = FogDeserializer::from_parser(val_parser);
            let array = Vec::<ValueRef>::deserialize(&mut de)?;

            if !self.in_list.is_empty() && !self.in_list.iter().any(|v| *v == array) {
                return Err(Error::FailValidate("Array is not on `in` list".to_string()));
            }

            if self.nin_list.iter().any(|v| *v == array) {
                return Err(Error::FailValidate("Array is on `nin` list".to_string()));
            }

            if self.unique
                && array
                    .iter()
                    .enumerate()
                    .any(|(index, lhs)| array.iter().skip(index).any(|rhs| lhs == rhs))
            {
                return Err(Error::FailValidate(
                    "Array does not contain unique elements".to_string(),
                ));
            }
        }

        // Loop through each item, verifying it with the appropriate validator
        let mut contains_result = vec![false; self.contains.len()];
        let mut validators = self.prefix.iter().chain(repeat(self.items.as_ref()));
        for _ in 0..len {
            // If we have a "contains", check
            if !self.contains.is_empty() {
                self.contains
                    .iter()
                    .zip(contains_result.iter_mut())
                    .for_each(|(validator, passed)| {
                        if !*passed {
                            let result =
                                validator.validate(types, parser.clone(), checklist.clone());
                            if let Ok((_, c)) = result {
                                *passed = true;
                                checklist = c;
                            }
                        }
                    });
            }
            let (p, c) = validators
                .next()
                .unwrap()
                .validate(types, parser, checklist)?;
            parser = p;
            checklist = c;
        }

        if !contains_result.iter().all(|x| *x) {
            let mut err_str = String::from("Array was missing items satisfying `contains` list:");
            let iter = contains_result
                .iter()
                .enumerate()
                .filter(|(_, pass)| !**pass)
                .map(|(index, _)| format!(" {},", index));
            err_str.extend(iter);
            err_str.pop(); // Remove the final comma
            return Err(Error::FailValidate(err_str));
        }
        Ok((parser, checklist))
    }

    fn query_check_self(
        &self,
        types: &BTreeMap<String, Validator>,
        other: &ArrayValidator,
    ) -> bool {
        let initial_check = (self.query || (other.in_list.is_empty() && other.nin_list.is_empty()))
            && (self.array || (other.prefix.is_empty() && validator_is_any(&other.items)))
            && (self.contains_ok || other.contains.is_empty())
            && (self.unique_ok || !other.unique)
            && (self.size || (usize_is_max(&other.max_len) && usize_is_zero(&other.min_len)));
        if !initial_check {
            return false;
        }
        if self.contains_ok {
            let contains_ok = other.contains.iter().all(|other| {
                self.items.query_check(types, other)
                    && self
                        .prefix
                        .iter()
                        .all(|mine| mine.query_check(types, other))
            });
            if !contains_ok {
                return false;
            }
        }
        if self.array {
            // Make sure item_type is OK, then check all items against their matching validator
            self.items.query_check(types, other.items.as_ref())
                && self
                    .prefix
                    .iter()
                    .chain(repeat(self.items.as_ref()))
                    .zip(other.prefix.iter().chain(repeat(other.items.as_ref())))
                    .take(self.prefix.len().max(other.prefix.len()))
                    .all(|(mine, other)| mine.query_check(types, other))
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
            Validator::Array(other) => self.query_check_self(types, other),
            Validator::Multi(list) => list.iter().all(|other| match other {
                Validator::Array(other) => self.query_check_self(types, other),
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
        let schema = ArrayValidator::default();
        let mut ser = FogSerializer::default();
        schema.serialize(&mut ser).unwrap();
        let expected: Vec<u8> = vec![0x80];
        let actual = ser.finish();
        println!("expected: {:x?}", expected);
        println!("actual:   {:x?}", actual);
        assert_eq!(expected, actual);

        let mut de = FogDeserializer::with_debug(&actual, "    ".into());
        let decoded = ArrayValidator::deserialize(&mut de).unwrap();
        println!("{}", de.get_debug().unwrap());
        assert_eq!(schema, decoded);
    }
}
