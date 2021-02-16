use unicode_normalization::UnicodeNormalization;
use regex;
use regex::Regex;

use Error;
use decode::*;
use super::{MAX_VEC_RESERVE, Validator};
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
    matches_work: bool,
    force_nfc: bool,
    force_nfkc: bool,
    query: bool,
    size: bool,
    regex: bool,
    query_used: bool,
    size_used: bool,
    regex_used: bool,
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
            matches_work: true,
            force_nfc: false,
            force_nfkc: false,
            query: is_query,
            size: is_query,
            regex: is_query,
            query_used: false,
            size_used: false,
            regex_used: false,
        }
    }

    pub fn from_const(constant: &str, is_query: bool) -> ValidStr {
        let mut v = ValidStr::new(is_query);
        let mut in_vec = Vec::with_capacity(1);
        in_vec.push(constant.to_string());
        v.in_vec = in_vec;
        v
    }

    fn conv_string(&self, s: &str) -> String {
        if self.force_nfkc {
            if unicode_normalization::is_nfkc(s) {
                s.to_string()
            }
            else {
                s.nfkc().collect::<String>()
            }
        }
        else if self.force_nfc {
            if unicode_normalization::is_nfc(s) {
                s.to_string()
            }
            else {
                s.nfc().collect::<String>()
            }
        }
        else {
            s.to_string()
        }
    }

    fn read_regex(&self, raw: &mut &[u8], len: usize) -> crate::Result<Option<Regex>> {
        let fail_len = raw.len();
        let temp_string: String; // Define here so it lives past the if-else block below
        let v = read_raw_str(raw, len)?;
        let v = if self.force_nfkc {
            if unicode_normalization::is_nfkc(v) {
                v
            }
            else {
                temp_string = v.nfkc().collect::<String>();
                temp_string.as_str()
            }
        }
        else if self.force_nfc {
            if unicode_normalization::is_nfc(v) {
                v
            }
            else {
                temp_string = v.nfc().collect::<String>();
                temp_string.as_str()
            }
        }
        else {
            v
        };
        let v = regex::RegexBuilder::new(v)
            .size_limit(1<<21)
            .dfa_size_limit(1<<20)
            .build();
        match v {
            Ok(regex) => {
                Ok(Some(regex))
            },
            Err(e) => {
                if let regex::Error::CompiledTooBig(_) = e {
                    Err(Error::ParseLimit(fail_len, "Regex hit size limit"))
                }
                else {
                    // Regex syntax failure is not an error; just means the validator 
                    // will always fail
                    Ok(None)
                }
            }
        }
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
                read_string(raw)?;
                Ok(true)
            }
            "force_nfc" => {
                self.force_nfc = read_bool(raw)?;
                Ok(true)
            },
            "force_nfkc" => {
                self.force_nfkc = read_bool(raw)?;
                Ok(true)
            },
            "in" => {
                self.query_used = true;
                match read_marker(raw)? {
                    MarkerType::String(len) => {
                        let v = read_raw_str(raw, len)?;
                        self.in_vec.reserve_exact(1);
                        self.in_vec.push(self.conv_string(v));
                    },
                    MarkerType::Array(len) => {
                        self.in_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _i in 0..len {
                            let v = read_str(raw)?;
                            self.in_vec.push(self.conv_string(v));
                        };
                        self.in_vec.sort_unstable();
                        self.in_vec.dedup();
                    },
                    _ => {
                        return Err(Error::FailValidate(fail_len, "String validator expected array or constant for `in` field"));
                    },
                }
                Ok(true)
            },
            "matches" => {
                self.regex_used = true;
                match read_marker(raw)? {
                    MarkerType::String(len) => {
                        let regex = self.read_regex(raw, len)?;
                        if let Some(regex) = regex {
                            self.matches.reserve_exact(1);
                            self.matches.push(regex);
                            Ok(true)
                        }
                        else {
                            Ok(false)
                        }
                    },
                    MarkerType::Array(len) => {
                        self.matches.reserve_exact(len.min(MAX_VEC_RESERVE));
                        self.matches_work = true;
                        for _i in 0..len {
                            let fail_len = raw.len();
                            let marker = read_marker(raw)?;
                            if let MarkerType::String(len) = marker {
                                let regex = self.read_regex(raw, len)?;
                                if let Some(regex) = regex {
                                    self.matches.push(regex);
                                }
                                else {
                                    self.matches_work = false;
                                }
                            }
                            else {
                                return Err(Error::FailValidate(fail_len, "Expected a string"))
                            }
                        };
                        Ok(self.matches_work)
                    },
                    _ => {
                        Err(Error::FailValidate(fail_len, "String validator expected array or string for `matches` field"))
                    },
                }
            },
            "max_char" => {
                self.size_used = true;
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.max_char = len as usize;
                    self.use_char = true;
                    Ok(true)
                }
                else {
                    Err(Error::FailValidate(fail_len, "String validator requires non-negative integer for `max_char` field"))
                }
            }
            "max_len" => {
                self.size_used = true;
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.max_len = len as usize;
                    Ok(true)
                }
                else {
                    Err(Error::FailValidate(fail_len, "String validator requires non-negative integer for `max_len` field"))
                }
            }
            "min_char" => {
                self.size_used = true;
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.min_char = len as usize;
                    self.use_char = true;
                    Ok(self.max_char >= self.min_char)
                }
                else {
                    Err(Error::FailValidate(fail_len, "String validator requires non-negative integer for `min_char` field"))
                }
            }
            "min_len" => {
                self.size_used = true;
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.min_len = len as usize;
                    Ok(self.max_len >= self.min_len)
                }
                else {
                    Err(Error::FailValidate(fail_len, "String validator requires non-negative integer for `min_len` field"))
                }
            }
            "nin" => {
                self.query_used = true;
                match read_marker(raw)? {
                    MarkerType::String(len) => {
                        let v = read_raw_str(raw, len)?;
                        self.nin_vec.reserve_exact(1);
                        self.nin_vec.push(self.conv_string(v));
                    },
                    MarkerType::Array(len) => {
                        self.nin_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _i in 0..len {
                            let v = read_str(raw)?;
                            self.nin_vec.push(self.conv_string(v));
                        };
                        self.nin_vec.sort_unstable();
                        self.nin_vec.dedup();
                    },
                    _ => {
                        return Err(Error::FailValidate(fail_len, "String validator expected array or constant for `nin` field"));
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
            "type" => if "Str" == read_str(raw)? { Ok(true) } else { Err(Error::FailValidate(fail_len, "Type doesn't match Str")) },
            _ => Err(Error::FailValidate(fail_len, "Unknown fields not allowed in string validator")),
        }
    }

    /// Final check on the validator. Returns true if at least one value can still pass the 
    /// validator.
    pub fn finalize(&mut self) -> bool {
        if !self.in_vec.is_empty() {
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
                if self.use_char && ((len_char > self.max_char) || (len_char < self.min_char)) {
                    continue;
                }
                if (val.len() >= self.min_len) && (val.len() <= self.max_len) 
                    && self.matches_work
                    && self.matches.iter().all(|reg| reg.is_match(val))
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
        let fail_len = doc.len();
        let temp_string: String; // Define here so it lives past the if-else block below
        let v = read_str(doc)?;
        let value = if self.force_nfkc {
            if unicode_normalization::is_nfkc(v) {
                v
            }
            else {
                temp_string = v.nfkc().collect::<String>();
                temp_string.as_str()
            }
        }
        else if self.force_nfc {
            if unicode_normalization::is_nfc(v) {
                v
            }
            else {
                temp_string = v.nfc().collect::<String>();
                temp_string.as_str()
            }
        }
        else {
            v
        };
        let len_char = if self.use_char { bytecount::num_chars(value.as_bytes()) } else { 0 };
        if !self.matches_work {
            Err(Error::FailValidate(fail_len, "Validator has regexes that don't compile"))
        }
        else if !self.in_vec.is_empty() && self.in_vec.binary_search_by(|probe| (**probe).cmp(value)).is_err() {
            Err(Error::FailValidate(fail_len, "String is not on the `in` list"))
        }
        else if self.use_char && (len_char < self.min_char) {
            Err(Error::FailValidate(fail_len, "String shorter than min char length"))
        }
        else if self.use_char && (len_char > self.max_char) {
            Err(Error::FailValidate(fail_len, "String longer than max char length"))
        }
        else if value.len() < self.min_len {
            Err(Error::FailValidate(fail_len, "String shorter than min length"))
        }
        else if value.len() > self.max_len {
            Err(Error::FailValidate(fail_len, "String longer than max length"))
        }
        else if self.nin_vec.binary_search_by(|probe| (**probe).cmp(value)).is_ok() {
            Err(Error::FailValidate(fail_len, "String is on the `nin` list"))
        }
        else if self.matches.iter().any(|reg| !reg.is_match(value)) {
            Err(Error::FailValidate(fail_len, "String fails regex check"))
        }
        else {
            Ok(())
        }
    }

    /// Verify the query is allowed to proceed. It can only proceed if the query type matches or is 
    /// a general Valid.
    pub fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::String(other) => {
                (self.query || !other.query_used)
                    && (self.size || !other.size_used)
                    && (self.regex || !other.regex_used)
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

    fn read_it(raw: &mut &[u8], is_query: bool) -> crate::Result<ValidStr> {
        let fail_len = raw.len();
        if let MarkerType::Object(len) = read_marker(raw)? {
            let mut validator = ValidStr::new(is_query);
            object_iterate(raw, len, |field, raw| {
                let fail_len = raw.len();
                if !validator.update(field, raw)? {
                    Err(Error::FailValidate(fail_len, "Not a valid string validator"))
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
    fn unicode_normalization() {
        let non_nfc = "ÅΩ";
        let nfc = "ÅΩ";
        let mut test1 = Vec::new();
        encode::write_value(&mut test1, &fogpack!({
            "force_nfc": true,
            "in": non_nfc,
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_str(non_nfc, &validator).is_ok());
        assert!(validate_str(nfc, &validator).is_ok());
        assert!(validate_str("AO", &validator).is_err());

        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "force_nfkc": true,
            "in": non_nfc,
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_str(non_nfc, &validator).is_ok());
        assert!(validate_str(nfc, &validator).is_ok());
        assert!(validate_str("AO", &validator).is_err());

        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "force_nfkc": true,
            "in": "⁵",
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_str("⁵", &validator).is_ok());
        assert!(validate_str("5", &validator).is_ok());
        assert!(validate_str("Five", &validator).is_err());
    }

}
