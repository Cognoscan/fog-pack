mod array;
mod bin;
mod bool;
mod checklist;
mod float32;
mod float64;
mod hash;
mod identity;
mod integer;
mod lock_id;
mod lockbox;
mod map;
mod serde_regex;
mod str;
mod stream_id;
mod time;

pub use self::array::*;
pub use self::bin::*;
pub use self::bool::*;
pub use self::checklist::*;
pub use self::float32::*;
pub use self::float64::*;
pub use self::hash::*;
pub use self::identity::*;
pub use self::integer::*;
pub use self::lock_id::*;
pub use self::lockbox::*;
pub use self::map::*;
pub use self::str::*;
pub use self::stream_id::*;
pub use self::time::*;
use crate::element::*;
use crate::error::{Error, Result};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Validator {
    Null,
    Bool(BoolValidator),
    Int(IntValidator),
    F32(F32Validator),
    F64(F64Validator),
    Bin(BinValidator),
    Str(StrValidator),
    Array(ArrayValidator),
    Map(MapValidator),
    Time(TimeValidator),
    Hash(HashValidator),
    Identity(IdentityValidator),
    StreamId(StreamIdValidator),
    LockId(LockIdValidator),
    DataLockbox(DataLockboxValidator),
    IdentityLockbox(IdentityLockboxValidator),
    StreamLockbox(StreamLockboxValidator),
    LockLockbox(LockLockboxValidator),
    Ref(String),
    Multi(Vec<Validator>),
    Enum(BTreeMap<String, Option<Validator>>),
    Any,
}

