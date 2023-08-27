//! Serialized data associated with a parent Document and key string.
//!
//! Entries are created by calling [`NewEntry::new`] with serializable data, the hash of the parent
//! document, and the key string. Once created, they can be signed and have their compression
//! settings chosen. Entries (new or otherwise) are verified and encoded using a
//! [`Schema`][crate::schema::Schema], which should match the schema used by the parent document.

use crate::error::{Error, Result};
use crate::{
    document::Document,
    compress::CompressType,
    de::FogDeserializer,
    element::{serialize_elem, Element},
    ser::FogSerializer,
    MAX_ENTRY_SIZE,
};
use byteorder::{LittleEndian, ReadBytesExt};
use fog_crypto::{
    hash::{Hash, HashState},
    identity::{Identity, IdentityKey},
};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

pub(crate) const ENTRY_PREFIX_LEN: usize = 3;

pub(crate) struct SplitEntry<'a> {
    pub compress_raw: u8,
    pub data: &'a [u8],
    pub signature_raw: &'a [u8],
}

impl<'a> SplitEntry<'a> {
    pub(crate) fn split(buf: &'a [u8]) -> Result<SplitEntry> {
        // Compression marker
        let (&compress_raw, mut buf) = buf.split_first().ok_or(Error::LengthTooShort {
            step: "get compress type",
            actual: 0,
            expected: 1,
        })?;
        // Data length
        let data_len = buf
            .read_u16::<LittleEndian>()
            .map_err(|_| Error::LengthTooShort {
                step: "get data length",
                actual: buf.len(),
                expected: 2,
            })? as usize;
        if data_len > buf.len() {
            return Err(Error::LengthTooShort {
                step: "get document data",
                actual: buf.len(),
                expected: data_len,
            });
        }
        // Data & signature
        let (data, signature_raw) = buf.split_at(data_len);
        Ok(Self {
            compress_raw,
            data,
            signature_raw,
        })
    }
}

/// A reference triplet to an [`Entry`], containing the hash of the entry's parent document, the
/// key string for the entry, and the hash of the entry itself. Note that the entry hash is still
/// formed in a way the includes the parent & key, so changing either means the entry hash would
/// also change.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct EntryRef {
    /// Hash of the parent document
    pub parent: Hash,
    /// Key for the entry
    pub key: String,
    /// Hash of the entry itself
    pub hash: Hash,
}

impl std::fmt::Display for EntryRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}-{}", self.parent, self.key, self.hash)
    }
}

#[derive(Clone, Debug)]
struct EntryInner {
    buf: Vec<u8>,
    /// Working memory for hash calculations. Should only be created by signing or new(), and only
    /// modified & read within signing operations.
    hash_state: Option<HashState>,
    id: EntryRef,
    schema_hash: Hash,
    signer: Option<Identity>,
    set_compress: Option<Option<u8>>,
}

impl EntryInner {

    fn data(&self) -> &[u8] {
        SplitEntry::split(&self.buf).unwrap().data
    }

    /// Get the hash of the Entry's parent [`Document`][crate::document::Document].
    fn parent(&self) -> &Hash {
        &self.id.parent
    }

    /// Get the hash of the [`Schema`][crate::schema::Schema] of the Entry's parent
    /// [`Document`][crate::document::Document].
    fn schema_hash(&self) -> &Hash {
        &self.schema_hash
    }

    /// Get the Entry's string key.
    fn key(&self) -> &str {
        &self.id.key
    }

    /// Get the Identity of the signer of this entry, if the entry is signed.
    fn signer(&self) -> Option<&Identity> {
        self.signer.as_ref()
    }

    /// Get the hash of the complete entry. This can change if the entry is signed again with the
    /// [`sign`][Self::sign] function.
    fn hash(&self) -> &Hash {
        &self.id.hash
    }

    fn reference(&self) -> &EntryRef {
        &self.id
    }

