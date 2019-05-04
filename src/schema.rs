use std::io;
use std::io::Error;
use std::io::ErrorKind::{InvalidData,Other};
use std::collections::{HashMap, HashSet};

use byteorder::{ReadBytesExt, BigEndian};
use regex::Regex;

use super::{Hash, Identity, Integer, Timestamp};
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
    pub fn from_raw(raw: &mut &[u8]) -> io::Result<Schema> {
        let min_fields = usize::min_value();
        let max_fields = usize::max_value();
        let required = Vec::new();
        let optional = Vec::new();
        let entries = Vec::new();
        let types = HashMap::new();
        let field_type = None;
        let unknown_ok = false;

        let num_fields = match read_marker(raw)? {
            MarkerType::Object(len) => len,
            _ => return Err(Error::new(InvalidData, "Schema wasn't an object")),
        };
        if num_fields > 8 {
            return Err(Error::new(InvalidData, "Schema has more fields than it should"));
        }

        // Read entries
        // Read field_type
        // Read min_fields
        // Read max_fields
        // Read optional
        // Read required
        // Read types
        // Read unknown_ok
        // Optimize


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

    pub fn read_validator(raw: &mut &[u8], is_query: bool) -> io::Result<Validator> {
        match read_marker(raw)? {
            MarkerType::Null => Ok(Validator::Valid),
            MarkerType::Boolean(v) => {
                Ok(Validator::Boolean(
                    ValidBool {
                        constant: Some(v),
                        query: is_query,
                    }
                ))
            },
            MarkerType::NegInt((len, v)) => {
                let val = read_neg_int(raw, len, v)?;
                let mut in_vec = Vec::with_capacity(1);
                in_vec.push(val);
                Ok(Validator::Integer(
                    ValidInt {
                        in_vec,
                        nin_vec: Vec::with_capacity(0),
                        min: Integer::from(i64::min_value()),
                        max: Integer::from(u64::max_value()),
                        bit_set: 0,
                        bit_clear: 0,
                        query: is_query,
                        ord: is_query,
                        bit: is_query,
                    }
                ))
            },
            MarkerType::PosInt((len, v)) => {
                let val = read_pos_int(raw, len, v)?;
                let mut in_vec = Vec::with_capacity(1);
                in_vec.push(val);
                Ok(Validator::Integer(
                    ValidInt {
                        in_vec,
                        nin_vec: Vec::with_capacity(0),
                        min: Integer::min_value(),
                        max: Integer::max_value(),
                        bit_set: 0,
                        bit_clear: 0,
                        query: is_query,
                        ord: is_query,
                        bit: is_query,
                    }
                ))
            },
            MarkerType::String(len) => {
                let val = read_raw_str(raw, len)?.to_string();
                let mut in_vec = Vec::with_capacity(1);
                in_vec.push(val);
                Ok(Validator::String(
                    ValidStr {
                        in_vec,
                        nin_vec: Vec::with_capacity(0),
                        min_len: usize::min_value(),
                        max_len: usize::max_value(),
                        matches: Vec::with_capacity(0),
                        query: is_query,
                        ord: is_query,
                        regex: is_query,
                    }
                ))
            },
            MarkerType::F32 => {
                let val = raw.read_f32::<BigEndian>()?;
                let mut in_vec = Vec::with_capacity(1);
                in_vec.push(val);
                Ok(Validator::F32(
                    ValidF32 {
                        in_vec,
                        nin_vec: Vec::with_capacity(0),
                        min: ::std::f32::NEG_INFINITY,
                        max: ::std::f32::INFINITY,
                        query: is_query,
                        ord: is_query,
                    }
                ))
            },
            MarkerType::F64 => {
                let val = raw.read_f64::<BigEndian>()?;
                let mut in_vec = Vec::with_capacity(1);
                in_vec.push(val);
                Ok(Validator::F64(
                    ValidF64 {
                        in_vec,
                        nin_vec: Vec::with_capacity(0),
                        min: ::std::f64::NEG_INFINITY,
                        max: ::std::f64::INFINITY,
                        query: is_query,
                        ord: is_query,
                    }
                ))
            },
            MarkerType::Binary(len) => {
                let val = read_raw_bin(raw, len)?.to_vec();
                let mut in_vec = Vec::with_capacity(1);
                in_vec.push(val);
                Ok(Validator::Binary(
                    ValidBin {
                        in_vec,
                        nin_vec: Vec::with_capacity(0),
                        min_len: usize::min_value(),
                        max_len: usize::max_value(),
                        bit_set: Vec::with_capacity(0),
                        bit_clear: Vec::with_capacity(0),
                        query: is_query,
                        ord: is_query,
                        bit: is_query,
                    }
                ))
            },
            MarkerType::Hash(len) => {
                let val = read_raw_hash(raw, len)?;
                let mut in_vec = Vec::with_capacity(1);
                in_vec.push(val);
                Ok(Validator::Hash(
                    ValidHash {
                        in_vec,
                        nin_vec: Vec::with_capacity(0),
                        link: None,
                        schema: Vec::with_capacity(0),
                        query: is_query,
                        link_ok: is_query,
                    }
                ))
            },
            MarkerType::Identity(len) => {
                let val = read_raw_id(raw, len)?;
                let mut in_vec = Vec::with_capacity(1);
                in_vec.push(val);
                Ok(Validator::Identity(
                    ValidIdent {
                        in_vec,
                        nin_vec: Vec::with_capacity(0),
                        query: is_query,
                    }
                ))
            }
            MarkerType::Lockbox(_) => {
                Err(Error::new(InvalidData, "Lockbox cannot be used in a schema"))
            }
            MarkerType::Timestamp(len) => {
                let val = read_raw_time(raw, len)?;
                let mut in_vec = Vec::with_capacity(1);
                in_vec.push(val);
                Ok(Validator::Timestamp(
                    ValidTime {
                        in_vec,
                        nin_vec: Vec::with_capacity(0),
                        min: Timestamp::min_value(),
                        max: Timestamp::max_value(),
                        query: is_query,
                        ord: is_query,
                    }
                ))
            },
            MarkerType::Array(len) => {
                Err(Error::new(Other, "Can't yet decode this validator type"))
            }
            MarkerType::Object(len) => {
                let mut field = read_str(raw)?;
                // | Field        | Content                      |
                // | --           | --                           |
                // | any          | Array of Validators          |
                // | array        | non-negative integer         |
                // | bit          | non-negative integer         |
                // | bits_clr     | Integer/Binary               |
                // | bits_set     | Integer/Binary               |
                // | comment      | String                       |
                // | contains     | Array of Validators          |
                // | contains_num | non-negative integer         |
                // | default      | Any                          |
                // | description  | String                       |
                // | entries      | Object                       |
                // | ex_max       | Boolean                      |
                // | ex_min       | Boolean                      |
                // | extra_items  | Validator                    |
                // | field_type   | Validator                    |
                // | in           | Array of type                |
                // | items        | Array of Validators          |
                // | link         | Validator                    |
                // | link_ok      | bool                         |
                // | match        | String Array                 |
                // | max          | Numeric type                 |
                // | max_fields   | Non-negative Integer         |
                // | max_len      | Non-negative Integer         |
                // | min          | Numeric type                 |
                // | min_fields   | Non-negative Integer         |
                // | min_len      | Non-negative Integer         |
                // | name         | String                       |
                // | nin          | Array of type                |
                // | opt          | Object with Validator Values |
                // | ord          | bool                         |
                // | query        | bool                         |
                // | regex        | non-negative integer         |
                // | req          | Object with Validator Values |
                // | schema       | Array of Hashes              |
                // | set          | non-negative integer         |
                // | type         | String                       |
                // | types        | Object                       |
                // | unique       | Boolean                      |
                // | unknown_ok   | Boolean                      |
                // | version      | Integer                      |


                Err(Error::new(Other, "Can't yet decode this validator type"))
            }
        }
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
            Validator::Valid => {
                verify_value(doc)?;
                Ok(())
            },
            Validator::Null => {
                read_null(doc)?;
                Ok(())
            },
            Validator::Multi(v) => {
                if v.any_of.iter().any(|validator| {
                    let validator = self.fetch_validator(validator);
                    if let Err(_) = self.run_validator(field, validator, doc) {
                        false
                    }
                    else {
                        true
                    }
                })
                {
                    Ok(())
                }
                else {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" failed against all allowed types.", field)))
                }
            },
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
                let value_raw = value.as_bits();
                if value < v.min {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is {}, less than minimum of {}", field, value, v.min)))
                }
                else if value > v.max {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is {}, greater than maximum of {}", field, value, v.max)))
                }
                else if let Ok(_) = v.nin_vec.binary_search(&value) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is {}, which is on the `nin` list", field, value)))
                }
                else if (v.in_vec.len() > 0) && !v.in_vec.contains(&value) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is {}, which is not in the `in` list", field, value)))
                }
                else if (v.bit_set & value_raw) != v.bit_set {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is 0x{:X}, but must have set bits 0x{:X}", field, value_raw, v.bit_set)))
                }
                else if (v.bit_clear & value_raw) == 0 {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" is 0x{:X}, but must have cleared bits 0x{:X}", field, value_raw, v.bit_clear)))
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
                else if v.matches.iter().any(|reg| !reg.is_match(value)) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" fails regex check", field)))
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
                else if v.bit_set.iter().zip(value).any(|(bit, val)| (bit & val) != *bit) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" does not have all required bits set", field)))
                }
                else if v.bit_clear.iter().zip(value).any(|(bit, val)| (bit & val) != 0) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" does not have all required bits cleared", field)))
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
                else if let Some(_) = v.link {
                    Err(Error::new(Other,
                        format!("Field \"{}\" requires checking with `link`, which isn't implemented", field)))
                }
                else if v.schema.len() > 0 {
                    Err(Error::new(Other,
                        format!("Field \"{}\" requires checking with `schema`, which isn't implemented", field)))
                }
                else {
                    Ok(())
                }
            }
            Validator::Identity(v) => {
                let value = read_id(doc)?;
                if v.nin_vec.contains(&value) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" has Identity on `nin` list", field)))
                }
                else if (v.in_vec.len() > 0) && !v.in_vec.contains(&value) {
                    Err(Error::new(InvalidData,
                        format!("Field \"{}\" has Identity not on `in` list", field)))
                }
                else {
                    Ok(())
                }
            },
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
                if num_items == 0 && v.min_len == 0 && v.items.len() == 0 && v.contains.len() == 0 {
                    return Ok(());
                }

                // Size checks
                if num_items < v.min_len {
                    return Err(Error::new(InvalidData,
                        format!("Field {} contains array with {} items, less than minimum of {}", field, num_items, v.min_len)));
                }
                if num_items > v.max_len {
                    return Err(Error::new(InvalidData,
                        format!("Field {} contains array with {} items, greater than maximum of {}", field, num_items, v.max_len)));
                }

                // Setup for iterating over array
                let mut unique_set: HashSet<&[u8]> = if v.unique {
                    HashSet::with_capacity(num_items)
                }
                else {
                    HashSet::with_capacity(0)
                };
                let mut contain_set: Vec<bool> = vec![false; v.contains.len()];

                // Run through the whole array
                for i in 0..num_items {
                    // Validate as appropriate
                    let item_start = doc.clone();
                    if let Some(item_validator) = v.items.get(i) {
                        let item_validator = self.fetch_validator(item_validator);
                        if let Err(e) = self.run_validator(field, item_validator, doc) {
                            return Err(e);
                        }
                    }
                    else if let Some(ref item_validator) = v.extra_items {
                        let item_validator = self.fetch_validator(item_validator);
                        if let Err(e) = self.run_validator(field, item_validator, doc) {
                            return Err(e);
                        }
                    }
                    else {
                        verify_value(doc)?;
                    }
                    let (item, _) = item_start.split_at(item_start.len()-doc.len());

                    // Check for uniqueness
                    if v.unique {
                        if !unique_set.insert(item) {
                            return Err(Error::new(InvalidData,
                                format!("Field {} contains a repeated item at index {}", field, i)));
                        }
                    }
                    // Check to see if any `contains` requirements are met
                    contain_set.iter_mut()
                        .zip(v.contains.iter())
                        .filter(|(checked,_)| !**checked)
                        .for_each(|(checked,contains_item)| {
                            let item_validator = self.fetch_validator(contains_item);
                            if let Ok(()) = self.run_validator(field, item_validator, &mut item.clone()) {
                                *checked = true;
                            }
                        });
                }
                if contain_set.contains(&false) {
                    Err(Error::new(InvalidData,
                        format!("Field {} does not satisfy all `contains` requirements", field)))
                }
                else {
                    Ok(())
                }
            },
        }
    }
}

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
    Identity(ValidIdent),
    Lockbox(ValidLock),
    Timestamp(ValidTime),
    Multi(ValidMulti),
}

