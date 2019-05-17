use std::io;
use std::io::Error;
use std::io::ErrorKind::InvalidData;
use std::collections::{HashMap, HashSet};

use decode::*;
use super::*;
use marker::MarkerType;

#[derive(Clone)]
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
    contains_ok: bool,
}

/// Array type validator
impl ValidArray {
    pub fn new(is_query: bool) -> ValidArray {
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
            contains_ok: is_query,
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
            "array" => {
                self.array = read_bool(raw)?;
                Ok(true)
            }
            "contains" => {
                if let MarkerType::Array(len) = read_marker(raw)? {
                    for _ in 0..len {
                        let v = Validator::read_validator(raw, is_query, types, type_names)?;
                        self.contains.push(v);
                    }
                    Ok(true)
                }
                else {
                    Err(Error::new(InvalidData, "Array `contains` isn't a valid array of validators"))
                }
            },
            "contains_ok" => {
                self.contains_ok = read_bool(raw)?;
                Ok(true)
            },
            "default" => {
                if let MarkerType::Array(len) = read_marker(raw)? {
                    for _ in 0..len {
                        verify_value(raw)?;
                    }
                    Ok(true)
                }
                else {
                    Err(Error::new(InvalidData, "Array `default` isn't a valid array"))
                }
            },
            "extra_items" => {
                self.extra_items = Some(Validator::read_validator(raw, is_query, types, type_names)?);
                Ok(true)
            },
            "in" => {
                if let MarkerType::Array(len) = read_marker(raw)? {
                    // Push without reserving - otherwise recursive reserving is possible and 
                    // can lead to an exponential amount of memory reservation.
                    for _ in 0..len {
                        let v = if let MarkerType::Array(len) = read_marker(raw)? {
                            get_raw_array(raw, len)?
                        }
                        else {
                            return Err(Error::new(InvalidData, "Array validator expected array of arrays for `in` field"));
                        };
                        self.in_vec.push(v);
                    };
                    self.in_vec.sort_unstable();
                    self.in_vec.dedup();
                }
                else {
                    return Err(Error::new(InvalidData, "Array validator expected array of arrays for `in` field"));
                }
                Ok(true)
            },
            "items" => {
                if let MarkerType::Array(len) = read_marker(raw)? {
                    for _ in 0..len {
                        let v = Validator::read_validator(raw, is_query, types, type_names)?;
                        self.items.push(v);
                    }
                    Ok(true)
                }
                else {
                    Err(Error::new(InvalidData, "Array `items` isn't a valid array of validators"))
                }
            },
            "max_len" => {
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.max_len = len as usize;
                    Ok(true)
                }
                else {
                    Ok(false)
                }
            },
            "min_len" => {
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.min_len = len as usize;
                    Ok(self.max_len >= self.min_len)
                }
                else {
                    Ok(false)
                }
            },
            "nin" => {
                if let MarkerType::Array(len) = read_marker(raw)? {
                    // Push without reserving - otherwise recursive reserving is possible and 
                    // can lead to an exponential amount of memory reservation.
                    for _ in 0..len {
                        let v = if let MarkerType::Array(len) = read_marker(raw)? {
                            get_raw_array(raw, len)?
                        }
                        else {
                            return Err(Error::new(InvalidData, "Array validator expected array of arrays for `in` field"));
                        };
                        self.in_vec.push(v);
                    };
                    self.in_vec.sort_unstable();
                    self.in_vec.dedup();
                }
                else {
                    return Err(Error::new(InvalidData, "Array validator expected array of arrays for `in` field"));
                }
                Ok(true)
            },
            "query" => {
                self.query = read_bool(raw)?;
                Ok(true)
            },
            "unique" => {
                self.query = read_bool(raw)?;
                Ok(true)
            },
            "type" => Ok("Array" == read_str(raw)?),
            _ => Err(Error::new(InvalidData, "Unknown fields not allowed in Array validator")),
        }
    }

    /// Final check on the validator. Returns true if at least one value can (probably) still pass the 
    /// validator. We do not check the `in` and `nin` against all validation parts
    pub fn finalize(&mut self) -> bool {
        if self.in_vec.len() > 0 {
            let mut in_vec: Vec<Vec<u8>> = Vec::with_capacity(self.in_vec.len());
            let mut nin_index = 0;
            for val in self.in_vec.iter() {
                while let Some(nin) = self.nin_vec.get(nin_index) {
                    if nin < val { nin_index += 1; } else { break; }
                }
                if let Some(nin) = self.nin_vec.get(nin_index) {
                    if nin == val { continue; }
                }
                if (val.len() >= self.min_len) && (val.len() <= self.max_len) 
                {
                    in_vec.push(val.clone());
                }
            }
            in_vec.shrink_to_fit();
            self.in_vec = in_vec;
            self.nin_vec = Vec::with_capacity(0);
            self.in_vec.len() > 0
        }
        else {
            self.nin_vec.shrink_to_fit();
            self.min_len <= self.max_len
        }
    }

    /// Validates that the next value is a Hash that meets the validator requirements. Fails if the 
    /// requirements are not met. If it passes, the optional returned Hash indicates that an 
    /// additional document (referenced by the Hash) needs to be checked.
    pub fn validate(&self,
                    field: &str,
                    doc: &mut &[u8],
                    types: &Vec<Validator>,
                    list: &mut Checklist,
                    ) -> io::Result<()>
    {
        let num_items = match read_marker(doc)? {
            MarkerType::Array(len) => len,
            _ => return Err(Error::new(InvalidData, format!("Array for field \"{}\"not found", field))),
        };
        if num_items == 0 && self.min_len == 0 && self.items.len() == 0 && self.contains.len() == 0 {
            return Ok(());
        }

        // Size checks
        if num_items < self.min_len {
            return Err(Error::new(InvalidData,
                format!("Field {} contains array with {} items, less than minimum of {}", field, num_items, self.min_len)));
        }
        if num_items > self.max_len {
            return Err(Error::new(InvalidData,
                format!("Field {} contains array with {} items, greater than maximum of {}", field, num_items, self.max_len)));
        }

        // Setup for iterating over array
        let mut unique_set: HashSet<&[u8]> = if self.unique {
            HashSet::with_capacity(num_items)
        }
        else {
            HashSet::with_capacity(0)
        };
        let mut contain_set: Vec<bool> = vec![false; self.contains.len()];

        // Run through the whole array
        for i in 0..num_items {
            // Validate as appropriate
            let item_start = doc.clone();
            if let Some(v_index) = self.items.get(i) {
                if let Err(e) = types[*v_index].validate(field, doc, types, *v_index, list) {
                    return Err(e);
                }
            }
            else if let Some(v_index) = self.extra_items {
                if let Err(e) = types[v_index].validate(field, doc, types, v_index, list) {
                    return Err(e);
                }
            }
            else {
                verify_value(doc)?;
            }
            let (item, _) = item_start.split_at(item_start.len()-doc.len());

            // Check for uniqueness
            if self.unique {
                if !unique_set.insert(item) {
                    return Err(Error::new(InvalidData,
                        format!("Field {} contains a repeated item at index {}", field, i)));
                }
            }
            // Check to see if any `contains` requirements are met
            contain_set.iter_mut()
                .zip(self.contains.iter())
                .filter(|(checked,_)| !**checked)
                .for_each(|(checked,contains_item)| {
                    if let Ok(()) = types[*contains_item].validate(field, &mut item.clone(), types, *contains_item, list) {
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
    }

    /// Intersection of Array with other Validators. Returns Err only if `query` is true and the 
    /// other validator contains non-allowed query parameters.
    pub fn intersect(&self,
                 other: &Validator,
                 query: bool,
                 self_types: &Vec<Validator>,
                 other_types: &Vec<Validator>,
                 new_types: &mut Vec<Validator>,
                 self_map: &mut Vec<usize>,
                 other_map: &mut Vec<usize>,
                 )
        -> Result<Validator, ()>
    {
        let new_types_len = new_types.len();
        if query && !self.query && !self.array && !self.contains_ok { return Err(()); }
        match other {
            Validator::Array(other) => {
                if query && (
                    (!self.query &&
                     ((other.max_len < usize::max_value()) || (other.min_len > usize::min_value())
                      || other.unique || !other.in_vec.is_empty() || !other.nin_vec.is_empty()))
                    || (!self.array && 
                        (!other.items.is_empty() || other.extra_items.is_some()))
                    || (!self.contains_ok && !other.contains.is_empty()))
                {
                    Err(())
                }
                else {
                    // Get instersection of `in` vectors
                    let in_vec = if (self.in_vec.len() > 0) && (other.in_vec.len() > 0) {
                        sorted_intersection(&self.in_vec[..], &other.in_vec[..], |a,b| a.cmp(b))
                    }
                    else if self.in_vec.len() > 0 {
                        self.in_vec.clone()
                    }
                    else {
                        other.in_vec.clone()
                    };

                    // Get intersection of items
                    let items_len = self.items.len().max(other.items.len());
                    let mut items = Vec::with_capacity(items_len);
                    for i in 0..items_len {
                        let self_index = if let Some(index) = self.items.get(i) {
                            *index
                        }
                        else if let Some(index) = self.extra_items {
                            index
                        }
                        else {
                            1
                        };
                        let other_index = if let Some(index) = other.items.get(i) {
                            *index
                        }
                        else if let Some(index) = other.extra_items {
                            index
                        }
                        else {
                            1
                        };

                        let v = if self_index == 1 {
                            clone_validator(other_types, new_types, other_map, other_index)
                        }
                        else if other_index == 1 {
                            clone_validator(self_types, new_types, self_map, self_index)
                        }
                        else {
                            let v = self_types[self_index]
                                .intersect(&other_types[other_index], query, self_types, other_types, new_types, self_map, other_map)?;
                            add_validator(new_types, v)
                        };
                        items.push(v);
                    }

                    // Get extra items
                    let extra_items = if let (Some(self_extra), Some(other_extra)) = (self.extra_items,other.extra_items) {
                        let v = self_types[self_extra]
                            .intersect(&other_types[other_extra], query, self_types, other_types, new_types, self_map, other_map)?;
                        Some(add_validator(new_types, v))
                    }
                    else if let Some(extra_items) = self.extra_items {
                        Some(clone_validator(self_types, new_types, self_map, extra_items))
                    }
                    else if let Some(extra_items) = other.extra_items {
                        Some(clone_validator(other_types, new_types, other_map, extra_items))
                    }
                    else {
                        None
                    };

                    // Check that this isn't an invalid validator before proceeding
                    if items.contains(&0) {
                        new_types.truncate(new_types_len);
                        return Ok(Validator::Invalid);
                    }

                    let mut contains: Vec<usize> = Vec::with_capacity(self.contains.len() + other.contains.len());
                    contains.extend(self.contains.iter()
                        .map(|x| clone_validator(self_types, new_types, self_map, *x)));
                    contains.extend(other.contains.iter()
                        .map(|x| clone_validator(other_types, new_types, other_map, *x)));

                    // Create new Validator
                    let mut new_validator = ValidArray {
                        in_vec: in_vec,
                        nin_vec: sorted_union(&self.nin_vec[..], &other.nin_vec[..], |a,b| a.cmp(b)),
                        min_len: self.min_len.max(other.min_len),
                        max_len: self.max_len.min(other.max_len),
                        items: items,
                        extra_items: extra_items,
                        contains: contains,
                        unique: self.unique || other.unique,
                        query: self.query && other.query,
                        array: self.array && other.array,
                        contains_ok: self.contains_ok && other.contains_ok,
                    };
                    if new_validator.in_vec.len() == 0 && (self.in_vec.len()+other.in_vec.len() > 0) {
                        return Ok(Validator::Invalid);
                    }
                    let valid = new_validator.finalize();
                    if !valid {
                        new_types.truncate(new_types_len);
                        Ok(Validator::Invalid)
                    }
                    else {
                        Ok(Validator::Array(new_validator))
                    }
                }
            },
            Validator::Valid => Ok(Validator::Array(self.clone())),
            _ => Ok(Validator::Invalid),
        }
    }
}

fn get_raw_array(raw: &mut &[u8], len: usize) -> io::Result<Vec<u8>> {
    let start = raw.clone();
    for _ in 0..len {
        verify_value(raw)?;
    }
    let (array, _) = start.split_at(start.len()-raw.len());
    Ok(array.to_vec())
}



