//! Serialized data optionally adhering to a schema.
//!
//! Documents are created by taking a serializable struct and calling
//! [`NewDocument::new`] with it, along with an optional Hash of the schema the document will
//! adhere to. Once created, it can be signed and the compression setting can be chosen. To create
//! a complete, verified document, it must be passed to [`NoSchema`][crate::schema::NoSchema] or a
//! [`Schema`][crate::schema::Schema], as appropriate.
//!
//! In addition to direct creation, two additional builder structs are available.
//! [`VecDocumentBuilder`] can be used to take a long iterator and create many documents that are
//! arrays of the serialized items in the iterator. The builder produces documents 512 kiB in size
//! or lower. This is useful for serializing large lists that don't fit in the Document maximum
//! size limit of 1 MiB. [`AsyncVecDocumentBuilder`] does the same, but for asynchronous Streams.
//!

use crate::{compress::CompressType, de::FogDeserializer, ser::FogSerializer, MAX_DOC_SIZE};
use crate::{
    element::serialize_elem,
    error::{Error, Result},
};
use byteorder::{LittleEndian, ReadBytesExt};
use fog_crypto::{
    hash::{Hash, HashState},
    identity::{Identity, IdentityKey},
};
use futures_core::{ready, FusedStream, Stream};
use pin_project_lite::pin_project;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use std::{
    convert::TryFrom,
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

/// Attempt to get the schema for a raw document. Fails if the raw byte slice doesn't conform to 
/// the right format, or if the hash is invalid.
pub fn get_doc_schema(doc: &[u8]) -> Result<Option<Hash>> {
    let hash_raw = SplitDoc::split(doc)?.hash_raw;
    if hash_raw.is_empty() {
        Ok(None)
    }
    else {
        Ok(Some(hash_raw.try_into()?))
    }
}

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
    this_hash: Hash,
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
        self.this_hash = self.hash_state.hash();
        Ok(self)
    }

    /// Get what the document's hash will be, given its current state
    fn hash(&self) -> &Hash {
        &self.this_hash
    }

    fn split(&self) -> SplitDoc {
        SplitDoc::split(&self.buf).unwrap()
    }

    fn data(&self) -> &[u8] {
        self.split().data
    }

    fn complete(self) -> (Hash, Vec<u8>, Option<Option<u8>>) {
        (self.this_hash, self.buf, self.set_compress)
    }
}

#[derive(Clone, Debug)]
struct VecDocumentInner {
    done: bool,
    ser: FogSerializer,
    item_buf: Vec<u8>,
    schema: Option<Hash>,
    signer: Option<IdentityKey>,
    set_compress: Option<Option<u8>>,
}

impl VecDocumentInner {
    fn new(schema: Option<&Hash>) -> Self {
        Self {
            done: false,
            ser: FogSerializer::default(),
            item_buf: Vec::new(),
            schema: schema.cloned(),
            signer: None,
            set_compress: None,
        }
    }

    fn new_ordered(schema: Option<&Hash>) -> Self {
        Self {
            done: false,
            ser: FogSerializer::with_params(true),
            item_buf: Vec::new(),
            schema: schema.cloned(),
            signer: None,
            set_compress: None,
        }
    }

    fn compression(mut self, setting: Option<u8>) -> Self {
        self.set_compress = Some(setting);
        self
    }

    fn sign(mut self, key: &IdentityKey) -> Self {
        self.signer = Some(key.clone());
        self
    }

    fn data_len(&self) -> usize {
        // Precalculate the target size, and don't go past it:
        // - 5 bytes from the header base
        // - N bytes from the schema hash
        // - N bytes at most from the signature
        // - 4 bytes at most from the array element
        let header_len = self.schema.as_ref().map_or(5, |h| 5 + h.as_ref().len());
        let sign_len = self.signer.as_ref().map_or(0, |k| k.max_signature_size());
        (MAX_DOC_SIZE >> 1) - header_len - sign_len - 4
    }

