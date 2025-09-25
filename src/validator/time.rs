use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use crate::Timestamp;
use educe::Educe;
use serde::{Deserialize, Serialize};
use std::default::Default;

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}

const MIN_TIME: Timestamp = Timestamp::min_value();
const MAX_TIME: Timestamp = Timestamp::max_value();

#[inline]
fn time_is_min(v: &Timestamp) -> bool {
    *v == MIN_TIME
}

#[inline]
fn time_is_max(v: &Timestamp) -> bool {
    *v == MAX_TIME
}

/// Validator for timestamps.
///
/// This validator will only pass timestamps. Validation passes if:
///
/// - If the `in` list is not empty, the timestamp must be among the timestamp in the list.
/// - The timestamp must not be among the timestamp in the `nin` list.
/// - The timestamp is less than the maximum in `max`, or equal to it if `ex_max` is not set to true.
/// - The timestamp is greater than the minimum in `min`, or equal to it if `ex_min` is not set to true.
///
/// # Defaults
///
/// Fields that aren't specified for the validator use their defaults instead. The defaults for
/// each field are:
///
/// - comment: ""
/// - max: maximum possible timestamp
/// - min: minimum possible timestamp
/// - ex_max: false
/// - ex_min: false
/// - in_list: empty
/// - nin_list: empty
/// - query: false
/// - ord: false
///
#[derive(Educe, Clone, Debug, Serialize, Deserialize)]
#[educe(Default, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct TimeValidator {
    /// An optional comment explaining the validator.
    #[educe(PartialEq(ignore))]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    /// The maximum allowed timestamp.
    #[educe(Default = MAX_TIME)]
    #[serde(skip_serializing_if = "time_is_max")]
    pub max: Timestamp,
    /// The minimum allowed timestamp.
    #[educe(Default = MIN_TIME)]
    #[serde(skip_serializing_if = "time_is_min")]
    pub min: Timestamp,
    /// Changes `max` into an exclusive maximum.
    #[serde(skip_serializing_if = "is_false")]
    pub ex_max: bool,
    /// Changes `min` into an exclusive maximum.
    #[serde(skip_serializing_if = "is_false")]
    pub ex_min: bool,
    /// A vector of specific allowed values, stored under the `in` field. If empty, this vector is not checked against.
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<Timestamp>,
    /// A vector of specific unallowed values, stored under the `nin` field.
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<Timestamp>,
    /// If true, queries against matching spots may have values in the `in` or `nin` lists.
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
    /// If true, queries against matching spots may set the `max`, `min`, `ex_max`, and `ex_min`
    /// values to non-defaults.
    #[serde(skip_serializing_if = "is_false")]
    pub ord: bool,
}

