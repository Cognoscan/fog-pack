use {Hash, Error, Document, Entry, Value, Identity, ValueRef};
use validator::{Validator, ValidatorChecklist};
use checklist::{DecodeChecklist, ChecklistItem};
use encode;
use decode;

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
    /// Take an existing entry and validate it against the Query. On success, a [`DecodeChecklist`] 
    /// is returned. Processing the checklist using this Query will complete the checklist and 
    /// yield the [`Entry`], successfully verifying it passes thie Query.
    ///
    /// [`Entry`]: ./struct.Entry.html
    /// [`DecodeChecklist`]: ./struct.DecodeChecklist.html
    pub fn validate_entry(&self, entry: Entry) -> crate::Result<DecodeChecklist> {
        let mut entry_ptr: &[u8] = entry.raw_entry();
        if entry.doc_hash() != self.doc_hash() {
            return Err(Error::FailValidate(entry_ptr.len(),"Entry doesn't have same document hash as the query"));
        }
        if entry.field() != self.field() {
            return Err(Error::FailValidate(entry_ptr.len(),"Entry doesn't have same field string as the query"));
        }

        let mut checklist = ValidatorChecklist::new();
        self.types[self.valid].validate(&mut entry_ptr, &self.types, self.valid, &mut checklist)?;
        Ok(DecodeChecklist::new(checklist, entry))
    }

    /// Checks a document against a given ChecklistItem. Marks the item as done on success. Fails 
    /// if validation fails. This should only be done with items coming from a DecodeChecklist 
    /// provided by a given Query's validate_entry function.
    ///
    /// A [`ChecklistItem`] comes from a [`DecodeChecklist`].
    ///
    /// [`ChecklistItem`] ./struct.ChecklistItem.html
    /// [`EncodeChecklist`] ./struct.EncodeChecklist.html
    /// [`DecodeChecklist`] ./struct.DecodeChecklist.html
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
/// ['Entry`]: ./struct.Entry.html
/// ['Document`]: ./struct.Document.html
pub fn encode_query(entry: Entry) -> Vec<u8> {
    let mut buf = Vec::new();
    entry.doc_hash().encode(&mut buf);
    encode::write_value(&mut buf, &Value::from(entry.field()));
    buf.extend_from_slice(entry.raw_entry());
    buf
}

