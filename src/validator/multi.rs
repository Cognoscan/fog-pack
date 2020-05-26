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
        if self.any_of.len() > 0 {
            self.any_of.sort_unstable();
            self.any_of.dedup();
            true
        }
        else {
            false
        }
    }

    pub fn validate(&self,
                    doc: &mut &[u8],
                    types: &[Validator],
                    list: &mut ValidatorChecklist,
                    ) -> crate::Result<()>
    {
        let fail_len = doc.len();
        if self.any_of.iter().any(|v_index| {
                let mut temp_list = ValidatorChecklist::new();
                if let Err(_) = types[*v_index].validate(doc, types, *v_index, &mut temp_list) {
                    false
                }
                else {
                    list.merge(temp_list);
                    true
                }
        })
        {
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

