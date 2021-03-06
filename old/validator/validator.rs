use super::*;
use Error;

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
    pub fn read_validator(
        raw: &mut &[u8],
        reader: &mut ValidReader
    )
        -> crate::Result<usize>
    {
        let is_query = reader.is_query;
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
                return Err(Error::FailValidate(raw.len(), "Lockbox cannot be used in a schema"));
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
                // possible_check contains the status of each validator as we iterate through the 
                // fields:
                //  - 2: it's still acceptable
                //  - 1: it's accepted but will never allow a value to pass
                //  - 0: the validator can't be used
                let mut possible_check = vec![2u8; possible.len()];

                let mut type_seen = false;

                // Try all of the possible validators on each field
                object_iterate(raw, len, |field, raw| {
                    match field {
                        "comment" => {
                            read_str(raw).map_err(|_e| Error::FailValidate(raw.len(), "`comment` field didn't contain string"))?;
                        },
                        _ => {
                            if field == "type" { type_seen = true; }
                            let raw_now = &raw[..];
                            possible_check.iter_mut()
                                .zip(possible.iter_mut())
                                .filter(|(check,_)| **check > 0)
                                .for_each(|(check,validator)| {
                                    let mut raw_local = &raw_now[..];
                                    let result = validator.update(field, &mut raw_local, reader)
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
                    // If there is more than one type, but one of them is Validator::Type, that 
                    // means we have one that's just one of the basic types with no other 
                    // constraints
                    if possible_check[1] > 0 {
                        possible[1].clone()
                    }
                    else {
                        // We didn't actually specify enough fields to narrow it down to one type.
                        return Err(Error::FailValidate(raw.len(), "Validator isn't specific enough. Specify more fields"))
                    }
                }
                else if possible_count != 0 {
                    let mut index: usize = 0;
                    for (i, possible) in possible_check.iter().enumerate() {
                        if *possible > 0 {
                            index = i;
                            break;
                        }
                    }
                    if type_seen {
                        //possible[index].finalize();
                        //if possible_check[index] == 1 || !valid {
                        possible[index].clone()
                    }
                    else {
                        return Err(Error::FailValidate(raw.len(), "Validator needs to include `type` field"));
                    }
                }
                else {
                    return Err(Error::FailValidate(raw.len(), "Not a recognized validator"));
                }
            },
        };

        if let Validator::Type(name) = validator {
            let types_len = reader.types.len();
            let index = reader.type_names.entry(name.clone()).or_insert_with(|| types_len);
            if *index == types_len {
                reader.types.push(match name.as_str() {
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
                    reader.types.push(validator);
                    Ok(reader.types.len()-1)
                },
            }
        }

    }

    fn update(&mut self,
              field: &str,
              raw: &mut &[u8],
              reader: &mut ValidReader
    )
        -> crate::Result<bool>
    {
        if field == "comment" {
            read_str(raw).map_err(|_e| Error::FailValidate(raw.len(), "`comment` field didn't contain string"))?;
            return Ok(true);
        }
        match self {
            Validator::Type(ref mut v) => {
                match field {
                    "type" => {
                        v.push_str(read_str(raw)?);
                        Ok(true)
                    },
                    _ => Err(Error::FailValidate(raw.len(), "Unknown fields not allowed in Type validator")),
                }
            },
            Validator::Null => {
                match field {
                    "type" => if "Null" == read_str(raw)? { Ok(true) } else { Err(Error::FailValidate(raw.len(), "Type doesn't match Null")) },
                    _ => Err(Error::FailValidate(raw.len(), "Unknown fields not allowed in Null validator")),
                }
            },
            Validator::Boolean(v) => v.update(field, raw),
            Validator::Integer(v) => v.update(field, raw),
            Validator::String(v) => v.update(field, raw),
            Validator::F32(v) => v.update(field, raw),
            Validator::F64(v) => v.update(field, raw),
            Validator::Binary(v) => v.update(field, raw),
            Validator::Array(v) => v.update(field, raw, reader),
            Validator::Object(v) => v.update(field, raw, reader),
            Validator::Hash(v) => v.update(field, raw, reader),
            Validator::Identity(v) => v.update(field, raw),
            Validator::Lockbox(v) => v.update(field, raw),
            Validator::Timestamp(v) => v.update(field, raw),
            Validator::Multi(v) => v.update(field, raw, reader),
            Validator::Valid => Err(Error::FailValidate(raw.len(), "Fields not allowed in Valid validator")),
            Validator::Invalid => Err(Error::FailValidate(raw.len(), "Fields not allowed in Invalid validator")),
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
                    doc: &mut &[u8],
                    types: &[Validator],
                    index: usize,
                    list: &mut ValidatorChecklist,
                    ) -> crate::Result<()>
    {
        match self {
            Validator::Invalid => Err(Error::FailValidate(doc.len(), "Always Invalid")),
            Validator::Valid => {
                verify_value(doc)?;
                Ok(())
            },
            Validator::Null => {
                read_null(doc)?;
                Ok(())
            },
            Validator::Type(_) => Err(Error::FailValidate(doc.len(), "Should never be validating a `Type` validator directly")),
            Validator::Boolean(v) => v.validate(doc),
            Validator::Integer(v) => v.validate(doc),
            Validator::String(v) => v.validate(doc),
            Validator::F32(v) => v.validate(doc),
            Validator::F64(v) => v.validate(doc),
            Validator::Binary(v) => v.validate(doc),
            Validator::Array(v) => v.validate(doc, types, list),
            Validator::Object(v) => v.validate(doc, types, list, false),
            Validator::Hash(v) => {
                if let Some(hash) = v.validate(doc)? {
                    list.add(hash, index);
                }
                Ok(())
            },
            Validator::Identity(v) => v.validate(doc),
            Validator::Lockbox(v) => v.validate(doc),
            Validator::Timestamp(v) => v.validate(doc),
            Validator::Multi(v) => v.validate(doc, types, list),
        }
    }

}

pub fn query_check(s: usize, q: usize, s_types: &[Validator], q_types: &[Validator]) -> bool {
    let q_index = q;
    let s_index = s;
    let s = &s_types[s];
    let q = &q_types[q];

    // If other is type multi, verify it against each of these. This logic would otherwise have to 
    // be in each and every validator.
    if let Validator::Multi(q) = q {
        q.iter().all(|q| {
            query_check(s_index, *q, s_types, q_types)
        })
    }
    else {
        match s {
            Validator::Invalid => false,
            Validator::Valid => false,
            Validator::Null => { if let Validator::Null = q { true } else { false } },
            Validator::Type(_) => false,
            Validator::Boolean(v) => v.query_check(q),
            Validator::Integer(v) => v.query_check(q),
            Validator::String(v) => v.query_check(q),
            Validator::F32(v) => v.query_check(q),
            Validator::F64(v) => v.query_check(q),
            Validator::Binary(v) => v.query_check(q),
            Validator::Array(v) => v.query_check(q, s_types, q_types),
            Validator::Object(v) => v.query_check(q, s_types, q_types),
            Validator::Hash(v) => v.query_check(q, s_types, q_types),
            Validator::Identity(v) => v.query_check(q),
            Validator::Lockbox(v) => v.query_check(q),
            Validator::Timestamp(v) => v.query_check(q),
            Validator::Multi(v) => v.query_check(q_index, s_types, q_types),
        }
    }
}


#[cfg(test)]
mod tests {
    /*
    use super::*;

    #[test]
    fn sizes() {
        println!("valid = {}", std::mem::size_of::<Validator>());
        println!("Bool  = {}", std::mem::size_of::<ValidBool>());
        println!("Int   = {}", std::mem::size_of::<ValidInt>());
        println!("Str   = {}", std::mem::size_of::<ValidStr>());
        println!("F32   = {}", std::mem::size_of::<ValidF32>());
        println!("F64   = {}", std::mem::size_of::<ValidF64>());
        println!("Bin   = {}", std::mem::size_of::<ValidBin>());
        println!("Array = {}", std::mem::size_of::<ValidArray>());
        println!("Obj   = {}", std::mem::size_of::<ValidObj>());
        println!("Hash  = {}", std::mem::size_of::<ValidHash>());
        println!("Ident = {}", std::mem::size_of::<ValidIdentity>());
        println!("Lock  = {}", std::mem::size_of::<ValidLock>());
        println!("Time  = {}", std::mem::size_of::<ValidTime>());
        println!("Multi = {}", std::mem::size_of::<ValidMulti>());
        panic!("oop");
    }
    */
}
