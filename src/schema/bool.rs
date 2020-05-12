use std::io;
use std::io::{Error, ErrorKind::InvalidData};
use decode::*;
use super::Validator;

/// Boolean type validator
#[derive(Clone, Debug)]
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
            "default" => {
                read_bool(raw)?;
                Ok(true)
            },
            "in" => {
                self.constant = Some(read_bool(raw)?);
                Ok(true)
            },
            "nin" => {
                let constant = !(read_bool(raw)?);
                if let Some(prev) = self.constant {
                    if prev != constant {
                        Ok(false)
                    }
                    else {
                        Ok(true)
                    }
                }
                else {
                    self.constant = Some(constant);
                    Ok(true)
                }
            }
            "query" => {
                self.query = read_bool(raw)?;
                Ok(true)
            }
            "type" => if "Bool" == read_str(raw)? { Ok(true) } else { Err(Error::new(InvalidData, "Type doesn't match Bool")) },
            _ => Err(Error::new(InvalidData, "Unknown fields not allowed in boolean validator")),
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

    pub fn intersect(&self, other: &Validator, query: bool) -> Result<Validator, ()> {
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

#[cfg(test)]
mod tests {
    use marker::MarkerType;
    use encode;
    use value::Value;
    use super::*;

    fn read_it(raw: &mut &[u8], is_query: bool) -> io::Result<ValidBool> {
        if let MarkerType::Object(len) = read_marker(raw)? {
            let mut validator = ValidBool::new(is_query);
            object_iterate(raw, len, |field, raw| {
                if !validator.update(field, raw)? {
                    Err(Error::new(InvalidData, "Wasn't a valid bool validator"))
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

    fn validate_bool(v: bool, validator: &ValidBool) -> io::Result<()> {
        let mut val = Vec::with_capacity(1);
        encode::write_value(&mut val, &Value::from(v));
        validator.validate("", &mut &val[..])
    }

    #[test]
    fn validate() {
        let mut test1 = Vec::new();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bool",
            "query": true,
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_bool(false, &validator).is_ok());
        assert!(validate_bool(true,  &validator).is_ok());

        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bool",
            "in": true
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_bool(false, &validator).is_err());
        assert!(validate_bool(true,  &validator).is_ok());

        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bool",
            "in": false
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_bool(false, &validator).is_ok());
        assert!(validate_bool(true,  &validator).is_err());

        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bool",
            "nin": true
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_bool(false, &validator).is_ok());
        assert!(validate_bool(true,  &validator).is_err());

        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bool",
            "nin": false
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_bool(false, &validator).is_err());
        assert!(validate_bool(true,  &validator).is_ok());

        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bool",
            "in": false,
            "nin": true,
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_bool(false, &validator).is_ok());
        assert!(validate_bool(true,  &validator).is_err());

        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bool",
            "in": true,
            "nin": false,
        }));
        let validator = read_it(&mut &test1[..], false).unwrap();
        assert!(validate_bool(false, &validator).is_err());
        assert!(validate_bool(true,  &validator).is_ok());
    }

    #[test]
    fn bad_validators() {
        let mut test1 = Vec::new();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bool",
            "query": 0,
        }));
        assert!(read_it(&mut &test1[..], false).is_err());

        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bool",
            "in": 0,
        }));
        assert!(read_it(&mut &test1[..], false).is_err());

        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bool",
            "nin": 0,
        }));
        assert!(read_it(&mut &test1[..], false).is_err());

        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bool",
            "in": true,
            "nin": true,
        }));
        assert!(read_it(&mut &test1[..], false).is_err());

        test1.clear();
        encode::write_value(&mut test1, &fogpack!({
            "type": "Bool",
            "in": false,
            "nin": false,
        }));
        assert!(read_it(&mut &test1[..], false).is_err());
    }

}
