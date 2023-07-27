//! The fog-pack Validators, for building Schemas and Queries.
//!
//! This submodule contains various validators, which can be transformed into the [`Validator`]
//! enum type for use in a Schema or a Query. Each struct acts as a constructor that can be
//! built into a `Validator`.
//!
//! Validators are not used directly; instead, they should be used to build a Schema or Query,
//! which will run them against fog-pack data.
//!
//! # Examples
//!
//! Say we want to build a Document that acts like a file directory. We want to store the creation
//! time of the directory, and a list of file names with associated Hashes, each of which will be
//! the Hash of a file or directory. Let's also assume we want a valid Unix file name, meaning "/"
//! and NUL cannot be in the name, it cannot be longer than 255 bytes, and shouldn't be "." or
//! "..". A validator for this Document might look like:
//!
//! ```
//! # use fog_pack::validator::*;
//! # use regex::Regex;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let dir = MapValidator::new()
//!     .req_add("created", TimeValidator::new().build())
//!     .req_add("contents", MapValidator::new()
//!         .keys(StrValidator::new()
//!             .nin_add(".")
//!             .nin_add("..")
//!             .ban_char("/\0")
//!             .max_len(255)
//!             .min_len(1)
//!         )
//!         .values(HashValidator::new().build())
//!         .build()
//!     )
//!     .build();
//! # Ok(())
//! # }
//! ```

mod array;
mod bin;
mod bool;
mod checklist;
mod enum_set;
mod float32;
mod float64;
mod hash;
mod identity;
mod integer;
mod lock_id;
mod lockbox;
mod map;
mod multi;
mod serde_regex;
mod str;
mod stream_id;
mod time;

pub use self::array::*;
pub use self::bin::*;
pub use self::bool::*;
pub use self::checklist::*;
pub use self::enum_set::*;
pub use self::float32::*;
pub use self::float64::*;
pub use self::hash::*;
pub use self::identity::*;
pub use self::integer::*;
pub use self::lock_id::*;
pub use self::lockbox::*;
pub use self::map::*;
pub use self::multi::*;
pub use self::str::*;
pub use self::stream_id::*;
pub use self::time::*;
use crate::element::*;
use crate::error::{Error, Result};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// [Unicode Normalization](http://www.unicode.org/reports/tr15/) settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Normalize {
    /// No normalization applied.
    None,
    /// NFC normalization applied.
    NFC,
    /// NFKC normalization applied.
    NFKC,
}

/// A fog-pack Validator, for verifying the form of a fog-pack Document or Entry.
///
/// Validators can be used to verify a fog-pack Document or Entry. Schemas use them for
/// verification, and they are also used by Queries to find matching Entries.
///
/// A Validator can range from the
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Validator {
    Null,
    Bool(Box<BoolValidator>),
    Int(Box<IntValidator>),
    F32(Box<F32Validator>),
    F64(Box<F64Validator>),
    Bin(Box<BinValidator>),
    Str(Box<StrValidator>),
    Array(Box<ArrayValidator>),
    Map(Box<MapValidator>),
    Time(Box<TimeValidator>),
    Hash(Box<HashValidator>),
    Identity(Box<IdentityValidator>),
    StreamId(Box<StreamIdValidator>),
    LockId(Box<LockIdValidator>),
    DataLockbox(Box<DataLockboxValidator>),
    IdentityLockbox(Box<IdentityLockboxValidator>),
    StreamLockbox(Box<StreamLockboxValidator>),
    LockLockbox(Box<LockLockboxValidator>),
    Ref(String),
    Multi(MultiValidator),
    Enum(EnumValidator),
    Any,
}

impl Validator {
    pub fn new_ref(name: impl Into<String>) -> Self {
        Self::Ref(name.into())
    }

    pub fn new_null() -> Self {
        Self::Null
    }

    pub fn new_any() -> Self {
        Self::Any
    }

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
                    .ok_or_else(|| Error::FailValidate("expected null".to_string()))??;
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
                let validator = types.get(ref_name).ok_or_else(|| {
                    Error::FailValidate(format!("validator Ref({}) not in list of types", ref_name))
                })?;
                match validator {
                    Validator::Ref(_) => Err(Error::FailValidate(format!(
                        "validator Ref({}) is itself a Ref",
                        ref_name
                    ))),
                    _ => validator.validate(types, parser, checklist),
                }
            }
            Validator::Multi(validator) => validator.validate(types, parser, checklist),
            Validator::Enum(validator) => validator.validate(types, parser, checklist),
            Validator::Any => {
                read_any(&mut parser)?;
                Ok((parser, checklist))
            }
        }
    }

    pub(crate) fn query_check(
        &self,
        types: &BTreeMap<String, Validator>,
        other: &Validator,
    ) -> bool {
        match self {
            Validator::Null => matches!(other, Validator::Null | Validator::Any),
            Validator::Bool(validator) => validator.query_check(other),
            Validator::Int(validator) => validator.query_check(other),
            Validator::F32(validator) => validator.query_check(other),
            Validator::F64(validator) => validator.query_check(other),
            Validator::Bin(validator) => validator.query_check(other),
            Validator::Str(validator) => validator.query_check(other),
            Validator::Time(validator) => validator.query_check(other),
            Validator::Array(validator) => validator.query_check(types, other),
            Validator::Map(validator) => validator.query_check(types, other),
            Validator::Hash(validator) => validator.query_check(types, other),
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
            Validator::Multi(validator) => validator.query_check(types, other),
            Validator::Enum(validator) => validator.query_check(types, other),
            Validator::Any => false,
        }
    }
}

fn read_any(parser: &mut Parser) -> Result<()> {
    fn get_elem<'a>(parser: &mut Parser<'a>) -> Result<Element<'a>> {
        parser
            .next()
            .ok_or_else(|| Error::FailValidate("expected another value".to_string()))?
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
                read_any(parser)?;
            }
            Ok(())
        }
        Element::Array(len) => {
            for _ in 0..len {
                read_any(parser)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
