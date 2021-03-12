use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use crate::Hash;
use serde::{Deserialize, Deserializer, Serialize};
use std::default::Default;

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}

fn get_validator<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Box<Validator>>, D::Error> {
    // Decode the validator. If this function is called, there should be an actual validator
    // present. Otherwise we fail. In other words, no `null` allowed.
    Ok(Some(Box::new(Validator::deserialize(deserializer)?)))
}

/// Validator for hashes.
///
/// This validator type will only pass hash values. Validation passes if:
///
/// - If the `in` list is not empty, the hash must be among the hashes in the list.
/// - The hash must not be among the hashes in the `nin` list.
/// - If `link` has a validator, the data in the Document referred to by the hash must pass that
///     validator.
/// - If the `schema` list is not empty, the Document referred to by the hash must use one of the
///     schemas listed. A `null` value on the list means the schema containing *this* validator is
///     also accepted.
///
/// **The `link` and `schema` checks only apply when validating Entries, not Documents.**
///
/// Hash validators are unique in that they do not always complete validation after examining a
/// single value. If used for checking an Entry, they can require an additional Document for
/// validation. For this reason, completing validation of an Entry requires completing a
/// [`DataChecklist`][DataChecklist]. See the [`Schema`][crate::Schema] documentation for more
/// details.
///
/// # Defaults
///
/// Fields that aren't specified for the validator use their defaults instead. The defaults for
/// each field are:
///
/// - comment: ""
/// - link: None
/// - schema: empty
/// - in_list: empty
/// - nin_list: empty
/// - query: false
/// - link_ok: false
/// - schema_ok: false
///
/// # Query Checking
///
/// Queries for hashes are only allowed to use non-default values for each field if the
/// corresponding query permission is set in the schema's validator:
///
/// - query: `in` and `nin` lists
/// - link_ok: `link`
/// - schema_ok: `schema`
///
/// In addition, if there is a validator for `link`, it is validated against the schema validator's
/// `link` validator.
///
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct HashValidator {
    /// An optional comment explaining the validator.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    /// An optional validator used to validate the data in a Document linked to by the hash. If
    /// not present, any data is allowed in the linked Document.
    #[serde(
        skip_serializing_if = "Option::is_none",
        deserialize_with = "get_validator"
    )]
    pub link: Option<Box<Validator>>,
    /// A list of allowed schemas for a Document linked to by the hash. A `None` value refers to
    /// the validator's containing schema. For validators used in queries, `None` is skipped. If
    /// empty, this list is ignored during checking.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub schema: Vec<Option<Hash>>,
    /// A vector of specific allowed values, stored under the `in` field. If empty, this vector is not checked against.
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<Hash>,
    /// A vector of specific unallowed values, stored under the `nin` field.
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<Hash>,
    /// If true, queries against matching spots may have values in the `in` or `nin` lists.
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
    /// If true, queries against matching spots may have a validator in `link`.
    #[serde(skip_serializing_if = "is_false")]
    pub link_ok: bool,
    /// If true, queries against matching spots may have values in the `schema` list.
    #[serde(skip_serializing_if = "is_false")]
    pub schema_ok: bool,
}

impl Default for HashValidator {
    fn default() -> Self {
        Self {
            comment: String::new(),
            link: None,
            schema: Vec::new(),
            in_list: Vec::new(),
            nin_list: Vec::new(),
            query: false,
            link_ok: false,
            schema_ok: false,
        }
    }
}

impl HashValidator {
    /// Make a new validator with the default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the `link` validator.
    pub fn link(mut self, link: Validator) -> Self {
        self.link = Some(Box::new(link));
        self
    }

    /// Add a Hash to the `schema` list.
    pub fn schema_add(mut self, add: impl Into<Hash>) -> Self {
        self.schema.push(Some(add.into()));
        self
    }

    /// Allow referred-to documents to use this validator's containing schema.
    pub fn schema_self(mut self) -> Self {
        self.schema.push(None);
        self
    }