    fn next_doc(
        &mut self,
        data_len: usize,
        prev_len: usize,
        mut array_len: usize,
    ) -> Result<Option<NewDocument>> {
        if !self.ser.buf.is_empty() {
            // If we have excess data, lop it off and hold it for later copying
            if self.ser.buf.len() > data_len {
                self.item_buf.extend_from_slice(&self.ser.buf[prev_len..]);
                self.ser.buf.truncate(prev_len);
                array_len -= 1;
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
            // Move any lopped off data back into the serializer. If we have no lopped off data,
            // then we are out of stuff to write and can terminate
            self.ser.buf.clear();
            if !self.item_buf.is_empty() {
                self.ser.buf.extend_from_slice(&self.item_buf);
                self.item_buf.clear();
            } else {
                self.done = true;
            }
            Ok(Some(doc))
        } else {
            self.done = true;
            Ok(None)
        }
    }
}

/// An iterator adapter for building many Documents.
///
/// Frequently, fog-pack's 1 MiB size limit can pose problems with large data sets. Generally,
/// these data sets can be treated as large arrays of relatively small data objects. This adaptor
/// can take an iterator over any set of data objects, and will produce a series of Documents that
/// are under the size limit.
///
/// For the asynchronous version that works on streams, see
/// [`AsyncVecDocumentBuilder`][AsyncVecDocumentBuilder].
#[derive(Clone, Debug)]
pub struct VecDocumentBuilder<I>
where
    I: Iterator,
    <I as Iterator>::Item: Serialize,
{
    iter: std::iter::Fuse<I>,
    inner: VecDocumentInner,
}

impl<I> VecDocumentBuilder<I>
where
    I: Iterator,
    <I as Iterator>::Item: Serialize,
{
    pub fn new(iter: I, schema: Option<&Hash>) -> Self {
        Self {
            iter: iter.fuse(),
            inner: VecDocumentInner::new(schema),
        }
    }

    pub fn new_ordered(iter: I, schema: Option<&Hash>) -> Self {
        Self {
            iter: iter.fuse(),
            inner: VecDocumentInner::new_ordered(schema),
        }
    }

    /// Override the default compression settings for all produced Documents. `None` will disable
    /// compression. `Some(level)` will compress with the provided level as the setting for the
    /// algorithm.
    pub fn compression(mut self, setting: Option<u8>) -> Self {
        self.inner = self.inner.compression(setting);
        self
    }

    /// Sign the all produced documents from this point onward.
    pub fn sign(mut self, key: &IdentityKey) -> Self {
        self.inner = self.inner.sign(key);
        self
    }

    fn next_doc(&mut self) -> Result<Option<NewDocument>> {
        let data_len = self.inner.data_len();

        let mut prev_len = self.inner.ser.buf.len();
        let mut array_len = !self.inner.ser.buf.is_empty() as usize;
        while self.inner.ser.buf.len() <= data_len {
            let item = if let Some(item) = self.iter.next() {
                item
            } else {
                break;
            };
            prev_len = self.inner.ser.buf.len();
            item.serialize(&mut self.inner.ser)?;
            array_len += 1;
        }

        self.inner.next_doc(data_len, prev_len, array_len)
    }
}

impl<I> Iterator for VecDocumentBuilder<I>
where
    I: Iterator,
    <I as Iterator>::Item: Serialize,
{
    type Item = Result<NewDocument>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.inner.done {
            return None;
        }
        let result = self.next_doc();
        if result.is_err() {
            self.inner.done = true;
        }
        result.transpose()
    }
}

pin_project! {
    /// An stream adapter for building many Documents.
    ///
    /// Frequently, fog-pack's 1 MiB size limit can pose problems with large data sets. Generally,
    /// these data sets can be treated as large arrays of relatively small data objects. This adaptor
    /// can take a stream over any set of data objects, and will produce a series of Documents
    /// that are under the size limit.
    ///
    /// For the synchronous version that works on iterators, see
    /// [`AsyncVecDocumentBuilder`][AsyncVecDocumentBuilder].
    #[must_use = "streams do nothing unless polled"]
    pub struct AsyncVecDocumentBuilder<St>
        where
            St: Stream,
            St::Item: Serialize,
    {
        #[pin]
        stream: St,
        inner: VecDocumentInner,
        array_len: usize,
    }
}

impl<St> fmt::Debug for AsyncVecDocumentBuilder<St>
where
    St: Stream + fmt::Debug,
    St::Item: Serialize + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("AsyncVecDocumentBuilder")
            .field("stream", &self.stream)
            .field("inner", &self.stream)
            .field("array_len", &self.array_len)
            .finish()
    }
}

