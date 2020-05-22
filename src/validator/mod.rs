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
use std::collections::HashMap;
use std::cmp::Ordering;
use std::mem;

use byteorder::{ReadBytesExt, BigEndian};

use MarkerType;
use decode::*;
use crypto::Hash;

mod validator;
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

pub use self::validator::Validator;
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

    pub fn to_map(self) -> HashMap<Hash, Vec<usize>> {
        self.list
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

pub struct ValidReader<'a> {
    pub is_query: bool,
    pub types: &'a mut Vec<Validator>,
    pub type_names: &'a mut HashMap<String, usize>,
    pub schema_hash: &'a Hash
}

impl <'a> ValidReader<'a> {
    pub fn new(
        is_query: bool,
        types: &'a mut Vec<Validator>,
        type_names: &'a mut HashMap<String, usize>,
        schema_hash: &'a Hash
    ) -> Self {
        Self {
            is_query,
            types,
            type_names,
            schema_hash,
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
