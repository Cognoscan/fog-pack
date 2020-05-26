use Error;
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
    size: bool,
    obj_ok: bool,
    query_used: bool,
    size_used: bool,
    obj_used: bool,
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
            unknown_ok: false,
            query: is_query,
            size: is_query,
            obj_ok: is_query,
            query_used: false,
            size_used: false,
            obj_used: false,
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
    pub fn update( &mut self, field: &str, raw: &mut &[u8], reader: &mut ValidReader) -> crate::Result<bool>
    {
        let fail_len = raw.len();
        // Note about this match: because fields are lexicographically ordered, the items in this 
        // match statement are either executed sequentially or are skipped.
        match field {
            "ban" => {
                self.obj_used = true;
                match read_marker(&mut raw.clone())? {
                    MarkerType::String(len) => {
                        let s = read_raw_str(raw, len)?;
                        if self.schema_top && (s == "") {
                            return Err(Error::FailValidate(fail_len, "Schema top `ban` cannot have empty string"));
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
                                return Err(Error::FailValidate(fail_len, "Schema top `ban` cannot have empty string in array"));
                            }
                            self.banned.push(s);
                        };
                        self.banned.sort_unstable();
                        self.banned.dedup();
                        Ok(true)
                    },
                    _ => {
                        Err(Error::FailValidate(fail_len, "`ban` field must contain string or array of strings"))
                    }
                }
            },
            "default" => {
                if let MarkerType::Object(len) = read_marker(raw)? {
                    verify_map(raw, len)?;
                    Ok(true)
                }
                else {
                    Err(Error::FailValidate(fail_len, "Object `default` isn't a valid object"))
                }
            },
            "field_type" => {
                self.obj_used = true;
                self.field_type = Some(Validator::read_validator(raw, reader)?);
                Ok(true)
            }
            "in" => {
                self.query_used = true;
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
                        return Err(Error::FailValidate(fail_len, "Object validator expected array or constant for `in` field"));
                    }
                }
                Ok(true)
            },
            "max_fields" => {
                self.size_used = true;
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.max_fields = len as usize;
                    Ok(true)
                }
                else {
                    Err(Error::FailValidate(fail_len, "Object validator expected non-negative value for `max_fields` field"))
                }
            },
            "min_fields" => {
                self.size_used = true;
                if let Some(len) = read_integer(raw)?.as_u64() {
                    self.min_fields = len as usize;
                    Ok(self.max_fields >= self.min_fields)
                }
                else {
                    Err(Error::FailValidate(fail_len, "Object validator expected non-negative value for `min_fields` field"))
                }
            },
            "nin" => {
                self.query_used = true;
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
                        return Err(Error::FailValidate(fail_len, "Object validator expected array or constant for `nin` field"));
                    }
                }
                Ok(true)
            },
            "obj_ok" => {
                self.obj_ok = read_bool(raw)?;
                Ok(true)
            },
            "opt" => {
                self.obj_used = true;
                let mut valid = true;
                if let MarkerType::Object(len) = read_marker(raw)? {
                    object_iterate(raw, len, |field, raw| {
                        if self.schema_top && (field == "") {
                            return Err(Error::FailValidate(fail_len, "Schema top `opt` cannot have empty string field"));
                        }
                        let v = Validator::read_validator(raw, reader)?;
                        if v == 0 { valid = false; }
                        self.optional.push((field.to_string(), v));
                        Ok(())
                    })?;
                    Ok(valid)
                }
                else {
                    Err(Error::FailValidate(fail_len, "`opt` field must contain an object."))
                }
            }
            "query" => {
                self.query = read_bool(raw)?;
                Ok(true)
            },
            "req" => {
                self.obj_used = true;
                let mut valid = true;
                if let MarkerType::Object(len) = read_marker(raw)? {
                    object_iterate(raw, len, |field, raw| {
                        if self.schema_top && (field == "") {
                            return Err(Error::FailValidate(fail_len, "Schema top `req` cannot have empty string field"));
                        }
                        let v = Validator::read_validator(raw, reader)?;
                        if v == 0 { valid = false; }
                        self.required.push((field.to_string(), v));
                        Ok(())
                    })?;
                    Ok(valid)
                }
                else {
                    Err(Error::FailValidate(fail_len, "`req` field must contain an object."))
                }
            }
            "size" => {
                self.size = read_bool(raw)?;
                Ok(true)
            },
            "type" => if "Obj" == read_str(raw)? { Ok(true) } else { Err(Error::FailValidate(fail_len, "Type doesn't match Obj")) },
            "unknown_ok" => {
                self.obj_used = true;
                self.unknown_ok = read_bool(raw)?;
                Ok(true)
            },
            _ => Err(Error::FailValidate(fail_len, "Unknown fields not allowed in Object validator")),
        }
    }

    /// Final check on the validator. Returns true if at least one value can (probably) still pass the 
    /// validator. We do not check the `in` and `nin` against all validation parts
    pub fn finalize(&mut self) -> bool {
        let optional = &mut self.optional;
        let required = &mut self.required;
        optional.retain(|x| required.binary_search_by(|y| y.0.cmp(&x.0)).is_err());
        (self.min_fields <= self.max_fields) && !self.required.iter().any(|x| x.1 == 0)
    }

    /// Validates that the next value is a Hash that meets the validator requirements. Fails if the 
    /// requirements are not met. If it passes, the optional returned Hash indicates that an 
    /// additional document (referenced by the Hash) needs to be checked.
    pub fn validate(&self,
                    doc: &mut &[u8],
                    types: &[Validator],
                    list: &mut ValidatorChecklist,
                    top_schema: bool
                    ) -> crate::Result<()>
    {
        let fail_len = doc.len();
        let obj_start = doc.clone();
        let mut num_fields = match read_marker(doc)? {
            MarkerType::Object(len) => len,
            _ => return Err(Error::FailValidate(fail_len, "Expected object")),
        };

        // Read out the schema field if this is a Document, and don't count it towards the field 
        // limit
        if top_schema {
            let mut schema = &doc[..];
            if read_str(&mut schema)?.len() == 0 {
                if read_hash(&mut schema).is_err() {
                    return Err(Error::FailValidate(fail_len, "Document schema field doesn't contain a Hash"));
                }
                else {
                    *doc = schema;
                    num_fields -= 1;
                }
            }
        }

        if num_fields < self.min_fields {
            return Err(Error::FailValidate(fail_len, "Object has fewer fields than allowed"));
        }
        if num_fields == 0 && self.required.len() == 0 { return Ok(()); }
        if num_fields > self.max_fields {
            return Err(Error::FailValidate(fail_len, "Object has more fields than allowed"));
        }

        // Setup for loop
        let mut req_index = 0;
        object_iterate(doc, num_fields, |field, doc| {
            // Check against required/optional/unknown types
            if self.banned.binary_search_by(|probe| (**probe).cmp(field)).is_ok() {
                Err(Error::FailValidate(fail_len, "Banned field present"))
            }
            else if Some(field) == self.required.get(req_index).map(|x| x.0.as_str()) {
                let v_index = self.required[req_index].1;
                req_index += 1;
                types[v_index].validate(doc, types, v_index, list)
            }
            else if let Ok(opt_index) = self.optional.binary_search_by(|probe| (probe.0).as_str().cmp(field)) {
                let v_index = self.optional[opt_index].1;
                types[v_index].validate(doc, types, v_index, list)
            }
            else if self.unknown_ok {
                if let Some(v_index) = self.field_type {
                    types[v_index].validate(doc, types, v_index, list)
                }
                else {
                    verify_value(doc)?;
                    Ok(())
                }
            }
            else {
                if self.required.binary_search_by(|probe| (probe.0).as_str().cmp(field)).is_ok() {
                    Err(Error::FailValidate(fail_len, "Missing required fields before this"))
                }
                else {
                    Err(Error::FailValidate(fail_len, "Unknown, invalid field in object"))
                }
            }
        })?;

        let (obj_start, _) = obj_start.split_at(obj_start.len()-doc.len());
        if self.nin_vec.iter().any(|x| obj_start == &x[..]) {
            Err(Error::FailValidate(fail_len, "Object in object `nin` list is present"))
        }
        else if (self.in_vec.len() > 0) && !self.in_vec.iter().any(|x| obj_start == &x[..]) {
            Err(Error::FailValidate(fail_len, "Object not in object `in` list is present"))
        }
        else if req_index < self.required.len() {
            Err(Error::FailValidate(fail_len, "Missing required fields"))
        }
        else {
            Ok(())
        }
    }

    /// Verify the query is allowed to proceed. It can only proceed if the query type matches or is 
    /// a general Valid.
    pub fn query_check(&self, other: &Validator, s_types: &[Validator], o_types: &[Validator]) -> bool {
        match other {
            Validator::Object(other) => {
                if (self.query || !other.query_used)
                    && (self.size || !other.size_used)
                    && (self.obj_ok || !other.obj_used)
                {
                    let mut req_list = Vec::with_capacity(self.required.len());
                    req_list.resize(self.required.len(), false);
                    let mut opt_list = Vec::with_capacity(self.required.len());
                    opt_list.resize(self.required.len(), false);
                    // Check required fields
                    for o_val in other.required.iter() {
                        if let Ok(s_index) = self.required.binary_search_by(|probe| (probe.0).as_str().cmp(&o_val.0)) {
                            req_list[s_index] = true;
                            if !query_check(self.required[s_index].1, o_val.1, s_types, o_types) {
                                return false;
                            }
                        }
                        else if let Ok(s_index) = self.optional.binary_search_by(|probe| (probe.0).as_str().cmp(&o_val.0)) {
                            opt_list[s_index] = true;
                            if !query_check(self.optional[s_index].1, o_val.1, s_types, o_types) {
                                return false;
                            }
                        }
                    }
                    // Check optional fields
                    for o_val in other.optional.iter() {
                        if let Ok(s_index) = self.required.binary_search_by(|probe| (probe.0).as_str().cmp(&o_val.0)) {
                            req_list[s_index] = true;
                            if !query_check(self.required[s_index].1, o_val.1, s_types, o_types) {
                                return false;
                            }
                        }
                        else if let Ok(s_index) = self.optional.binary_search_by(|probe| (probe.0).as_str().cmp(&o_val.0)) {
                            opt_list[s_index] = true;
                            if !query_check(self.optional[s_index].1, o_val.1, s_types, o_types) {
                                return false;
                            }
                        }
                    }
                    if let Some(o) = other.field_type {
                        // Check field types
                        if let Some(s) = self.field_type {
                            if !query_check(s, o, s_types, o_types) { return false; }
                        }
                        // Check remaining self.req fields
                        if !req_list.iter().enumerate().all(|(index, checked)| {
                            if !checked {
                                query_check(self.required[index].1, o, s_types, o_types)
                            }
                            else {
                                true
                            }
                        })
                        {
                            return false;
                        }
                        // Check remaining self.opt fields
                        if !opt_list.iter().enumerate().all(|(index, checked)| {
                            if !checked {
                                query_check(self.optional[index].1, o, s_types, o_types)
                            }
                            else {
                                true
                            }
                        })
                        {
                            return false;
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

fn get_obj(raw: &mut &[u8]) -> crate::Result<Box<[u8]>> {
    let fail_len = raw.len();
    let start = raw.clone();
    if let MarkerType::Object(len) = read_marker(raw)? {
        verify_map(raw, len)?;
    }
    else {
        return Err(Error::FailValidate(fail_len, "Expected objects in `in`/`nin` fields"));
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
        let mut reader = ValidReader::new(is_query, &mut types, &mut type_names, &schema_hash);
        let validator = Validator::read_validator(&mut &raw[..], &mut reader).unwrap();
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
        types[validator].validate(&mut &raw_doc[..], &types, validator, &mut list).unwrap();
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
        assert!(types[validator].validate(&mut &raw_test[..], &types, validator, &mut list).is_ok());

        // Should pass with only required fields
        raw_test.clear();
        let test: Value = fogpack!({
            "test": true,
        });
        encode::write_value(&mut raw_test, &test);
        let mut list = ValidatorChecklist::new();
        assert!(types[validator].validate(&mut &raw_test[..], &types, validator, &mut list).is_ok());

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
        assert!(types[validator].validate(&mut &raw_test[..], &types, validator, &mut list).is_err());
    }
}
