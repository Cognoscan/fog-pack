use byteorder::{ReadBytesExt, BigEndian};
use std::collections::HashMap;

use crypto;
use Error;
use decode::*;
use document::parse_schema_hash;
use validator::{query_check, ValidObj, Validator, ValidReader, ValidatorChecklist};
use {MAX_DOC_SIZE, MAX_ENTRY_SIZE, Entry, Hash, Document, Query, MarkerType, CompressType};
use checklist::{ChecklistItem, Checklist};
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

```
# use fog_pack::*;
# let core_schema = Document::new(fogpack!({"fake": "schema"})).unwrap();
# fogpack!(
{
    "": core_schema.hash().clone(),
    "name": "Simple Schema",
    "req": {
        "title": { "type": "Str", "max_len": 255},
        "text": { "type": "Str" }
    }
}
# );
```

A document that uses this "Basic Schema" would look like:

```
# use fog_pack::*;
# let basic_schema = Document::new(fogpack!({"fake": "schema"})).unwrap();
# fogpack!(
{
    "": basic_schema.hash().clone(),
    "title": "Example Document",
    "text": "This is an example document that meets a schema"
}
# );
```

Schema Document Format
======================

The most important concept in a schema document is the validator. A validator is 
a fog-pack object containing the validation rules for a particular part of a 
document. It can directly define the rules, or be aliased and used throughout 
the schema document. Validators are always one of the base fog-pack value types, 
or allow for several of them. See the [Validation Language] for more info.

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