impl<St> AsyncVecDocumentBuilder<St>
where
    St: Stream,
    St::Item: Serialize,
{
    pub fn new(stream: St, schema: Option<&Hash>) -> Self {
        Self {
            stream,
            inner: VecDocumentInner::new(schema),
            array_len: 0,
        }
    }

    pub fn new_ordered(stream: St, schema: Option<&Hash>) -> Self {
        Self {
            stream,
            inner: VecDocumentInner::new_ordered(schema),
            array_len: 0,
        }
    }

    /// Override the default compression settings for all produced Documents. `None` will disable
    /// compression. `Some(level)` will compress with the provided level as the setting for the
    /// algorithm.
    pub fn compression(mut self, setting: Option<u8>) -> Self {
        self.inner = self.inner.compression(setting);
        self
    }

    /// Sign the all produced documents from this point onward.
    pub fn sign(mut self, key: &IdentityKey) -> Self {
        self.inner = self.inner.sign(key);
        self
    }
}

impl<St> FusedStream for AsyncVecDocumentBuilder<St>
where
    St: Stream + FusedStream,
    St::Item: Serialize,
{
    fn is_terminated(&self) -> bool {
        self.inner.done && self.stream.is_terminated()
    }
}

impl<St> Stream for AsyncVecDocumentBuilder<St>
where
    St: Stream,
    St::Item: Serialize,
{
    type Item = Result<NewDocument>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<NewDocument>>> {
        let mut this = self.project();
        if this.inner.done {
            return Poll::Ready(None);
        }
        Poll::Ready(loop {
            // Our loop is simple: get data, and if none is available, we're done.
            if let Some(item) = ready!(this.stream.as_mut().poll_next(cx)) {
                // We got the next item. Try serializing it.
                let prev_len = this.inner.ser.buf.len();
                if let Err(e) = item.serialize(&mut this.inner.ser) {
                    this.inner.done = true;
                    break Some(Err(e));
                }
                *this.array_len += 1;

                // If we have enough data to make a document, try to do so and return the result.
                let data_len = this.inner.data_len();
                if this.inner.ser.buf.len() > data_len {
                    let res = this.inner.next_doc(data_len, prev_len, *this.array_len);
                    *this.array_len = !this.inner.ser.buf.is_empty() as usize;
                    if res.is_err() {
                        this.inner.done = true;
                    }
                    break res.transpose();
                }
            } else {
                // We yield one last document (maybe)
                if !this.inner.ser.buf.is_empty() {
                    let data_len = this.inner.data_len();
                    let res =
                        this.inner
                            .next_doc(data_len, this.inner.ser.buf.len(), *this.array_len);
                    *this.array_len = !this.inner.ser.buf.is_empty() as usize;
                    this.inner.done = true;
                    break res.transpose();
                } else {
                    break None;
                }
            }
        })
    }
}

/// A new Document that has not yet been validated.
///
/// This struct acts like a Document, but cannot be decoded until it has passed through either a
/// [`Schema`][crate::schema::Schema] or through [`NoSchema`][crate::schema::NoSchema].
#[derive(Clone, Debug)]
pub struct NewDocument(DocumentInner);

impl NewDocument {
    fn new_from<F>(schema: Option<&Hash>, encoder: F) -> Result<Self>
    where
        F: FnOnce(Vec<u8>) -> Result<Vec<u8>>,
    {
        // Create the header
        let mut buf: Vec<u8> = vec![CompressType::None.into()];
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
        let this_hash = doc_hash.clone();

        Ok(NewDocument(DocumentInner {
            buf,
            hash_state,
            this_hash,
            schema_hash: schema.cloned(),
            doc_hash,
            set_compress: None,
            signer: None,
        }))
    }

    /// Create a new Entry from any serializable data, a key, and the Hash of the parent document.
    pub fn new<S: Serialize>(data: S, schema: Option<&Hash>) -> Result<Self> {
        Self::new_from(schema, |buf| {
            // Encode the data
            let mut ser = FogSerializer::from_vec(buf, false);
            data.serialize(&mut ser)?;
            Ok(ser.finish())
        })
    }

