use regex::Regex;

use Error;
use decode::*;
use super::{MAX_VEC_RESERVE, sorted_union, sorted_intersection, Validator};
use marker::MarkerType;

/// String type validator
#[derive(Clone, Debug)]
pub struct ValidStr {
    in_vec: Vec<String>,
    nin_vec: Vec<String>,
    min_char: usize,
    max_char: usize,
    use_char: bool,
    min_len: usize,
    max_len: usize,
    matches: Vec<Regex>,
    query: bool,
    size: bool,
    regex: bool,
}

impl ValidStr {
    pub fn new(is_query: bool) -> ValidStr {
        ValidStr {
            in_vec: Vec::with_capacity(0),
            nin_vec: Vec::with_capacity(0),
            min_char: usize::min_value(),
            max_char: usize::max_value(),
            use_char: false,
            min_len: usize::min_value(),
            max_len: usize::max_value(),
            matches: Vec::with_capacity(0),
            query: is_query,
            size: is_query,
            regex: is_query,
        }
    }

    pub fn from_const(constant: &str, is_query: bool) -> ValidStr {
        let mut v = ValidStr::new(is_query);
        let mut in_vec = Vec::with_capacity(1);
        in_vec.push(constant.to_string());
        v.in_vec = in_vec;
        v
    }

