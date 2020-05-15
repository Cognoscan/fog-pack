use std::io;
use std::io::Error;
use std::io::ErrorKind::{InvalidData,Other};
use std::collections::HashMap;

use MarkerType;
use CompressType;
use decode::*;
use document::{extract_schema_hash, parse_schema_hash};
use validator::{ValidObj, Validator, ValidatorChecklist};
use Hash;
use Document;


enum Compression {
    NoCompress,
    Compress(i32),
    DictCompress((zstd_safe::CDict<'static>, zstd_safe::DDict<'static>))
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

    fn compress(&mut self, compressor: &mut zstd_safe::CCtx, raw: &[u8], buf: &mut Vec<u8>) {
        match self {
            Compression::NoCompress => {
                buf.extend_from_slice(raw);
            },
            Compression::Compress(level) => {
                let vec_len = buf.len();
                let mut buffer_len = zstd_safe::compress_bound(raw.len());
                buf.reserve(buffer_len);
                unsafe {
                    buf.set_len(vec_len + buffer_len);
                    buffer_len = zstd_safe::compress_cctx(
                        compressor,
                        &mut buf[vec_len..],
                        raw,
                        *level
                    ).expect("zstd library unexpectedly errored during compress_cctx!");
                    buf.set_len(vec_len + buffer_len);
                }
            },
            Compression::DictCompress((dict, _)) => {
                let vec_len = buf.len();
                let mut buffer_len = zstd_safe::compress_bound(raw.len());
                buf.reserve(buffer_len);
                unsafe {
                    buf.set_len(vec_len + buffer_len);
                    buffer_len = zstd_safe::compress_using_cdict(
                        compressor,
                        &mut buf[vec_len..],
                        raw,
                        dict
                    ).expect("zstd library unexpectedly errored during compress_cctx!");
                    buf.set_len(vec_len + buffer_len);
                }
            },
        }
    }

    // Decompress raw data, after it has been stripped of the `CompressType` byte and the header if 
    // present, which consists of the leading object tag and schema hash field-value pair. max_size 
    // should reflect if the header has already been read. The intent is that the caller of this 
    // function needed to vet the schema hash anyway, and verify that an Entry didn't start with 
    // the Compressed flag, and that a Document didn't start with the CompressedNoSchema flag.
    fn decompress(
        &mut self,
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
                if buf.len() > max_size {
                    return Err(io::Error::new(InvalidData, "Data is larger than maximum allowed size"));
                }
                decode.extend_from_slice(buf);
                Ok(())
            },
            CompressType::Compressed | CompressType::CompressedNoSchema => {
                // Decompress the data
                // Find the expected size, and fail if it's larger than the maximum allowed size.
                let decode_len = decode.len();
                let expected_len = zstd_safe::get_frame_content_size(buf);
                if ((decode_len as u64)+expected_len) > (max_size as u64) {
                    return Err(io::Error::new(InvalidData, "Expected decompressed size is larger than maximum allowed size"));
                }
                let expected_len = expected_len as usize;
                unsafe {
                    decode.set_len(decode_len + expected_len);
                    let len = zstd_safe::decompress_dctx(
                        decompressor,
                        &mut decode[..],
                        buf
                    ).map_err(|_| io::Error::new(InvalidData, "Decompression failed"))?;
                    decode.set_len(decode_len + len);
                }
                Ok(())
            },
            CompressType::DictCompressed => {
                // Decompress the data
                // Find the expected size, and fail if it's larger than the maximum allowed size.
                if let Compression::DictCompress((_,dict)) = &self {
                    // Decompress the data
                    // Find the expected size, and fail if it's larger than the maximum allowed size.
                    let decode_len = decode.len();
                    let expected_len = zstd_safe::get_frame_content_size(buf);
                    if ((decode_len as u64)+expected_len) > (max_size as u64) {
                        return Err(io::Error::new(InvalidData, "Expected decompressed size is larger than maximum allowed size"));
                    }
                    let expected_len = expected_len as usize;
                    unsafe {
                        decode.set_len(decode_len + expected_len);
                        let len = zstd_safe::decompress_using_ddict(
                            decompressor,
                            &mut decode[..],
                            buf,
                            dict
                        ).map_err(|_| io::Error::new(InvalidData, "Decompression failed"))?;
                        decode.set_len(decode_len + len);
                    }
                    Ok(())
                }
                else {
                    Err(io::Error::new(InvalidData, "Schema has no dictionary for this data"))
                }
            }
        }
    }

}


