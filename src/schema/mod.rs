use std::io;
use std::io::Error;
use std::io::ErrorKind::{InvalidData,Other};
use std::collections::{HashMap, HashSet};
use std::cmp::Ordering;

use byteorder::{ReadBytesExt, BigEndian};
use regex::Regex;

use super::Hash;
use MarkerType;
use decode::*;

mod bool;
mod integer;
mod float32;
mod float64;
mod time;
mod lock;
mod identity;
mod binary;

use self::bool::ValidBool;
use self::integer::ValidInt;
use self::float32::ValidF32;
use self::float64::ValidF64;
use self::time::ValidTime;
use self::lock::ValidLock;
use self::identity::ValidIdentity;
use self::binary::ValidBin;

const MAX_VEC_RESERVE: usize = 2048;

/// Struct holding the validation portions of a schema. Can be used for validation of a document or 
/// entry.
pub struct Schema {
    min_fields: usize,
    max_fields: usize,
    required: Vec<(String, usize)>,
    optional: Vec<(String, usize)>,
    entries: Vec<(String, usize)>,
    types: Vec<Validator>,
    field_type: Option<usize>,
    unknown_ok: bool
}

impl Schema {
    pub fn from_raw(raw: &mut &[u8]) -> io::Result<Schema> {
        let min_fields = usize::min_value();
        let max_fields = usize::max_value();
        let required = Vec::new();
        let optional = Vec::new();
        let entries = Vec::new();
        let types = Vec::new();
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
        object_iterate(doc, num_fields, |field, doc| {
            // Check against required/optional/unknown types
            if Some(field) == self.required.get(req_index).map(|x| x.0.as_str()) {
                let v_index = self.required[req_index].1;
                req_index += 1;
                self.run_validator(field, &self.types[v_index], doc)
            }
            else if Some(field) == self.optional.get(opt_index).map(|x| x.0.as_str()) {
                let v_index = self.optional[opt_index].1;
                opt_index += 1;
                self.run_validator(field, &self.types[v_index], doc)
            }
            else if self.unknown_ok {
                if let Some(v_index) = self.field_type {
                    self.run_validator(field, &self.types[v_index], doc)
                }
                else {
                    verify_value(doc)?;
                    Ok(())
                }
            }
            else {
                Err(Error::new(InvalidData, format!("Unknown, invalid field: \"{}\"", field)))
            }
        })?;

        if req_index >= self.required.len() {
            Ok(())
        }
        else {
            Err(Error::new(InvalidData,
                format!("Missing required fields, starting with {}", self.required[req_index].0.as_str())))
        }
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
            Validator::Type(type_name) => {
                Err(Error::new(Other,
                    format!("Logical error: Validator::Type({}) should never exist after creation", type_name)))
            },
            Validator::Multi(v) => {
                if v.any_of.iter().any(|v_index| {
                    if let Err(_) = self.run_validator(field, &self.types[*v_index], doc) {
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
            Validator::Boolean(v) => {
                v.validate(field, doc)
            },
            Validator::Integer(v) => {
                v.validate(field, doc)
            },
            Validator::F32(v) => {
                v.validate(field, doc)
            },
            Validator::F64(v) => {
                v.validate(field, doc)
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
                v.validate(field, doc)
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
                v.validate(field, doc)
            },
            Validator::Lockbox(v) => {
                v.validate(field, doc)
            },
            Validator::Timestamp(v) => {
                v.validate(field, doc)
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
                object_iterate(doc, num_fields, |field, doc| {
                    // Check against required/optional/unknown types
                    if Some(field) == v.required.get(req_index).map(|x| x.0.as_str()) {
                        let v_index = v.required[req_index].1;
                        req_index += 1;
                        self.run_validator(field, &self.types[v_index], doc)
                    }
                    else if Some(field) == v.optional.get(opt_index).map(|x| x.0.as_str()) {
                        let v_index = v.optional[opt_index].1;
                        opt_index += 1;
                        self.run_validator(field, &self.types[v_index], doc)
                    }
                    else if v.unknown_ok {
                        if let Some(v_index) = v.field_type {
                            self.run_validator(field, &self.types[v_index], doc)
                        }
                        else {
                            verify_value(doc)?;
                            Ok(())
                        }
                    }
                    else {
                        Err(Error::new(InvalidData, format!("Unknown, invalid field: \"{}\"", field)))
                    }
                })?;

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
                    if let Some(v_index) = v.items.get(i) {
                        if let Err(e) = self.run_validator(field, &self.types[*v_index], doc) {
                            return Err(e);
                        }
                    }
                    else if let Some(v_index) = v.extra_items {
                        if let Err(e) = self.run_validator(field, &self.types[v_index], doc) {
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
                            if let Ok(()) = self.run_validator(field, &self.types[*contains_item], &mut item.clone()) {
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
    Identity(ValidIdentity),
    Lockbox(ValidLock),
    Timestamp(ValidTime),
    Multi(ValidMulti),
}

impl Validator {
    pub fn read_validator(raw: &mut &[u8], is_query: bool) -> io::Result<Validator> {
        match read_marker(raw)? {
            MarkerType::Null => Ok(Validator::Valid),
            MarkerType::Boolean(v) => {
                Ok(Validator::Boolean(ValidBool::from_const(v, is_query)))
            },
            MarkerType::NegInt((len, v)) => {
                let val = read_neg_int(raw, len, v)?;
                Ok(Validator::Integer(ValidInt::from_const(val, is_query)))
            },
            MarkerType::PosInt((len, v)) => {
                let val = read_pos_int(raw, len, v)?;
                Ok(Validator::Integer(ValidInt::from_const(val, is_query)))
            },
            MarkerType::String(len) => {
                let val = read_raw_str(raw, len)?;
                Ok(Validator::String(ValidStr::from_const(val, is_query)))
            },
            MarkerType::F32 => {
                let val = raw.read_f32::<BigEndian>()?;
                Ok(Validator::F32(ValidF32::from_const(val, is_query)))
            },
            MarkerType::F64 => {
                let val = raw.read_f64::<BigEndian>()?;
                Ok(Validator::F64(ValidF64::from_const(val, is_query)))
            },
            MarkerType::Binary(len) => {
                let val = read_raw_bin(raw, len)?;
                Ok(Validator::Binary(ValidBin::from_const(val, is_query)))
            },
            MarkerType::Hash(len) => {
                let val = read_raw_hash(raw, len)?;
                Ok(Validator::Hash(ValidHash::from_const(val, is_query)))
            },
            MarkerType::Identity(len) => {
                let val = read_raw_id(raw, len)?;
                Ok(Validator::Identity(ValidIdentity::from_const(val, is_query)))
            }
            MarkerType::Lockbox(_) => {
                Err(Error::new(InvalidData, "Lockbox cannot be used in a schema"))
            }
            MarkerType::Timestamp(len) => {
                let val = read_raw_time(raw, len)?;
                Ok(Validator::Timestamp(ValidTime::from_const(val, is_query)))
            },
            MarkerType::Array(_len) => {
                Err(Error::new(Other, "Can't yet decode this validator type"))
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
                let mut possible_check = vec![true; possible.len()];

                object_iterate(raw, len, |field, raw| {
                    match field {
                        "comment" => {
                            read_str(raw).map_err(|_e| Error::new(InvalidData, "`comment` field didn't contain string"))?;
                        },
                        _ => {
                            let mut raw_now = &raw[..];
                            possible_check.iter_mut()
                                .zip(possible.iter_mut())
                                .filter(|(check,_)| !**check)
                                .for_each(|(check,validator)| {
                                    let mut raw_local = &raw_now[..];
                                    let result = validator.update(field, &mut raw_local).unwrap_or(false);
                                    if result {
                                        *raw = raw_local;
                                    }
                                    *check = result;
                                });
                        }
                    }
                    Ok(())
                })?;

                    // For each validator, check to see if it can parse this field.
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

    fn update(&mut self, field: &str, raw: &mut &[u8]) -> io::Result<bool> {
        match self {
            Validator::Type(ref mut v) => {
                match field {
                    "type" => {
                        v.push_str(read_str(raw)?);
                        Ok(true)
                    },
                    _ => Ok(false),
                }
            },
            Validator::Null => {
                match field {
                    "type" => Ok("Null" == read_str(raw)?),
                    _ => Ok(false)
                }
            },
            Validator::Boolean(v) => v.update(field, raw),
            Validator::Integer(v) => v.update(field, raw),
            _ => Ok(false),
        }
    }

    pub fn validate(&self, field: &str, doc: &mut &[u8]) -> io::Result<()> {
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
            Validator::Boolean(v) => {
                v.validate(field, doc)
            },
            Validator::Integer(v) => {
                v.validate(field, doc)
            },
            Validator::F32(v) => {
                v.validate(field, doc)
            },
            Validator::F64(v) => {
                v.validate(field, doc)
            },
            Validator::Binary(v) => {
                v.validate(field, doc)
            },
            Validator::Identity(v) => {
                v.validate(field, doc)
            },
            Validator::Lockbox(v) => {
                v.validate(field, doc)
            },
            Validator::Timestamp(v) => {
                v.validate(field, doc)
            },
            _ => Err(Error::new(Other, "Can't validate this type yet")),
        }
    }
}

/// String type validator
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

impl ValidStr {
    fn new(is_query: bool) -> ValidStr {
        ValidStr {
            in_vec: Vec::with_capacity(0),
            nin_vec: Vec::with_capacity(0),
            min_len: usize::min_value(),
            max_len: usize::max_value(),
            matches: Vec::with_capacity(0),
            query: is_query,
            ord: is_query,
            regex: is_query,
        }
    }

    fn from_const(constant: &str, is_query: bool) -> ValidStr {
        let mut v = ValidStr::new(is_query);
        let mut in_vec = Vec::with_capacity(1);
        in_vec.push(constant.to_string());
        v.in_vec = in_vec;
        v
    }
}

pub struct ValidArray {
    /// Raw msgpack to compare against
    in_vec: Vec<Vec<u8>>,
    nin_vec: Vec<Vec<u8>>,
    min_len: usize,
    max_len: usize,
    items: Vec<usize>,
    extra_items: Option<usize>,
    contains: Vec<usize>,
    unique: bool,
    query: bool,
    array: bool,
}

/// Array type validator
impl ValidArray {
    fn new(is_query: bool) -> ValidArray {
        ValidArray {
            in_vec: Vec::with_capacity(0),
            nin_vec: Vec::with_capacity(0),
            min_len: usize::min_value(),
            max_len: usize::max_value(),
            items: Vec::with_capacity(0),
            extra_items: None,
            contains: Vec::with_capacity(0),
            unique: false,
            query: is_query,
            array: is_query,
        }
    }
}

/// Object type validator
pub struct ValidObj {
    in_vec: Vec<Vec<u8>>,
    nin_vec: Vec<Vec<u8>>,
    required: Vec<(String, usize)>,
    optional: Vec<(String, usize)>,
    min_fields: usize,
    max_fields: usize,
    field_type: Option<usize>,
    unknown_ok: bool,
    query: bool,
}

impl ValidObj {
    fn new(is_query: bool) -> ValidObj {
        // For `unknown_ok`, default to no unknowns allowed, the only time we are not permissive by 
        // default in schema validators.
        ValidObj {
            in_vec: Vec::with_capacity(0),
            nin_vec: Vec::with_capacity(0),
            required: Vec::with_capacity(0),
            optional: Vec::with_capacity(0),
            min_fields: usize::min_value(),
            max_fields: usize::max_value(),
            field_type: None,
            unknown_ok: is_query,
            query: is_query,
        }
    }
}

/// Hash type validator
pub struct ValidHash {
    in_vec: Vec<Hash>,
    nin_vec: Vec<Hash>,
    link: Option<Box<Validator>>,
    schema: Vec<Hash>,
    query: bool,
    link_ok: bool,
}

impl ValidHash {
    fn new(is_query: bool) -> ValidHash {
        ValidHash {
            in_vec: Vec::with_capacity(0),
            nin_vec: Vec::with_capacity(0),
            link: None,
            schema: Vec::with_capacity(0),
            query: is_query,
            link_ok: is_query,
        }
    }

    fn from_const(constant: Hash, is_query: bool) -> ValidHash {
        let mut v = ValidHash::new(is_query);
        let mut in_vec = Vec::with_capacity(1);
        in_vec.push(constant);
        v.in_vec = in_vec;
        v
    }
}

/// Container for multiple accepted Validators
pub struct ValidMulti {
    any_of: Vec<usize>,
}

impl ValidMulti {
    fn new(_is_query: bool) -> ValidMulti {
        ValidMulti {
            any_of: Vec::with_capacity(0)
        }
    }
}

/// Returns the union of two slices that have been sorted and deduplicated. The union is also 
/// sorted and deduplicated.
fn sorted_union<T,F>(in1: &[T], in2: &[T], compare: F) -> Vec<T> 
    where T: Clone, F: Fn(&T, &T) -> Ordering
{
    let mut new = Vec::with_capacity(in1.len() + in2.len());
    let mut i1 = 0;
    let mut i2 = 0;
    if (in2.len() > 0)  && (in1.len() > 0) {
        i1 = in1.binary_search_by(|probe| compare(probe, &in2[0])).unwrap_or_else(|x| x);
        new.extend_from_slice(&in1[0..i1]);
    }
    while let (Some(item1), Some(item2)) = (in1.get(i1), in2.get(i2)) {
        match compare(item1, item2) {
            Ordering::Less => {
                new.push(item1.clone());
                i1 += 1;
            },
            Ordering::Equal => {
                new.push(item1.clone());
                i1 += 1;
                i2 += 1;
            },
            Ordering::Greater => {
                new.push(item2.clone());
                i2 += 1;
            },
        }
    }
    if i1 < in1.len() {
        new.extend_from_slice(&in1[i1..]);
    }
    else {
        new.extend_from_slice(&in2[i2..]);
    }
    new.shrink_to_fit();
    new
}

/// Returns the intersection of two slices that have been sorted and deduplicated. The intersection 
/// is also sorted and deduplicated.
fn sorted_intersection<T,F>(in1: &[T], in2: &[T], compare: F) -> Vec<T> 
    where T: Clone, F: Fn(&T, &T) -> Ordering
{
    let mut new = Vec::with_capacity(in1.len().min(in2.len()));
    let mut i1 = 0;
    let mut i2 = 0;
    if (in2.len() > 0)  && (in1.len() > 0) {
        i1 = in1.binary_search_by(|probe| compare(probe, &in2[0])).unwrap_or_else(|x| x);
    }
    while let (Some(item1), Some(item2)) = (in1.get(i1), in2.get(i2)) {
        match compare(item1, item2) {
            Ordering::Less => {
                i1 += 1;
            },
            Ordering::Equal => {
                new.push(item1.clone());
                i1 += 1;
                i2 += 1;
            },
            Ordering::Greater => {
                i2 += 1;
            },
        }
    }
    new.shrink_to_fit();
    new
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;

    fn comp(in1: &i8, in2: &i8) -> Ordering {
        if in1 < in2 {
            Ordering::Less
        }
        else if in1 == in2 {
            Ordering::Equal
        }
        else {
            Ordering::Greater
        }
    }

    #[test]
    fn test_sorted_union() {
        let num_iter = 5000;
        let mut rng = rand::thread_rng();
        let range = rand::distributions::Uniform::new(-20,20);
        let len_range = rand::distributions::Uniform::new(0,32);

        let mut success = true;

        for _ in 0..num_iter {
            let len_x = rng.sample(len_range);
            let len_y = rng.sample(len_range);
            let mut x: Vec<i8> = Vec::with_capacity(len_x);
            let mut y: Vec<i8> = Vec::with_capacity(len_y);
            for _ in 0..len_x {
                x.push(rng.sample(range));
            }
            for _ in 0..len_y {
                y.push(rng.sample(range));
            }
            let mut z: Vec<i8> = Vec::with_capacity(len_x+len_y);
            z.extend_from_slice(&x);
            z.extend_from_slice(&y);
            z.sort_unstable();
            z.dedup();

            x.sort_unstable();
            x.dedup();
            y.sort_unstable();
            y.dedup();
            let z_test = sorted_union(&x,&y,comp);
            let equal = z == z_test;
            if !equal { success = false; break; }
        }

        assert!(success, "sorted_union did not work for all random vectors");
    }

    #[test]
    fn test_sorted_intersection() {
        let num_iter = 5000;
        let mut rng = rand::thread_rng();
        let range = rand::distributions::Uniform::new(-20,20);
        let len_range = rand::distributions::Uniform::new(0,32);

        let mut success = true;

        for _ in 0..num_iter {
            let len_x = rng.sample(len_range);
            let len_y = rng.sample(len_range);
            let mut x: Vec<i8> = Vec::with_capacity(len_x);
            let mut y: Vec<i8> = Vec::with_capacity(len_y);
            for _ in 0..len_x {
                x.push(rng.sample(range));
            }
            for _ in 0..len_y {
                y.push(rng.sample(range));
            }
            x.sort_unstable();
            x.dedup();
            y.sort_unstable();
            y.dedup();

            let z: Vec<i8> = x.iter()
                .filter(|x_val| y.binary_search(x_val).is_ok())
                .map(|&x| x)
                .collect();

            let z_test = sorted_intersection(&x,&y,comp);
            let equal = z == z_test;
            if !equal { success = false; break; }
        }

        assert!(success, "sorted_intersection did not work for all random vectors");
    }
}
