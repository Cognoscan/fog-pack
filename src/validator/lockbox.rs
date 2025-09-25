use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use educe::Educe;
use serde::{Deserialize, Serialize};

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}

#[inline]
fn u32_is_zero(v: &u32) -> bool {
    *v == 0
}

#[inline]
fn u32_is_max(v: &u32) -> bool {
    *v == u32::MAX
}

macro_rules! lockbox_validator {
    ($t: ty, $e: ident, $v: ident, $link:expr, $name:expr) => {
        #[doc = "Validator for a [`"]
        #[doc = $name]
        #[doc = "`]["]
        #[doc = $link]
        #[doc = "].\n\n"]
        #[doc = "This validator will only pass a "]
        #[doc = $name]
        #[doc = " value. Validation passes if:\n\n"]
        #[doc = "- The number of bytes in the lockbox is less than or equal to `max_len`\n"]
        #[doc = "- The number of bytes in the lockbox is greater than or equal to `min_len`\n"]
        /// # Defaults
        ///
        /// Fields that aren't specified for the validator use their defaults instead. The defaults for
        /// each field are:
        ///
        /// - comment: ""
        /// - max_len: u32::MAX
        /// - min_len: 0
        /// - size: false
        ///
        /// # Query Checking
        ///
        /// Queries for lockboxes are only allowed to use non default values for `max_len` and
        /// `min_len` if `size` is set in the schema's validator.
        ///
        #[derive(Educe, Clone, Debug, Serialize, Deserialize)]
        #[educe(PartialEq, Default)]
        #[serde(deny_unknown_fields, default)]
        pub struct $v {
            /// An optional comment explaining the validator.
            #[educe(PartialEq(ignore))]
            #[serde(skip_serializing_if = "String::is_empty")]
            pub comment: String,
            /// Set the maximum allowed number of bytes.
            #[educe(Default = u32::MAX)]
            #[serde(skip_serializing_if = "u32_is_max")]
            pub max_len: u32,
            /// Set the minimum allowed number of bytes.
            #[educe(Default = u32::MIN)]
            #[serde(skip_serializing_if = "u32_is_zero")]
            pub min_len: u32,
            /// If true, queries against matching spots may set the `min_len` and `max_len` values
            /// to non-defaults.
            #[serde(skip_serializing_if = "is_false")]
            pub size: bool,
        }

        impl $v {

            /// Make a new validator with the default configuration.
            pub fn new() -> Self {
                Self::default()
            }

            /// Set a comment for the validator.
            pub fn comment(mut self, comment: impl Into<String>) -> Self {
                self.comment = comment.into();
                self
            }

            /// Set the maximum number of allowed bytes.
            pub fn max_len(mut self, max_len: u32) -> Self {
                self.max_len = max_len;
                self
            }

            /// Set the minimum number of allowed bytes.
            pub fn min_len(mut self, min_len: u32) -> Self {
                self.min_len = min_len;
                self
            }

            /// Set whether or not queries can use the `max_len` and `min_len` values.
            pub fn size(mut self, size: bool) -> Self {
                self.size = size;
                self
            }

            /// Build this into a [`Validator`] enum.
            pub fn build(self) -> Validator {
                Validator::$e(Box::new(self))
            }

            pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
                let elem = parser
                    .next()
                    .ok_or_else(|| Error::FailValidate(concat!("Expected a ",$name).to_string()))??;
                let elem = if let Element::$e(v) = elem {
                    v
                } else {
                    return Err(Error::FailValidate(format!(
                                concat!("Expected ", $name, ", got {}"),
                                elem.name()
                    )));
                };

                let len = elem.as_bytes().len() as u32;
                if len > self.max_len {
                    return Err(Error::FailValidate(
                            concat!($name, " is longer than max_len").to_string()
                    ));
                }
                if len < self.min_len {
                    return Err(Error::FailValidate(
                            concat!($name, " is shorter than min_len").to_string()
                    ));
                }

                Ok(())
            }

            fn query_check_self(&self, other: &Self) -> bool {
                self.size || (u32_is_max(&other.max_len) && u32_is_zero(&other.min_len))
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
    };

    ($t: ty, $e: ident, $v: ident) => {
        lockbox_validator!($t, $e, $v, concat!("fog_crypto::lockbox::", stringify!($t)), stringify!($t));
    }
}

lockbox_validator!(DataLockbox, DataLockbox, DataLockboxValidator);
lockbox_validator!(IdentityLockbox, IdentityLockbox, IdentityLockboxValidator);
lockbox_validator!(StreamLockbox, StreamLockbox, StreamLockboxValidator);
lockbox_validator!(LockLockbox, LockLockbox, LockLockboxValidator);
