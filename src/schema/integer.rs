use std::io;
use std::io::Error;
use std::io::ErrorKind::InvalidData;
use decode::*;
use super::{sorted_union, sorted_intersection, Validator};
use integer::Integer;
use marker::MarkerType;

/// Integer type validator
#[derive(Clone)]
pub struct ValidInt {
    in_vec: Vec<Integer>,
    nin_vec: Vec<Integer>,
    min: Integer,
    max: Integer,
    bit_set: u64,
    bit_clear: u64,
    query: bool,
    ord: bool,
    bit: bool,
    ex_min: bool, // setup only
    ex_max: bool, // setup only
}

impl ValidInt {
    pub fn new(is_query: bool) -> ValidInt {
        ValidInt {
            in_vec: Vec::with_capacity(0),
            nin_vec: Vec::with_capacity(0),
            min: Integer::min_value(),
            max: Integer::max_value(),
            bit_set: 0,
            bit_clear: 0,
            query: is_query,
            ord: is_query,
            bit: is_query,
            ex_min: false,
            ex_max: false,
        }
    }

    pub fn from_const(constant: Integer, is_query: bool) -> ValidInt {
        let mut v = ValidInt::new(is_query);
        let mut in_vec = Vec::with_capacity(1);
        in_vec.push(constant);
        v.in_vec = in_vec;
        v
    }

    /// Update the validator. Returns `Ok(true)` if everything is read out Ok, `Ok(false)` if we 
    /// don't recognize the field type or value, and `Err` if we recognize the field but fail to 
    /// parse the expected contents. The updated `raw` slice reference is only accurate if 
    /// `Ok(true)` was returned.
    pub fn update(&mut self, field: &str, raw: &mut &[u8]) -> io::Result<bool> {
        // Note about this match: because fields are lexicographically ordered, the items in this 
        // match statement are either executed sequentially or are skipped.
        match field {
            "bit" => {
                self.bit = read_bool(raw)?;
                Ok(true)
            },
            "bits_clr" => {
                self.bit_clear = read_integer(raw)?.as_bits();
                Ok(true)
            },
            "bits_set" => {
                self.bit_set = read_integer(raw)?.as_bits();
                Ok((self.bit_set & self.bit_clear) == 0)
            },
            "default" => {
                read_integer(raw)?;
                Ok(true)
            }
            "ex_max" => {
                self.ex_max = read_bool(raw)?;
                self.max = self.max - 1;
                Ok(true)
            },
            "ex_min" => {
                self.ex_min = read_bool(raw)?;
                self.min = self.min + 1;
                Ok(true)
            },
            "in" => {
                match read_marker(raw)? {
                    MarkerType::PosInt((len, v)) => {
                        let v = read_pos_int(raw, len, v)?;
                        self.in_vec.reserve_exact(1);
                        self.in_vec.push(v);
                    },
                    MarkerType::NegInt((len, v)) => {
                        let v = read_neg_int(raw, len, v)?;
                        self.in_vec.reserve_exact(1);
                        self.in_vec.push(v);
                    },
                    MarkerType::Array(len) => {
                        self.in_vec.reserve_exact(len);
                        for _i in 0..len {
                            self.in_vec.push(read_integer(raw)?);
                        };
                        self.in_vec.sort_unstable();
                        self.in_vec.dedup();
                    },
                    _ => {
                        return Err(Error::new(InvalidData, "Integer validator expected array or constant for `in` field"));
                    },
                }
                Ok(true)
            },
            "max" => {
                let max = read_integer(raw)?;
                if self.ex_max && max == Integer::min_value() {
                    Ok(false)
                }
                else {
                    self.max = if self.ex_max { max - 1 } else { max };
                    Ok(true)
                }
            }
            "min" => {
                let min = read_integer(raw)?;
                if self.ex_min && min == Integer::max_value() {
                    Ok(false)
                }
                else {
                    self.min = if self.ex_min { min + 1 } else { min };
                    // Valid only if min <= max. Only need to check after both min & max loaded in
                    Ok(self.min <= self.max)
                }
            }
            "nin" => {
                match read_marker(raw)? {
                    MarkerType::PosInt((len, v)) => {
                        let v = read_pos_int(raw, len, v)?;
                        self.nin_vec.reserve_exact(1);
                        self.nin_vec.push(v);
                    },
                    MarkerType::NegInt((len, v)) => {
                        let v = read_neg_int(raw, len, v)?;
                        self.nin_vec.reserve_exact(1);
                        self.nin_vec.push(v);
                    },
                    MarkerType::Array(len) => {
                        self.nin_vec.reserve_exact(len);
                        for _i in 0..len {
                            self.nin_vec.push(read_integer(raw)?);
                        };
                        self.nin_vec.sort_unstable();
                        self.nin_vec.dedup();
                    },
                    _ => {
                        return Err(Error::new(InvalidData, "Integer validator expected array or constant for `nin` field"));
                    },
                }
                Ok(true)
            }
            "ord" => {
                self.ord = read_bool(raw)?;
                Ok(true)
            }
            "query" => {
                self.query = read_bool(raw)?;
                Ok(true)
            }
            "type" => Ok("Int" == read_str(raw)?),
            _ => Ok(false),
        }
    }

