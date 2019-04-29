use std::io;
use std::io::ErrorKind::InvalidData;

use MarkerType;
use super::{Hash, Value, ValueRef};
use super::crypto::{HashState, Vault, Key, Identity, CryptoError};
use decode;
use crypto;

#[derive(Clone)]
pub struct Document {
    hash_state: HashState,
    doc_hash: Hash,
    doc_len: usize,
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
        let doc_len = doc.len();
        Ok(Document {
            hash_state,
            doc_hash,
            doc_len,
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

    /// Get the length of the encoded value only (without signatures).
    pub fn doc_len(&self) -> usize {
        self.doc_len
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
pub fn extract_schema(buf: &[u8]) -> io::Result<Option<Hash>> {
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
    let field = decode::read_str(&mut buf)?;
    if field.len() > 0 {
        return Ok(None);
    }
    decode::read_hash(&mut buf)
        .map(|v| Some(v))
        .map_err(|_e| io::Error::new(InvalidData, "Empty string field doesn't have a Hash as its value"))
}

/// Convert from a raw vector straight into a document. This should *only* be called by the 
/// internal database, as it does not do complete validity checking. It only verifies the hash and 
/// extracts signatues. It should not be expected that these checks will always occur.
pub fn from_raw(hash: &Hash, data: Vec<u8>, doc_len: usize) -> io::Result<Document> {
    if doc_len > data.len() { 
        return Err(io::Error::new(InvalidData, "Document length greater than raw data length"));
    }
    let mut hash_state = HashState::new(1).unwrap(); // Shouldn't fail if version == 1
    let mut signed_by = Vec::new();
    {
        let mut index = &mut &data[doc_len..];
        while index.len() > 0 {
            let signature = crypto::Signature::decode(&mut index)
                .map_err(|_e| io::Error::new(InvalidData, "Invalid signature in raw document"))?;
            signed_by.push(signature.signed_by().clone());
        }
    }
    // Get the HashState up to the correct point
    hash_state.update(&data[..doc_len]);
    let doc_hash = hash_state.get_hash();
    hash_state.update(&data[doc_len..]);
    if hash != &hash_state.get_hash() {
        return Err(io::Error::new(InvalidData, "Raw document doesn't match stored hash"));
    }
    Ok(Document {
        hash_state,
        doc_hash,
        doc_len,
        doc: data,
        signed_by,
    })
}














