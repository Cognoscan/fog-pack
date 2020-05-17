use std::io;
use std::io::Error;
use std::io::ErrorKind::InvalidData;
use std::collections::HashMap;
use std::cmp::Ordering;
use std::mem;
use Hash;

use decode::*;
use super::*;
use marker::MarkerType;

/// Object type validator
#[derive(Clone, Debug)]
pub struct ValidObj {
    in_vec: Vec<Box<[u8]>>,
    nin_vec: Vec<Box<[u8]>>,
    required: Vec<(String, usize)>,
    optional: Vec<(String, usize)>,
    banned: Vec<String>,
    min_fields: usize,
    max_fields: usize,
    field_type: Option<usize>,
    unknown_ok: bool,
    query: bool,
    schema_top: bool,
}

impl ValidObj {
    pub fn new(is_query: bool) -> Self {
        // For `unknown_ok`, default to no unknowns allowed, the only time we are not permissive by 
        // default in schema validators.
        ValidObj {
            in_vec: Vec::with_capacity(0),
            nin_vec: Vec::with_capacity(0),
            required: Vec::with_capacity(0),
            optional: Vec::with_capacity(0),
            banned: Vec::with_capacity(0),
            min_fields: usize::min_value(),
            max_fields: usize::max_value(),
            field_type: None,
            unknown_ok: is_query,
            query: is_query,
            schema_top: false,
        }
    }

    pub fn new_schema() -> ValidObj {
        let mut obj = Self::new(false);
        obj.schema_top = true;
        obj
    }


