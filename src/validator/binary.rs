use std::iter::repeat;

use Error;
use decode::*;
use super::{MAX_VEC_RESERVE, Validator};
use marker::MarkerType;

/// Binary type validator
#[derive(Clone, Debug)]
pub struct ValidBin {
    in_vec: Vec<Box<[u8]>>,
    nin_vec: Vec<Box<[u8]>>,
    min_len: usize,
    max_len: usize,
    always_fail: bool,
    min: Option<Box<[u8]>>,
    max: Option<Box<[u8]>>,
    bits_set: Vec<u8>,
    bits_clr: Vec<u8>,
    query: bool,
    ord: bool,
    size: bool,
    bit: bool,
    ex_min: bool, // setup only
    ex_max: bool, // setup only
    query_used: bool,
    ord_used: bool,
    size_used: bool,
    bit_used: bool,
}

impl ValidBin {
    pub fn new(is_query: bool) -> ValidBin {
        ValidBin {
            in_vec: Vec::with_capacity(0),
            nin_vec: Vec::with_capacity(0),
            min_len: usize::min_value(),
            max_len: usize::max_value(),
            always_fail: false,
            min: None,
            max: None,
            bits_set: Vec::with_capacity(0),
            bits_clr: Vec::with_capacity(0),
            query: is_query,
            ord: is_query,
            size: is_query,
            bit: is_query,
            ex_min: false,
            ex_max: false,
            query_used: false,
            ord_used: false,
            size_used: false,
            bit_used: false,
        }
    }

