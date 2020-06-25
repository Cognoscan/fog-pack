use std::io;
use std::io::ErrorKind::InvalidData;

use Error;
use CompressType;
use crypto;
use {MAX_ENTRY_SIZE, Hash, Value, ValueRef};
use crypto::{HashState, Vault, Key, Identity, CryptoError};
use encode;
use zstd_help;

#[derive(Clone)]
/// A fog-pack value that can be signed and compressed, with an associated document hash and field.
pub struct Entry {
    hash_state: Option<HashState>,
    entry_hash: Option<Hash>,
    hash: Hash,
    doc: Hash,
    field: String,
    entry_len: usize,
    entry: Vec<u8>,
    signed_by: Vec<Identity>,
    compressed: Option<Vec<u8>>,
    compress_cache: bool,
    override_compression: bool,
    compression: Option<i32>,
    validated: bool,
}

// Entries are completely identical (including parent hash and field) if their hashes match
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.hash() == other.hash()
    }
}

impl Eq for Entry {}

// Anything working with Entry in a mutable fashion (or creating one) is responsible for keeping 
// the hashes updated. Namely, `hash_state` must be updated when entry/field/doc change, and 
// entry_hash must be updated if field/doc change or the encoded value changes. The `hash` must 
// always be kept up-to-date.
impl Entry {

    /// Not to be used outside the crate. Allows for creation of an Entry from internal parts. 
    /// Should only be used by Schema impl for completing the decoding process.
    pub(crate) fn from_decoded(
        doc: Hash,
        field: String,
        entry: Vec<u8>,
        entry_len: usize,
        compressed: Option<Vec<u8>>,
        hash: Option<Hash>,
        do_checks: bool,
    ) -> crate::Result<Entry>
    {
        // Optionally compute the hashes
        // -----------------------------
        let (hash_state, entry_hash, hash) = if let Some(hash) = hash {
            (None, None, hash)
        }
        else {
            let (hash_state, entry_hash, hash) = Self::calc_hash_state(&doc, field.as_str(), &entry[..], entry_len);
            (Some(hash_state), Some(entry_hash), hash)
        };

        // Signature Processing
        // --------------------
        let mut signed_by = Vec::new();
        let mut index = &mut &entry[entry_len..];
        while !index.is_empty() {
            let signature = crypto::Signature::decode(&mut index)?;
            if do_checks {
                // Unwrap the entry_hash because it darn well better be populated if we're doing 
                // checks.
                let verify = signature.verify(entry_hash.as_ref().unwrap());
                if !verify {
                    return Err(Error::BadSignature);
                }
            }
            signed_by.push(signature.signed_by().clone());
        }

        Ok(Entry {
            doc,
            field,
            hash_state,
            entry_hash,
            hash,
            entry_len,
            entry,
            signed_by,
            compressed,
            compress_cache: true,
            override_compression: false,
            compression: None,
            validated: true
        })
    }

    /// Create a new entry from a given Value. Fails if the resulting entry is larger than the 
    /// maximum allowed size.
    pub fn new(doc: Hash, field: String, v: Value) -> Result<Entry, ()> {
        let mut entry = Vec::new();
        CompressType::Uncompressed.encode(&mut entry);
        // Future location of the entry length
        entry.push(0u8);
        entry.push(0u8);
        encode::write_value(&mut entry, &v);
        let entry_len = entry.len();
        if entry_len >= MAX_ENTRY_SIZE {
            return Err(()); // Entry is too big
        }
        let data_len = entry_len - 3;
        entry[1] = ((data_len & 0xFF00) >>  8) as u8;
        entry[2] = ( data_len & 0x00FF)        as u8;

        let (hash_state, entry_hash, hash) = Self::calc_hash_state(&doc, field.as_str(), &entry[..], entry_len);

        Ok(Entry {
            doc,
            field,
            hash_state: Some(hash_state),
            entry_hash: Some(entry_hash),
            hash,
            entry_len,
            entry,
            signed_by: Vec::new(),
            compressed: None,
            compress_cache: false,
            override_compression: false,
            compression: None,
            validated: false
        })
    }

