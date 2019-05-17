use std::io;
use std::io::Error;
use std::io::ErrorKind::InvalidData;

use decode::*;
use super::{MAX_VEC_RESERVE, sorted_union, sorted_intersection, Validator};
use timestamp::Timestamp;
use marker::MarkerType;

/// Timestamp type validator
#[derive(Clone)]
pub struct ValidTime {
    in_vec: Vec<Timestamp>,
    nin_vec: Vec<Timestamp>,
    min: Timestamp,
    max: Timestamp,
    query: bool,
    ord: bool,
    ex_min: bool, // setup only
    ex_max: bool, // setup only
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
    pub fn update(&mut self, field: &str, raw: &mut &[u8]) -> io::Result<bool> {
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
                        return Err(Error::new(InvalidData, "Timestamp validator expected array or constant for `in` field"));
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
                        return Err(Error::new(InvalidData, "Timestamp validator expected array or constant for `nin` field"));
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
            "type" => Ok("Time" == read_str(raw)?),
            _ => Err(Error::new(InvalidData, "Unknown fields not allowed in Timestamp validator")),
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

    pub fn validate(&self, field: &str, doc: &mut &[u8]) -> io::Result<()> {
        let value = read_time(doc)?;
        if (self.in_vec.len() > 0) && self.in_vec.binary_search(&value).is_err() {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" is {}, which is not in the `in` list", field, value)))
        }
        else if self.in_vec.len() > 0 {
            Ok(())
        }
        else if value < self.min {
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
        else {
            Ok(())
        }

    }

    /// Intersection of Timestamp with other Validators. Returns Err only if `query` is true and the 
    /// other validator contains non-allowed query parameters.
    pub fn intersect(&self, other: &Validator, query: bool) -> Result<Validator, ()> {
        if query && !self.query && !self.ord { return Err(()); }
        match other {
            Validator::Timestamp(other) => {
                if query && (
                    (!self.query && (!other.in_vec.is_empty() || !other.nin_vec.is_empty()))
                    || (!self.ord && ((other.min > Timestamp::min_value()) || (other.max < Timestamp::max_value()))))
                {
                    Err(())
                }
                else if (self.min > other.max) || (self.max < other.min) 
                {
                    Ok(Validator::Invalid)
                }
                else {
                    let in_vec = if (self.in_vec.len() > 0) && (other.in_vec.len() > 0) {
                        sorted_intersection(&self.in_vec[..], &other.in_vec[..], |a,b| a.cmp(b))
                    }
                    else if self.in_vec.len() > 0 {
                        self.in_vec.clone()
                    }
                    else {
                        other.in_vec.clone()
                    };
                    if in_vec.len() == 0 && (self.in_vec.len()+other.in_vec.len() > 0) {
                        return Ok(Validator::Invalid);
                    }
                    let mut new_validator = ValidTime {
                        in_vec: in_vec,
                        nin_vec: sorted_union(&self.nin_vec[..], &other.nin_vec[..], |a,b| a.cmp(b)),
                        min: self.min.max(other.min),
                        max: self.max.min(other.max),
                        query: self.query && other.query,
                        ord: self.ord && other.ord,
                        ex_min: false, // Doesn't get used by this point - for setup of validator only.
                        ex_max: false, // Doesn't get used by this point - for setup of validator only.
                    };
                    let valid = new_validator.finalize();
                    if !valid {
                        Ok(Validator::Invalid)
                    }
                    else {
                        Ok(Validator::Timestamp(new_validator))
                    }
                }
            },
            Validator::Valid => Ok(Validator::Timestamp(self.clone())),
            _ => Ok(Validator::Invalid),
        }
    }
}

#[cfg(test)]
mod tests {
    use encode;
    use value::Value;
    use super::super::Checklist;
    use super::*;
    use rand::prelude::*;

