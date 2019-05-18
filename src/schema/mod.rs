use std::io;
use std::io::Error;
use std::io::ErrorKind::{InvalidData,Other};
use std::collections::HashMap;
use std::cmp::Ordering;

use byteorder::{ReadBytesExt, BigEndian};

use MarkerType;
use decode::*;
use crypto::Hash;

mod bool;
mod integer;
mod float32;
mod float64;
mod time;
mod lock;
mod identity;
mod binary;
mod string;
mod hash;
mod array;

use self::bool::ValidBool;
use self::integer::ValidInt;
use self::float32::ValidF32;
use self::float64::ValidF64;
use self::time::ValidTime;
use self::lock::ValidLock;
use self::identity::ValidIdentity;
use self::binary::ValidBin;
use self::string::ValidStr;
use self::hash::ValidHash;
use self::array::ValidArray;

const MAX_VEC_RESERVE: usize = 2048;

pub struct Checklist {
    list: HashMap<Hash, Vec<usize>>
}

impl Checklist {
    pub fn new() -> Checklist {
        Checklist { list: HashMap::new() }
    }

    pub fn add(&mut self, hash: Hash, index: usize) {
        self.list
            .entry(hash)
            .or_insert(Vec::with_capacity(1))
            .push(index)
    }

    pub fn get_list(&self, hash: &Hash) -> Option<&Vec<usize>> {
        self.list.get(hash)
    }

    pub fn check_off(&mut self, hash: &Hash) {
        self.list.remove(hash);
    }

    pub fn list_size(&self) -> usize {
        self.list.len()
    }
}

pub struct ValidBuilder<'a> {
    types1: &'a [Validator],
    types2: &'a [Validator],
    dest: Vec<Validator>,
    map1: Vec<usize>,
    map2: Vec<usize>
}

impl <'a> ValidBuilder<'a> {
    fn init(types1: &'a [Validator], types2: &'a [Validator]) -> ValidBuilder<'a> {
        ValidBuilder {
            types1,
            types2,
            dest: Vec::new(),
            map1: vec![0; types1.len()],
            map2: vec![0; types2.len()],
        }
    }

    fn intersect(&mut self, query: bool, type1: usize, type2: usize) -> Result<usize,()> {
        Ok(if ((type1 <= 1) && (type2 <= 1)) || (type1 == 0) || (type2 == 0) {
            // Only Valid if both valid, else invalid
            type1 & type2
        }
        else if type1 == 1 {
            // Clone type2 into the new validator list
            if self.map2[type2] == 0 {
                self.dest.push(self.types2[type2].clone());
                let new_index = self.dest.len() - 1;
                self.map2[type2] = new_index;
                new_index
            }
            else {
                self.map2[type2]
            }
        }
        else if type2 == 1 {
            // Clone type1 into the new validator list
            if self.map1[type1] == 0 {
                self.dest.push(self.types1[type1].clone());
                let new_index = self.dest.len() - 1;
                self.map1[type1] = new_index;
                new_index
            }
            else {
                self.map1[type1]
            }
        }
        else {
            // Actual new validator; perform instersection and add
            let v = self.types1[type1].intersect(&self.types2[type2], query, self)?;
            if let Validator::Invalid = v {
                0
            }
            else {
                self.dest.push(v);
                self.dest.len() - 1
            }
        })
    }

    fn len(&self) -> usize {
        self.dest.len()
    }

    fn undo_to(&mut self, len: usize) {
        self.dest.truncate(len);
        self.map1.iter_mut().for_each(|x| if *x >= len { *x = 0; });
        self.map2.iter_mut().for_each(|x| if *x >= len { *x = 0; });
    }

    fn build(self) -> Vec<Validator> {
        self.dest
    }
}

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
        let mut types = Vec::with_capacity(1);
        let field_type = None;
        let unknown_ok = false;
        types.push(Validator::Invalid);
        types.push(Validator::Valid);

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
    pub fn validate_doc(&self, doc: &mut &[u8]) -> io::Result<Checklist> {
        let mut checklist = Checklist::new();
        let num_fields = match read_marker(doc)? {
            MarkerType::Object(len) => len,
            _ => return Err(Error::new(InvalidData, "Document wasn't an object")),
        };
        if num_fields < self.min_fields {
            return Err(Error::new(InvalidData,
                format!("Document contains {} fields, less than the {} required",
                    num_fields, self.min_fields)));
        }
        if num_fields == 0 && self.required.len() == 0 { return Ok(checklist); }
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
                self.run_validator(field, &self.types[v_index], doc, v_index, &mut checklist)
            }
            else if Some(field) == self.optional.get(opt_index).map(|x| x.0.as_str()) {
                let v_index = self.optional[opt_index].1;
                opt_index += 1;
                self.run_validator(field, &self.types[v_index], doc, v_index, &mut checklist)
            }
            else if self.unknown_ok {
                if let Some(v_index) = self.field_type {
                    self.run_validator(field, &self.types[v_index], doc, v_index, &mut checklist)
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
            Ok(checklist)
        }
        else {
            Err(Error::new(InvalidData,
                format!("Missing required fields, starting with {}", self.required[req_index].0.as_str())))
        }
    }

    fn run_validator(&self, field: &str, validator: &Validator, doc: &mut &[u8], index: usize, list: &mut Checklist) -> io::Result<()> {
        let result = match validator {
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
                    if let Err(_) = self.run_validator(field, &self.types[*v_index], doc, *v_index, list) {
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
                v.validate(field, doc)
            }
            Validator::Binary(v) => {
                v.validate(field, doc)
            }
            Validator::Hash(v) => {
                if let Some(hash) = v.validate(field, doc)? {
                    list.add(hash, index);
                }
                Ok(())
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
                        self.run_validator(field, &self.types[v_index], doc, v_index, list)
                    }
                    else if Some(field) == v.optional.get(opt_index).map(|x| x.0.as_str()) {
                        let v_index = v.optional[opt_index].1;
                        opt_index += 1;
                        self.run_validator(field, &self.types[v_index], doc, v_index, list)
                    }
                    else if v.unknown_ok {
                        if let Some(v_index) = v.field_type {
                            self.run_validator(field, &self.types[v_index], doc, v_index, list)
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
                v.validate(field, doc, &self.types, list)
            },
        };
        if let Err(e) = result { return Err(e); }
        Ok(())
    }

}