/// Struct holding the validation portions of a schema. Can be used for validation of a document or 
/// entry.
pub struct Schema {
    hash: Hash,
    object: ValidObj,
    entries: Vec<(String, usize)>,
    types: Vec<Validator>,
    doc_encode: Option<zstd_safe::CCtx<'static>>,
    doc_decode: Option<zstd_safe::DCtx<'static>>,
    entry_encode: Vec<(String, Option<zstd_safe::CCtx<'static>>)>,
    entry_decode: Vec<(String, Option<zstd_safe::DCtx<'static>>)>,
}

impl Schema {
    pub fn from_raw(raw: &mut &[u8]) -> io::Result<Schema> {
        let raw_for_hash: &[u8] = raw;
        let mut entries = Vec::new();
        let mut types = Vec::with_capacity(2);
        let mut type_names = HashMap::new();
        let mut object = ValidObj::new(true); // Documents can always be queried, hence "true"
        let doc_encode = Some(zstd_safe::create_cctx());
        let doc_decode = Some(zstd_safe::create_dctx());
        let mut entry_encode = Vec::new();
        let mut entry_decode = Vec::new();
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
                    if let MarkerType::Object(len) = read_marker(raw)? {
                        let mut format = None;
                        let mut setting_seen = false;
                        let mut setting = false;
                        let mut setting_bin = None;
                        object_iterate(raw, len, |field, raw| {
                            match field {
                                "format" => {
                                    format = Some(read_integer(raw)?);
                                },
                                "setting" => {
                                    setting_seen = true;
                                    match read_marker(raw)? {
                                        MarkerType::Boolean(v) => {
                                            setting = v;
                                        },
                                        MarkerType::Binary(len) => {
                                            let v = read_raw_bin(raw, len)?;
                                            setting = true;
                                            setting_bin = Some(v.to_vec());
                                        },
                                        _ => {
                                            return Err(Error::new(InvalidData,
                                                    "`doc_compress`/`setting` field didn't contain boolean or binary data"));
                                        }
                                    }
                                },
                                _ => {
                                    return Err(Error::new(InvalidData,
                                            format!("`doc_compress` contains unrecognized field `{}`", field)));
                                }
                            }
                            if format.is_none() && setting_bin.is_some() {
                                return Err(Error::new(InvalidData,
                                        "Compression specifies a binary setting, but not a format"));
                            }
                            Ok(())
                        })?;
                    }
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
                            let v = Validator::read_validator(raw, false, &mut types, &mut type_names)?;
                            entries.push((field.to_string(), v));
                            entry_encode.push((field.to_string(), Some(zstd_safe::create_cctx())));
                            entry_decode.push((field.to_string(), Some(zstd_safe::create_dctx())));
                            Ok(())
                        })?;
                    }
                    else {
                        return Err(Error::new(InvalidData, "`entries` field doesn't contain an Object"));
                    }
                },
                /*
                "entries_compress" => {
                    if let MarkerType::Object(len) = read_marker(raw)? {
                        object_iterate(raw, len, |field, raw| {
                            Err(Error::new(InvalidData "entries_compress not implemented yet"))
                        }
                },
                */
               "field_type" | "max_fields" | "min_fields" | "req" | "opt" | "unknown_ok" => {
                   object.update(field, raw, false, &mut types, &mut type_names)?;
                },
                "types" => {
                    if let MarkerType::Object(len) = read_marker(raw)? {
                        object_iterate(raw, len, |field, raw| {
                            let v = Validator::read_validator(raw, false, &mut types, &mut type_names)?;
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

        let hash = Hash::new(raw_for_hash);
        Ok(Schema {
            hash,
            object,
            entries,
            types,
            doc_encode,
            doc_decode,
            entry_encode,
            entry_decode,
        })
    }

    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    pub fn encode_doc(&self, _doc: &Document, _buf: &mut Vec<u8>) -> io::Result<()> {
        Err(Error::new(Other, "Document doesn't use this schema"))
    }

    /// Validates a document against this schema. Does not check the schema field itself.
    pub fn validate_doc(&self, doc: &mut &[u8]) -> io::Result<()> {
        let mut checklist = ValidatorChecklist::new();
        self.object.validate("", doc, &self.types, &mut checklist, true).and(Ok(()))
    }

    /// Validates a given entry against this schema.
    pub fn validate_entry(&self, entry: &str, doc: &mut &[u8]) -> io::Result<ValidatorChecklist> {
        let mut checklist = ValidatorChecklist::new();
        let v = self.entries.binary_search_by(|x| x.0.as_str().cmp(entry));
        if v.is_err() { return Err(Error::new(InvalidData, "Entry field type doesn't exist in schema")); }
        let v = self.entries[v.unwrap()].1;
        self.types[v].validate("", doc, &self.types, 0, &mut checklist)?;
        Ok(checklist)
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

