use std::io;
use super::Value;
use super::Hash;
use super::crypto::{Vault, Key, Signature, CryptoError};

pub struct Document {
    hash: Hash,
    doc: Vec<u8>,
    signatures: Vec<Signature>,
    len: usize,
}

impl Document {
    /// Create a new document from a given Value.
    pub fn new(v: Value) -> Option<Document> {
        let mut doc = Vec::new();
        super::encode::write_value(&mut doc, &v);
        let hash = Hash::new(1, &doc[..]).unwrap(); // Shouldn't fail if version == 1
        let len = doc.len() + 1; // +1 for the number of signatures at the end
        Some(Document {
            hash,
            doc,
            signatures: Vec::new(),
            len
        })
    }

    /// Sign the document with a given Key from a given Vault. Documents can be signed up to 127 
    /// times. Fails if the key is invalid (`BadKey`), can't be found (`NotInStorage`), or is being 
    /// signed more than 127 times (`BadLength`)
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

    /// Create a document from a given value and immediately sign it.
    pub fn new_signed(v: Value, vault: &Vault, key: &Key) -> Result<Document, CryptoError> {
        let mut doc = Document::new(v).unwrap();
        doc.sign(vault, key)?;
        Ok(doc)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn hash(&self) -> Hash {
        self.hash.clone()
    }

    pub fn encode(&self, buf: &mut Vec<u8>) {
        buf.reserve(self.len);
        buf.extend_from_slice(&self.doc[..]);
        buf.push(self.num_signatures());
        for sig in self.signatures.iter() {
            sig.encode(buf);
        }
    }

    pub fn get_value(&self) -> io::Result<Value> {
        super::decode::read_value(&mut &self.doc[..])
    }

    pub fn decode(_buf: &mut &[u8]) -> Result<Document, CryptoError> {
        // Extract the document, but don't decode the Value
        // hash: Hash,
        // doc: Vec<u8>,
        // signatures: Vec<Signature>,
        // len: usize,
        // Get number of signatures
        // Attempt to read in all signatures
        Err(CryptoError::UnsupportedVersion)
    }
}
