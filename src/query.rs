use std::collections::BTreeMap;

use crate::validator::Validator;
use crate::Entry;
use crate::{
    de::FogDeserializer,
    element::Parser,
    error::{Error, Result},
    ser::FogSerializer,
    validator::{Checklist, DataChecklist},
    value_ref::ValueRef,
    MAX_QUERY_SIZE,
};
use fog_crypto::hash::Hash;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InnerQuery {
    key: String,
    query: Validator,
}

/// A new Query, ready for encoding.
///
/// New queries must first be encoded by a schema, and can then be decoded later by that same
/// schema into a proper [`Query`][Query].
///
/// A Query contains a single validator and a key, which may be used for querying a set of Entries.
/// Entries that pass the validator can be returned as the query results.
///
/// Queries are not meant to be used without associated context; they should be provided alongside
/// information about what Document they are being used to query.
#[derive(Clone, Debug)]
pub struct NewQuery {
    inner: InnerQuery,
}

impl NewQuery {
    pub fn new(key: &str, query: Validator) -> Self {
        Self {
            inner: InnerQuery {
                key: key.to_owned(),
                query,
            },
        }
    }

    pub fn validator(&self) -> &Validator {
        &self.inner.query
    }

    pub fn key(&self) -> &str {
        &self.inner.key
    }

    pub(crate) fn complete(self, max_regex: u8) -> Result<Vec<u8>> {
        fn parse_validator(v: &Validator) -> usize {
            match v {
                Validator::Str(val) => val.matches.is_some() as usize,
                Validator::Map(val) => {
                    val.keys.matches.is_some() as usize
                        + val
                            .req
                            .values()
                            .fold(0, |acc, val| acc + parse_validator(val))
                        + val
                            .opt
                            .values()
                            .fold(0, |acc, val| acc + parse_validator(val))
                        + val.values.as_ref().map_or(0, |val| parse_validator(val))
                }
                Validator::Array(val) => {
                    val.contains
                        .iter()
                        .fold(0, |acc, val| acc + parse_validator(val))
                        + parse_validator(val.items.as_ref())
                        + val
                            .prefix
                            .iter()
                            .fold(0, |acc, val| acc + parse_validator(val))
                }
                Validator::Hash(val) => val.link.as_ref().map_or(0, |val| parse_validator(val)),
                Validator::Enum(val) => val
                    .values()
                    .fold(0, |acc, val| acc + val.as_ref().map_or(0, parse_validator)),
                Validator::Multi(val) => val.iter().fold(0, |acc, val| acc + parse_validator(val)),
                _ => 0,
            }
        }
        let regexes = parse_validator(&self.inner.query);
        if regexes > (max_regex as usize) {
            return Err(Error::FailValidate(format!(
                "Found {} regexes in query, only {} allowed",
                regexes, max_regex
            )));
        }
        let mut ser = FogSerializer::default();
        self.inner.serialize(&mut ser)?;
        let buf = ser.finish();
        if buf.len() > MAX_QUERY_SIZE {
            Err(Error::LengthTooLong {
                max: MAX_QUERY_SIZE,
                actual: buf.len(),
            })
        } else {
            Ok(buf)
        }
    }
}

/// For querying Entries.
///
/// A Query contains a single validator and a key, which may be used for querying a set of Entries.
/// Entries that pass the validator can be returned as the query results.
///
/// Queries are not meant to be used without associated context; they should be provided alongside
/// information about what Document they are being used to query.
#[derive(Clone, Debug)]
pub struct Query {
    inner: InnerQuery,
    schema: Hash,
    types: BTreeMap<String, Validator>,
}

