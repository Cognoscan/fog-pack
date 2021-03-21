/*!

The raw data format used by fog-pack.

fog-pack is a binary data format that builds off of the MessagePack 
specification, and limits it in important ways to create a canonical encoding 
for all data. It also extends MessagePack's types to support cryptographic 
hashes, Public keys, and encrypted data (including private keys, symmetric keys, 
and arbitrary data).

# Differences from MessagePack

MessagePack provides an excellent self-describing, yet compact format. The 
fog-pack format primarily places limitations on MessagePack to create a 
canonical encoding for all data, and further limits it to make validation 
easier. The main differences are:

- Data must be encoded using the shortest available encoding.
- Positive integers always use positive integer encodings.
- 32-bit and 64-bit floating point types are considered completely separate types.
- The string type only permits valid UTF-8 byte sequences.
- Objects replace the Map type. They only allow strings as keys, the field-value 
    pairs must be in lexicographical order, and the keys must be unique.
- The extension types are all reserved and may not be extended by an application.
- Hash, Identity, and Lockbox extension types are defined for cryptographic purposes.
- Timestamps are explicitly UTC and let leapseconds be handled with a nanosecond 
    interval of 0 to 1,999,999,999.
- Decoding fails if any of the above limitations aren't met.

Lexicographical order, here, means ordered by the string's encoded UTF-8 byte sequence, with 
absence of a byte being the absolute minimum, followed by 0-255.

## Why This Differs from MessagePack

MessagePack is already a great binary encoding format, but it's easy to end up encoding data 
differently each time - for example, 0 can be encoded 9 different ways! This is useful for 
extremely optimized encoders, where one might want to serialize a 32-bit signed value into a 1 byte 
tag plus the value itself, but it causes problems for when we want identical data to have identical 
hashes. This is where a "canonical" encoding comes in: identical data will always have a single 
defined encoding. Any format that refers to data by its hash should want this, as it prevents 
duplication and prevents identical data from hashing differently depending on how the encoder was 
written.

As a baseline, MessagePack can create a "canonical" encoding by simply choosing the shortest 
available encoding for each of its base types.

However, fog-pack doesn't stop there. It also makes some assumptions about how the format will be 
used, and limits MessagePack accordingly.

### Strings are UTF-8

First, MessagePack allows invalid UTF-8 in order to allow broken data to be easily serialized, and 
leaves it up to the data consumer to make decisions about invalid UTF-8. fog-pack takes a different 
approach, expecting that the consumer should always want valid UTF-8 strings, and requires the data 
producer to decide how to handle invalid UTF-8 input - usually, by failing or by coercing it into 
valid UTF-8.

### Map only allows unique strings as keys, ordered lexicographically

Second, MessagePack allows maps to contain arbitrary data as their keys, and allows them to be in 
any order. This makes serialization trivial for many objects, and allows one to easily encode a 
hash map with arbitrary keys. fog-pack limits the map type to only allow unique keys, requires 
they be strings, and requires they be lexicographically ordered by key. The expectation is that 
maps will be deserialized into structures with named fields, or into a key-value struct that 
expects unique keys (which is almost all of them). Requiring an order makes deserialization easier, 
as the exact order is known ahead of time, and ensures identical structures can't be encoded in 
multiple ways. Further limiting the keys to strings makes schema creation easier, makes data order 
more intuitive (no need to order by encoded byte sequence), and maps well to structures with named 
fields.

### Limited extension types

Finally, MessagePack allows for arbitrary extension types for end-users to mark byte sequences with 
special handling requirements. fog-pack uses this to define 3 additional types for working with 
cryptographic primitives, and bans the remainder. Since fog-pack is meant to be a public 
interchange format, allowing for custom obscured types would create conflicting interpretations of 
identical data. As such, non-recognized extensions are prohibited.

# Types

fog-pack takes the existing MessagePack type families and defines required 
encoding forms, as well as defining several additional extended types. The full 
type list is:

| Type          | Short Name | Description                                            |
| --            | --         | --                                                     |
| [`Null`]      | Null       | Null/nil value                                         |
| [`Boolean`]   | Bool       | True or false                                          |
| [`Integer`]   | Int        | Any integer from `-(2^63)` up to `(2^64)-1`            |
| [`String`]    | Str        | Any valid UTF-8 string up to `(2^32)-1` bytes          |
| [`F32`]       | F32        | An IEEE 754 single precision value                     |
| [`F64`]       | F64        | An IEEE 754 double precision value                     |
| [`Binary`]    | Bin        | Any sequence of bytes up to `(2^32)-1` long            |
| [`Array`]     | Array      | A sequence of elements                                 |
| [`Object`]    | Obj        | Field-value pairs of elements, where Field is a String |
| [`Hash`]      | Hash       | A cryptographic hash                                   |
| [`Identity`]  | Ident      | A public key                                           |
| [`Lockbox`]   | Lock       | An encrypted sequence of bytes                         |
| [`Timestamp`] | Time       | A UTC timestamp with nanosecond precision              |

[`Null`]: #null
[`Boolean`]: #boolean
[`Integer`]: #integer
[`String`]: #string
[`F32`]: #f32
[`F64`]: #f64
[`Binary`]: #binary
[`Array`]: #array
[`Object`]: #object
[`Hash`]: #hash
[`Identity`]: #identity
[`Lockbox`]: #lockbox
[`Timestamp`]: #timestamp

User defined types are explicitly prohibited, as are Hash/Identity/Lockbox 
elements whose cryptographic scheme isn't documented here.

The Hash, Identity, Lockbox, and Timestamp utilize MessagePack's ext format 
family, which is reproduced here for convenience.

## Ext Family

The Ext Family of formats isn't directly used as a type of its own, but is used 
to encapsulate the Hash, Identity, Lockbox, and Timestamp types. The length of 
the encoded element is determined and then encapsulated as a byte sequence by 
the appropriate ext format. The reserved Ext "Types" are as follows:

| Type Number | Type      |
| --          | --        |
| -1          | Timestamp |
| 1           | Hash      |
| 2           | Identity  |
| 3           | Lockbox   |

Unknown types should cause a decoding error.

```text
fixext1 stores a single byte
+----------+----------+----------+
|   0xd4   |   type   |   data   |
+----------+----------+----------+

fixext2 stores 2 bytes
+----------+----------+==========+
|   0xd5   |   type   |   data   |
+----------+----------+==========+

fixext4 stores 4 bytes
+----------+----------+==========+
|   0xd5   |   type   |   data   |
+----------+----------+==========+

fixext8 stores 8 bytes
+----------+----------+==========+
|   0xd7   |   type   |   data   |
+----------+----------+==========+

fixext16 stores 16 bytes
+----------+----------+==========+
|   0xd8   |   type   |   data   |
+----------+----------+==========+

ext8 stores the length and up to (2^8)-1 bytes,
unless the length is 1, 2, 4, 8, or 16
+----------+----------+----------+==========+
|   0xc7   | XXXXXXXX |   type   |   data   |
+----------+----------+----------+==========+

ext16 stores the length and between 2^8 and (2^16)-1 bytes
+----------+----------+----------+----------+==========+
|   0xc8   | YYYYYYYY | YYYYYYYY |   type   |   data   |
+----------+----------+----------+----------+==========+

ext32 stores the length and between 2^16 and (2^32)-1 bytes
+----------+----------+----------+----------+----------+----------+==========+
|   0xc9   | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ |   type   |   data   |
+----------+----------+----------+----------+----------+----------+==========+

where:
- N is the length of the encoded type byte sequence
- XXXXXXXX is a 8-bit unsigned integer representing N
- YYYYYYYY_YYYYYYYY is a 16-bit big-endian unsigned integer representing N
- ZZZZZZZZ_ZZZZZZZZ_ZZZZZZZZ_ZZZZZZZZ is a 32-bit big-endian unsigned integer 
    representing N
```

## Null

Stores Null in 1 byte.

```text
null:
+----------+
|   0xc0   |
+----------+
```

## Boolean

Booleans store true or false in 1 byte.

```text
false:
+----------+
|   0xc2   |
+----------+

true:
+----------+
|   0xc3   |
+----------+
```

## Integer

Integers are stored in 1, 2, 3, 5, or 9 bytes. If the integer is non-negative, 
it is stored using the minimum-length of the following formats:

```text
positive fixnum stores a 7-bit positive integer,
(0XXXXXXX is a 8-bit unsigned integer)
+----------+
| 0XXXXXXX |
+----------+

uint8 stores a 8-bit unsigned integer >= 128
+----------+----------+
|   0xcc   | ZZZZZZZZ |
+----------+----------+

uint16 stores a 16-bit big-endian unsigned integer >= 256
+----------+----------+----------+
|   0xcd   | ZZZZZZZZ | ZZZZZZZZ |
+----------+----------+----------+

uint32 stores a 32-bit big-endian unsigned integer >= 65536
+----------+----------+----------+----------+----------+
|   0xce   | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ |
+----------+----------+----------+----------+----------+

uint64 stores a 64-bit big-endian unsigned integer >= 2^32
+----------+----------+----------+----------+----------+----------+----------+----------+----------+
|   0xcf   | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ |
+----------+----------+----------+----------+----------+----------+----------+----------+----------+
```

If the integer is negative, it is stored as a two's complement number using the 
minimum length of the following formats:

```text
negative fixnum stores a 5-bit negative integer,
(111YYYYY is a 8-bit signed integer)
+----------+
| 111YYYYY |
+----------+

int8 stores a 8-bit signed integer < -32
+----------+----------+
|   0xd0   | ZZZZZZZZ |
+----------+----------+

int16 stores a 16-bit big-endian signed integer < -128
+----------+----------+----------+
|   0xd1   | ZZZZZZZZ | ZZZZZZZZ |
+----------+----------+----------+

int32 stores a 32-bit big-endian signed integer < -32768
+----------+----------+----------+----------+----------+
|   0xd2   | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ |
+----------+----------+----------+----------+----------+

int64 stores a 64-bit big-endian signed integer < 2^-31
+----------+----------+----------+----------+----------+----------+----------+----------+----------+
|   0xd3   | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ |
+----------+----------+----------+----------+----------+----------+----------+----------+----------+
```

## F32

An IEEE 754 single precision number encoded using 5 bytes. When ordered, the 
IEEE 754 total order predicate is used.

```text
float32 stores a IEEE 754 single precision number,
written in big-endian byte order:
+----------+----------+----------+----------+----------+
|   0xca   | XXXXXXXX | XXXXXXXX | XXXXXXXX | XXXXXXXX |
+----------+----------+----------+----------+----------+
```

### F64

An IEEE 754 double precision number encoded using 9 bytes. When ordered, the 
IEEE 754 total order predicate is used.

```text
float64 stores a IEEE 754 double precision number,
written in big-endian byte order:
+----------+----------+----------+----------+----------+----------+----------+----------+----------+
|   0xcb   | XXXXXXXX | XXXXXXXX | XXXXXXXX | XXXXXXXX | XXXXXXXX | XXXXXXXX | XXXXXXXX | XXXXXXXX |
+----------+----------+----------+----------+----------+----------+----------+----------+----------+
```

## String

String stores a valid UTF-8 byte sequence with 1, 2, 3, or 5 bytes of overhead 
beyond the sequence itself. Invalid UTF-8 is considered an encoding error and 
should be treated as such by the encoder/decoder.

```text
fixstr stores a sequence up to 31 bytes in length
+----------+==========+
| 101XXXXX |   data   |
+----------+==========+

str8 stores a sequence between 32 and (2^8)-1 bytes in length
+----------+----------+==========+
|   0xd9   | YYYYYYYY |   data   |
+----------+----------+==========+

str16 stores a sequence between 2^8 and (2^16)-1 bytes in length
+----------+----------+----------+==========+
|   0xda   | ZZZZZZZZ | ZZZZZZZZ |   data   |
+----------+----------+----------+==========+

str32 stores a sequence between 2^16 and (2^32)-1 bytes in length
+----------+----------+----------+----------+----------+==========+
|   0xdb   | AAAAAAAA | AAAAAAAA | AAAAAAAA | AAAAAAAA |   data   |
+----------+----------+----------+----------+----------+==========+

where:
- N is the length of the byte sequence
- XXXXX is a 5-bit unsigned integer representing N
- YYYYYYYY is a 8-bit unsigned integer representing N
- ZZZZZZZZ_ZZZZZZZZ is a 16-bit big-endian unsigned integer representing N
- AAAAAAAA_AAAAAAAA_AAAAAAAA_AAAAAAAA is a 32-bit big-endian unsigned integer 
    representing N
```

## Binary

Binary stores any byte sequence with 2, 3, or 5 bytes of overhead beyond the 
sequence itself.

```text
bin8 stores a sequence up to (2^8)-1 bytes in length
+----------+----------+==========+
|   0xc4   | XXXXXXXX |   data   |
+----------+----------+==========+

bin16 stores a sequence between 2^8 and (2^16)-1 bytes in length
+----------+----------+----------+==========+
|   0xc5   | YYYYYYYY | YYYYYYYY |   data   |
+----------+----------+----------+==========+

bin32 stores a sequence between 2^16 and (2^32)-1 bytes in length
+----------+----------+----------+----------+----------+==========+
|   0xc6   | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ |   data   |
+----------+----------+----------+----------+----------+==========+

where:
- N is the length of the byte sequence
- XXXXXXXX is a 8-bit unsigned integer representing N
- YYYYYYYY_YYYYYYYY is a 16-bit big-endian unsigned integer representing N
- ZZZZZZZZ_ZZZZZZZZ_ZZZZZZZZ_ZZZZZZZZ is a 32-bit big-endian unsigned integer 
    representing N

```

## Array

Array stores a sequence of FogPack elements with 1, 3, or 5 bytes of overhead.

```text
fixarray stores a sequence of up to 15 elements
+----------+~~~~~~~~~~~~+
| 1001XXXX | N Elements |
+----------+~~~~~~~~~~~~+

array16 stores a sequence between 16 and (2^16)-1 elements
+----------+----------+----------+~~~~~~~~~~~~+
|   0xdc   | YYYYYYYY | YYYYYYYY | N Elements |
+----------+----------+----------+~~~~~~~~~~~~+

array32 stores a sequence between 2^16 and (2^32)-1 elements
+----------+----------+----------+----------+----------+~~~~~~~~~~~~+
|   0xdd   | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | N Elements |
+----------+----------+----------+----------+----------+~~~~~~~~~~~~+

where:
- N is the number of elements
- XXXX is a 4-bit unsigned integer representing N
- YYYYYYYY_YYYYYYYY is a 16-bit big-endian unsigned integer representing N
- ZZZZZZZZ_ZZZZZZZZ_ZZZZZZZZ_ZZZZZZZZ is a 32-bit big-endian unsigned integer 
  representing N
```

## Object

Object stores a sequence of field-value pairs with 1, 3, or 5 bytes of overhead. 
The keys *must* be of the String type and arranged in lexicographical order, 
while the values can be elements of any type. Field-value pairs are encoded in 
order, such that a key string will be the first element, followed by its value, 
and so on.

This type replaces MessagePack's `map` format family as a strict subset of what 
MessagePack could encode.

```text
fixobj stores a sequence of up to 15 field-value pairs
+----------+~~~~~~~~~~~~~~+
| 1000XXXX | N*2 elements |
+----------+~~~~~~~~~~~~~~+

obj16 stores a sequence between 16 and (2^16)-1 field-value pairs
+----------+----------+----------+~~~~~~~~~~~~~~+
|   0xde   | YYYYYYYY | YYYYYYYY | N*2 Elements |
+----------+----------+----------+~~~~~~~~~~~~~~+

obj32 stores a sequence between 2^16 and (2^32)-1 field-value pairs
+----------+----------+----------+----------+----------+~~~~~~~~~~~~~~+
|   0xdf   | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | N*2 Elements |
+----------+----------+----------+----------+----------+~~~~~~~~~~~~~~+

where:
- N is the number of field-value pairs
- XXXX is a 4-bit unsigned integer representing N
- YYYYYYYY_YYYYYYYY is a 16-bit big-endian unsigned integer representing N
- ZZZZZZZZ_ZZZZZZZZ_ZZZZZZZZ_ZZZZZZZZ is a 32-bit big-endian unsigned integer 
  representing N
```

## Hash

Hash stores a single cryptographic hash. Only one hashing algorithm is 
supported, with the expectation that new algorithms will only be added when the 
current one is being deprecated due to security concerns. The algorithm used is 
indicated by a version byte, which should be one of the following:

| Version | Meaning                     |
| --      | --                          |
| 0       | No hash                     |
| 1       | BLAKE2b with 32-byte digest |

The current recommended hash algorithm is BLAKE2b with a 32-byte digest. The 
Version 0 hash is available primarily for schema to reference themselves, taking 
the place of their hash.

The encoded element is wrapped within the appropriate [ext format](#ext-family), 
as shown:

```text
fixext1 is used for Hash Version 0:
+----------+----------+----------+
|   0xd4   |   0x01   |   0x00   |
+----------+----------+----------+

ext8 is used for Hash Version 1:
+----------+----------+----------+----------+==========+
|   0xc7   | XXXXXXXX |   type   | version  |   hash   |
+----------+----------+----------+----------+==========+
+----------+----------+----------+----------+==========+
|   0xc7   | XXXXXXXX |   0x01   |   0x01   |   hash   |
+----------+----------+----------+----------+==========+

where:
- XXXXXXXX is an unsigned integer indicating the hash length plus 1. For version 
  1, this is fixed at 33.
```

## Identity

Identity stores a single cryptographic public key, used for both signing and 
encryption. Only one public key cryptography method is supported, with the 
expectation that new algorithms will only be added when the current one is 
deprecated due to security concerns. The method used is indicated by a version 
byte, which should be one of the following:

| Version | Meaning            |
| --      | --                 |
| 0       | Reserved           |
| 1       | Ed25519/Curve25519 |

The Ed25519 public key can be converted to a X25519 public key (the curves are 
birationally equivalent). Any future version of the Identity element will be 
intended for use as both a signing key and encryption key, even if that requires 
publishing two separate keys within one element.

```text
ext8 is used for Identity Version 1:
+----------+----------+----------+----------+==============+
|   0xc7   | XXXXXXXX |   type   | version  |   identity   |
+----------+----------+----------+----------+==============+
+----------+----------+----------+----------+========================+
|   0xc7   | XXXXXXXX |   0x02   |   0x01   |   Ed25519 public key   |
+----------+----------+----------+----------+========================+

where:
- XXXXXXXX is an unsigned integer indicating the hash length plus 1. For version 
  1, this is fixed at 33.
```

## Lockbox

Lockbox stores authenticated, encrypted arbitrary data, prepended with an 
identifier indicating the public key or symmetric key used to encrypt it. The 
public key is part of an Identity - if the private portion of an Identity is 
known, the Lockbox may be decrypted. It can likewise be decrypted if the 
symmetric key is known.

The primary uses of Lockbox are to securely store sensitive data for an extended 
period, to pass the secret key of an Identity, and to pass a symmetric key used 
in other Lockboxes.

Lockbox can be described as three nested formats: The ext format wrapper, the 
Lockbox structure, and the internal encrypted data. 

#### Lockbox Format Wrapper

```text
ext8 is used when Lockbox is up to 255 bytes:
+----------+----------+----------+==============+
|   0xc7   | XXXXXXXX |   0x03   |   Lockbox    |
+----------+----------+----------+==============+

ext16 is used when Lockbox is between 256 and (2^16)-1 bytes:
+----------+----------+----------+----------+==============+
|   0xc8   | YYYYYYYY | YYYYYYYY |   0x03   |   Lockbox    |
+----------+----------+----------+----------+==============+

ext32 is used when Lockbox is between (2^16) and (2^32)-1 bytes:
+----------+----------+----------+----------+----------+----------+==============+
|   0xc9   | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ | ZZZZZZZZ |   0x03   |   Lockbox    |
+----------+----------+----------+----------+----------+----------+==============+

- N is the number of bytes in the Lockbox structure
- XXXXXXXX is a 8-bit unsigned integer representing N
- YYYYYYYY_YYYYYYYY is a 16-bit big-endian unsigned integer representing N
- ZZZZZZZZ_ZZZZZZZZ_ZZZZZZZZ_ZZZZZZZZ is a 32-bit big-endian unsigned integer 
  representing N
```

### Lockbox Structure
The internal lockbox format differs depending on whether a symmetric key was 
used for encryption or an Identity was used for encryption. Both use XChaCha20 
for encryption with an AEAD construction with no additional data. If an Identity 
was used for encryption, the symmetric key is derived by:

1. Calculate Curve25519 public encryption key from Ed25519 public key.
2. Calculate the shared secret between the Curve25519 key and an ephemeral 
    Curve25519 key pair. If decrypting, the ephemeral public key can be combined 
    with the key was used for encryption. If encrypting, the ephemeral private key 
    can be combined with the public encryption key.

For a public key / Identity, the format consists of a version byte, a byte set 
to 1, the public Identity signing key, a public ephemeral key randomly generated 
for the Lockbox, a nonce, the ciphertext, and a Poly1305 message authentication 
tag.

For a symmetric key, the format consists of a version byte, a byte set to 2, an 
identifier derived from the key, a nonce, the ciphertext, and a Poly1305 message 
authentication tag. The identifier is generated using libsodium's key derivation 
function using the secret key as an input, "fogpack" as the context, and a 
subkey ID of 1.

Currently the version byte must be set to 1.

```text
+----------+----------+==========+==========+==========+==============+=====+
| Version  |   0x01   | SignKey  |  EphKey  |  Nonce   |  Ciphertext  | Tag |
+----------+----------+==========+==========+==========+==============+=====+

+----------+----------+==========+==========+==============+=====+
| Version  |   0x02   | StreamId |  Nonce   |  Ciphertext  | Tag |
+----------+----------+==========+==========+==============+=====+

- SignKey is a 32-byte Ed25519 public key
- EphKey is a 32-byte Curve25519 public key
- StreamId is a 32-byte hash of the encryption key (see above documentation)
- Nonce is a 24-byte random nonce
- Ciphertext is the internal data, encrypted with XChaCha20
- Tag is the authentication tag produced using the XChaCha20-Poly1305 AEAD 
    construction.

```

Internal data:

```text
+----------+=============+
|   0x01   | Private Key |
+----------+=============+
+----------+=============+
|   0x02   | Secret Key  |
+----------+=============+
+----------+=============+
|   0x03   |    Data     |
+----------+=============+
```

# Cryptography Considerations

Choices for cryptographic algorithms were based on the preferred algorithms used 
by libsodium. This resulted in the choices of:

- BLAKE2b for hashes
- Ed25519 for signing
- Curve25519 for public-key encryption
- XChaCha20 with Poly1305 message authentication codes

## Hashing

With 64-bit processors being increasingly common in consumer electronics, 
BLAKE2b seemed the preferrable choice within the BLAKE2 series of hashing 
algorithms.

A 32-byte hash was selected primarily because it provides 128-bit security, a 
commonly accepted sufficient level for all applications.

BLAKE3 was considered as well, but is too recent and not thoroughly vetted at 
the time of this standard's creation.

*/
