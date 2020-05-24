use std::f32;
use std::cmp::Ordering;

use byteorder::{ReadBytesExt, BigEndian};
use ieee754::Ieee754;

use Error;
use decode::*;
use super::{MAX_VEC_RESERVE, Validator};
use marker::MarkerType;

/// F32 type validator
#[derive(Clone,Debug)]
pub struct ValidF32 {
    in_vec: Vec<f32>,
    nin_vec: Vec<f32>,
    min: f32,
    max: f32,
    nan_ok: bool,
    query: bool,
    ord: bool,
    ex_min: bool, // setup only
    ex_max: bool, // setup only
    query_used: bool,
    ord_used: bool,
}

impl ValidF32 {
    pub fn new(is_query: bool) -> ValidF32 {
        ValidF32 {
            in_vec: Vec::with_capacity(0),
            nin_vec: Vec::with_capacity(0),
            min: f32::NEG_INFINITY,
            max: f32::INFINITY,
            nan_ok: true,
            query: is_query,
            ord: is_query,
            ex_min: false,
            ex_max: false,
            query_used: false,
            ord_used: false,
        }
    }

    pub fn from_const(constant: f32, is_query: bool) -> ValidF32 {
        let mut v = ValidF32::new(is_query);
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
                read_f32(raw)?;
                Ok(true)
            }
            "ex_max" => {
                self.ord_used = true;
                self.ex_max = read_bool(raw)?;
                self.max = self.max.prev();
                self.nan_ok = false;
                Ok(true)
            },
            "ex_min" => {
                self.ord_used = true;
                self.ex_min = read_bool(raw)?;
                self.min = self.min.next();
                self.nan_ok = false;
                Ok(true)
            },
            "in" => {
                self.query_used = true;
                match read_marker(raw)? {
                    MarkerType::F32 => {
                        let v = raw.read_f32::<BigEndian>()?;
                        self.in_vec.reserve_exact(1);
                        self.in_vec.push(v);
                    },
                    MarkerType::Array(len) => {
                        self.in_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _i in 0..len {
                            self.in_vec.push(read_f32(raw)?);
                        };
                        self.in_vec.sort_unstable_by(|a,b| a.total_cmp(b));
                        self.in_vec.dedup();
                    },
                    _ => {
                        return Err(Error::FailValidate(fail_len, "F32 validator expected array or constant for `in` field"));
                    },
                }
                Ok(true)
            },
            "max" => {
                self.ord_used = true;
                let max = read_f32(raw)?;
                if max.is_nan() {
                    Err(Error::FailValidate(fail_len, "F32 validator does not accept NaN for `max` field"))
                }
                else if self.ex_max && (max == f32::NEG_INFINITY) {
                    Ok(false)
                }
                else {
                    self.nan_ok = false;
                    self.max = if self.ex_max { max.prev() } else { max };
                    Ok(true)
                }
            }
            "min" => {
                self.ord_used = true;
                let min = read_f32(raw)?;
                if min.is_nan() {
                    Err(Error::FailValidate(fail_len, "F32 validator does not accept NaN for `min` field"))
                }
                else if self.ex_min && (min == f32::INFINITY) {
                    Ok(false)
                }
                else {
                    self.nan_ok = false;
                    self.min = if self.ex_min { min.next() } else { min };
                    Ok(self.min <= self.max)
                }
            }
            "nin" => {
                self.query_used = true;
                match read_marker(raw)? {
                    MarkerType::F32 => {
                        let v = raw.read_f32::<BigEndian>()?;
                        self.nin_vec.reserve_exact(1);
                        self.nin_vec.push(v);
                    },
                    MarkerType::Array(len) => {
                        self.nin_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _i in 0..len {
                            self.nin_vec.push(read_f32(raw)?);
                        };
                        self.nin_vec.sort_unstable_by(|a,b| a.total_cmp(b));
                        self.nin_vec.dedup();
                    },
                    _ => {
                        return Err(Error::FailValidate(fail_len, "F32 validator expected array or constant for `nin` field"));
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
            "type" => if "F32" == read_str(raw)? { Ok(true) } else { Err(Error::FailValidate(fail_len, "Type doesn't match F32")) },
            _ => Err(Error::FailValidate(fail_len, "Unknown fields not allowed in f32 validator")),
        }
    }

    /// Final check on the validator. Returns true if at least one value can still pass the 
    /// validator.
    pub fn finalize(&mut self) -> bool {
        if self.in_vec.len() > 0 {
            let mut in_vec: Vec<f32> = Vec::with_capacity(self.in_vec.len());
            let mut nin_index = 0;
            for val in self.in_vec.iter() {
                while let Some(nin) = self.nin_vec.get(nin_index) {
                    if nin.total_cmp(val) == Ordering::Less { nin_index += 1; } else { break; }
                }
                if let Some(nin) = self.nin_vec.get(nin_index) {
                    if nin.total_cmp(val) == Ordering::Equal { continue; }
                }
                if self.nan_ok {
                    in_vec.push(*val);
                }
                else if !val.is_nan() && (*val >= self.min) && (*val <= self.max) {
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
            let nan_ok = self.nan_ok;
            // Only keep `nin` values that would otherwise pass
            self.nin_vec.retain(|val| {
                nan_ok || (!val.is_nan() && (*val >= min) && (*val <= max))
            });
            self.nin_vec.shrink_to_fit();
            true
        }
    }

    pub fn validate(&self, doc: &mut &[u8]) -> crate::Result<()> {
        let fail_len = doc.len();
        let value = read_f32(doc)?;
        if (self.in_vec.len() > 0) && self.in_vec.binary_search_by(|probe| probe.total_cmp(&value)).is_err() {
            Err(Error::FailValidate(fail_len, "F32 is not on the `in` list"))
        }
        else if self.in_vec.len() > 0 {
            println!("Passed in_vec test with {}", value);
            Ok(())
        }
        else if value.is_nan() && !self.nan_ok
        {
            Err(Error::FailValidate(fail_len, "F32 is NaN and therefore out of range"))
        }
        else if !self.nan_ok && (value < self.min) {
            Err(Error::FailValidate(fail_len, "F32 is less than minimum allowed"))
        }
        else if !self.nan_ok && (value > self.max) {
            Err(Error::FailValidate(fail_len, "F32 is greater than maximum allowed"))
        }
        else if self.nin_vec.binary_search_by(|probe| probe.total_cmp(&value)).is_ok() {
            Err(Error::FailValidate(fail_len, "F32 is on the `nin` list"))
        }
        else {
            Ok(())
        }

    }

    /// Verify the query is allowed to proceed. It can only proceed if the query type matches or is 
    /// a general Valid.
    pub fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::F32(other) => {
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
    use std::f32;
    use rand::distributions::Uniform;

    fn read_it(raw: &mut &[u8], is_query: bool) -> crate::Result<ValidF32> {
        let fail_len = raw.len();
        if let MarkerType::Object(len) = read_marker(raw)? {
            let mut validator = ValidF32::new(is_query);
            object_iterate(raw, len, |field, raw| {
                let fail_len = raw.len();
                if !validator.update(field, raw)? {
                    Err(Error::FailValidate(fail_len, "Wasn't a valid F32 validator"))
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


    fn rand_float<R: Rng>(rng: &mut R) -> f32 {
        rng.gen()
    }

    #[test]
    fn generate() {
        let valid_count = 10;
        let test_count = 100;

        // Variables used in all tests
        let mut rng = rand::thread_rng();
        let mut test1 = Vec::new();
        let mut val = Vec::with_capacity(9);

        // Test passing any f32
        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "F32"
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        for _ in 0..test_count {
            val.clear();
            encode::write_value(&mut val, &Value::from(rand_float(&mut rng)));
            validator.validate(&mut &val[..]).unwrap();
        }
        encode::write_value(&mut val, &Value::from(f32::NAN));
        validator.validate(&mut &val[..]).unwrap();
        encode::write_value(&mut val, &Value::from(f32::INFINITY));
        validator.validate(&mut &val[..]).unwrap();
        encode::write_value(&mut val, &Value::from(f32::NEG_INFINITY));
        validator.validate(&mut &val[..]).unwrap();

        // Test floats in a range
        for _ in 0..valid_count {
            test1.clear();
            let val1 = rand_float(&mut rng);
            let val2 = rand_float(&mut rng);
            let (min, max) = if val1 < val2 { (val1, val2) } else { (val2, val1) };
            encode::write_value(&mut test1, &fogpack!({
                "min": min,
                "max": max
            }));
            let validator = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
            for _ in 0..test_count {
                val.clear();
                let test_val = rand_float(&mut rng);
                encode::write_value(&mut val, &Value::from(test_val.clone()));
                assert_eq!(
                    (test_val >= min) && (test_val <= max),
                    validator.validate(&mut &val[..]).is_ok(),
                    "{:e} was between {:e} and {:e} but failed validation", test_val, min, max);
            }
        }

        // Test -10 to 10 in a range
        let range = Uniform::new(-10i8, 10i8); 
        for _ in 0..valid_count {
            test1.clear();
            let val1 = rng.sample(range) as f32;
            let val2 = rng.sample(range) as f32;
            let (min, max) = if val1 < val2 { (val1, val2) } else { (val2, val1) };
            encode::write_value(&mut test1, &fogpack!({
                "min": min,
                "max": max
            }));
            let validator = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
            for _ in 0..test_count {
                val.clear();
                let test_val = rng.sample(range) as f32;
                encode::write_value(&mut val, &Value::from(test_val.clone()));
                assert_eq!(
                    (test_val >= min) && (test_val <= max),
                    validator.validate(&mut &val[..]).is_ok(),
                    "{} was between {} and {} but failed validation", test_val, min, max);
            }
            val.clear();
            let test_val = f32::NAN;
            encode::write_value(&mut val, &Value::from(test_val.clone()));
            assert!(validator.validate(&mut &val[..]).is_err(), "NAN passed a F32 validator with range");
        }

        // Test -10 to 10 with in/nin
        for _ in 0..valid_count {
            let range = Uniform::new(-10i8, 10i8); 
            test1.clear();
            let mut in_vec: Vec<f32> = Vec::with_capacity(valid_count);
            let mut nin_vec: Vec<f32> = Vec::with_capacity(valid_count);
            for _ in 0..valid_count {
                in_vec.push(rng.sample(range) as f32);
                nin_vec.push(rng.sample(range) as f32);
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
                let test_val = rng.sample(range) as f32;
                encode::write_value(&mut val, &Value::from(test_val.clone()));
                assert_eq!(
                    in_vec.contains(&test_val) && !nin_vec.contains(&test_val),
                    validator.validate(&mut &val[..]).is_ok(),
                    "{:e} was in `in` and not `nin` but failed validation", test_val);
            }
        }

        // Test in with NAN & infinities
        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "in": vec![Value::from(f32::NAN), Value::from(f32::INFINITY), Value::from(f32::NEG_INFINITY)]
        }));
        let validator = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
        val.clear();
        encode::write_value(&mut val, &Value::from(f32::NAN));
        assert!(validator.validate(&mut &val[..]).is_ok(), "NAN was in `in` but failed validation");
        val.clear();
        encode::write_value(&mut val, &Value::from(f32::INFINITY));
        assert!(validator.validate(&mut &val[..]).is_ok(), "INFINITY was in `in` but failed validation");
        val.clear();
        encode::write_value(&mut val, &Value::from(f32::NEG_INFINITY));
        assert!(validator.validate(&mut &val[..]).is_ok(), "NEG_INFINITY was in `in` but failed validation");
        val.clear();
        encode::write_value(&mut val, &Value::from(0f32));
        assert!(validator.validate(&mut &val[..]).is_err(), "0 was not in `in` but passed validation");

        // Test nin with NAN & infinities
        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "nin": vec![Value::from(f32::NAN), Value::from(f32::INFINITY), Value::from(f32::NEG_INFINITY)]
        }));
        let validator = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
        val.clear();
        encode::write_value(&mut val, &Value::from(f32::NAN));
        assert!(validator.validate(&mut &val[..]).is_err(), "NAN was in `nin` but passed validation");
        val.clear();
        encode::write_value(&mut val, &Value::from(f32::INFINITY));
        assert!(validator.validate(&mut &val[..]).is_err(), "INFINITY was in `nin` but passed validation");
        val.clear();
        encode::write_value(&mut val, &Value::from(f32::NEG_INFINITY));
        assert!(validator.validate(&mut &val[..]).is_err(), "NEG_INFINITY was in `nin` but passed validation");
        val.clear();
        encode::write_value(&mut val, &Value::from(0f32));
        assert!(validator.validate(&mut &val[..]).is_ok(), "0 was not in `nin` but failed validation");
    }

}