pub struct ValidBool {
    constant: Option<bool>,
    query: bool,
}

pub struct ValidInt {
    in_vec: Vec<Integer>,
    nin_vec: Vec<Integer>,
    min: Integer,
    max: Integer,
    bit_set: u64,
    bit_clear: u64,
    query: bool,
    ord: bool,
    bit: bool,
}

pub struct ValidStr {
    in_vec: Vec<String>,
    nin_vec: Vec<String>,
    min_len: usize,
    max_len: usize,
    matches: Vec<Regex>,
    query: bool,
    ord: bool,
    regex: bool,
}

pub struct ValidF32 {
    in_vec: Vec<f32>,
    nin_vec: Vec<f32>,
    min: f32,
    max: f32,
    query: bool,
    ord: bool,
}

pub struct ValidF64 {
    in_vec: Vec<f64>,
    nin_vec: Vec<f64>,
    min: f64,
    max: f64,
    query: bool,
    ord: bool,
}

pub struct ValidBin {
    in_vec: Vec<Vec<u8>>,
    nin_vec: Vec<Vec<u8>>,
    min_len: usize,
    max_len: usize,
    bit_set: Vec<u8>,
    bit_clear: Vec<u8>,
    query: bool,
    ord: bool,
    bit: bool,
}

pub struct ValidArray {
    /// Raw msgpack to compare against
    in_vec: Vec<Vec<u8>>,
    nin_vec: Vec<Vec<u8>>,
    min_len: usize,
    max_len: usize,
    items: Vec<Validator>,
    extra_items: Option<Box<Validator>>,
    contains: Vec<Validator>,
    unique: bool,
    query: bool,
    array: bool,
}

pub struct ValidObj {
    in_vec: Vec<Vec<u8>>,
    nin_vec: Vec<Vec<u8>>,
    required: Vec<(String, Validator)>,
    optional: Vec<(String, Validator)>,
    min_fields: usize,
    max_fields: usize,
    field_type: Option<Box<Validator>>,
    unknown_ok: bool,
    query: bool,
}

pub struct ValidHash {
    in_vec: Vec<Hash>,
    nin_vec: Vec<Hash>,
    link: Option<Box<Validator>>,
    schema: Vec<Hash>,
    query: bool,
    link_ok: bool,
}

pub struct ValidIdent {
    in_vec: Vec<Identity>,
    nin_vec: Vec<Identity>,
    query: bool,
}

pub struct ValidLock {
    max_len: usize,
    query: bool,
}

pub struct ValidTime {
    in_vec: Vec<Timestamp>,
    nin_vec: Vec<Timestamp>,
    min: Timestamp,
    max: Timestamp,
    query: bool,
    ord: bool,
}

pub struct ValidMulti {
    any_of: Vec<Validator>,
}






