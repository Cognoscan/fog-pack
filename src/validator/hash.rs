use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use crate::Hash;
use serde::{Deserialize, Deserializer, Serialize};
use std::default::Default;

const DEFAULT_HASH_RAW: &[u8] = &[
    0x01, 0xda, 0x22, 0x3b, 0x09, 0x96, 0x7c, 0x5b, 0xd2, 0x11, 0x07, 0x43, 0x30, 0x7e, 0x0a, 0xf6,
    0xd3, 0x9f, 0x61, 0x72, 0x0a, 0xa7, 0x21, 0x8a, 0x64, 0x0a, 0x08, 0xee, 0xd1, 0x2d, 0xd5, 0x75,
    0xc7,
];

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}
#[inline]
fn hash_is_default(v: &Hash) -> bool {
    let bytes: &[u8] = v.as_ref();
    bytes == DEFAULT_HASH_RAW
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
pub struct HashValidator {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    #[serde(skip_serializing_if = "hash_is_default")]
    pub default: Hash,
    #[serde(
        skip_serializing_if = "Option::is_none",
        deserialize_with = "get_validator"
    )]
    pub link: Option<Box<Validator>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub schema: Vec<Option<Hash>>,
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<Hash>,
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<Hash>,
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub link_ok: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub schema_ok: bool,
}

impl Default for HashValidator {
    fn default() -> Self {
        use std::convert::TryFrom;
        Self {
            comment: String::new(),
            default: Hash::try_from(DEFAULT_HASH_RAW).unwrap(),
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
    pub(crate) fn validate<'c>(
        &'c self,
        parser: &mut Parser,
        checklist: &mut Option<Checklist<'c>>,
    ) -> Result<()> {
        let elem = parser
            .next()
            .ok_or(Error::FailValidate("Expected a hash".to_string()))??;
        let val = if let Element::Hash(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "Expected Hash, got {}",
                elem.name()
            )));
        };

        // in/nin checks
        if self.in_list.len() > 0 {
            if !self.in_list.iter().any(|v| *v == val) {
                return Err(Error::FailValidate(
                    "Timestamp is not on `in` list".to_string(),
                ));
            }
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

    fn query_check_self(&self, other: &Self) -> bool {
        (self.query || (other.in_list.is_empty() && other.nin_list.is_empty()))
            && (self.link_ok || other.link.is_none())
            && (self.schema_ok || other.schema.is_empty())
    }

    pub(crate) fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::Hash(other) => self.query_check_self(other),
            Validator::Multi(list) => list.iter().all(|other| match other {
                Validator::Hash(other) => self.query_check_self(other),
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

        let mut de = FogDeserializer::with_debug(&actual, "    ".into());
        let decoded = HashValidator::deserialize(&mut de).unwrap();
        println!("{}", de.get_debug().unwrap());
        assert_eq!(schema, decoded);
    }

    #[test]
    fn verify_simple() {
        let mut schema = HashValidator::default();
        schema.link = Some(Box::new(Validator::Hash(HashValidator::default())));
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
