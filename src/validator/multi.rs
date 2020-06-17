use super::*;
use Error;

/// Container for multiple accepted Validators
#[derive(Clone, Debug)]
pub struct ValidMulti {
    any_of: Vec<usize>,
}

impl ValidMulti {
    // When implementing this, figure out how to handle mergine `link` field in ValidHash too
    pub fn new(_is_query: bool) -> ValidMulti {
        ValidMulti {
            any_of: Vec::with_capacity(0)
        }
    }

    /// Update the validator. Returns `Ok(true)` if everything is read out Ok, `Ok(false)` if we 
    /// don't recognize the field type or value, and `Err` if we recognize the field but fail to 
    /// parse the expected contents. The updated `raw` slice reference is only accurate if 
    /// `Ok(true)` was returned.
    pub fn update(&mut self, field: &str, raw: &mut &[u8], reader: &mut ValidReader) -> crate::Result<bool>
    {
        let fail_len = raw.len();
        // Note about this match: because fields are lexicographically ordered, the items in this 
        // match statement are either executed sequentially or are skipped.
        match field {
            "any_of" => {
                if let MarkerType::Array(len) = read_marker(raw)? {
                    for _ in 0..len {
                        let v = Validator::read_validator(raw, reader)?;
                        self.any_of.push(v);
                    }
                    Ok(true)
                }
                else {
                    Err(Error::FailValidate(fail_len, "Multi `any_of` isn't a valid array of validators"))
                }
            }
            "type" => if "Multi" == read_str(raw)? { Ok(true) } else { Err(Error::FailValidate(fail_len, "Type doesn't match Multi")) },
            _ => Err(Error::FailValidate(fail_len, "Unknown fields not allowed in Multi validator")),
        }
    }

    /// Final check on the validator. Returns true if at least one value can (probably) still pass the 
    /// validator.
    pub fn finalize(&mut self) -> bool {
        !self.any_of.is_empty()
    }

    pub fn validate(&self,
                    doc: &mut &[u8],
                    types: &[Validator],
                    list: &mut ValidatorChecklist,
                    ) -> crate::Result<()>
    {
        let fail_len = doc.len();
        let any_of_pass = self.any_of.iter().any(|v_index| {
                let mut temp_list = ValidatorChecklist::new();
                let mut doc_local = &doc[..];
                if types[*v_index].validate(&mut doc_local, types, *v_index, &mut temp_list).is_err() {
                    false
                }
                else {
                    *doc = doc_local;
                    list.merge(temp_list);
                    true
                }
        });
        if any_of_pass {
            Ok(())
        }
        else {
            Err(Error::FailValidate(fail_len, "Failed against all of Multi's any_of validators"))
        }
    }

    pub fn query_check(&self, other: usize, s_types: &[Validator], o_types: &[Validator]) -> bool {
        self.any_of.iter().any(|s| {
            query_check(*s, other, s_types, o_types)
        })
    }

    pub fn iter(&self) -> std::slice::Iter<usize> {
        self.any_of.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use encode;
    use Value;

    fn read_it(raw: &mut &[u8], is_query: bool) -> (usize, Vec<Validator>) {
        let mut types = Vec::new();
        types.push(Validator::Invalid);
        types.push(Validator::Valid);
        let mut type_names = HashMap::new();
        let schema_hash = Hash::new(b"test");
        let mut reader = ValidReader::new(is_query, &mut types, &mut type_names, &schema_hash);
        let validator = Validator::read_validator(&mut &raw[..], &mut reader).unwrap();
        for (i, v) in types.iter().enumerate() {
            println!("{}: {:?}", i, v);
        }
        match types[validator] {
            Validator::Multi(_) => (),
            _ => panic!("Parsing a multi validator didn't yield a multi validator!"),
        }
        (validator, types)
    }

    #[test]
    fn multi_obj() {
        let mut raw_schema = Vec::new();
        let schema: Value = fogpack!({
            "type": "Multi",
            "comment": "Describes a recommended compression format for a doc/entry",
            "any_of": [
                { "type": "Obj", "req": { "setting": false } },
                { 
                    "type": "Obj",
                    "req": {
                        "format": { "type": "Int", "min": 0, "max": 31 },
                        "setting": { "type": "Multi", "any_of": [
                            { "type": "Bin" },
                            true
                        ] },
                    },
                    "opt": {
                        "level": { "type": "Int", "min": 0, "max": 255 }
                    }
                }
            ]
        });
        encode::write_value(&mut raw_schema, &schema);
        println!("Schema = {}", &schema);
        //let (validator, types) = read_it(&mut &raw_schema[..], false);
    }


}




