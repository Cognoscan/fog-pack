use crate::error::{Error, Result};
use crate::{compress::CompressType, de::FogDeserializer, ser::FogSerializer, MAX_DOC_SIZE};
use byteorder::{LittleEndian, ReadBytesExt};
use fog_crypto::{
    hash::{Hash, HashState},
    identity::{Identity, IdentityKey},
};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

// Header format:
//  1. Compression Type marker
//  2. If schema is used: one byte indicating length of hash (must be 127 or
//      lower), then the schema hash.
//  3. 3-byte length of data
//  4. The data
//  5. The optional signature
//
//  If compressed, only the data portion is compressed, and the 3-byte length is updated
//  accordingly

pub(crate) struct SplitDoc<'a> {
    pub compress_raw: u8,
    pub hash_raw: &'a [u8],
    pub data: &'a [u8],
    pub signature_raw: &'a [u8],
}

impl<'a> SplitDoc<'a> {
    pub(crate) fn split(buf: &'a [u8]) -> Result<SplitDoc> {
        let (&compress_raw, buf) = buf.split_first().ok_or_else(|| Error::LengthTooShort {
            step: "get compress type",
            actual: 0,
            expected: 1,
        })?;
        let (hash_len, buf) = buf.split_first().ok_or_else(|| Error::LengthTooShort {
            step: "get hash length",
            actual: 0,
            expected: 1,
        })?;
        let hash_len = *hash_len as usize;
        if hash_len > 127 {
            return Err(Error::BadHeader(format!(
                "Hash length must be 0-127, marked as {}",
                hash_len
            )));
        }
        if buf.len() < hash_len + 3 {
            return Err(Error::LengthTooShort {
                step: "get hash then data length",
                actual: buf.len(),
                expected: hash_len + 3,
            });
        }
        let (hash_raw, mut buf) = buf.split_at(hash_len);
        let data_len = buf.read_u24::<LittleEndian>().unwrap() as usize; // Checked earlier
        if data_len > buf.len() {
            return Err(Error::LengthTooShort {
                step: "get document data",
                actual: buf.len(),
                expected: data_len,
            });
        }
        let (data, signature_raw) = buf.split_at(data_len);
        Ok(Self {
            compress_raw,
            hash_raw,
            data,
            signature_raw,
        })
    }
}

#[derive(Clone, Debug)]
pub struct NewDocument {
    buf: Vec<u8>,
    hash_state: HashState,
    schema_hash: Option<Hash>,
    doc_hash: Hash,
    has_signature: bool,
    set_compress: Option<Option<u8>>,
}

impl NewDocument {
    pub fn new<S: Serialize>(data: S, schema: Option<&Hash>) -> Result<Self> {
        // Create the header
        let mut buf: Vec<u8> = vec![CompressType::NoCompress.into()];
        if let Some(ref hash) = schema {
            let hash_len = hash.as_ref().len();
            assert!(hash_len < 128);
            buf.push(hash_len as u8);
            buf.extend_from_slice(hash.as_ref());
        } else {
            buf.push(0u8);
        }
        buf.extend_from_slice(&[0, 0, 0]);
        let start = buf.len();

        // Encode the data
        let mut ser = FogSerializer::from_vec(buf, false);
        data.serialize(&mut ser)?;
        let mut buf = ser.finish();

        if buf.len() > MAX_DOC_SIZE {
            return Err(Error::LengthTooLong {
                max: MAX_DOC_SIZE,
                actual: buf.len(),
            });
        }
        // Write out the data length
        let data_len = (buf.len() - start).to_le_bytes();
        buf[start - 3] = data_len[0];
        buf[start - 2] = data_len[1];
        buf[start - 1] = data_len[2];

        // Set up the hasher
        let mut hash_state = HashState::new();
        match schema {
            None => hash_state.update(&[0u8]),
            Some(hash) => hash_state.update(hash),
        }
        hash_state.update(&buf[start..]);
        let doc_hash = hash_state.hash();

        Ok(NewDocument {
            buf,
            hash_state,
            schema_hash: schema.cloned(),
            doc_hash,
            set_compress: None,
            has_signature: false,
        })
    }

