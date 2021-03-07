use std::{collections::BTreeMap, convert::{TryInto, TryFrom}};

use de::FogDeserializer;
use element::Parser;

use crate::*;
use crate::error::{Error, Result};
use crate::{
    compress::{CompressType, Compress},
    validator::{Validator, Checklist, ValidatorContext},
};
use serde::{Serialize, Deserialize};

pub struct NoSchema;

impl NoSchema {

    /// Encode a [`NewDocument`], getting the resulting Document's hash and fully encoded format. 
    /// Fails if the internal data isn't actually valid fog-pack, which can sometimes happen with a 
    /// bad Serialize implementation for the data.
    pub fn encode_new_doc(doc: NewDocument) -> Result<(Hash, Vec<u8>)> {
        let split = doc.split();
        if !split.hash_raw.is_empty() {
            return Err(Error::SchemaMismatch { actual: split.hash_raw.try_into().ok(), expected: None });
        }
        let types = BTreeMap::new();
        let parser = Parser::new(split.data);
        let (parser, _) = Validator::Any.validate(&ValidatorContext::new(&types), parser, Checklist::new())?;
        parser.finish()?;
        let (hash, doc, compression) = doc.complete();
        let compression = match compression {
            None => {
                Compress::General { algorithm: 0, level: 3 }
            },
            Some(None) => {
                Compress::None
            },
            Some(Some(level)) => {
                Compress::General { algorithm: 0, level, }
            },
        };

        Ok((hash, compress_doc(doc, &compression)))
    }

    /// Re-encode a validated [`Document`], getting the resulting Document's hash and fully encoded 
    /// format.
    pub fn encode_doc(doc: Document) -> Result<(Hash, Vec<u8>)> {
        let split = doc.split();
        if !split.hash_raw.is_empty() {
            return Err(Error::SchemaMismatch { actual: split.hash_raw.try_into().ok(), expected: None });
        }
        let (hash, doc, compression) = doc.complete();
        let compression = match compression {
            None => {
                Compress::General { algorithm: 0, level: 3 }
            },
            Some(None) => {
                Compress::None
            },
            Some(Some(level)) => {
                Compress::General { algorithm: 0, level, }
            },
        };
        Ok((hash, compress_doc(doc, &compression)))
    }

    pub fn decode_doc(doc: Vec<u8>) -> Result<Document> {
        // Check for hash
        let split = SplitDoc::split(&doc)?;
        if !split.hash_raw.is_empty() {
            return Err(Error::SchemaMismatch { actual: split.hash_raw.try_into().ok(), expected: None });
        }

        // Decompress
        let doc = Document::new(decompress_doc(doc, &Compress::None)?)?;

        // Validate
        let types = BTreeMap::new();
        let parser = Parser::new(doc.data());
        let (parser, _) = Validator::Any.validate(&ValidatorContext::new(&types), parser, Checklist::new())?;
        parser.finish()?;
        
        Ok(doc)
    }
}

fn compress_doc(doc: Vec<u8>, compression: &Compress) -> Vec<u8> {
    if let Compress::None = compression { return doc; }
    let split = SplitDoc::split(&doc).unwrap();
    let header_len = doc.len() - split.data.len() - split.signature_raw.len();
    let max_len = zstd_safe::compress_bound(split.data.len());
    let mut compress = Vec::with_capacity(doc.len() + max_len - split.data.len());
    compress.extend_from_slice(&doc[..header_len]);
    match compression.compress(compress, split.data) {
        Ok(mut compress) => {
            let data_len = (compress.len() - header_len).to_le_bytes();
            compress[header_len-3] = data_len[0];
            compress[header_len-2] = data_len[1];
            compress[header_len-1] = data_len[2];
            compress.extend_from_slice(split.signature_raw);
            compress
        },
        Err(()) => doc,
    }
}

fn decompress_doc(compress: Vec<u8>, compression: &Compress) -> Result<Vec<u8>> {
    // Gather info from compressed vec
    let split = SplitDoc::split(&compress)?;
    let marker = CompressType::try_from(split.compress_raw)
        .map_err(|m| Error::BadHeader(format!("unrecognized compression marker 0x{:x}", m)))?;
    if let CompressType::NoCompress = marker { return Ok(compress); }
    let header_len = compress.len() - split.data.len() - split.signature_raw.len();

    // Decompress
    let mut doc = Vec::new();
    doc.extend_from_slice(&compress[..header_len]);
    let mut doc = compression.decompress(doc, split.data, marker, split.signature_raw.len(), MAX_DOC_SIZE)?;
    let data_len = (doc.len() - header_len).to_le_bytes();
    doc[header_len-3] = data_len[0];
    doc[header_len-2] = data_len[1];
    doc[header_len-1] = data_len[2];
    doc.extend_from_slice(split.signature_raw);
    Ok(doc)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InnerSchema {
    description: String,
    doc: Validator,
    doc_compress: Compress,
    entries: BTreeMap<String, Validator>,
    entries_compress: BTreeMap<String, Compress>,
    name: String,
    types: BTreeMap<String, Validator>,
    version: Integer,
}

pub struct Schema {
    inner: InnerSchema,
}

impl Schema {
    pub fn new(buf: Vec<u8>) -> Result<Self> {
        let mut de = FogDeserializer::new(&buf);
        let inner = InnerSchema::deserialize(&mut de)?;
        de.finish()?;

        Ok(Self {
            inner
        })
    }
}
