use std::io;
use std::io::Error;
use std::io::ErrorKind::{InvalidData,Other};
use std::collections::HashMap;

use crypto;
use decode::*;
use encode;
use document::{extract_schema_hash, parse_schema_hash};
use validator::{ValidObj, Validator, ValidatorChecklist};
use {MAX_DOC_SIZE, MAX_ENTRY_SIZE, Value, Entry, Hash, Document, MarkerType, CompressType};
use checklist::{ChecklistItem, EncodeChecklist, DecodeChecklist};
use super::zstd_help;

/**
Struct holding the validation portions of a schema. Can be used for validation of a document or 
entry.

Schema are a special type of [`Document`](./struct.Document.html) that describes 
the format of other documents and their associated entries. They also include 
recommended compression settings for documents adhering to them, and optionally 
may include compression dictionaries for improved compression.

Much like how many file formats start with a "magic number" to indicate what 
their format is, any document adhering to a schema uses the schema's document 
hash in the empty field. For example, a schema may look like:

```json
{
    "": "<Hash(fog-pack Core Schema)>",
    "name": "Simple Schema",
    "req": {
        "title": { "type": "Str", "max_len": 255},
        "text": { "type": "Str" }
    }
}
```

A document that uses this "Basic Schema" would look like:

```json
{
    "": "<Hash(Basic Schema)>",
    "title": "Example Document",
    "text": "This is an example document that meets a schema"
}
```

Schema Document Format
======================

The most important concept in a schema document is the validator. A validator is 
a fog-pack object containing the validation rules for a particular part of a 
document. It can directly define the rules, or be aliased and used throughout 
the schema document. Validators are always one of the base fog-pack value types, 
or allow for several of them. See the Validation Language for more info.

At the top level, a schema is a validator for an object but without support for 
the `in`, `nin`, `comment`, `default`, or `query` optional fields.  Instead, it 
supports a few additional optional fields for documentation, entry validation, 
and compression:

- `name`: A brief string to name the schema.
- `description`: A brief string describing the purpose of the schema.
- `version`: An integer for tracking schema versions.
- `entries`: An object containing validators for each allowed Entry that may be 
	attached to a Document following the schema.
- `types`: An object containing aliased validators that may be referred to 
- anywhere within the schema
- `doc_compress`: Optionally specifies recommended compression settings for 
	Documents using the schema.
- `entries_compress`: Optionally specifies recommended compression settings for 
	entries attached to documents using the schema.

*/
pub struct Schema {
    hash: Hash,
    object: ValidObj,
    object_valid: bool,
    entries: Vec<(String, usize)>,
    types: Vec<Validator>,
    compressor: zstd_safe::CCtx<'static>,
    decompressor: zstd_safe::DCtx<'static>,
    doc_compress: Compression,
    entries_compress: Vec<(String, Compression)>,
}

impl Schema {

    pub fn from_doc(doc: Document) -> io::Result<Self> {
        Self::from_raw(&mut doc.raw_doc(), Some(doc.hash().clone()))
    }

