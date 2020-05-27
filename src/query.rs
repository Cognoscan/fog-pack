use {Hash, Error, Document, Entry, Value, Identity, ValueRef};
use validator::{Validator, ValidatorChecklist};
use checklist::{Checklist, ChecklistItem};
use encode;
use decode;

/// A decoded Query, which may be use to check if an Entry matches it or not.
///
/// A Query can be produced by calling [`decode_query`] on the schema used by the Document being 
/// queried. Queries are Entries that have been encoded using [`encode_query`] and have a Validator 
/// as the contained fog-pack value. See the [Validation Spec] for more information.
///
/// It can be passed an [`Entry`] to validate, and will produce a Checklist for further 
/// validation if necessary. 
///
/// [`encode_query`]: ./fn.encode_query.html
/// [Validation Spec]: ./spec/validation/index.html
/// [`decode_query`]: ./struct.Schema.html#method.decode_query
/// [`Entry`]: ./struct.Entry.html
#[derive(Clone)]
pub struct Query {
    valid: usize,
    types: Box<[Validator]>,
    hash: Hash,
    doc: Hash,
    field: String,
    content: Vec<u8>,
    signed_by: Vec<Identity>,
}

impl Query {
    pub(crate) fn from_parts(
        valid: usize,
        types: Box<[Validator]>,
        hash: Hash,
        doc: Hash,
        field: String,
        content: Vec<u8>,
        signed_by: Vec<Identity>,
    ) -> Query {
        Query {
            valid,
            types,
            hash,
            doc,
            field,
            content,
            signed_by,
        }
    }

    /// Get an iterator over all known signers of the Query. Note: this does *not* authenticate the 
    /// originator of the query; at best, it indicates the signers have seen and signed this query 
    /// at some arbitrary point in the past.
    pub fn signed_by(&self) -> std::slice::Iter<Identity> {
        self.signed_by.iter()
    }

    /// Get the Hash of the Query from when it was encoded. This matches the hash of the Query 
    /// when it was written as an entry.
    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    /// Get the Hash of the document being queried against.
    pub fn doc_hash(&self) -> &Hash {
        &self.doc
    }

    /// Get the Entry field being queried.
    pub fn field(&self) -> &str {
        self.field.as_str()
    }

    /// Retrieve the value stored inside the Query as a `ValueRef`. This is useful for ancillary 
    /// parsing - for example, matching query contents against pre-built indices. This value has 
    /// the same lifetime as the Query; it can be converted to a `Value` if it needs to outlast the 
    /// Query.
    pub fn value(&self) -> ValueRef {
        decode::read_value_ref(&mut &self.content[..]).unwrap()
    }

    /// Take an entry and verify if it matches the Query or not. May require additional 
    /// Take an existing entry and validate it against the Query. On success, a [`Checklist`] 
    /// is returned. Processing the checklist using this Query will complete the checklist and 
    /// yield an `OK(())` result, indicating the Entry matched this Query.
    ///
    /// [`Entry`]: ./struct.Entry.html
    /// [`Checklist`]: ./checklist/struct.Checklist.html
    pub fn validate_entry(&self, entry: &Entry) -> crate::Result<Checklist<()>> {
        let mut entry_ptr: &[u8] = entry.raw_entry();
        if entry.doc_hash() != self.doc_hash() {
            return Err(Error::FailValidate(entry_ptr.len(),"Entry doesn't have same document hash as the query"));
        }
        if entry.field() != self.field() {
            return Err(Error::FailValidate(entry_ptr.len(),"Entry doesn't have same field string as the query"));
        }

        let mut checklist = ValidatorChecklist::new();
        self.types[self.valid].validate(&mut entry_ptr, &self.types, self.valid, &mut checklist)?;
        Ok(Checklist::new(checklist, ()))
    }