impl TimeValidator {
    /// Make a new validator with the default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a comment for the validator.
    pub fn comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = comment.into();
        self
    }

    /// Set the maximum allowed value.
    pub fn max(mut self, max: impl Into<Timestamp>) -> Self {
        self.max = max.into();
        self
    }

    /// Set the minimum allowed value.
    pub fn min(mut self, min: impl Into<Timestamp>) -> Self {
        self.min = min.into();
        self
    }

    /// Set whether or or not `max` is an exclusive maximum.
    pub fn ex_max(mut self, ex_max: bool) -> Self {
        self.ex_max = ex_max;
        self
    }

    /// Set whether or or not `min` is an exclusive maximum.
    pub fn ex_min(mut self, ex_min: bool) -> Self {
        self.ex_min = ex_min;
        self
    }

    /// Add a value to the `in` list.
    pub fn in_add(mut self, add: impl Into<Timestamp>) -> Self {
        self.in_list.push(add.into());
        self
    }

    /// Add a value to the `nin` list.
    pub fn nin_add(mut self, add: impl Into<Timestamp>) -> Self {
        self.nin_list.push(add.into());
        self
    }

    /// Set whether or not queries can use the `in` and `nin` lists.
    pub fn query(mut self, query: bool) -> Self {
        self.query = query;
        self
    }

    /// Set whether or not queries can use the `max`, `min`, `ex_max`, and `ex_min` values.
    pub fn ord(mut self, ord: bool) -> Self {
        self.ord = ord;
        self
    }

    /// Build this into a [`Validator`] enum.
    pub fn build(self) -> Validator {
        Validator::Time(Box::new(self))
    }

    pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
        let elem = parser
            .next()
            .ok_or_else(|| Error::FailValidate("Expected a timestamp".to_string()))??;
        let val = if let Element::Timestamp(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "Expected Time, got {}",
                elem.name()
            )));
        };

        // Range checks
        let max_pass = if self.ex_max {
            val < self.max
        } else {
            val <= self.max
        };
        let min_pass = if self.ex_min {
            val > self.min
        } else {
            val >= self.min
        };
        if !max_pass {
            return Err(Error::FailValidate(
                "Timestamp greater than maximum allowed".to_string(),
            ));
        }
        if !min_pass {
            return Err(Error::FailValidate(
                "Timestamp less than minimum allowed".to_string(),
            ));
        }

        // in/nin checks
        if !self.in_list.is_empty() && !self.in_list.iter().any(|v| *v == val) {
            return Err(Error::FailValidate(
                "Timestamp is not on `in` list".to_string(),
            ));
        }
        if self.nin_list.iter().any(|v| *v == val) {
            return Err(Error::FailValidate(
                "Timestamp is on `nin` list".to_string(),
            ));
        }

        Ok(())
    }

    fn query_check_self(&self, other: &Self) -> bool {
        (self.query || (other.in_list.is_empty() && other.nin_list.is_empty()))
            && (self.ord
                || (!other.ex_min
                    && !other.ex_max
                    && time_is_min(&other.min)
                    && time_is_max(&other.max)))
    }

    pub(crate) fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::Time(other) => self.query_check_self(other),
            Validator::Multi(list) => list.iter().all(|other| match other {
                Validator::Time(other) => self.query_check_self(other),
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
    fn default_ser() {
        // Should be an empty map if we use the defaults
        let schema = TimeValidator::default();
        let mut ser = FogSerializer::default();
        schema.serialize(&mut ser).unwrap();
        let expected: Vec<u8> = vec![0x80];
        let actual = ser.finish();
        println!("expected: {:x?}", expected);
        println!("actual:   {:x?}", actual);
        assert_eq!(expected, actual);

        let mut de = FogDeserializer::new(&actual);
        let decoded = TimeValidator::deserialize(&mut de).unwrap();
        assert_eq!(schema, decoded);
    }

    #[test]
    fn example_ser() {
        let schema = TimeValidator {
            comment: "The year 2020".to_string(),
            min: Timestamp::from_utc(1577854800, 0).unwrap(),
            max: Timestamp::from_utc(1609477200, 0).unwrap(),
            ex_min: false,
            ex_max: true,
            in_list: Vec::new(),
            nin_list: Vec::new(),
            query: true,
            ord: true,
        };
        let mut ser = FogSerializer::default();
        schema.serialize(&mut ser).unwrap();
        let mut expected: Vec<u8> = vec![0x86];
        serialize_elem(&mut expected, Element::Str("comment"));
        serialize_elem(&mut expected, Element::Str("The year 2020"));
        serialize_elem(&mut expected, Element::Str("ex_max"));
        serialize_elem(&mut expected, Element::Bool(true));
        serialize_elem(&mut expected, Element::Str("max"));
        serialize_elem(
            &mut expected,
            Element::Timestamp(Timestamp::from_utc(1609477200, 0).unwrap()),
        );
        serialize_elem(&mut expected, Element::Str("min"));
        serialize_elem(
            &mut expected,
            Element::Timestamp(Timestamp::from_utc(1577854800, 0).unwrap()),
        );
        serialize_elem(&mut expected, Element::Str("ord"));
        serialize_elem(&mut expected, Element::Bool(true));
        serialize_elem(&mut expected, Element::Str("query"));
        serialize_elem(&mut expected, Element::Bool(true));
        let actual = ser.finish();
        println!("expected: {:x?}", expected);
        println!("actual:   {:x?}", actual);
        assert_eq!(expected, actual);

        let mut de = FogDeserializer::with_debug(&actual, "    ".to_string());
        match TimeValidator::deserialize(&mut de) {
            Ok(decoded) => assert_eq!(schema, decoded),
            Err(e) => {
                println!("{}", de.get_debug().unwrap());
                println!("Error: {}", e);
                panic!("Couldn't decode");
            }
        }
    }
}
