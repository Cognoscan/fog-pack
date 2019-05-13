
use std::io;
use std::io::Error;
use std::io::ErrorKind::InvalidData;
use decode::*;
use super::{MAX_VEC_RESERVE, sorted_union, sorted_intersection, Validator};
use marker::MarkerType;

/// Binary type validator
#[derive(Clone)]
pub struct ValidBin {
    in_vec: Vec<Box<[u8]>>,
    nin_vec: Vec<Box<[u8]>>,
    min_len: usize,
    max_len: usize,
    bit_set: Vec<u8>,
    bit_clear: Vec<u8>,
    query: bool,
    ord: bool,
    bit: bool,
}

impl ValidBin {
    pub fn new(is_query: bool) -> ValidBin {
        ValidBin {
            in_vec: Vec::with_capacity(0),
            nin_vec: Vec::with_capacity(0),
            min_len: usize::min_value(),
            max_len: usize::max_value(),
            bit_set: Vec::with_capacity(0),
            bit_clear: Vec::with_capacity(0),
            query: is_query,
            ord: is_query,
            bit: is_query,
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
    pub fn update(&mut self, field: &str, raw: &mut &[u8]) -> io::Result<bool> {
        // Note about this match: because fields are lexicographically ordered, the items in this 
        // match statement are either executed sequentially or are skipped.
        match field {
            "bit" => {
                self.bit = read_bool(raw)?;
                Ok(true)
            },
            "bits_clr" => {
                self.bit_clear = read_vec(raw)?;
                Ok(true)
            },
            "bits_set" => {
                self.bit_set = read_vec(raw)?;
                Ok(self.bit_set.iter()
                   .zip(self.bit_clear.iter())
                   .all(|(set,clr)| (set & clr) == 0))
            },
            "default" => {
                read_vec(raw)?;
                Ok(true)
            }
            "in" => {
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
                        return Err(Error::new(InvalidData, "Binary validator expected array or constant for `in` field"));
                    },
                }
                Ok(true)
            },
            "max_len" => {
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.max_len = len as usize;
                    Ok(true)
                }
                else {
                    Ok(false)
                }
            }
            "min_len" => {
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.max_len = len as usize;
                    Ok(self.max_len >= self.min_len)
                }
                else {
                    Ok(false)
                }
            }
            "nin" => {
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
                        return Err(Error::new(InvalidData, "Binary validator expected array or constant for `nin` field"));
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
                    && self.bit_set.iter().zip(val.iter()).all(|(bit, val)| (bit & val) == *bit)
                    && self.bit_clear.iter().zip(val.iter()).all(|(bit, val)| (bit & val) == 0)
                {
                    in_vec.push(val.clone());
                }
            }
            in_vec.shrink_to_fit();
            self.in_vec = in_vec;
            self.nin_vec = Vec::with_capacity(0);
            self.in_vec.len() > 0
        }
        else {
            let min_len = self.min_len;
            let max_len = self.max_len;
            let bit_set = self.bit_set.clone();
            let bit_clear = self.bit_clear.clone();
            // Only keep `nin` values that would otherwise pass
            self.nin_vec.retain(|val| {
                (val.len() >= min_len) && (val.len() <= max_len) 
                    && bit_set.iter().zip(val.iter()).all(|(bit, val)| (bit & val) == *bit)
                    && bit_clear.iter().zip(val.iter()).all(|(bit, val)| (bit & val) == 0)
            });
            self.nin_vec.shrink_to_fit();
            true
        }
    }

    pub fn validate(&self, field: &str, doc: &mut &[u8]) -> io::Result<()> {
        let value = read_bin(doc)?;
        if (self.in_vec.len() > 0) && self.in_vec.binary_search_by(|probe| (**probe).cmp(value)).is_err() {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" contains binary not on the `in` list", field)))
        }
        else if self.in_vec.len() > 0 {
            Ok(())
        }
        else if value.len() < self.min_len {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" contains binary shorter than min length of {}", field, self.min_len)))
        }
        else if value.len() > self.max_len {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" contains binary longer than max length of {}", field, self.min_len)))
        }
        else if self.bit_set.iter().zip(value).any(|(bit, val)| (bit & val) != *bit) {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" does not have all required bits set", field)))
        }
        else if self.bit_clear.iter().zip(value).any(|(bit, val)| (bit & val) != 0) {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" does not have all required bits cleared", field)))
        }
        else if self.nin_vec.binary_search_by(|probe| (**probe).cmp(value)).is_ok() {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" contains binary on the `nin` list", field)))
        }
        else {
            Ok(())
        }
    }

    /// Intersection of Binary with other Validators. Returns Err only if `query` is true and the 
    /// other validator contains non-allowed query parameters.
    fn intersect(&self, other: &Validator, query: bool) -> Result<Validator, ()> {
        if query && !self.query && !self.ord && !self.bit { return Err(()); }
        match other {
            Validator::Binary(other) => {
                if query && (
                    (!self.query && (!other.in_vec.is_empty() || !other.nin_vec.is_empty()))
                    || (!self.ord && ((other.min_len > usize::min_value()) || (other.max_len < usize::max_value())))
                    || (!self.bit && ((other.bit_set.len() > 0) || (other.bit_clear.len() > 0))))
                {
                    Err(())
                }
                else if (self.min_len > other.max_len) || (self.max_len < other.min_len) 
                    || self.bit_set.iter().zip(other.bit_clear.iter()).any(|(a,b)| (a&b) != 0)
                    || self.bit_clear.iter().zip(other.bit_set.iter()).any(|(a,b)| (a&b) != 0)
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
                    let mut new_validator = ValidBin {
                        in_vec: in_vec,
                        nin_vec: sorted_union(&self.nin_vec[..], &other.nin_vec[..], |a,b| a.cmp(b)),
                        min_len: self.min_len.max(other.min_len),
                        max_len: self.max_len.min(other.max_len),
                        bit_set: self.bit_set.iter().zip(other.bit_set.iter()).map(|(a,b)| a | b).collect(),
                        bit_clear: self.bit_clear.iter().zip(other.bit_clear.iter()).map(|(a,b)| a | b).collect(),
                        query: self.query && other.query,
                        ord: self.ord && other.ord,
                        bit: self.bit && other.bit,
                    };
                    if new_validator.in_vec.len() == 0 && (self.in_vec.len()+other.in_vec.len() > 0) {
                        return Ok(Validator::Invalid);
                    }
                    let valid = new_validator.finalize();
                    if !valid {
                        Ok(Validator::Invalid)
                    }
                    else {
                        Ok(Validator::Binary(new_validator))
                    }
                }
            },
            Validator::Valid => Ok(Validator::Binary(self.clone())),
            _ => Ok(Validator::Invalid),
        }
    }
}