    /// Checks a document against a given ChecklistItem. Marks the item as done on success. Fails 
    /// if validation fails. This should only be done with items coming from a Checklist 
    /// provided by a given Query's validate_entry function.
    ///
    /// A [`ChecklistItem`] comes from a [`Checklist`].
    ///
    /// [`ChecklistItem`] ./struct.ChecklistItem.html
    /// [`Checklist`] ./checklist/struct.Checklist.html
    pub fn check_item(&self, doc: &Document, item: &mut ChecklistItem) -> crate::Result<()> {
        for index in item.iter() {
            if let Validator::Hash(ref v) = self.types[*index] {
                // Check against acceptable schemas
                if v.schema_required() {
                    if let Some(hash) = doc.schema_hash() {
                        if !v.schema_in_set(&hash) {
                            return Err(Error::FailValidate(doc.len(), "Document uses unrecognized schema"));
                        }
                    }
                    else {
                        return Err(Error::FailValidate(doc.len(), "Document doesn't have schema, but needs one"));
                    }
                }
                if let Some(link) = v.link() {
                    let mut checklist = ValidatorChecklist::new();
                    if let Validator::Object(ref v) = self.types[link] {
                        v.validate(&mut doc.raw_doc(), &self.types, &mut checklist, true)?;
                    }
                    else {
                        return Err(Error::FailValidate(doc.len(), "Can't validate a document against a non-object validator"));
                    }
                }
            }
            else {
                return Err(Error::FailValidate(doc.len(), "Can't validate against non-hash validator"));
            }
        };
        item.mark_done();
        Ok(())
    }

}

/// Encode an Entry for later decoding into a query.
///
/// An [`Entry`] can be used to describe a query to be made against a [`Document`], where the 
/// Entry's parent document is the document to be queried, and the field is the specific entry type 
/// to be queried for.
///
/// [`Entry`]: ./struct.Entry.html
/// [`Document`]: ./struct.Document.html
pub fn encode_query(entry: Entry) -> Vec<u8> {
    let mut buf = Vec::new();
    entry.doc_hash().encode(&mut buf);
    encode::write_value(&mut buf, &Value::from(entry.field()));
    buf.extend_from_slice(entry.raw_entry());
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use {Schema, Entry, Document};

    fn simple_schema() -> Schema {
        let schema: Value = fogpack!({
            "req": {
                "title": { "type": "Str", "max_len": 200 },
                "text": { "type": "Str" },
            },
            "entries": {
                "rel": {
                    "type": "Obj",
                    "req": {
                        "name": { "type": "Str", "query": true },
                        "link": { "type": "Hash" },
                    },
                    "obj_ok": true,
                },
                "misc": { "type": "Str" }
            },
            "doc_compress": {
                "format": 0,
                "level": 3,
                "setting": true,
            },
            "entries_compress": {
                "rel": { "setting": false },
                "misc": { "setting": false },
            }
        });
        let schema = Document::new(schema).expect("Should've been able to encode as a document");
        Schema::from_doc(schema).unwrap()
    }

    fn simple_doc(schema: &Hash) -> Document {
        let doc: Value = fogpack!({
            "": Value::from(schema.clone()),
            "title": "A Test",
            "text": "This is a test of a schema document"
        });
        println!("{}", doc);
        Document::new(doc).expect("Should've been able to encode as document")
    }

    fn rel_entry(doc: &Hash) -> Entry {
        let test: Value = fogpack!({
            "name": "test_entry",
            "link": Hash::new(b"fake hash")
        });
        Entry::new(doc.clone(), String::from("rel"), test).expect("Should've been able to encode as an entry")
    }

    fn misc_entry(doc: &Hash) -> Entry {
        let test: Value = fogpack!( "this is a misc value that can't be queried" );
        Entry::new(doc.clone(), String::from("misc"), test).expect("Should've been able to encode as an entry")
    }


    #[test]
    fn make_query() {
        let schema = simple_schema();
        let doc = simple_doc(schema.hash());
        let rel_entry = rel_entry(doc.hash());
        let misc_entry = misc_entry(doc.hash());

        let query: Value = fogpack!({
            "type": "Obj",
            "req": {
                "name": { "type": "Str", "in": "test_entry" },
            },
            "unknown_ok": true
        });
        let query = Entry::new(doc.hash().clone(), String::from("rel"), query).unwrap();
        let query = encode_query(query);
        let query = schema.decode_query(&mut &query[..]).expect("Should be an accepted query");


        let check = query.validate_entry(&rel_entry).expect("Should validate OK");
        check.complete().expect("Checklist should've already been complete!");

        let query: Value = fogpack!({ "type": "Str", "nin": ""});
        let query = Entry::new(doc.hash().clone(), String::from("misc"), query).unwrap();
        let query = encode_query(query);
        assert!(schema.decode_query(&mut &query[..]).is_err());
    }
}



