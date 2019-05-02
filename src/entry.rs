use super::Value;
use super::Hash;
use super::crypto::{Vault, Key, Signature, CryptoError};

#[derive(Clone)]
pub struct Entry {
    doc: Hash,
    hash: Hash,
    field: String,
    content: Vec<u8>,
    signatures: Vec<Signature>,
    len: usize,
}

impl Entry {
    /// Create a new entry from a given Value.
    pub fn new(doc: Hash, field: String, v: Value) -> Option<Entry> {
        let mut content = Vec::new();
        super::encode::write_value(&mut content, &Value::from(doc.clone()));
        super::encode::write_value(&mut content, &Value::from(field.clone()));
        super::encode::write_value(&mut content, &v);
        let hash = Hash::new(1, &content[..]).unwrap(); // Shouldn't fail if version == 1
        let len = content.len() + 1; // +1 for the number of signatures at the end
        Some(Entry {
            doc,
            hash,
            field,
            content,
            signatures: Vec::new(),
            len
        })
    }

    /// Sign the entry with a given Key from a given Vault. Entries can be signed up to 127 times. 
    /// Fails if the key is invalid (`BadKey`), can't be found (`NotInStorage`), or is being signed 
    /// more than 127 times (`BadLength`)
    pub fn sign(&mut self, vault: &Vault, key: &Key) -> Result<(), CryptoError> {
        if self.signatures.len() >= 127 { return Err(CryptoError::BadLength); }
        let signature = vault.sign(&self.hash, key)?;
        self.len += signature.len();
        self.signatures.push(signature);
        Ok(())
    }

    pub fn num_signatures(&self) -> u8 {
        self.signatures.len() as u8
    }

    /// Create an entry and immediately sign it.
    pub fn new_signed(doc: Hash, field: String, v: Value, vault: &Vault, key: &Key) -> Result<Entry, CryptoError> {
        let mut entry = Entry::new(doc, field, v).unwrap();
        entry.sign(vault, key)?;
        Ok(entry)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn doc(&self) -> Hash {
        self.doc.clone()
    }

    pub fn hash(&self) -> Hash {
        self.hash.clone()
    }

    pub fn encode(&self, buf: &mut Vec<u8>) {
        buf.reserve(self.len);
        buf.extend_from_slice(&self.content[..]);
        buf.push(self.num_signatures());
        for sig in self.signatures.iter() {
            sig.encode(buf);
        }
    }

    pub fn decode(_buf: &mut &[u8]) -> Result<Entry, CryptoError> {
        Err(CryptoError::UnsupportedVersion)
    }
}