    pub fn from_const(constant: &[u8], is_query: bool) -> ValidBin {
        let mut v = ValidBin::new(is_query);
        let mut in_vec = Vec::with_capacity(1);
        in_vec.push(constant.to_vec().into_boxed_slice());
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
            "bit" => {
                self.bit = read_bool(raw)?;
                Ok(true)
            },
            "bits_clr" => {
                self.bit_used = true;
                self.bits_clr = read_vec(raw)?;
                Ok(true)
            },
            "bits_set" => {
                self.bit_used = true;
                self.bits_set = read_vec(raw)?;
                Ok(self.bits_set.iter()
                   .zip(self.bits_clr.iter())
                   .all(|(set,clr)| (set & clr) == 0))
            },
            "default" => {
                read_vec(raw)?;
                Ok(true)
            }
            "ex_max" => {
                self.ord_used = true;
                self.ex_max = read_bool(raw)?;
                Ok(true)
            },
            "ex_min" => {
                self.ord_used = true;
                self.ex_min = read_bool(raw)?;
                self.min = Some(vec![1u8].into_boxed_slice());
                Ok(true)
            },
            "in" => {
                self.query_used = true;
                match read_marker(raw)? {
                    MarkerType::Binary(len) => {
                        let v = read_raw_bin(raw, len)?;
                        self.in_vec.reserve_exact(1);
                        self.in_vec.push(v.to_vec().into_boxed_slice());
                    },
                    MarkerType::Array(len) => {
                        self.in_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _i in 0..len {
                            self.in_vec.push(read_vec(raw)?.into_boxed_slice());
                        };
                        self.in_vec.sort_unstable();
                        self.in_vec.dedup();
                    },
                    _ => {
                        return Err(Error::FailValidate(fail_len, "Binary validator expected array or constant for `in` field"));
                    },
                }
                Ok(true)
            },
            "max" => {
                self.ord_used = true;
                let mut max = read_vec(raw)?;
                let res = if self.ex_max {
                    let below_0 = max.iter_mut().fold(true, |acc, x| {
                        let (y, carry) = x.overflowing_sub(acc as u8);
                        *x = y;
                        carry
                    });
                    self.always_fail |= below_0;
                    Ok(!below_0)
                }
                else {
                    Ok(true)
                };
                let end = max.iter().enumerate().rev().find_map(|x| if x.1 > &0 { Some(x.0) } else { None });
                if let Some(end) = end {
                    max.truncate(end+1);
                }
                self.max = Some(max.into_boxed_slice());
                res
            }
            "max_len" => {
                self.size_used = true;
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.max_len = len as usize;
                    Ok(true)
                }
                else {
                    Err(Error::FailValidate(fail_len, "Binary validator requires non-negative integer for `max_len` field"))
                }
            }
            "min" => {
                self.ord_used = true;
                let mut min = read_vec(raw)?;
                if self.ex_min {
                    let carry = min.iter_mut().fold(true, |acc, x| {
                        let (y, carry) = x.overflowing_add(acc as u8);
                        *x = y;
                        carry
                    });
                    if carry {
                        min.push(1u8);
                    }
                }
                let end = min.iter().enumerate().rev().find_map(|x| if x.1 > &0 { Some(x.0) } else { None});
                if let Some(end) = end {
                    min.truncate(end+1);
                }
                self.min = Some(min.into_boxed_slice());
                Ok(true)
            }
            "min_len" => {
                self.size_used = true;
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.min_len = len as usize;
                    Ok(self.max_len >= self.min_len)
                }
                else {
                    Err(Error::FailValidate(fail_len, "Binary validator requires non-negative integer for `max_len` field"))
                }
            }
            "nin" => {
                self.query_used = true;
                match read_marker(raw)? {
                    MarkerType::Binary(len) => {
                        let v = read_raw_bin(raw, len)?;
                        self.nin_vec.reserve_exact(1);
                        self.nin_vec.push(v.to_vec().into_boxed_slice());
                    },
                    MarkerType::Array(len) => {
                        self.nin_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _i in 0..len {
                            self.nin_vec.push(read_vec(raw)?.into_boxed_slice());
                        };
                        self.nin_vec.sort_unstable();
                        self.nin_vec.dedup();
                    },
                    _ => {
                        return Err(Error::FailValidate(fail_len, "Binary validator expected array or constant for `nin` field"));
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
            "size" => {
                self.size = read_bool(raw)?;
                Ok(true)
            }
            "type" => if "Bin" == read_str(raw)? { Ok(true) } else { Err(Error::FailValidate(fail_len, "Type doesn't match Bin")) },
            _ => Err(Error::FailValidate(fail_len, "Unknown fields not allowed in binary validator")),
        }
    }

    /// Final check on the validator. Returns true if at least one value can still pass the 
    /// validator.
    pub fn finalize(&mut self) -> bool {
        if !self.in_vec.is_empty() {
            let mut in_vec: Vec<Box<[u8]>> = Vec::with_capacity(self.in_vec.len());
            let mut nin_index = 0;
            for val in self.in_vec.iter() {
                while let Some(nin) = self.nin_vec.get(nin_index) {
                    if nin < val { nin_index += 1; } else { break; }
                }
                if let Some(nin) = self.nin_vec.get(nin_index) {
                    if nin == val { continue; }
                }
                if (val.len() >= self.min_len) && (val.len() <= self.max_len) 
                    && self.bits_set.iter()
                        .zip(val.iter().chain(repeat(&0u8)))
                        .all(|(bit, val)| (bit & val) == *bit)
                    && self.bits_clr.iter()
                        .zip(val.iter().chain(repeat(&0u8)))
                        .all(|(bit, val)| (bit & val) == 0)
                {
                    in_vec.push(val.clone());
                }
            }
            in_vec.shrink_to_fit();
            self.in_vec = in_vec;
            self.nin_vec = Vec::with_capacity(0);
            !self.in_vec.is_empty()
        }
        else {
            let min_len = self.min_len;
            let max_len = self.max_len;
            let bits_set = &self.bits_set;
            let bits_clr = &self.bits_clr;
            let nin_vec = &mut self.nin_vec;
            // Only keep `nin` values that would otherwise pass
            nin_vec.retain(|val| {
                (val.len() >= min_len) && (val.len() <= max_len) 
                    && bits_set.iter()
                        .zip(val.iter().chain(repeat(&0u8)))
                        .all(|(bit, val)| (bit & val) == *bit)
                    && bits_clr.iter()
                        .zip(val.iter().chain(repeat(&0u8)))
                        .all(|(bit, val)| (bit & val) == 0)
            });
            nin_vec.shrink_to_fit();
            true
        }
    }

    pub fn validate(&self, doc: &mut &[u8]) -> crate::Result<()> {
        let fail_len = doc.len();
        let value = read_bin(doc)?;
        let above_max = self.max.as_ref().map_or(false, |v| {
            let max_len = v.len();
            if max_len > value.len() {
                false
            }
            else {
                // Returns true if max minus val carries, i.e. val > max
                value.iter()
                    .zip(v.iter().chain(repeat(&0u8)))
                    .fold(false, |carry, (val, max)| {
                        let (cmp, carry1) = max.overflowing_sub(*val);
                        let (_, carry2) = cmp.overflowing_sub(carry as u8);
                        carry1 | carry2
                    })
            }
        });
        let below_min = self.min.as_ref().map_or(false, |v| {
            let min_len = v.len();
            if min_len > value.len() {
                // Value literally can't contain the minimum allowed
                true
            }
            else {
                // Returns true if val minus min carries, i.e. min > val
                value.iter()
                    .zip(v.iter().chain(repeat(&0u8)))
                    .fold(false, |carry, (val, min)| {
                        let (cmp, carry1) = val.overflowing_sub(*min);
                        let (_, carry2) = cmp.overflowing_sub(carry as u8);
                        carry1 | carry2
                    })
            }
        });

        if self.always_fail {
            Err(Error::FailValidate(fail_len, "Binary validator always fails (min is 0 and ex_min is true)"))
        }
        else if !self.in_vec.is_empty() && self.in_vec.binary_search_by(|probe| (**probe).cmp(value)).is_err() {
            Err(Error::FailValidate(fail_len, "Binary is not on the `in` list"))
        }
        else if value.len() < self.min_len {
            Err(Error::FailValidate(fail_len, "Binary length is shorter than min allowed"))
        }
        else if value.len() > self.max_len {
            Err(Error::FailValidate(fail_len, "Binary length is longer than max allowed"))
        }
        else if above_max {
            Err(Error::FailValidate(fail_len, "Binary is greater than max value"))
        }
        else if below_min {
            Err(Error::FailValidate(fail_len, "Binary is less than min value"))
        }
        else if self.bits_set.iter()
            .zip(value.iter().chain(repeat(&0u8)))
            .any(|(bit, val)| (bit & val) != *bit)
        {
            Err(Error::FailValidate(fail_len, "Binary does not have all required bits set"))
        }
        else if self.bits_clr.iter()
            .zip(value.iter().chain(repeat(&0u8)))
            .any(|(bit, val)| (bit & val) != 0)
        {
            Err(Error::FailValidate(fail_len, "Binary does not have all required bits cleared"))
        }
        else if self.nin_vec.binary_search_by(|probe| (**probe).cmp(value)).is_ok() {
            Err(Error::FailValidate(fail_len, "Binary is on the `nin` list"))
        }
        else {
            Ok(())
        }
    }

    /// Verify the query is allowed to proceed. It can only proceed if the query type matches or is 
    /// a general Valid.
    pub fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::Binary(other) => {
                (self.query || !other.query_used)
                    && (self.ord || !other.query_used)
                    && (self.size || !other.size_used)
                    && (self.bit || !other.bit_used)
            }
            Validator::Valid => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use encode;
    use value::Value;
    use super::*;

    fn read_it(raw: &mut &[u8], is_query: bool) -> crate::Result<ValidBin> {
        let fail_len = raw.len();
        if let MarkerType::Object(len) = read_marker(raw)? {
            let mut validator = ValidBin::new(is_query);
            object_iterate(raw, len, |field, raw| {
                let fail_len = raw.len();
                if !validator.update(field, raw)? {
                    Err(Error::FailValidate(fail_len, "Not a valid binary validator"))
                }
                else {
                    Ok(())
                }
            })?;
            validator.finalize(); // Don't care about if the validator can pass values or not
            Ok(validator)

        }
        else {
            Err(Error::FailValidate(fail_len, "Not an object"))
        }
    }

    fn validate_bin(bin: Vec<u8>, validator: &ValidBin) -> crate::Result<()> {
        let mut val = Vec::with_capacity(3+bin.len());
        encode::write_value(&mut val, &Value::from(bin));
        validator.validate(&mut &val[..])
    }

    #[test]
    fn bad_validators() {
        // Exclusive max of 0 would be OK, but invalid
        let mut test1 = Vec::new();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bin",
            "ex_max": true,
            "max": vec![0]
        }));
        let validator = read_it(&mut &test1[..], false);
        assert!(validator.is_err());

        // If `in` field isn't an array of vec or a bin, should fail
        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bin",
            "in": true,
        }));
        let validator = read_it(&mut &test1[..], false);
        assert!(validator.is_err());

        // If `nin` field isn't an array of vec or a bin, should fail
        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bin",
            "nin": true,
        }));
        let validator = read_it(&mut &test1[..], false);
        assert!(validator.is_err());

        // If `max_len` is negative, should fail
        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bin",
            "max_len": -1,
        }));
        let validator = read_it(&mut &test1[..], false);
        assert!(validator.is_err());

        // If `min_len` is negative, should fail
        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bin",
            "min_len": -1,
        }));
        let validator = read_it(&mut &test1[..], false);
        assert!(validator.is_err());

        // If `ord` is not a bool, should fail
        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bin",
            "ord": 1,
        }));
        let validator = read_it(&mut &test1[..], false);
        assert!(validator.is_err());

        // If `query` is not a bool, should fail
        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bin",
            "query": 1,
        }));
        let validator = read_it(&mut &test1[..], false);
        assert!(validator.is_err());

        // If `size` is not a bool, should fail
        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bin",
            "size": 1,
        }));
        let validator = read_it(&mut &test1[..], false);
        assert!(validator.is_err());
    }

    #[test]
    fn any_bin() {

        let mut test1 = Vec::new();

        // Test passing any binary data
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bin",
            "default": vec![0x00],
            "ord": true,
            "query": true,
            "size": true,
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_bin(vec![0,1,2,3,4,5], &validator).is_ok());
        assert!(validate_bin(Vec::new(), &validator).is_ok());
        assert!(validate_bin(vec![0], &validator).is_ok());
        assert!(validate_bin(vec![0,0,0,0,0,0], &validator).is_ok());
        assert!(validate_bin(vec![255,255,255,255], &validator).is_ok());
        let mut val = Vec::with_capacity(1);
        encode::write_value(&mut val, &Value::from(0u8));
        assert!(validator.validate(&mut &val[..]).is_err());
        val.clear();
        encode::write_value(&mut val, &Value::from(false));
        assert!(validator.validate(&mut &val[..]).is_err());
    }

    #[test]
    fn len_range() {
        let mut test1 = Vec::new();

        // Test min/max length
        encode::write_value(&mut test1, &fogpack!({
            "min_len": 3,
            "max_len": 6
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_bin(vec![0,1,2], &validator).is_ok());
        assert!(validate_bin(vec![0,1,2,3,4,5], &validator).is_ok());
        assert!(validate_bin(Vec::new(), &validator).is_err());
        assert!(validate_bin(vec![0], &validator).is_err());
        assert!(validate_bin(vec![0,0,0,0,0,0,0], &validator).is_err());
    }

    #[test]
    fn val_range() {
        let mut test1 = Vec::new();

        encode::write_value(&mut test1, &fogpack!({
            "min": vec![0x01],
            "max": vec![0x02]
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_bin(vec![0x00], &validator).is_err());
        assert!(validate_bin(vec![0x01], &validator).is_ok());
        assert!(validate_bin(vec![0x02], &validator).is_ok());
        assert!(validate_bin(vec![0x03], &validator).is_err());
        assert!(validate_bin(vec![0x02, 0x00], &validator).is_ok());
        assert!(validate_bin(vec![0x00, 0x01], &validator).is_err());

        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "min": vec![0x01, 0x00, 0x00, 0x00],
            "max": vec![0x02, 0x00, 0x00, 0x00]
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        // Verify the min/max vectors have their trailing zeros stripped
        assert!(validator.min.as_ref().unwrap().len() == 1);
        assert!(validator.max.as_ref().unwrap().len() == 1);
        // Check against allowed ranges
        assert!(validate_bin(vec![0x00], &validator).is_err());
        assert!(validate_bin(vec![0x01], &validator).is_ok());
        assert!(validate_bin(vec![0x02], &validator).is_ok());
        assert!(validate_bin(vec![0x03], &validator).is_err());
        assert!(validate_bin(vec![0x02, 0x00], &validator).is_ok());
        assert!(validate_bin(vec![0x00, 0x01], &validator).is_err());

        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "ex_min": true,
            "ex_max": true,
            "min": vec![0x01, 0xFF, 0x03, 0x00],
            "max": vec![0xFF, 0xFE, 0x04, 0x00]
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_bin(vec![0x00], &validator).is_err());
        assert!(validate_bin(vec![0x00, 0x00, 0x00, 0x00], &validator).is_err());
        assert!(validate_bin(vec![0x00, 0x00, 0x00, 0x01], &validator).is_err());
        assert!(validate_bin(vec![0x01, 0xFF, 0x03, 0x00], &validator).is_err());
        assert!(validate_bin(vec![0x02, 0xFF, 0x03, 0x00], &validator).is_ok());
        assert!(validate_bin(vec![0xFE, 0xFE, 0x04, 0x00], &validator).is_ok());
        assert!(validate_bin(vec![0xFF, 0xFE, 0x04, 0x00], &validator).is_err());

        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "ex_min": true,
            "ex_max": true,
            "min": vec![0xFF, 0xFF],
            "max": vec![0x00, 0xFF, 0x01]
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_bin(vec![0xFF, 0xFF, 0x00], &validator).is_err());
        assert!(validate_bin(vec![0x00, 0x00, 0x01], &validator).is_ok());
        assert!(validate_bin(vec![0xFF, 0xFE, 0x01], &validator).is_ok());
        assert!(validate_bin(vec![0x00, 0xFF, 0x01], &validator).is_err());
    }

    #[test]
    fn bits() {
        let mut test1 = Vec::new();

        encode::write_value(&mut test1, &fogpack!({
            "bits_set": vec![0xAA, 0x0F, 0xF0],
            "bits_clr": vec![0x05, 0x30, 0x0C]
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_bin(vec![0xAA], &validator).is_err());
        assert!(validate_bin(vec![0xAA, 0x0F, 0xF0], &validator).is_ok());
        assert!(validate_bin(vec![0xAA, 0xCF, 0xF3], &validator).is_ok());
        assert!(validate_bin(vec![0xAA, 0xCF, 0xF3, 0xBE], &validator).is_ok());
        assert!(validate_bin(vec![0xAA, 0x3F, 0xFC], &validator).is_err());
        assert!(validate_bin(vec![0x5A, 0xC0, 0x33], &validator).is_err());
    }

    #[test]
    fn in_nin_sets() {
        let mut test1 = Vec::new();

        let in_vec: Vec<u8> = vec![0xAA, 0x0F, 0xF0];
        let nin_vec: Vec<u8> = vec![0x05, 0x30, 0x0C];
        encode::write_value(&mut test1, &fogpack!({
            "in": vec![Value::from(in_vec)],
            "nin": vec![Value::from(nin_vec)]
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_bin(vec![0xAA, 0x0F, 0xF0], &validator).is_ok());
        assert!(validate_bin(vec![0xAA, 0x0F], &validator).is_err());
        assert!(validate_bin(vec![0x05, 0x30, 0x0C], &validator).is_err());
        assert!(validate_bin(vec![0xAA, 0x0F, 0xF1], &validator).is_err());

        test1.clear();
        let in_vec: Vec<u8> = vec![0xAA, 0x0F, 0xF0];
        let nin_vec: Vec<u8> = vec![0x05, 0x30, 0x0C];
        encode::write_value(&mut test1, &fogpack!({
            "in": in_vec,
            "nin": nin_vec,
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_bin(vec![0xAA, 0x0F, 0xF0], &validator).is_ok());
        assert!(validate_bin(vec![0xAA, 0x0F], &validator).is_err());
        assert!(validate_bin(vec![0x05, 0x30, 0x0C], &validator).is_err());
        assert!(validate_bin(vec![0xAA, 0x0F, 0xF1], &validator).is_err());

        let nin_vec: Vec<u8> = vec![0x05, 0x30, 0x0C];
        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "nin": vec![Value::from(nin_vec)]
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_bin(vec![0xAA, 0x0F, 0xF0], &validator).is_ok());
        assert!(validate_bin(vec![0x05, 0x30], &validator).is_ok());
        assert!(validate_bin(vec![0x05, 0x30, 0x0C], &validator).is_err());
        assert!(validate_bin(vec![0x05, 0x30, 0x0C, 0x01], &validator).is_ok());
    }

}
