use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}

#[inline]
fn usize_is_zero(v: &usize) -> bool {
    *v == 0
}

#[inline]
fn usize_is_max(v: &usize) -> bool {
    *v == usize::MAX
}

macro_rules! lockbox_validator {
    ($t: ty, $e: ident, $v: ident) => {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields, default)]
        pub struct $v {
            #[serde(skip_serializing_if = "String::is_empty")]
            pub comment: String,
            #[serde(skip_serializing_if = "usize_is_max")]
            pub max_len: usize,
            #[serde(skip_serializing_if = "usize_is_zero")]
            pub min_len: usize,
            #[serde(skip_serializing_if = "is_false")]
            pub size: bool,
        }

        impl std::default::Default for $v {
            fn default() -> Self {
                Self {
                    comment: String::new(),
                    max_len: usize::MAX,
                    min_len: usize::MIN,
                    size: false,
                }
            }
        }

        impl $v {
            pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
                let elem = parser
                    .next()
                    .ok_or(Error::FailValidate(concat!("Expected a ",stringify!($t)).to_string()))??;
                let elem = if let Element::$e(v) = elem {
                    v
                } else {
                    return Err(Error::FailValidate(format!(
                                concat!("Expected ", stringify!($t), ", got {}"),
                                elem.name()
                    )));
                };

                if elem.as_bytes().len() > self.max_len {
                    return Err(Error::FailValidate(
                            concat!(stringify!($t), " is longer than max_len").to_string()
                    ));
                }
                if elem.as_bytes().len() < self.min_len {
                    return Err(Error::FailValidate(
                            concat!(stringify!($t), " is shorter than min_len").to_string()
                    ));
                }

                Ok(())
            }

            fn query_check_self(&self, other: &Self) -> bool {
                self.size || (usize_is_max(&other.max_len) && usize_is_zero(&other.min_len))
            }

            pub(crate) fn query_check(&self, other: &Validator) -> bool {
                match other {
                    Validator::$e(other) => self.query_check_self(other),
                    Validator::Multi(list) => list.iter().all(|other| match other {
                        Validator::$e(other) => self.query_check_self(other),
                        _ => false,
                    }),
                    Validator::Any => true,
                    _ => false,
                }
            }
        }
    }
}

lockbox_validator!(DataLockbox, DataLockbox, DataLockboxValidator);
lockbox_validator!(IdentityLockbox, IdentityLockbox, IdentityLockboxValidator);
lockbox_validator!(StreamLockbox, StreamLockbox, StreamLockboxValidator);
lockbox_validator!(LockLockbox, LockLockbox, LockLockboxValidator);