    /// Update the validator. Returns `Ok(true)` if everything is read out Ok, `Ok(false)` if we 
    /// don't recognize the field type or value, and `Err` if we recognize the field but fail to 
    /// parse the expected contents. The updated `raw` slice reference is only accurate if 
    /// `Ok(true)` was returned.
    pub fn update(&mut self, field: &str, raw: &mut &[u8]) -> crate::Result<bool> {
        // Note about this match: because fields are lexicographically ordered, the items in this 
        // match statement are either executed sequentially or are skipped.
        match field {
            "default" => {
                read_string(raw)?;
                Ok(true)
            }
            "in" => {
                match read_marker(raw)? {
                    MarkerType::String(len) => {
                        let v = read_raw_str(raw, len)?;
                        self.in_vec.reserve_exact(1);
                        self.in_vec.push(v.to_string());
                    },
                    MarkerType::Array(len) => {
                        self.in_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _i in 0..len {
                            self.in_vec.push(read_string(raw)?);
                        };
                        self.in_vec.sort_unstable();
                        self.in_vec.dedup();
                    },
                    _ => {
                        return Err(Error::FailValidate(raw.len(), "String validator expected array or constant for `in` field"));
                    },
                }
                Ok(true)
            },
            "matches" => {
                match read_marker(raw)? {
                    MarkerType::String(len) => {
                        let v = read_raw_str(raw, len)?;
                        match Regex::new(v) {
                            Ok(regex) => {
                                self.matches.reserve_exact(1);
                                self.matches.push(regex);
                                Ok(true)
                            },
                            Err(_) => {
                                // Failed regex creation is not an error, just means the validator 
                                // will always fail
                                Ok(false)
                            }
                        }
                    },
                    MarkerType::Array(len) => {
                        self.matches.reserve_exact(len.min(MAX_VEC_RESERVE));
                        let mut regexes_ok = true;
                        for _i in 0..len {
                            let s = read_str(raw)?;
                            match Regex::new(s) {
                                Ok(regex) => self.matches.push(regex),
                                Err(_) => {
                                    regexes_ok = false;
                                    break;
                                }
                            }
                        };
                        Ok(regexes_ok)
                    },
                    _ => {
                        Err(Error::FailValidate(raw.len(), "String validator expected array or string for `matches` field"))
                    },
                }
            },
            "max_char" => {
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.max_char = len as usize;
                    self.use_char = true;
                    Ok(true)
                }
                else {
                    Err(Error::FailValidate(raw.len(), "String validator requires non-negative integer for `max_char` field"))
                }
            }
            "max_len" => {
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.max_len = len as usize;
                    Ok(true)
                }
                else {
                    Err(Error::FailValidate(raw.len(), "String validator requires non-negative integer for `max_len` field"))
                }
            }
            "min_char" => {
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.min_char = len as usize;
                    self.use_char = true;
                    Ok(self.max_char >= self.min_char)
                }
                else {
                    Err(Error::FailValidate(raw.len(), "String validator requires non-negative integer for `min_char` field"))
                }
            }
            "min_len" => {
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.min_len = len as usize;
                    Ok(self.max_len >= self.min_len)
                }
                else {
                    Err(Error::FailValidate(raw.len(), "String validator requires non-negative integer for `min_len` field"))
                }
            }
            "nin" => {
                match read_marker(raw)? {
                    MarkerType::String(len) => {
                        let v = read_raw_str(raw, len)?;
                        self.nin_vec.reserve_exact(1);
                        self.nin_vec.push(v.to_string());
                    },
                    MarkerType::Array(len) => {
                        self.nin_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _i in 0..len {
                            self.nin_vec.push(read_string(raw)?);
                        };
                        self.nin_vec.sort_unstable();
                        self.nin_vec.dedup();
                    },
                    _ => {
                        return Err(Error::FailValidate(raw.len(), "String validator expected array or constant for `nin` field"));
                    },
                }
                Ok(true)
            },
            "query" => {
                self.query = read_bool(raw)?;
                Ok(true)
            },
            "regex" => {
                self.regex = read_bool(raw)?;
                Ok(true)
            },
            "size" => {
                self.size = read_bool(raw)?;
                Ok(true)
            },
            "type" => if "Str" == read_str(raw)? { Ok(true) } else { Err(Error::FailValidate(raw.len(), "Type doesn't match Str")) },
            _ => Err(Error::FailValidate(raw.len(), "Unknown fields not allowed in string validator")),
        }
    }

    /// Final check on the validator. Returns true if at least one value can still pass the 
    /// validator.
    pub fn finalize(&mut self) -> bool {
        if self.in_vec.len() > 0 {
            let mut in_vec: Vec<String> = Vec::with_capacity(self.in_vec.len());
            let mut nin_index = 0;
            for val in self.in_vec.iter() {
                while let Some(nin) = self.nin_vec.get(nin_index) {
                    if nin < val { nin_index += 1; } else { break; }
                }
                if let Some(nin) = self.nin_vec.get(nin_index) {
                    if nin == val { continue; }
                }
                let len_char = bytecount::num_chars(val.as_bytes());
                if self.use_char {
                    if (len_char > self.max_char) || (len_char < self.min_char) {
                        continue;
                    }
                }
                if (val.len() >= self.min_len) && (val.len() <= self.max_len) 
                    && self.matches.iter().all(|reg| reg.is_match(val))
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
            // Only keep `nin` values that would otherwise pass
            let mut nin_vec = self.nin_vec.clone();
            nin_vec.retain(|val| {
                let len_char = bytecount::num_chars(val.as_bytes());
                (val.len() >= min_len) && (val.len() <= max_len) 
                    && (len_char >= self.min_char) && (len_char <= self.max_char)
                    && self.matches.iter().all(|reg| reg.is_match(val))
            });
            nin_vec.shrink_to_fit();
            self.nin_vec = nin_vec;
            true
        }
    }

    pub fn validate(&self, doc: &mut &[u8]) -> crate::Result<()> {
        let value = read_str(doc)?;
        let len_char = if self.use_char { bytecount::num_chars(value.as_bytes()) } else { 0 };
        if (self.in_vec.len() > 0) && self.in_vec.binary_search_by(|probe| (**probe).cmp(value)).is_err() {
            Err(Error::FailValidate(doc.len(), "String is not on the `in` list"))
        }
        else if self.in_vec.len() > 0 {
            Ok(())
        }
        else if self.use_char && (len_char < self.min_char) {
            Err(Error::FailValidate(doc.len(), "String shorter than min char length"))
        }
        else if self.use_char && (len_char > self.max_char) {
            Err(Error::FailValidate(doc.len(), "String longer than max char length"))
        }
        else if value.len() < self.min_len {
            Err(Error::FailValidate(doc.len(), "String shorter than min length"))
        }
        else if value.len() > self.max_len {
            Err(Error::FailValidate(doc.len(), "String longer than max length"))
        }
        else if self.nin_vec.binary_search_by(|probe| (**probe).cmp(value)).is_ok() {
            Err(Error::FailValidate(doc.len(), "String is on the `nin` list"))
        }
        else if self.matches.iter().any(|reg| !reg.is_match(value)) {
            Err(Error::FailValidate(doc.len(), "String fails regex check"))
        }
        else {
            Ok(())
        }
    }

    /// Intersection of String with other Validators. Returns Err only if `query` is true and the 
    /// other validator contains non-allowed query parameters.
    pub fn intersect(&self, other: &Validator, query: bool) -> Result<Validator, ()> {
        if query && !self.query && !self.size && !self.regex { return Err(()); }
        match other {
            Validator::String(other) => {
                if query && (
                    (!self.query && (!other.in_vec.is_empty() || !other.nin_vec.is_empty()))
                    || (!self.size && (other.use_char || (other.min_len > usize::min_value()) || (other.max_len < usize::max_value())))
                    || (!self.regex && (other.matches.len() > 0)))
                {
                    Err(())
                }
                else if (self.min_len > other.max_len) || (self.max_len < other.min_len) 
                {
                    Ok(Validator::Invalid)
                }
                else if (self.min_char > other.max_char) || (self.max_char < other.min_char)
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
                    let mut matches = self.matches.clone();
                    matches.extend_from_slice(&other.matches);
                    let mut new_validator = ValidStr {
                        in_vec: in_vec,
                        nin_vec: sorted_union(&self.nin_vec[..], &other.nin_vec[..], |a,b| a.cmp(b)),
                        min_len: self.min_len.max(other.min_len),
                        max_len: self.max_len.min(other.max_len),
                        min_char: self.min_char.max(other.min_char),
                        max_char: self.max_char.min(other.max_char),
                        use_char: self.use_char || other.use_char,
                        matches: matches,
                        query: self.query && other.query,
                        size: self.size && other.size,
                        regex: self.regex && other.regex,
                    };
                    if new_validator.in_vec.len() == 0 && (self.in_vec.len()+other.in_vec.len() > 0) {
                        return Ok(Validator::Invalid);
                    }
                    let valid = new_validator.finalize();
                    if !valid {
                        Ok(Validator::Invalid)
                    }
                    else {
                        Ok(Validator::String(new_validator))
                    }
                }
            },
            Validator::Valid => Ok(Validator::String(self.clone())),
            _ => Ok(Validator::Invalid),
        }
    }
}

