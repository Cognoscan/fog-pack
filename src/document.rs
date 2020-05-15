use std::io;
use std::io::ErrorKind::InvalidData;

use MarkerType;
use CompressType;
use super::{MAX_DOC_SIZE, Hash, Value, ValueRef};
use super::crypto::{HashState, Vault, Key, Identity, CryptoError};
use decode;

/// A single, immutable fog-pack object that can be signed, hashed, and compressed.
#[derive(Clone)]
pub struct Document {
    hash_state: Option<HashState>,
    doc_hash: Option<Hash>,
    hash: Hash,
    doc_len: usize,
    doc: Vec<u8>,
    compressed: Option<Vec<u8>>,
    override_compression: bool,
    compression: Option<i32>,
    signed_by: Vec<Identity>,
    schema_hash: Option<Hash>,
}

// Documents with matching hashes are completely identical
impl PartialEq for Document {
    fn eq(&self, other: &Self) -> bool {
        self.hash() == other.hash()
    }
}

impl Eq for Document {}

impl Document {

    /// Not to be used outside the crate. Allows for creation of a Document from its internal 
    /// parts. Should only be used by Schema/NoSchema for completing the decoding process.
    pub(crate) fn from_parts(
        hash_state: Option<HashState>,
        doc_hash: Option<Hash>,
        hash: Hash,
        doc_len: usize,
        doc: Vec<u8>,
        compressed: Option<Vec<u8>>,
        override_compression: bool,
        compression: Option<i32>,
        signed_by: Vec<Identity>,
        schema_hash: Option<Hash>,
        ) -> Document {

        Document {
            hash_state,
            doc_hash,
            hash,
            doc_len,
            doc,
            compressed,
            override_compression,
            compression,
            signed_by,
            schema_hash,
        }
    }

    /// Create a new document from a given Value. Fails if value isn't an Object, if the value 
    /// has an empty string ("") field that doesn't contain a hash, or if the encoded value is 
    /// greater than the maximum allowed document size.
    pub fn new(v: Value) -> Result<Document, ()> {
        let schema_hash = if let Some(obj) = v.as_obj() {
            if let Some(val) = obj.get("") {
                if let Some(hash) = val.as_hash() {
                    Some(hash.clone())
                }
                else {
                    return Err(()); // Empty string field doesn't contain a hash.
                }
            }
            else {
                None
            }
        }
        else {
            return Err(()); // Value isn't an object
        };
        let mut doc = Vec::new();
        super::encode::write_value(&mut doc, &v);
        let doc_len = doc.len();
        if doc_len > MAX_DOC_SIZE {
            return Err(());
        }
        let mut hash_state = HashState::new();
        hash_state.update(&doc[..]); 
        let hash = hash_state.get_hash();
        let doc_hash = Some(hash.clone());
        Ok(Document {
            hash_state: Some(hash_state),
            doc_hash,
            hash,
            doc_len,
            doc,
            compressed: None,
            override_compression: false,
            compression: None,
            signed_by: Vec::new(),
            schema_hash,
        })
    }

    /// Sign the document with a given Key from a given Vault. Fails if the key is invalid 
    /// (`BadKey`), or can't be found (`NotInStorage`). Also fails if the resulting document is 
    /// larger than the maximum allowed document size.
    pub fn sign(&mut self, vault: &Vault, key: &Key) -> Result<(), CryptoError> {

        // Create the hasher, compute the inner document hasher, and update the hasher to include 
        // any existing signatures.
        if self.hash_state.is_none() || self.doc_hash.is_none() {
            let mut hash_state = HashState::new();
            hash_state.update(&self.doc[..self.doc_len]);
            let doc_hash = hash_state.get_hash();
            if self.doc.len() > self.doc_len {
                hash_state.update(&self.doc[self.doc_len..]);
            }
            self.hash_state = Some(hash_state);
            self.doc_hash = Some(doc_hash);
        }

        let signature = vault.sign(self.doc_hash.as_ref().unwrap(), key)?;
        self.signed_by.push(signature.signed_by().clone());
        let len = self.doc.len();
        signature.encode(&mut self.doc);
        let new_len = self.doc.len();
        if new_len > MAX_DOC_SIZE {
            return Err(CryptoError::Io(io::Error::new(InvalidData, "Document is too large with signature")));
        }
        if new_len > len {
            let hash_state = self.hash_state.as_mut().unwrap();
            hash_state.update(&self.doc[len..]);
            self.hash = hash_state.get_hash();
        }
        self.compressed = None;
        Ok(())
    }

    /// Specifically set compression, overriding default schema settings. If None, no compression 
    /// will be used. If some `i32`, the value will be passed to the zstd compressor. Note: if the 
    /// document has no schema settings, it defaults to generic compression with default zstd 
    /// settings.
    pub fn set_compression(&mut self, compression: Option<i32>) {
        self.override_compression = true;
        self.compression = compression;
    }