[Validation Language]: ./spec/validation/index.html
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

    pub fn from_doc(doc: Document) -> crate::Result<Self> {
        Ok(Self::from_raw(&mut &doc.raw_doc()[4..doc.doc_len()], Some(doc.hash().clone()))?)
    }

    fn from_raw(raw: &mut &[u8], hash: Option<Hash>) -> crate::Result<Self> {
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

        let mut reader = ValidReader::new(false, &mut types, &mut type_names, &hash);

        let num_fields = match read_marker(raw)? {
            MarkerType::Object(len) => len,
            _ => return Err(Error::BadEncode(raw.len(), "Schema isn't a Document")),
        };
        object_iterate(raw, num_fields, |field, raw| {
            match field {
                "" => {
                    read_hash(raw)?;
                },
                "doc_compress" => {
                    doc_compress = Compression::read_raw(raw)?;
                }
                "description" => {
                    read_str(raw)?;
                },
                "name" => {
                    read_str(raw)?;
                },
                "version" => {
                    read_integer(raw)?;
                },
                "entries" => {
                    if let MarkerType::Object(len) = read_marker(raw)? {
                        object_iterate(raw, len, |field, raw| {
                            let v = Validator::read_validator(raw, &mut reader)?;
                            entries.push((field.to_string(), v));
                            Ok(())
                        })?;
                    }
                    else {
                        return Err(Error::FailValidate(raw.len(), "`entries` field doesn't contain an Object"));
                    }
                },
                "entries_compress" => {
                    if let MarkerType::Object(len) = read_marker(raw)? {
                        // Note that because this is an object, the entries are already sorted by 
                        // key
                        object_iterate(raw, len, |field, raw| {
                            let c = Compression::read_raw(raw)?;
                            entries_compress.push((field.to_string(), c));
                            Ok(())
                        })?;
                    }
                    else {
                        return Err(Error::FailValidate(raw.len(), "`entries_compress` field doesn't contain an Object"));
                    }
                },
                "field_type" | "max_fields" | "min_fields" | "req" | "opt" | "ban" | "unknown_ok" => {
                    let valid = object.update(field, raw, &mut reader)?;
                    object_valid = object_valid && valid;
                },
                "types" => {
                    if let MarkerType::Object(len) = read_marker(raw)? {
                        object_iterate(raw, len, |field, raw| {
                            let v = Validator::read_validator(raw, &mut reader)?;
                            if v == (reader.types.len() - 1) {
                                let v = reader.types.pop();
                                match field {
                                    "Null" | "Bool" | "Int" | "Str" | "F32" | "F64" | "Bin" |
                                        "Array" | "Obj" | "Hash" | "Ident" | "Lock" | "Time" | "Multi" => (),
                                    _ => {
                                        if let Some(index) = reader.type_names.get(field) {
                                            reader.types[*index] = v.unwrap();
                                        }
                                    }
                                }
                            }
                            Ok(())
                        })?;
                    }
                    else {
                        return Err(Error::FailValidate(raw.len(), "`entries` field doesn't contain an Object"));
                    }
                }
                _ => {
                    return Err(Error::FailValidate(raw.len(), "Unrecognized field in schema document"));
                }
            }
            Ok(())
        })?;

        object_valid = object_valid && object.finalize();

        drop(reader); // Drop reference to `types` so we can move it into the Schema

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
    pub fn encode_doc(&mut self, doc: Document) -> crate::Result<Vec<u8>> {
        let len = doc.size();
        assert!(len < MAX_DOC_SIZE,
            "Document was larger than maximum size! Document implementation should've made this impossible!");

        // Verify we're the right schema hash
        if let Some(doc_schema) = doc.schema_hash() {
            if doc_schema != self.hash() { return Err(Error::SchemaMismatch); }
        }
        else {
            return Err(Error::SchemaMismatch);
        }

        // Verify the document passes validation
        if self.object_valid {
            if !doc.validated() {
                let mut checklist = ValidatorChecklist::new();
                let mut raw: &[u8] = &doc.raw_doc()[4..];
                self.object.validate(&mut raw, &self.types, &mut checklist, true)?;
            }
        }
        else {
            return Err(Error::FailValidate(doc.raw_doc().len()-4, "This schema cannot pass anything"));
        }

        if doc.compress_cache() {
            Ok(doc.into_compressed_vec())
        }
        else if doc.override_compression() {
            if let Some(level) = doc.compression() {
                let mut buf = Vec::new();
                // Header with length
                CompressType::Compressed.encode(&mut buf);
                buf.push(0u8);
                buf.push(0u8);
                buf.push(0u8);
                let mut raw: &[u8] = &doc.raw_doc()[4..doc.doc_len()];
                // Extend header with schema hash
                let _ = parse_schema_hash(&mut raw)
                    .expect("Document has invalid vec!")
                    .expect("Document has invalid vec!");
                let header_len = doc.raw_doc().len() - raw.len();
                buf.extend_from_slice(&doc.raw_doc()[4..header_len]);
                // Compress the data
                let raw: &[u8] = &doc.raw_doc()[header_len..doc.doc_len()];
                zstd_help::compress(&mut self.compressor, level, raw, &mut buf);
                // If the compressed version isn't smaller, ditch it and use the uncompressed 
                // version
                if buf.len() >= doc.doc_len() {
                    Ok(doc.into_vec())
                }
                else {
                    // Complete the compressed version by filling in the compressed size and 
                    // appending the signatures
                    let compress_len = buf.len()-4;
                    buf[1] = ((compress_len & 0x00FF_0000) >> 16) as u8;
                    buf[2] = ((compress_len & 0x0000_FF00) >>  8) as u8;
                    buf[3] = ( compress_len & 0x0000_00FF)        as u8;
                    buf.extend_from_slice(&doc.raw_doc()[doc.doc_len()..]);
                    Ok(buf)
                }
            }
            else {
                Ok(doc.into_vec())
            }
        }
        else {
            match self.doc_compress {
                Compression::NoCompress => {
                    Ok(doc.into_vec())
                }
                Compression::Compress(_) | Compression::DictCompress(_) => {
                    let mut buf = Vec::new();
                    self.doc_compress.encode(&mut buf);
                    buf.push(0u8);
                    buf.push(0u8);
                    buf.push(0u8);
                    let mut raw: &[u8] = &doc.raw_doc()[4..doc.doc_len()];
                    let _ = parse_schema_hash(&mut raw)
                        .expect("Document has invalid vec!")
                        .expect("Document has invalid vec!");
                    let header_len = doc.raw_doc().len() - raw.len();
                    buf.extend_from_slice(&doc.raw_doc()[4..header_len]);
                    let raw: &[u8] = &doc.raw_doc()[header_len..doc.doc_len()];
                    self.doc_compress.compress(&mut self.compressor, raw, &mut buf);
                    if buf.len() >= doc.doc_len() {
                        Ok(doc.into_vec())
                    }
                    else {
                        let compress_len = buf.len()-4;
                        buf[1] = ((compress_len & 0x00FF_0000) >> 16) as u8;
                        buf[2] = ((compress_len & 0x0000_FF00) >>  8) as u8;
                        buf[3] = ( compress_len & 0x0000_00FF)        as u8;
                        buf.extend_from_slice(&doc.raw_doc()[doc.doc_len()..]);
                        Ok(buf)
                    }
                }
            }
        }
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
    pub fn trusted_decode_doc(&mut self, buf: &mut &[u8], hash: Option<Hash>) -> crate::Result<Document> {
        self.internal_decode_doc(buf, hash, false)
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
    pub fn decode_doc(&mut self, buf: &mut &[u8]) -> crate::Result<Document> {
        self.internal_decode_doc(buf, None, true)
    }

    /// Internal function for decoding. If do_checks is True, hash MUST be None or it will panic. 
    fn internal_decode_doc(&mut self, buf: &mut &[u8], hash: Option<Hash>, do_checks: bool)
        -> crate::Result<Document>
    {
        let raw_ref: &[u8] = buf;
        if buf.len() >= MAX_DOC_SIZE {
            return Err(Error::BadSize);
        }

        // Read header & check it
        // ----------------------
        // Check for non-schema header byte
        let compress_type = CompressType::decode(buf)?;
        if let CompressType::CompressedNoSchema = compress_type {
            return Err(Error::SchemaMismatch);
        }
        // Get the data payload length & length of attached signatures
        let data_size = buf.read_u24::<BigEndian>()? as usize;
        if data_size > buf.len() {
            return Err(Error::BadHeader);
        }
        let sign_size = buf.len() - data_size;
        // There should *always* be a schema used. Parse it and fail if it's not there or doesn't 
        // match this Schema.
        let schema_hash = parse_schema_hash(buf)? // Just pull off the schema hash & object header
            .ok_or(Error::SchemaMismatch)?;
        if &schema_hash != self.hash() { return Err(Error::SchemaMismatch); }

        // Construct decoded data
        // ----------------------
        let (doc, compressed, doc_len) = match compress_type {
            CompressType::Uncompressed => {
                (Vec::from(raw_ref), None, data_size+4)
            },
            CompressType::CompressedNoSchema => {
                return Err(Error::SchemaMismatch);
            },
            CompressType::Compressed | CompressType::DictCompressed => {
                let mut doc = Vec::new();
                // Push header to decoded data
                let header_size = raw_ref.len() - buf.len();
                doc.extend_from_slice(&raw_ref[..header_size]);
                // Decompress payload
                self.doc_compress.decompress(
                    &mut self.decompressor,
                    MAX_DOC_SIZE,
                    sign_size+header_size,
                    compress_type,
                    &buf[..(buf.len()-sign_size)],
                    &mut doc
                )?;
                // Calculate the data size (no 4-byte header) and document length (with 4-byte header)
                let data_size = doc.len() - 4;
                // Overwrite the stale data size in the header (was copied as part of the original header)
                doc[1] = ((data_size & 0x00FF_0000) >> 16) as u8;
                doc[2] = ((data_size & 0x0000_FF00) >>  8) as u8;
                doc[3] = ( data_size & 0x0000_00FF)        as u8;
                // Append the signatures
                doc.extend_from_slice(&buf[(buf.len()-sign_size)..]);
                // Check for a bad size one last time before completing the decoding
                if doc.len() >= MAX_DOC_SIZE {
                    return Err(Error::BadSize);
                }
                (doc, Some(Vec::from(raw_ref)), data_size + 4)
            }
        };

        
        // Verify the document passes validation
        // -------------------------------------
        if do_checks {
            if self.object_valid {
                let mut doc_ptr: &[u8] = &doc[4..doc_len];
                let mut checklist = ValidatorChecklist::new();
                self.object.validate(&mut doc_ptr, &self.types, &mut checklist, true)?;
                if !doc_ptr.is_empty() {
                    // Fail with BadHeader since this means the header's 3-byte size tag 
                    // mis-represented the size of the fog-pack object. Or, alternately, that the 
                    // compressed data payload had more than just the rest of a fog-pack object in it.
                    return Err(Error::BadHeader);
                }
            }
            else {
                return Err(Error::FailValidate(doc.len(), "This schema cannot pass any documents"));
            }
        }

        // Pass the everything on to create the actual Document
        Document::from_decoded(
            doc,
            doc_len,
            compressed,
            Some(schema_hash),
            hash,
            do_checks
        )
    }

    /// Encodes an [`Entry`]'s contents and returns a [`Checklist`] containing the encoded 
    /// byte vector. The entry's parent hash and field are not included. This will fail if entry 
    /// validation fails, or the entry's field is not covered by the schema.
    ///
    /// [`Entry`]: ./checklist/struct.Entry.html
    /// [`Checklist`]: ./checklist/struct.Checklist.html
    pub fn encode_entry(&mut self, entry: Entry) -> crate::Result<Checklist<Vec<u8>>> {
        let len = entry.size();
        assert!(len < MAX_ENTRY_SIZE,
            "Entry was larger than maximum size! Entry implementation should've made this impossible!");

        // Verify the entry passes validation
        let checklist = if !entry.validated() {
            let mut checklist = ValidatorChecklist::new();
            let v = self.entries.binary_search_by(|x| x.0.as_str().cmp(entry.field()));
            if let Ok(v) = v {
                let v = self.entries[v].1;
                self.types[v].validate(&mut entry.entry_val(), &self.types, 0, &mut checklist)?;
            }
            else {
                return Err(Error::FailValidate(len-3, "Entry field is not in schema"));
            }
            checklist
        }
        else {
            ValidatorChecklist::new()
        };

        let buf = if entry.compress_cache() {
            entry.into_compressed_vec()
        }
        else if entry.override_compression() {
            // Compression defaults overridden by entry settings
            if let Some(level) = entry.compression() {
                let mut buf = Vec::new();
                CompressType::CompressedNoSchema.encode(&mut buf);
                buf.push(0u8);
                buf.push(0u8);
                zstd_help::compress(&mut self.compressor, level, entry.entry_val(), &mut buf);
                // If the compressed version isn't smaller, ditch it and use the uncompressed 
                // version
                if buf.len() >= entry.entry_len() {
                    entry.into_vec()
                }
                else {
                    // Complete the compressed version by filling in the compressed size and 
                    // appending the signatures
                    let compress_len = buf.len()-3;
                    buf[1] = ((compress_len & 0xFF00) >>  8) as u8;
                    buf[2] = ( compress_len & 0x00FF)        as u8;
                    buf.extend_from_slice(&entry.raw_entry()[entry.entry_len()..]);
                    buf
                }
            }
            else {
                entry.into_vec()
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
                        entry.into_vec()
                    }
                    Compression::Compress(_) | Compression::DictCompress(_) => {
                        let mut buf = Vec::new();
                        compress.encode(&mut buf);
                        buf.push(0u8);
                        buf.push(0u8);
                        compress.compress(&mut self.compressor, entry.entry_val(), &mut buf);
                        if buf.len() >= entry.entry_len() {
                            entry.into_vec()
                        }
                        else {
                            // Complete the compressed version by filling in the compressed size and 
                            // appending the signatures
                            let compress_len = buf.len()-3;
                            buf[1] = ((compress_len & 0xFF00) >>  8) as u8;
                            buf[2] = ( compress_len & 0x00FF)        as u8;
                            buf.extend_from_slice(&entry.raw_entry()[entry.entry_len()..]);
                            buf
                        }
                    }
                }
            }
            else {
                // Entry has no associated compression settings; use default
                let mut buf = Vec::new();
                CompressType::CompressedNoSchema.encode(&mut buf);
                buf.push(0u8);
                buf.push(0u8);
                zstd_help::compress(&mut self.compressor, zstd_safe::CLEVEL_DEFAULT, entry.entry_val(), &mut buf);
                // If the compressed version isn't smaller, ditch it and use the uncompressed 
                // version
                if buf.len() >= entry.entry_len() {
                    entry.into_vec()
                }
                else {
                    // Complete the compressed version by filling in the compressed size and 
                    // appending the signatures
                    let compress_len = buf.len()-3;
                    buf[1] = ((compress_len & 0xFF00) >>  8) as u8;
                    buf[2] = ( compress_len & 0x00FF)        as u8;
                    buf.extend_from_slice(&entry.raw_entry()[entry.entry_len()..]);
                    buf
                }
            }
        };

        Ok(Checklist::new(checklist, buf))
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
    pub fn trusted_decode_entry(&mut self, buf: &mut &[u8], doc: Hash, field: String, hash: Option<Hash>) -> crate::Result<Entry> {
        let checklist = self.internal_decode_entry(buf, doc, field, hash, false)?;
        // If we run internal_decode_entry with no checks, the checklist should be empty and we can 
        // unwrap
        Ok(checklist.complete().unwrap())
    }

    /// Read an [`Entry`] from a byte slice, performing a full set of validation checks when decoding. 
    /// On successful validation of the entry, a [`Checklist`] is returned, which contains the 
    /// decoded entry. Processing the checklist with this schema will complete the checklist and 
    /// yield the decoded [`Entry`].
    ///
    /// [`Entry`]: ./struct.Entry.html
    /// [`Checklist`]: ./checklist/struct.Checklist.html
    pub fn decode_entry(&mut self, buf: &mut &[u8], doc: Hash, field: String) -> crate::Result<Checklist<Entry>> {
        self.internal_decode_entry(buf, doc, field, None, true)
    }

    fn internal_decode_entry(&mut self, buf: &mut &[u8], doc: Hash, field: String, hash: Option<Hash>, do_checks: bool)
        -> crate::Result<Checklist<Entry>>
    {
        let raw_ref: &[u8] = buf;
        if buf.len() >= MAX_ENTRY_SIZE {
            return Err(Error::BadSize);
        }

        // Make sure the schema accepts this entry type
        let validator = self.entries.binary_search_by(|x| x.0.as_str().cmp(&field))
            .map_err(|_| Error::FailValidate(buf.len(), "Entry field is not in schema"))?;
        let validator = self.entries[validator].1;

        // Read the header & check it
        let compress_type = CompressType::decode(buf)?;
        if let CompressType::Compressed = compress_type {
            return Err(Error::BadHeader);
        }
        let data_size = buf.read_u16::<BigEndian>()? as usize;
        if data_size > buf.len() {
            return Err(Error::BadHeader);
        }
        let sign_size = buf.len() - data_size;

        // Construct decoded data
        // ----------------------
        let (entry, compressed, entry_len) = match compress_type {
            CompressType::Compressed => {
                return Err(Error::BadHeader); // We shouldn't have been able to reach here - should've been ccaught earlier.
            },
            CompressType::Uncompressed => {
                (Vec::from(raw_ref), None, data_size+3)
            },
            CompressType::CompressedNoSchema | CompressType::DictCompressed => {
                // Push header to decoded data
                let mut entry = vec![CompressType::Uncompressed.into_u8(), 0, 0];

                // Decompress payload
                if let Ok(index) = self.entries_compress.binary_search_by(|x| x.0.as_str().cmp(&field)) {
                    let compress = &self.entries_compress[index].1;
                    compress.decompress(
                        &mut self.decompressor,
                        MAX_ENTRY_SIZE,
                        sign_size+3,
                        compress_type,
                        &buf[..(buf.len()-sign_size)],
                        &mut entry
                    )?;
                }
                else if let CompressType::CompressedNoSchema = compress_type {
                    zstd_help::decompress(
                        &mut self.decompressor,
                        MAX_ENTRY_SIZE,
                        sign_size+3,
                        &buf[..(buf.len()-sign_size)],
                        &mut entry
                    )?;
                }
                else {
                    return Err(Error::FailDecompress);
                }

                // Calculate the data size (no 3-byte header) and entry length (with 3-byte header)
                let data_size = entry.len() - 3;
                // Write the data size to the header
                entry[1] = ((data_size & 0xFF00) >> 8) as u8;
                entry[2] = ( data_size & 0x00FF)       as u8;
                entry.extend_from_slice(&buf[(buf.len()-sign_size)..]);
                // Check for a bad size one last time before completing the decoding
                if entry.len() >= MAX_ENTRY_SIZE {
                    return Err(Error::BadSize);
                }
                (entry, Some(Vec::from(raw_ref)), data_size + 3)
            }
        };

        // Verify the entry passes validation
        // ----------------------------------
        let mut checklist = ValidatorChecklist::new();
        if do_checks {
            let mut entry_ptr: &[u8] = &entry[3..entry_len];
            self.types[validator].validate(&mut entry_ptr, &self.types, 0, &mut checklist)?;
            if !entry_ptr.is_empty() {
                // Fail with BadHeader since this means the header's 2-byte size tag 
                // mis-represented the size of the fog-pack value. Or, alternately, that the 
                // compressed data payload had more than just the rest of a fog-pack value in it.
                return Err(Error::BadHeader);
            }
        }

        let entry = Entry::from_decoded(
            doc,
            field,
            entry,
            entry_len,
            compressed,
            hash,
            do_checks
        )?;

        Ok(Checklist::new(checklist, entry))
    }

    /// Checks a document against a given ChecklistItem. Marks the item as done on success. Fails 
    /// if validation fails.
    ///
    /// A [`ChecklistItem`] comes from a [`Checklist`].
    ///
    /// [`ChecklistItem`]: ./checklist/struct.ChecklistItem.html
    /// [`Checklist`]: ./checklist/struct.Checklist.html
    pub fn check_item(&self, doc: &Document, item: &mut ChecklistItem) -> crate::Result<()> {
        for index in item.iter() {
            if let Validator::Hash(ref v) = self.types[*index] {
                // Check against acceptable schemas
                if v.schema_required() {
                    if let Some(hash) = doc.schema_hash() {
                        if !v.schema_in_set(&hash) {
                            return Err(Error::FailValidate(doc.size(), "Document uses unrecognized schema"));
                        }
                    }
                    else {
                        return Err(Error::FailValidate(doc.size(), "Document doesn't have schema, but needs one"));
                    }
                }
                if let Some(link) = v.link() {
                    let mut checklist = ValidatorChecklist::new();
                    if let Validator::Object(ref v) = self.types[link] {
                        v.validate(&mut doc.raw_doc(), &self.types, &mut checklist, true)?;
                    }
                    else {
                        return Err(Error::FailValidate(doc.size(), "Can't validate a document against a non-object validator"));
                    }
                }
            }
            else {
                return Err(Error::FailValidate(doc.size(), "Can't validate against non-hash validator"));
            }
        };
        item.mark_done();
        Ok(())
    }

    /// Read a [`Query`] from a byte slice, performing a set of checks to verify the query is 
    /// allowed by this schema. It can fail if the encoded query doesn't match allowed validator 
    /// semantics, or if the query is not permitted by the schema.
    ///
    /// [`Query`]: ./struct.Query.html
    pub fn decode_query(&self, buf: &mut &[u8]) -> crate::Result<Query> {

        // Grab document hash and field first
        let doc_hash = Hash::decode(buf)?;
        let field = read_string(buf)?;

        // Make sure we recognize the field
        let self_validator = self.entries.binary_search_by(|x| x.0.as_str().cmp(&field))
            .map_err(|_| Error::FailValidate(buf.len(), "Query field is not in schema"))?;
        let self_validator = self.entries[self_validator].1;

        // Read the query's content
        match CompressType::decode(buf)? {
            CompressType::Uncompressed => {},
            _ => { return Err(Error::BadHeader); }
        }
        let data_size = buf.read_u16::<BigEndian>()? as usize;
        if data_size > buf.len() {
            return Err(Error::BadHeader);
        }
        let content = Vec::from(&buf[..data_size]);

        // Read the validator out
        let mut types = Vec::with_capacity(3);
        let mut type_names = HashMap::new();
        types.push(Validator::Invalid);
        types.push(Validator::Valid);
        let empty_hash = Hash::new_empty();
        let valid = {
            let mut reader = ValidReader::new(false, &mut types, &mut type_names, &empty_hash);
            Validator::read_validator(&mut &content[..], &mut reader)?
        };

        // Check to see if validator is allowed by schema
        if !query_check(self_validator, valid, &self.types, &types) {
            return Err(Error::FailValidate(content.len(), "Query is not allowed by schema"));
        }

        // Compute the hashes
        let (_, content_hash, hash) =
            Entry::calc_hash_state(&doc_hash, field.as_str(), &buf[..], data_size);

        // Get & verify signatures
        let mut signed_by = Vec::new();
        let mut buf: &[u8] = &buf[data_size..];
        while !buf.is_empty() {
            let signature = crypto::Signature::decode(&mut buf)?;
            if !signature.verify(&content_hash) {
                return Err(Error::BadSignature);
            }
            signed_by.push(signature.signed_by().clone());
        }

        Ok(Query::from_parts(
            valid,
            types.into_boxed_slice(),
            hash,
            doc_hash,
            field,
            content,
            signed_by,
        ))
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
    fn read_raw(raw: &mut &[u8]) -> crate::Result<Compression> {
        let mut setting_seen = false;
        let mut format_seen = false;
        let mut level_seen = false;
        let mut level = zstd_safe::CLEVEL_DEFAULT;
        let mut format = 0;
        let mut setting = None;
        let mut setting_bool = false;

        let num_fields = match read_marker(raw)? {
            MarkerType::Object(len) => len,
            _ => return Err(Error::FailValidate(raw.len(), "Compress spec wasn't an object")),
        };
        object_iterate(raw, num_fields, |field, raw| {
            match field {
                "format" => {
                    format_seen = true;
                    if let Some(i) = read_integer(raw)?.as_u64() {
                        if i > 31 {
                            Err(Error::FailValidate(raw.len(),
                                "Compress `format` field didn't contain integer between 0 and 31"))
                        }
                        else {
                            format = i;
                            Ok(())
                        }
                    }
                    else {
                        Err(Error::FailValidate(raw.len(),
                            "Compress `format` field didn't contain integer between 0 and 31"))
                    }
                },
                "level" => {
                    level_seen = true;
                    if let Some(i) = read_integer(raw)?.as_u64() {
                        if i > 255 {
                            Err(Error::FailValidate(raw.len(),
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
                        Err(Error::FailValidate(raw.len(),
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
                            Err(Error::FailValidate(raw.len(),
                                "Compress `setting` field didn't contain boolean or binary data"))
                        }
                    }
                },
                _ => {
                    Err(Error::FailValidate(raw.len(), "Compress contains unrecognized field"))
                }
            }
        })?;

        // Checks to verify we met the allowed object format
        if !setting_seen {
            return Err(Error::FailValidate(raw.len(), "Compress spec didn't have setting field"));
        }
        if !setting_bool && (format_seen || level_seen) {
            return Err(Error::FailValidate(raw.len(), "Compress spec had false setting field, but other fields were also present"));
        }
        if !format_seen && setting_bool {
            return Err(Error::FailValidate(raw.len(), "Compress spec had setting field not set to false, but no format field"));
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
        extra_size: usize,
        compress_type: CompressType,
        buf: &[u8],
        decode: &mut Vec<u8>
    )
        -> crate::Result<()>
    {
        match compress_type {
            CompressType::Uncompressed => {
                if (buf.len() + decode.len() + extra_size) >= max_size {
                    return Err(Error::BadSize);
                }
                decode.extend_from_slice(buf);
                Ok(())
            },
            CompressType::Compressed | CompressType::CompressedNoSchema => {
                zstd_help::decompress(decompressor, max_size, extra_size, buf, decode)
            },
            CompressType::DictCompressed => {
                // Decompress the data
                // Find the expected size, and fail if it's larger than the maximum allowed size.
                if let Compression::DictCompress((_,dict)) = &self {
                    zstd_help::dict_decompress(decompressor, dict, max_size, extra_size, buf, decode)
                }
                else {
                    Err(Error::FailDecompress)
                }
            }
        }
    }

    fn encode(&self, buf: &mut Vec<u8>) {
        match self {
            Compression::NoCompress => CompressType::Uncompressed.encode(buf),
            Compression::Compress(_) => CompressType::Compressed.encode(buf),
            Compression::DictCompress(_) => CompressType::DictCompressed.encode(buf),
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
        let mut test = simple_doc(schema.hash());

        let enc = schema.encode_doc(test.clone()).unwrap();
        let dec = schema.decode_doc(&mut &enc[..]).unwrap();
        assert!(test == dec, "Document didn't stay same through enc/dec");

        test.set_compression(None);
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

    #[test]
    fn entry_encode_decode() {
        let schema = simple_schema();
        let mut schema = Schema::from_doc(schema).unwrap();
        let test_doc = simple_doc(schema.hash());
        let mut test = simple_entry(test_doc.hash());

        test.set_compression(None);
        let enc = schema.encode_entry(test.clone()).unwrap().complete().unwrap();
        println!("{:X?}", enc);
        println!("{}", enc.len());
        let dec = schema.decode_entry(&mut &enc[..], test.doc_hash().clone(), test.field().to_string())
            .unwrap().complete().unwrap();
        assert!(test == dec, "Entry didn't stay same through compressed enc/dec");

        test.reset_compression();
        let enc = schema.encode_entry(test.clone()).unwrap().complete().unwrap();
        let dec = schema.decode_entry(&mut &enc[..], test.doc_hash().clone(), test.field().to_string())
            .unwrap().complete().unwrap();
        assert!(test == dec, "Entry didn't stay same through uncompressed enc/dec");
    }

    /*
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

