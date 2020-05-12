use super::*;

#[derive(Clone,Debug)]
pub enum Validator {
    Invalid,
    Valid,
    Null,
    Type(String),
    Boolean(ValidBool),
    Integer(ValidInt),
    String(ValidStr),
    F32(ValidF32),
    F64(ValidF64),
    Binary(ValidBin),
    Array(ValidArray),
    Object(ValidObj),
    Hash(ValidHash),
    Identity(ValidIdentity),
    Lockbox(ValidLock),
    Timestamp(ValidTime),
    Multi(ValidMulti),
}

impl Validator {
    pub fn read_validator(raw: &mut &[u8], is_query: bool, types: &mut Vec<Validator>, type_names: &mut HashMap<String, usize>)
        -> io::Result<usize>
    {
        let validator = match read_marker(raw)? {
            MarkerType::Null => Validator::Valid,
            MarkerType::Boolean(v) => {
                Validator::Boolean(ValidBool::from_const(v, is_query))
            },
            MarkerType::NegInt((len, v)) => {
                let val = read_neg_int(raw, len, v)?;
                Validator::Integer(ValidInt::from_const(val, is_query))
            },
            MarkerType::PosInt((len, v)) => {
                let val = read_pos_int(raw, len, v)?;
                Validator::Integer(ValidInt::from_const(val, is_query))
            },
            MarkerType::String(len) => {
                let val = read_raw_str(raw, len)?;
                Validator::String(ValidStr::from_const(val, is_query))
            },
            MarkerType::F32 => {
                let val = raw.read_f32::<BigEndian>()?;
                Validator::F32(ValidF32::from_const(val, is_query))
            },
            MarkerType::F64 => {
                let val = raw.read_f64::<BigEndian>()?;
                Validator::F64(ValidF64::from_const(val, is_query))
            },
            MarkerType::Binary(len) => {
                let val = read_raw_bin(raw, len)?;
                Validator::Binary(ValidBin::from_const(val, is_query))
            },
            MarkerType::Hash(len) => {
                let val = read_raw_hash(raw, len)?;
                Validator::Hash(ValidHash::from_const(val, is_query))
            },
            MarkerType::Identity(len) => {
                let val = read_raw_id(raw, len)?;
                Validator::Identity(ValidIdentity::from_const(val, is_query))
            }
            MarkerType::Lockbox(_) => {
                return Err(Error::new(InvalidData, "Lockbox cannot be used in a schema"));
            }
            MarkerType::Timestamp(len) => {
                let val = read_raw_time(raw, len)?;
                Validator::Timestamp(ValidTime::from_const(val, is_query))
            },
            MarkerType::Array(len) => {
                let val = get_raw_array(raw, len)?;
                Validator::Array(ValidArray::from_const(val, is_query))
            }
            MarkerType::Object(len) => {
                // Create new validators and try them all.
                let mut possible = vec![
                    Validator::Null,
                    Validator::Type(String::from("")),
                    Validator::Boolean(ValidBool::new(is_query)),
                    Validator::Integer(ValidInt::new(is_query)),
                    Validator::String(ValidStr::new(is_query)),
                    Validator::F32(ValidF32::new(is_query)),
                    Validator::F64(ValidF64::new(is_query)),
                    Validator::Binary(ValidBin::new(is_query)),
                    Validator::Array(ValidArray::new(is_query)),
                    Validator::Object(ValidObj::new(is_query)),
                    Validator::Hash(ValidHash::new(is_query)),
                    Validator::Identity(ValidIdentity::new(is_query)),
                    Validator::Timestamp(ValidTime::new(is_query)),
                    Validator::Multi(ValidMulti::new(is_query)),
                ];
                let mut possible_check = vec![2u8; possible.len()];

                let mut type_seen = false;

                // Try all of the possible validators on each field
                object_iterate(raw, len, |field, raw| {
                    match field {
                        "comment" => {
                            read_str(raw).map_err(|_e| Error::new(InvalidData, "`comment` field didn't contain string"))?;
                        },
                        _ => {
                            if field == "type" { type_seen = true; }
                            let raw_now = &raw[..];
                            possible_check.iter_mut()
                                .zip(possible.iter_mut())
                                .filter(|(check,_)| **check > 0)
                                .for_each(|(check,validator)| {
                                    let mut raw_local = &raw_now[..];
                                    let result = validator.update(field, &mut raw_local, is_query, types, type_names)
                                        .and_then(|x| if x { Ok(2) } else { Ok(1) })
                                        .unwrap_or(0);
                                    if result != 0 {
                                        *raw = raw_local;
                                    }
                                    if (*check == 2) || ((*check == 1) && (result == 0)) {
                                        *check = result;
                                    }
                                });
                        }
                    }
                    Ok(())
                })?;

                let possible_count = possible_check.iter().fold(0, |acc, x| acc + if *x > 0 { 1 } else { 0 });
                if possible_count == possible.len() {
                    // Generic "valid" validator
                    Validator::Valid
                }
                else if possible_count > 1 {
                    if possible_check[1] > 0 {
                        possible[1].clone()
                    }
                    else {
                        return Err(Error::new(InvalidData, "Validator isn't specific enough. Specify more fields"))
                    }
                }
                else if possible_count != 0 {
                    let mut index: usize = 0;
                    for i in 0..possible_check.len() {
                        if possible_check[i] > 0 {
                            index = i;
                            break;
                        }
                    }
                    if type_seen {
                        let valid = possible[index].finalize();
                        if possible_check[index] == 1 || !valid {
                            Validator::Invalid
                        }
                        else {
                            possible[index].clone()
                        }
                    }
                    else {
                        return Err(Error::new(InvalidData, "Validator needs to include `type` field"));
                    }
                }
                else {
                    return Err(Error::new(InvalidData, "Not a recognized validator"));
                }
            },
        };

        if let Validator::Type(name) = validator {
            let index = type_names.entry(name.clone()).or_insert(types.len());
            if *index == types.len() {
                types.push(match name.as_str() {
                    "Null"  => Validator::Null,
                    "Bool"  => Validator::Boolean(ValidBool::new(is_query)),
                    "Int"   => Validator::Integer(ValidInt::new(is_query)),
                    "Str"   => Validator::String(ValidStr::new(is_query)),
                    "F32"   => Validator::F32(ValidF32::new(is_query)),
                    "F64"   => Validator::F64(ValidF64::new(is_query)),
                    "Bin"   => Validator::Binary(ValidBin::new(is_query)),
                    "Array" => Validator::Array(ValidArray::new(is_query)),
                    "Obj"   => Validator::Object(ValidObj::new(is_query)),
                    "Hash"  => Validator::Hash(ValidHash::new(is_query)),
                    "Ident" => Validator::Identity(ValidIdentity::new(is_query)),
                    "Lock"  => Validator::Lockbox(ValidLock::new(is_query)),
                    "Time"  => Validator::Timestamp(ValidTime::new(is_query)),
                    "Multi" => Validator::Invalid,
                    _ => Validator::Invalid,
                });
            }
            Ok(*index)
        }
        else {
            match validator {
                Validator::Invalid => Ok(INVALID),
                Validator::Valid => Ok(VALID),
                _ => {
                    types.push(validator);
                    Ok(types.len()-1)
                },
            }
        }

    }

