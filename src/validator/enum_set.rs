use super::*;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::default::Default;

/// "Enum" validator that selects a validator based on the value's enum variant.
///
/// This validator expects a serialized Rust enum. A serialized enum consists of either a single
/// string (a unit variant) or a map with a single key-value pair, where the key is the name of the
/// enum variant and the value is the associated data. The associated data is validated against the
/// matching validator in the contained `BTreeMap`. If there is no match, validation fails.
///
/// For unit variants, there is no validator, and they pass as long as their name is a key in the
/// `BTreeMap`.
///
/// # Query Checking
///
/// The query validator must be an Any or an Enum validator, and the maps are directly checked against
/// each other. The query validator may use a subset of the enum list. For unit variants, both the
/// query validator and schema validator must have `None` instead of a validator. As an example,
/// see the following:
///
/// ```
/// # use fog_pack::{
/// #     validator::*,
/// #     schema::*,
/// #     document::*,
/// #     entry::*,
/// #     query::*,
/// #     types::*,
/// # };
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
///
/// // Say we have a enum like this:
/// enum ExampleEnum {
///     Empty,
///     Integer(Integer),
///     String(String),
/// }
///
/// // Let's use this as our schema-side validator
/// let entry_validator = EnumValidator::new()
///     .insert("Empty", None)
///     .insert("Integer", Some(IntValidator::new().build()))
///     .insert("String", Some(StrValidator::new().build()))
///     .build();
///
/// // We'll build a full schema, so we can do query validation
/// let schema_doc = SchemaBuilder::new(Validator::Null)
///     .entry_add("item", entry_validator, None)
///     .build()
///     .unwrap();
/// let schema = Schema::from_doc(&schema_doc).unwrap();
///
/// // This query is accepted because all enum validators match. Note how
/// // String isn't present, because the query doesn't need to have all
/// // possible enums.
/// let query_validator = EnumValidator::new()
///     .insert("Empty", None)
///     .insert("Integer", Some(IntValidator::new().build()))
///     .build();
/// let query = NewQuery::new("item", query_validator);
/// assert!(schema.encode_query(query).is_ok());
///
/// // This query, however, has a validator for "Empty", so it doesn't work:
/// let query_validator = EnumValidator::new()
///     .insert("Empty", Some(Validator::Null))
///     .insert("Integer", Some(IntValidator::new().build()))
///     .build();
/// let query = NewQuery::new("item", query_validator);
/// assert!(schema.encode_query(query).is_err());
///
/// # Ok(())
/// # }
/// ```
///
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnumValidator(pub BTreeMap<String, Option<Validator>>);

impl EnumValidator {
    /// Make a new validator with the default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new enum to the set.
    pub fn insert(mut self, variant: impl Into<String>, validator: Option<Validator>) -> Self {
        self.0.insert(variant.into(), validator);
        self
    }

    /// Build this into a [`Validator`] enum.
    pub fn build(self) -> Validator {
        Validator::Enum(self)
    }

    /// Iterate over all the enum variants.
    pub fn iter(&self) -> std::collections::btree_map::Iter<String, Option<Validator>> {
        self.0.iter()
    }

    /// Iterate over all the validators in this enum.
    pub fn values(&self) -> std::collections::btree_map::Values<String, Option<Validator>> {
        self.0.values()
    }

    pub(crate) fn validate<'de, 'c>(
        &'c self,
        types: &'c BTreeMap<String, Validator>,
        mut parser: Parser<'de>,
        checklist: Option<Checklist<'c>>,
    ) -> Result<(Parser<'de>, Option<Checklist<'c>>)> {
        // Get the enum itself, which should be a map with 1 key-value pair or a string.
        let elem = parser
            .next()
            .ok_or_else(|| Error::FailValidate("expected a enum".to_string()))??;
        let (key, has_value) = match elem {
            Element::Str(v) => (v, false),
            Element::Map(1) => {
                let key = parser
                    .next()
                    .ok_or_else(|| Error::FailValidate("expected a string".to_string()))??;
                if let Element::Str(key) = key {
                    (key, true)
                } else {
                    return Err(Error::FailValidate("expected a string".to_string()));
                }
            }
            _ => return Err(Error::FailValidate("expected an enum".to_string())),
        };

        // Find the matching validator and verify the (possible) content against it
        let validator = self
            .0
            .get(key)
            .ok_or_else(|| Error::FailValidate(format!("{} is not in enum list", key)))?;
        match (validator, has_value) {
            (None, false) => Ok((parser, checklist)),
            (None, true) => Err(Error::FailValidate(format!(
                "enum {} shouldn't have any associated value",
                key
            ))),
            (Some(_), false) => Err(Error::FailValidate(format!(
                "enum {} should have an associated value",
                key
            ))),
            (Some(validator), true) => validator.validate(types, parser, checklist),
        }
    }

    pub(crate) fn query_check(
        &self,
        types: &BTreeMap<String, Validator>,
        other: &Validator,
    ) -> bool {
        match other {
            Validator::Enum(other) => {
                // For each entry in the query's enum, make sure it:
                // 1. Has a corresponding entry in our enum
                // 2. That our enum's matching validator would allow the query's validator
                //    for that enum.
                // 3. If both have a "None" instead of a validator, that's also OK
                other
                    .0
                    .iter()
                    .all(|(other_k, other_v)| match (self.0.get(other_k), other_v) {
                        (Some(Some(validator)), Some(other_v)) => {
                            validator.query_check(types, other_v)
                        }
                        (Some(None), None) => true,
                        _ => false,
                    })
            }
            Validator::Any => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn example_schema() {
        use crate::schema::{Schema, SchemaBuilder};
        // Let's use this as our schema-side validator, and make a full schema.
        let entry_validator = EnumValidator::new()
            .insert("Empty", None)
            .insert("Integer", Some(IntValidator::new().build()))
            .insert("String", Some(StrValidator::new().build()))
            .build();
        let schema_doc = SchemaBuilder::new(Validator::Null)
            .entry_add("item", entry_validator, None)
            .build()
            .unwrap();
        Schema::from_doc(&schema_doc).unwrap();
    }
}
