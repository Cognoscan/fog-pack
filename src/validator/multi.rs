use super::*;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::default::Default;

/// "Multi" validator that checks with several validators at once.
///
/// This validator will accept any value that passes at least one of its contained validators. This
/// can be used like an "any of" operator, or a logical OR of validators.
///
/// When this validator is used, the contained validators are checked in order, passing when the 
/// first contained validator passes. When performing [`Entry`] validation, this can mean that a 
/// linked document may be added to the list of documents needed for final validation, even if 
/// another contained validator (later in the list) would also pass without it.
///
/// When going through the contained validators, some rules are followed to avoid possible cyclic
/// references:
///
/// - Contained Multi-validators are skipped
/// - Contained Ref validators that refer to a Multi-validator are skipped.
/// - Contained Ref validators that refer to a Ref validator are skipped.
///
/// More succintly, the banned sequences are: Multi->Multi, Multi->Ref->Multi, Multi->Ref->Ref.
///
/// # Query Checking
///
/// The validator for a query must be accepted by at least one of the validators in the
/// Multi-validator. Contained validators that violate the cyclic reference rules are skipped (see
/// above).
///
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MultiValidator(pub Vec<Validator>);

impl MultiValidator {
    /// Make a new validator with the default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new validator to the list.
    pub fn push(mut self, validator: Validator) -> Self {
        self.0.push(validator);
        self
    }

    /// Build this into a [`Validator`] enum.
    pub fn build(self) -> Validator {
        Validator::Multi(self)
    }

    pub fn iter(&self) -> std::slice::Iter<Validator> {
        self.0.iter()
    }

    pub(crate) fn validate<'de, 'c>(
        &'c self,
        types: &'c BTreeMap<String, Validator>,
        parser: Parser<'de>,
        checklist: Option<Checklist<'c>>,
    ) -> Result<(Parser<'de>, Option<Checklist<'c>>)> {
        // Iterate through Multi list, but skip any validators that could potentially be
        // cyclic. Banned: Multi->Multi, Multi->Ref->Multi, Multi->Ref->Ref.
        for validator in self.0.iter() {
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

    pub(crate) fn query_check(
        &self,
        types: &BTreeMap<String, Validator>,
        other: &Validator,
    ) -> bool {
        self.0.iter().any(|validator| match validator {
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
        })
    }
}
