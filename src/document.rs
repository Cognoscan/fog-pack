use std::io;
use std::io::ErrorKind::InvalidData;

use MarkerType;
use super::{Hash, Value, ValueRef};
use super::crypto::{HashState, Vault, Key, Identity, CryptoError};
use decode;

#[derive(Clone)]
pub struct Document {
    hash_state: HashState,
    doc_hash: Hash,
    doc: Vec<u8>,
    signed_by: Vec<Identity>,
}

impl Document {

    /// Create a new document from a given Value. Fails if value isn't an Object.
    pub fn new(v: Value) -> Result<Document, ()> {
        if !v.is_obj() { return Err(()); }
        let mut doc = Vec::new();
        super::encode::write_value(&mut doc, &v);
        let mut hash_state = HashState::new(1).unwrap(); // Shouldn't fail if version == 1
        hash_state.update(&doc[..]); 
        let doc_hash = hash_state.get_hash();
        Ok(Document {
            hash_state,
            doc_hash,
            doc,
            signed_by: Vec::new(),
        })
    }

    /// Sign the document with a given Key from a given Vault. Fails if the key is invalid 
    /// (`BadKey`), can't be found (`NotInStorage`).
    pub fn sign(&mut self, vault: &Vault, key: &Key) -> Result<(), CryptoError> {
        let signature = vault.sign(&self.doc_hash, key)?;
        self.signed_by.push(key.get_identity());
        let len = self.doc.len();
        signature.encode(&mut self.doc);
        if self.doc.len() > len {
            self.hash_state.update(&self.doc[len..]);
        }
        Ok(())
    }

    /// Get an iterator over all known signers of the document.
    pub fn signed_by(&self) -> std::slice::Iter<Identity> {
        self.signed_by.iter()
    }

    /// Get the length of the document when it is fully encoded.
    pub fn len(&self) -> usize {
        self.doc.len()
    }

    /// Turn the document into a raw encoded byte vector.
    pub fn to_vec(self) -> Vec<u8> {
        self.doc
    }

    /// Get the Hash of the document as it currently is. Note that adding additional signatures 
    /// will change the Hash.
    pub fn hash(&self) -> Hash {
        self.hash_state.get_hash()
    }

    /// Retrieve the value stored inside the document as a `ValueRef`. This value has the same 
    /// lifetime as the Document; it can be converted to a `Value` if it needs to outlast the 
    /// Document.
    pub fn get_value(&self) -> ValueRef {
        super::decode::read_value_ref(&mut &self.doc[..]).unwrap()
    }
}

// Finds the schema hash for a raw, encoded document. Fails if raw data isn't an object, or if 
// the empty field ("") doesn't contain a Hash. If there is no empty field, `None` is returned.
fn extract_schema(buf: &[u8]) -> io::Result<Option<Hash>> {
    let mut buf: &[u8] = buf;
    // Get the object tag & number of field/value pairs it has
    let obj_len = if let MarkerType::Object(len) = decode::read_marker(&mut buf)? {
        len
    }
    else {
        return Err(io::Error::new(InvalidData, "Raw document isn't a msgpack object"));
    };
    if obj_len == 0 { return Ok(None); }

    // Get the first field - should be the empty string if there is a schema used.
    let field = if let MarkerType::String(len) = decode::read_marker(&mut buf)? {
        decode::read_str(&mut buf, len)?
    }
    else {
        return Err(io::Error::new(InvalidData, "Field in document isn't a String"));
    };
    if field.len() > 0 {
        return Ok(None);
    }
    if let MarkerType::Hash(len) = decode::read_marker(&mut buf)? {
        Ok(Some(decode::read_hash(&mut buf, len)?))
    }
    else {
        Err(io::Error::new(InvalidData, "Empty string field doesn't have a Hash as its value"))
    }
}














