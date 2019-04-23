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
        let len = doc.len();
        Some(Document {
            hash,
            doc,
            signatures: Vec::new(),
            len
        })
    }

    /// Sign the document with a given Key from a given Vault. Documents can be signed up to 127 
    /// times.
    pub fn sign(&mut self, vault: &Vault, key: &Key) -> Result<(), CryptoError> {
        let _sig_list_len = self.signatures.len();
        //if sig_list_len >= 127 { return Err(CryptoError::LengthExceeded); }
        let signature = vault.sign(&self.hash, key)?;
        self.len += signature.len();
        self.signatures.push(signature);
        Ok(())
    }

    pub fn num_signatures(&self) -> usize {
        self.signatures.len()
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
        for sig in self.signatures.iter() {
            sig.encode(buf);
        }
    }

    pub fn decode(_buf: &mut &[u8]) -> Result<Document, CryptoError> {
        Err(CryptoError::UnsupportedVersion)
    }
}
