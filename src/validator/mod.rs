mod bool;
mod float32;
mod float64;
mod integer;
mod str;
mod bin;
mod time;

pub use self::bool::*;
pub use self::float32::*;
pub use self::float64::*;
pub use self::integer::*;
pub use self::str::*;
pub use self::bin::*;
pub use self::time::*;
use std::collections::BTreeMap;
use crate::error::{Error, Result};
use crate::element::*;

pub enum Validator {
    Null,
    Bool(BoolValidator),
    Int(IntValidator),
    F32(F32Validator),
    F64(F64Validator),
    Bin(BinValidator),
    Str(StrValidator),
    Time(TimeValidator),
    Hash,
    Identity,
    StreamId,
    LockId,
    DataLockbox,
    IdentityLockbox,
    StreamLockbox,
    LockLockbox,
    Ref(String),
    Multi(Vec<Validator>),
    Enum(BTreeMap<String, Option<Validator>>),
    Any,
}

pub struct ValidatorContext<'c> {
    types: &'c BTreeMap<String, Validator>
}

impl Validator {
    pub fn validate<'a>(
        &self,
        context: &ValidatorContext,
        mut parser: Parser<'a>
    ) -> Result<Parser<'a>> {
        match self {
            Validator::Null => {
                let elem = parser
                    .next()
                    .ok_or(Error::FailValidate("expected null".to_string()))??;
                if let Element::Null = elem { Ok(parser) }
                else {
                    Err(Error::FailValidate("expected null".to_string()))
                }
            },
            Validator::Bool(validator) => {
                validator.validate(&mut parser)?;
                Ok(parser)
            },
            Validator::Int(validator) => {
                validator.validate(&mut parser)?;
                Ok(parser)
            },
            Validator::F32(validator) => {
                validator.validate(&mut parser)?;
                Ok(parser)
            },
            Validator::F64(validator) => {
                validator.validate(&mut parser)?;
                Ok(parser)
            },
            Validator::Bin(validator) => {
                validator.validate(&mut parser)?;
                Ok(parser)
            },
            Validator::Str(validator) => {
                validator.validate(&mut parser)?;
                Ok(parser)
            },
            Validator::Time(validator) => {
                validator.validate(&mut parser)?;
                Ok(parser)
            },
            Validator::Hash => Ok(parser),
            Validator::Identity => Ok(parser),
            Validator::StreamId => Ok(parser),
            Validator::LockId => Ok(parser),
            Validator::DataLockbox => Ok(parser),
            Validator::IdentityLockbox => Ok(parser),
            Validator::StreamLockbox => Ok(parser),
            Validator::LockLockbox => Ok(parser),
            Validator::Ref(ref_name) => {
                let validator = context.types.get(ref_name)
                    .ok_or(Error::FailValidate(format!("validator Ref({}) not in list of types", ref_name)))?;
                validator.validate(context, parser)
            },
            Validator::Multi(multi) => {
                for validator in multi.iter() {
                    // We clone the parser each time because the validator modifies its state while 
                    // processing. On a pass, we return the parser state that passed
                    let new_parser = parser.clone();
                    if let Ok(new_parser) = validator.validate(context, new_parser) {
                        return Ok(new_parser);
                    }
                }
                Err(Error::FailValidate("validator Multi had no passing validators".to_string()))
            },
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
                        if let Element::Str(key) = key { (key, true) } else {
                            return Err(Error::FailValidate("expected a string".to_string()));
                        }
                    },
                    _ => return Err(Error::FailValidate("expected an enum".to_string())),
                };
                // Find the matching validator and verify the (possible) content against it
                let validator = enum_map.get(key)
                    .ok_or(Error::FailValidate(format!("{} is not in enum list", key)))?;
                match (validator, has_value) {
                    (None, false) => Ok(parser),
                    (None, true) => Err(Error::FailValidate(format!("enum {} shouldn't have any associated value", key))),
                    (Some(_), false) => Err(Error::FailValidate(format!("enum {} should have an associated value", key))),
                    (Some(validator), true) => validator.validate(context, parser),
                }
            },
            Validator::Any => {
                read_any(&mut parser)?;
                Ok(parser)
            },
        }
    }

    pub fn query_check(
        &self, 
        context: &ValidatorContext,
        other: &Validator
    ) -> bool {
        match self {
            Validator::Null => {
                match other {
                    Validator::Null => true,
                    Validator::Any => true,
                    _ => false,
                }
            }
            Validator::Bool(validator) => validator.query_check(other),
            Validator::Int(validator) => validator.query_check(other),
            Validator::F32(validator) => validator.query_check(other),
            Validator::F64(validator) => validator.query_check(other),
            Validator::Bin(validator) => validator.query_check(other),
            Validator::Str(validator) => validator.query_check(other),
            Validator::Time(validator) => validator.query_check(other),
            Validator::Hash => false,
            Validator::Identity => false,
            Validator::StreamId => false,
            Validator::LockId => false,
            Validator::DataLockbox => false,
            Validator::IdentityLockbox => false,
            Validator::StreamLockbox => false,
            Validator::LockLockbox => false,
            Validator::Ref(ref_name) => {
                match context.types.get(ref_name) {
                    None => false,
                    Some(validator) => validator.query_check(context, other),
                }
            },
            Validator::Multi(list) => list.iter().any(|validator| validator.query_check(context, other)),
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
                                (Some(Some(validator)), Some(other_v)) => validator.query_check(context, other_v),
                                (Some(None), None) => true,
                                _ => false,
                            }
                        })
                    },
                    Validator::Any => true,
                    _ => false,
                }
            },
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
                }
                else {
                    return Err(Error::FailValidate("expected string for map key".to_string()));
                }
                get_elem(parser)?;
            }
            Ok(())
        },
        Element::Array(len) => {
            for _ in 0..len {
                get_elem(parser)?;
            }
            Ok(())
        },
        _ => Ok(()),
    }
}

