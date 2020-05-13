use std::io;
use std::io::Error;
use std::io::ErrorKind::InvalidData;

use decode::*;
use super::Validator;

/// Lock type validator
#[derive(Clone, Debug)]
pub struct ValidLock {
    max_len: usize,
    query: bool,
}

impl ValidLock {
    pub fn new(is_query: bool) -> ValidLock {
        ValidLock {
            max_len: usize::max_value(),
            query: is_query
        }
    }

    /// Update the validator. Returns `Ok(true)` if everything is read out Ok, `Ok(false)` if we 
    /// don't recognize the field type or value, and `Err` if we recognize the field but fail to 
    /// parse the expected contents. The updated `raw` slice reference is only accurate if 
    /// `Ok(true)` was returned.
    pub fn update(&mut self, field: &str, raw: &mut &[u8]) -> io::Result<bool> {
        // Note about this match: because fields are lexicographically ordered, the items in this 
        // match statement are either executed sequentially or are skipped.
        match field {
            "max_len" => {
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.max_len = len as usize;
                    Ok(true)
                }
                else {
                    Err(Error::new(InvalidData, "Lockbox validator expected non-negative value for `max_len` field"))
                }
            }
            "query" => {
                self.query = read_bool(raw)?;
                Ok(true)
            }
            "type" => if "Lock" == read_str(raw)? { Ok(true) } else { Err(Error::new(InvalidData, "Type doesn't match Lock")) },
            _ => Err(Error::new(InvalidData, "Unknown fields not allowed in Lockbox validator")),
        }
    }

    /// Final check on the validator. Returns true if at least one value can still pass the 
    /// validator.
    pub fn finalize(&mut self) -> bool {
        true
    }

    pub fn validate(&self, field: &str, doc: &mut &[u8]) -> io::Result<()> {
        let value = read_lockbox(doc)?;
        if value.len() > self.max_len {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" contains lockbox longer than max length of {}", field, self.max_len)))
        }
        else {
            Ok(())
        }
    }

    /// Intersection of Lockbox with other Validators. Returns Err only if `query` is true and the 
    /// other validator contains non-allowed query parameters.
    pub fn intersect(&self, other: &Validator, query: bool) -> Result<Validator, ()> {
        if query && !self.query { return Err(()); }
        match other {
            Validator::Lockbox(other) => {
                if query && !self.query && (other.max_len < usize::max_value())
                {
                    Err(())
                }
                else {
                    let new_validator = ValidLock {
                        max_len: self.max_len.min(other.max_len),
                        query: self.query && other.query,
                    };
                    Ok(Validator::Lockbox(new_validator))
                }
            },
            Validator::Valid => Ok(Validator::Lockbox(self.clone())),
            _ => Ok(Validator::Invalid),
        }
    }
}