    fn update(&mut self,
              field: &str,
              raw: &mut &[u8],
              is_query: bool,
              types: &mut Vec<Validator>,
              type_names: &mut HashMap<String,usize>
    )
        -> io::Result<bool>
    {
        if field == "comment" {
            read_str(raw).map_err(|_e| Error::new(InvalidData, "`comment` field didn't contain string"))?;
            return Ok(true);
        }
        match self {
            Validator::Type(ref mut v) => {
                match field {
                    "type" => {
                        v.push_str(read_str(raw)?);
                        Ok(true)
                    },
                    _ => Err(Error::new(InvalidData, "Unknown fields not allowed in Type validator")),
                }
            },
            Validator::Null => {
                match field {
                    "type" => if "Null" == read_str(raw)? { Ok(true) } else { Err(Error::new(InvalidData, "Type doesn't match Null")) },
                    _ => Err(Error::new(InvalidData, "Unknown fields not allowed in Null validator")),
                }
            },
            Validator::Boolean(v) => v.update(field, raw),
            Validator::Integer(v) => v.update(field, raw),
            Validator::String(v) => v.update(field, raw),
            Validator::F32(v) => v.update(field, raw),
            Validator::F64(v) => v.update(field, raw),
            Validator::Binary(v) => v.update(field, raw),
            Validator::Array(v) => v.update(field, raw, is_query, types, type_names),
            Validator::Object(v) => v.update(field, raw, is_query, types, type_names),
            Validator::Hash(v) => v.update(field, raw, is_query, types, type_names),
            Validator::Identity(v) => v.update(field, raw),
            Validator::Lockbox(v) => v.update(field, raw),
            Validator::Timestamp(v) => v.update(field, raw),
            Validator::Multi(v) => v.update(field, raw, is_query, types, type_names),
            Validator::Valid => Err(Error::new(InvalidData, "Fields not allowed in Valid validator")),
            Validator::Invalid => Err(Error::new(InvalidData, "Fields not allowed in Invalid validator")),
        }
    }

