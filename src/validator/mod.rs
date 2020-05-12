// Broad overview of the code
// ==========================
//
// The top-level struct here  is the Schema, which can be created from parsing raw msgpack that 
// satisfies the schema formating. It is created by parsing "validators", which are represented 
// here as an enum, `Validator`, which can be one of several types of validator. Each type has its 
// own module, with the exceptions of Null, Valid, Invalid, and Type.
//
// Validators are all stored in the Schema as a flat Vec, and are always indexed into this Vec. 
// This way, all validators are in a simple, constant-time lookup flat structure. The first two 
// elements of this structure are reserved for "Invalid" and "Valid", which are often used in other 
// validator code, especially in instersections.
//
// Because schema can have aliased, named validators in the "types" top-level field, construction 
// of these is a bit complex. The validators in the "types" fields are parsed, then, if a validator 
// is appended at the very end of the "types" Vec, it is popped off and put in the appropriate 
// place. If it isn't at the very end, then it is referencing another type and is ignored.
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

pub use self::bool::ValidBool;
pub use self::integer::ValidInt;
pub use self::float32::ValidF32;
pub use self::float64::ValidF64;
pub use self::time::ValidTime;
pub use self::lock::ValidLock;
pub use self::identity::ValidIdentity;
pub use self::binary::ValidBin;
pub use self::string::ValidStr;
pub use self::hash::ValidHash;
pub use self::array::{ValidArray, get_raw_array};
pub use self::object::ValidObj;
pub use self::multi::ValidMulti;

const MAX_VEC_RESERVE: usize = 2048;
const INVALID: usize = 0;
const VALID: usize = 1;

pub struct ValidatorChecklist {
    list: HashMap<Hash, Vec<usize>>
}

impl ValidatorChecklist {
    pub fn new() -> ValidatorChecklist {
        ValidatorChecklist { list: HashMap::new() }
    }

    pub fn add(&mut self, hash: Hash, index: usize) {
        self.list
            .entry(hash)
            .or_insert(Vec::with_capacity(1))
            .push(index)
    }

    pub fn merge(&mut self, mut other: ValidatorChecklist) {
        for (hash, mut items) in other.list.drain() {
            self.list
                .entry(hash)
                .and_modify(|i| i.append(&mut items))
                .or_insert(items);
        }
    }

    pub fn iter(&self) -> ::std::collections::hash_map::Iter<Hash, Vec<usize>> {
        self.list.iter()
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
                    list: &mut ValidatorChecklist,
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
