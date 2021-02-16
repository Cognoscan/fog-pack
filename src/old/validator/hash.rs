use Error;
use decode::*;
use super::*;
use marker::MarkerType;
use crypto::Hash;

/// Hash type validator
#[derive(Clone, Debug)]
pub struct ValidHash {
    in_vec: Vec<Hash>,
    nin_vec: Vec<Hash>,
    link: Option<usize>,
    schema: Vec<Hash>,
    query: bool,
    link_ok: bool,
    schema_ok: bool,
    query_used: bool,
    schema_used: bool,
    link_used: bool,
}

impl ValidHash {

    pub fn new(is_query: bool) -> ValidHash {
        ValidHash {
            in_vec: Vec::with_capacity(0),
            nin_vec: Vec::with_capacity(0),
            link: None,
            schema: Vec::with_capacity(0),
            query: is_query,
            link_ok: is_query,
            schema_ok: is_query,
            query_used: false,
            schema_used: false,
            link_used: false,
        }
    }

    pub fn from_const(constant: Hash, is_query: bool) -> ValidHash {
        let mut v = ValidHash::new(is_query);
        let mut in_vec = Vec::with_capacity(1);
        in_vec.push(constant);
        v.in_vec = in_vec;
        v
    }

    /// Update the validator. Returns `Ok(true)` if everything is read out Ok, `Ok(false)` if we 
    /// don't recognize the field type or value, and `Err` if we recognize the field but fail to 
    /// parse the expected contents. The updated `raw` slice reference is only accurate if 
    /// `Ok(true)` was returned.
    pub fn update(&mut self, field: &str, raw: &mut &[u8], reader: &mut ValidReader) -> crate::Result<bool>
    {
        let fail_len = raw.len();
        let schema_hash = reader.schema_hash;
        // Note about this match: because fields are lexicographically ordered, the items in this 
        // match statement are either executed sequentially or are skipped.
        match field {
            "default" => {
                read_hash(raw)?;
                Ok(true)
            }
            "in" => {
                self.query_used = true;
                match read_marker(raw)? {
                    MarkerType::Hash(len) => {
                        let v = read_raw_hash(raw, len)?;
                        self.in_vec.reserve_exact(1);
                        if v.version() == 0 {
                            self.in_vec.push(schema_hash.clone());
                        }
                        else {
                            self.in_vec.push(v);
                        }
                    },
                    MarkerType::Array(len) => {
                        self.in_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _i in 0..len {
                            let v = read_hash(raw)?;
                            if v.version() == 0 {
                                self.in_vec.push(schema_hash.clone());
                            }
                            else {
                                self.in_vec.push(v);
                            }
                        };
                        self.in_vec.sort_unstable();
                        self.in_vec.dedup();
                    },
                    _ => {
                        return Err(Error::FailValidate(fail_len, "Hash validator expected array or constant for `in` field"));
                    },
                }
                Ok(true)
            },
            "link" => {
                self.link_used = true;
                self.link = Some(Validator::read_validator(raw, reader)?);
                Ok(true)
            }
            "link_ok" => {
                self.link_ok = read_bool(raw)?;
                Ok(true)
            }
            "nin" => {
                self.query_used = true;
                match read_marker(raw)? {
                    MarkerType::Hash(len) => {
                        let v = read_raw_hash(raw, len)?;
                        self.nin_vec.reserve_exact(1);
                        if v.version() == 0 {
                            self.nin_vec.push(schema_hash.clone());
                        }
                        else {
                            self.nin_vec.push(v);
                        }
                    },
                    MarkerType::Array(len) => {
                        self.nin_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _i in 0..len {
                            let v = read_hash(raw)?;
                            if v.version() == 0 {
                                self.nin_vec.push(schema_hash.clone());
                            }
                            else {
                                self.nin_vec.push(v);
                            }
                        };
                        self.nin_vec.sort_unstable();
                        self.nin_vec.dedup();
                    },
                    _ => {
                        return Err(Error::FailValidate(fail_len, "Hash validator expected array or constant for `nin` field"));
                    },
                }
                Ok(true)
            }
            "query" => {
                self.query = read_bool(raw)?;
                Ok(true)
            }
            "schema" => {
                self.schema_used = true;
                match read_marker(raw)? {
                    MarkerType::Hash(len) => {
                        let v = read_raw_hash(raw, len)?;
                        self.schema.reserve_exact(1);
                        if v.version() == 0 {
                            self.schema.push(schema_hash.clone());
                        }
                        else {
                            self.schema.push(v);
                        }
                    },
                    MarkerType::Array(len) => {
                        self.schema.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _i in 0..len {
                            let v = read_hash(raw)?;
                            if v.version() == 0 {
                                self.schema.push(schema_hash.clone());
                            }
                            else {
                                self.schema.push(v);
                            }
                        };
                        self.schema.sort_unstable();
                        self.schema.dedup();
                    },
                    _ => {
                        return Err(Error::FailValidate(fail_len, "Hash validator expected array or constant for `schema` field"));
                    },
                }
                Ok(true)
            }
            "schema_ok" => {
                self.schema_ok = read_bool(raw)?;
                Ok(true)
            }
            "type" => if "Hash" == read_str(raw)? { Ok(true) } else { Err(Error::FailValidate(fail_len, "Type doesn't match Hash")) },
            _ => Err(Error::FailValidate(fail_len, "Unknown fields not allowed in Hash validator")),
        }
    }

