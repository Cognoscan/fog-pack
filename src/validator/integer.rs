use Error;
use decode::*;
use super::{MAX_VEC_RESERVE, Validator};
use integer::Integer;
use marker::MarkerType;

/// Integer type validator
#[derive(Clone, Debug)]
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
    query_used: bool,
    ord_used: bool,
    bit_used: bool,
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
            query_used: false,
            ord_used: false,
            bit_used: false,
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
                self.bit_clear = read_integer(raw)?.as_bits();
                Ok(true)
            },
            "bits_set" => {
                self.bit_used = true;
                self.bit_set = read_integer(raw)?.as_bits();
                Ok((self.bit_set & self.bit_clear) == 0)
            },
            "default" => {
                read_integer(raw)?;
                Ok(true)
            }
            "ex_max" => {
                self.ord_used = true;
                self.ex_max = read_bool(raw)?;
                self.max = self.max - 1;
                Ok(true)
            },
            "ex_min" => {
                self.ord_used = true;
                self.ex_min = read_bool(raw)?;
                self.min = self.min + 1;
                Ok(true)
            },
            "in" => {
                self.query_used = true;
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
                        self.in_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _i in 0..len {
                            self.in_vec.push(read_integer(raw)?);
                        };
                        self.in_vec.sort_unstable();
                        self.in_vec.dedup();
                    },
                    _ => {
                        return Err(Error::FailValidate(fail_len, "Integer validator expected array or constant for `in` field"));
                    },
                }
                Ok(true)
            },
            "max" => {
                self.ord_used = true;
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
                self.ord_used = true;
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
                self.query_used = true;
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
                        self.nin_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _i in 0..len {
                            self.nin_vec.push(read_integer(raw)?);
                        };
                        self.nin_vec.sort_unstable();
                        self.nin_vec.dedup();
                    },
                    _ => {
                        return Err(Error::FailValidate(fail_len, "Integer validator expected array or constant for `nin` field"));
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
            "type" => if "Int" == read_str(raw)? { Ok(true) } else { Err(Error::FailValidate(fail_len, "Type doesn't match Int")) },
            _ => Err(Error::FailValidate(fail_len, "Unknown fields not allowed in Integer validator")),
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

    pub fn validate(&self, doc: &mut &[u8]) -> crate::Result<()> {
        let fail_len = doc.len();
        let value = read_integer(doc)?;
        let value_raw = value.as_bits();
        if (self.in_vec.len() > 0) && self.in_vec.binary_search(&value).is_err() {
            Err(Error::FailValidate(fail_len, "Integer is not on the `in` list"))
        }
        else if self.in_vec.len() > 0 {
            Ok(())
        }
        else if value < self.min {
            Err(Error::FailValidate(fail_len, "Integer is less than minimum allowed"))
        }
        else if value > self.max {
            Err(Error::FailValidate(fail_len, "Integer is greater than maximum allowed"))
        }
        else if self.nin_vec.binary_search(&value).is_ok() {
            Err(Error::FailValidate(fail_len, "Integer is on the `nin` list"))
        }
        else if (self.bit_set & value_raw) != self.bit_set {
            Err(Error::FailValidate(fail_len, "Integer does not have all required bits set"))
        }
        else if (self.bit_clear & value_raw) != 0 {
            Err(Error::FailValidate(fail_len, "Integer does not have all required bits cleared"))
        }
        else {
            Ok(())
        }

    }

    /// Verify the query is allowed to proceed. It can only proceed if the query type matches or is 
    /// a general Valid.
    pub fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::Integer(other) => {
                (self.query || !other.query_used) && (self.ord || !other.ord_used) && (self.bit || !other.bit_used)
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
    use super::super::ValidatorChecklist;
    use rand::prelude::*;

    fn read_it(raw: &mut &[u8], is_query: bool) -> crate::Result<ValidInt> {
        let fail_len = raw.len();
        if let MarkerType::Object(len) = read_marker(raw)? {
            let mut validator = ValidInt::new(is_query);
            object_iterate(raw, len, |field, raw| {
                let fail_len = raw.len();
                if !validator.update(field, raw)? {
                    Err(Error::FailValidate(fail_len, "Wasn't a valid integer validator"))
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


    fn rand_integer<R: Rng>(rng: &mut R) -> Integer {
        if rng.gen() {
            let v: i64 = rng.gen();
            Integer::from(v)
        }
        else {
            let v: u64 = rng.gen();
            Integer::from(v)
        }
    }

    fn rand_i8<R: Rng>(rng: &mut R) -> Integer {
        let v: i8 = rng.gen();
        Integer::from(v)
    }

    #[test]
    fn generate() {
        let valid_count = 10;
        let test_count = 100;

        // Variables used in all tests
        let mut rng = rand::thread_rng();
        let mut test1 = Vec::new();
        let mut val = Vec::with_capacity(9);

        // Test passing any integer
        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Int"
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        for _ in 0..test_count {
            val.clear();
            encode::write_value(&mut val, &Value::from(rand_integer(&mut rng)));
            validator.validate(&mut &val[..]).unwrap();
        }

        // Test integers in a range
        for _ in 0..valid_count {
            test1.clear();
            let val1 = rand_integer(&mut rng);
            let val2 = rand_integer(&mut rng);
            let (min, max) = if val1 < val2 { (val1, val2) } else { (val2, val1) };
            encode::write_value(&mut test1, &fogpack!({
                "min": min,
                "max": max
            }));
            let validator = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
            for _ in 0..test_count {
                val.clear();
                let test_val = rand_integer(&mut rng);
                encode::write_value(&mut val, &Value::from(test_val.clone()));
                assert_eq!(
                    (test_val >= min) && (test_val <= max),
                    validator.validate(&mut &val[..]).is_ok(),
                    "{} was between {} and {} but failed validation", test_val, min, max);
            }
        }

        // Test integers with bitset / bitclear
        for _ in 0..valid_count {
            test1.clear();
            let set: u64 = rng.gen();
            let clr: u64 = rng.gen::<u64>() & !set;
            encode::write_value(&mut test1, &fogpack!({
                "bits_set": set,
                "bits_clr": clr
            }));
            let validator = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
            for _ in 0..test_count {
                val.clear();
                let test_val = rand_integer(&mut rng);
                encode::write_value(&mut val, &Value::from(test_val.clone()));
                let test_val = test_val.as_bits();
                assert_eq!(
                    ((test_val & set) == set) && ((test_val & clr) == 0),
                    validator.validate(&mut &val[..]).is_ok(),
                    "{:X} had {:X} set and {:X} clear but failed validation", test_val, set, clr);
            }
        }

        // Test i8 in a range
        for _ in 0..valid_count {
            test1.clear();
            let val1 = rand_i8(&mut rng);
            let val2 = rand_i8(&mut rng);
            let (min, max) = if val1 < val2 { (val1, val2) } else { (val2, val1) };
            encode::write_value(&mut test1, &fogpack!({
                "min": min,
                "max": max
            }));
            let validator = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
            for _ in 0..test_count {
                val.clear();
                let test_val = rand_i8(&mut rng);
                encode::write_value(&mut val, &Value::from(test_val.clone()));
                assert_eq!(
                    (test_val >= min) && (test_val <= max),
                    validator.validate(&mut &val[..]).is_ok(),
                    "{} was between {} and {} but failed validation", test_val, min, max);
            }
        }

        // Test i8 with bitset / bitclear
        for _ in 0..valid_count {
            test1.clear();
            let set: u64 = rng.gen();
            let clr: u64 = rng.gen::<u64>() & !set;
            encode::write_value(&mut test1, &fogpack!({
                "bits_set": set,
                "bits_clr": clr
            }));
            let validator = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
            for _ in 0..test_count {
                val.clear();
                let test_val = rand_i8(&mut rng);
                encode::write_value(&mut val, &Value::from(test_val.clone()));
                let test_val = test_val.as_bits();
                assert_eq!(
                    ((test_val & set) == set) && ((test_val & clr) == 0),
                    validator.validate(&mut &val[..]).is_ok(),
                    "{:X} had {:X} set and {:X} clear but failed validation", test_val, set, clr);
            }
        }

        // Test i8 with in/nin
        test1.clear();
        let mut in_vec: Vec<Integer> = Vec::with_capacity(valid_count);
        let mut nin_vec: Vec<Integer> = Vec::with_capacity(valid_count);
        for _ in 0..valid_count {
            in_vec.push(rand_i8(&mut rng));
            nin_vec.push(rand_i8(&mut rng));
        }
        let in_vec_val: Vec<Value> = in_vec.iter().map(|&x| Value::from(x)).collect();
        let nin_vec_val: Vec<Value> = nin_vec.iter().map(|&x| Value::from(x)).collect();
        encode::write_value(&mut test1, &fogpack!({
            "in": in_vec_val,
            "nin": nin_vec_val,
        }));
        let validator = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
        for _ in 0..test_count {
            val.clear();
            let test_val = rand_i8(&mut rng);
            encode::write_value(&mut val, &Value::from(test_val.clone()));
            assert_eq!(
                in_vec.contains(&test_val) && !nin_vec.contains(&test_val),
                validator.validate(&mut &val[..]).is_ok(),
                "{:X} was in `in` and not `nin` but failed validation", test_val);
        }
    }

}