#[cfg(test)]
mod tests {
    use encode;
    use value::Value;
    use super::*;

    fn read_it(raw: &mut &[u8], is_query: bool) -> crate::Result<ValidStr> {
        if let MarkerType::Object(len) = read_marker(raw)? {
            let mut validator = ValidStr::new(is_query);
            object_iterate(raw, len, |field, raw| {
                if !validator.update(field, raw)? {
                    Err(Error::FailValidate(raw.len(), "Not a valid string validator"))
                }
                else {
                    Ok(())
                }
            })?;
            validator.finalize(); // Don't care about if the validator can pass values or not
            Ok(validator)

        }
        else {
            Err(Error::FailValidate(raw.len(), "Not an object"))
        }
    }

    fn validate_str(s: &str, validator: &ValidStr) -> crate::Result<()> {
        let mut val = Vec::with_capacity(1+s.len());
        encode::write_value(&mut val, &Value::from(s));
        validator.validate(&mut &val[..])
    }

    #[test]
    fn any_str() {

        let mut test1 = Vec::new();

        // Test passing any string data
        encode::write_value(&mut test1, &fogpack!({
            "type": "Str"
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_str("String", &validator).is_ok());
        assert!(validate_str("", &validator).is_ok());
        let mut val = Vec::with_capacity(1);
        encode::write_value(&mut val, &Value::from(0u8));
        assert!(validator.validate(&mut &val[..]).is_err());
        val.clear();
        encode::write_value(&mut val, &Value::from(false));
        assert!(validator.validate(&mut &val[..]).is_err());
    }

    #[test]
    fn range() {
        let mut test1 = Vec::new();

        // Test min/max length
        encode::write_value(&mut test1, &fogpack!({
            "min_len": 3,
            "max_len": 6
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_str("", &validator).is_err());
        assert!(validate_str("Te", &validator).is_err());
        assert!(validate_str("Tes", &validator).is_ok());
        assert!(validate_str("Test", &validator).is_ok());
        assert!(validate_str("TestSt", &validator).is_ok());
        assert!(validate_str("TestStr", &validator).is_err());
        assert!(validate_str("TestString", &validator).is_err());

        // Test min/max characters
        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "min_char": 3,
            "max_char": 6
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_str("", &validator).is_err());
        assert!(validate_str("Te", &validator).is_err());
        assert!(validate_str("Tes", &validator).is_ok());
        assert!(validate_str("Test", &validator).is_ok());
        assert!(validate_str("TestSt", &validator).is_ok());
        assert!(validate_str("TestStr", &validator).is_err());
        assert!(validate_str("TestString", &validator).is_err());
        assert!(validate_str("メカジキ", &validator).is_ok());
    }

    #[test]
    fn matches() {
        let mut test1 = Vec::new();

        // Test min/max length
        encode::write_value(&mut test1, &fogpack!({
            "matches": "test",
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_str("", &validator).is_err());
        assert!(validate_str("te", &validator).is_err());
        assert!(validate_str("tes", &validator).is_err());
        assert!(validate_str("test", &validator).is_ok());
        assert!(validate_str("testSt", &validator).is_ok());
        assert!(validate_str("testStr", &validator).is_ok());
        assert!(validate_str("testString", &validator).is_ok());
        assert!(validate_str("Other", &validator).is_err());
        assert!(validate_str("noTest", &validator).is_err());
        assert!(validate_str("notest", &validator).is_ok());
    }

    #[test]
    fn range_intersect() {
        let mut test1 = Vec::new();

        // Test min/max length
        encode::write_value(&mut test1, &fogpack!({
            "type": "Str",
            "min_len": 2,
            "max_len": 6
        }));
        let valid1 = read_it(&mut &test1[..], false).unwrap();
        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Str",
            "min_len": 3,
            "max_len": 10
        }));
        let valid2 = read_it(&mut &test1[..], false).unwrap();
        let validi = valid1.intersect(&Validator::String(valid2), false).unwrap();
        let validi = if let Validator::String(v) = validi {
            v
        }
        else {
            panic!("Intersection invalid");
        };
        assert!(validate_str("", &validi).is_err());
        assert!(validate_str("Te", &validi).is_err());
        assert!(validate_str("Tes", &validi).is_ok());
        assert!(validate_str("Test", &validi).is_ok());
        assert!(validate_str("TestSt", &validi).is_ok());
        assert!(validate_str("TestStr", &validi).is_err());
        assert!(validate_str("TestString", &validi).is_err());
    }

    #[test]
    fn regex_intersect() {
        let mut test1 = Vec::new();

        // Test min/max length
        encode::write_value(&mut test1, &fogpack!({
            "matches": "test",
        }));
        let valid1 = read_it(&mut &test1[..], false).unwrap();
        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "matches": "str",
        }));
        let valid2 = read_it(&mut &test1[..], false).unwrap();
        let validi = valid1.intersect(&Validator::String(valid2), false).unwrap();
        let validi = if let Validator::String(v) = validi {
            v
        }
        else {
            panic!("Intersection invalid");
        };
        assert!(validate_str("", &validi).is_err());
        assert!(validate_str("te", &validi).is_err());
        assert!(validate_str("tes", &validi).is_err());
        assert!(validate_str("test", &validi).is_err());
        assert!(validate_str("testst", &validi).is_err());
        assert!(validate_str("teststr", &validi).is_ok());
        assert!(validate_str("teststring", &validi).is_ok());
        assert!(validate_str("string", &validi).is_err());
    }

}