    fn from_raw(raw: &mut &[u8], hash: Option<Hash>) -> io::Result<Self> {
        let hash = if let Some(hash) = hash {
            hash
        }
        else {
            let raw_for_hash: &[u8] = raw;
            Hash::new(raw_for_hash)
        };

        let mut entries = Vec::new();
        let mut types = Vec::with_capacity(2);
        let mut type_names = HashMap::new();
        let mut object = ValidObj::new_schema(); // Documents can always be queried, hence "true"
        let mut object_valid = true;
        let mut doc_compress = Default::default();
        let mut entries_compress = Vec::new();
        types.push(Validator::Invalid);
        types.push(Validator::Valid);

        let num_fields = match read_marker(raw)? {
            MarkerType::Object(len) => len,
            _ => return Err(Error::new(InvalidData, "Schema wasn't an object")),
        };
        object_iterate(raw, num_fields, |field, raw| {
            match field {
                "" => {
                    read_hash(raw).map_err(|_e| Error::new(InvalidData, "Schema's empty field didn't contain root Schema Hash"))?;
                },
                "doc_compress" => {
                    doc_compress = Compression::read_raw(raw)?;
                }
                "description" => {
                    read_str(raw).map_err(|_e| Error::new(InvalidData, "`description` field didn't contain string"))?;
                },
                "name" => {
                    read_str(raw).map_err(|_e| Error::new(InvalidData, "`name` field didn't contain string"))?;
                },
                "version" => {
                    read_integer(raw).map_err(|_e| Error::new(InvalidData, "`name` field didn't contain integer"))?;
                },
                "entries" => {
                    if let MarkerType::Object(len) = read_marker(raw)? {
                        object_iterate(raw, len, |field, raw| {
                            let v = Validator::read_validator(raw, false, &mut types, &mut type_names, &hash)?;
                            entries.push((field.to_string(), v));
                            Ok(())
                        })?;
                    }
                    else {
                        return Err(Error::new(InvalidData, "`entries` field doesn't contain an Object"));
                    }
                },
                "entries_compress" => {
                    if let MarkerType::Object(len) = read_marker(raw)? {
                        object_iterate(raw, len, |field, raw| {
                            let c = Compression::read_raw(raw)?;
                            entries_compress.push((field.to_string(), c));
                            Ok(())
                        })?;
                    }
                    else {
                        return Err(Error::new(InvalidData, "`entries_compress` field doesn't contain an Object"));
                    }
                },
               "field_type" | "max_fields" | "min_fields" | "req" | "opt" | "ban" | "unknown_ok" => {
                   let valid = object.update(field, raw, false, &mut types, &mut type_names, &hash)?;
                   object_valid = object_valid && valid;
                },
                "types" => {
                    if let MarkerType::Object(len) = read_marker(raw)? {
                        object_iterate(raw, len, |field, raw| {
                            let v = Validator::read_validator(raw, false, &mut types, &mut type_names, &hash)?;
                            if v == (types.len() - 1) {
                                let v = types.pop();
                                match field {
                                    "Null" | "Bool" | "Int" | "Str" | "F32" | "F64" | "Bin" |
                                    "Array" | "Obj" | "Hash" | "Ident" | "Lock" | "Time" | "Multi" => (),
                                    _ => {
                                        if let Some(index) = type_names.get(field) {
                                            types[*index] = v.unwrap();
                                        }
                                    }
                                }
                            }
                            Ok(())
                        })?;
                    }
                    else {
                        return Err(Error::new(InvalidData, "`entries` field doesn't contain an Object"));
                    }
                }
                _ => {
                    return Err(Error::new(InvalidData, "Unrecognized field in schema document"));
                }
            }
            Ok(())
        })?;

        object_valid = object_valid && object.finalize();

        Ok(Schema {
            hash,
            object,
            object_valid,
            entries,
            types,
            compressor: zstd_safe::create_cctx(),
            decompressor: zstd_safe::create_dctx(),
            doc_compress,
            entries_compress,
        })
    }

    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    /// Encode the document and write it to an output buffer. Schema compression defaults are used 
    /// unless overridden by the Document. This will fail if the document's schema hash doesn't 
    /// match this schema, or validation fails.
    ///
    /// # Panics
    /// Panics if the underlying zstd calls return an error, which shouldn't be possible. Also 
    /// panics if the document somehow became larger than the maximum allowed size, which should be 
    /// impossible with the public Document interface.
    pub fn encode_doc(&mut self, doc: Document) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        let len = doc.len();
        let mut raw: &[u8] = doc.raw_doc();
        assert!(len <= MAX_DOC_SIZE,
            "Document was larger than maximum size! Document implementation should've made this impossible!");

        // Verify we're the right schema hash
        if let Some(doc_schema) = doc.schema_hash() {
            if doc_schema != self.hash() { return Err(io::Error::new(InvalidData, "Document doesn't use this schema")); }
        }
        else {
            return Err(io::Error::new(InvalidData, "Document doesn't use this schema"));
        }

        // Verify the document passes validation
        if self.object_valid {
            if !doc.validated() {
                let mut checklist = ValidatorChecklist::new();
                self.object.validate("", &mut doc.raw_doc(), &self.types, &mut checklist, true)?;
            }
        }
        else {
            return Err(io::Error::new(InvalidData, "This schema cannot pass anything"));
        }

        if doc.override_compression() {
            if let Some(level) = doc.compression() {
                CompressType::Compressed.encode(&mut buf);
                let _ = parse_schema_hash(&mut raw)
                    .expect("Document has invalid vec!")
                    .expect("Document has invalid vec!");
                let header_len = doc.raw_doc().len() - raw.len();
                buf.extend_from_slice(&doc.raw_doc()[..header_len]);
                // Compress everything else
                zstd_help::compress(&mut self.compressor, level, raw, &mut buf);
            }
            else {
                CompressType::Uncompressed.encode(&mut buf);
                buf.extend_from_slice(raw);
            }
        }
        else {
            match self.doc_compress {
                Compression::NoCompress => {
                    CompressType::Uncompressed.encode(&mut buf);
                }
                Compression::Compress(_) => {
                    CompressType::Compressed.encode(&mut buf);
                    let _ = parse_schema_hash(&mut raw)
                        .expect("Document has invalid vec!")
                        .expect("Document has invalid vec!");
                    let header_len = doc.raw_doc().len() - raw.len();
                    buf.extend_from_slice(&doc.raw_doc()[..header_len]);
                }
                Compression::DictCompress(_) => {
                    CompressType::DictCompressed.encode(&mut buf);
                    let _ = parse_schema_hash(&mut raw)
                        .expect("Document has invalid vec!")
                        .expect("Document has invalid vec!");
                    let header_len = doc.raw_doc().len() - raw.len();
                    buf.extend_from_slice(&doc.raw_doc()[..header_len]);
                }
            }
            self.doc_compress.compress(&mut self.compressor, raw, &mut buf);
        }

