use std::io;
use std::io::Error;
use std::io::ErrorKind::{InvalidData,Other};
use std::collections::HashMap;

use regex::Regex;

use super::{Hash, Integer, Timestamp};
use MarkerType;
use decode::*;


/// Struct holding the validation portions of a schema. Can be used for validation of a document or 
/// entry.
pub struct Schema {
    min_fields: usize,
    max_fields: usize,
    required: Vec<(String, Validator)>,
    optional: Vec<(String, Validator)>,
    entries: Vec<(String, Validator)>,
    types: HashMap<String, Validator>,
    field_type: Option<Validator>,
    unknown_ok: bool
}

impl Schema {
    pub fn from_raw(_raw: &[u8]) -> io::Result<Schema> {
        let min_fields = usize::min_value();
        let max_fields = usize::max_value();
        let required = Vec::new();
        let optional = Vec::new();
        let entries = Vec::new();
        let types = HashMap::new();
        let field_type = None;
        let unknown_ok = false;

        Ok(Schema {
            min_fields,
            max_fields,
            required,
            optional,
            entries,
            types,
            field_type,
            unknown_ok,
        })
    }

    /// Validates a document against this schema. Does not check the schema field itself.
    pub fn validate_doc(&self, doc: &mut &[u8]) -> io::Result<()> {
        let num_fields = match read_marker(doc)? {
            MarkerType::Object(len) => len,
            _ => return Err(Error::new(InvalidData, "Document wasn't an object")),
        };
        if num_fields < self.min_fields {
            return Err(Error::new(InvalidData,
                format!("Document contains {} fields, less than the {} required",
                    num_fields, self.min_fields)));
        }
        if num_fields == 0 && self.required.len() == 0 { return Ok(()); }
        if num_fields > self.max_fields {
            return Err(Error::new(InvalidData,
                format!("Document contains {} fields, more than the {} allowed",
                    num_fields, self.max_fields)));
        }

        // Setup for loop
        let mut req_index = 0;
        let mut opt_index = 0;
        let mut old_field = read_str(&mut doc.clone())?;
        let mut field: &str;
        // Main loop to check all fields
        for i in 0..num_fields {
            field = read_str(doc)?;
            // Check for proper lexicographic order of fields
            if i != 0 {
                if field <= old_field {
                    return Err(Error::new(InvalidData, "Repeated or improperly ordered fields in document"));
                }
            }
            old_field = field;

            // Check against required/optional/unknown types
            let validator = 
                if Some(field) == self.required.get(req_index).map(|x| x.0.as_str()) {
                    let validator = &self.required[req_index].1;
                    req_index += 1;
                    validator
                }
                else if Some(field) == self.optional.get(opt_index).map(|x| x.0.as_str()) {
                    let validator = &self.optional[opt_index].1;
                    opt_index += 1;
                    validator
                }
                else if self.unknown_ok {
                    if let Some(ref validator) = self.field_type {
                        validator
                    }
                    else {
                        verify_value(doc)?;
                        &Validator::Valid
                    }
                }
                else {
                    return Err(Error::new(InvalidData, format!("Unknown, invalid field: \"{}\"", field)));
                };

            // Run check
            let validator = self.fetch_validator(validator);
            if let Err(e) = self.run_validator(field, validator, doc) {
                return Err(e);
            }
        }

        if req_index >= self.required.len() {
            Ok(())
        }
        else {
            Err(Error::new(InvalidData,
                format!("Missing required fields, starting with {}", self.required[req_index].0.as_str())))
        }
    }

    fn fetch_validator<'a>(&'a self, v: &'a Validator) -> &'a Validator {
        if let &Validator::Type(ref s) = v {
            if let Some(ty) = self.types.get(s) {
                ty
            }
            else { &Validator::Invalid }
        }
        else { v }
    }

