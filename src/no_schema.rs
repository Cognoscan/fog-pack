use std::io;
use std::io::ErrorKind::InvalidData;
use CompressType;
use super::{MAX_DOC_SIZE, MAX_ENTRY_SIZE, Hash, Document, Entry, Value};
use super::document::parse_schema_hash;
use decode;
use encode;
use crypto;
use zstd_help;

/// An encoder/decoder for when no Schema is being used.
///
/// `NoSchema` is used to encode/decode both Documents & Entries when there is no associated Schema 
/// in use. It shouldn't be used with any Document that has a schema, or with any Entry whose 
/// parent Document has a schema.
pub struct NoSchema {
    compress: zstd_safe::CCtx<'static>,
    decompress: zstd_safe::DCtx<'static>,
}

impl NoSchema {
    
    /// Create a new NoSchema instance.
    pub fn new() -> NoSchema {
        NoSchema {
            compress: zstd_safe::create_cctx(),
            decompress: zstd_safe::create_dctx(),
        }
    }

    /// Encode the document and write it to an output buffer. By default, compression with the zstd 
    /// default will be used, which may be overridden by the Document.
    ///
    /// # Errors
    ///
    /// Fails if the Document has an associated schema.
    ///
    /// # Panics
    ///
    /// Panics if the underlying zstd calls return an error, which shouldn't be possible with the 
    /// way they are used in this library.
    pub fn encode_doc(&mut self, doc: Document, buf: &mut Vec<u8>) -> io::Result<()> {
        let len = doc.len();
        let raw: &[u8] = doc.raw_doc();
        assert!(len <= MAX_DOC_SIZE,
            "Document was larger than maximum size! Document implementation should've made this impossible!");

        let compress = if doc.override_compression() {
            doc.compression()
        }
        else {
            Some(zstd_safe::CLEVEL_DEFAULT)
        };

        if doc.schema_hash().is_some() {
            return Err(io::Error::new(InvalidData, "Document has a schema (encode_doc)"));
        }

        if let Some(level) = compress {
            CompressType::CompressedNoSchema.encode(buf);
            zstd_help::compress(&mut self.compress, level, raw, buf);
        }
        else {
            CompressType::Uncompressed.encode(buf);
            buf.extend_from_slice(raw);
        }
        Ok(())
    }

    /// Encode an entry and write it to an output buffer. Includes the entry content only, not the 
    /// parent document hash or the field. By default, compression with the zstd default will be 
    /// used, which may be overridden by the Document. This panics if the underlying zstd calls 
    /// return an error, which shouldn't be possible witih the way they are used in this library.
    pub fn encode_entry(&mut self, entry: &Entry, buf: &mut Vec<u8>) {
        let len = entry.len();
        let raw: &[u8] = entry.raw_entry();
        assert!(len <= MAX_ENTRY_SIZE,
            "Entry was larger than maximum size! Entry implementation should've made this impossible!");

        let compress = if entry.override_compression() {
            entry.compression()
        }
        else {
            Some(zstd_safe::CLEVEL_DEFAULT)
        };

        if let Some(level) = compress {
            CompressType::CompressedNoSchema.encode(buf);
            zstd_help::compress(&mut self.compress, level, raw, buf);
        }
        else {
            CompressType::Uncompressed.encode(buf);
            buf.extend_from_slice(raw);
        }
    }

