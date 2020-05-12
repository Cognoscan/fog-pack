
use std::io;
use std::io::Error;
use std::io::ErrorKind::{InvalidData,Other};
use std::collections::HashMap;

use MarkerType;
use decode::*;
use document::extract_schema_hash;
use validator::{ValidObj, Validator, ValidatorChecklist};
use Hash;

/// Struct holding the validation portions of a schema. Can be used for validation of a document or 
/// entry.
#[derive(Clone, Debug)]
pub struct Schema {
    hash: Hash,
    object: ValidObj,
    entries: Vec<(String, usize)>,
    types: Vec<Validator>,
}

impl Schema {
    pub fn from_raw(raw: &mut &[u8]) -> io::Result<Schema> {
        let raw_for_hash: &[u8] = raw;
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
                "" => {
                    read_hash(raw).map_err(|_e| Error::new(InvalidData, "Schema's empty field didn't contain root Schema Hash"))?;
                },
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
                "types" => {
                    if let MarkerType::Object(len) = read_marker(raw)? {
                        object_iterate(raw, len, |field, raw| {
                            let v = Validator::read_validator(raw, false, &mut types, &mut type_names)?;
                            if v == (types.len() - 1) {
                                let v = types.pop();
                                match field {
                                    "Null" | "Bool" | "Int" | "Str" | "F32" | "F64" | "Bin" |
                                    "Array" | "Obj" | "Hash" | "Ident" | "Lock" | "Time" | "Multi" => (),
                                    _ => {
                                        if let Some(index) = type_names.get(field) {
                                            types[*index] = v.unwrap();
                                        }
                                    }
                                }
                            }
                            Ok(())
                        })?;
                    }
                    else {
                        return Err(Error::new(InvalidData, "`entries` field doesn't contain an Object"));
                    }
                }
                _ => {
                    return Err(Error::new(InvalidData, "Unrecognized field in schema document"));
                }
            }
            Ok(())
        })?;

        let hash = Hash::new(raw_for_hash);
        Ok(Schema {
            hash,
            object,
            entries,
            types,
        })
    }

    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    /// Validates a document against this schema. Does not check the schema field itself.
    pub fn validate_doc(&self, doc: &mut &[u8]) -> io::Result<()> {
        let mut checklist = ValidatorChecklist::new();
        self.object.validate("", doc, &self.types, &mut checklist, true).and(Ok(()))
    }

    /// Validates a given entry against this schema.
    pub fn validate_entry(&self, entry: &str, doc: &mut &[u8]) -> io::Result<ValidatorChecklist> {
        let mut checklist = ValidatorChecklist::new();
        let v = self.entries.binary_search_by(|x| x.0.as_str().cmp(entry));
        if v.is_err() { return Err(Error::new(InvalidData, "Entry field type doesn't exist in schema")); }
        let v = self.entries[v.unwrap()].1;
        self.types[v].validate("", doc, &self.types, 0, &mut checklist)?;
        Ok(checklist)
    }

    /// Validates a document against a specific Hash Validator. Should be used in conjunction with 
    /// a ValidatorChecklist returned from `validate_entry` to confirm that all documents referenced in an 
    /// entry meet the schema's criteria.
    pub fn validate_checklist_item(&self, index: usize, doc: &mut &[u8]) -> io::Result<()> {
        if let Validator::Hash(ref v) = self.types[index] {
            // Extract schema. Also verifies we are dealing with an Object (an actual document)
            let doc_schema = extract_schema_hash(&doc.clone())?;
            // Check against acceptable schemas
            if v.schema_required() {
                if let Some(hash) = doc_schema {
                    if !v.schema_in_set(&hash) {
                        return Err(Error::new(InvalidData, "Document uses unrecognized schema"));
                    }
                }
                else {
                    return Err(Error::new(InvalidData, "Document doesn't have schema, but needs one"));
                }
            }
            if let Some(link) = v.link() {
                let mut checklist = ValidatorChecklist::new();
                if let Validator::Object(ref v) = self.types[link] {
                    v.validate("", doc, &self.types, &mut checklist, true).and(Ok(()))
                }
                else {
                    Err(Error::new(Other, "Can't validate a document against a non-object validator"))
                }
            }
            else {
                Ok(())
            }
        }
        else {
            Err(Error::new(Other, "Can't validate against non-hash validator"))
        }

    }
}

