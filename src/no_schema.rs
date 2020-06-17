use byteorder::{ReadBytesExt, BigEndian};

use CompressType;
use super::{MAX_DOC_SIZE, Hash, Document};
use super::document::parse_schema_hash;
use decode;
use zstd_help;
use Error;

/// An encoder/decoder for when no Schema is being used.
///
/// `NoSchema` is used to encode/decode Documents when there is no associated Schema in use. It 
/// shouldn't be used with any Document that has a schema, and cannot be used with Entries at all.
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

    /// Encode the document and write it to a byte vector. By default, compression with the zstd 
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
    pub fn encode_doc(&mut self, doc: Document) -> crate::Result<Vec<u8>> {
        let len = doc.size();
        assert!(len < MAX_DOC_SIZE,
            "Document was larger than maximum size! Document implementation should've made this impossible!");

        // Determine what compression we'll use
        let compress = if doc.override_compression() {
            doc.compression()
        }
        else {
            Some(zstd_safe::CLEVEL_DEFAULT)
        };

        // Give up now if the document has an associated schema.
        if doc.schema_hash().is_some() {
            return Err(Error::SchemaMismatch);
        }

        // Attempt compression, but throw away the result if it's larger than before.
        // Additionally, if we already have the compressed data, just reuse that.
        if doc.compress_cache() {
            Ok(doc.into_compressed_vec())
        }
        else if let Some(level) = compress {
            let mut buf = Vec::new();
            // Header with placeholder bytes for compressed size
            CompressType::CompressedNoSchema.encode(&mut buf);
            buf.push(0u8);
            buf.push(0u8);
            buf.push(0u8);
            let raw: &[u8] = &doc.raw_doc()[4..doc.doc_len()];
            zstd_help::compress(&mut self.compress, level, raw, &mut buf);
            // If the compressed version isn't smaller, ditch it and use the uncompressed version
            if buf.len() >= doc.doc_len() {
                Ok(doc.into_vec())
            }
            else {
                // Complete the compressed version by filling in the compressed size and appending 
                // the signatures
                let compress_len = buf.len()-4;
                buf[1] = ((compress_len & 0x00FF_0000) >> 16) as u8;
                buf[2] = ((compress_len & 0x0000_FF00) >>  8) as u8;
                buf[3] = ( compress_len & 0x0000_00FF)        as u8;
                buf.extend_from_slice(&doc.raw_doc()[doc.doc_len()..]);
                Ok(buf)
            }
        }
        else {
            // We don't have to do anything if we're not compressing, we already have the encoded 
            // data inside the Document
            Ok(doc.into_vec())
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
    pub fn trusted_decode_doc(&mut self, buf: &mut &[u8], hash: Option<Hash>) -> crate::Result<Document> {
        self.internal_decode_doc(buf, hash, false)
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
        // Read header
        let compress_type = CompressType::decode(buf)?;
        let data_size = buf.read_u24::<BigEndian>()? as usize;
        if data_size > buf.len() {
            return Err(Error::BadHeader);
        }
        // Attempt to read and decode contents, copying out both the compressed and uncompressed 
        // data. doc_len is the size of the data payload plus the 4-byte header.
        let (doc, compressed, doc_len) = match compress_type {
            CompressType::Uncompressed => {
                (Vec::from(raw_ref), None, data_size+4)
            },
            CompressType::CompressedNoSchema => {
                let mut decode = Vec::new();
                CompressType::Uncompressed.encode(&mut decode);
                decode.push(0u8);
                decode.push(0u8);
                decode.push(0u8);
                zstd_help::decompress(&mut self.decompress, MAX_DOC_SIZE-4, buf.len()-data_size, &buf[..data_size], &mut decode)?;
                let decoded_size = decode.len() - 4; // 4 from header
                decode[1] = ((decoded_size & 0x00FF_0000) >> 16) as u8;
                decode[2] = ((decoded_size & 0x0000_FF00) >>  8) as u8;
                decode[3] = ( decoded_size & 0x0000_00FF)        as u8;
                decode.extend_from_slice(&buf[data_size..]); // Extend with signatures
                if decode.len() >= MAX_DOC_SIZE {
                    return Err(Error::BadSize);
                }
                (decode, Some(Vec::from(raw_ref)), decoded_size+4)
            },
            CompressType::Compressed | CompressType::DictCompressed => {
                return Err(Error::SchemaMismatch);
            },
        };

        // Verify there is a valid fog-pack value with no schema
        if parse_schema_hash(&mut &doc[4..doc_len])?.is_some() {
            return Err(Error::SchemaMismatch);
        }
        if do_checks {
            // Verify we have a fog-pack value, and fail if there's more data stuffed in after it. 
            // Fail with BadHeader since that means the header's 3-byte size tag mis-represented 
            // the size of the fog-pack object.
            let len = decode::verify_value(&mut &doc[4..doc_len])?;
            if len != doc_len-4 { return Err(Error::BadHeader); }
        }

        // Pass the everything on to create the actual Document
        Document::from_decoded(
            doc,
            doc_len,
            compressed,
            None,
            hash,
            do_checks
        )
    }

}

impl Default for NoSchema {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn doc_empty_content() {
        let test = Document::new(fogpack!({})).unwrap();
        let mut schema_none = NoSchema::new();
        let enc = schema_none.encode_doc(test.clone()).unwrap();
        println!("{:X?}", enc);
        let dec = schema_none.decode_doc(&mut &enc[..]).expect("Decoding should have worked");
        assert!(test == dec, "Encode->Decode should yield same document");
    }

    #[test]
    fn doc_encode_decode() {
        let mut test = test_doc();
        test.set_compression(None);
        let mut schema_none = NoSchema::new();
        let enc = schema_none.encode_doc(test.clone()).unwrap();
        let mut dec = schema_none.trusted_decode_doc(&mut &enc[..], None).expect("Decoding should have worked");
        dec.set_compression(None);
        let enc2 = schema_none.encode_doc(dec.clone()).unwrap();
        assert!(test == dec, "Encode->Decode should yield same document");
        assert!(enc == enc2, "Encode->Decode->encode didn't yield identical results");
    }

    #[test]
    fn doc_compress_decompress() {
        let test = test_doc();
        let mut schema_none = NoSchema::new();
        let enc = schema_none.encode_doc(test.clone()).unwrap();
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
        let enc = schema_none.encode_doc(test.clone()).unwrap();
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
        let enc = schema_none.encode_doc(test.clone()).unwrap();
        let mut dec = schema_none.trusted_decode_doc(&mut &enc[..], Some(test.hash().clone())).expect("Decoding should have worked");
        test.sign(&vault, &key).expect("Should have been able to sign test document");
        dec.sign(&vault, &key).expect("Should have been able to sign decoded document");
        assert!(test == dec, "Compress->Decode should yield same document, even after signing");
    }

    #[test]
    fn doc_compress_schema_decode_fails() {
        let (mut schema, test) = test_doc_with_schema();
        let mut schema_none = NoSchema::new();
        assert!(schema_none.encode_doc(test.clone()).is_err());
        let enc = schema.encode_doc(test.clone()).unwrap();
        let dec = schema_none.trusted_decode_doc(&mut &enc[..], Some(test.hash().clone()));
        assert!(dec.is_err(), "Decompression should have failed, as a schema was in the document");
    }

    #[test]
    fn doc_strict_decode() {
        // Prep encode/decode & byte vector
        let mut schema_none = NoSchema::new();

        // Prep schema-using document
        let (mut schema, mut test) = test_doc_with_schema();

        test.set_compression(None);
        assert!(schema_none.encode_doc(test.clone()).is_err());
        let enc = schema.encode_doc(test.clone()).unwrap();
        let dec = schema_none.decode_doc(&mut &enc[..]);
        assert!(dec.is_err(), "Decoding should have failed when a schema was in the document");

        test.reset_compression();
        let enc = schema.encode_doc(test.clone()).unwrap();
        let dec = schema_none.decode_doc(&mut &enc[..]);
        assert!(dec.is_err(), "Decompression should have failed when a schema was in the document");

        // Prep new non-schema document with signature
        let (vault, key) = prep_vault();
        let mut test = test_doc();
        test.sign(&vault, &key).expect("Should have been able to sign test document");

        test.set_compression(None);
        let enc = schema_none.encode_doc(test.clone()).unwrap();
        let dec = schema_none.decode_doc(&mut &enc[..]);
        assert!(dec.is_ok(), "Decoding a valid document should have succeeded");
        
    }

    #[test]
    fn doc_corrupted_data() {
        // Prep encode/decode & byte vector
        let mut schema_none = NoSchema::new();
        // Prep a non-schema document with a signature
        let (vault, key) = prep_vault();
        let mut test = test_doc();
        test.sign(&vault, &key).expect("Should have been able to sign test document");
        test.set_compression(None);

        let mut enc = schema_none.encode_doc(test.clone()).unwrap();
        *(enc.last_mut().unwrap()) = *enc.last_mut().unwrap() ^ 0xFF;
        let dec = schema_none.decode_doc(&mut &enc[..]);
        assert!(dec.is_err(), "Document signature was corrupted, but decoding succeeded anyway");

        let mut enc = schema_none.encode_doc(test.clone()).unwrap();
        enc[10] = 0xFF;
        let dec = schema_none.decode_doc(&mut &enc[..]);
        assert!(dec.is_err(), "Document payload was corrupted, but decoding succeeded anyway");

        let mut enc = schema_none.encode_doc(test.clone()).unwrap();
        enc[0] = 0x1;
        let dec = schema_none.decode_doc(&mut &enc[..]);
        assert!(dec.is_err(), "Document payload was corrupted, but decoding succeeded anyway");

        let mut enc = schema_none.encode_doc(test.clone()).unwrap();
        enc[1] = 0xFF;
        enc[2] = 0xFF;
        enc[3] = 0xFF;
        let dec = schema_none.decode_doc(&mut &enc[..]);
        assert!(dec.is_err(), "Document payload was corrupted, but decoding succeeded anyway");
    }

}