    /// Read a document from a byte slice, trusting the origin of the slice and doing as few checks 
    /// as possible when decoding. It fails if there isn't a valid fogpack value, the compression 
    /// isn't recognized/is invalid, the slice terminates early, or if the document is using a 
    /// compression schema or compression method requiring a schema.
    ///
    /// Rather than compute the hash, the document hash can optionally be provided. If integrity 
    /// checking is desired, provide no hash and compare the expected hash with the hash of the 
    /// resulting document.
    ///
    /// The *only* time this should be used is if the byte slice is coming from a well-trusted 
    /// location, like an internal database.
    pub fn trusted_decode_doc(&mut self, buf: &mut &[u8], hash: Option<Hash>) -> io::Result<Document> {
        // TODO: Change this function so that it doesn't copy any data until the very end.
        let (doc, compressed) = self.decode_raw(MAX_DOC_SIZE, buf)?;

        // Check for a schema
        if let Some(_) = parse_schema_hash(&mut &doc[..])? {
            return Err(io::Error::new(InvalidData, "Document has a schema (trusted_decode_doc)"));
        }

        // Parse the document itself & optionally start up the hasher
        let doc_len = decode::verify_value(&mut &doc[..])?;

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

    /// Read a document from a byte slice, performing a full set of validation checks when 
    /// decoding. Success guarantees that the resulting Document is valid, and as such, this can be 
    /// used with untrusted inputs.
    ///
    /// Validation checking means this will fail if:
    /// - The data is corrupted or incomplete
    /// - The data isn't valid fogpack 
    /// - The compression is invalid or expands to larger than the maximum allowed size
    /// - The compression requires the schema to decode
    /// - The decompressed document has an associated schema hash
    /// - Any of the attached signatures are invalid
    pub fn decode_doc(&mut self, buf: &mut &[u8]) -> io::Result<Document> {
        // TODO: Change this function so that it doesn't copy any data until the very end.
        let (doc, compressed) = self.decode_raw(MAX_DOC_SIZE, buf)?;

        // Parse the document itself
        if parse_schema_hash(&mut &doc[..])?.is_some() {
            return Err(io::Error::new(InvalidData, "Document has a schema (decode_doc)"));
        }
        let doc_len = decode::verify_value(&mut &doc[..])?;

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
        let mut index = &mut &doc[doc_len..];
        while index.len() > 0 {
            let signature = crypto::Signature::decode(&mut index)
                .map_err(|_e| io::Error::new(InvalidData, "Invalid signature in raw document"))?;
            if !signature.verify(&doc_hash) {
                return Err(io::Error::new(InvalidData, "Signature doesn't verify against document"));
            }
            signed_by.push(signature.signed_by().clone());
        }

        let override_compression = false;
        let compression = None;
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

    /// Read an entry from a byte slice, trusting the origin of the slice and doing as few checks 
    /// as possible when decoding. Functions like [`trusted_decode_doc`], but for entries.
    ///
    /// [`trusted_decode_doc`]: #method.trusted_decode_doc
    pub fn trusted_decode_entry(&mut self, buf: &mut &[u8], doc: Hash, field: String, hash: Option<Hash>) -> io::Result<Entry> {
        // TODO: Change this function so that it doesn't copy any data until the very end.
        let (entry, compressed) = self.decode_raw(MAX_ENTRY_SIZE, buf)?;

        // Parse the document itself & load in the optional hash
        let entry_len = decode::verify_value(&mut &entry[..])?;
        let hash_provided = hash.is_some();
        let hash = hash.unwrap_or(Hash::new_empty());

        // Get signatures
        let mut signed_by = Vec::new();
        let mut index = &mut &entry[entry_len..];
        while index.len() > 0 {
            let signature = crypto::Signature::decode(&mut index)
                .map_err(|_e| io::Error::new(InvalidData, "Invalid signature in raw document"))?;
            signed_by.push(signature.signed_by().clone());
        }

        let override_compression = false;
        let compression = None;
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

    /// Read an entry from a byte slice, performing a full set of validation checks when decoding. 
    /// Success guarantees the resulting entry is valid, and as such, this can be used with 
    /// untrusted inputs. Functions like [`decode_doc`]; see its documentation for the possible 
    /// failure cases.
    ///
    /// [`decode_doc`]: #method.decode_doc
    pub fn decode_entry(&mut self, buf: &mut &[u8], doc: Hash, field: String) -> io::Result<Entry> {
        // TODO: Change this function so that it doesn't copy any data until the very end.
        let (entry, compressed) = self.decode_raw(MAX_ENTRY_SIZE, buf)?;

        let entry_len = decode::verify_value(&mut &entry[..])?;

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
        } else {
            entry_hash.clone()
        };

        // Get signatures
        let mut signed_by = Vec::new();
        let mut index = &mut &entry[entry_len..];
        while index.len() > 0 {
            let signature = crypto::Signature::decode(&mut index)
                .map_err(|_e| io::Error::new(InvalidData, "Invalid signature in raw entry"))?;
            if !signature.verify(&entry_hash) {
                return Err(io::Error::new(InvalidData, "Signature doesn't verify against entry"));
            }
            signed_by.push(signature.signed_by().clone());
        }

        let override_compression = false;
        let compression = None;
        Ok(Entry::from_parts(
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
        ))
    }

    fn decode_raw(&mut self, max_size: usize, buf: &mut &[u8]) -> io::Result<(Vec<u8>, Option<Vec<u8>>)> {
        let compress_type = CompressType::decode(buf)?;
        match compress_type {
            CompressType::Uncompressed => {
                if buf.len() > max_size {
                    return Err(io::Error::new(InvalidData, "Data is larger than maximum allowed size"));
                }
                let mut doc = Vec::new();
                doc.extend_from_slice(buf);
                Ok((doc, None))
            },
            CompressType::CompressedNoSchema => {
                let mut compressed = Vec::new();
                // Save off the compressed data
                compress_type.encode(&mut compressed);
                compressed.extend_from_slice(buf);
                let mut decode = Vec::new();
                zstd_help::decompress(&mut self.decompress, max_size, buf, &mut decode)?;
                Ok((decode, Some(compressed)))
            },
            CompressType::Compressed | CompressType::DictCompressed => {
                return Err(io::Error::new(InvalidData, "Data uses a schema, but NoSchema struct was used for decoding"));
            },
        }
    }

}

fn _assert_traits() {
    fn _assert_send<T: Send>(_: T) {}
    _assert_send(NoSchema::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::Value;
    use crate::crypto::{Vault, PasswordLevel, Key};
    use Schema;

    fn prep_vault() -> (Vault, Key) {
        let mut vault = Vault::new_from_password(PasswordLevel::Interactive, "test".to_string())
            .expect("Should have been able to make a new vault for testing");
        let key = vault.new_key();
        (vault, key)
    }

    fn test_doc() -> Document {
        let test: Value = fogpack!({
            "test": true,
            "boolean": true,
            "positive": 1,
            "negative": -1,
            "string": "string",
            "float32": 1.0f32,
            "float64": 1.0f64,
            "binary": vec![0u8,1u8,2u8],
            "array": [Value::from(0), Value::from("an_array")] 
        });
        Document::new(test).expect("Should've been able to encode as a document")
    }

    fn test_doc_with_schema() -> (Schema, Document) {
        let schema: Value = fogpack!({
            "req": {
                "test": { "type": "Bool" },
                "boolean": { "type": "Bool" }
            }
        });
        let schema = Document::new(schema).unwrap();
        let schema = Schema::from_doc(schema).unwrap();
        let test: Value = fogpack!({
            "" : Value::from(schema.hash().clone()),
            "test": true,
            "boolean": true,
        });
        let doc = Document::new(test).expect("Should've been able to encode as a document");
        (schema, doc)
    }


    fn test_entry() -> Entry {
        let test: Value = fogpack!(vec![0u8, 1u8, 2u8]);
        Entry::new(Hash::new_empty(), String::from(""), test).expect("Should've been able to encode as an entry")
    }

    #[test]
    fn doc_empty_content() {
        let test = Document::new(fogpack!({})).unwrap();
        let mut enc = Vec::new();
        let mut schema_none = NoSchema::new();
        schema_none.encode_doc(test.clone(), &mut enc).unwrap();
        let dec = schema_none.decode_doc(&mut &enc[..]).expect("Decoding should have worked");
        assert!(test == dec, "Encode->Decode should yield same document");
    }

    #[test]
    fn doc_encode_decode() {
        let mut test = test_doc();
        test.set_compression(None);
        let mut schema_none = NoSchema::new();
        let mut enc = Vec::new();
        schema_none.encode_doc(test.clone(), &mut enc).unwrap();
        let mut dec = schema_none.trusted_decode_doc(&mut &enc[..], None).expect("Decoding should have worked");
        let mut enc2 = Vec::new();
        dec.set_compression(None);
        schema_none.encode_doc(dec.clone(), &mut enc2).unwrap();
        assert!(test == dec, "Encode->Decode should yield same document");
        assert!(enc == enc2, "Encode->Decode->encode didn't yield identical results");
    }

    #[test]
    fn doc_compress_decompress() {
        let test = test_doc();
        let mut schema_none = NoSchema::new();
        let mut enc = Vec::new();
        schema_none.encode_doc(test.clone(), &mut enc).unwrap();
        let dec = schema_none.trusted_decode_doc(&mut &enc[..], None).expect("Decoding should have worked");
        assert!(test == dec, "Compress->Decode should yield same document");
    }

    #[test]
    fn doc_compress_decompress_sign() {
        let mut test = test_doc();
        let (mut vault, key0) = prep_vault();
        let key1 = vault.new_key();
        let key2 = vault.new_key();
        test.sign(&vault, &key0).expect("Should have been able to sign test document w/ key0");
        test.sign(&vault, &key1).expect("Should have been able to sign test document w/ key1");
        let mut schema_none = NoSchema::new();
        let mut enc = Vec::new();
        schema_none.encode_doc(test.clone(), &mut enc).unwrap();
        let mut dec = schema_none.trusted_decode_doc(&mut &enc[..], None).expect("Decoding should have worked");
        test.sign(&vault, &key2).expect("Should have been able to sign test document w/ key2");
        dec.sign(&vault, &key2).expect("Should have been able to sign decoded document w/ key2");
        assert!(test == dec, "Compress->Decode should yield same document, even after signing");
    }

    #[test]
    fn doc_compress_sign_existing_hash() {
        let mut test = test_doc();
        let (vault, key) = prep_vault();
        let mut schema_none = NoSchema::new();
        let mut enc = Vec::new();
        schema_none.encode_doc(test.clone(), &mut enc).unwrap();
        let mut dec = schema_none.trusted_decode_doc(&mut &enc[..], Some(test.hash().clone())).expect("Decoding should have worked");
        test.sign(&vault, &key).expect("Should have been able to sign test document");
        dec.sign(&vault, &key).expect("Should have been able to sign decoded document");
        assert!(test == dec, "Compress->Decode should yield same document, even after signing");
    }

    #[test]
    fn doc_compress_schema_decode_fails() {
        let (mut schema, test) = test_doc_with_schema();
        let mut schema_none = NoSchema::new();
        let mut enc = Vec::new();
        assert!(schema_none.encode_doc(test.clone(), &mut enc).is_err());
        schema.encode_doc(test.clone(), &mut enc).unwrap();
        let dec = schema_none.trusted_decode_doc(&mut &enc[..], Some(test.hash().clone()));
        assert!(dec.is_err(), "Decompression should have failed, as a schema was in the document");
    }

    #[test]
    fn doc_strict_decode() {
        // Prep encode/decode & byte vector
        let mut schema_none = NoSchema::new();
        let mut enc = Vec::new();

        // Prep schema-using document
        let (mut schema, mut test) = test_doc_with_schema();

        test.set_compression(None);
        assert!(schema_none.encode_doc(test.clone(), &mut enc).is_err());
        schema.encode_doc(test.clone(), &mut enc).unwrap();
        let dec = schema_none.decode_doc(&mut &enc[..]);
        assert!(dec.is_err(), "Decoding should have failed when a schema was in the document");

        enc.clear();
        test.reset_compression();
        schema.encode_doc(test.clone(), &mut enc).unwrap();
        let dec = schema_none.decode_doc(&mut &enc[..]);
        assert!(dec.is_err(), "Decompression should have failed when a schema was in the document");

        // Prep new non-schema document with signature
        let (vault, key) = prep_vault();
        let mut test = test_doc();
        test.sign(&vault, &key).expect("Should have been able to sign test document");

        enc.clear();
        test.set_compression(None);
        schema_none.encode_doc(test.clone(), &mut enc).unwrap();
        let dec = schema_none.decode_doc(&mut &enc[..]);
        assert!(dec.is_ok(), "Decoding a valid document should have succeeded");
        
    }

    #[test]
    fn doc_corrupted_data() {
        // Prep encode/decode & byte vector
        let mut schema_none = NoSchema::new();
        let mut enc = Vec::new();
        // Prep a non-schema document with a signature
        let (vault, key) = prep_vault();
        let mut test = test_doc();
        test.sign(&vault, &key).expect("Should have been able to sign test document");

        test.set_compression(None);
        schema_none.encode_doc(test.clone(), &mut enc).unwrap();
        *(enc.last_mut().unwrap()) = *enc.last_mut().unwrap() ^ 0xFF;
        let dec = schema_none.decode_doc(&mut &enc[..]);
        assert!(dec.is_err(), "Document signature was corrupted, but decoding succeeded anyway");

        enc.clear();
        schema_none.encode_doc(test.clone(), &mut enc).unwrap();
        enc[10] = 0xFF;
        let dec = schema_none.decode_doc(&mut &enc[..]);
        assert!(dec.is_err(), "Document payload was corrupted, but decoding succeeded anyway");

        enc.clear();
        schema_none.encode_doc(test.clone(), &mut enc).unwrap();
        enc[0] = 0x1;
        let dec = schema_none.decode_doc(&mut &enc[..]);
        assert!(dec.is_err(), "Document payload was corrupted, but decoding succeeded anyway");
    }

    #[test]
    fn entry_encode_decode() {
        let mut test = test_entry();
        test.set_compression(None);
        let mut schema_none = NoSchema::new();
        let mut enc = Vec::new();
        schema_none.encode_entry(&test, &mut enc);
        let mut dec = schema_none.trusted_decode_entry(&mut &enc[..], Hash::new_empty(), String::from(""), None)
            .expect("Decoding should have worked");
        let mut enc2 = Vec::new();
        dec.set_compression(None);
        schema_none.encode_entry(&dec, &mut enc2);
        assert!(test == dec, "Encode->Decode should yield same entry");
        assert!(enc == enc2, "Encode->Decode->encode didn't yield identical results");
    }

    #[test]
    fn entry_compress_decompress() {
        let test = test_entry();
        let mut schema_none = NoSchema::new();
        let mut enc = Vec::new();
        schema_none.encode_entry(&test, &mut enc);
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
        let mut enc = Vec::new();
        schema_none.encode_entry(&test, &mut enc);
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
        let mut enc = Vec::new();
        schema_none.encode_entry(&test, &mut enc);
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
        let mut enc = Vec::new();
        schema_none.encode_entry(&test, &mut enc);
        let dec = schema_none.decode_entry(&mut &enc[..], Hash::new_empty(), String::from(""))
            .expect("Decoding should have worked");
        assert!(test == dec, "Compress->Decode should yield same entry");
    }

    #[test]
    fn entry_corrupted_data() {
        // Prep encode/decode & byte vector
        let mut schema_none = NoSchema::new();
        let mut enc = Vec::new();
        // Prep a entry
        let (vault, key) = prep_vault();
        let mut test = test_entry();
        test.sign(&vault, &key).expect("Should have been able to sign test entry");

        test.set_compression(None);
        schema_none.encode_entry(&test, &mut enc);
        *(enc.last_mut().unwrap()) = *enc.last_mut().unwrap() ^ 0xFF;
        let dec = schema_none.decode_entry(&mut &enc[..], Hash::new_empty(), String::from(""));
        assert!(dec.is_err(), "Entry signature was corrupted, but decoding succeeded anyway");

        enc.clear();
        schema_none.encode_entry(&test, &mut enc);
        // Targets part of the binary sequence, which should break the signature, but not the Value verification
        enc[4] = 0xFF;
        let dec = schema_none.decode_entry(&mut &enc[..], Hash::new_empty(), String::from(""));
        assert!(dec.is_err(), "Entry payload was corrupted, but decoding succeeded anyway");

        enc.clear();
        schema_none.encode_entry(&test, &mut enc);
        enc[0] = 0x1; // Targets the compression marker
        let dec = schema_none.decode_entry(&mut &enc[..], Hash::new_empty(), String::from(""));
        assert!(dec.is_err(), "Entry payload was corrupted, but decoding succeeded anyway");
    }

}