#[cfg(test)]
mod tests {
    use encode;
    use value::Value;
    use super::*;
    use rand::prelude::*;

    fn read_it(raw: &mut &[u8], is_query: bool) -> io::Result<ValidInt> {
        if let MarkerType::Object(len) = read_marker(raw)? {
            let mut validator = ValidInt::new(is_query);
            object_iterate(raw, len, |field, raw| {
                if !validator.update(field, raw)? {
                    Err(Error::new(InvalidData, "Wasn't a valid integer validator"))
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
        encode::write_value(&mut test1, &msgpack!({
            "type": "Int"
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        for _ in 0..test_count {
            val.clear();
            encode::write_value(&mut val, &Value::from(rand_integer(&mut rng)));
            validator.validate("", &mut &val[..]).unwrap();
        }

        // Test integers in a range
        for _ in 0..valid_count {
            test1.clear();
            let val1 = rand_integer(&mut rng);
            let val2 = rand_integer(&mut rng);
            let (min, max) = if val1 < val2 { (val1, val2) } else { (val2, val1) };
            encode::write_value(&mut test1, &msgpack!({
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
                    validator.validate("", &mut &val[..]).is_ok(),
                    "{} was between {} and {} but failed validation", test_val, min, max);
            }
        }

        // Test integers with bitset / bitclear
        for _ in 0..valid_count {
            test1.clear();
            let set: u64 = rng.gen();
            let clr: u64 = rng.gen::<u64>() & !set;
            encode::write_value(&mut test1, &msgpack!({
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
                    validator.validate("", &mut &val[..]).is_ok(),
                    "{:X} had {:X} set and {:X} clear but failed validation", test_val, set, clr);
            }
        }

        // Test i8 in a range
        for _ in 0..valid_count {
            test1.clear();
            let val1 = rand_i8(&mut rng);
            let val2 = rand_i8(&mut rng);
            let (min, max) = if val1 < val2 { (val1, val2) } else { (val2, val1) };
            encode::write_value(&mut test1, &msgpack!({
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
                    validator.validate("", &mut &val[..]).is_ok(),
                    "{} was between {} and {} but failed validation", test_val, min, max);
            }
        }

        // Test i8 with bitset / bitclear
        for _ in 0..valid_count {
            test1.clear();
            let set: u64 = rng.gen();
            let clr: u64 = rng.gen::<u64>() & !set;
            encode::write_value(&mut test1, &msgpack!({
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
                    validator.validate("", &mut &val[..]).is_ok(),
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
        encode::write_value(&mut test1, &msgpack!({
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
                validator.validate("", &mut &val[..]).is_ok(),
                "{:X} was in `in` and not `nin` but failed validation", test_val);
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

        // Test passing any integer
        // Test i8 in a range
        for _ in 0..valid_count {
            test1.clear();
            let val1 = rand_i8(&mut rng);
            let val2 = rand_i8(&mut rng);
            let (min, max) = if val1 < val2 { (val1, val2) } else { (val2, val1) };
            encode::write_value(&mut test1, &msgpack!({
                "min": min,
                "max": max
            }));
            let valid1 = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
            test1.clear();
            let val1 = rand_i8(&mut rng);
            let val2 = rand_i8(&mut rng);
            let (min, max) = if val1 < val2 { (val1, val2) } else { (val2, val1) };
            encode::write_value(&mut test1, &msgpack!({
                "min": min,
                "max": max
            }));
            let valid2 = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
            let validi = valid1.intersect(&Validator::Integer(valid2.clone()), false).unwrap();
            for _ in 0..test_count {
                val.clear();
                let test_val = rand_i8(&mut rng);
                encode::write_value(&mut val, &Value::from(test_val.clone()));
                assert_eq!(
                    valid1.validate("", &mut &val[..]).is_ok()
                    && valid2.validate("", &mut &val[..]).is_ok(),
                    validi.validate("", &mut &val[..]).is_ok(),
                    "Min/Max intersection for Integer validators fails");
            }
        }

        // Test i8 with bitset / bitclear
        for _ in 0..valid_count {
            test1.clear();
            let set: u64 = rng.gen();
            let clr: u64 = rng.gen::<u64>() & !set;
            encode::write_value(&mut test1, &msgpack!({
                "bits_set": set,
                "bits_clr": clr
            }));
            let valid1 = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
            test1.clear();
            let set: u64 = rng.gen();
            let clr: u64 = rng.gen::<u64>() & !set;
            encode::write_value(&mut test1, &msgpack!({
                "bits_set": set,
                "bits_clr": clr
            }));
            let valid2 = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
            let validi = valid1.intersect(&Validator::Integer(valid2.clone()), false).unwrap();
            for _ in 0..test_count {
                val.clear();
                let test_val = rand_i8(&mut rng);
                encode::write_value(&mut val, &Value::from(test_val.clone()));
                assert_eq!(
                    valid1.validate("", &mut &val[..]).is_ok()
                    && valid2.validate("", &mut &val[..]).is_ok(),
                    validi.validate("", &mut &val[..]).is_ok(),
                    "Bit set/clear intersection for Integer validators fails");
            }
        }

        // Test i8 with in/nin
        test1.clear();
        let mut in_vec: Vec<Value> = Vec::with_capacity(valid_count);
        let mut nin_vec: Vec<Value> = Vec::with_capacity(valid_count);
        for _ in 0..valid_count {
            in_vec.push(Value::from(rand_i8(&mut rng)));
            nin_vec.push(Value::from(rand_i8(&mut rng)));
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
            in_vec.push(Value::from(rand_i8(&mut rng)));
            nin_vec.push(Value::from(rand_i8(&mut rng)));
        }
        encode::write_value(&mut test1, &msgpack!({
            "in": in_vec,
            "nin": nin_vec,
        }));
        let valid2 = read_it(&mut &test1[..], false).expect(&format!("{:X?}",test1));
        let validi = valid1.intersect(&Validator::Integer(valid2.clone()), false).unwrap();
        println!("Valid1_in = {:?}", valid1.in_vec);
        println!("Valid1_nin = {:?}", valid1.nin_vec);
        println!("Valid2_in = {:?}", valid2.in_vec);
        println!("Valid2_nin = {:?}", valid2.nin_vec);
        if let Validator::Integer(ref v) = validi {
            println!("Validi_in = {:?}", v.in_vec);
            println!("Validi_nin = {:?}", v.nin_vec);
        }
        else {
            println!("Validi always false");
        }
        for _ in 0..(10*test_count) {
            val.clear();
            let test_val = rand_i8(&mut rng);
            encode::write_value(&mut val, &Value::from(test_val.clone()));
            assert_eq!(
                valid1.validate("", &mut &val[..]).is_ok()
                && valid2.validate("", &mut &val[..]).is_ok(),
                validi.validate("", &mut &val[..]).is_ok(),
                "Set intersection for Integer validators fails with {}", test_val);
        }
    }
}
