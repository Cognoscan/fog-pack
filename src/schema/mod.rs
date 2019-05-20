use std::io;
use std::io::Error;
use std::io::ErrorKind::{InvalidData,Other};
use std::collections::HashMap;
use std::cmp::Ordering;
use std::mem;

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
mod object;
mod multi;

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
use self::object::ValidObj;
use self::multi::ValidMulti;

const MAX_VEC_RESERVE: usize = 2048;
const INVALID: usize = 0;
const VALID: usize = 1;

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

    pub fn merge(&mut self, mut other: Checklist) {
        for (hash, mut items) in other.list.drain() {
            self.list
                .entry(hash)
                .and_modify(|i| i.append(&mut items))
                .or_insert(items);
        }
    }

    pub fn get_list(&self, hash: &Hash) -> Option<&Vec<usize>> {
        self.list.get(hash)
    }

    pub fn check_off(&mut self, hash: &Hash) {
        self.list.remove(hash);
    }

    pub fn len(&self) -> usize {
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

    fn push(&mut self, new_type: Validator) -> usize {
        self.dest.push(new_type);
        self.len() - 1
    }

    fn intersect(&mut self, query: bool, type1: usize, type2: usize) -> Result<usize,()> {
        Ok(if ((type1 <= 1) && (type2 <= 1)) || (type1 == 0) || (type2 == 0) {
            // Only Valid if both valid, else invalid
            type1 & type2
        }
        else if type1 == 1 {
            // Clone type2 into the new validator list
            if self.map2[type2] == 0 {
                let v = self.types2[type2].intersect(&Validator::Valid, query, self)?;
                self.dest.push(v);
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
                let v = self.types1[type1].intersect(&Validator::Valid, query, self)?;
                self.dest.push(v);
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

    fn swap(&mut self) {
        mem::swap(&mut self.types1, &mut self.types2);
        mem::swap(&mut self.map1, &mut self.map2);
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
    object: ValidObj,
    entries: Vec<(String, usize)>,
    types: Vec<Validator>,
}

impl Schema {
    pub fn from_raw(raw: &mut &[u8]) -> io::Result<Schema> {
        let mut entries = Vec::new();
        let mut types = Vec::with_capacity(2);
        let mut type_names = HashMap::new();
        let mut object = ValidObj::new(true); // Documents can always be queried, hence "true"
        types.push(Validator::Invalid);
        types.push(Validator::Valid);

        let num_fields = match read_marker(raw)? {
            MarkerType::Object(len) => len,
            _ => return Err(Error::new(InvalidData, "Schema wasn't an object")),
        };
        object_iterate(raw, num_fields, |field, raw| {
            match field {
                "description" => {
                    read_str(raw).map_err(|_e| Error::new(InvalidData, "`description` field didn't contain string"))?;
                },
                "name" => {
                    read_str(raw).map_err(|_e| Error::new(InvalidData, "`name` field didn't contain string"))?;
                },
                "version" => {
                    read_integer(raw).map_err(|_e| Error::new(InvalidData, "`name` field didn't contain integer"))?;
                },
                "entries" => {
                    if let MarkerType::Object(len) = read_marker(raw)? {
                        object_iterate(raw, len, |field, raw| {
                            let v = Validator::read_validator(raw, false, &mut types, &mut type_names)?;
                            entries.push((field.to_string(), v));
                            Ok(())
                        })?;
                    }
                    else {
                        return Err(Error::new(InvalidData, "`entries` field doesn't contain an Object"));
                    }
                }
               "field_type" | "max_fields" | "min_fields" | "req" | "opt" | "unknown_ok" => {
                   object.update(field, raw, false, &mut types, &mut type_names)?;
                },
                _ => {
                    return Err(Error::new(InvalidData, "Unrecognized field in schema document"));
                }
            }
            Ok(())
        })?;

        Ok(Schema {
            object,
            entries,
            types,
        })
    }

    /// Validates a document against this schema. Does not check the schema field itself.
    pub fn validate_doc(&self, doc: &mut &[u8]) -> io::Result<()> {
        let mut checklist = Checklist::new();
        self.object.validate("", doc, &self.types, &mut checklist, true).and(Ok(()))
    }

    /// Validates a given entry against this schema.
    pub fn validate_entry(&self, entry: &str, doc: &mut &[u8]) -> io::Result<Checklist> {
        let mut checklist = Checklist::new();
        let v = self.entries.binary_search_by(|x| x.0.as_str().cmp(entry));
        if v.is_err() { return Err(Error::new(InvalidData, "Entry field type doesn't exist in schema")); }
        let v = self.entries[v.unwrap()].1;
        self.types[v].validate("", doc, &self.types, 0, &mut checklist)?;
        Ok(checklist)
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
            Validator::Object(v) => v.intersect(other, query, builder),
            Validator::Hash(v) => v.intersect(other, query, builder),
            Validator::Identity(v) => v.intersect(other, query),
            Validator::Lockbox(v) => v.intersect(other, query),
            Validator::Timestamp(v) => v.intersect(other, query),
            Validator::Multi(v) => v.intersect(other, query, builder),
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