        Ok(buf)
    }

    /// Read a document from a byte slices, trusting the origin of the slice and doing as few 
    /// checks as possible when decoding. It fails if there isn't a valid fog-pack value, The 
    /// compression isn't valid, the slice terminates early, or if the document isn't using this 
    /// schema.
    ///
    /// Rather than compute the hash, the document hash can optionally be provided. If integrity 
    /// checking is desired, provide no hash and compare the expected hash with the hash of the 
    /// resulting document.
    ///
    /// The *only* time this should be used is if the byte slice is coming from a well-trusted 
    /// location, like an internal database.
    pub fn trusted_decode_doc(&mut self, buf: &mut &[u8], hash: Option<Hash>) -> io::Result<Document> {
        let mut buf_ptr: &[u8] = buf;
        let compress_type = CompressType::decode(&mut buf_ptr)?;
        if let CompressType::CompressedNoSchema = compress_type {
            return Err(io::Error::new(InvalidData, "Schema does not support decoding non-schema document"));
        }
        let mut doc = Vec::new();
        let mut buf_post_hash: &[u8] = buf_ptr;
        parse_schema_hash(&mut buf_post_hash)?; // Just pull off the schema hash & object header
        doc.extend_from_slice(&buf_ptr[..(buf_ptr.len()-buf_post_hash.len())]);
        self.doc_compress.decompress(&mut self.decompressor, MAX_DOC_SIZE, compress_type, &mut buf_post_hash, &mut doc)?;
        
        // Find the document length
        let doc_len = verify_value(&mut &doc[..])?;

        // Optionally compute the hashes
        let (hash_state, doc_hash, hash) = if let Some(hash) = hash {
            (None, None, hash)
        }
        else {
            let mut hash_state = crypto::HashState::new();
            hash_state.update(&doc[..doc_len]);
            let doc_hash = hash_state.get_hash();
            let hash = if doc.len() > doc_len {
                hash_state.update(&doc[doc_len..]);
                hash_state.get_hash()
            }
            else {
                doc_hash.clone()
            };
            (Some(hash_state), Some(doc_hash), hash)
        };

        // Get signatures
        let mut signed_by = Vec::new();
        let mut index = &mut &doc[doc_len..];
        while index.len() > 0 {
            let signature = crypto::Signature::decode(&mut index)
                .map_err(|_e| io::Error::new(InvalidData, "Invalid signature in raw document"))?;
            signed_by.push(signature.signed_by().clone());
        }

        let override_compression = false;
        let compression = None;
        let compressed = None;
        Ok(Document::from_parts(
            hash_state,
            doc_hash,
            hash,
            doc_len,
            doc,
            compressed,
            override_compression,
            compression,
            signed_by,
            None
        ))
    }

    /// Read a document from a byte slice, performing the full set of validation checks when 
    /// decoding. Success guarantees that the resulting Document is valid and passed schema 
    /// validation, and as such, this can be used with untrusted inputs.
    ///
    /// Validation checking means this will fail if:
    /// - The data is corrupted or incomplete
    /// - The data isn't valid fogpack 
    /// - The data doesn't use this schema
    /// - The data doesn't adhere to this schema
    /// - The compression is invalid or expands to larger than the maximum allowed size
    /// - Any of the attached signatures are invalid
    pub fn decode_doc(&mut self, buf: &mut &[u8]) -> io::Result<Document> {
        let mut buf_ptr: &[u8] = buf;
        let mut doc = Vec::new();

        // Decompress the document
        let compress_type = CompressType::decode(&mut buf_ptr)?;
        if let CompressType::CompressedNoSchema = compress_type {
            return Err(io::Error::new(InvalidData, "Schema does not support decoding non-schema document"));
        }
        let mut buf_post_hash: &[u8] = buf_ptr;
        parse_schema_hash(&mut buf_post_hash)?; // Just pull off the schema hash & object header
        doc.extend_from_slice(&buf_ptr[..(buf_ptr.len()-buf_post_hash.len())]);
        self.doc_compress.decompress(&mut self.decompressor, MAX_DOC_SIZE, compress_type, &mut buf_post_hash, &mut doc)?;
        
        // Verify the document passes validation
        let mut doc_ptr: &[u8] = &doc[..];
        if self.object_valid {
            let mut checklist = ValidatorChecklist::new();
            self.object.validate("", &mut doc_ptr, &self.types, &mut checklist, true)?;
        }
        else {
            return Err(io::Error::new(InvalidData, "Decoding failed: Schema doesn't allow any documents to pass"));
        }

        let doc_len = doc.len() - doc_ptr.len();

        // Compute the document hashes
        let mut hash_state = crypto::HashState::new();
        hash_state.update(&doc[..doc_len]);
        let doc_hash = hash_state.get_hash();
        let hash = if doc.len() > doc_len {
            hash_state.update(&doc[doc_len..]);
            hash_state.get_hash()
        }
        else {
            doc_hash.clone()
        };

        // Get & verify signatures
        let mut signed_by = Vec::new();
        while doc_ptr.len() > 0 {
            let signature = crypto::Signature::decode(&mut doc_ptr)
                .map_err(|_e| io::Error::new(InvalidData, "Invalid signature in raw document"))?;
            if !signature.verify(&doc_hash) {
                return Err(io::Error::new(InvalidData, "Signature doesn't verify against document"));
            }
            signed_by.push(signature.signed_by().clone());
        }

        let override_compression = false;
        let compression = None;
        let compressed = None;
        Ok(Document::from_parts(
            Some(hash_state),
            Some(doc_hash),
            hash,
            doc_len,
            doc,
            compressed,
            override_compression,
            compression,
            signed_by,
            None
        ))
    }

    /// Encodes an [`Entry`]'s contents and returns an [`EncodeChecklist`] containing the encoded 
    /// byte vector. The entry's parent hash and field are not included. This will fail if entry 
    /// validation fails, or the entry's field is not covered by the schema.
    ///
    /// [`Entry`]: ./checklist/struct.Entry.html
    /// [`EncodeChecklist`]: ./checklist/struct.EncodeChecklist.html
    pub fn encode_entry(&mut self, entry: Entry) -> io::Result<EncodeChecklist> {
        let mut buf = Vec::new();
        let len = entry.len();
        let raw: &[u8] = entry.raw_entry();
        assert!(len <= MAX_ENTRY_SIZE,
            "Entry was larger than maximum size! Entry implementation should've made this impossible!");

        // Verify the entry passes validation
        let checklist = if !entry.validated() {
            let mut checklist = ValidatorChecklist::new();
            let v = self.entries.binary_search_by(|x| x.0.as_str().cmp(entry.field()));
            if let Ok(v) = v {
                let v = self.entries[v].1;
                self.types[v].validate("", &mut entry.raw_entry(), &self.types, 0, &mut checklist)?;
            }
            else {
                return Err(io::Error::new(InvalidData, "Entry field is not in schema"));
            }
            checklist
        }
        else {
            ValidatorChecklist::new()
        };

        if entry.override_compression() {
            // Compression defaults overridden by entry settings
            if let Some(level) = entry.compression() {
                CompressType::CompressedNoSchema.encode(&mut buf);
                zstd_help::compress(&mut self.compressor, level, raw, &mut buf);
            }
            else {
                CompressType::Uncompressed.encode(&mut buf);
                buf.extend_from_slice(raw);
            }
        }
        else {
            // Look up default settings for this entry field type
            let compress = self.entries_compress.binary_search_by(|x| x.0.as_str().cmp(entry.field()));
            if let Ok(compress_index) = compress {
                // Entry has associated compression settings; use them
                let compress = &self.entries_compress[compress_index].1;
                match compress {
                    Compression::NoCompress => {
                        CompressType::Uncompressed.encode(&mut buf);
                    }
                    Compression::Compress(_) => {
                        CompressType::CompressedNoSchema.encode(&mut buf);
                    }
                    Compression::DictCompress(_) => {
                        CompressType::DictCompressed.encode(&mut buf);
                    }
                }
               compress.compress(&mut self.compressor, raw, &mut buf);
            }
            else {
                // Entry has no associated compression settings; use default
                CompressType::CompressedNoSchema.encode(&mut buf);
                zstd_help::compress(&mut self.compressor, zstd_safe::CLEVEL_DEFAULT, raw, &mut buf);
            }
        }

        Ok(EncodeChecklist::new(checklist, buf))
    }

    /// Read an [`Entry`] from a byte slice, trusting the origin of the slice and doing as few 
    /// checks as possible when decoding. It fails if there isn't a valid fog-pack value, the 
    /// compression isn't valid, or the slice terminates early.
    ///
    /// Rather than compute the hash, the entry's hash can optionally be provided. If integrity 
    /// checking is desired, provide no hash and compare the expected hash with the hash of the 
    /// resulting entry.
    ///
    /// The *only* time this should be used is if the byte slice is coming from a well-trusted 
    /// location, like an internal database.
    ///
    /// [`Entry`]: ./struct.Entry.html
    /// [`DecodeChecklist`]: ./struct.DecodeChecklist.html
    pub fn trusted_decode_entry(&mut self, buf: &mut &[u8], doc: Hash, field: String, hash: Option<Hash>) -> io::Result<Entry> {
        let mut buf_ptr: &[u8] = buf;
        let mut entry = Vec::new();

        // Decompress the entry
        let compress_type = CompressType::decode(&mut buf_ptr)?;
        match compress_type {
            CompressType::Compressed => {
                return Err(io::Error::new(InvalidData, "Entries don't allow compression with schema!"));
            },
            CompressType::Uncompressed => {
                entry.extend_from_slice(buf_ptr);
            },
            CompressType::CompressedNoSchema => {
                zstd_help::decompress(&mut self.decompressor, MAX_ENTRY_SIZE, &mut buf_ptr, &mut entry)?;
            },
            CompressType::DictCompressed => {
                let compress = self.entries_compress.binary_search_by(|x| x.0.as_str().cmp(&field))
                    .map_err(|_| io::Error::new(InvalidData,
                            format!("Schema has no dictionary for field \"{}\"", field)))?;
                let compress = &self.entries_compress[compress].1;
                compress.decompress(&mut self.decompressor, MAX_ENTRY_SIZE, compress_type, &mut buf_ptr, &mut entry)?;
            }
        }

        // Parse the entry itself & load in the optional hash
        let entry_len = verify_value(&mut &entry[..])?;
        let hash_provided = hash.is_some();
        let hash = hash.unwrap_or(Hash::new_empty());

        // Get signatures
        let mut signed_by = Vec::new();
        let mut index = &mut &entry[entry_len..];
        while index.len() > 0 {
            let signature = crypto::Signature::decode(&mut index)
                .map_err(|_e| io::Error::new(InvalidData, "Invalid signature in raw entry"))?;
            signed_by.push(signature.signed_by().clone());
        }

        let override_compression = false;
        let compression = None;
        let compressed = None;

        let mut entry = Entry::from_parts(
            None,
            None,
            hash,
            doc,
            field,
            entry_len,
            entry,
            signed_by,
            compressed,
            override_compression,
            compression,
        );

        if !hash_provided {
            entry.populate_hash_state();
        }

        Ok(entry)
    }

    /// Read an [`Entry`] from a byte slice, performing a full set of validation checks when decoding. 
    /// On successful validation of the entry, a [`DecodeChecklist`] is returned, which contains 
    /// the decoded entry. Processing the checklist with this schema will complete the checklist 
    /// and yield the decoded [`Entry`].
    ///
    /// [`Entry`]: ./struct.Entry.html
    /// [`DecodeChecklist`]: ./struct.DecodeChecklist.html
    pub fn decode_entry(&mut self, buf: &mut &[u8], doc: Hash, field: String) -> io::Result<DecodeChecklist> {
        let mut buf_ptr: &[u8] = buf;
        let mut entry = Vec::new();

        let validator = self.entries.binary_search_by(|x| x.0.as_str().cmp(&field))
            .map_err(|_| io::Error::new(InvalidData, "Entry field is not in schema"))?;
        let validator = self.entries[validator].1;

        // Decompress the entry
        let compress_type = CompressType::decode(&mut buf_ptr)?;
        match compress_type {
            CompressType::Compressed => {
                return Err(io::Error::new(InvalidData, "Entries don't allow compression with schema!"));
            },
            CompressType::Uncompressed => {
                entry.extend_from_slice(buf_ptr);
            },
            CompressType::CompressedNoSchema => {
                zstd_help::decompress(&mut self.decompressor, MAX_ENTRY_SIZE, &mut buf_ptr, &mut entry)?;
            },
            CompressType::DictCompressed => {
                let compress = self.entries_compress.binary_search_by(|x| x.0.as_str().cmp(&field))
                    .map_err(|_| io::Error::new(InvalidData,
                            format!("Schema has no dictionary for field \"{}\"", field)))?;
                let compress = &self.entries_compress[compress].1;
                compress.decompress(&mut self.decompressor, MAX_ENTRY_SIZE, compress_type, &mut buf_ptr, &mut entry)?;
            }
        }

        // Verify the entry passes validation
        let mut entry_ptr: &[u8] = &entry[..];
        let mut checklist = ValidatorChecklist::new();
        self.types[validator].validate("", &mut entry_ptr, &self.types, 0, &mut checklist)?;
        let entry_len = entry.len() - entry_ptr.len();

        // Compute the entry hashes
        let mut temp = Vec::new();
        let mut hash_state = crypto::HashState::new();
        encode::write_value(&mut temp, &Value::from(doc.clone()));
        hash_state.update(&temp[..]);
        temp.clear();
        encode::write_value(&mut temp, &Value::from(field.clone()));
        hash_state.update(&temp[..]);
        hash_state.update(&entry[..entry_len]);
        let entry_hash = hash_state.get_hash();
        let hash = if entry.len() > entry_len {
            hash_state.update(&entry[entry_len..]);
            hash_state.get_hash()
        }
        else {
            entry_hash.clone()
        };


        // Get & verify signatures
        let mut signed_by = Vec::new();
        while entry_ptr.len() > 0 {
            let signature = crypto::Signature::decode(&mut entry_ptr)
                .map_err(|_e| io::Error::new(InvalidData, "Invalid signature in raw entry"))?;
            if !signature.verify(&entry_hash) {
                return Err(io::Error::new(InvalidData, "Signature doesn't verify against entry"));
            }
            signed_by.push(signature.signed_by().clone());
        }

        let override_compression = false;
        let compression = None;
        let compressed = None;

        let entry = Entry::from_parts(
            Some(hash_state),
            Some(entry_hash),
            hash,
            doc,
            field,
            entry_len,
            entry,
            signed_by,
            compressed,
            override_compression,
            compression,
        );

        Ok(DecodeChecklist::new(checklist, entry))
    }

    /// Checks a document against a given ChecklistItem. Marks the item as done on success. Fails 
    /// if validation fails.
    ///
    /// A [`ChecklistItem`] comes from either a [`EncodeChecklist`] or a [`DecodeChecklist`]. 
    ///
    /// [`ChecklistItem`] ./struct.ChecklistItem.html
    /// [`EncodeChecklist`] ./struct.EncodeChecklist.html
    /// [`DecodeChecklist`] ./struct.DecodeChecklist.html
    pub fn check_item(&self, doc: &Document, item: &mut ChecklistItem) -> io::Result<()> {
        for index in item.iter() {
            if let Validator::Hash(ref v) = self.types[*index] {
                // Check against acceptable schemas
                if v.schema_required() {
                    if let Some(hash) = doc.schema_hash() {
                        if !v.schema_in_set(&hash) {
                            return Err(Error::new(InvalidData, "Document uses unrecognized schema"));
                        }
                    }
                    else {
                        return Err(Error::new(InvalidData, "Document doesn't have schema, but needs one"));
                    }
                }
                if let Some(link) = v.link() {
                    let mut checklist = ValidatorChecklist::new();
                    if let Validator::Object(ref v) = self.types[link] {
                        v.validate("", &mut doc.raw_doc(), &self.types, &mut checklist, true)?;
                    }
                    else {
                        return Err(Error::new(Other, "Can't validate a document against a non-object validator"));
                    }
                }
            }
            else {
                return Err(Error::new(Other, "Can't validate against non-hash validator"));
            }
        };
        item.mark_done();
        Ok(())
    }

    /// Validates a document against a specific Hash Validator. Should be used in conjunction with 
    /// a ValidatorChecklist returned from `validate_entry` to confirm that all documents referenced in an 
    /// entry meet the schema's criteria.
    pub fn validate_checklist_item(&self, index: usize, doc: &mut &[u8]) -> io::Result<()> {
        if let Validator::Hash(ref v) = self.types[index] {
            // Extract schema. Also verifies we are dealing with an Object (an actual document)
            let doc_schema = extract_schema_hash(&doc.clone())?;
            // Check against acceptable schemas
            if v.schema_required() {
                if let Some(hash) = doc_schema {
                    if !v.schema_in_set(&hash) {
                        return Err(Error::new(InvalidData, "Document uses unrecognized schema"));
                    }
                }
                else {
                    return Err(Error::new(InvalidData, "Document doesn't have schema, but needs one"));
                }
            }
            if let Some(link) = v.link() {
                let mut checklist = ValidatorChecklist::new();
                if let Validator::Object(ref v) = self.types[link] {
                    v.validate("", doc, &self.types, &mut checklist, true).and(Ok(()))
                }
                else {
                    Err(Error::new(Other, "Can't validate a document against a non-object validator"))
                }
            }
            else {
                Ok(())
            }
        }
        else {
            Err(Error::new(Other, "Can't validate against non-hash validator"))
        }

    }
}