    /// Update the validator. Returns `Ok(true)` if everything is read out Ok, `Ok(false)` if we 
    /// don't recognize the field type or value, and `Err` if we recognize the field but fail to 
    /// parse the expected contents. The updated `raw` slice reference is only accurate if 
    /// `Ok(true)` was returned.
    pub fn update(
        &mut self,
        field: &str,
        raw: &mut &[u8],
        is_query: bool,
        types: &mut Vec<Validator>,
        type_names: &mut HashMap<String,usize>,
        schema_hash: &Hash
    )
        -> io::Result<bool>
    {
        // Note about this match: because fields are lexicographically ordered, the items in this 
        // match statement are either executed sequentially or are skipped.
        match field {
            "ban" => {
                match read_marker(&mut raw.clone())? {
                    MarkerType::String(len) => {
                        let s = read_raw_str(raw, len)?;
                        if self.schema_top && (s == "") {
                            return Err(Error::new(InvalidData, "Schema top `ban` cannot have empty string"));
                        }
                        self.banned.reserve_exact(1);
                        self.banned.push(s.to_string());
                        Ok(true)
                    },
                    MarkerType::Array(len) => {
                        self.banned.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _ in 0..len {
                            let s = read_string(raw)?;
                            if self.schema_top && (s == "") {
                                return Err(Error::new(InvalidData, "Schema top `ban` cannot have empty string in array"));
                            }
                            self.banned.push(s);
                        };
                        self.banned.sort_unstable();
                        self.banned.dedup();
                        Ok(true)
                    },
                    _ => {
                        Err(Error::new(InvalidData, "`ban` field must contain string or array of strings"))
                    }
                }
            },
            "default" => {
                if let MarkerType::Object(len) = read_marker(raw)? {
                    verify_map(raw, len)?;
                    Ok(true)
                }
                else {
                    Err(Error::new(InvalidData, "Object `default` isn't a valid object"))
                }
            },
            "field_type" => {
                self.field_type = Some(Validator::read_validator(raw, is_query, types, type_names, schema_hash)?);
                Ok(true)
            }
            "in" => {
                match read_marker(&mut raw.clone())? {
                    MarkerType::Object(_) => {
                        let v = get_obj(raw)?;
                        self.in_vec.reserve_exact(1);
                        self.in_vec.push(v);
                    },
                    MarkerType::Array(len) => {
                        self.in_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _ in 0..len {
                            let v = get_obj(raw)?;
                            self.in_vec.push(v);
                        }
                        self.in_vec.sort_unstable();
                        self.in_vec.dedup();
                    },
                    _ => {
                        return Err(Error::new(InvalidData, "Object validator expected array or constant for `in` field"));
                    }
                }
                Ok(true)
            },
            "max_fields" => {
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.max_fields = len as usize;
                    Ok(true)
                }
                else {
                    Err(Error::new(InvalidData, "Object validator expected non-negative value for `max_fields` field"))
                }
            },
            "min_fields" => {
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.min_fields = len as usize;
                    Ok(self.max_fields >= self.min_fields)
                }
                else {
                    Err(Error::new(InvalidData, "Object validator expected non-negative value for `min_fields` field"))
                }
            },
            "nin" => {
                match read_marker(&mut raw.clone())? {
                    MarkerType::Object(_) => {
                        let v = get_obj(raw)?;
                        self.nin_vec.reserve_exact(1);
                        self.nin_vec.push(v);
                    },
                    MarkerType::Array(len) => {
                        self.nin_vec.reserve_exact(len.min(MAX_VEC_RESERVE));
                        for _ in 0..len {
                            let v = get_obj(raw)?;
                            self.nin_vec.push(v);
                        }
                        self.nin_vec.sort_unstable();
                        self.nin_vec.dedup();
                    },
                    _ => {
                        return Err(Error::new(InvalidData, "Object validator expected array or constant for `nin` field"));
                    }
                }
                Ok(true)
            },
            "opt" => {
                let mut valid = true;
                if let MarkerType::Object(len) = read_marker(raw)? {
                    object_iterate(raw, len, |field, raw| {
                        if self.schema_top && (field == "") {
                            return Err(Error::new(InvalidData, "Schema top `opt` cannot have empty string field"));
                        }
                        let v = Validator::read_validator(raw, is_query, types, type_names, schema_hash)?;
                        if v == 0 { valid = false; }
                        self.optional.push((field.to_string(), v));
                        Ok(())
                    })?;
                    Ok(valid)
                }
                else {
                    Err(Error::new(InvalidData, "`opt` field must contain an object."))
                }
            }
            "query" => {
                self.query = read_bool(raw)?;
                Ok(true)
            },
            "req" => {
                let mut valid = true;
                if let MarkerType::Object(len) = read_marker(raw)? {
                    object_iterate(raw, len, |field, raw| {
                        if self.schema_top && (field == "") {
                            return Err(Error::new(InvalidData, "Schema top `req` cannot have empty string field"));
                        }
                        let v = Validator::read_validator(raw, is_query, types, type_names, schema_hash)?;
                        if v == 0 { valid = false; }
                        self.required.push((field.to_string(), v));
                        Ok(())
                    })?;
                    Ok(valid)
                }
                else {
                    Err(Error::new(InvalidData, "`req` field must contain an object."))
                }
            }
            "type" => if "Obj" == read_str(raw)? { Ok(true) } else { Err(Error::new(InvalidData, "Type doesn't match Obj")) },
            "unknown_ok" => {
                self.unknown_ok = read_bool(raw)?;
                Ok(true)
            },
            _ => Err(Error::new(InvalidData, "Unknown fields not allowed in Array validator")),
        }
    }

    /// Final check on the validator. Returns true if at least one value can (probably) still pass the 
    /// validator. We do not check the `in` and `nin` against all validation parts
    pub fn finalize(&mut self) -> bool {
        // There's probably a better way to satisfy the borrow checker, but temporarily pulling the 
        // `optional` Vec out, operating on it, and putting it back in works OK for now.
        let mut optional = Vec::with_capacity(0);
        mem::swap(&mut self.optional, &mut optional);
        optional.retain(|x| self.required.binary_search_by(|y| y.0.cmp(&x.0)).is_err());
        mem::swap(&mut self.optional, &mut optional);
        (self.min_fields <= self.max_fields) && !self.required.iter().any(|x| x.1 == 0)
    }

    /// Validates that the next value is a Hash that meets the validator requirements. Fails if the 
    /// requirements are not met. If it passes, the optional returned Hash indicates that an 
    /// additional document (referenced by the Hash) needs to be checked.
    pub fn validate(&self,
                    field: &str,
                    doc: &mut &[u8],
                    types: &Vec<Validator>,
                    list: &mut ValidatorChecklist,
                    top_schema: bool
                    ) -> io::Result<()>
    {
        let obj_start = doc.clone();
        let mut num_fields = match read_marker(doc)? {
            MarkerType::Object(len) => len,
            _ => return Err(Error::new(InvalidData, "Object not found")),
        };

        // Read out the schema field if this is a Document, and don't count it towards the field 
        // limit
        if top_schema {
            let mut schema = &doc[..];
            if read_str(&mut schema)?.len() == 0 {
                if read_hash(&mut schema).is_err() {
                    return Err(Error::new(InvalidData, "Document schema field doesn't contain a Hash"));
                }
                else {
                    *doc = schema;
                    num_fields -= 1;
                }
            }
        }

        if num_fields < self.min_fields {
            return Err(Error::new(InvalidData,
                format!("Field \"{}\" contains object with {} fields, less than the {} required",
                    field, num_fields, self.min_fields)));
        }
        if num_fields == 0 && self.required.len() == 0 { return Ok(()); }
        if num_fields > self.max_fields {
            return Err(Error::new(InvalidData,
                format!("Field \"{}\" contains object with {} fields, more than the {} required",
                    field, num_fields, self.max_fields)));
        }

        // Setup for loop
        let parent_field = field;
        let mut req_index = 0;
        object_iterate(doc, num_fields, |field, doc| {
            // Check against required/optional/unknown types
            if self.banned.binary_search_by(|probe| (**probe).cmp(field)).is_ok() {
                Err(Error::new(InvalidData, "Field \"{}\" is banned"))
            }
            else if Some(field) == self.required.get(req_index).map(|x| x.0.as_str()) {
                let v_index = self.required[req_index].1;
                req_index += 1;
                types[v_index].validate(field, doc, types, v_index, list)
            }
            else if let Ok(opt_index) = self.optional.binary_search_by(|probe| (probe.0).as_str().cmp(field)) {
                let v_index = self.optional[opt_index].1;
                types[v_index].validate(field, doc, types, v_index, list)
            }
            else if self.unknown_ok {
                if let Some(v_index) = self.field_type {
                    types[v_index].validate(field, doc, types, v_index, list)
                }
                else {
                    verify_value(doc)?;
                    Ok(())
                }
            }
            else {
                if self.required.binary_search_by(|probe| (probe.0).as_str().cmp(field)).is_ok() {
                    Err(Error::new(InvalidData, format!("Missing required fields before \"{}\"", field)))
                }
                else {
                    Err(Error::new(InvalidData, format!("Unknown, invalid field: \"{}\"", field)))
                }
            }
        })?;

        let (obj_start, _) = obj_start.split_at(obj_start.len()-doc.len());
        if self.nin_vec.iter().any(|x| obj_start == &x[..]) {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" contains object on the `nin` list", parent_field)))
        }
        else if (self.in_vec.len() > 0) && !self.in_vec.iter().any(|x| obj_start == &x[..]) {
            Err(Error::new(InvalidData,
                format!("Field \"{}\" contains object not in the `in` list", parent_field)))
        }
        else if req_index < self.required.len() {
            Err(Error::new(InvalidData,
                format!("Missing required fields, starting with {}", self.required[req_index].0.as_str())))
        }
        else {
            Ok(())
        }
    }

    /// Intersection of Object with other Validators. Returns Err only if `query` is true and the 
    /// other validator contains non-allowed query parameters.
    pub fn intersect(&self,
                 other: &Validator,
                 query: bool,
                 builder: &mut ValidBuilder
                 )
        -> Result<Validator, ()>
    {
        let builder_len = builder.len();
        if query && !self.query { return Err(()); }
        match other {
            Validator::Object(other) => {
                // Get intersection of `in` vectors
                let in_vec = if (self.in_vec.len() > 0) && (other.in_vec.len() > 0) {
                    sorted_intersection(&self.in_vec[..], &other.in_vec[..], |a,b| a.cmp(b))
                }
                else if self.in_vec.len() > 0 {
                    self.in_vec.clone()
                }
                else {
                    other.in_vec.clone()
                };

                // Get intersection of required & optional
                let mut required: Vec<(String, usize)> = Vec::new();
                let mut optional: Vec<(String, usize)> = Vec::new();
                let mut self_req_i = 0;
                let mut self_opt_i = 0;
                let mut other_req_i = 0;
                let mut other_opt_i = 0;
                let self_type = ("".to_string(), if self.unknown_ok {
                    if let Some(self_type) = self.field_type {
                        self_type
                    }
                    else {
                        VALID
                    }
                }
                else {
                    INVALID
                });
                let other_type = ("".to_string(), if self.unknown_ok {
                    if let Some(other_type) = other.field_type {
                        other_type
                    }
                    else {
                        VALID
                    }
                }
                else {
                    INVALID
                });
                while (other_req_i < other.required.len()) || (other_opt_i < other.optional.len())
                    || (self_req_i < self.required.len()) || (self_opt_i < self.optional.len())
                {
                    let (s, s_is_req, s_is_opt) = match (self.required.get(self_req_i), self.optional.get(self_opt_i)) {
                        (Some(s_req), Some(s_opt)) => {
                            match s_req.0.cmp(&s_opt.0) {
                                Ordering::Less => (s_req, true, false),
                                Ordering::Equal => (s_req, true, true),
                                Ordering::Greater => (s_opt, false, true),
                            }
                        },
                        (Some(s_req), None) => (s_req, true, false),
                        (None, Some(s_opt)) => (s_opt, false, true),
                        (None, None) => (&self_type, false, false),
                    };
                    let (o, o_is_req, o_is_opt) = match (other.required.get(other_req_i), other.optional.get(other_opt_i)) {
                        (Some(o_req), Some(o_opt)) => {
                            match o_req.0.cmp(&o_opt.0) {
                                Ordering::Less => (o_req, true, false),
                                Ordering::Equal => (o_req, true, true),
                                Ordering::Greater => (o_opt, false, true),
                            }
                        },
                        (Some(o_req), None) => (o_req, true, false),
                        (None, Some(o_opt)) => (o_opt, false, true),
                        (None, None) => (&other_type, false, false),
                    };

                    // Rules:
                    // - If in req for both, intersect and use in req
                    // - If in req for one and opt for other, intersect and use in req
                    // - If in opt for both, intersect and use in opt
                    // - If in req for one and not in other, intersect with field_type and use in
                    //   req, but only if unknown_ok true in other
                    // - If in opt for one and not in other, intersect with field_type and use in 
                    //   opt, but only if unknown_ok true in other
                    if (s_is_req || s_is_opt) && (o_is_req || o_is_opt) {
                        // Detemine intersection & increment pointers
                        let v = match s.0.cmp(&o.0) {
                            Ordering::Less => {
                                if s_is_req { self_req_i += 1; }
                                if s_is_opt { self_opt_i += 1; }
                                builder.intersect(query, s.1, other_type.1)?
                            },
                            Ordering::Equal => {
                                if s_is_req { self_req_i += 1; }
                                if s_is_opt { self_opt_i += 1; }
                                if o_is_req { other_req_i += 1; }
                                if o_is_opt { other_opt_i += 1; }
                                builder.intersect(query, s.1, o.1)?
                            },
                            Ordering::Greater => {
                                if o_is_req { other_req_i += 1; }
                                if o_is_opt { other_opt_i += 1; }
                                builder.intersect(query, self_type.1, o.1)?
                            },
                        };
                        // Add to appropriate list
                        if s_is_req || o_is_req {
                            required.push((s.0.clone(), v));
                        }
                        else {
                            optional.push((s.0.clone(), v));
                        }
                    }
                    else if s_is_req || s_is_opt {
                        let v = builder.intersect(query, s.1, other_type.1)?;
                        if s_is_req { self_req_i += 1; }
                        if s_is_opt { self_opt_i += 1; }
                        if s_is_req {
                            required.push((s.0.clone(), v));
                        }
                        else {
                            optional.push((s.0.clone(), v));
                        }
                    }
                    else if o_is_req || o_is_opt {
                        let v = builder.intersect(query, self_type.1, o.1)?;
                        if o_is_req { other_req_i += 1; }
                        if o_is_opt { other_opt_i += 1; }
                        if o_is_req {
                            required.push((o.0.clone(), v));
                        }
                        else {
                            optional.push((o.0.clone(), v));
                        }
                    }
                }

                // Get extra items
                let field_type = if let (Some(self_type), Some(other_type)) = (self.field_type, other.field_type) {
                    Some(builder.intersect(query, self_type, other_type)?)
                }
                else if let Some(field_type) = self.field_type {
                    Some(builder.intersect(query, field_type, 1).unwrap())
                }
                else if let Some(field_type) = other.field_type {
                    Some(builder.intersect(query, 1, field_type).unwrap())
                }
                else {
                    None
                };

                // Check that this isn't an invalid validator before proceeding
                /*
                if items.contains(&0) {
                    builder.undo_to(builder_len);
                    return Ok(Validator::Invalid);
                }
                */

                // Create new Validator
                let mut new_validator = ValidObj {
                    in_vec: in_vec,
                    nin_vec: sorted_union(&self.nin_vec[..], &other.nin_vec[..], |a,b| a.cmp(b)),
                    required: required,
                    optional: optional,
                    banned: sorted_union(&self.banned[..], &other.banned[..], |a,b| a.cmp(b)),
                    min_fields: self.min_fields.max(other.min_fields),
                    max_fields: self.max_fields.min(other.max_fields),
                    field_type: field_type,
                    unknown_ok: self.unknown_ok && other.unknown_ok,
                    query: self.query && other.query,
                    schema_top: false, // Only used during initial creation
                };
                if new_validator.in_vec.len() == 0 && (self.in_vec.len()+other.in_vec.len() > 0) {
                    builder.undo_to(builder_len);
                    return Ok(Validator::Invalid);
                }
                let valid = new_validator.finalize();
                if !valid {
                    builder.undo_to(builder_len);
                    Ok(Validator::Invalid)
                }
                else {
                    Ok(Validator::Object(new_validator))
                }
            },
            Validator::Valid => {
                let mut v = self.clone();
                let mut required = Vec::with_capacity(self.required.len());
                required.extend(self.required.iter()
                    .map(|x| (x.0.clone(), builder.intersect(query, x.1, 1).unwrap())));
                v.required = required;

                let mut optional = Vec::with_capacity(self.optional.len());
                optional.extend(self.optional.iter()
                    .map(|x| (x.0.clone(), builder.intersect(query, x.1, 1).unwrap())));
                v.optional = optional;

                if let Some(field_type) = self.field_type {
                    v.field_type = Some(builder.intersect(query, field_type, 1).unwrap());
                }
                Ok(Validator::Object(v))
            }
            _ => Ok(Validator::Invalid),
        }
    }
}

