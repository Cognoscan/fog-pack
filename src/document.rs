use crate::{element::serialize_elem, error::{Error, Result}};
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
        let (&compress_raw, buf) = buf.split_first().ok_or(Error::LengthTooShort {
            step: "get compress type",
            actual: 0,
            expected: 1,
        })?;
        let (hash_len, buf) = buf.split_first().ok_or(Error::LengthTooShort {
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
struct DocumentInner {
    buf: Vec<u8>,
    hash_state: HashState,
    schema_hash: Option<Hash>,
    doc_hash: Hash,
    signer: Option<Identity>,
    set_compress: Option<Option<u8>>,
}

impl DocumentInner {
    fn signer(&self) -> Option<&Identity> {
        self.signer.as_ref()
    }

    /// Get the hash of the schema this document adheres to.
    fn schema_hash(&self) -> Option<&Hash> {
        self.schema_hash.as_ref()
    }

    /// Override the default compression settings. `None` will disable compression. `Some(level)`
    /// will compress with the provided level as the setting for the algorithm.
    fn compression(&mut self, setting: Option<u8>) -> &mut Self {
        self.set_compress = Some(setting);
        self
    }

    /// Sign the document, or or replace the existing signature if one exists already. Fails if the
    /// signature would grow the document size beyond the maximum allowed.
    fn sign(mut self, key: &IdentityKey) -> Result<Self> {
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

    /// Get what the document's hash will be, given its current state
    fn hash(&self) -> Hash {
        self.hash_state.hash()
    }

    fn split(&self) -> SplitDoc {
        SplitDoc::split(&self.buf).unwrap()
    }

    fn data(&self) -> &[u8] {
        self.split().data
    }

    fn complete(self) -> (Hash, Vec<u8>, Option<Option<u8>>) {
        (self.hash_state.finalize(), self.buf, self.set_compress)
    }
}

pub struct VecDocumentBuilder<I>
where
    I: Iterator,
    <I as Iterator>::Item: Serialize,
{
    iter: std::iter::Fuse<I>,
    done: bool,
    ser: FogSerializer,
    item_buf: Vec<u8>,
    schema: Option<Hash>,
    signer: Option<IdentityKey>,
    set_compress: Option<Option<u8>>,
}

impl<I> VecDocumentBuilder<I>
where
    I: Iterator,
    <I as Iterator>::Item: Serialize,
{
    pub fn new(iter: I, schema: Option<&Hash>) -> Self {
        Self {
            iter: iter.fuse(),
            done: false,
            ser: FogSerializer::default(),
            item_buf: Vec::new(),
            schema: schema.cloned(),
            signer: None,
            set_compress: None,
        }
    }

    /// Override the default compression settings for all produced Documents. `None` will disable 
    /// compression. `Some(level)` will compress with the provided level as the setting for the 
    /// algorithm.
    pub fn compression(mut self, setting: Option<u8>) -> Self {
        self.set_compress = Some(setting);
        self
    }

    /// Sign the all produced documents from this point onward.
    pub fn sign(mut self, key: &IdentityKey) -> Self {
        self.signer = Some(key.clone());
        self
    }

    fn next_doc(&mut self) -> Result<Option<NewDocument>> {
        // Precalculate the target size, and don't go past it:
        // - 5 bytes from the header base
        // - N bytes from the schema hash
        // - N bytes at most from the signature
        // - 4 bytes at most from the array element
        let header_len = self.schema.as_ref().map_or(5, |h| 5+h.as_ref().len());
        let sign_len = self.signer.as_ref().map_or(0, |k| k.max_signature_size());
        let data_len = (MAX_DOC_SIZE>>1) - header_len - sign_len - 4;

        let mut prev_len = self.ser.buf.len();
        let mut array_len = !self.ser.buf.is_empty() as usize;
        while self.ser.buf.len() < data_len {
            let item = if let Some(item) = self.iter.next() { item } else { break };
            prev_len = self.ser.buf.len();
            item.serialize(&mut self.ser)?;
            array_len +=1;
        }

        if !self.ser.buf.is_empty() {
            // If we have excess data, lop it off and hold it for later copying
            if prev_len != self.ser.buf.len() {
                self.item_buf.extend_from_slice(&self.ser.buf[prev_len..]);
                self.ser.buf.truncate(prev_len);
            }
            // Create the new document
            let doc = NewDocument::new_from(self.schema.as_ref(), |mut buf| {
                serialize_elem(&mut buf, crate::element::Element::Array(array_len));
                buf.extend_from_slice(&self.ser.buf);
                Ok(buf)
            })?;
            let doc = match self.set_compress {
                Some(set_compress) => doc.compression(set_compress),
                None => doc,
            };
            let doc = match self.signer {
                Some(ref signer) => doc.sign(signer)?,
                None => doc,
            };
            // Move any lopped off data back into the serializer
            if !self.item_buf.is_empty() {
                self.ser.buf.clear();
                self.ser.buf.extend_from_slice(&self.item_buf);
                self.item_buf.clear();
            }
            Ok(Some(doc))
        }
        else {
            Ok(None)
        }
    }
}

impl<I> Iterator for VecDocumentBuilder<I>
where
    I: Iterator,
    <I as Iterator>::Item: Serialize,
{
    type Item = Result<NewDocument>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done { return None; }
        let result = self.next_doc();
        if result.is_err() { self.done = true; }
        result.transpose()
    }
}

#[derive(Clone, Debug)]
pub struct NewDocument(DocumentInner);

impl NewDocument {
    pub fn new_from<F>(schema: Option<&Hash>, encoder: F) -> Result<Self>
        where F: FnOnce(Vec<u8>) -> Result<Vec<u8>>
    {
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
        let mut buf = encoder(buf)?;

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

        Ok(NewDocument(DocumentInner {
            buf,
            hash_state,
            schema_hash: schema.cloned(),
            doc_hash,
            set_compress: None,
            signer: None,
        }))
    }

    pub fn new<S: Serialize>(data: S, schema: Option<&Hash>) -> Result<Self> {
        Self::new_from(schema, |buf| {
            // Encode the data
            let mut ser = FogSerializer::from_vec(buf, false);
            data.serialize(&mut ser)?;
            Ok(ser.finish())
        })
    }

    /// Get the hash of the schema this document adheres to.
    pub fn schema_hash(&self) -> Option<&Hash> {
        self.0.schema_hash()
    }

    /// Override the default compression settings. `None` will disable compression. `Some(level)`
    /// will compress with the provided level as the setting for the algorithm.
    pub fn compression(mut self, setting: Option<u8>) -> Self {
        self.0.compression(setting);
        self
    }

    /// Sign the document, or or replace the existing signature if one exists already. Fails if the
    /// signature would grow the document size beyond the maximum allowed. In the event of a
    /// failure, the document is dropped.
    pub fn sign(self, key: &IdentityKey) -> Result<Self> {
        Ok(Self(self.0.sign(key)?))
    }

    /// Get what the document's hash will be, given its current state
    pub fn hash(&self) -> Hash {
        self.0.hash()
    }

    pub(crate) fn data(&self) -> &[u8] {
        self.0.data()
    }
}

#[derive(Clone, Debug)]
pub struct Document(DocumentInner);

impl Document {
    pub(crate) fn from_new(doc: NewDocument) -> Document {
        Self(doc.0)
    }

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
        let schema_hash = if !split.hash_raw.is_empty() {
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

        let signer = if !split.signature_raw.is_empty() {
            let unverified =
                fog_crypto::identity::UnverifiedSignature::try_from(split.signature_raw)?;
            let verified = unverified.verify(&doc_hash)?;
            Some(verified.signer().clone())
        } else {
            None
        };

        Ok(Self(DocumentInner {
            buf,
            schema_hash,
            hash_state,
            doc_hash,
            signer,
            set_compress: None,
        }))
    }

    pub(crate) fn data(&self) -> &[u8] {
        self.0.data()
    }

    /// Get the hash of the schema this document adheres to.
    pub fn schema_hash(&self) -> Option<&Hash> {
        self.0.schema_hash()
    }

    /// Get the Identity of the signer of this document, if the document is signed.
    pub fn signer(&self) -> Option<&Identity> {
        self.0.signer()
    }

    /// Get the hash of the complete document. This can change if the document is signed again with
    /// the [`sign`][Self::sign] function.
    pub fn hash(&self) -> Hash {
        self.0.hash()
    }

    pub fn deserialize<'de, D: Deserialize<'de>>(&'de self) -> Result<D> {
        let buf = self.0.data();
        let mut de = FogDeserializer::new(buf);
        D::deserialize(&mut de)
    }

    /// Override the default compression settings. `None` will disable compression. `Some(level)`
    /// will compress with the provided level as the setting for the algorithm. This only has
    /// meaning when the document is re-encoded.
    pub fn compression(mut self, setting: Option<u8>) -> Self {
        self.0.compression(setting);
        self
    }

    /// Sign the document, or or replace the existing signature if one exists already. Fails if the
    /// signature would grow the document size beyond the maximum allowed.
    pub fn sign(self, key: &IdentityKey) -> Result<Self> {
        Ok(Self(self.0.sign(key)?))
    }

    pub(crate) fn complete(self) -> (Hash, Vec<u8>, Option<Option<u8>>) {
        self.0.complete()
    }
}

#[cfg(test)]
mod test {
    use fog_crypto::lock::DEFAULT_LOCK_VERSION;

    use super::*;

    #[test]
    fn create_new() {
        let new_doc = NewDocument::new(&1u8, None).unwrap();
        assert!(new_doc.schema_hash().is_none());
        let expected_hash = Hash::new(&[0u8, 1u8]);
        assert_eq!(new_doc.hash(), expected_hash);
        assert_eq!(new_doc.data(), &[1u8]);
        let expected = vec![0u8, 0u8, 1u8, 0u8, 0u8, 1u8];
        let (doc_hash, doc_vec, doc_compress) = Document::from_new(new_doc).complete();
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
            &signed_doc.0.buf[..(signed_doc.0.buf.len() - sign_len)],
            &new_doc.0.buf[..]
        );

        // Should be just large enough
        let new_doc = NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE - 10)]), None).unwrap();
        let mut expected = vec![0x00, 0x00];
        expected.extend_from_slice(&(MAX_DOC_SIZE - 6).to_le_bytes()[..3]);
        assert_eq!(new_doc.0.buf[0..5], expected);
        let new_doc = NewDocument::new(Bytes::new(&vec[..(MAX_DOC_SIZE - 9)]), None).unwrap();
        let mut expected = vec![0x00, 0x00];
        expected.extend_from_slice(&(MAX_DOC_SIZE - 5).to_le_bytes()[..3]);
        assert_eq!(new_doc.0.buf[0..5], expected);
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
            &signed_doc.0.buf[..(signed_doc.0.buf.len() - sign_len)],
            &new_doc.0.buf[..]
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
        assert_eq!(new_doc.0.buf[0..(5 + hash_len)], expected);

        // Should be at max - check against expected format
        let new_doc = NewDocument::new(
            Bytes::new(&vec[..(MAX_DOC_SIZE - 9 - hash_len)]),
            Some(&schema_hash),
        )
        .unwrap();
        let mut expected = vec![0x00, hash_len as u8];
        expected.extend_from_slice(schema_hash.as_ref());
        expected.extend_from_slice(&(MAX_DOC_SIZE - 5 - hash_len).to_le_bytes()[..3]);
        assert_eq!(new_doc.0.buf[0..(5 + hash_len)], expected);
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

    #[test]
    fn sign_roundtrip() {
        let key = IdentityKey::new_temp(&mut rand::rngs::OsRng);
        let new_doc = NewDocument::new(&1u8, None).unwrap().sign(&key).unwrap();
        assert_eq!(new_doc.data(), &[1u8]);
        let (doc_hash, doc_vec, _) = Document::from_new(new_doc).complete();
        let doc = Document::new(doc_vec).unwrap();
        let val: u8 = doc.deserialize().unwrap();
        assert_eq!(doc_hash, doc.hash());
        assert_eq!(val, 1u8);
        assert_eq!(doc.signer().unwrap(), key.id());
    }

    #[test]
    fn vec_document_encode() {
        #[derive(Clone, Serialize)]
        struct Example {
            a: u32,
            b: String,
        }

        let mut builder = VecDocumentBuilder::new(std::iter::repeat(Example { a: 234235, b: "Ok".into()}), None);
        let mut docs = Vec::new();
        for _ in 0..4 {
            let iter = builder.next();
            let result = iter.unwrap();
            let doc = result.unwrap();
            docs.push(doc);
        }
        assert!(docs.iter().all(|doc| {
            let len = doc.0.buf.len();
            len <= (MAX_DOC_SIZE>>1) && len > (MAX_DOC_SIZE>>2)
        }));
    }
}