    /// Calculate the internal HashState, entry data hash, and overall hash.
    pub(crate) fn calc_hash_state(doc: &Hash, field: &str, entry: &[u8], entry_len: usize) -> (HashState, Hash, Hash) {
        let mut temp = Vec::new();
        let mut hash_state = HashState::new();
        doc.encode(&mut temp);
        hash_state.update(&temp[..]);
        temp.clear();
        encode::write_value(&mut temp, &Value::from(field));
        hash_state.update(&temp[..]);
        hash_state.update(&entry[3..entry_len]);
        let entry_hash = hash_state.get_hash();
        let hash = if entry.len() > entry_len {
            hash_state.update(&entry[entry_len..]);
            hash_state.get_hash()
        } else {
            entry_hash.clone()
        };
        (hash_state, entry_hash, hash)
    }

    /// Sign the entry with a given Key from a given Vault.  Fails if the key is invalid 
    /// (`BadKey`), can't be found (`NotInStorage`), or the resulting entry is larger than the 
    /// maximum allowed entry size.
    pub fn sign(&mut self, vault: &Vault, key: &Key) -> Result<(), CryptoError> {
        if self.hash_state.is_none() || self.entry_hash.is_none() {
            let (hash_state, entry_hash, _) = 
                Self::calc_hash_state(self.doc_hash(), self.field(), self.raw_entry(), self.entry_len());
            self.hash_state = Some(hash_state);
            self.entry_hash = Some(entry_hash);
        }
        let signature = vault.sign(self.entry_hash.as_ref().unwrap(), key)?;
        self.signed_by.push(signature.signed_by().clone());
        let len = self.entry.len();
        signature.encode(&mut self.entry);
        let new_len = self.entry.len();
        if new_len >= MAX_ENTRY_SIZE {
            return Err(CryptoError::Io(io::Error::new(InvalidData, "Entry is too large with signature")));
        }
        if new_len > len {
            let hash_state = self.hash_state.as_mut().unwrap();
            hash_state.update(&self.entry[len..]);
            self.hash = hash_state.get_hash();
        }
        self.compressed = None;
        self.compress_cache = false;
        Ok(())
    }

    /// Specifically set compression, overriding default schema settings. If None, no compression 
    /// will be used. If some `i32`, the value will be passed to the zstd compressor. Note: if the 
    /// document has no schema settings, it defaults to generic compression with default zstd 
    /// settings.
    pub fn set_compression(&mut self, compression: Option<i32>) {
        self.override_compression = true;
        self.compression = compression;
        self.compressed = None;
        self.compress_cache = false;
    }

    /// Remove any overrides on the compression settings set by [`set_compression`].
    ///
    /// [`set_compression`]: #method.set_compression
    pub fn reset_compression(&mut self) {
        self.override_compression = false;
        self.compressed = None;
        self.compress_cache = false;
    }

    /// Clear out any cached compression data. If the Entry was decoded and had been compressed, 
    /// the compressed version is cached on load in case this is to be re-encoded. This can be 
    /// called to clear out the cached compressed version - it will also be cleared if either
    /// [`set_compression`] or [`reset_compression`] is called.
    ///
    /// [`set_compression`]: #method.set_compression
    /// [`reset_compression`]: #method.reset_compression
    pub fn clear_compress_cache(&mut self) {
        self.compressed = None;
        self.compress_cache = false;
    }

    /// Get an iterator over all known signers of the document.
    pub fn signed_by(&self) -> std::slice::Iter<Identity> {
        self.signed_by.iter()
    }

    /// Get the size of the entry in bytes (minus document hash & field name) prior to encoding
    pub fn size(&self) -> usize {
        self.entry.len()
    }

    /// Get the Hash of the Entry as it currently is. Note that adding additional signatures 
    /// will change the Hash.
    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    /// Get the Hash of the parent document for the Entry.
    pub fn doc_hash(&self) -> &Hash {
        &self.doc
    }

    /// Get the field for this entry.
    pub fn field(&self) -> &str {
        self.field.as_str()
    }

    /// Returns the compression setting that will be used if the compression is overridden. Check 
    /// override status with [`override_compression`].
    ///
    /// [`override_compression`]: #method.override_compression
    pub fn compression(&self) -> Option<i32> {
        self.compression
    }