    /// Deserialize the entry's contained data into a value.
    fn deserialize<'de, D: Deserialize<'de>>(&'de self) -> Result<D> {
        let buf = self.data();
        let mut de = FogDeserializer::new(buf);
        D::deserialize(&mut de)
    }

    /// Override the default compression settings. `None` will disable compression. `Some(level)`
    /// will compress with the provided level as the setting for the algorithm.
    fn compression(&mut self, setting: Option<u8>) -> &mut Self {
        self.set_compress = Some(setting);
        self
    }

    /// Set up the hash state for an entry. The data passed in must not include the prefix bytes.
    fn setup_hash_state(parent_hash: Hash, key: &str, data: &[u8]) -> HashState {
        let mut hash_state = HashState::new();
        let mut prefix = Vec::new();
        serialize_elem(&mut prefix, Element::Hash(parent_hash));
        serialize_elem(&mut prefix, Element::Str(key));
        hash_state.update(&prefix);
        hash_state.update(data);
        hash_state
    }

    /// Sign the entry, or or replace the existing signature if one exists already. Fails if the
    /// signature would grow the entry size beyond the maximum allowed. In the event of a failure.
    /// the entry is dropped.
    fn sign(mut self, key: &IdentityKey) -> Result<Self> {

        // If a signature already exists, reload the hash state
        let pre_sign_len = if self.signer.is_some() {
            let split = SplitEntry::split(&self.buf).unwrap();
            let new_len = split.data.len() + ENTRY_PREFIX_LEN;
            self.hash_state = Some(Self::setup_hash_state(self.id.parent.clone(), &self.id.key, split.data));
            new_len
        }
        else {
            self.buf.len()
        };

        // Load the hash state
        if self.hash_state.is_none() {
            let split = SplitEntry::split(&self.buf).unwrap();
            let state = Self::setup_hash_state(self.id.parent.clone(), &self.id.key, split.data);
            self.hash_state = Some(state);
        }
        let hash_state = self.hash_state.as_mut().unwrap();

        // Hash state does not yet contain the signature - thus, it holds the hash we're going to
        // sign
        let entry_hash = hash_state.hash();

        // Sign and check for size violation
        let signature = key.sign(&entry_hash);
        let new_len = pre_sign_len + signature.size();
        if new_len > MAX_ENTRY_SIZE {
            return Err(Error::LengthTooLong {
                max: MAX_ENTRY_SIZE,
                actual: self.buf.len(),
            });
        }

        // Append the signature and update the hasher
        self.buf.resize(pre_sign_len, 0);
        signature.encode_vec(&mut self.buf);
        hash_state.update(&self.buf[pre_sign_len..]);
        self.id.hash = hash_state.hash();
        self.signer = Some(key.id().clone());
        Ok(self)
    }

    fn complete(self) -> (EntryRef, Vec<u8>, Option<Option<u8>>) {
        (self.id, self.buf, self.set_compress)
    }
}

/// A new Entry that has not yet been validated.
///
/// This struct acts like an Entry, but cannot be decoded until it has passed through a
/// [`Schema`][crate::schema::Schema].
#[derive(Clone, Debug)]
pub struct NewEntry(EntryInner);

impl NewEntry {
    fn new_from<F>(key: &str, parent: &Document, encoder: F) -> Result<Self>
    where
        F: FnOnce(Vec<u8>) -> Result<Vec<u8>>,
    {
        // Serialize the data
        let buf: Vec<u8> = vec![CompressType::None.into(), 0u8, 0u8];
        let mut buf = encoder(buf)?;

        // Check the total size and update the data length
        if buf.len() > MAX_ENTRY_SIZE {
            return Err(Error::LengthTooLong {
                max: MAX_ENTRY_SIZE,
                actual: buf.len(),
            });
        }
        let data_len = (buf.len() - ENTRY_PREFIX_LEN).to_le_bytes();
        buf[1] = data_len[0];
        buf[2] = data_len[1];

        // Create and update the Hash state
        let hash_state = EntryInner::setup_hash_state(parent.hash().clone(), key, &buf[ENTRY_PREFIX_LEN..]);
        let this_hash = hash_state.hash();

        let schema_hash = match parent.schema_hash() {
            Some(h) => h.clone(),
            None => return Err(Error::FailValidate(
                    "Entries can only be created for documents that use a schema.".into())),
        };

        Ok(Self(EntryInner {
            buf,
            hash_state: Some(hash_state),
            id: EntryRef {
                parent: parent.hash().clone(),
                key: key.to_owned(),
                hash: this_hash,
            },
            schema_hash,
            signer: None,
            set_compress: None,
        }))
    }