    /// Final check on the validator. Returns true if at least one value can still pass the 
    /// validator.
    pub fn finalize(&mut self) -> bool {
        if self.in_vec.len() > 0 {
            let mut in_vec: Vec<Integer> = Vec::with_capacity(self.in_vec.len());
            let mut nin_index = 0;
            for val in self.in_vec.iter() {
                while let Some(nin) = self.nin_vec.get(nin_index) {
                    if nin < val { nin_index += 1; } else { break; }
                }
                if let Some(nin) = self.nin_vec.get(nin_index) {
                    if nin == val { continue; }
                }
                if (*val >= self.min) && (*val <= self.max) 
                    && ((val.as_bits() & self.bit_set) == self.bit_set)
                    && ((val.as_bits() & self.bit_clear) == 0)
                {
                    in_vec.push(*val);
                }
            }
            in_vec.shrink_to_fit();
            self.in_vec = in_vec;
            self.nin_vec = Vec::with_capacity(0);
            self.in_vec.len() > 0
        }
        else {
            let min = self.min;
            let max = self.max;
            let bit_set = self.bit_set;
            let bit_clear = self.bit_clear;
            // Only keep `nin` values that would otherwise pass
            self.nin_vec.retain(|val| {
                (*val >= min) && (*val <= max)
                    && ((val.as_bits() & bit_set) == bit_set)
                    && ((val.as_bits() & bit_clear) == 0)
            });
            self.nin_vec.shrink_to_fit();
            true
        }
    }

    pub fn validate(&self, field: &str, doc: &mut &[u8]) -> io::Result<()> {
        let value = read_integer(doc)?;
        let value_raw = value.as_bits();
        if value < self.min {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" is {}, less than minimum of {}", field, value, self.min)))
        }
        else if value > self.max {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" is {}, greater than maximum of {}", field, value, self.max)))
        }
        else if self.nin_vec.binary_search(&value).is_ok() {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" is {}, which is on the `nin` list", field, value)))
        }
        else if (self.in_vec.len() > 0) && self.in_vec.binary_search(&value).is_err() {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" is {}, which is not in the `in` list", field, value)))
        }
        else if (self.bit_set & value_raw) != self.bit_set {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" is 0x{:X}, but must have set bits 0x{:X}", field, value_raw, self.bit_set)))
        }
        else if (self.bit_clear & value_raw) == 0 {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" is 0x{:X}, but must have cleared bits 0x{:X}", field, value_raw, self.bit_clear)))
        }
        else {
            Ok(())
        }

    }

    /// Intersection of Integer with other Validators. Returns Err only if `query` is true and the 
    /// other validator contains non-allowed query parameters.
    fn intersect(&self, other: &Validator, query: bool) -> Result<Validator, ()> {
        if query && !self.query && !self.ord && !self.bit { return Err(()); }
        match other {
            Validator::Integer(other) => {
                if query && (
                    (!self.query && (!other.in_vec.is_empty() || !other.nin_vec.is_empty()))
                    || (!self.ord && ((other.min > Integer::min_value()) || (other.max < Integer::max_value())))
                    || (!self.bit && ((other.bit_set > 0) || (other.bit_clear > 0))))
                {
                    Err(())
                }
                else if (self.min > other.max) || (self.max < other.min) 
                    || ((self.bit_set & other.bit_clear) != 0) || ((self.bit_clear & other.bit_set) != 0)
                {
                    Ok(Validator::Invalid)
                }
                else {
                    let mut new_validator = ValidInt {
                        in_vec: sorted_intersection(&self.in_vec[..], &other.in_vec[..], |a,b| a.cmp(b)),
                        nin_vec: sorted_union(&self.nin_vec[..], &other.nin_vec[..], |a,b| a.cmp(b)),
                        min: self.min.max(other.min),
                        max: self.max.min(other.max),
                        bit_set: self.bit_set | other.bit_set,
                        bit_clear: self.bit_clear | other.bit_clear,
                        query: self.query && other.query,
                        ord: self.ord && other.ord,
                        bit: self.bit && other.bit,
                        ex_min: false, // Doesn't get used by this point - for setup of validator only.
                        ex_max: false, // Doesn't get used by this point - for setup of validator only.
                    };
                    let valid = new_validator.finalize();
                    if !valid {
                        Ok(Validator::Invalid)
                    }
                    else {
                        Ok(Validator::Integer(new_validator))
                    }
                }
            },
            Validator::Valid => Ok(Validator::Integer(self.clone())),
            _ => Ok(Validator::Invalid),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    //use rand::prelude::*;

    fn read_it(raw: &mut &[u8]) -> io::Result<ValidInt> {
        Err(Error::new(InvalidData, "Not an object"))
    }


    #[test]
    fn generate() {
        let test1 = msgpack!({
            "type": "Int"
        });
    }
}