    /// Create a new Entry from any serializable data whose keys are all ordered. For structs, this
    /// means all fields are declared in lexicographic order. For maps, this means a `BTreeMap`
    /// type must be used, whose keys are ordered such that they serialize to lexicographically
    /// ordered strings. All sub-structs and sub-maps must be similarly ordered.
    pub fn new_ordered<S: Serialize>(data: S, schema: Option<&Hash>) -> Result<Self> {
        Self::new_from(schema, |buf| {
            // Encode the data
            let mut ser = FogSerializer::from_vec(buf, true);
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
    pub fn hash(&self) -> &Hash {
        self.0.hash()
    }

    pub(crate) fn data(&self) -> &[u8] {
        self.0.data()
    }
}

/// Holds serialized data optionally adhering to a schema.
///
/// A Document holds a piece of serialized data, which may be deserialized by calling
/// [`deserialize`][Document::deserialize]. If it adheres to a schema, Entries may also be attached
/// to it, in accordance with the schema.
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
        let this_hash = hash_state.hash();

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
            this_hash,
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
    pub fn hash(&self) -> &Hash {
        self.0.hash()
    }

    /// Attempt to deserialize the data into anything implementing `Deserialize`.
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
    use rand::Rng;
    use std::mem;
    use std::ops;

    use super::*;

    #[test]
    fn create_new() {
        let new_doc = NewDocument::new(&1u8, None).unwrap();
        assert!(new_doc.schema_hash().is_none());
        let expected_hash = Hash::new(&[0u8, 1u8]);
        assert_eq!(new_doc.hash(), &expected_hash);
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
        assert_eq!(doc.hash(), &expected_hash);
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
        assert_eq!(&doc_hash, doc.hash());
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

        let mut builder = VecDocumentBuilder::new(
            std::iter::repeat(Example {
                a: 234235,
                b: "Ok".into(),
            }),
            None,
        );
        let mut docs = Vec::new();
        for _ in 0..4 {
            let iter = builder.next();
            let result = iter.unwrap();
            let doc = result.unwrap();
            docs.push(doc);
        }
        assert!(docs.iter().all(|doc| {
            let len = doc.0.buf.len();
            len <= (MAX_DOC_SIZE >> 1) && len > (MAX_DOC_SIZE >> 2)
        }));
    }

    #[test]
    fn vec_document_encode_all() {
        #[derive(Clone, Serialize)]
        struct Example {
            a: u32,
            b: String,
        }

        let iter = std::iter::repeat(Example {
            a: 23456,
            b: "Ok".into(),
        })
        .take(MAX_DOC_SIZE + 12);
        let builder = VecDocumentBuilder::new(iter, None);
        let docs = builder.collect::<Result<Vec<NewDocument>>>().unwrap();
        assert!(docs.iter().take(docs.len() - 1).all(|doc| {
            let len = doc.0.buf.len();
            len <= (MAX_DOC_SIZE >> 1) && len > (MAX_DOC_SIZE >> 2)
        }));
        assert!(!docs.last().unwrap().data().is_empty());
    }

    pub trait Generate {
        fn generate<R: Rng>(rng: &mut R) -> Self;
    }

    impl Generate for () {
        fn generate<R: Rng>(_: &mut R) -> Self {}
    }

    impl Generate for bool {
        fn generate<R: Rng>(rng: &mut R) -> Self {
            rng.gen_bool(0.5)
        }
    }

    macro_rules! impl_generate {
        ($ty:ty) => {
            impl Generate for $ty {
                fn generate<R: Rng>(rng: &mut R) -> Self {
                    rng.gen()
                }
            }
        };
    }

    impl_generate!(u8);
    impl_generate!(u16);
    impl_generate!(u32);
    impl_generate!(u64);
    impl_generate!(u128);
    impl_generate!(usize);
    impl_generate!(i8);
    impl_generate!(i16);
    impl_generate!(i32);
    impl_generate!(i64);
    impl_generate!(i128);
    impl_generate!(isize);
    impl_generate!(f32);
    impl_generate!(f64);

    macro_rules! impl_tuple {
        () => {};
        ($first:ident, $($rest:ident,)*) => {
            impl<$first: Generate, $($rest: Generate,)*> Generate for ($first, $($rest,)*) {
                fn generate<R: Rng>(rng: &mut R) -> Self {
                    ($first::generate(rng), $($rest::generate(rng),)*)
                }
            }

            impl_tuple!($($rest,)*);
        };
    }

    impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11,);

    macro_rules! impl_array {
        () => {};
        ($len:literal, $($rest:literal,)*) => {
            impl<T: Generate> Generate for [T; $len] {
                fn generate<R: Rng>(rng: &mut R) -> Self {
                    let mut result = mem::MaybeUninit::<Self>::uninit();
                    let result_ptr = result.as_mut_ptr().cast::<T>();
                    #[allow(clippy::reversed_empty_ranges)]
                    for i in 0..$len {
                        unsafe {
                            result_ptr.add(i).write(T::generate(rng));
                        }
                    }
                    unsafe {
                        result.assume_init()
                    }
                }
            }

            impl_array!($($rest,)*);
        }
    }

    impl_array!(
        31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9,
        8, 7, 6, 5, 4, 3, 2, 1, 0,
    );

    impl<T: Generate> Generate for Option<T> {
        fn generate<R: Rng>(rng: &mut R) -> Self {
            if rng.gen_bool(0.5) {
                Some(T::generate(rng))
            } else {
                None
            }
        }
    }

    pub fn generate_vec<R: Rng, T: Generate>(rng: &mut R, range: ops::Range<usize>) -> Vec<T> {
        let len = rng.gen_range(range.start, range.end);
        let mut result = Vec::with_capacity(len);
        for _ in 0..len {
            result.push(T::generate(rng));
        }
        result
    }

    #[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct Address {
        pub x0: u8,
        pub x1: u8,
        pub x2: u8,
        pub x3: u8,
    }