enum Compression {
    NoCompress,
    Compress(i32),
    DictCompress((zstd_safe::CDict<'static>, zstd_safe::DDict<'static>))
}

impl std::default::Default for Compression {
    fn default() -> Self {
        Compression::Compress(zstd_safe::CLEVEL_DEFAULT)
    }
}

impl Compression {
    fn read_raw(raw: &mut &[u8]) -> io::Result<Compression> {
        let mut setting_seen = false;
        let mut format_seen = false;
        let mut level_seen = false;
        let mut level = zstd_safe::CLEVEL_DEFAULT;
        let mut format = 0;
        let mut setting = None;
        let mut setting_bool = false;

        let num_fields = match read_marker(raw)? {
            MarkerType::Object(len) => len,
            _ => return Err(Error::new(InvalidData, "Compress spec wasn't an object")),
        };
        object_iterate(raw, num_fields, |field, raw| {
            match field {
                "format" => {
                    format_seen = true;
                    if let Some(i) = read_integer(raw)?.as_u64() {
                        if i > 31 {
                            Err(Error::new(InvalidData,
                                "Compress `format` field didn't contain integer between 0 and 31"))
                        }
                        else {
                            format = i;
                            Ok(())
                        }
                    }
                    else {
                        Err(Error::new(InvalidData,
                            "Compress `format` field didn't contain integer between 0 and 31"))
                    }
                },
                "level" => {
                    level_seen = true;
                    if let Some(i) = read_integer(raw)?.as_u64() {
                        if i > 255 {
                            Err(Error::new(InvalidData,
                                "Compress `level` field didn't contain integer between 0 and 255"))
                        }
                        else {
                            level = i as i32;
                            let max = zstd_safe::max_c_level();
                            if level > max {
                                level = max;
                            }
                            Ok(())
                        }
                    }
                    else {
                        Err(Error::new(InvalidData,
                            "Compress `level` field didn't contain integer between 0 and 255"))
                    }
                },
                "setting" => {
                    setting_seen = true;
                    match read_marker(raw)? {
                        MarkerType::Boolean(v) => {
                            setting_bool = v;
                            Ok(())
                        },
                        MarkerType::Binary(len) => {
                            let v = read_raw_bin(raw, len)?;
                            setting = Some(v.to_vec());
                            setting_bool = true;
                            Ok(())
                        },
                        _ => {
                            Err(Error::new(InvalidData,
                                "Compress `setting` field didn't contain boolean or binary data"))
                        }
                    }
                },
                _ => {
                    Err(Error::new(InvalidData,
                        format!("Compress contains unrecognized field `{}`", field)))
                }
            }
        })?;

        // Checks to verify we met the allowed object format
        if !setting_seen {
            return Err(Error::new(InvalidData, "Compress spec didn't have setting field"));
        }
        if !setting_bool && (format_seen || level_seen) {
            return Err(Error::new(InvalidData, "Compress spec had false setting field, but other fields were also present"));
        }
        if !format_seen && setting_bool {
            return Err(Error::new(InvalidData, "Compress spec had setting field not set to false, but no format field"));
        }

        Ok(
            if !setting_bool {
                Compression::NoCompress
            }
            else if format > 0 {
                // We know compression was desired, but don't recognize the format. Just use 
                // default compression instead.
                Compression::Compress(zstd_safe::CLEVEL_DEFAULT)
            }
            else if let Some(bin) = setting {
                Compression::DictCompress((
                    zstd_safe::create_cdict(&bin[..], level),
                    zstd_safe::create_ddict(&bin[..])
                ))
            }
            else {
                Compression::Compress(level)
            }
        )
    }