    /// Final check on the validator. Returns true if at least one value can (probably) still pass the 
    /// validator. We do not check to see if Hashes in `schema` field are for valid schema, or if 
    /// they intersect with the `link` field's validator.
    pub fn finalize(&mut self) -> bool {
        if !self.in_vec.is_empty() {
            let mut in_vec: Vec<Hash> = Vec::with_capacity(self.in_vec.len());
            for val in self.in_vec.iter() {
                if !self.nin_vec.contains(&val) && !in_vec.contains(&val) {
                    in_vec.push(val.clone());
                }
            }
            in_vec.shrink_to_fit();
            self.in_vec = in_vec;
            self.nin_vec = Vec::with_capacity(0);
            !self.in_vec.is_empty()
        }
        else {
            self.nin_vec.shrink_to_fit();
            true
        }
    }

    pub fn schema_in_set(&self, hash: &Hash) -> bool {
        self.schema.contains(hash)
    }

    pub fn schema_required(&self) -> bool {
        !self.schema.is_empty()
    }

    pub fn link(&self) -> Option<usize> {
        self.link
    }

    /// Validates that the next value is a Hash that meets the validator requirements. Fails if the 
    /// requirements are not met. If it passes, the optional returned Hash indicates that an 
    /// additional document (referenced by the Hash) needs to be checked.
    pub fn validate(&self, doc: &mut &[u8]) -> crate::Result<Option<Hash>> {
        let fail_len = doc.len();
        let value = read_hash(doc)?;
        if !self.in_vec.is_empty() && self.in_vec.binary_search(&value).is_err() {
            Err(Error::FailValidate(fail_len, "Hash is not on the `in` list"))
        }
        else if self.nin_vec.binary_search(&value).is_ok() {
            Err(Error::FailValidate(fail_len, "Hash is on the `nin` list"))
        }
        else if self.link.is_some() || !self.schema.is_empty() {
            Ok(Some(value))
        }
        else {
            Ok(None)
        }
    }

    /// Verify the query is allowed to proceed. It can only proceed if the query type matches or is 
    /// a general Valid.
    pub fn query_check(&self, other: &Validator, s_types: &[Validator], o_types: &[Validator]) -> bool {
        match other {
            Validator::Hash(other) => {
                if (self.query || !other.query_used)
                    && (self.schema_ok || !other.schema_used)
                    && (self.link_ok || !other.link_used)
                {
                    // Check to see if both have link validators, and run a check if so.
                    // If schema has none & link_ok is true, anything goes.
                    // If query has none, then we're also OK.
                    if let Some(s) = self.link {
                        if let Some(o) = other.link {
                            query_check(s, o, s_types, o_types)
                        }
                        else {
                            true
                        }
                    }
                    else {
                        true
                    }
                }
                else {
                    false
                }
            }
            Validator::Valid => true,
            _ => false,
        }
    }

}