impl Query {
    pub(crate) fn new(buf: Vec<u8>, max_regex: u8) -> Result<Self> {
        // Check to see how many regexes are in the validator
        fn parse_validator(v: &ValueRef) -> usize {
            // First, unpack the validator enum
            if let ValueRef::Map(map) = v {
                // Enums should be a map with one key-value pair
                if map.len() > 1 {
                    return 0;
                }
                match map.iter().next() {
                    // String validator
                    Some((&"Str", val)) => val["matches"].is_str() as usize,
                    // Map validator
                    Some((&"Map", val)) => {
                        if !val.is_map() {
                            return 0;
                        }
                        let key_matches = if val["keys"]["matches"].is_str() {
                            1
                        } else {
                            0
                        };
                        let req_matches = val["req"].as_map().map_or(0, |map| {
                            map.values().fold(0, |acc, val| acc + parse_validator(val))
                        });
                        let opt_matches = val["opt"].as_map().map_or(0, |map| {
                            map.values().fold(0, |acc, val| acc + parse_validator(val))
                        });
                        let values_matches = parse_validator(&val["values"]);
                        key_matches + req_matches + opt_matches + values_matches
                    }
                    // Array validator
                    Some((&"Array", val)) => {
                        if !val.is_map() {
                            return 0;
                        }
                        let contains_matches = val["contains"].as_array().map_or(0, |array| {
                            array.iter().fold(0, |acc, val| acc + parse_validator(val))
                        });
                        let items_matches = parse_validator(&val["items"]);
                        let prefix_matches = val["contains"].as_array().map_or(0, |array| {
                            array.iter().fold(0, |acc, val| acc + parse_validator(val))
                        });
                        contains_matches + items_matches + prefix_matches
                    }
                    // Hash validator
                    Some((&"Hash", val)) => {
                        if !val.is_map() {
                            return 0;
                        }
                        parse_validator(&val["link"])
                    }
                    // Enum validator
                    Some((&"Enum", val)) => val.as_map().map_or(0, |map| {
                        map.values().fold(0, |acc, val| acc + parse_validator(val))
                    }),
                    // Multi validator
                    Some((&"Multi", val)) => val.as_array().map_or(0, |array| {
                        array.iter().fold(0, |acc, val| acc + parse_validator(val))
                    }),
                    _ => 0,
                }
            } else {
                0
            }
        }
        let mut de = FogDeserializer::new(&buf);
        let regex_check = ValueRef::deserialize(&mut de)?;
        let regexes = parse_validator(&regex_check["query"]);
        if regexes > (max_regex as usize) {
            return Err(Error::FailValidate(format!(
                "Found {} regexes in query, only {} allowed",
                regexes, max_regex
            )));
        }

        // Parse into an actual validator
        let mut de = FogDeserializer::new(&buf);
        let inner = InnerQuery::deserialize(&mut de)?;
        Ok(Self {
            inner,
            schema: Hash::new(&[]),
            types: BTreeMap::new(),
        })
    }

    pub fn validator(&self) -> &Validator {
        &self.inner.query
    }

    pub fn key(&self) -> &str {
        &self.inner.key
    }

    pub fn query(&self, entry: &Entry) -> Result<DataChecklist<()>> {
        let parser = Parser::new(entry.data());
        let checklist = Some(Checklist::new(&self.schema, &self.types));
        let (_, checklist) = self.inner.query.validate(&self.types, parser, checklist)?;
        Ok(DataChecklist::from_checklist(checklist.unwrap(), ()))
    }
}

#[cfg(test)]
mod test {
    use regex::Regex;

    use crate::validator::{KeyValidator, MapValidator, StrValidator};

    use super::*;

    #[test]
    fn max_regex_in_key() {
        let map = MapValidator {
            keys: KeyValidator {
                matches: Some(Box::new(Regex::new("[a-z]").unwrap())),
                ..Default::default()
            },
            ..Default::default()
        };

        let validator = Validator::Map(map);
        NewQuery::new("test", validator.clone())
            .complete(0)
            .unwrap_err();
        let enc_query = NewQuery::new("test", validator).complete(1).unwrap();
        assert!(Query::new(enc_query.clone(), 0).is_err());
        assert!(Query::new(enc_query.clone(), 1).is_ok());
        assert!(Query::new(enc_query, 2).is_ok());
    }

    #[test]
    fn max_regex_in_str() {
        let matches = Some(Box::new(Regex::new("[a-z]").unwrap()));
        let validator = Validator::Str(StrValidator {
            matches,
            ..Default::default()
        });
        NewQuery::new("test", validator.clone())
            .complete(0)
            .unwrap_err();
        let enc_query = NewQuery::new("test", validator).complete(1).unwrap();
        assert!(Query::new(enc_query.clone(), 0).is_err());
        assert!(Query::new(enc_query.clone(), 1).is_ok());
        assert!(Query::new(enc_query, 2).is_ok());
    }
}
