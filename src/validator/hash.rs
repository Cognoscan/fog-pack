use std::io;
use std::io::Error;
use std::io::ErrorKind::InvalidData;
use std::collections::HashMap;

use decode::*;
use super::{MAX_VEC_RESERVE, sorted_union, sorted_intersection, Validator, ValidBuilder};
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
    pub fn update(&mut self, field: &str, raw: &mut &[u8], is_query: bool, types: &mut Vec<Validator>, type_names: &mut HashMap<String,usize>, schema_hash: &Hash)
        -> io::Result<bool>
    {
        // Note about this match: because fields are lexicographically ordered, the items in this 
        // match statement are either executed sequentially or are skipped.
        match field {
            "default" => {
                read_hash(raw)?;
                Ok(true)
            }
            "in" => {
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
                        return Err(Error::new(InvalidData, "Hash validator expected array or constant for `in` field"));
                    },
                }
                Ok(true)
            },
            "link" => {
                self.link = Some(Validator::read_validator(raw, is_query, types, type_names, schema_hash)?);
                Ok(true)
            }
            "link_ok" => {
                self.link_ok = read_bool(raw)?;
                Ok(true)
            }
            "nin" => {
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
                        return Err(Error::new(InvalidData, "Hash validator expected array or constant for `nin` field"));
                    },
                }
                Ok(true)
            }
            "query" => {
                self.query = read_bool(raw)?;
                Ok(true)
            }
            "schema" => {
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
                        return Err(Error::new(InvalidData, "Hash validator expected array or constant for `schema` field"));
                    },
                }
                Ok(true)
            }
            "schema_ok" => {
                self.schema_ok = read_bool(raw)?;
                Ok(true)
            }
            "type" => if "Hash" == read_str(raw)? { Ok(true) } else { Err(Error::new(InvalidData, "Type doesn't match Hash")) },
            _ => Err(Error::new(InvalidData, "Unknown fields not allowed in Hash validator")),
        }
    }

    /// Final check on the validator. Returns true if at least one value can (probably) still pass the 
    /// validator. We do not check to see if Hashes in `schema` field are for valid schema, or if 
    /// they intersect with the `link` field's validator.
    pub fn finalize(&mut self) -> bool {
        if self.in_vec.len() > 0 {
            let mut in_vec: Vec<Hash> = Vec::with_capacity(self.in_vec.len());
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

    pub fn schema_in_set(&self, hash: &Hash) -> bool {
        self.schema.contains(hash)
    }

    pub fn schema_required(&self) -> bool {
        self.schema.len() > 0
    }

    pub fn link(&self) -> Option<usize> {
        self.link
    }

    /// Validates that the next value is a Hash that meets the validator requirements. Fails if the 
    /// requirements are not met. If it passes, the optional returned Hash indicates that an 
    /// additional document (referenced by the Hash) needs to be checked.
    pub fn validate(&self, field: &str, doc: &mut &[u8]) -> io::Result<Option<Hash>> {
        let value = read_hash(doc)?;
        if (self.in_vec.len() > 0) && self.in_vec.binary_search(&value).is_err() {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" contains Hash not on the `in` list", field)))
        }
        else if self.nin_vec.binary_search(&value).is_ok() {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" contains Hash on the `nin` list", field)))
        }
        else if let Some(_) = self.link {
            Ok(Some(value))
        }
        else if self.schema.len() > 0 {
            Ok(Some(value))
        }
        else {
            Ok(None)
        }
    }

    /// Intersection of Hash with other Validators. Returns Err only if `query` is true and the 
    /// other validator contains non-allowed query parameters.
    pub fn intersect(&self,
                 other: &Validator,
                 query: bool,
                 builder: &mut ValidBuilder
                 )
        -> Result<Validator, ()>
    {
        if query && !self.query && !self.link_ok && !self.schema_ok{ return Err(()); }
        match other {
            Validator::Hash(other) => {
                let builder_len = builder.len();
                if query && (
                    (!self.query && (!other.in_vec.is_empty() || !other.nin_vec.is_empty()))
                    || (!self.link_ok && other.link.is_some())
                    || (!self.schema_ok && (other.schema.len() > 0)))
                {
                    Err(())
                }
                else {
                    // Get instersection of `in` vectors
                    let in_vec = if (self.in_vec.len() > 0) && (other.in_vec.len() > 0) {
                        sorted_intersection(&self.in_vec[..], &other.in_vec[..], |a,b| a.cmp(b))
                    }
                    else if self.in_vec.len() > 0 {
                        self.in_vec.clone()
                    }
                    else {
                        other.in_vec.clone()
                    };
                    // Get instersection of schema
                    let schema = if (self.schema.len() > 0) && (other.schema.len() > 0) {
                        sorted_intersection(&self.schema[..], &other.schema[..], |a,b| a.cmp(b))
                    }
                    else if self.schema.len() > 0 {
                        self.schema.clone()
                    }
                    else {
                        other.schema.clone()
                    };
                    // Get link
                    let link = if let (Some(self_link), Some(other_link)) = (self.link,other.link) {
                        Some(builder.intersect(query, self_link, other_link)?)
                    }
                    else if let Some(link) = self.link {
                        Some(builder.intersect(query, link, 1)?)
                    }
                    else if let Some(link) = other.link {
                        Some(builder.intersect(query, 1, link)?)
                    }
                    else {
                        None
                    };
                    // Create new Validator
                    let mut new_validator = ValidHash {
                        in_vec: in_vec,
                        nin_vec: sorted_union(&self.nin_vec[..], &other.nin_vec[..], |a,b| a.cmp(b)),
                        schema: schema,
                        link: link,
                        query: self.query && other.query,
                        link_ok: self.link_ok && other.link_ok,
                        schema_ok: self.schema_ok && other.schema_ok,
                    };
                    if new_validator.in_vec.len() == 0 && (self.in_vec.len()+other.in_vec.len() > 0) {
                        builder.undo_to(builder_len);
                        return Ok(Validator::Invalid);
                    }
                    if new_validator.schema.len() == 0 && (self.schema.len()+other.schema.len() > 0) {
                        builder.undo_to(builder_len);
                        return Ok(Validator::Invalid);
                    }
                    let valid = new_validator.finalize();
                    if !valid {
                        builder.undo_to(builder_len);
                        Ok(Validator::Invalid)
                    }
                    else {
                        Ok(Validator::Hash(new_validator))
                    }
                }
            },
            Validator::Valid => {
                // Copy link
                let mut v = self.clone();
                if let Some(link) = self.link {
                    v.link = Some(builder.intersect(query, link, 1)?);
                }
                Ok(Validator::Hash(v))
            }
            _ => Ok(Validator::Invalid),
        }
    }
}