fn get_obj(raw: &mut &[u8]) -> io::Result<Box<[u8]>> {
    let start = raw.clone();
    if let MarkerType::Object(len) = read_marker(raw)? {
        verify_map(raw, len)?;
    }
    else {
        return Err(Error::new(InvalidData, "Expected objects in `in`/`nin` fields"));
    }
    let (obj, _) = start.split_at(start.len()-raw.len());
    Ok(obj.to_vec().into_boxed_slice())
}


#[cfg(test)]
mod tests {
    use encode;
    use value::Value;
    use crypto::Hash;
    use timestamp::Timestamp;
    use super::*;

    fn read_it(raw: &mut &[u8], is_query: bool) -> (usize, Vec<Validator>) {
        let mut types = Vec::new();
        types.push(Validator::Invalid);
        types.push(Validator::Valid);
        let mut type_names = HashMap::new();
        let schema_hash = Hash::new(b"test");
        let validator = Validator::read_validator(&mut &raw[..], is_query, &mut types, &mut type_names, &schema_hash).unwrap();
        for (i, v) in types.iter().enumerate() {
            println!("{}: {:?}", i, v);
        }
        match types[validator] {
            Validator::Object(_) => (),
            _ => panic!("Parsing an object validator didn't yield an object validator!"),
        }
        (validator, types)
    }

