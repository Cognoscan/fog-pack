use Error;
use decode::*;
use super::{MAX_VEC_RESERVE, Validator};
use timestamp::Timestamp;
use marker::MarkerType;

/// Timestamp type validator
#[derive(Clone, Debug)]
pub struct ValidTime {
    in_vec: Vec<Timestamp>,
    nin_vec: Vec<Timestamp>,
    min: Timestamp,
    max: Timestamp,
    query: bool,
    ord: bool,
    ex_min: bool, // setup only
    ex_max: bool, // setup only
    query_used: bool,
    ord_used: bool,
}

impl ValidTime {
    pub fn new(is_query: bool) -> ValidTime {
        ValidTime {
            in_vec: Vec::with_capacity(0),
            nin_vec: Vec::with_capacity(0),
            min: Timestamp::min_value(),
            max: Timestamp::max_value(),
            query: is_query,
            ord: is_query,
            ex_min: false,
            ex_max: false,
            query_used: false,
            ord_used: false,
        }
    }

    pub fn from_const(constant: Timestamp, is_query: bool) -> ValidTime {
        let mut v = ValidTime::new(is_query);
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
                read_time(raw)?;
                Ok(true)
            }
            "ex_max" => {
                self.ex_max = read_bool(raw)?;
                self.max = self.max.prev();
                Ok(true)
            },
            "ex_min" => {
                self.ex_min = read_bool(raw)?;
                self.min = self.min.next();
                Ok(true)
            },
            "in" => {
                match read_marker(raw)? {
                    MarkerType::Timestamp(len) => {
                        let v = read_raw_time(raw, len)?;
                        self.in_vec.reserve_exact(1);
                        self.in_vec.push(v);
                    },
                    MarkerType::Array(len) => {
                        self.in_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _i in 0..len {
                            self.in_vec.push(read_time(raw)?);
                        };
                        self.in_vec.sort_unstable();
                        self.in_vec.dedup();
                    },
                    _ => {
                        return Err(Error::FailValidate(fail_len, "Timestamp validator expected array or constant for `in` field"));
                    },
                }
                Ok(true)
            },
            "max" => {
                let max = read_time(raw)?;
                if self.ex_max && max == Timestamp::min_value() {
                    Ok(false)
                }
                else {
                    self.max = if self.ex_max { max.prev() } else { max };
                    Ok(true)
                }
            }
            "min" => {
                let min = read_time(raw)?;
                if self.ex_min && min == Timestamp::max_value() {
                    Ok(false)
                }
                else {
                    self.min = if self.ex_min { min.next() } else { min };
                    // Valid only if min <= max. Only need to check after both min & max loaded in
                    Ok(self.min <= self.max)
                }
            }
            "nin" => {
                match read_marker(raw)? {
                    MarkerType::Timestamp(len) => {
                        let v = read_raw_time(raw, len)?;
                        self.nin_vec.reserve_exact(1);
                        self.nin_vec.push(v);
                    },
                    MarkerType::Array(len) => {
                        self.nin_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _i in 0..len {
                            self.nin_vec.push(read_time(raw)?);
                        };
                        self.nin_vec.sort_unstable();
                        self.nin_vec.dedup();
                    },
                    _ => {
                        return Err(Error::FailValidate(fail_len, "Timestamp validator expected array or constant for `nin` field"));
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
            "type" => if "Time" == read_str(raw)? { Ok(true) } else { Err(Error::FailValidate(fail_len, "Type doesn't match Time")) },
            _ => Err(Error::FailValidate(fail_len, "Unknown fields not allowed in Timestamp validator")),
        }
    }

    /// Final check on the validator. Returns true if at least one value can still pass the 
    /// validator.
    pub fn finalize(&mut self) -> bool {
        if self.in_vec.len() > 0 {
            let mut in_vec: Vec<Timestamp> = Vec::with_capacity(self.in_vec.len());
            let mut nin_index = 0;
            for val in self.in_vec.iter() {
                while let Some(nin) = self.nin_vec.get(nin_index) {
                    if nin < val { nin_index += 1; } else { break; }
                }
                if let Some(nin) = self.nin_vec.get(nin_index) {
                    if nin == val { continue; }
                }
                if (*val >= self.min) && (*val <= self.max) 
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
            // Only keep `nin` values that would otherwise pass
            self.nin_vec.retain(|val| {
                (*val >= min) && (*val <= max)
            });
            self.nin_vec.shrink_to_fit();
            true
        }
    }

    pub fn validate(&self, doc: &mut &[u8]) -> crate::Result<()> {
        let fail_len = doc.len();
        let value = read_time(doc)?;
        if (self.in_vec.len() > 0) && self.in_vec.binary_search(&value).is_err() {
            Err(Error::FailValidate(fail_len, "Time is not on the `in` list"))
        }
        else if self.in_vec.len() > 0 {
            Ok(())
        }
        else if value < self.min {
            Err(Error::FailValidate(fail_len, "Time is less than minimum allowed"))
        }
        else if value > self.max {
            Err(Error::FailValidate(fail_len, "Time is greater than maximum allowed"))
        }
        else if self.nin_vec.binary_search(&value).is_ok() {
            Err(Error::FailValidate(fail_len, "Time is on the `nin` list"))
        }
        else {
            Ok(())
        }

    }

    /// Verify the query is allowed to proceed. It can only proceed if the query type matches or is 
    /// a general Valid.
    pub fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::Timestamp(other) => {
                (self.query || !other.query_used) && (self.ord || !other.ord_used)
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
    use rand::prelude::*;

    fn read_it(raw: &mut &[u8], is_query: bool) -> crate::Result<ValidTime> {
        let fail_len = raw.len();
        if let MarkerType::Object(len) = read_marker(raw)? {
            let mut validator = ValidTime::new(is_query);
            object_iterate(raw, len, |field, raw| {
                let fail_len = raw.len();
                if !validator.update(field, raw)? {
                    Err(Error::FailValidate(fail_len, "Wasn't a valid timestamp validator"))
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


    fn rand_time<R: Rng>(rng: &mut R) -> Timestamp {
        let sec: i64 = rng.gen();
        let nano: u32 = rng.gen_range(0, 1_999_999_999);
        Timestamp::from_raw(sec, nano).unwrap()
    }

    fn rand_limited_time<R: Rng>(rng: &mut R) -> Timestamp {
        let sec: i64 = rng.gen_range(-5, 5);
        let nano: u32 = if rng.gen() { 0 } else { 1_999_999_999 };
        Timestamp::from_raw(sec, nano).unwrap()
    }

    #[test]
    fn generate() {
        let valid_count = 10;
        let test_count = 100;

        // Variables used in all tests
        let mut rng = rand::thread_rng();
        let mut test1 = Vec::new();
        let mut val = Vec::with_capacity(9);

        // Test passing any timestamp
        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Time"
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        for _ in 0..test_count {
            val.clear();
            encode::write_value(&mut val, &Value::from(rand_time(&mut rng)));
            validator.validate(&mut &val[..]).unwrap();
        }

        // Test timestamps in a range
        for _ in 0..valid_count {
            test1.clear();
            let val1 = rand_time(&mut rng);
            let val2 = rand_time(&mut rng);
            let (min, max) = if val1 < val2 { (val1, val2) } else { (val2, val1) };
            encode::write_value(&mut test1, &fogpack!({
                "min": min,
                "max": max
            }));
            let validator = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
            for _ in 0..test_count {
                val.clear();
                let test_val = rand_time(&mut rng);
                encode::write_value(&mut val, &Value::from(test_val.clone()));
                assert_eq!(
                    (test_val >= min) && (test_val <= max),
                    validator.validate(&mut &val[..]).is_ok(),
                    "{} was between {} and {} but failed validation", test_val, min, max);
            }
        }

        // Test timestamps in a narrow range
        for _ in 0..valid_count {
            test1.clear();
            let val1 = rand_limited_time(&mut rng);
            let val2 = rand_limited_time(&mut rng);
            let (min, max) = if val1 < val2 { (val1, val2) } else { (val2, val1) };
            encode::write_value(&mut test1, &fogpack!({
                "min": min,
                "max": max
            }));
            let validator = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
            for _ in 0..test_count {
                val.clear();
                let test_val = rand_limited_time(&mut rng);
                encode::write_value(&mut val, &Value::from(test_val.clone()));
                assert_eq!(
                    (test_val >= min) && (test_val <= max),
                    validator.validate(&mut &val[..]).is_ok(),
                    "{} was between {} and {} but failed validation", test_val, min, max);
            }
        }

        // Test timestamps with in/nin
        for _ in 0..valid_count {
            test1.clear();
            let mut in_vec: Vec<Timestamp> = Vec::with_capacity(valid_count);
            let mut nin_vec: Vec<Timestamp> = Vec::with_capacity(valid_count);
            for _ in 0..valid_count {
                in_vec.push(rand_limited_time(&mut rng));
                nin_vec.push(rand_limited_time(&mut rng));
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
                let test_val = rand_limited_time(&mut rng);
                encode::write_value(&mut val, &Value::from(test_val.clone()));
                assert_eq!(
                    in_vec.contains(&test_val) && !nin_vec.contains(&test_val),
                    validator.validate(&mut &val[..]).is_ok(),
                    "{} was in `in` and not `nin` but failed validation", test_val);
            }
        }
    }

}