    impl Generate for Address {
        fn generate<R: Rng>(rand: &mut R) -> Self {
            Self {
                x0: rand.gen(),
                x1: rand.gen(),
                x2: rand.gen(),
                x3: rand.gen(),
            }
        }
    }

    #[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct Log {
        pub address: Address,
        pub identity: String,
        pub userid: String,
        pub date: String,
        pub request: String,
        pub code: u16,
        pub size: u64,
    }

    impl Generate for Log {
        fn generate<R: Rng>(rand: &mut R) -> Self {
            const USERID: [&str; 9] = [
                "-", "alice", "bob", "carmen", "david", "eric", "frank", "george", "harry",
            ];
            const MONTHS: [&str; 12] = [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ];
            const TIMEZONE: [&str; 25] = [
                "-1200", "-1100", "-1000", "-0900", "-0800", "-0700", "-0600", "-0500", "-0400",
                "-0300", "-0200", "-0100", "+0000", "+0100", "+0200", "+0300", "+0400", "+0500",
                "+0600", "+0700", "+0800", "+0900", "+1000", "+1100", "+1200",
            ];
            let date = format!(
                "{}/{}/{}:{}:{}:{} {}",
                rand.gen_range(1, 29),
                MONTHS[rand.gen_range(0, 12)],
                rand.gen_range(1970, 2022),
                rand.gen_range(0, 24),
                rand.gen_range(0, 60),
                rand.gen_range(0, 60),
                TIMEZONE[rand.gen_range(0, 25)],
            );
            const CODES: [u16; 63] = [
                100, 101, 102, 103, 200, 201, 202, 203, 204, 205, 206, 207, 208, 226, 300, 301,
                302, 303, 304, 305, 306, 307, 308, 400, 401, 402, 403, 404, 405, 406, 407, 408,
                409, 410, 411, 412, 413, 414, 415, 416, 417, 418, 421, 422, 423, 424, 425, 426,
                428, 429, 431, 451, 500, 501, 502, 503, 504, 505, 506, 507, 508, 510, 511,
            ];
            const METHODS: [&str; 5] = ["GET", "POST", "PUT", "UPDATE", "DELETE"];
            const ROUTES: [&str; 7] = [
                "/favicon.ico",
                "/css/index.css",
                "/css/font-awsome.min.css",
                "/img/logo-full.svg",
                "/img/splash.jpg",
                "/api/login",
                "/api/logout",
            ];
            const PROTOCOLS: [&str; 4] = ["HTTP/1.0", "HTTP/1.1", "HTTP/2", "HTTP/3"];
            let request = format!(
                "{} {} {}",
                METHODS[rand.gen_range(0, 5)],
                ROUTES[rand.gen_range(0, 7)],
                PROTOCOLS[rand.gen_range(0, 4)],
            );
            Self {
                address: Address::generate(rand),
                identity: "-".into(),
                userid: USERID[rand.gen_range(0, USERID.len())].into(),
                date,
                request,
                code: CODES[rand.gen_range(0, CODES.len())],
                size: rand.gen_range(0, 100_000_000),
            }
        }
    }

    #[test]
    fn logs_encode() {
        // Generate a whole pile of log items
        let mut rng = rand::thread_rng();
        const LOGS: usize = 10_000;
        let logs = generate_vec::<_, Log>(&mut rng, LOGS..LOGS + 1);

        // Try to make them into documents
        let builder = VecDocumentBuilder::new(logs.iter(), None);
        let docs = builder.collect::<Result<Vec<NewDocument>>>().unwrap();
        for (index, doc) in docs.iter().enumerate() {
            let mut parser = crate::element::Parser::with_debug(doc.data(), "  ");
            for x in &mut parser {
                x.unwrap();
            }
            println!("Doc #{}: \n{}", index, parser.get_debug().unwrap());
        }
        assert!(docs.iter().take(docs.len() - 1).all(|doc| {
            let len = doc.0.buf.len();
            len <= (MAX_DOC_SIZE >> 1) && len > (MAX_DOC_SIZE >> 2)
        }));
    }

    #[test]
    fn logs_decode() {
        // Generate a whole pile of log items
        let mut rng = rand::thread_rng();
        const LOGS: usize = 10_000;
        let logs = generate_vec::<_, Log>(&mut rng, LOGS..LOGS + 1);

        // Try to make them into documents
        let builder = VecDocumentBuilder::new(logs.iter(), None);
        let mut docs = builder.collect::<Result<Vec<NewDocument>>>().unwrap();

        let docs: Vec<Document> = docs
            .drain(0..)
            .map(|doc| crate::schema::NoSchema::validate_new_doc(doc).unwrap())
            .collect();
        let dec_logs: Vec<Log> = docs
            .iter()
            .map(|doc| doc.deserialize::<Vec<Log>>().unwrap())
            .flatten()
            .collect();
        assert!(dec_logs == logs, "Didn't decode identically")
    }

    #[test]
    fn async_logs_encode() {
        // Generate a whole pile of log items
        let mut rng = rand::thread_rng();
        const LOGS: usize = 20_000;
        let logs = generate_vec::<_, Log>(&mut rng, LOGS..LOGS + 1);

        // Try to make them into documents
        let mut builder =
            AsyncVecDocumentBuilder::new(futures_util::stream::iter(logs.iter()), None);
        use futures_util::StreamExt;
        let docs = futures_executor::block_on(async {
            let mut docs = Vec::new();
            while let Some(result) = builder.next().await {
                match result {
                    Ok(doc) => docs.push(doc),
                    Err(e) => return Err(e),
                }
            }
            Ok(docs)
        })
        .unwrap();
        for (index, doc) in docs.iter().enumerate() {
            let mut parser = crate::element::Parser::with_debug(doc.data(), "  ");
            for x in &mut parser {
                x.unwrap();
            }
            println!("Doc #{}: \n{}", index, parser.get_debug().unwrap());
        }
        println!("A total of {} documents", docs.len());
        assert!(docs.iter().take(docs.len() - 1).all(|doc| {
            let len = doc.0.buf.len();
            len <= (MAX_DOC_SIZE >> 1) && len > (MAX_DOC_SIZE >> 2)
        }));
    }

    #[test]
    fn async_logs_decode() {
        // Generate a whole pile of log items
        let mut rng = rand::thread_rng();
        const LOGS: usize = 20_000;
        let logs = generate_vec::<_, Log>(&mut rng, LOGS..LOGS + 1);

        // Try to make them into documents
        let mut builder =
            AsyncVecDocumentBuilder::new(futures_util::stream::iter(logs.iter()), None);
        use futures_util::StreamExt;
        let mut docs = futures_executor::block_on(async {
            let mut docs = Vec::new();
            while let Some(result) = builder.next().await {
                match result {
                    Ok(doc) => docs.push(doc),
                    Err(e) => return Err(e),
                }
            }
            Ok(docs)
        })
        .unwrap();

        // Parse them
        let docs: Vec<Document> = docs
            .drain(0..)
            .map(|doc| crate::schema::NoSchema::validate_new_doc(doc).unwrap())
            .collect();
        let dec_logs: Vec<Log> = docs
            .iter()
            .map(|doc| doc.deserialize::<Vec<Log>>().unwrap())
            .map(|doc| {
                println!("Document item count = {}", doc.len());
                doc
            })
            .flatten()
            .collect();
        println!("We have a total of {} logs", dec_logs.len());
        println!("We expected a total of {} logs", logs.len());
        assert!(dec_logs == logs, "Didn't decode identically")
    }
}
