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
}

impl ValidIdentity {
    pub fn new(is_query: bool) -> ValidIdentity {
        ValidIdentity {
            in_vec: Vec::with_capacity(0),
            nin_vec: Vec::with_capacity(0),
            query: is_query,
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
        // Note about this match: because fields are lexicographically ordered, the items in this 
        // match statement are either executed sequentially or are skipped.
        match field {
            "default" => {
                read_id(raw)?;
                Ok(true)
            }
            "in" => {
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
                        return Err(Error::FailValidate(raw.len(), "Identity validator expected array or constant for `in` field"));
                    },
                }
                Ok(true)
            },
            "nin" => {
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
                        return Err(Error::FailValidate(raw.len(), "Identity validator expected array or constant for `nin` field"));
                    },
                }
                Ok(true)
            }
            "query" => {
                self.query = read_bool(raw)?;
                Ok(true)
            }
            "type" => if "Ident" == read_str(raw)? { Ok(true) } else { Err(Error::FailValidate(raw.len(), "Type doesn't match Ident")) },
            _ => Err(Error::FailValidate(raw.len(), "Unknown fields not allowed in Identity validator")),
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
        let value = read_id(doc)?;
        if self.nin_vec.contains(&value) {
            Err(Error::FailValidate(doc.len(), "Identity is on the `nin` list"))
        }
        else if (self.in_vec.len() > 0) && !self.in_vec.contains(&value) {
            Err(Error::FailValidate(doc.len(), "Identity is not on the `in` list"))
        }
        else {
            Ok(())
        }
    }

    /// Intersection of Identity with other Validators. Returns Err only if `query` is true and the 
    /// other validator contains non-allowed query parameters.
    pub fn intersect(&self, other: &Validator, query: bool) -> Result<Validator, ()> {
        if query && !self.query { return Err(()); }
        match other {
            Validator::Identity(other) => {
                if query && (
                    (!self.query && (!other.in_vec.is_empty() || !other.nin_vec.is_empty())))
                {
                    Err(())
                }
                else {
                    let in_vec = if (self.in_vec.len() > 0) && (other.in_vec.len() > 0) {
                        let mut vec: Vec<Identity> = Vec::new();
                        for val in other.in_vec.iter() {
                            if self.in_vec.contains(&val) {
                                vec.push(val.clone());
                            }
                        }
                        vec
                    }
                    else if self.in_vec.len() > 0 {
                        self.in_vec.clone()
                    }
                    else {
                        other.in_vec.clone()
                    };
                    let mut nin_vec = self.nin_vec.clone();
                    nin_vec.extend_from_slice(&other.nin_vec[..]);
                    let mut new_validator = ValidIdentity {
                        in_vec: in_vec,
                        nin_vec: nin_vec,
                        query: self.query && other.query,
                    };
                    if new_validator.in_vec.len() == 0 && (self.in_vec.len()+other.in_vec.len() > 0) {
                        return Ok(Validator::Invalid);
                    }
                    let valid = new_validator.finalize();
                    if !valid {
                        Ok(Validator::Invalid)
                    }
                    else {
                        Ok(Validator::Identity(new_validator))
                    }
                }
            },
            Validator::Valid => Ok(Validator::Identity(self.clone())),
            _ => Ok(Validator::Invalid),
        }
    }
}