    /// Remove any overrides on the compression settings set by [`set_compression`].
    pub fn reset_compression(&mut self) {
        self.override_compression = false;
    }

    /// Get an iterator over all known signers of the document.
    pub fn signed_by(&self) -> std::slice::Iter<Identity> {
        self.signed_by.iter()
    }

    /// Get the length of the raw document, prior to encoding.
    pub fn len(&self) -> usize {
        self.doc.len()
    }

    /// Get the Hash of the document as it currently is. Note that adding additional signatures 
    /// will change the Hash.
    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    /// Get the Hash of the schema used by the document, if it exists.
    pub fn schema_hash(&self) -> &Option<Hash> {
        &self.schema_hash
    }

    /// Returns the compression setting that will be used if the compression is overridden. Check 
    /// override status with [`override_compression`].
    pub fn compression(&self) -> Option<i32> {
        self.compression
    }

    /// Returns true if compression is being overridden. If true, the setting returned by 
    /// [`compression`] will be used.
    pub fn override_compression(&self) -> bool {
        self.override_compression
    }

    /// Retrieve the value stored inside the document as a `ValueRef`. This value has the same 
    /// lifetime as the Document; it can be converted to a `Value` if it needs to outlast the 
    /// Document.
    pub fn value(&self) -> ValueRef {
        super::decode::read_value_ref(&mut &self.doc[..]).unwrap()
    }

    pub(crate) fn raw_doc(&self) -> &[u8] {
        &self.doc
    }

}

/// Finds the schema hash for a raw, encoded document. Fails if raw data isn't an object, or if 
/// the empty field ("") doesn't contain a Hash. If there is no empty field, `None` is returned.
pub fn extract_schema_hash(buf: &[u8]) -> io::Result<Option<Hash>> {
    let mut buf: &[u8] = buf;
    let compressed = CompressType::decode(&mut buf)?;
    match compressed {
        CompressType::CompressedNoSchema => Ok(None),
        CompressType::Uncompressed | CompressType::Compressed | CompressType::DictCompressed 
            => parse_schema_hash(&mut buf),
    }
}

/// Parses the schema hash and advances the slice pointer past the hash. Used when we already 
/// parsed the compression type and want to try reading the schema hash
pub(crate) fn parse_schema_hash(buf: &mut &[u8]) -> io::Result<Option<Hash>> {
    // Get the object tag & number of field/value pairs it has
    let obj_len = if let MarkerType::Object(len) = decode::read_marker(buf)? {
        len
    }
    else {
        return Err(io::Error::new(InvalidData, "Raw document isn't a fogpack object"));
    };
    if obj_len == 0 { return Ok(None); }

    // Get the first field - should be the empty string if there is a schema used.
    let field = decode::read_str(buf)?;
    if field.len() > 0 {
        return Ok(None);
    }
    decode::read_hash(buf)
        .map(|v| Some(v))
        .map_err(|_e| io::Error::new(InvalidData, "Empty string field doesn't have a Hash as its value"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{Vault, PasswordLevel};

    fn test_doc() -> Document {
        let test: Value = fogpack!({
            "test": true,
            "boolean": true,
            "positive": 1,
            "negative": -1,
            "string": "string",
            "float32": 1.0f32,
            "float64": 1.0f64,
            "binary": vec![0u8,1u8,2u8],
            "array": [Value::from(0), Value::from("an_array")] 
        });
        Document::new(test).expect("Should've been able to encode as a document")
    }

    fn test_doc_with_schema() -> Document {
        let fake_hash = Hash::new("test".as_bytes());
        let test: Value = fogpack!({
            "" : fake_hash,
            "test": true,
            "boolean": true,
        });
        Document::new(test).expect("Should've been able to encode as a document")
    }

    fn prep_vault() -> (Vault, Key) {
        let mut vault = Vault::new_from_password(PasswordLevel::Interactive, "test".to_string())
            .expect("Should have been able to make a new vault for testing");
        let key = vault.new_key();
        (vault, key)
    }

    #[test]
    fn equality_checks() {
        let test0 = test_doc_with_schema();
        let test1 = test_doc();
        let test2 = test_doc();
        assert!(test0 != test1, "Different documents were considered equal");
        assert!(test2 == test1, "Identically-generated documents weren't considered equal");
    }

    #[test]
    fn large_data() {
        let mut large_bin = Vec::new();
        large_bin.resize(MAX_DOC_SIZE, 0u8);
        let test: Value = fogpack!({
            "b": large_bin.clone(),
        });
        let test = Document::new(test);
        assert!(test.is_err(), "Should've been too large to encode as a document");

        large_bin.resize(MAX_DOC_SIZE-8, 0u8);
        let test: Value = fogpack!({
            "b": large_bin,
        });
        let test = Document::new(test);
        assert!(test.is_ok(), "Should've been just small enough to encode as a document");

        let mut test = test.unwrap();
        let (vault, key) = prep_vault();
        assert!(test.sign(&vault, &key).is_err(), "Should've failed because signing put it past the maximum allowed size");
    }
}