    /// Add a value to the `in` list.
    pub fn in_add(mut self, add: impl Into<Hash>) -> Self {
        self.in_list.push(add.into());
        self
    }

    /// Add a value to the `nin` list.
    pub fn nin_add(mut self, add: impl Into<Hash>) -> Self {
        self.nin_list.push(add.into());
        self
    }

    /// Set whether or not queries can use the `in` and `nin` lists.
    pub fn query(mut self, query: bool) -> Self {
        self.query = query;
        self
    }

    /// Set whether or not queries can use `link`.
    pub fn link_ok(mut self, link_ok: bool) -> Self {
        self.link_ok = link_ok;
        self
    }

    /// Set whether or not queries can use `schema`.
    pub fn schema_ok(mut self, schema_ok: bool) -> Self {
        self.schema_ok = schema_ok;
        self
    }

    /// Build this into a [`Validator`] enum.
    pub fn build(self) -> Validator {
        Validator::Hash(self)
    }

    pub(crate) fn validate<'c>(
        &'c self,
        parser: &mut Parser,
        checklist: &mut Option<Checklist<'c>>,
    ) -> Result<()> {
        let elem = parser
            .next()
            .ok_or_else(|| Error::FailValidate("Expected a hash".to_string()))??;
        let val = if let Element::Hash(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "Expected Hash, got {}",
                elem.name()
            )));
        };

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

        if let Some(checklist) = checklist {
            match (self.schema.is_empty(), self.link.as_ref()) {
                (false, Some(link)) => checklist.insert(val, Some(&self.schema), Some(link)),
                (false, None) => checklist.insert(val, Some(&self.schema), None),
                (true, Some(link)) => checklist.insert(val, None, Some(link)),
                _ => (),
            }
        }

        Ok(())
    }

    fn query_check_self(&self, types: &BTreeMap<String, Validator>, other: &HashValidator) -> bool {
        let initial_check = (self.query || (other.in_list.is_empty() && other.nin_list.is_empty()))
            && (self.link_ok || other.link.is_none())
            && (self.schema_ok || other.schema.is_empty());
        if !initial_check {
            return false;
        }
        if self.link_ok {
            match (&self.link, &other.link) {
                (None, None) => true,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (Some(s), Some(o)) => s.query_check(types, o.as_ref()),
            }
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
            Validator::Hash(other) => self.query_check_self(types, other),
            Validator::Multi(list) => list.iter().all(|other| match other {
                Validator::Hash(other) => self.query_check_self(types, other),
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
        let schema = HashValidator::default();
        let mut ser = FogSerializer::default();
        schema.serialize(&mut ser).unwrap();
        let expected: Vec<u8> = vec![0x80];
        let actual = ser.finish();
        println!("expected: {:x?}", expected);
        println!("actual:   {:x?}", actual);
        assert_eq!(expected, actual);

        let mut de = FogDeserializer::with_debug(&actual, "    ");
        let decoded = HashValidator::deserialize(&mut de).unwrap();
        println!("{}", de.get_debug().unwrap());
        assert_eq!(schema, decoded);
    }

    #[test]
    fn verify_simple() {
        let mut schema = HashValidator {
            link: Some(Box::new(Validator::Hash(HashValidator::default()))),
            ..HashValidator::default()
        };
        schema
            .schema
            .push(Some(Hash::new(b"Pretend I am a real schema")));
        schema.schema.push(None);
        let mut ser = FogSerializer::default();

        Hash::new(b"Data to make a hash")
            .serialize(&mut ser)
            .unwrap();
        let encoded = ser.finish();
        let mut parser = Parser::new(&encoded);
        let fake_schema = Hash::new(b"Pretend I, too, am a real schema");
        let fake_types = BTreeMap::new();
        let mut checklist = Some(Checklist::new(&fake_schema, &fake_types));
        schema
            .validate(&mut parser, &mut checklist)
            .expect("should succeed as a validator");
    }
}
