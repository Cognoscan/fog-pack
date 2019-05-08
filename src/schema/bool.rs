use std::io;
use std::io::Error;
use std::io::ErrorKind::InvalidData;
use decode::*;
use super::Validator;

/// Boolean type validator
#[derive(Clone)]
pub struct ValidBool {
    constant: Option<bool>,
    query: bool,
}

impl ValidBool {
    pub fn new(is_query: bool) -> ValidBool {
        ValidBool {
            constant: None,
            query: is_query
        }
    }

    pub fn from_const(constant: bool, is_query: bool) -> ValidBool {
        let mut v = ValidBool::new(is_query);
        let constant = Some(constant);
        v.constant = constant;
        v
    }

    /// Update the validator. Returns `Ok(true)` if everything is read out Ok, `Ok(false)` if we 
    /// don't recognize the field type or value, and `Err` if we recognize the field but fail to 
    /// parse the expected contents. The updated `raw` slice reference is only accurate if 
    /// `Ok(true)` was returned.
    pub fn update(&mut self, field: &str, raw: &mut &[u8]) -> io::Result<bool> {
        match field {
            "type" => Ok("Bool" == read_str(raw)?),
            "in" => {
                self.constant = Some(read_bool(raw)?);
                Ok(true)
            },
            "nin" => {
                self.constant = Some(!(read_bool(raw)?));
                Ok(true)
            }
            "query" => {
                self.query = read_bool(raw)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub fn finalize(&mut self) -> bool {
        true
    }

    pub fn validate(&self, field: &str, doc: &mut &[u8]) -> io::Result<()> {
        let value = read_bool(doc)?;
        match self.constant {
            Some(b) => {
                if b == value {
                    Ok(())
                }
                else {
                    Err(Error::new(InvalidData, format!("Field \"{}\" isn't set to {}", field, b)))
                }
            },
            None => Ok(()),
        }
    }

    pub fn intersect(&self, other: Validator, query: bool) -> Result<Validator, ()> {
        if !self.query && query { return Err(()); }
        match other {
            Validator::Boolean(other) => {
                if let Some(o) = other.constant {
                    if let Some(s) = self.constant {
                        if s == o {
                            Ok(Validator::Boolean(ValidBool {
                                constant: self.constant,
                                query: self.query && other.query,
                            }))
                        }
                        else {
                            Ok(Validator::Invalid)
                        }
                    }
                    else {
                        Ok(Validator::Boolean(ValidBool {
                            constant: other.constant,
                            query: self.query && other.query,
                        }))
                    }
                }
                else {
                    Ok(Validator::Boolean(ValidBool {
                        constant: None,
                        query: self.query && other.query,
                    }))
                }
            },
            Validator::Valid => Ok(Validator::Boolean(self.clone())),
            _ => Ok(Validator::Invalid),
        }
    }
}

