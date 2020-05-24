use std::collections::HashSet;

use Error;
use decode::*;
use super::*;
use marker::MarkerType;

#[derive(Clone, Debug)]
pub struct ValidArray {
    /// Raw fogpack to compare against
    in_vec: Vec<Box<[u8]>>,
    nin_vec: Vec<Box<[u8]>>,
    min_len: usize,
    max_len: usize,
    items: Vec<usize>,
    extra_items: Option<usize>,
    contains: Vec<usize>,
    unique: bool,
    query: bool,
    array: bool,
    size: bool,
    unique_ok: bool,
    contains_ok: bool,
    query_used: bool,
    array_used: bool,
    unique_used: bool,
    contains_used: bool,
    size_used: bool,
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
            size: is_query,
            contains_ok: is_query,
            unique_ok: is_query,
            query_used: false,
            array_used: false,
            unique_used: false,
            contains_used: false,
            size_used: false,
        }
    }

    pub fn from_const(constant: Box<[u8]>, is_query: bool) -> ValidArray {
        let mut v = ValidArray::new(is_query);
        let mut in_vec = Vec::with_capacity(1);
        in_vec.push(constant);
        v.in_vec = in_vec;
        v
    }

    /// Update the validator. Returns `Ok(true)` if everything is read out Ok, `Ok(false)` if we 
    /// don't recognize the field type or value, and `Err` if we recognize the field but fail to 
    /// parse the expected contents. The updated `raw` slice reference is only accurate if 
    /// `Ok(true)` was returned.
    pub fn update(&mut self, field: &str, raw: &mut &[u8], reader: &mut ValidReader)
        -> crate::Result<bool>
    {
        let fail_len = raw.len();
        // Note about this match: because fields are lexicographically ordered, the items in this 
        // match statement are either executed sequentially or are skipped.
        match field {
            "array" => {
                self.array = read_bool(raw)?;
                Ok(true)
            }
            "contains" => {
                self.contains_used = true;
                if let MarkerType::Array(len) = read_marker(raw)? {
                    for _ in 0..len {
                        let v = Validator::read_validator(raw, reader)?;
                        self.contains.push(v);
                    }
                    Ok(true)
                }
                else {
                    Err(Error::FailValidate(fail_len, "Array `contains` isn't a valid array of validators"))
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
                    Err(Error::FailValidate(fail_len, "Array `default` isn't a valid array"))
                }
            },
            "extra_items" => {
                self.array_used = true;
                self.extra_items = Some(Validator::read_validator(raw, reader)?);
                Ok(true)
            },
            "in" => {
                self.query_used = true;
                if let MarkerType::Array(len) = read_marker(raw)? {
                    // Push without reserving - otherwise recursive reserving is possible and 
                    // can lead to an exponential amount of memory reservation.
                    for _ in 0..len {
                        let v = if let MarkerType::Array(len) = read_marker(raw)? {
                            get_raw_array(raw, len)?
                        }
                        else {
                            return Err(Error::FailValidate(fail_len, "Array validator expected array of arrays for `in` field"));
                        };
                        self.in_vec.push(v);
                    };
                    self.in_vec.sort_unstable();
                    self.in_vec.dedup();
                }
                else {
                    return Err(Error::FailValidate(fail_len, "Array validator expected array of arrays for `in` field"));
                }
                Ok(true)
            },
            "items" => {
                self.array_used = true;
                if let MarkerType::Array(len) = read_marker(raw)? {
                    for _ in 0..len {
                        let v = Validator::read_validator(raw, reader)?;
                        self.items.push(v);
                    }
                    Ok(true)
                }
                else {
                    Err(Error::FailValidate(fail_len, "Array `items` isn't a valid array of validators"))
                }
            },
            "max_len" => {
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.max_len = len as usize;
                    Ok(true)
                }
                else {
                    Err(Error::FailValidate(fail_len, "Array validator requires non-negative integer for `max_len` field"))
                }
            },
            "min_len" => {
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.min_len = len as usize;
                    Ok(self.max_len >= self.min_len)
                }
                else {
                    Err(Error::FailValidate(fail_len, "Array validator requires non-negative integer for `min_len` field"))
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
                            return Err(Error::FailValidate(fail_len, "Array validator expected array of arrays for `in` field"));
                        };
                        self.nin_vec.push(v);
                    };
                    self.nin_vec.sort_unstable();
                    self.nin_vec.dedup();
                }
                else {
                    return Err(Error::FailValidate(fail_len, "Array validator expected array of arrays for `in` field"));
                }
                Ok(true)
            },
            "query" => {
                self.query = read_bool(raw)?;
                Ok(true)
            },
            "size" => {
                self.size = read_bool(raw)?;
                Ok(true)
            },
            "unique" => {
                self.unique_used = true;
                self.unique = read_bool(raw)?;
                Ok(true)
            },
            "unique_ok" => {
                self.unique_ok = read_bool(raw)?;
                Ok(true)
            },
            "type" => if "Array" == read_str(raw)? { Ok(true) } else { Err(Error::FailValidate(fail_len, "Type doesn't match Array")) },
            _ => Err(Error::FailValidate(fail_len, "Unknown fields not allowed in Array validator")),
        }
    }

    /// Final check on the validator. Returns true if at least one value can (probably) still pass the 
    /// validator. We do not check the `in` and `nin` against all validation parts
    pub fn finalize(&mut self) -> bool {
        if self.in_vec.len() > 0 {
            let mut in_vec: Vec<Box<[u8]>> = Vec::with_capacity(self.in_vec.len());
            let mut nin_index = 0;
            for val in self.in_vec.iter() {
                while let Some(nin) = self.nin_vec.get(nin_index) {
                    if nin < val { nin_index += 1; } else { break; }
                }
                if let Some(nin) = self.nin_vec.get(nin_index) {
                    if nin == val { continue; }
                }
                in_vec.push(val.clone());
            }
            in_vec.shrink_to_fit();
            self.in_vec = in_vec;
            self.nin_vec = Vec::with_capacity(0);
            (self.in_vec.len() > 0) && (self.min_len <= self.max_len)
        }
        else {
            self.nin_vec.shrink_to_fit();
            self.min_len <= self.max_len
        }
    }

    /// Validates that the next value is a Hash that meets the validator requirements. Fails if the 
    /// requirements are not met. If it passes, the optional returned Hash indicates that an 
    /// additional document (referenced by the Hash) needs to be checked.
    pub fn validate(&self, doc: &mut &[u8], types: &Vec<Validator>, list: &mut ValidatorChecklist) -> crate::Result<()>
    {
        let fail_len = doc.len();
        let num_items = match read_marker(doc)? {
            MarkerType::Array(len) => len,
            _ => return Err(Error::FailValidate(fail_len, "Expected array")),
        };
        if num_items == 0 && self.min_len == 0 && self.items.len() == 0 && self.contains.len() == 0 {
            return Ok(());
        }

        let array_start = doc.clone();

        // Size checks
        if num_items < self.min_len {
            return Err(Error::FailValidate(fail_len, "Array has fewer than minimum number of items allowed"))
        }
        if num_items > self.max_len {
            return Err(Error::FailValidate(fail_len, "Array has greater than maximum number of items allowed"))
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
                if let Err(e) = types[*v_index].validate(doc, types, *v_index, list) {
                    return Err(e);
                }
            }
            else if let Some(v_index) = self.extra_items {
                if let Err(e) = types[v_index].validate(doc, types, v_index, list) {
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
                    return Err(Error::FailValidate(fail_len, "Array contains a repeated item"));
                }
            }
            // Check to see if any `contains` requirements are met
            contain_set.iter_mut()
                .zip(self.contains.iter())
                .filter(|(checked,_)| !**checked)
                .for_each(|(checked,contains_item)| {
                    if let Ok(()) = types[*contains_item].validate(&mut item.clone(), types, *contains_item, list) {
                        *checked = true;
                    }
                });
        }

        let (array, _) = array_start.split_at(array_start.len()-doc.len());
        if contain_set.contains(&false) {
            Err(Error::FailValidate(fail_len, "Array does not satisfy all `contains` requirements"))
        }
        else if self.nin_vec.binary_search_by(|probe| (**probe).cmp(array)).is_ok() {
            Err(Error::FailValidate(fail_len, "Array is on `nin` list"))
        }
        else if (self.in_vec.len() > 0) && self.in_vec.binary_search_by(|probe| (**probe).cmp(array)).is_err() {
            Err(Error::FailValidate(fail_len, "Array is not on `in` list"))
        }
        else {
            Ok(())
        }
    }

    /// Verify the query is allowed to proceed. It can only proceed if the query type matches or is 
    /// a general Valid.
    pub fn query_check(&self, other: &Validator, s_types: &[Validator], o_types: &[Validator]) -> bool {
        match other {
            Validator::Array(other) => {
                if (self.query || !other.query_used)
                    && (self.size || !other.size_used)
                    && (self.unique_ok || !other.unique_used)
                    && (self.array || !other.array_used)
                    && (self.contains_ok || !other.contains_used)
                {
                    // Check the extra_items
                    if let Some(s_extra) = self.extra_items {
                        if let Some(o_extra) = other.extra_items {
                            if !query_check(s_extra, o_extra, s_types, o_types) { return false; }
                        }
                    }

                    // Prepare the iterators to go over the items lists
                    let s_extra = self.extra_items.unwrap_or(VALID);
                    let o_extra = other.extra_items.unwrap_or(VALID);
                    let s_iter = self.items.iter().chain(std::iter::repeat(&s_extra));
                    let o_iter = other.items.iter().chain(std::iter::repeat(&o_extra));

                    // Go over entire self.items list, and if self.extra_items exists, make sure to 
                    // validate all the other.items as well if they are longer than self.items.
                    let take = if self.extra_items.is_some() {
                        (self.items.len()).max(other.items.len())
                    }
                    else {
                        self.items.len()
                    };

                    let items_eval = s_iter
                        .zip(o_iter)
                        .take(take)
                        .all(|(&s, &o)| {
                            query_check(s, o, s_types, o_types)
                        });
                    if !items_eval { return false; }

                    // Go over entire other.contains against each and every self.items, plus 
                    // self.extra_items
                    if let Some(s) = self.extra_items {
                        for contain in other.contains.iter() {
                            if !query_check(s, *contain, s_types, o_types) { return false; }
                        }
                    }
                    for contain in other.contains.iter() {
                        for item in self.items.iter() {
                            if !query_check(*item, *contain, s_types, o_types) { return false; }
                        }
                    }

                    true
                }
                else {
                    false
                }
            }
            Validator::Valid => true,
            _ => false,
        }
    }
}

pub fn get_raw_array(raw: &mut &[u8], len: usize) -> crate::Result<Box<[u8]>> {
    let start = raw.clone();
    for _ in 0..len {
        verify_value(raw)?;
    }
    let (array, _) = start.split_at(start.len()-raw.len());
    Ok(array.to_vec().into_boxed_slice())
}



