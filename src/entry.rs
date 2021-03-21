//! Serialized data associated with a parent Document and key string.
//!
//! Entries are created by calling [`NewEntry::new`] with serializable data, the hash of the parent
//! document, and the key string. Once created, they can be signed and have their compression
//! settings chosen. Entries (new or otherwise) are verified and encoded using a
//! [`Schema`][crate::schema::Schema], which should match the schema used by the parent document.

use crate::error::{Error, Result};
use crate::{
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

/// A new Entry that has not yet been validated.
///
/// This struct acts like an Entry, but cannot be decoded until it has passed through a
/// [`Schema`][crate::schema::Schema].
pub struct NewEntry {
    buf: Vec<u8>,
    hash_state: HashState,
    key: String,
    parent_hash: Hash,
    entry_hash: Hash,
    has_signature: bool,
    set_compress: Option<Option<u8>>,
}

impl NewEntry {
    fn new_from<F>(key: &str, parent: &Hash, encoder: F) -> Result<Self>
    where
        F: FnOnce(Vec<u8>) -> Result<Vec<u8>>,
    {
        // Serialize the data
        let buf: Vec<u8> = vec![CompressType::NoCompress.into(), 0u8, 0u8];
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

        // Create and update the Hasher
        let mut hash_state = HashState::new();
        let mut prefix = Vec::new();
        serialize_elem(&mut prefix, Element::Hash(parent.clone()));
        serialize_elem(&mut prefix, Element::Str(key));
        hash_state.update(&prefix);
        hash_state.update(&buf[ENTRY_PREFIX_LEN..]);
        let entry_hash = hash_state.hash();

        Ok(Self {
            buf,
            hash_state,
            key: key.to_owned(),
            parent_hash: parent.to_owned(),
            entry_hash,
            has_signature: false,
            set_compress: None,
        })
    }

    /// Create a new Entry from any serializable data, a key, and the Hash of the parent document.
    pub fn new<S: Serialize>(data: S, key: &str, parent: &Hash) -> Result<Self> {
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
    pub fn new_ordered<S: Serialize>(data: S, key: &str, parent: &Hash) -> Result<Self> {
        Self::new_from(key, parent, |buf| {
            // Serialize the data
            let mut ser = FogSerializer::from_vec(buf, false);
            data.serialize(&mut ser)?;
            Ok(ser.finish())
        })
    }

    /// Override the default compression settings. `None` will disable compression. `Some(level)`
    /// will compress with the provided level as the setting for the algorithm.
    pub fn compression(mut self, setting: Option<u8>) -> Self {
        self.set_compress = Some(setting);
        self
    }

    /// Sign the document, or or replace the existing signature if one exists already. Fails if the
    /// signature would grow the document size beyond the maximum allowed.
    pub fn sign(mut self, key: &IdentityKey) -> Result<Self> {
        // Sign and check for size violation
        let signature = key.sign(&self.entry_hash);
        let new_len = if self.has_signature {
            self.buf.len() - self.split().signature_raw.len() + signature.size()
        } else {
            self.buf.len() + signature.size()
        };
        if new_len > MAX_ENTRY_SIZE {
            return Err(Error::LengthTooLong {
                max: MAX_ENTRY_SIZE,
                actual: self.buf.len(),
            });
        }

        if self.has_signature {
            let split = SplitEntry::split(&self.buf).unwrap();
            let new_len = split.data.len() + ENTRY_PREFIX_LEN;
            let mut hash_state = HashState::new();
            let mut prefix = Vec::new();
            serialize_elem(&mut prefix, Element::Hash(self.parent_hash.clone()));
            serialize_elem(&mut prefix, Element::Str(&self.key));
            hash_state.update(&prefix);
            hash_state.update(split.data);
            self.buf.resize(new_len, 0);
            self.hash_state = hash_state;
        }

        // Append the signature and update the hasher
        let pre_len = self.buf.len();
        signature.encode_vec(&mut self.buf);
        self.hash_state.update(&self.buf[pre_len..]);
        self.has_signature = true;
        Ok(self)
    }

    /// Get what the document's hash will be, given its current state
    pub fn hash(&self) -> Hash {
        self.hash_state.hash()
    }

    pub(crate) fn split(&self) -> SplitEntry {
        SplitEntry::split(&self.buf).unwrap()
    }

    pub(crate) fn data(&self) -> &[u8] {
        self.split().data
    }

    /// Get the hash of the Entry's parent [`Document`][crate::document::Document].
    pub fn parent(&self) -> &Hash {
        &self.parent_hash
    }

    /// Get the Entry's string key.
    pub fn key(&self) -> &str {
        &self.key
    }

    pub(crate) fn complete(self) -> (Hash, Vec<u8>, Option<Option<u8>>) {
        (self.hash_state.finalize(), self.buf, self.set_compress)
    }
}

/// Holds serialized data associated with a parent document and a key string.
///
/// An Entry holds a piece of serialized data, which may be deserialized by calling
/// [`deserialize`][Entry::deserialize].
pub struct Entry {
    buf: Vec<u8>,
    hash_state: HashState,
    key: String,
    parent_hash: Hash,
    entry_hash: Hash,
    signer: Option<Identity>,
    set_compress: Option<Option<u8>>,
}

impl Entry {
    pub(crate) fn new(buf: Vec<u8>, key: &str, parent: &Hash) -> Result<Self> {
        if buf.len() > MAX_ENTRY_SIZE {
            return Err(Error::LengthTooLong {
                max: MAX_ENTRY_SIZE,
                actual: buf.len(),
            });
        }

        let split = SplitEntry::split(&buf)?;

        let mut hash_state = HashState::new();
        let mut prefix = Vec::new();
        serialize_elem(&mut prefix, Element::Hash(parent.clone()));
        serialize_elem(&mut prefix, Element::Str(key));
        hash_state.update(&prefix);
        hash_state.update(split.data);
        let entry_hash = hash_state.hash();
        hash_state.update(split.signature_raw);

        let signer = if !split.signature_raw.is_empty() {
            let unverified =
                fog_crypto::identity::UnverifiedSignature::try_from(split.signature_raw)?;
            let verified = unverified.verify(&entry_hash)?;
            Some(verified.signer().clone())
        } else {
            None
        };

        Ok(Self {
            buf,
            hash_state,
            key: key.to_owned(),
            parent_hash: parent.to_owned(),
            entry_hash,
            signer,
            set_compress: None,
        })
    }

    pub(crate) fn split(&self) -> SplitEntry {
        SplitEntry::split(&self.buf).unwrap()
    }

    pub(crate) fn data(&self) -> &[u8] {
        self.split().data
    }

    /// Get the hash of the Entry's parent [`Document`][crate::document::Document].
    pub fn parent(&self) -> &Hash {
        &self.parent_hash
    }

    /// Get the Entry's string key.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Get the Identity of the signer of this document, if the document is signed.
    pub fn signer(&self) -> Option<&Identity> {
        self.signer.as_ref()
    }

    /// Get the hash of the complete entry. This can change if the entry is signed again with the
    /// [`sign`][Self::sign] function.
    pub fn hash(&self) -> Hash {
        self.hash_state.hash()
    }

    /// Deserialize the entry's contained data into a value.
    pub fn deserialize<'de, D: Deserialize<'de>>(&'de self) -> Result<D> {
        let buf = self.data();
        let mut de = FogDeserializer::new(buf);
        D::deserialize(&mut de)
    }

    /// Override the default compression settings. `None` will disable compression. `Some(level)`
    /// will compress with the provided level as the setting for the algorithm.
    pub fn compression(mut self, setting: Option<u8>) -> Self {
        self.set_compress = Some(setting);
        self
    }

    /// Sign the entry, or or replace the existing signature if one exists already. Fails if the
    /// signature would grow the entry size beyond the maximum allowed. In the event of a failure.
    /// the entry is unmodified.
    pub fn sign(mut self, key: &IdentityKey) -> Result<Self> {
        // Sign and check for size violation
        let signature = key.sign(&self.entry_hash);
        let new_len = if self.signer.is_some() {
            self.buf.len() - self.split().signature_raw.len() + signature.size()
        } else {
            self.buf.len() + signature.size()
        };
        if new_len > MAX_ENTRY_SIZE {
            return Err(Error::LengthTooLong {
                max: MAX_ENTRY_SIZE,
                actual: self.buf.len(),
            });
        }

        if self.signer.is_some() {
            let split = SplitEntry::split(&self.buf).unwrap();
            let new_len = split.data.len() + ENTRY_PREFIX_LEN;
            let mut hash_state = HashState::new();
            let mut prefix = Vec::new();
            serialize_elem(&mut prefix, Element::Hash(self.parent_hash.clone()));
            serialize_elem(&mut prefix, Element::Str(&self.key));
            hash_state.update(&prefix);
            hash_state.update(split.data);
            self.buf.resize(new_len, 0);
            self.hash_state = hash_state;
        }

        // Append the signature and update the hasher
        let pre_len = self.buf.len();
        signature.encode_vec(&mut self.buf);
        self.hash_state.update(&self.buf[pre_len..]);
        self.signer = Some(key.id().clone());
        Ok(self)
    }

    pub(crate) fn complete(self) -> (Hash, Vec<u8>, Option<Option<u8>>) {
        (self.hash_state.finalize(), self.buf, self.set_compress)
    }
}