impl Validator {
    pub(crate) fn validate<'de, 'c>(
        &'c self,
        types: &'c BTreeMap<String, Validator>,
        mut parser: Parser<'de>,
        mut checklist: Option<Checklist<'c>>,
    ) -> Result<(Parser<'de>, Option<Checklist<'c>>)> {
        match self {
            Validator::Null => {
                let elem = parser
                    .next()
                    .ok_or(Error::FailValidate("expected null".to_string()))??;
                if let Element::Null = elem {
                    Ok((parser, checklist))
                } else {
                    Err(Error::FailValidate("expected null".to_string()))
                }
            }
            Validator::Bool(validator) => {
                validator.validate(&mut parser)?;
                Ok((parser, checklist))
            }
            Validator::Int(validator) => {
                validator.validate(&mut parser)?;
                Ok((parser, checklist))
            }
            Validator::F32(validator) => {
                validator.validate(&mut parser)?;
                Ok((parser, checklist))
            }
            Validator::F64(validator) => {
                validator.validate(&mut parser)?;
                Ok((parser, checklist))
            }
            Validator::Bin(validator) => {
                validator.validate(&mut parser)?;
                Ok((parser, checklist))
            }
            Validator::Str(validator) => {
                validator.validate(&mut parser)?;
                Ok((parser, checklist))
            }
            Validator::Array(validator) => validator.validate(types, parser, checklist),
            Validator::Map(validator) => validator.validate(types, parser, checklist),
            Validator::Time(validator) => {
                validator.validate(&mut parser)?;
                Ok((parser, checklist))
            }
            Validator::Hash(validator) => {
                validator.validate(&mut parser, &mut checklist)?;
                Ok((parser, checklist))
            }
            Validator::Identity(validator) => {
                validator.validate(&mut parser)?;
                Ok((parser, checklist))
            }
            Validator::StreamId(validator) => {
                validator.validate(&mut parser)?;
                Ok((parser, checklist))
            }
            Validator::LockId(validator) => {
                validator.validate(&mut parser)?;
                Ok((parser, checklist))
            }
            Validator::DataLockbox(validator) => {
                validator.validate(&mut parser)?;
                Ok((parser, checklist))
            }
            Validator::IdentityLockbox(validator) => {
                validator.validate(&mut parser)?;
                Ok((parser, checklist))
            }
            Validator::StreamLockbox(validator) => {
                validator.validate(&mut parser)?;
                Ok((parser, checklist))
            }
            Validator::LockLockbox(validator) => {
                validator.validate(&mut parser)?;
                Ok((parser, checklist))
            }
            Validator::Ref(ref_name) => {
                // Fail if cyclic validation is possible, by banning Ref->Ref.
                // Ref->Multi->... checks are in the Multi validator code further down.
                // All other validators pull at least one element, ensuring infinite
                // recursion/cycling is impossible.
                let validator = types.get(ref_name).ok_or(Error::FailValidate(format!(
                    "validator Ref({}) not in list of types",
                    ref_name
                )))?;
                match validator {
                    Validator::Ref(_) => Err(Error::FailValidate(format!(
                        "validator Ref({}) is itself a Ref",
                        ref_name
                    ))),
                    _ => validator.validate(types, parser, checklist),
                }
            }
            Validator::Multi(multi) => {
                // Iterate through Multi list, but skip any validators that could potentially be
                // cyclic. Banned: Multi->Multi, Multi->Ref->Multi, Multi->Ref->Ref.
                for validator in multi.iter() {
                    let new_parser = parser.clone();
                    let new_checklist = checklist.clone();
                    let new_result = match validator {
                        Validator::Ref(ref_name) => match types.get(ref_name) {
                            None => continue,
                            Some(validator) => match validator {
                                Validator::Ref(_) => continue,
                                Validator::Multi(_) => continue,
                                _ => validator.validate(types, new_parser, new_checklist),
                            },
                        },
                        Validator::Multi(_) => {
                            continue;
                        }
                        _ => validator.validate(types, new_parser, new_checklist),
                    };
                    // We clone the parser each time because the validator modifies its state while
                    // processing. On a pass, we return the parser state that passed
                    if new_result.is_ok() {
                        return new_result;
                    }
                }
                Err(Error::FailValidate(
                    "validator Multi had no passing validators".to_string(),
                ))
            }
            Validator::Enum(enum_map) => {
                // Get the enum itself, which should be a map with 1 key-value pair or a string.
                let elem = parser
                    .next()
                    .ok_or(Error::FailValidate("expected a enum".to_string()))??;
                let (key, has_value) = match elem {
                    Element::Str(v) => (v, false),
                    Element::Map(1) => {
                        let key = parser
                            .next()
                            .ok_or(Error::FailValidate("expected a string".to_string()))??;
                        if let Element::Str(key) = key {
                            (key, true)
                        } else {
                            return Err(Error::FailValidate("expected a string".to_string()));
                        }
                    }
                    _ => return Err(Error::FailValidate("expected an enum".to_string())),
                };
                // Find the matching validator and verify the (possible) content against it
                let validator = enum_map
                    .get(key)
                    .ok_or(Error::FailValidate(format!("{} is not in enum list", key)))?;
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
            Validator::Any => {
                read_any(&mut parser)?;
                Ok((parser, checklist))
            }
        }
    }

    pub fn query_check(&self, types: &BTreeMap<String, Validator>, other: &Validator) -> bool {
        match self {
            Validator::Null => match other {
                Validator::Null => true,
                Validator::Any => true,
                _ => false,
            },
            Validator::Bool(validator) => validator.query_check(other),
            Validator::Int(validator) => validator.query_check(other),
            Validator::F32(validator) => validator.query_check(other),
            Validator::F64(validator) => validator.query_check(other),
            Validator::Bin(validator) => validator.query_check(other),
            Validator::Str(validator) => validator.query_check(other),
            Validator::Time(validator) => validator.query_check(other),
            Validator::Array(validator) => validator.query_check(types, other),
            Validator::Map(validator) => validator.query_check(types, other),
            Validator::Hash(validator) => validator.query_check(other),
            Validator::Identity(validator) => validator.query_check(other),
            Validator::StreamId(validator) => validator.query_check(other),
            Validator::LockId(validator) => validator.query_check(other),
            Validator::DataLockbox(validator) => validator.query_check(other),
            Validator::IdentityLockbox(validator) => validator.query_check(other),
            Validator::StreamLockbox(validator) => validator.query_check(other),
            Validator::LockLockbox(validator) => validator.query_check(other),
            Validator::Ref(ref_name) => match types.get(ref_name) {
                None => false,
                Some(validator) => {
                    if let Validator::Ref(_) = validator {
                        false
                    } else {
                        validator.query_check(types, other)
                    }
                }
            },
            Validator::Multi(list) => list.iter().any(|validator| match validator {
                Validator::Ref(ref_name) => match types.get(ref_name) {
                    None => false,
                    Some(validator) => match validator {
                        Validator::Ref(_) => false,
                        Validator::Multi(_) => false,
                        _ => validator.query_check(types, other),
                    },
                },
                Validator::Multi(_) => false,
                _ => validator.query_check(types, other),
            }),
            Validator::Enum(validator) => {
                match other {
                    Validator::Enum(other) => {
                        // For each entry in the query's enum, make sure it:
                        // 1. Has a corresponding entry in our enum
                        // 2. That our enum's matching validator would allow the query's validator
                        //    for that enum.
                        // 3. If both have a "None" instead of a validator, that's also OK
                        other.iter().all(|(other_k, other_v)| {
                            match (validator.get(other_k), other_v) {
                                (Some(Some(validator)), Some(other_v)) => {
                                    validator.query_check(types, other_v)
                                }
                                (Some(None), None) => true,
                                _ => false,
                            }
                        })
                    }
                    Validator::Any => true,
                    _ => false,
                }
            }
            Validator::Any => false,
        }
    }
}

fn read_any(parser: &mut Parser) -> Result<()> {
    fn get_elem<'a>(parser: &mut Parser<'a>) -> Result<Element<'a>> {
        parser
            .next()
            .ok_or(Error::FailValidate("expected another value".to_string()))?
    }
    let elem = get_elem(parser)?;
    match elem {
        Element::Map(len) => {
            let mut last_key = None;
            for _ in 0..len {
                if let Element::Str(key) = get_elem(parser)? {
                    if let Some(last_key) = last_key {
                        if key <= last_key {
                            return Err(Error::FailValidate(format!(
                                "map keys are unordered: {} follows {}",
                                key, last_key
                            )));
                        }
                    }
                    last_key = Some(key);
                } else {
                    return Err(Error::FailValidate(
                        "expected string for map key".to_string(),
                    ));
                }
                get_elem(parser)?;
            }
            Ok(())
        }
        Element::Array(len) => {
            for _ in 0..len {
                get_elem(parser)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
