use Error;
use decode::*;
use super::Validator;

/// Lock type validator
#[derive(Clone, Debug)]
pub struct ValidLock {
    max_len: usize,
    size: bool,
    size_used: bool,
}

impl ValidLock {
    pub fn new(is_query: bool) -> ValidLock {
        ValidLock {
            max_len: usize::max_value(),
            size: is_query,
            size_used: false,
        }
    }

    /// Update the validator. Returns `Ok(true)` if everything is read out Ok, `Ok(false)` if we 
    /// don't recognize the field type or value, and `Err` if we recognize the field but fail to 
    /// parse the expected contents. The updated `raw` slice reference is only accurate if 
    /// `Ok(true)` was returned.
    pub fn update(&mut self, field: &str, raw: &mut &[u8]) -> crate::Result<bool> {
        let fail_len = raw.len();
        // Note about this match: because fields are lexicographically ordered, the items in this 
        // match statement are either executed sequentially or are skipped.
        match field {
            "max_len" => {
                self.size_used = true;
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.max_len = len as usize;
                    Ok(true)
                }
                else {
                    Err(Error::FailValidate(fail_len, "Lockbox validator expected non-negative value for `max_len` field"))
                }
            }
            "query" => {
                self.size = read_bool(raw)?;
                Ok(true)
            }
            "type" => if "Lock" == read_str(raw)? { Ok(true) } else { Err(Error::FailValidate(fail_len, "Type doesn't match Lock")) },
            _ => Err(Error::FailValidate(fail_len, "Unknown fields not allowed in Lockbox validator")),
        }
    }

    /// Final check on the validator. Returns true if at least one value can still pass the 
    /// validator.
    pub fn finalize(&mut self) -> bool {
        true
    }

    pub fn validate(&self, doc: &mut &[u8]) -> crate::Result<()> {
        let fail_len = doc.len();
        let value = read_lockbox(doc)?;
        if value.size() > self.max_len {
            Err(Error::FailValidate(fail_len, "Lockbox longer than max length allowed"))
        }
        else {
            Ok(())
        }
    }

    /// Verify the query is allowed to proceed. It can only proceed if the query type matches or is 
    /// a general Valid.
    pub fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::Lockbox(other) => self.size || !other.size_used,
            Validator::Valid => true,
            _ => false,
        }
    }

}
