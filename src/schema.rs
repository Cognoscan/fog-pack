//! Schema, which can be used to encode/decode a document or entry, while verifying its
//! contents.
//!
//! A schema is always decoded from a [`Document`][crate::document::Document]. Schema documents can
//! be easily built from scratch using a [`SchemaBuilder`].
//!
use std::{
    collections::BTreeMap,
    convert::{TryFrom, TryInto},
};

use crate::document::*;
use crate::entry::*;
pub use compress::*;
use element::Parser;
use query::{NewQuery, Query};

use crate::error::{Error, Result};
use crate::validator::{Checklist, DataChecklist, Validator};
use crate::*;
use serde::{Deserialize, Serialize};

#[inline]
fn compress_is_default(val: &Compress) -> bool {
    if let Compress::General { algorithm, level } = val {
        *algorithm == ALGORITHM_ZSTD && *level == 3
    } else {
        false
    }
}

#[inline]
fn int_is_zero(v: &Integer) -> bool {
    v.as_u64().map(|v| v == 0).unwrap_or(false)
}

#[inline]
fn u8_is_zero(v: &u8) -> bool {
    *v == 0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InnerSchema {
    doc: Validator, // required
    #[serde(skip_serializing_if = "String::is_empty", default)]
    description: String,
    #[serde(skip_serializing_if = "compress_is_default", default)]
    doc_compress: Compress,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    entries: BTreeMap<String, EntrySchema>,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    name: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    types: BTreeMap<String, Validator>,
    #[serde(skip_serializing_if = "int_is_zero", default)]
    version: Integer,
    #[serde(skip_serializing_if = "u8_is_zero", default)]
    max_regex: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EntrySchema {
    entry: Validator, // required
    #[serde(skip_serializing_if = "compress_is_default", default)]
    compress: Compress,
}

/// Validation for documents without a schema.
///
/// Not all documents adhere to a schema, but they must still be verified for correctness and be
/// optionally compressed on encoding. This `NoSchema` struct acts like a Schema to accomplish
/// this.
///
/// As schemaless documents cannot have attached entries, `NoSchema` does not do any entry
/// encode/decode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoSchema;

impl NoSchema {
    /// Validate a [`NewDocument`], turning it into a [`Document`]. Fails if the internal data 
    /// isn't actually valid fog-pack, which can sometimes happen with a bad Serialize 
    /// implementation for the data.
    pub fn validate_new_doc(doc: NewDocument) -> Result<Document> {
        // Check that this document doesn't have a schema
        if let Some(schema) = doc.schema_hash() {
            return Err(Error::SchemaMismatch {
                actual: Some(schema.to_owned()),
                expected: None,
            });
        }

        // Cursory validation of the data
        let types = BTreeMap::new();
        let parser = Parser::new(doc.data());
        let (parser, _) = Validator::Any.validate(&types, parser, None)?;
        parser.finish()?;

        Ok(Document::from_new(doc))
    }

    /// Re-encode a validated [`Document`], returning the resulting Document's hash and fully encoded
    /// format.
    pub fn encode_doc(doc: Document) -> Result<(Hash, Vec<u8>)> {
        // Check that this document doesn't have a schema
        if let Some(schema) = doc.schema_hash() {
            return Err(Error::SchemaMismatch {
                actual: Some(schema.to_owned()),
                expected: None,
            });
        }

        // Compress the document
        let (hash, doc, compression) = doc.complete();
        let compression = match compression {
            None => Compress::General {
                algorithm: 0,
                level: 3,
            },
            Some(None) => Compress::None,
            Some(Some(level)) => Compress::General {
                algorithm: 0,
                level,
            },
        };
        Ok((hash, compress_doc(doc, &compression)))
    }

    /// Decode a document that doesn't have a schema.
    pub fn decode_doc(doc: Vec<u8>) -> Result<Document> {
        // Check for hash
        let split = SplitDoc::split(&doc)?;
        if !split.hash_raw.is_empty() {
            return Err(Error::SchemaMismatch {
                actual: split.hash_raw.try_into().ok(),
                expected: None,
            });
        }

        // Decompress
        let doc = Document::new(decompress_doc(doc, &Compress::None)?)?;

        // Validate
        let types = BTreeMap::new();
        let parser = Parser::new(doc.data());
        let (parser, _) = Validator::Any.validate(&types, parser, None)?;
        parser.finish()?;

        Ok(doc)
    }

    /// Decode a Document, skipping any checks of the data. This should only be run when the raw
    /// document has definitely been passed through validation before, i.e. if it is stored in a
    /// local database after going through [`encode_doc`][Self::encode_doc].
    pub fn trusted_decode_doc(doc: Vec<u8>) -> Result<Document> {
        // Check for hash
        let split = SplitDoc::split(&doc)?;
        if !split.hash_raw.is_empty() {
            return Err(Error::SchemaMismatch {
                actual: split.hash_raw.try_into().ok(),
                expected: None,
            });
        }

        // Decompress
        let doc = Document::new(decompress_doc(doc, &Compress::None)?)?;
        Ok(doc)
    }
}

fn compress_doc(doc: Vec<u8>, compression: &Compress) -> Vec<u8> {
    // Skip if we aren't compressing
    if let Compress::None = compression {
        return doc;
    }

    // Gather info from the raw document
    let split = SplitDoc::split(&doc).unwrap();
    let header_len = doc.len() - split.data.len() - split.signature_raw.len();
    let max_len = zstd_safe::compress_bound(split.data.len());
    let mut compress = Vec::with_capacity(doc.len() + max_len - split.data.len());
    compress.extend_from_slice(&doc[..header_len]);

    // Compress, update the header, append the signature
    match compression.compress(compress, split.data) {
        Ok(mut compress) => {
            let data_len = (compress.len() - header_len).to_le_bytes();
            compress[0] = CompressType::type_of(compression).into();
            compress[header_len - 3] = data_len[0];
            compress[header_len - 2] = data_len[1];
            compress[header_len - 1] = data_len[2];
            compress.extend_from_slice(split.signature_raw);
            compress
        }
        Err(()) => doc,
    }
}

fn decompress_doc(compress: Vec<u8>, compression: &Compress) -> Result<Vec<u8>> {
    // Gather info from compressed vec
    let split = SplitDoc::split(&compress)?;
    let marker = CompressType::try_from(split.compress_raw)
        .map_err(|m| Error::BadHeader(format!("unrecognized compression marker 0x{:x}", m)))?;
    if let CompressType::None = marker {
        return Ok(compress);
    }
    let header_len = compress.len() - split.data.len() - split.signature_raw.len();

    // Decompress, update the header, append the signature
    let mut doc = Vec::new();
    doc.extend_from_slice(&compress[..header_len]);
    let mut doc = compression.decompress(
        doc,
        split.data,
        marker,
        split.signature_raw.len(),
        MAX_DOC_SIZE,
    )?;
    let data_len = (doc.len() - header_len).to_le_bytes();
    doc[0] = CompressType::None.into();
    doc[header_len - 3] = data_len[0];
    doc[header_len - 2] = data_len[1];
    doc[header_len - 1] = data_len[2];
    doc.extend_from_slice(split.signature_raw);
    Ok(doc)
}

fn compress_entry(entry: Vec<u8>, compression: &Compress) -> Vec<u8> {
    // Skip if we aren't compressing
    if let Compress::None = compression {
        return entry;
    }

    // Gather info from the raw entry
    let split = SplitEntry::split(&entry).unwrap();
    let max_len = zstd_safe::compress_bound(split.data.len());
    let mut compress = Vec::with_capacity(entry.len() + max_len - split.data.len());
    compress.extend_from_slice(&entry[..ENTRY_PREFIX_LEN]);

    // Compress, update the header, append the signature
    match compression.compress(compress, split.data) {
        Ok(mut compress) => {
            let data_len = (compress.len() - ENTRY_PREFIX_LEN).to_le_bytes();
            compress[0] = CompressType::type_of(compression).into();
            compress[1] = data_len[0];
            compress[2] = data_len[1];
            compress.extend_from_slice(split.signature_raw);
            compress
        }
        Err(()) => entry,
    }
}

fn decompress_entry(compress: Vec<u8>, compression: &Compress) -> Result<Vec<u8>> {
    // Gather info from compressed vec
    let split = SplitEntry::split(&compress)?;
    let marker = CompressType::try_from(split.compress_raw)
        .map_err(|m| Error::BadHeader(format!("unrecognized compression marker 0x{:x}", m)))?;
    if let CompressType::None = marker {
        return Ok(compress);
    }

    // Decompress, update the header, append the signature
    let mut entry = Vec::new();
    entry.extend_from_slice(&compress[..ENTRY_PREFIX_LEN]);
    let mut entry = compression.decompress(
        entry,
        split.data,
        marker,
        split.signature_raw.len(),
        MAX_ENTRY_SIZE,
    )?;
    let data_len = (entry.len() - ENTRY_PREFIX_LEN).to_le_bytes();
    entry[0] = CompressType::None.into();
    entry[1] = data_len[0];
    entry[2] = data_len[1];
    entry.extend_from_slice(split.signature_raw);
    Ok(entry)
}

/// Builds schemas up from Validators.
///
/// A schema can be directly made from any document, but it's generally much easier to construct
/// them from [`Validator`][crate::validator::Validator] structs and turn the result into a
/// Document.
#[derive(Clone, Debug)]
pub struct SchemaBuilder {
    inner: InnerSchema,
}

impl SchemaBuilder {
    /// Start building a new schema. Requires the validator to use for any documents adhering to
    /// this schema.
    pub fn new(doc: Validator) -> Self {
        Self {
            inner: InnerSchema {
                doc,
                description: String::default(),
                doc_compress: Compress::default(),
                entries: BTreeMap::new(),
                name: String::default(),
                types: BTreeMap::new(),
                version: Integer::default(),
                max_regex: 0,
            },
        }
    }

    /// Set the schema description. This is only used for documentation purposes.
    pub fn description(mut self, description: &str) -> Self {
        self.inner.description = description.to_owned();
        self
    }

    /// Set the default compression to use for documents adhering to this schema.
    pub fn doc_compress(mut self, doc_compress: Compress) -> Self {
        self.inner.doc_compress = doc_compress;
        self
    }

    /// Add a new entry type to the schema, where `entry` is the key for the entry, `validator`
    /// will be used to validate each entry, and `compress` optionally overrides the default
    /// compression with a specific compression setting.
    pub fn entry_add(
        mut self,
        entry: &str,
        validator: Validator,
        compress: Option<Compress>,
    ) -> Self {
        let compress = compress.unwrap_or_default();
        self.inner.entries.insert(
            entry.to_owned(),
            EntrySchema {
                entry: validator,
                compress,
            },
        );
        self
    }

    /// Set the schema name. This is only used for documentation purposes.
    pub fn name(mut self, name: &str) -> Self {
        self.inner.name = name.to_owned();
        self
    }

    /// Add a new stored type to the schema.
    pub fn type_add(mut self, type_ref: &str, validator: Validator) -> Self {
        self.inner.types.insert(type_ref.to_owned(), validator);
        self
    }

    /// Set the schema version. This is only used for documentation purposes.
    pub fn version<T: Into<Integer>>(mut self, version: T) -> Self {
        self.inner.version = version.into();
        self
    }

    /// Build the Schema, compiling the result into a Document
    pub fn build(self) -> Result<Document> {
        let doc = NewDocument::new(self.inner, None)?;
        NoSchema::validate_new_doc(doc)
    }
}

/// A Schema, which can be used to encode/decode a document or entry, while verifying its
/// contents.
///
/// Schema are decoded from a correctly formatted [`Document`] that describes the format of other
/// documents and their associated entries. They also include recommended compression settings for
/// documents & entries adhering to them, which may include compression dictionaries.
///
/// A schema must come from a Document. To create one directly, use the [`SchemaBuilder`], then
/// decode the resulting Document into a schema.
#[derive(Clone, Debug)]
pub struct Schema {
    hash: Hash,
    inner: InnerSchema,
}

impl Schema {
    /// Attempt to create a schema from a given document. Fails if the document isn't a schema.
    pub fn from_doc(doc: &Document) -> Result<Self> {
        let inner = doc.deserialize()?;
        let hash = doc.hash().clone();
        Ok(Self { hash, inner })
    }

    /// Get the hash of this schema.
    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    /// Validate a [`NewDocument`], turning it into a [`Document`]. Fails if the document doesn't
    /// use this schema, or if it doesn't meet this schema's requirements.
    pub fn validate_new_doc(&self, doc: NewDocument) -> Result<Document> {
        // Check that the document uses this schema
        match doc.schema_hash() {
            Some(hash) if hash == &self.hash => (),
            actual => {
                return Err(Error::SchemaMismatch {
                    actual: actual.cloned(),
                    expected: Some(self.hash.clone()),
                })
            }
        }

        // Validate the data
        let parser = Parser::new(doc.data());
        let (parser, _) = self.inner.doc.validate(&self.inner.types, parser, None)?;
        parser.finish()?;

        Ok(Document::from_new(doc))
    }

    /// Encode a [`Document`], returning the resulting Document's hash and fully encoded format.
    /// Fails if the document doesn't use this schema.
    pub fn encode_doc(&self, doc: Document) -> Result<(Hash, Vec<u8>)> {
        // Check that the document uses this schema
        match doc.schema_hash() {
            Some(hash) if hash == &self.hash => (),
            actual => {
                return Err(Error::SchemaMismatch {
                    actual: actual.cloned(),
                    expected: Some(self.hash.clone()),
                })
            }
        }

        // Compress the document
        let (hash, doc, compression) = doc.complete();
        let doc = match compression {
            None => compress_doc(doc, &self.inner.doc_compress),
            Some(None) => doc,
            Some(Some(level)) => compress_doc(
                doc,
                &Compress::General {
                    algorithm: 0,
                    level,
                },
            ),
        };

        Ok((hash, doc))
    }

    fn check_schema(&self, doc: &[u8]) -> Result<()> {
        // Check that the document uses this schema
        let split = SplitDoc::split(doc)?;
        if split.hash_raw.is_empty() {
            return Err(Error::SchemaMismatch {
                actual: None,
                expected: Some(self.hash.clone()),
            });
        }
        let schema = Hash::try_from(split.hash_raw)
            .map_err(|_| Error::BadHeader("Unable to decode schema hash".into()))?;
        if schema != self.hash {
            Err(Error::SchemaMismatch {
                actual: Some(schema),
                expected: Some(self.hash.clone()),
            })
        } else {
            Ok(())
        }
    }

    /// Decode a document that uses this schema.
    pub fn decode_doc(&self, doc: Vec<u8>) -> Result<Document> {
        self.check_schema(&doc)?;

        // Decompress
        let doc = Document::new(decompress_doc(doc, &self.inner.doc_compress)?)?;

        // Validate
        let parser = Parser::new(doc.data());
        let (parser, _) = self.inner.doc.validate(&self.inner.types, parser, None)?;
        parser.finish()?;

        Ok(doc)
    }

    /// Decode a Document, skipping any checks of the data. This should only be run when the raw
    /// document has definitely been passed through validation before, i.e. if it is stored in a
    /// local database after going through [`encode_doc`][Self::encode_doc].
    pub fn trusted_decode_doc(&self, doc: Vec<u8>) -> Result<Document> {
        self.check_schema(&doc)?;

        // Decompress
        let doc = Document::new(decompress_doc(doc, &Compress::None)?)?;
        Ok(doc)
    }

    /// Validate a [`NewEntry`], turning it into a [`Entry`]. Fails if provided the wrong parent 
    /// document, the parent document doesn't use this schema, or the entry doesn't meet the schema 
    /// requirements. The resulting Entry is stored in a [`DataChecklist`] that must be iterated 
    /// over in order to finish validation.
    pub fn validate_new_entry(&self, entry: NewEntry) -> Result<DataChecklist<Entry>> {
        // Check that the entry's parent document uses this schema
        if entry.schema_hash() != &self.hash {
            return Err(Error::SchemaMismatch {
                actual: Some(entry.schema_hash().clone()),
                expected: Some(self.hash.clone()),
            });
        }

        // Validate the data and generate a checklist of remaining documents to check
        let parser = Parser::new(entry.data());
        let entry_schema = self.inner.entries.get(entry.key()).ok_or_else(|| {
            Error::FailValidate(format!("entry key \"{:?}\" is not in schema", entry.key()))
        })?;
        let checklist = Some(Checklist::new(&self.hash, &self.inner.types));
        let (parser, checklist) =
            entry_schema
                .entry
                .validate(&self.inner.types, parser, checklist)?;
        parser.finish()?;

        Ok(DataChecklist::from_checklist(
                checklist.unwrap(),
                Entry::from_new(entry)
        ))
    }

    /// Encode an [`Entry`], returning the resulting Entry's reference, its fully encoded format, 
    /// and a list of Hashes of the Documents it needs for validation.
    /// Fails if provided the wrong parent document or the parent document doesn't use this schema.
    pub fn encode_entry(&self, entry: Entry) -> Result<(EntryRef, Vec<u8>, Vec<Hash>)> {
        // Check that the entry's parent document uses this schema
        if entry.schema_hash() != &self.hash {
            return Err(Error::SchemaMismatch {
                actual: Some(entry.schema_hash().clone()),
                expected: Some(self.hash.clone()),
            });
        }

        // We re-run validation here just to collect the hashes of documents needed for validation 
        // (i.e. the documents that would need to be provided with this entry if it were 
        // transferred or stored)
        //
        // At some point, it's plausible this could be performed with a more minimal validation 
        // check. 
        let entry_schema = self.inner.entries.get(entry.key()).ok_or_else(|| {
            Error::FailValidate(format!("entry key \"{:?}\" is not in schema", entry.key()))
        })?;
        let parser = Parser::new(entry.data());
        let checklist = Some(Checklist::new(&self.hash, &self.inner.types));
        let (parser, checklist) =
            entry_schema
                .entry
                .validate(&self.inner.types, parser, checklist)?;
        parser.finish()?;
        let needed_docs: Vec<Hash> = checklist.unwrap().iter().map(|(hash, _)| hash).collect();

        // Compress the entry
        let (entry_ref, entry, compression) = entry.complete();
        let entry = match compression {
            None => compress_entry(entry, &entry_schema.compress),
            Some(None) => entry,
            Some(Some(level)) => compress_entry(
                entry,
                &Compress::General {
                    algorithm: 0,
                    level,
                },
            ),
        };

        Ok((entry_ref, entry, needed_docs))
    }

    /// Decode an entry, given the key and parent Hash. Result is in a [`DataChecklist`] that must
    /// be iterated over in order to finish verification and get the resulting Entry.
    pub fn decode_entry(
        &self,
        entry: Vec<u8>,
        key: &str,
        parent: &Document,
    ) -> Result<DataChecklist<Entry>> {
        // Check that the entry's parent document uses this schema
        match parent.schema_hash() {
            Some(hash) if hash == &self.hash => (),
            actual => {
                return Err(Error::SchemaMismatch {
                    actual: actual.cloned(),
                    expected: Some(self.hash.clone()),
                })
            }
        }

        // Find the entry
        let entry_schema = self.inner.entries.get(key).ok_or_else(|| {
            Error::FailValidate(format!("entry key \"{:?}\" is not in schema", key))
        })?;

        // Decompress
        let entry = Entry::new(
            decompress_entry(entry, &entry_schema.compress)?,
            key,
            parent,
        )?;

        // Validate
        let parser = Parser::new(entry.data());
        let checklist = Some(Checklist::new(&self.hash, &self.inner.types));
        let (parser, checklist) =
            entry_schema
                .entry
                .validate(&self.inner.types, parser, checklist)?;
        parser.finish()?;

        Ok(DataChecklist::from_checklist(checklist.unwrap(), entry))
    }

    /// Decode a Entry, skipping most checks of the data. This should only be run when the raw
    /// entry has definitely been passed through validation before, i.e. if it is stored in a
    /// local database after going through [`encode_entry`][Self::encode_entry] or
    /// [`encode_new_entry`][Self::encode_new_entry].
    pub fn trusted_decode_entry(
        &self,
        entry: Vec<u8>,
        key: &str,
        parent: &Document,
        entry_hash: &Hash,
    ) -> Result<Entry> {
        // Check that the entry's parent document uses this schema
        match parent.schema_hash() {
            Some(hash) if hash == &self.hash => (),
            actual => {
                return Err(Error::SchemaMismatch {
                    actual: actual.cloned(),
                    expected: Some(self.hash.clone()),
                })
            }
        }
        // Find the entry
        let entry_schema = self.inner.entries.get(key).ok_or_else(|| {
            Error::FailValidate(format!("entry key \"{:?}\" is not in schema", key))
        })?;

        // Decompress
        let entry = Entry::trusted_new(
            decompress_entry(entry, &entry_schema.compress)?,
            key,
            parent,
            entry_hash,
        )?;
        Ok(entry)
    }

    pub fn encode_query(&self, query: NewQuery) -> Result<Vec<u8>> {
        let key = query.key();
        let entry_schema = self.inner.entries.get(key).ok_or_else(|| {
            Error::FailValidate(format!("entry key \"{:?}\" is not in schema", key))
        })?;
        if entry_schema
            .entry
            .query_check(&self.inner.types, query.validator())
        {
            query.complete(self.inner.max_regex)
        } else {
            Err(Error::FailValidate("Query is not allowed by schema".into()))
        }
    }

    pub fn decode_query(&self, query: Vec<u8>) -> Result<Query> {
        let query = Query::new(query, self.inner.max_regex)?;
        let key = query.key();
        let entry_schema = self.inner.entries.get(key).ok_or_else(|| {
            Error::FailValidate(format!("entry key \"{:?}\" is not in schema", key))
        })?;
        if entry_schema
            .entry
            .query_check(&self.inner.types, query.validator())
        {
            Ok(query)
        } else {
            Err(Error::FailValidate("Query is not allowed by schema".into()))
        }
    }
}
