use std::io;
use std::io::ErrorKind::InvalidData;

use super::{Hash, Value, ValueRef};
use super::crypto::{HashState, Vault, Key, Identity, CryptoError};
use crypto;

#[derive(Clone)]
pub struct Entry {
    doc: Hash,
    field: String,
    hash_state: HashState,
    entry_hash: Hash,
    entry_len: usize,
    entry: Vec<u8>,
    signed_by: Vec<Identity>,
}

impl Entry {
    /// Create a new entry from a given Value.
    pub fn new(doc: Hash, field: String, v: Value) -> Option<Entry> {
        let mut entry = Vec::new();
        let mut hash_state = HashState::new(1).unwrap(); // Shouldn't fail if version == 1
        super::encode::write_value(&mut entry, &Value::from(doc.clone()));
        hash_state.update(&entry[..]);
        entry.clear();
        super::encode::write_value(&mut entry, &Value::from(field.clone()));
        hash_state.update(&entry[..]);
        entry.clear();
        super::encode::write_value(&mut entry, &v);
        hash_state.update(&entry[..]);
        let entry_hash = hash_state.get_hash();
        let entry_len = entry.len();
        Some(Entry {
            doc,
            field,
            hash_state,
            entry_hash,
            entry_len,
            entry,
            signed_by: Vec::new(),
        })
    }

    /// Sign the entry with a given Key from a given Vault. Entries can be signed up to 127 times. 
    /// Fails if the key is invalid (`BadKey`), can't be found (`NotInStorage`), or is being signed 
    /// more than 127 times (`BadLength`)
    pub fn sign(&mut self, vault: &Vault, key: &Key) -> Result<(), CryptoError> {
        let signature = vault.sign(&self.entry_hash, key)?;
        self.signed_by.push(signature.signed_by().clone());
        let len = self.entry.len();
        signature.encode(&mut self.entry);
        if self.entry.len() > len {
            self.hash_state.update(&self.entry[len..]);
        }
        Ok(())
    }

    /// Get an iterator over all known signers of the document.
    pub fn signed_by(&self) -> std::slice::Iter<Identity> {
        self.signed_by.iter()
    }

    /// Get the length of the entry (minus document hash & field name) when it is fully encoded.
    pub fn len(&self) -> usize {
        self.entry.len()
    }

    /// Get the length of the encoded value only (without signatures).
    pub fn entry_len(&self) -> usize {
        self.entry_len
    }

    /// Turn the entry into its component parts.
    pub fn to_parts(self) -> (Hash, String, Vec<u8>) {
        (self.doc, self.field, self.entry)
    }

    /// Get the Hash of the Entry as it currently is. Note that adding additional signatures 
    /// will change the Hash.
    pub fn hash(&self) -> Hash {
        self.hash_state.get_hash()
    }

    /// Get the Hash of the parent document for the Entry.
    pub fn doc_hash(&self) -> &Hash {
        &self.doc
    }

    /// Get the field for this entry.
    pub fn field(&self) -> &str {
        self.field.as_str()
    }

    /// Retrieve the value stored inside the entry as a `ValueRef`. This value has the same 
    /// lifetime as the Entry; it can be converted to a `Value` if it needs to outlast the 
    /// Entry.
    pub fn value(&self) -> ValueRef {
        super::decode::read_value_ref(&mut &self.entry[..]).unwrap()
    }

}

pub fn from_raw(
    hash: &Hash,
    doc_hash: Hash,
    field: &str,
    data: Vec<u8>,
    entry_len: usize
    ) -> io::Result<Entry>
{
    if entry_len > data.len() {
        return Err(io::Error::new(InvalidData, "Entry length greater than raw data length"));
    }
    let mut hash_state = HashState::new(1).unwrap(); // Shouldn't fail if version == 1
    let mut temp_vec = Vec::new();
    super::encode::write_value(&mut temp_vec, &Value::from(doc_hash.clone()));
    hash_state.update(&temp_vec[..]);
    temp_vec.clear();
    super::encode::write_value(&mut temp_vec, &Value::from(field.clone()));
    hash_state.update(&temp_vec[..]);
    hash_state.update(&data[..entry_len]);
    let entry_hash = hash_state.get_hash();

    // Get & validate signatures
    let mut signed_by = Vec::new();
    {
        let mut index = &mut &data[entry_len..];
        while index.len() > 0 {
            let signature = crypto::Signature::decode(&mut index)
                .map_err(|_e| io::Error::new(InvalidData, "Invalid signature in raw entry"))?;
            if !signature.verify(&entry_hash) {
                return Err(io::Error::new(InvalidData, "Signatures did not validate for entry"));
            }
            signed_by.push(signature.signed_by().clone());
        }
    }

    // Finish updating the hash state
    hash_state.update(&data[entry_len..]);
    if hash != &hash_state.get_hash() {
        return Err(io::Error::new(InvalidData, "Raw entry doesn't match stored hash"));
    }
    Ok(Entry {
        doc: doc_hash,
        field: field.to_string(),
        hash_state,
        entry_hash,
        entry_len,
        entry: data,
        signed_by,
    })
}