    fn run_validator(&self, field: &str, validator: &Validator, doc: &mut &[u8]) -> io::Result<()> {
        match validator {
            Validator::Invalid => Err(Error::new(InvalidData, format!("Field \"{}\" is always invalid", field))),
            Validator::Valid => Ok(()),
            Validator::Type(_) => Err(Error::new(Other,
                format!("Shouldn't happen: didn't get validator for Field \"{}\"", field))),
            Validator::Boolean(v) => {
                let value = read_bool(doc)?;
                match v.constant {
                    Some(b) => if b == value { Ok(()) } else { Err(Error::new(InvalidData,
                        format!("Field \"{}\" isn't set to {}", field, b))) },
                    None => Ok(()),
                }
            },
            Validator::Integer(v) => {
                let value = read_integer(doc)?;
                if value < v.min {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is {}, less than minimum of {}", field, value, v.min)))
                }
                else if value > v.max {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is {}, greater than maximum of {}", field, value, v.max)))
                }
                else if v.nin_vec.contains(&value) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is {}, which is on the `nin` list", field, value)))
                }
                else if (v.in_vec.len() > 0) && !v.in_vec.contains(&value) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is {}, which is not in the `in` list", field, value)))
                }
                else {
                    Ok(())
                }
            },
            Validator::F32(v) => {
                let value = read_f32(doc)?;
                if value < v.min {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is {}, less than minimum of {}", field, value, v.min)))
                }
                else if value > v.max {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is {}, greater than maximum of {}", field, value, v.max)))
                }
                else if v.nin_vec.contains(&value) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is {}, which is on the `nin` list", field, value)))
                }
                else if (v.in_vec.len() > 0) && !v.in_vec.contains(&value) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is {}, which is not in the `in` list", field, value)))
                }
                else {
                    Ok(())
                }
            },
            Validator::F64(v) => {
                let value = read_f64(doc)?;
                if value < v.min {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is {}, less than minimum of {}", field, value, v.min)))
                }
                else if value > v.max {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is {}, greater than maximum of {}", field, value, v.max)))
                }
                else if v.nin_vec.contains(&value) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is {}, which is on the `nin` list", field, value)))
                }
                else if (v.in_vec.len() > 0) && !v.in_vec.contains(&value) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is {}, which is not in the `in` list", field, value)))
                }
                else {
                    Ok(())
                }
            },
            Validator::String(v) => {
                let value = read_str(doc)?;
                if value.len() < v.min_len {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" contains string shorter than min length of {}", field, v.min_len)))
                }
                else if value.len() > v.max_len {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" contains string longer than max length of {}", field, v.min_len)))
                }
                else if v.nin_vec.iter().any(|x| x == value) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" contains string on the `nin` list", field)))
                }
                else if (v.in_vec.len() > 0) && !v.in_vec.iter().any(|x| x == value) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" contains string not on the `in` list", field)))
                }
                else if let Some(ref reg) = v.matches {
                    if !reg.is_match(value) {
                        Err(Error::new(InvalidData,
                            format!("Field \"{}\" fails regex check", field)))
                    }
                    else {
                        Ok(())
                    }
                }
                else {
                    Ok(())
                }
            }
            Validator::Binary(v) => {
                let value = read_bin(doc)?;
                if value.len() < v.min_len {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" contains binary shorter than min length of {}", field, v.min_len)))
                }
                else if value.len() > v.max_len {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" contains binary longer than max length of {}", field, v.min_len)))
                }
                else if v.nin_vec.iter().any(|x| value == &x[..]) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" contains binary on the `nin` list", field)))
                }
                else if (v.in_vec.len() > 0) && !v.in_vec.iter().any(|x| value == &x[..]) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" contains binary not on the `in` list", field)))
                }
                else {
                    Ok(())
                }
            }
            Validator::Hash(v) => {
                let value = read_hash(doc)?;
                if v.nin_vec.contains(&value) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" contains hash on the `nin` list", field)))
                }
                else if (v.in_vec.len() > 0) && !v.in_vec.contains(&value) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" contains hash not on the `in` list", field)))
                }
                else {
                    Ok(())
                }
            }
            Validator::Lockbox(v) => {
                let value = read_lockbox(doc)?;
                if value.len() > v.max_len {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" contains lockbox longer than max length of {}", field, v.max_len)))
                }
                else {
                    Ok(())
                }
            },
            Validator::Timestamp(v) => {
                let value = read_time(doc)?;
                if value < v.min {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" Timestamp = {}, less than minimum of {}", field, value, v.min)))
                }
                else if value > v.max {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" Timestamp = {}, greater than maximum of {}", field, value, v.max)))
                }
                else if v.nin_vec.contains(&value) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" Timestamp = {}, which is on the `nin` list", field, value)))
                }
                else if (v.in_vec.len() > 0) && !v.in_vec.contains(&value) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" Timestamp = {}, which is not in the `in` list", field, value)))
                }
                else {
                    Ok(())
                }
            },
            Validator::Object(v) => {
                let num_fields = match read_marker(doc)? {
                    MarkerType::Object(len) => len,
                    _ => return Err(Error::new(InvalidData, "Object not found")),
                };
                if num_fields < v.min_fields {
                    return Err(Error::new(InvalidData,
                        format!("Field \"{}\" contains object with {} fields, less than the {} required",
                            field, num_fields, v.min_fields)));
                }
                if num_fields == 0 && v.required.len() == 0 { return Ok(()); }
                if num_fields > v.max_fields {
                    return Err(Error::new(InvalidData,
                        format!("Field \"{}\" contains object with {} fields, more than the {} required",
                            field, num_fields, v.max_fields)));
                }

                // Setup for loop
                let parent_field = field;
                let obj_start = doc.clone();
                let mut req_index = 0;
                let mut opt_index = 0;
                let mut old_field = read_str(&mut doc.clone())?;
                let mut field: &str;
                // Main loop to check all fields
                for i in 0..num_fields {
                    field = read_str(doc)?;
                    // Check for proper lexicographic order of fields
                    if i != 0 {
                        if field <= old_field {
                            return Err(Error::new(InvalidData, "Repeated or improperly ordered fields in object"));
                        }
                    }
                    old_field = field;

                    // Check against required/optional/unknown types
                    let validator = 
                        if Some(field) == v.required.get(req_index).map(|x| x.0.as_str()) {
                            let validator = &v.required[req_index].1;
                            req_index += 1;
                            validator
                        }
                        else if Some(field) == v.optional.get(opt_index).map(|x| x.0.as_str()) {
                            let validator = &v.optional[opt_index].1;
                            opt_index += 1;
                            validator
                        }
                        else if v.unknown_ok {
                            if let Some(ref validator) = v.field_type {
                                validator
                            }
                            else {
                                verify_value(doc)?;
                                &Validator::Valid
                            }
                        }
                        else {
                            return Err(Error::new(InvalidData, format!("Unknown, invalid field: \"{}\"", field)));
                        };
                    
                    // Run check
                    let validator = self.fetch_validator(validator);
                    if let Err(e) = self.run_validator(field, validator, doc) {
                        return Err(e);
                    }
                }

                let (obj_start, _) = obj_start.split_at(obj_start.len()-doc.len());
                if v.nin_vec.iter().any(|x| obj_start == &x[..]) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" contains object on the `nin` list", parent_field)))
                }
                else if (v.in_vec.len() > 0) && !v.in_vec.iter().any(|x| obj_start == &x[..]) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" contains object not in the `in` list", parent_field)))
                }
                else if req_index < v.required.len() {
                    Err(Error::new(InvalidData,
                        format!("Missing required fields, starting with {}", v.required[req_index].0.as_str())))
                }
                else {
                    Ok(())
                }
            },
            Validator::Array(v) => {
                let num_items = match read_marker(doc)? {
                    MarkerType::Array(len) => len,
                    _ => return Err(Error::new(InvalidData, format!("Array for field \"{}\"not found", field))),
                };
                //if num_items == 0 && v.min_len == 0 { return Ok(()); }
                //min_len: usize,
                //max_len: usize,
                //items: Vec<Validator>,
                //extra_items: Option<Box<Validator>>,
                //contains: Vec<Validator>,
                //unique: bool,
                Err(Error::new(InvalidData, format!("Checking for field \"{}\" not implemented", field)))
            },
        }
    }
}