    /// Get the hash of the schema this document adheres to.
    pub fn schema_hash(&self) -> Option<&Hash> {
        self.schema_hash.as_ref()
    }

    /// Override the default compression settings. `None` will disable compression. `Some(level)`
    /// will compress with the provided level as the setting for the algorithm.
    pub fn compression(&mut self, setting: Option<u8>) -> &mut Self {
        self.set_compress = Some(setting);
        self
    }

    /// Sign the document, or or replace the existing signature if one exists already. Fails if the
    /// signature would grow the document size beyond the maximum allowed. In the event of a
    /// failure, the document is dropped.
    pub fn sign(mut self, key: &IdentityKey) -> Result<Self> {
        // Sign and check for size violation
        let signature = key.sign(&self.doc_hash);
        let new_len = if self.has_signature {
            self.buf.len() - self.split().signature_raw.len() + signature.size()
        } else {
            self.buf.len() + signature.size()
        };
        if new_len > MAX_DOC_SIZE {
            return Err(Error::LengthTooLong {
                max: MAX_DOC_SIZE,
                actual: self.buf.len(),
            });
        }

        // Erase previous signature & recalculate hash, if needed
        if self.has_signature {
            let split = SplitDoc::split(&self.buf).unwrap();
            let new_len = split.hash_raw.len() + split.data.len() + 5;
            let mut hash_state = HashState::new();
            match self.schema_hash {
                None => hash_state.update(&[0u8]),
                Some(ref hash) => hash_state.update(hash),
            }
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

    pub(crate) fn split(&self) -> SplitDoc {
        SplitDoc::split(&self.buf).unwrap()
    }

    pub(crate) fn data(&self) -> &[u8] {
        self.split().data
    }

    pub(crate) fn complete(self) -> (Hash, Vec<u8>, Option<Option<u8>>) {
        (self.hash_state.finalize(), self.buf, self.set_compress)
    }
}

#[derive(Clone, Debug)]
pub struct Document {
    buf: Vec<u8>,
    schema_hash: Option<Hash>,
    hash_state: HashState,
    doc_hash: Hash,
    signer: Option<Identity>,
    set_compress: Option<Option<u8>>,
}

impl Document {
    /// Create the document from a raw byte vec without fully verifying it.
    /// After creation, if the data is untrusted, you must still run it through a validator
    pub(crate) fn new(buf: Vec<u8>) -> Result<Self> {
        if buf.len() > MAX_DOC_SIZE {
            return Err(Error::LengthTooLong {
                max: MAX_DOC_SIZE,
                actual: buf.len(),
            });
        }

        let split = SplitDoc::split(&buf)?;
        let schema_hash = if split.hash_raw.len() > 0 {
            Some(Hash::try_from(split.hash_raw)?)
        } else {
            None
        };

        let mut hash_state = HashState::new();
        match schema_hash {
            None => hash_state.update(&[0u8]),
            Some(ref hash) => hash_state.update(hash.as_ref()),
        }
        hash_state.update(split.data);
        let doc_hash = hash_state.hash();
        hash_state.update(split.signature_raw);

        let signer = if split.signature_raw.len() > 0 {
            let unverified =
                fog_crypto::identity::UnverifiedSignature::try_from(split.signature_raw)?;
            let verified = unverified.verify(&doc_hash)?;
            Some(verified.signer().clone())
        } else {
            None
        };

        Ok(Self {
            buf,
            schema_hash,
            hash_state,
            doc_hash,
            signer,
            set_compress: None,
        })
    }

    pub(crate) fn split(&self) -> SplitDoc {
        SplitDoc::split(&self.buf).unwrap()
    }

    pub(crate) fn data(&self) -> &[u8] {
        self.split().data
    }

    /// Get the hash of the schema this document adheres to.
    pub fn schema_hash(&self) -> Option<&Hash> {
        self.schema_hash.as_ref()
    }

    /// Get the Identity of the signer of this document, if the document is signed.
    pub fn signer(&self) -> Option<&Identity> {
        self.signer.as_ref()
    }

    /// Get the hash of the complete document. This can change if the document is signed again with
    /// the [`sign`] function.
    pub fn hash(&self) -> Hash {
        self.hash_state.hash()
    }

    pub fn deserialize<'de, D: Deserialize<'de>>(&'de self) -> Result<D> {
        let buf = self.data();
        let mut de = FogDeserializer::new(buf);
        D::deserialize(&mut de)
    }

    /// Override the default compression settings. `None` will disable compression. `Some(level)`
    /// will compress with the provided level as the setting for the algorithm. This only has
    /// meaning when the document is re-encoded.
    pub fn compression(&mut self, setting: Option<u8>) -> &mut Self {
        self.set_compress = Some(setting);
        self
    }

    /// Sign the document, or or replace the existing signature if one exists already. Fails if the
    /// signature would grow the document size beyond the maximum allowed.
    pub fn sign(mut self, key: &IdentityKey) -> Result<Self> {
        // Sign and check for size violation
        let signature = key.sign(&self.doc_hash);
        let new_len = if self.signer.is_some() {
            self.buf.len() - self.split().signature_raw.len() + signature.size()
        } else {
            self.buf.len() + signature.size()
        };
        if new_len > MAX_DOC_SIZE {
            return Err(Error::LengthTooLong {
                max: MAX_DOC_SIZE,
                actual: self.buf.len(),
            });
        }

        // Erase previous signature & recalculate hash, if needed
        if self.signer.is_some() {
            let split = SplitDoc::split(&self.buf).unwrap();
            let new_len = split.hash_raw.len() + split.data.len() + 5;
            let mut hash_state = HashState::new();
            match self.schema_hash {
                None => hash_state.update(&[0u8]),
                Some(ref hash) => hash_state.update(hash),
            }
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn create_new() {
        let new_doc = NewDocument::new(&1u8, None).unwrap();
        assert!(new_doc.schema_hash().is_none());
        let expected_hash = Hash::new(&[0u8, 1u8]);
        assert_eq!(new_doc.hash(), expected_hash);
        assert_eq!(new_doc.data(), &[1u8]);
        let expected = vec![0u8, 0u8, 1u8, 0u8, 0u8, 1u8];
        let (doc_hash, doc_vec, doc_compress) = new_doc.complete();
        assert_eq!(doc_hash, expected_hash);
        assert_eq!(doc_vec, expected);
        assert_eq!(doc_compress, None);
    }

    #[test]
    fn create_doc() {
        let encoded = vec![0u8, 0u8, 1u8, 0u8, 0u8, 1u8];
        let doc = Document::new(encoded.clone()).unwrap();
        let expected_hash = Hash::new(&[0u8, 1u8]);
        assert_eq!(doc.hash(), expected_hash);
        assert_eq!(doc.data(), &[1u8]);
        let val: u8 = doc.deserialize().unwrap();
        assert_eq!(val, 1u8);
        let (doc_hash, doc_vec, doc_compress) = doc.complete();
        assert_eq!(doc_hash, expected_hash);
        assert_eq!(doc_vec, encoded);
        assert_eq!(doc_compress, None);
    }

    #[test]
    fn new_doc_limits() {
        use serde_bytes::Bytes;
        let vec = vec![0xAAu8; MAX_DOC_SIZE]; // Make it too big
        let key = IdentityKey::new_temp(&mut rand::rngs::OsRng);
        // 5 bytes for the Bin element header, 5 for the document header

        // Should be large enough to include the signature
        let sign_len = key.sign(&Hash::new(b"meh")).size();
        let new_doc =
            NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE - 9 - sign_len)]), None).unwrap();
        let signed_doc = new_doc.clone().sign(&key).unwrap();
        assert_eq!(
            &signed_doc.buf[..(signed_doc.buf.len() - sign_len)],
            &new_doc.buf[..]
        );

        // Should be just large enough
        let new_doc = NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE - 10)]), None).unwrap();
        let mut expected = vec![0x00, 0x00];
        expected.extend_from_slice(&(MAX_DOC_SIZE - 6).to_le_bytes()[..3]);
        assert_eq!(new_doc.buf[0..5], expected);
        let new_doc = NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE - 9)]), None).unwrap();
        let mut expected = vec![0x00, 0x00];
        expected.extend_from_slice(&(MAX_DOC_SIZE - 5).to_le_bytes()[..3]);
        assert_eq!(new_doc.buf[0..5], expected);
        new_doc.sign(&key).unwrap_err(); // We have no space for a signature

        // Should fail by virtue of being too large
        NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE - 8)]), None).unwrap_err();
        NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE - 7)]), None).unwrap_err();
        NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE - 6)]), None).unwrap_err();
        NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE - 5)]), None).unwrap_err();
    }

    #[test]
    fn new_doc_schema_limits() {
        use serde_bytes::Bytes;
        let vec = vec![0xAAu8; MAX_DOC_SIZE]; // Make it too big
        let key = IdentityKey::new_temp(&mut rand::rngs::OsRng);
        let schema_hash = Hash::new(b"I'm totally a real schema, trust me");
        let hash_len = schema_hash.as_ref().len();
        // 5 bytes for the Bin element header, 5 for the document header

        // Should be large enough to include the signature
        let sign_len = key.sign(&Hash::new(b"meh")).size();
        let new_doc = NewDocument::new(
            Bytes::new(&vec[..(MAX_DOC_SIZE - 9 - sign_len - hash_len)]),
            Some(&schema_hash),
        )
        .unwrap();
        let signed_doc = new_doc.clone().sign(&key).unwrap();
        assert_eq!(
            &signed_doc.buf[..(signed_doc.buf.len() - sign_len)],
            &new_doc.buf[..]
        );

        // Should be 1 byte below max - check against expected format
        let new_doc = NewDocument::new(
            Bytes::new(&vec[..(MAX_DOC_SIZE - 10 - hash_len)]),
            Some(&schema_hash),
        )
        .unwrap();
        let mut expected = vec![0x00, hash_len as u8];
        expected.extend_from_slice(schema_hash.as_ref());
        expected.extend_from_slice(&(MAX_DOC_SIZE - 6 - hash_len).to_le_bytes()[..3]);
        assert_eq!(new_doc.buf[0..(5 + hash_len)], expected);

        // Should be at max - check against expected format
        let new_doc = NewDocument::new(
            Bytes::new(&vec[..(MAX_DOC_SIZE - 9 - hash_len)]),
            Some(&schema_hash),
        )
        .unwrap();
        let mut expected = vec![0x00, hash_len as u8];
        expected.extend_from_slice(schema_hash.as_ref());
        expected.extend_from_slice(&(MAX_DOC_SIZE - 5 - hash_len).to_le_bytes()[..3]);
        assert_eq!(new_doc.buf[0..(5 + hash_len)], expected);
        new_doc.sign(&key).unwrap_err(); // We have no space for a signature

        // Should fail by virtue of being too large
        NewDocument::new(
            Bytes::new(&vec[..(MAX_DOC_SIZE - 8 - hash_len)]),
            Some(&schema_hash),
        )
        .unwrap_err();
        NewDocument::new(
            Bytes::new(&vec[..(MAX_DOC_SIZE - 7 - hash_len)]),
            Some(&schema_hash),
        )
        .unwrap_err();
        NewDocument::new(
            Bytes::new(&vec[..(MAX_DOC_SIZE - 6 - hash_len)]),
            Some(&schema_hash),
        )
        .unwrap_err();
        NewDocument::new(
            Bytes::new(&vec[..(MAX_DOC_SIZE - 5 - hash_len)]),
            Some(&schema_hash),
        )
        .unwrap_err();
    }

    /*
        #[test]
        fn doc_limits() {
            let key = IdentityKey::new_temp(&mut rand::rngs::OsRng);

            // Create the encoded data
            let mut enc = vec![0u8, 0u8];
            let data_len = MAX_DOC_SIZE - 5;
            enc.extend_from_slice(&data_len.to_le_bytes()[..3]);
            enc.push(0xc6);
            enc.extend_from_slice(&(data_len-5).to_le_bytes()[..4]);
            enc.resize(data_len+10, 0xAA);
            // 5 bytes for the Bin element header, 5 for the document header

            // Should be large enough to include the signature
            let sign_len = key.sign(&Hash::new(b"meh")).size();
            let new_doc = NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE-10-sign_len)]), None).unwrap();
            let signed_doc = new_doc.clone().sign(&key).unwrap();
            assert_eq!(&signed_doc.buf[..(signed_doc.buf.len()-sign_len)], &new_doc.buf[..]);

            // Should be just large enough
            let new_doc = NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE-11)]), None).unwrap();
            let mut expected = vec![0x00, 0x00];
            expected.extend_from_slice(&(MAX_DOC_SIZE-6).to_le_bytes()[..3]);
            assert_eq!(new_doc.buf[0..5], expected);
            let new_doc = NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE-10)]), None).unwrap();
            let mut expected = vec![0x00, 0x00];
            expected.extend_from_slice(&(MAX_DOC_SIZE-5).to_le_bytes()[..3]);
            assert_eq!(new_doc.buf[0..5], expected);
            new_doc.sign(&key).unwrap_err(); // We have no space for a signature

            // Should fail by virtue of being too large
            NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE-9)]), None).unwrap_err();
            NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE-8)]), None).unwrap_err();
            NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE-7)]), None).unwrap_err();
            NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE-6)]), None).unwrap_err();
        }

        #[test]
        fn doc_schema_limits() {
            use serde_bytes::Bytes;
            let vec = vec![0xAAu8; MAX_DOC_SIZE]; // Make it too big
            let key = IdentityKey::new_temp(&mut rand::rngs::OsRng);
            let schema_hash = Hash::new(b"I'm totally a real schema, trust me");
            let hash_len = schema_hash.as_ref().len();
            // 5 bytes for the Bin element header, 5 for the document header

            // Should be large enough to include the signature
            let sign_len = key.sign(&Hash::new(b"meh")).size();
            let new_doc = NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE-10-sign_len-hash_len)]), Some(&schema_hash)).unwrap();
            let signed_doc = new_doc.clone().sign(&key).unwrap();
            assert_eq!(&signed_doc.buf[..(signed_doc.buf.len()-sign_len)], &new_doc.buf[..]);

            // Should be just large enough
            let new_doc = NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE-11-hash_len)]), Some(&schema_hash)).unwrap();
            let mut expected = vec![0x00, hash_len as u8];
            expected.extend_from_slice(schema_hash.as_ref());
            expected.extend_from_slice(&(MAX_DOC_SIZE-6-hash_len).to_le_bytes()[..3]);
            assert_eq!(new_doc.buf[0..(5+hash_len)], expected);
            let new_doc = NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE-10-hash_len)]), Some(&schema_hash)).unwrap();
            let mut expected = vec![0x00, hash_len as u8];
            expected.extend_from_slice(schema_hash.as_ref());
            expected.extend_from_slice(&(MAX_DOC_SIZE-5-hash_len).to_le_bytes()[..3]);
            assert_eq!(new_doc.buf[0..(5+hash_len)], expected);
            new_doc.sign(&key).unwrap_err(); // We have no space for a signature

            // Should fail by virtue of being too large
            NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE-9-hash_len)]), Some(&schema_hash)).unwrap_err();
            NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE-8-hash_len)]), Some(&schema_hash)).unwrap_err();
            NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE-7-hash_len)]), Some(&schema_hash)).unwrap_err();
            NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE-6-hash_len)]), Some(&schema_hash)).unwrap_err();
        }
    */

    #[test]
    fn sign_roundtrip() {
        let key = IdentityKey::new_temp(&mut rand::rngs::OsRng);
        let new_doc = NewDocument::new(&1u8, None).unwrap().sign(&key).unwrap();
        assert_eq!(new_doc.data(), &[1u8]);
        let (doc_hash, doc_vec, _) = new_doc.complete();
        let doc = Document::new(doc_vec).unwrap();
        let val: u8 = doc.deserialize().unwrap();
        assert_eq!(doc_hash, doc.hash());
        assert_eq!(val, 1u8);
        assert_eq!(doc.signer().unwrap(), key.id());
    }
}