    fn compress(&self, compressor: &mut zstd_safe::CCtx, raw: &[u8], buf: &mut Vec<u8>) {
        match self {
            Compression::NoCompress => {
                buf.extend_from_slice(raw);
            },
            Compression::Compress(level) => {
                zstd_help::compress(compressor, *level, raw, buf);
            },
            Compression::DictCompress((dict, _)) => {
                zstd_help::dict_compress(compressor, dict, raw, buf);
            },
        }
    }

    // Decompress raw data, after it has been stripped of the `CompressType` byte and the header if 
    // present, which consists of the leading object tag and schema hash field-value pair.
    // The intent is that the caller of this function needed to vet the schema hash anyway, and 
    // verify that an Entry didn't start with the Compressed flag, and that a Document didn't start 
    // with the CompressedNoSchema flag.
    fn decompress(
        &self,
        decompressor: &mut zstd_safe::DCtx,
        max_size: usize,
        compress_type: CompressType,
        buf: &mut &[u8],
        decode: &mut Vec<u8>
    )
        -> io::Result<()>
    {
        match compress_type {
            CompressType::Uncompressed => {
                if (buf.len() + decode.len()) > max_size {
                    return Err(io::Error::new(InvalidData, "Data is larger than maximum allowed size"));
                }
                decode.extend_from_slice(buf);
                Ok(())
            },
            CompressType::Compressed | CompressType::CompressedNoSchema => {
                zstd_help::decompress(decompressor, max_size, buf, decode)
            },
            CompressType::DictCompressed => {
                // Decompress the data
                // Find the expected size, and fail if it's larger than the maximum allowed size.
                if let Compression::DictCompress((_,dict)) = &self {
                    zstd_help::dict_decompress(decompressor, dict, max_size, buf, decode)
                }
                else {
                    Err(io::Error::new(InvalidData, "Schema has no dictionary for this data"))
                }
            }
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use super::super::Value;
    use NoSchema;
    //use crate::crypto::{Vault, PasswordLevel, Key};

    fn simple_schema() -> Document {
        let schema: Value = fogpack!({
            "req": {
                "title": { "type": "Str", "max_len": 200 },
                "text": { "type": "Str" }
            },
            "entries": {
                "rel": {
                    "type": "Obj",
                    "req": {
                        "name": { "type": "Str" },
                        "link": { "type": "Hash" }
                    }
                }
            },
            "doc_compress": {
                "format": 0,
                "level": 3,
                "setting": true
            },
            "entries_compress": {
                "rel": { "setting": false }
            }
        });
        Document::new(schema).expect("Should've been able to encode as a document")
    }

    fn simple_doc(schema: &Hash) -> Document {
        let doc: Value = fogpack!({
            "": Value::from(schema.clone()),
            "title": "A Test",
            "text": "This is a test of a schema document"
        });
        println!("{}", doc);
        Document::new(doc).expect("Should've been able to encode as document")
    }

    fn simple_bad_doc(schema: &Hash) -> Document {
        let doc: Value = fogpack!({
            "": Value::from(schema.clone()),
            "title": "A Test",
            "text": 0
        });
        Document::new(doc).expect("Should've been able to encode as document")
    }

    fn simple_entry(doc: &Hash) -> Entry {
        let test: Value = fogpack!({
            "name": "test_entry",
            "link": Hash::new(b"fake hash")
        });
        Entry::new(doc.clone(), String::from("rel"), test).expect("Should've been able to encode as an entry")
    }

    #[test]
    fn use_simple_schema() {
        let schema = simple_schema();
        let mut schema = Schema::from_doc(schema).unwrap();
        let mut no_schema = NoSchema::new();
        let test = simple_doc(schema.hash());

        let enc = schema.encode_doc(test.clone()).unwrap();
        let dec = schema.decode_doc(&mut &enc[..]).unwrap();
        assert!(test == dec, "Document didn't stay same through enc/dec");

        let test = simple_bad_doc(schema.hash());
        assert!(schema.encode_doc(test.clone()).is_err());
        assert!(no_schema.encode_doc(test.clone()).is_err());
        let mut enc = Vec::new();
        CompressType::Uncompressed.encode(&mut enc);
        enc.extend_from_slice(&test.raw_doc()[..]);
        assert!(schema.decode_doc(&mut &enc[..]).is_err());
    }

    /*
    #[test]
    fn entry_encode_decode() {
        let mut test = test_entry();
        test.set_compression(None);
        let mut schema_none = NoSchema::new();
        let enc = schema_none.encode_entry(&test);
        let mut dec = schema_none.trusted_decode_entry(&mut &enc[..], Hash::new_empty(), String::from(""), None)
            .expect("Decoding should have worked");
        dec.set_compression(None);
        let enc2 = schema_none.encode_entry(&dec);
        assert!(test == dec, "Encode->Decode should yield same entry");
        assert!(enc == enc2, "Encode->Decode->encode didn't yield identical results");
    }

    #[test]
    fn entry_compress_decompress() {
        let test = test_entry();
        let mut schema_none = NoSchema::new();
        let enc = schema_none.encode_entry(&test);
        let dec = schema_none.trusted_decode_entry(&mut &enc[..], Hash::new_empty(), String::from(""), None)
            .expect("Decoding should have worked");
        assert!(test == dec, "Compress->Decode should yield same entry");
    }

    #[test]
    fn entry_compress_decompress_sign() {
        let mut test = test_entry();
        let (mut vault, key0) = prep_vault();
        let key1 = vault.new_key();
        let key2 = vault.new_key();
        test.sign(&vault, &key0).expect("Should have been able to sign test entry w/ key0");
        test.sign(&vault, &key1).expect("Should have been able to sign test entry w/ key1");
        let mut schema_none = NoSchema::new();
        let enc = schema_none.encode_entry(&test);
        let mut dec = schema_none.trusted_decode_entry(&mut &enc[..], Hash::new_empty(), String::from(""), None)
            .expect("Decoding should have worked");
        test.sign(&vault, &key2).expect("Should have been able to sign test entry w/ key2");
        dec.sign(&vault, &key2).expect("Should have been able to sign decoded entry w/ key2");
        assert!(test == dec, "Compress->Decode should yield same entry, even after signing");
    }

    #[test]
    fn entry_compress_sign_existing_hash() {
        let mut test = test_entry();
        let (vault, key) = prep_vault();
        let mut schema_none = NoSchema::new();
        let enc = schema_none.encode_entry(&test);
        let mut dec = schema_none.trusted_decode_entry(
            &mut &enc[..],
            Hash::new_empty(),
            String::from(""),
            Some(test.hash().clone())
        )
            .expect("Decoding should have worked");

        test.sign(&vault, &key).expect("Should have been able to sign test entry");
        dec.sign(&vault, &key).expect("Should have been able to sign decoded entry");
        assert!(test == dec, "Compress->Decode should yield same entry, even after signing");
    }

    #[test]
    fn entry_strict_decode() {
        let test = test_entry();
        let mut schema_none = NoSchema::new();
        let enc = schema_none.encode_entry(&test);
        let dec = schema_none.decode_entry(&mut &enc[..], Hash::new_empty(), String::from(""))
            .expect("Decoding should have worked");
        assert!(test == dec, "Compress->Decode should yield same entry");
    }

    #[test]
    fn entry_corrupted_data() {
        // Prep encode/decode & byte vector
        let mut schema_none = NoSchema::new();
        // Prep a entry
        let (vault, key) = prep_vault();
        let mut test = test_entry();
        test.sign(&vault, &key).expect("Should have been able to sign test entry");

        test.set_compression(None);
        let mut enc = schema_none.encode_entry(&test);
        *(enc.last_mut().unwrap()) = *enc.last_mut().unwrap() ^ 0xFF;
        let dec = schema_none.decode_entry(&mut &enc[..], Hash::new_empty(), String::from(""));
        assert!(dec.is_err(), "Entry signature was corrupted, but decoding succeeded anyway");

        let mut enc = schema_none.encode_entry(&test);
        // Targets part of the binary sequence, which should break the signature, but not the Value verification
        enc[4] = 0xFF;
        let dec = schema_none.decode_entry(&mut &enc[..], Hash::new_empty(), String::from(""));
        assert!(dec.is_err(), "Entry payload was corrupted, but decoding succeeded anyway");

        let mut enc = schema_none.encode_entry(&test);
        enc[0] = 0x1; // Targets the compression marker
        let dec = schema_none.decode_entry(&mut &enc[..], Hash::new_empty(), String::from(""));
        assert!(dec.is_err(), "Entry payload was corrupted, but decoding succeeded anyway");
    }
    */
}