    /// Create a new Entry from any serializable data, a key, and the Hash of the parent document.
    pub fn new<S: Serialize>(key: &str, parent: &Document, data: S) -> Result<Self> {
        Self::new_from(key, parent, |buf| {
            // Serialize the data
            let mut ser = FogSerializer::from_vec(buf, false);
            data.serialize(&mut ser)?;
            Ok(ser.finish())
        })
    }

    /// Create a new Entry from a key, the Hash of the parent document, and any serializable data
    /// whose keys are all ordered. For structs, this means all fields are declared in
    /// lexicographic order. For maps, this means a `BTreeMap` type must be used, whose keys are
    /// ordered such that they serialize to lexicographically ordered strings. All sub-structs and
    /// sub-maps must be similarly ordered.
    pub fn new_ordered<S: Serialize>(data: S, key: &str, parent: &Document) -> Result<Self> {
        Self::new_from(key, parent, |buf| {
            // Serialize the data
            let mut ser = FogSerializer::from_vec(buf, true);
            data.serialize(&mut ser)?;
            Ok(ser.finish())
        })
    }

    /// Override the default compression settings. `None` will disable compression. `Some(level)`
    /// will compress with the provided level as the setting for the algorithm.
    pub fn compression(mut self, setting: Option<u8>) -> Self {
        self.0.compression(setting);
        self
    }

    /// Sign the document, or or replace the existing signature if one exists already. Fails if the
    /// signature would grow the document size beyond the maximum allowed.
    pub fn sign(self, key: &IdentityKey) -> Result<Self> {
        Ok(Self(self.0.sign(key)?))
    }

    /// Get what the document's hash will be, given its current state
    pub fn hash(&self) -> &Hash {
        self.0.hash()
    }

    pub(crate) fn data(&self) -> &[u8] {
        self.0.data()
    }

    /// Get the hash of the Entry's parent [`Document`][crate::document::Document].
    pub fn parent(&self) -> &Hash {
        self.0.parent()
    }

    /// Get the hash of the [`Schema`][crate::schema::Schema] of the Entry's parent
    /// [`Document`][crate::document::Document].
    pub fn schema_hash(&self) -> &Hash {
        self.0.schema_hash()
    }

    /// Get the Entry's string key.
    pub fn key(&self) -> &str {
        self.0.key()
    }

    /// Get a [`EntryRef`] containing a full reference to the entry.
    pub fn reference(&self) -> &EntryRef {
        self.0.reference()
    }

}

/// Holds serialized data associated with a parent document and a key string.
///
/// An Entry holds a piece of serialized data, which may be deserialized by calling
/// [`deserialize`][Entry::deserialize].
#[derive(Clone, Debug)]
pub struct Entry(EntryInner);

impl Entry {

    pub(crate) fn from_new(entry: NewEntry) -> Entry {
        Self(entry.0)
    }

    pub(crate) fn trusted_new(buf: Vec<u8>, key: &str, parent: &Document, entry: &Hash) -> Result<Self> {
        if buf.len() > MAX_ENTRY_SIZE {
            return Err(Error::LengthTooLong {
                max: MAX_ENTRY_SIZE,
                actual: buf.len(),
            });
        }

        let split = SplitEntry::split(&buf)?;

        let signer = if !split.signature_raw.is_empty() {
            let unverified =
                fog_crypto::identity::UnverifiedSignature::try_from(split.signature_raw)?;
            Some(unverified.signer().clone())
        }
        else {
            None
        };

        let schema_hash = match parent.schema_hash() {
            Some(h) => h.clone(),
            None => return Err(Error::FailValidate(
                    "Entries can only be created for documents that use a schema.".into())),
        };

        Ok(Self(EntryInner {
            buf,
            hash_state: None,
            id: EntryRef {
                parent: parent.hash().to_owned(),
                key: key.to_owned(),
                hash: entry.to_owned(),
            },
            schema_hash,
            signer,
            set_compress: None,
        }))
    }