#[derive(Clone)]
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
            MarkerType::Array(_len) => {
                return Err(Error::new(Other, "Can't yet decode this validator type"));
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
                                    let result = validator.update(field, &mut raw_local, is_query, types, type_names).unwrap_or(false);
                                    if result {
                                        *raw = raw_local;
                                    }
                                    *check = result;
                                });
                        }
                    }
                    Ok(())
                })?;

                // Multi
                // | any          | Array of Validators          |
                //
                // Array
                // | array        | non-negative integer         |
                // | contains     | Array of Validators          |
                // | contains_num | non-negative integer         |
                // | extra_items  | Validator                    |
                // | items        | Array of Validators          |
                // | unique       | Boolean                      |
                //
                // Object
                // | field_type   | Validator                    |
                // | max_fields   | Non-negative Integer         |
                // | min_fields   | Non-negative Integer         |
                // | opt          | Object with Validator Values |
                // | req          | Object with Validator Values |
                // | unknown_ok   | Boolean                      |
                //
                // | name         | String                       |
                // | version      | Integer                      |
                // | types        | Object                       |
                // | description  | String                       |
                // | entries      | Object                       |


                return Err(Error::new(Other, "Can't yet decode this validator type"));
            }
        };

        if let Validator::Type(name) = validator {
            let index = type_names.entry(name).or_insert(types.len());
            if *index == types.len() {
                types.push(Validator::Invalid);
            }
            Ok(*index)
        }
        else {
            types.push(validator);
            Ok(types.len()-1)
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
                    "type" => Ok("Null" == read_str(raw)?),
                    _ => Err(Error::new(InvalidData, "Unknown fields not allowed in Null validator")),
                }
            },
            Validator::Boolean(v) => v.update(field, raw),
            Validator::Integer(v) => v.update(field, raw),
            Validator::String(v) => v.update(field, raw),
            Validator::F32(v) => v.update(field, raw),
            Validator::F64(v) => v.update(field, raw),
            Validator::Binary(v) => v.update(field, raw),
            Validator::Array(_) => Err(Error::new(Other, "Validator not supported yet")),
            Validator::Object(_) => Err(Error::new(Other, "Validator not supported yet")),
            Validator::Hash(v) => v.update(field, raw, is_query, types, type_names),
            Validator::Identity(v) => v.update(field, raw),
            Validator::Lockbox(v) => v.update(field, raw),
            Validator::Timestamp(v) => v.update(field, raw),
            Validator::Multi(_) => Err(Error::new(Other, "Validator not supported yet")),
            _ => Ok(false),
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
            Validator::Array(_) => false, //v.finalize(),
            Validator::Object(_) => false, //v.finalize(),
            Validator::Hash(v) => v.finalize(),
            Validator::Identity(v) => v.finalize(),
            Validator::Lockbox(v) => v.finalize(),
            Validator::Timestamp(v) => v.finalize(),
            Validator::Multi(_) => false, //v.finalize(),
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
            Validator::Boolean(v) => {
                v.validate(field, doc)
            },
            Validator::Integer(v) => {
                v.validate(field, doc)
            },
            Validator::String(v) => {
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
            Validator::Array(v) => {
                v.validate(field, doc, types, list)
            },
            Validator::Hash(v) => {
                if let Some(hash) = v.validate(field, doc)? {
                    list.add(hash, index);
                }
                Ok(())
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

    pub fn intersect(&self,
                 other: &Validator,
                 query: bool,
                 builder: &mut ValidBuilder
                 )
        -> Result<Validator, ()>
    {
        match self {
            Validator::Invalid => Ok(Validator::Invalid),
            Validator::Valid => Ok(other.clone()),
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
            Validator::Object(_) => Err(()), //v.intersect(other, query, self_types, other_types, new_types),
            Validator::Hash(v) => v.intersect(other, query, builder),
            Validator::Identity(v) => v.intersect(other, query),
            Validator::Lockbox(v) => v.intersect(other, query),
            Validator::Timestamp(v) => v.intersect(other, query),
            Validator::Multi(_) => Err(()), //v.intersect(other, query, self_types, other_types, new_types),
        }
    }
}

/// Object type validator
#[derive(Clone)]
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

/// Container for multiple accepted Validators
#[derive(Clone)]
pub struct ValidMulti {
    any_of: Vec<usize>,
}

impl ValidMulti {
    // When implementing this, figure out how to handle mergine `link` field in ValidHash too
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