enum Validator {
    Invalid,
    Valid,
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
    Lockbox(ValidLock),
    Timestamp(ValidTime),
}

struct ValidBool {
    constant: Option<bool>,
}

struct ValidInt {
    in_vec: Vec<Integer>,
    nin_vec: Vec<Integer>,
    min: Integer,
    max: Integer,
}

struct ValidStr {
    in_vec: Vec<String>,
    nin_vec: Vec<String>,
    min_len: usize,
    max_len: usize,
    matches: Option<Box<Regex>>,
}

struct ValidF32 {
    in_vec: Vec<f32>,
    nin_vec: Vec<f32>,
    min: f32,
    max: f32,
}

struct ValidF64 {
    in_vec: Vec<f64>,
    nin_vec: Vec<f64>,
    min: f64,
    max: f64,
}

struct ValidBin {
    in_vec: Vec<Vec<u8>>,
    nin_vec: Vec<Vec<u8>>,
    min_len: usize,
    max_len: usize,
}

struct ValidArray {
    /// Raw msgpack to compare against
    in_vec: Vec<Vec<u8>>,
    nin_vec: Vec<Vec<u8>>,
    min_len: usize,
    max_len: usize,
    items: Vec<Validator>,
    extra_items: Option<Box<Validator>>,
    contains: Vec<Validator>,
    unique: bool,
}

struct ValidObj {
    in_vec: Vec<Vec<u8>>,
    nin_vec: Vec<Vec<u8>>,
    required: Vec<(String, Validator)>,
    optional: Vec<(String, Validator)>,
    min_fields: usize,
    max_fields: usize,
    field_type: Option<Box<Validator>>,
    unknown_ok: bool
}

struct ValidHash {
    in_vec: Vec<Hash>,
    nin_vec: Vec<Hash>,
}

struct ValidLock {
    max_len: usize,
}

struct ValidTime {
    in_vec: Vec<Timestamp>,
    nin_vec: Vec<Timestamp>,
    min: Timestamp,
    max: Timestamp,
}