    /// Returns true if compression is being overridden. If true, the setting returned by 
    /// [`compression`] will be used.
    ///
    /// [`compression`]: #method.compression
    pub fn override_compression(&self) -> bool {
        self.override_compression
    }

    /// Returns true if the document has previously been validated by a schema or the general 
    /// fog-pack validator. This is always true on Entries that were decoded from raw byte 
    /// slices.
    pub fn validated(&self) -> bool {
        self.validated
    }

    /// Retrieve the value stored inside the entry as a `ValueRef`. This value has the same 
    /// lifetime as the Entry; it can be converted to a `Value` if it needs to outlast the 
    /// Entry.
    pub fn value(&self) -> ValueRef {
        super::decode::read_value_ref(&mut &self.entry[..]).unwrap()
    }

    pub(crate) fn raw_entry(&self) -> &[u8] {
        &self.entry
    }

    pub(crate) fn into_vec(self) -> Vec<u8> {
        self.entry
    }

    pub(crate) fn compress_cache(&self) -> bool {
        self.compress_cache
    }

    pub(crate) fn into_compressed_vec(self) -> Vec<u8> {
        self.compressed.unwrap_or(self.entry)
    }

    pub(crate) fn entry_len(&self) -> usize {
        self.entry_len
    }

    pub(crate) fn entry_val(&self) -> &[u8] {
        &self.entry[3..self.entry_len]
    }
}

/// Train a zstd dictionary from a sequence of entries.
///
/// Dictionaries can be limited to a maximum size. On failure, a zstd library error code is 
/// returned.
///
/// The zstd documentation recommends around 100 times as many input bytes as the desired 
/// dictionary size. It can be useful to check the resulting dictionary for overlearning - just 
/// dump the dictionary to a file and look for human-readable strings. These can occur when the 
/// dictionary is larger than necessary, and begins encoding the randomized portions of the 
/// Entries. In the future, this function may become smarter and get better at eliminating 
/// low-probability dictionary items.
pub fn train_entry_dict(max_size: usize, entries: Vec<Entry>) -> Result<Vec<u8>, usize> {
    let samples = entries
        .iter()
        .map(|entry| Vec::from(entry.entry_val()))
        .collect::<Vec<Vec<u8>>>();
    zstd_help::train_dict(max_size, samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{Vault, PasswordLevel};

    fn test_entry() -> Entry {
        let test: Value = fogpack!(vec![0u8, 1u8, 2u8]);
        Entry::new(Hash::new_empty(), String::from(""), test).expect("Should be able to make entry")
    }

    fn prep_vault() -> (Vault, Key) {
        let mut vault = Vault::new_from_password(PasswordLevel::Interactive, "test".to_string())
            .expect("Should have been able to make a new vault for testing");
        let key = vault.new_key();
        (vault, key)
    }

    #[test]
    fn equality_checks() {
        let test0 = Entry::new(Hash::new_empty(), String::from("a"), fogpack!(vec![0u8, 1u8, 2u8]))
            .expect("Should be able to make entry");
        let test1 = test_entry();
        let test2 = test_entry();
        assert!(test0 != test1, "Different entries were considered equal");
        assert!(test2 == test1, "Identically-generated entries weren't considered equal");
    }

    #[test]
    fn large_data() {
        let mut large_bin = Vec::new();
        large_bin.resize(MAX_ENTRY_SIZE-6, 0u8);
        let test: Value = fogpack!(large_bin.clone());
        let test = Entry::new(Hash::new_empty(), String::from(""), test);
        assert!(test.is_err(), "Should've been too large to encode as a document");

        large_bin.resize(MAX_ENTRY_SIZE-7, 0u8);
        let test: Value = fogpack!(large_bin);
        let test = Entry::new(Hash::new_empty(), String::from(""), test);
        assert!(test.is_ok(), "Should've been just small enough to encode as a document");

        let mut test = test.unwrap();
        let (vault, key) = prep_vault();
        assert!(test.sign(&vault, &key).is_err(), "Should've failed because signing put it past the maximum allowed size");
    }


}