    fn read_it(raw: &mut &[u8], is_query: bool) -> io::Result<ValidTime> {
        if let MarkerType::Object(len) = read_marker(raw)? {
            let mut validator = ValidTime::new(is_query);
            object_iterate(raw, len, |field, raw| {
                if !validator.update(field, raw)? {
                    Err(Error::new(InvalidData, "Wasn't a valid timestamp validator"))
                }
                else {
                    Ok(())
                }
            })?;
            validator.finalize(); // Don't care about if the validator can pass values or not
            Ok(validator)

        }
        else {
            Err(Error::new(InvalidData, "Not an object"))
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
        encode::write_value(&mut test1, &msgpack!({
            "type": "Time"
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        for _ in 0..test_count {
            val.clear();
            encode::write_value(&mut val, &Value::from(rand_time(&mut rng)));
            validator.validate("", &mut &val[..]).unwrap();
        }

        // Test timestamps in a range
        for _ in 0..valid_count {
            test1.clear();
            let val1 = rand_time(&mut rng);
            let val2 = rand_time(&mut rng);
            let (min, max) = if val1 < val2 { (val1, val2) } else { (val2, val1) };
            encode::write_value(&mut test1, &msgpack!({
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
                    validator.validate("", &mut &val[..]).is_ok(),
                    "{} was between {} and {} but failed validation", test_val, min, max);
            }
        }

        // Test timestamps in a narrow range
        for _ in 0..valid_count {
            test1.clear();
            let val1 = rand_limited_time(&mut rng);
            let val2 = rand_limited_time(&mut rng);
            let (min, max) = if val1 < val2 { (val1, val2) } else { (val2, val1) };
            encode::write_value(&mut test1, &msgpack!({
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
                    validator.validate("", &mut &val[..]).is_ok(),
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
            encode::write_value(&mut test1, &msgpack!({
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
                    validator.validate("", &mut &val[..]).is_ok(),
                    "{} was in `in` and not `nin` but failed validation", test_val);
            }
        }
    }

    #[test]
    fn intersect() {
        let valid_count = 10;
        let test_count = 1000;

        // Variables used in all tests
        let mut rng = rand::thread_rng();
        let mut test1 = Vec::new();
        let mut val = Vec::with_capacity(9);

        // Test passing timestamp in a range
        for _ in 0..valid_count {
            test1.clear();
            let val1 = rand_limited_time(&mut rng);
            let val2 = rand_limited_time(&mut rng);
            let (min, max) = if val1 < val2 { (val1, val2) } else { (val2, val1) };
            encode::write_value(&mut test1, &msgpack!({
                "min": min,
                "max": max
            }));
            let valid1 = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
            test1.clear();
            let val1 = rand_limited_time(&mut rng);
            let val2 = rand_limited_time(&mut rng);
            let (min, max) = if val1 < val2 { (val1, val2) } else { (val2, val1) };
            encode::write_value(&mut test1, &msgpack!({
                "min": min,
                "max": max
            }));
            let valid2 = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
            let validi = valid1.intersect(&Validator::Timestamp(valid2.clone()), false).unwrap();
            for _ in 0..test_count {
                val.clear();
                let test_val = rand_limited_time(&mut rng);
                encode::write_value(&mut val, &Value::from(test_val.clone()));
                assert_eq!(
                    valid1.validate("", &mut &val[..]).is_ok()
                    && valid2.validate("", &mut &val[..]).is_ok(),
                    validi.validate("", &mut &val[..], &Vec::new(), 0, &mut Checklist::new()).is_ok(),
                    "Min/Max intersection for Timestamp validators fails");
            }
        }

        // Test passing timestamp with in/nin
        test1.clear();
        let mut in_vec: Vec<Value> = Vec::with_capacity(valid_count);
        let mut nin_vec: Vec<Value> = Vec::with_capacity(valid_count);
        for _ in 0..valid_count {
            in_vec.push(Value::from(rand_limited_time(&mut rng)));
            nin_vec.push(Value::from(rand_limited_time(&mut rng)));
        }
        encode::write_value(&mut test1, &msgpack!({
            "in": in_vec,
            "nin": nin_vec,
        }));
        let valid1 = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
        test1.clear();
        let mut in_vec: Vec<Value> = Vec::with_capacity(valid_count);
        let mut nin_vec: Vec<Value> = Vec::with_capacity(valid_count);
        for _ in 0..valid_count {
            in_vec.push(Value::from(rand_limited_time(&mut rng)));
            nin_vec.push(Value::from(rand_limited_time(&mut rng)));
        }
        encode::write_value(&mut test1, &msgpack!({
            "in": in_vec,
            "nin": nin_vec,
        }));
        let valid2 = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
        let validi = valid1.intersect(&Validator::Timestamp(valid2.clone()), false).unwrap();
        for _ in 0..(10*test_count) {
            val.clear();
            let test_val = rand_limited_time(&mut rng);
            encode::write_value(&mut val, &Value::from(test_val.clone()));
            assert_eq!(
                valid1.validate("", &mut &val[..]).is_ok()
                && valid2.validate("", &mut &val[..]).is_ok(),
                validi.validate("", &mut &val[..], &Vec::new(), 0, &mut Checklist::new()).is_ok(),
                "Set intersection for Timestamp validators fails with {}", test_val);
        }
    }
}
