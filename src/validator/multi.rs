use super::*;

/// Container for multiple accepted Validators
#[derive(Clone, Debug)]
pub struct ValidMulti {
    any_of: Vec<Vec<usize>>,
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
    pub fn update(&mut self, field: &str, raw: &mut &[u8], is_query: bool, types: &mut Vec<Validator>, type_names: &mut HashMap<String,usize>)
        -> io::Result<bool>
    {
        // Note about this match: because fields are lexicographically ordered, the items in this 
        // match statement are either executed sequentially or are skipped.
        match field {
            "any_of" => {
                if let MarkerType::Array(len) = read_marker(raw)? {
                    self.any_of.push(Vec::new());
                    for _ in 0..len {
                        let v = Validator::read_validator(raw, is_query, types, type_names)?;
                        self.any_of[0].push(v);
                    }
                    Ok(true)
                }
                else {
                    Err(Error::new(InvalidData, "Multi `any_of` isn't a valid array of validators"))
                }
            }
            "type" => if "Multi" == read_str(raw)? { Ok(true) } else { Err(Error::new(InvalidData, "Type doesn't match Multi")) },
            _ => Err(Error::new(InvalidData, "Unknown fields not allowed in Multi validator")),
        }
    }

    /// Final check on the validator. Returns true if at least one value can (probably) still pass the 
    /// validator.
    pub fn finalize(&mut self) -> bool {
        if self.any_of.len() > 0 {
            self.any_of[0].sort_unstable();
            self.any_of[0].dedup();
            true
        }
        else {
            false
        }
    }

    pub fn validate(&self,
                    field: &str,
                    doc: &mut &[u8],
                    types: &Vec<Validator>,
                    list: &mut ValidatorChecklist,
                    ) -> io::Result<()>
    {
        if self.any_of.iter().all(|any_list| {
            any_list.iter().any(|v_index| {
                let mut temp_list = ValidatorChecklist::new();
                if let Err(_) = types[*v_index].validate(field, doc, types, *v_index, &mut temp_list) {
                    false
                }
                else {
                    list.merge(temp_list);
                    true
                }
            })
        })
        {
            Ok(())
        }
        else {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" failed against all allowed types.", field)))
        }
    }

    /// Intersection of Array with other Validators. Returns Err only if `query` is true and the 
    /// other validator contains non-allowed query parameters.
    pub fn intersect(&self,
                 other: &Validator,
                 query: bool,
                 builder: &mut ValidBuilder
                 )
        -> Result<Validator, ()>
    {
        match other {
            Validator::Invalid => Ok(Validator::Invalid),
            Validator::Valid => {
                let mut any_of = Vec::with_capacity(self.any_of.len());
                for list in self.any_of.iter() {
                    let mut new_list = Vec::with_capacity(list.len());
                    for item in list.iter() {
                        let v = builder.intersect(query, *item, VALID).unwrap();
                        new_list.push(v);
                    }
                    any_of.push(new_list);
                }
                Ok(Validator::Multi(ValidMulti { any_of }))
            },
            Validator::Multi(other) => {
                let mut any_of = Vec::with_capacity(self.any_of.len() + other.any_of.len());
                for list in self.any_of.iter() {
                    let mut new_list = Vec::with_capacity(list.len());
                    for item in list.iter() {
                        let v = builder.intersect(query, *item, VALID).unwrap();
                        new_list.push(v);
                    }
                    any_of.push(new_list);
                }
                for list in other.any_of.iter() {
                    let mut new_list = Vec::with_capacity(list.len());
                    for item in list.iter() {
                        let v = builder.intersect(query, VALID, *item).unwrap();
                        new_list.push(v);
                    }
                    any_of.push(new_list);
                }
                Ok(Validator::Multi(ValidMulti { any_of }))
            }
            _ => {
                let v_new = vec![builder.push(other.clone())];
                let mut v = self.clone();
                v.any_of.push(v_new);
                Ok(Validator::Multi(v))
            }
        }
    }
}

