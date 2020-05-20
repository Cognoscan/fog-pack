/*!

Encoding formats used for Documents, Entries, and Queries.

The formats are all similar, and both Documents and Entries start with a 
compression marker to indicate what compression was used, if any.

# Compression Marker

This is a single byte at the start of a Document or Entry indicating the 
compression algorithm used, along with whether a schema hash is present in the 
encoded Document.

```text
+----------+
| 0XXXXXYY |
+----------+

XXXXX is the 5-bit compression algorithm code
YY is the compression type marker
```

Currently, only one algorithm is supported, zstd, which has a code of 0.

The compression type marker can be one of the following:

| Marker | Name               | Description                                 |
| --     | --                 | --                                          |
| 0b00   | Uncompressed       | No compression was performed                |
| 0b01   | CompressedNoSchema | Compression without including a schema hash |
| 0b10   | Compressed         | Compression with a schema hash included     |
| 0b11   | DictCompressed     | Compression using a dictionary              |

# Documents

Encoded documents always start with a compression marker byte, and what follows 
depends on the compression type indicated:

- Uncompressed: No compression used. The document's raw value & signatures 
    directly follow the marker byte.
- CompressedNoSchema: A single zstd frame follows the marker byte, containing 
    the compressed version of the document's raw value & signatures. The 
    document should not contain the empty string as a top-level field.
- Compressed: The document's object marker (with size) follows the marker byte, 
    then the first field-value pair of the object, and finally a zstd frame 
    containing the remainder of the document with signatures. The first 
    field-value pair *must* be the empty string and the hash of a schema.
- DictCompressed: Same as Compressed, except that the zstd frame was made using 
    a dictionary embedded in the schema used by the document.

For all of the above, the zstd frame always contains the estimated decompressed 
length.

# Entries

An encoded entry does not contain the parent hash or field string, which are 
assumed to be provided through some other means - usually, they are implied by 
a query that was used to fetch/generate them. As such, an encoded entry only 
contains the entry's fog-pack value & associated signatures.

Encoded entries always start with a compression marker byte, and what follows 
depends on the compression type indicated:

- Uncompressed: No compression used. The entry's raw value & signatures directly 
    follow the marker byte.
- CompressedNoSchema: A single zstd frame follows the marker byte, containing 
    the compressed version of the entry's raw value & signatures.
- Compressed: Invalid for entries, and should result in a decoding error.
- DictCompressed: Same as CompressedNoSchema, except that the zstd frame was 
    made using a dictionary embedded in the schema used by the entry's parent 
    document. The dictionary used depends on the entry's field.

For all of the above, the zstd frame always contains the estimated decompressed 
length.

# Queries

Queries do not use compression, and are simply a byte sequence of:

1. The hash of the queried document, encoded like the Hash type but without the 
   ext family wrapper - effectively just the version byte followed by the hash 
   itself.
2. The field string being queried, encoded as a normal String type.
3. The encoded body of the query, which should be an encoded fog-pack value.

It is presumed that, in general, queries will be very small and infrequently 
transmitted compared to Documents & Entries.

*/