    pub fn finalize(&mut self) -> bool {
        match self {
            Validator::Invalid => false,
            Validator::Valid => true,
            Validator::Null => true,
            Validator::Type(_) => true,
            Validator::Boolean(v) => v.finalize(),
            Validator::Integer(v) => v.finalize(),
            Validator::String(v) => v.finalize(),
            Validator::F32(v) => v.finalize(),
            Validator::F64(v) => v.finalize(),
            Validator::Binary(v) => v.finalize(),
            Validator::Array(v) => v.finalize(),
            Validator::Object(v) => v.finalize(),
            Validator::Hash(v) => v.finalize(),
            Validator::Identity(v) => v.finalize(),
            Validator::Lockbox(v) => v.finalize(),
            Validator::Timestamp(v) => v.finalize(),
            Validator::Multi(v) => v.finalize(),
        }
    }

    pub fn validate(&self,
                    field: &str,
                    doc: &mut &[u8],
                    types: &Vec<Validator>,
                    index: usize,
                    list: &mut Checklist,
                    ) -> io::Result<()>
    {
        match self {
            Validator::Invalid => Err(Error::new(InvalidData, format!("Field \"{}\" is always invalid", field))),
            Validator::Valid => {
                verify_value(doc)?;
                Ok(())
            },
            Validator::Null => {
                read_null(doc)?;
                Ok(())
            },
            Validator::Type(_) => Err(Error::new(Other, "Should never be validating a `Type` validator directly")),
            Validator::Boolean(v) => v.validate(field, doc),
            Validator::Integer(v) => v.validate(field, doc),
            Validator::String(v) => v.validate(field, doc),
            Validator::F32(v) => v.validate(field, doc),
            Validator::F64(v) => v.validate(field, doc),
            Validator::Binary(v) => v.validate(field, doc),
            Validator::Array(v) => v.validate(field, doc, types, list),
            Validator::Object(v) => v.validate(field, doc, types, list, false),
            Validator::Hash(v) => {
                if let Some(hash) = v.validate(field, doc)? {
                    list.add(hash, index);
                }
                Ok(())
            },
            Validator::Identity(v) => v.validate(field, doc),
            Validator::Lockbox(v) => v.validate(field, doc),
            Validator::Timestamp(v) => v.validate(field, doc),
            Validator::Multi(v) => v.validate(field, doc, types, list),
        }
    }

    pub fn intersect(&self,
                 other: &Validator,
                 query: bool,
                 builder: &mut ValidBuilder
                 )
        -> Result<Validator, ()>
    {
        match self {
            Validator::Invalid => Ok(Validator::Invalid),
            Validator::Valid => {
                if query { return Err(()); } // Can't query a generic "Valid" validator
                // Check if other is also Valid to avoid infinite recursion
                if let Validator::Valid = other {
                    return Ok(Validator::Valid);
                }
                // Swap the builder's contents and intersect the other validator.
                builder.swap();
                let v = other.intersect(self, query, builder)?;
                builder.swap();
                Ok(v)
            }
            Validator::Null => {
                if let Validator::Null = other {
                    Ok(Validator::Null)
                }
                else {
                    Ok(Validator::Invalid)
                }
            },
            Validator::Type(_) => Err(()),
            Validator::Boolean(v) => v.intersect(other, query),
            Validator::Integer(v) => v.intersect(other, query),
            Validator::String(v) => v.intersect(other, query),
            Validator::F32(v) => v.intersect(other, query),
            Validator::F64(v) => v.intersect(other, query),
            Validator::Binary(v) => v.intersect(other, query),
            Validator::Array(v) => v.intersect(other, query, builder),
            Validator::Object(v) => v.intersect(other, query, builder),
            Validator::Hash(v) => v.intersect(other, query, builder),
            Validator::Identity(v) => v.intersect(other, query),
            Validator::Lockbox(v) => v.intersect(other, query),
            Validator::Timestamp(v) => v.intersect(other, query),
            Validator::Multi(v) => v.intersect(other, query, builder),
        }
    }
}
