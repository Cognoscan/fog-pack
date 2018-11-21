# Condense-db Specification #

The concepts behind Condense-db can be broken down into two categories: encoding 
definitions and implementation recommendations. The goal of Condense-db is 
ultimately to define a mechanism for data exchange, verification, retrieval, and 
modification. The true goal is that this mechanism should fit easily into any 
protocol stack, such that a user need not concern themselves with exact network 
implementations.

## Encoding Definitions ##

Condense-db defines the following structures:

- Document: A key-value map
- Entry: A key-value pair with a parent document
- Query: A query for retrieving documents and entries from a database
- Schema: A definition of what format is allowed for documents, entries, and 
  queries run against them.
- Certificate: A public key signed with another public key, with associated 
  metadata.

These structures must have consistant encodings, as this is a necessary 
prerequisite for consistant hashing, encryption, and cryptographic signing. As 
such, the following encodings need to be defined:

- MessagePack with cryptographic extensions
	- Used for all structures
	- Extended with types for cryptographic keys, encryption, hashes, and 
	  signatures.
	- Extended with a required order for maps
	- Reduced to require that all values use the most compact available encoding 
	  for its data - ex. a 32-bit unsigned integer will be encoded as an 8-bit 
		unsigned if its value is 255 or less.
- Document encoding
	- Simple Key-Value map where all keys are strings, with the option to sign the 
		entire document.
- Entry encoding
	- Key (string), Value, hash of parent document, with the option to sign all 
	- three.
- Certificate encoding
	- Defines a special document with only two keys: "type" and "key". "type" is 
	  always set to "cert" and "key" is always set to a public key.
	- Entries appended to this special document always have the key "cert" with a 
		string-keyed map as the value. The map contains "start", "end", "name", and 
		optional "meta" keys.
- Query language
	- Complex language that is used for document and entry retrieval.
	- Can specify specific documents
	- Can specify entries whose keys match, and whose values meet certain 
	  criteria.
	- Can specify entries whose values are hashes, and select only those whose 
	  linked documents meet certain criteria.
- Schema language
	- Complex language used for defining document format and format of attached 
	  entries.
	- Also defines what in the document may be queried and how it may be queried.
		
## Implementation Recommendations ##

- Use a local database for storing documents. Just needs to be a key-value 
  store, where the keys are the document hashes.
- Attach entries directly to each document in a list
- Maintain indexing data for documents as required by their schema in this 
  database
- Use permissions.



## MessagePack Encoding ##


### Extension Type ###

MessagePack is extended with the following `ext` types:

| Type | Name      | Description                                                  |
| --   | --        | --                                                           |
| 2    | Hash      | Cryptographic hash of some byte sequence                     |
| 3    | Identity  | A public key, usable for encryption and signing verification |
| 4    | Signature | A public key, and signature of some byte sequence            |
| 5    | Lockbox   | Encrypted data and the ID of the key needed to decrypt it    |

All positive types are reserved for future use. If a specific additional 
primitive is desired, this specification or the MessagePack specification should 
be expanded.

### Unambiguous Encoding ###

Arbitrary MessagePack is not allowed. A specific set of encoding rules must be 
used to ensure MessagePack always unambiguously encodes values to a specific 
byte sequence:

1. The shortest available encoding is always used. This covers the `int`, `str`, 
`bin`, `array`, `map`, and `ext` encodings.
2. Any integer is encoded using the shortest possible sequence in the `int` 
	family of encodings. If the integer cannot fit in a 64-bit signed or 
	unsigned value, it shall be considered a byte array, not an integer.
3. IEEE single-precision and double-precision floating points are not 
	interchangeable. Each is considered a unique type.
4. The `map` format family has a specific order for its key-value pairs:
	1. For each pair, encode both and concatenate them together as a key-value 
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

### Identity ###

An Identity encodes a public key that may be used for encrypting data or 
verifying a cryptographic signature. If separate keys are used for encryption 
and verification, they are both provided as a single unit here.

The first byte of an Identity is the "type byte", which indicates what type of 
public key information is provided.

Currently, only one type is supported: an Ed25519 public key, which can be 
transformed into a Curve25519 key for encryption. For this type, the type 
byte is set to 1.

Each public key type has a matching private key type. See the description of 
Lockbox for how private keys may be encoded.

### Signature ###

A signature encodes a digital signature: a structure verifying a particular 
encoded MessagePack sequence has been seen & signed by a particular Identity, 
and has not been modified since.

Signatures are special in that they are always associated with another 
MessagePack object: the object being signed. As such, a Signature *must* be part 
of an array. The first element of this array is the MessagePack object that was 
signed, and all other elements must be Signatures signing that object.

An encoded signature contains 4 elements: 

### Lockbox ###

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

















