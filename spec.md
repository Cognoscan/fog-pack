# Condense-db Specification #

Introduction
------------

Condense-db is a system for locating, exchanging, and verifying portable data 
objects. The core of it is a flat document database, where each document is 
immutable. Documents may be added and removed from the database at any time, 
and "entries" may be appended to a document. By appending entries, documents may 
change over time in the database while still being locatable by their hash.

An application interacts with the database by storing and removing documents and 
entries. The database may also be queried using a special "query" document, 
whose format is described in the [query specification](query.md).

Documents and their attached entries may follow a specific "schema" document. A 
schema document defines the allowed format for a document, any attached entries, 
and which fields in the document may be queried. This allows the database to 
index a document & its entries to speed up querying. See the 
[schema specification](schema.md) for details on the format of schema documents 
and their usage.

Documents are formatted using a modified version of MessagePack. In addition to 
the base MessagePack specification, a unambiguous encoding is defined, and 
extension types are added for cryptographic primitives. To use these 
cryptographic extensions, Condense-db also includes a keystore of public, 
private, and secret keys for encryption and verification purposes.

In addition to the keystore, a certificate store is maintained within the 
document database. For every known public key, there exists a coresponding 
document (called a "Certificate List") in the certificate store. Entries 
appended to this document are signed by other keys, allowing for the issuing of 
certificates. See the [certificate specification](certs.md) for details. The 
certificate documents are a special component used in queries, allowing for more 
complex signature schemes.

Finally, the database may be accessed by other applications or systems.
Queries, likewise, may be spread to other systems that maintain their own 
Condense-db databases. Permissions may be set on the database and each query to 
determine under what circumstances they are presented to other systems. See 
the [permissions specification](permissions.md) for details.

Throughout these specifications, MessagePack values are regularly referred to. 
Rather than provide the serialized data, a human-readable format is used. See 
[Human-Readable MessagePack](readable_msgpack.md) for details.

Encoding Definitions
--------------------

Condense-db defines the following objects:

- Document: A field-value object. Defined in [Document Format](#document-format).
- Entry: A field-value pair with a parent document. Defined in
	[Document Format](#document-format)
- Query: A query for retrieving documents and entries from a database. Defined 
	in `query.md`.
- Schema: A definition of what format is allowed for documents, entries, and 
	queries run against them. Defined in `schema.md`.
- Certificate: A public key signed with another public key, with associated 
	metadata. Defined in `certs.md`.

These structures must have consistant encodings, as this is a necessary 
prerequisite for consistant hashing, encryption, and cryptographic signing. See 
[MessagePack Encoding](#messagepack-encoding) for the consistant encoding used 
by all of the structures.


MessagePack Encoding
--------------------

Condense-db encodes everything using [MessagePack](https://msgpack.org). Because 
Condense-db makes use of cryptographic hashes, an unambiguous encoding for 
MessagePack must be defined. In addition, various cryptographic primitives are 
defined as MessagePack `ext` types.

In addition, MessagePack normally supports `map` types with arbitrary keys and 
leaves the decision of what to do with invalid UTF-8 strings up to the 
application. Condense-db only allows `map` types whose keys (fields) are strings 
and are unique within the `map`. This type is referred to as an "object" 
throughout the documentation. As for UTF-8 strings, if they are not valid UTF-8, 
decoding should fail and the MessagePack value should be marked as invalid.

A human-readable version of MessagePack is used throughout these specifications. 
It cannot be encoded back into MessagePack, but is useful for discussing 
MessagePack structures. See [Human-Readable MessagePack](readable_msgpack.md) 
for details.

### Extension Type ###

MessagePack is extended with the following `ext` types:

| Type | Name      | Description                                                  |
| --   | --        | --                                                           |
| 2    | Hash      | Cryptographic hash of some byte sequence                     |
| 3    | Identity  | A public key, usable for encryption and signing verification |
| 4    | Signature | A public key, and signature of some byte sequence            |
| 5    | Lockbox   | Encrypted data and the ID of the key needed to decrypt it    |

All positive numbered types are reserved for future use. If a specific additional 
primitive is desired, this specification or the MessagePack specification should 
be expanded.

### Type Naming ###

The MessagePack type families, along with the defined extensions, are given 
additional short names. They may also be queried in different ways. See the
[query specification](query.md) for the meaning of each query type. 

| Type      | Short Name | Possible Query Types |
| --        | --         | --                   |
| Nil       | Nil        |                      |
| Boolean   | Bool       | ord                  |
| Integer   | Int        | ord, bit             |
| String    | Str        | ord, regex           |
| F32       | F32        | ord                  |
| F64       | F64        | ord                  |
| Binary    | Bin        | ord, bit             |
| Array     | Array      | array                |
| Object    | Obj        |                      |
| Hash      | Hash       | link, ref            |
| Identity  | Ident      |                      |
| Signature | Sign       |                      |
| Lockbox   | Lock       |                      |
| Timestamp | Time       | ord                  |

Note: although the Signature type has a short name, it is never used in actual 
queries or schema, as signatures are always indirectly interacted with.

### Unambiguous Encoding ###

Arbitrary MessagePack is not allowed. A specific set of encoding rules must be 
used to ensure MessagePack always unambiguously encodes values to a specific 
byte sequence:

1. The shortest available encoding is always used. This covers the `int`, `str`, 
`bin`, `array`, `map`, and `ext` encodings.
2. IEEE single-precision and double-precision floating points are not 
	interchangeable. Each is considered a unique type.
3. The `map` format family has a specific order for its field-value pairs:
	1. For each pair, encode both and concatenate them together as a field-value 
		set.
	2. Sort the pairs by byte order, appending the pair with the smallest value 
	first.

### Hash ###

After being encoded, any MessagePack may be put through a hash function to yield 
a Hash MessagePack value. The result is encoded as an `ext` type with a type of 
2, and where the first byte indicates the hashing method used, and the remaining 
bytes are equal to the digest.

Only one hash method is currently supported: BLAKE2B with a 64-byte message 
digest. This hash method has the first byte set to 1.

As an example, hashing a hex byte sequence of `00010203040506070809` should 
create the MessagePack byte sequence of

```
C7410201
29102511D749DB3CC9B4E335FA1F5E8FACA8421D558F6A3F3321D50D044A248B
A595CFC3EFD3D2ADC97334DA732413F5CBF4751C362BA1D53862AC1E8DABEEE8
```

Where `C7` indicates the `ext 8` type, `41` is the size (65 bytes), `02` 
indicates this is a hash, and `01` indicates it is a BLAKE2B hash with a 64-byte 
digest.

If the first byte is set to 0, the hash has a special meaning and refers to the 
current document it is within. There shall be no other bytes present in the hash 
value, meaning it should be encoded as `D40200`.

### Identity ###

An Identity encodes a public key that may be used for encrypting data or 
verifying a cryptographic signature. If separate keys are used for encryption 
and verification, they are both provided as a single unit here.

The first byte of an Identity is the "type byte", which indicates what type of 
public key information is provided. The remaining bytes are the key information.

Currently, only one type is supported: an Ed25519 public key, which can be 
transformed into a Curve25519 key for encryption. For this type, the type 
byte is set to 1, and the Ed25519 public key is then attached.

Each public key type has a matching private key type. See the description of 
Lockbox for how private keys may be encoded.

### Signature ###

A signature encodes a digital signature: a structure verifying a particular 
encoded MessagePack sequence has been seen & signed by a particular Identity, 
and has not been modified since.

Signatures are special in that they are always associated with another 
MessagePack object: the object being signed. As such, a Signature *must* be part 
of an array. The first element of this array is the MessagePack object that was 
signed, and all other elements must be Signatures signing that object. This 
signed array is not directly accessed as an array in queries and schema 
validation; instead, the array is treated as though it were the first element in 
the array, but with the option to verify signatures on it.

An encoded signature contains 4 elements in the following order: 

1. The "type byte" of the Identity that was used in the signature.
2. A byte indicating what hashing method was used to create the digest that was 
   signed.
3. The Identity used in signing, except for the "type byte".
4. The digital signature. Format is determined by the Identity type.

Note: the signature signs only the first element of the array - this does not 
include the MessagePack encoding byte

### Lockbox ###

A lockbox stores encrypted information. It can store one of 3 things: A private 
key, a secret key, or a MessagePack object. It may be encrypted using either a 
secret key or with an ephemeral secret key, generated from an Identity (public 
key) and a randomly-generated public-private keypair.

A lockbox contains the following information:
1. The "type byte" of the Identity / secret key used for encryption.
2. The "type byte" specifying if an Identity or a secret key was used for 
	encryption. 1 indicates an Identity was used, 2 indicates a secret key was 
	used.
3. The keys used. If an Identity was used, the Identity's public signing key is 
	attached followed by an ephemeral public encryption key. If a secret key was 
	used, a cryptographic hash of the key is attached.
4. A randomly-generated nonce used for the encryption payload.
5. The encrypted data itself.

The encrypted payload consists of a single byte indicating what type of data is 
encrypted, followed by the data itself.

### Timestamps ###
The timestamp type (-1) is supported, with caveats. Timestamps specifically use 
UTC time, as seconds elapsed since 1970-01-01T00:00:00Z. How leap seconds are 
handled is not defined, but it is hoped that one two methods will be used:

- Leap seconds will be "smeared" out by skewing the clock over a 24-hour period, 
	centered around the leap second time. No special handling is required.
- Leap seconds will show up as the clock being halted for one second, while the 
	nanoseconds count will roll past 1000000000 and continue up to 1999999999.

Due to the 2nd method, nanoseconds may be higher than 999999999. This is 
accepted, and when the nanosecond value is greater than 1073741824 (2^30-1), the 
timestamp 96 format shall be used.

**The most compact timestamp format shall always be used.**

#### TAI Time ####
A user of the timestamp type is advised to watch out for systems that use TAI 
instead of UTC; TAI systems do not use leap seconds and are thus desynchronized 
from UTC systems.

If TAI (or another timescale) becomes a requirement for future implementations, 
it is recommended that a new `ext` type be reserved.


Document Format
---------------

Documents are immutable objects that may be signed. A document is referred to by 
its cryptographic hash, which is a hash of the object and any attached 
signatures. It is expected that a document will be sent as an array containing 
the object and the associated signatures. As an example, here is a simple 
document that has been signed by two separate Identities:

```json
[
  {
    name: "Example document",
    data: "Test data"
  },
  <Signature(ID Signer 0)>,
  <Signature(ID Signer 1)>
]
```

Documents may have "entries" attached to them, which consist of a hash of
the parent document, a single field, and a value for that field. These are 
stored in sequence in an array, which may be signed. The signature is for the 
complete array of hash, field, and value.

```json
[
  [
    <Hash(parent document)>,
    "field",
    "test entry data"
  ],
  <Signature(ID Signer 0)>
]
```

### Transmitting Entries

When entries are transmitted as a group, the parent hash may be omitted, but 
must be recoverable within the transmission in order to determine what the 
parent document is and whether the signatures are valid. In general, the most 
efficient alternate encoding will be a heterogeneous array whose first element 
is the parent hash and whose subsequent elements are arrays containing the 
field, value, and signatures in sequence. This creates a coding overhead of 1 
byte per entry (the array marker), with a group overhead of 69 bytes if fewer 
than 15 entries are sent. The group overhead increases by 2 bytes for fewer than 
(2^16)-2 entries, and 4 bytes for all others (max 2^32-2).

An example set of entries encoded for transmission is below.

```json
[
  <Hash(parent document)>,
  [
    "field",
    "test data 1",
    <Signature(ID Signer 1)>
  ],
  [
    "field",
    "test data 2",
    <Signature(ID Signer 2)>
  ],
  [
    "field",
    "test data 3",
    <Signature(ID Signer 3)>
  ]
]
```

If all entries use the exact same field, the field may also be omitted from each 
entry array and instead placed right after the parent document hash. The same 
example as before is reproduced here:

```json
[
  <Hash(parent document)>,
  "field",
  [
    "test data 1",
    <Signature(ID Signer 1)>
  ],
  [
    "test data 2",
    <Signature(ID Signer 2)>
  ],
  [
    "test data 3",
    <Signature(ID Signer 3)>
  ]
]
```














