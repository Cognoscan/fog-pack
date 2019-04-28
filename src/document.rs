use super::{Hash, Value, ValueRef};
use super::crypto::{HashState, Vault, Key, Identity, CryptoError};

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

    pub fn signed_by(&self) -> std::slice::Iter<Identity> {
        self.signed_by.iter()
    }

    pub fn len(&self) -> usize {
        self.doc.len()
    }

    pub fn to_vec(self) -> Vec<u8> {
        self.doc
    }

    /// Get the Hash of the document as it currently is. Note that adding additional signatures 
    /// will change the Hash.
    pub fn hash(&self) -> Hash {
        self.hash_state.get_hash()
    }

    pub fn get_value(&self) -> ValueRef {
        super::decode::read_value_ref(&mut &self.doc[..]).unwrap()
    }
}

// Finds the schema hash for a raw, encoded document. Fails if raw data isn't an object, or if 
// the empty field ("") doesn't contain a Hash. If there is no empty field, `None` is returned.
//fn extract_schema(&[u8]) -> Result<Option<Hash>, ()> {
//
//}