    #[test]
    fn simple_obj() {
        let schema: Value = fogpack!({
            "type": "Obj",
            "req": {
                "title": { "type": "Str", "max_len": 200 },
                "text": { "type": "Str" }
            },
        });
        let mut raw_schema = Vec::new();
        encode::write_value(&mut raw_schema, &schema);

        let doc: Value = fogpack!({
            "title": "A Test",
            "text": "This is a test of a schema document"
        });
        let mut raw_doc = Vec::new();
        encode::write_value(&mut raw_doc, &doc);
        
        let (validator, types) = read_it(&mut &raw_schema[..], false);
        let mut list = ValidatorChecklist::new();
        types[validator].validate("", &mut &raw_doc[..], &types, validator, &mut list).unwrap();
    }

    #[test]
    fn basic_tests() {
        let now = Timestamp::now().unwrap();
        let mut raw_schema = Vec::new();
        let schema: Value = fogpack!({
            "type": "Obj",
            "req": {
                "test": true
            },
            "opt": {
                "boolean": true,
                "positive": 1,
                "negative": -1,
                "string": "string",
                "float32": 1.0f32,
                "float64": 1.0f64,
                "binary": vec![0u8,1u8,2u8],
                "hash": Hash::new_empty(),
                "timestamp": now,
                "array": [Value::from(0), Value::from("an_array")] 
            }
        });
        encode::write_value(&mut raw_schema, &schema);
        println!("Schema = {}", &schema);

        let (validator, types) = read_it(&mut &raw_schema[..], false);

        // Should pass with all fields
        let mut raw_test = Vec::new();
        let test: Value = fogpack!({
            "test": true,
            "boolean": true,
            "positive": 1,
            "negative": -1,
            "string": "string",
            "float32": 1.0f32,
            "float64": 1.0f64,
            "binary": vec![0u8,1u8,2u8],
            "hash": Hash::new_empty(),
            "timestamp": now,
            "array": [Value::from(0), Value::from("an_array")] 
        });
        encode::write_value(&mut raw_test, &test);
        let mut list = ValidatorChecklist::new();
        assert!(types[validator].validate("", &mut &raw_test[..], &types, validator, &mut list).is_ok());

        // Should pass with only required fields
        raw_test.clear();
        let test: Value = fogpack!({
            "test": true,
        });
        encode::write_value(&mut raw_test, &test);
        let mut list = ValidatorChecklist::new();
        assert!(types[validator].validate("", &mut &raw_test[..], &types, validator, &mut list).is_ok());

        // Should fail if we remove one of the required fields
        raw_test.clear();
        let test: Value = fogpack!({
            "boolean": true,
            "positive": 1,
            "negative": -1,
            "string": "string",
            "float32": 1.0f32,
            "float64": 1.0f64,
            "binary": vec![0u8,1u8,2u8],
            "hash": Hash::new_empty(),
            "timestamp": now,
            "array": [Value::from(0), Value::from("an_array")] 
        });
        encode::write_value(&mut raw_test, &test);
        let mut list = ValidatorChecklist::new();
        assert!(types[validator].validate("", &mut &raw_test[..], &types, validator, &mut list).is_err());
    }
}
