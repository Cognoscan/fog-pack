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

Encoded documents always start with a compression marker byte, and are followed 
by a 3-byte big-endian integer indicating the length of the data payload. The 
data payload follows, and the last portion of the document are any signatures 
appended to the document. Note that the total size of the document cannot be 
calculated from the data itself - it is assumed that an encapsulating protocol 
will include the length of the document.

```text
+----------+----------+----------+----------+==========+============+
| 0XXXXXYY | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ |   Data   | Signatures |
+----------+----------+----------+----------+==========+============+

- XXXXX is the 5-bit compression algorithm code
- YY is the compression type marker
- ZZZZZZZZ_ZZZZZZZZ_ZZZZZZZZ is a 24-bit big-endian unsigned integer 
    representing the length of the Data.

```

The data payload depends on the compression type indicated:

- Uncompressed: No compression used. The document's raw value is the data payload.
- CompressedNoSchema: The payload is a single zstd frame, containing the 
    compressed version of the document's raw value. The raw value should not 
    contain the empty string as a top-level field - this would be a coding 
    error.
- Compressed: The payload starts with the object marker (with size), then the 
    first field-value pair of the object, and finally a zstd frame containing 
    the remainder of the raw value.  The first field-value pair *must* be the 
    empty string and the hash of a schema.
- DictCompressed: Same as Compressed, except that the zstd frame was made using 
    a dictionary embedded in the schema used by the document.

For all of the above, the zstd frame always contains the estimated decompressed 
length.

# Entries

An encoded entry does not contain the parent hash or field string, which are 
assumed to be provided through some other means - usually, they are implied by 
a query that was used to fetch/generate them. As such, an encoded entry only 
contains the entry's fog-pack value & associated signatures.

Encoded entries always start with a compression marker byte, and are followed by 
a 2-byte big-endian integer indicating the length of the data payload. The data 
payload follows, and the last portion of the entry holds any signatures appended 
to the entry. Not e that the total size of the entry cannot be calculated from 
the data itself - it is assuemd that an encapsulating protocol will include the 
length of an entry.

```text
+----------+----------+----------+==========+============+
| 0XXXXXYY | ZZZZZZZZ | ZZZZZZZZ |   Data   | Signatures |
+----------+----------+----------+==========+============+

- XXXXX is the 5-bit compression algorithm code
- YY is the compression type marker
- ZZZZZZZZ_ZZZZZZZZ is a 16-bit big-endian unsigned integer representing the 
    length of the Data.

```

The data payload depends on the compression type indicated:

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
2. The field string being queried, encoded as a normal String value, including 
   the leading marker.
3. A compression marker byte, which must be set to Uncompressed. This is 
   reserved for possible future encoding approaches.
4. A 16-bit big-endian unsigned integer representing the length of an encoded 
   fog-pack value.
5. The encoded body of the query, which should be an encoded fog-pack value.
6. Any signatures appended to the query.

It is presumed that, in general, queries will be very small and infrequently 
transmitted compared to Documents & Entries.

*/
