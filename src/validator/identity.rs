use Error;
use decode::*;
use super::{MAX_VEC_RESERVE, Validator};
use marker::MarkerType;
use crypto::Identity;

/// Identity type validator
#[derive(Clone, Debug)]
pub struct ValidIdentity {
    in_vec: Vec<Identity>,
    nin_vec: Vec<Identity>,
    query: bool,
    query_used: bool,
}

impl ValidIdentity {
    pub fn new(is_query: bool) -> ValidIdentity {
        ValidIdentity {
            in_vec: Vec::with_capacity(0),
            nin_vec: Vec::with_capacity(0),
            query: is_query,
            query_used: false,
        }
    }

    pub fn from_const(constant: Identity, is_query: bool) -> ValidIdentity {
        let mut v = ValidIdentity::new(is_query);
        let mut in_vec = Vec::with_capacity(1);
        in_vec.push(constant);
        v.in_vec = in_vec;
        v
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
            "default" => {
                read_id(raw)?;
                Ok(true)
            }
            "in" => {
                self.query_used = true;
                match read_marker(raw)? {
                    MarkerType::Identity(len) => {
                        let v = read_raw_id(raw, len)?;
                        self.in_vec.reserve_exact(1);
                        self.in_vec.push(v);
                    },
                    MarkerType::Array(len) => {
                        self.in_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _i in 0..len {
                            self.in_vec.push(read_id(raw)?);
                        };
                    },
                    _ => {
                        return Err(Error::FailValidate(fail_len, "Identity validator expected array or constant for `in` field"));
                    },
                }
                Ok(true)
            },
            "nin" => {
                self.query_used = true;
                match read_marker(raw)? {
                    MarkerType::Identity(len) => {
                        let v = read_raw_id(raw, len)?;
                        self.nin_vec.reserve_exact(1);
                        self.nin_vec.push(v);
                    },
                    MarkerType::Array(len) => {
                        self.nin_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _i in 0..len {
                            self.nin_vec.push(read_id(raw)?);
                        };
                    },
                    _ => {
                        return Err(Error::FailValidate(fail_len, "Identity validator expected array or constant for `nin` field"));
                    },
                }
                Ok(true)
            }
            "query" => {
                self.query = read_bool(raw)?;
                Ok(true)
            }
            "type" => if "Ident" == read_str(raw)? { Ok(true) } else { Err(Error::FailValidate(fail_len, "Type doesn't match Ident")) },
            _ => Err(Error::FailValidate(fail_len, "Unknown fields not allowed in Identity validator")),
        }
    }

    /// Final check on the validator. Returns true if at least one value can still pass the 
    /// validator.
    pub fn finalize(&mut self) -> bool {
        if self.in_vec.len() > 0 {
            let mut in_vec: Vec<Identity> = Vec::with_capacity(self.in_vec.len());
            for val in self.in_vec.iter() {
                if !self.nin_vec.contains(&val) && !in_vec.contains(&val) {
                    in_vec.push(val.clone());
                }
            }
            in_vec.shrink_to_fit();
            self.in_vec = in_vec;
            self.nin_vec = Vec::with_capacity(0);
            self.in_vec.len() > 0
        }
        else {
            self.nin_vec.shrink_to_fit();
            true
        }
    }

    pub fn validate(&self, doc: &mut &[u8]) -> crate::Result<()> {
        let fail_len = doc.len();
        let value = read_id(doc)?;
        if self.nin_vec.contains(&value) {
            Err(Error::FailValidate(fail_len, "Identity is on the `nin` list"))
        }
        else if (self.in_vec.len() > 0) && !self.in_vec.contains(&value) {
            Err(Error::FailValidate(fail_len, "Identity is not on the `in` list"))
        }
        else {
            Ok(())
        }
    }

    /// Verify the query is allowed to proceed. It can only proceed if the query type matches or is 
    /// a general Valid.
    pub fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::Identity(other) => self.query || !other.query_used,
            Validator::Valid => true,
            _ => false,
        }
    }
}