    pub(crate) fn new(buf: Vec<u8>, key: &str, parent: &Document) -> Result<Self> {
        if buf.len() > MAX_ENTRY_SIZE {
            return Err(Error::LengthTooLong {
                max: MAX_ENTRY_SIZE,
                actual: buf.len(),
            });
        }

        let split = SplitEntry::split(&buf)?;

        let mut hash_state = EntryInner::setup_hash_state(parent.hash().clone(), key, split.data);
        let entry_hash = hash_state.hash();
        if !split.signature_raw.is_empty() { hash_state.update(split.signature_raw); }
        let this_hash = hash_state.hash();

        let signer = if !split.signature_raw.is_empty() {
            let unverified =
                fog_crypto::identity::UnverifiedSignature::try_from(split.signature_raw)?;
            let verified = unverified.verify(&entry_hash)?;
            Some(verified.signer().clone())
        } else {
            None
        };

        let schema_hash = match parent.schema_hash() {
            Some(h) => h.clone(),
            None => return Err(Error::FailValidate(
                    "Entries can only be created for documents that use a schema.".into())),
        };

        Ok(Self(EntryInner {
            buf,
            hash_state: Some(hash_state),
            id: EntryRef {
                parent: parent.hash().to_owned(),
                key: key.to_owned(),
                hash: this_hash,
            },
            schema_hash,
            signer,
            set_compress: None,
        }))
    }

    pub(crate) fn data(&self) -> &[u8] {
        self.0.data()
    }

    /// Find all hashes in this entry and return them.
    pub fn find_hashes(&self) -> Vec<Hash> {
        crate::find_hashes(self.data())
    }

    /// Get the hash of the Entry's parent [`Document`][crate::document::Document].
    pub fn parent(&self) -> &Hash {
        self.0.parent()
    }

    /// Get the hash of the [`Schema`][crate::schema::Schema] of the Entry's parent
    /// [`Document`][crate::document::Document].
    pub fn schema_hash(&self) -> &Hash {
        self.0.schema_hash()
    }

    /// Get the Entry's string key.
    pub fn key(&self) -> &str {
        self.0.key()
    }

    /// Get a [`EntryRef`] containing a full reference to the entry.
    pub fn reference(&self) -> &EntryRef {
        self.0.reference()
    }

    /// Get the Identity of the signer of this entry, if the entry is signed.
    pub fn signer(&self) -> Option<&Identity> {
        self.0.signer()
    }

    /// Get the hash of the complete entry. This can change if the entry is signed again with the
    /// [`sign`][Self::sign] function.
    pub fn hash(&self) -> &Hash {
        self.0.hash()
    }

    /// Deserialize the entry's contained data into a value.
    pub fn deserialize<'de, D: Deserialize<'de>>(&'de self) -> Result<D> {
        self.0.deserialize()
    }

    /// Override the default compression settings. `None` will disable compression. `Some(level)`
    /// will compress with the provided level as the setting for the algorithm.
    pub fn compression(mut self, setting: Option<u8>) -> Self {
        self.0.compression(setting);
        self
    }

    /// Sign the entry, or or replace the existing signature if one exists already. Fails if the
    /// signature would grow the entry size beyond the maximum allowed. In the event of a failure.
    /// the entry is unmodified.
    pub fn sign(self, key: &IdentityKey) -> Result<Self> {
        Ok(Self(self.0.sign(key)?))
    }

    pub(crate) fn complete(self) -> (EntryRef, Vec<u8>, Option<Option<u8>>) {
        self.0.complete()
    }
}
