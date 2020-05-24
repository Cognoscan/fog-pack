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
or allow for several of them (see [`Multi`](#multi)). See the [Validation 
Language](#validation-language) for more info.

At the top level, a schema is a validator for an [object](#obj) but without 
support for the `in`, `nin`, `comment`, `default`, or `query` optional fields. 
Instead, it supports a few additional optional fields for documentation, entry 
validation, and compression:

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


Validation Language
===================

This specifies the language used to validate fog-pack values. A validator is 
a fog-pack value that may be parsed and used for validation of fog-pack 
values. Ancillary information may also be included with a validator, for 
used by schema and queries, the primary use-cases for validators.

Validators can range from the very basic, up to complex nested sets of 
validators. The simplest is just a value to match against, ex. `true` is a 
validator that passes only if the value is a boolean set to true. Allowing 
any boolean would look like:

```json
{
    "type": "Bool"
}
```

A more complex validator, specifying an object with multiple fields, might 
look like:

```json
{
    "type": "Obj",
    "req": {
        "name": { "type": "Str" },
        "data": {
            "type": "Multi",
            "any_of": [
                { "type": "Bin" },
                { "type": "Hash" }
            ]
        }
    },
    "opt": {
        "tags": { "type": "Array", "items": "Str" }
    }
}
```

Data Types
----------

A minimum validator is just a non-object value, which acts as a simple 
match. An object can be used to specify a validator for any type, and allows 
for multiple types or identical types with differing conditions.

There is a validation format for each of the basic fog-pack types:`Null`, 
`Bool`, `Int`, `F32`, `F64`, `Bin`, `Str`, `Obj`, `Array`, `Hash`, `Ident`, 
`Lock`, and `Time`. See below for documentation on building validators for 
each of these.

A special validator type, `Multi`, doesn't validate a specify fog-pack value 
type, but can be used to make a validator that matches if *any* of its 
sub-validators pass.

Finally, a validator may be an object with 0 field-value pairs, in which case it 
is the empty validator. The empty validator passes for any value, but permits no 
queries against it.

### Aliasing Validators

Besides the basic type validators, schema allow for validator aliasing. A 
special `types` object lets more complex validators be reused throughout the 
schema. See the schema documentation for details.

Common Validator Fields
-----------------------

All validators (except for the empty validator) have the required firled `type` 
and the optional field `comment`. Many of them also have the `query`, `in`, 
`nin`, and `default` fields. Optional fields limit the accepted values in some 
way, or determine what optional fields a query validator in the same spot may 
have.

### `type`

All validators besides the empty validator must have the `type` field, set to 
that name of the base type or set to an aliased type name. In the latter's case, 
no other fields besides `type` and `comment` are allowed.

### `comment`

All types may have a `comment` field, which is a descriptive string not used 
for validation. It is purely used to provide context to anyone developing 
with the schema.

### `default`

Most types have a `default` field, which provides a default value that may 
be used if the described value is absent or when one is constructing a 
fog-pack value and needs a default value.

### `query`, `in`, and `nin`

The `query` field (if present) contains a boolean. If set to true, it allows 
queries to have validators with `in` and `nin` fields. `in` specifies a limited 
set of allowed values, while `nin` specifies a limited set of prohibited values.

Validator Types
---------------

### Null

Null types describe the Null value. They have no additional fields besides 
`comment`.

If the provided value is Null, validation passes.

#### Examples

```json
{
    "type": "Null",
    "comment": "Example Null type validator"
}
```

### Bool

Bool types describe a Boolean value. They have the following 
optional fields:

- `default`
- `comment`
- `in`: A boolean the value must match.
- `nin`:A boolean the value must not match.
- `query`: Boolean. Allows queries against the value to use `in` and `nin`.

Validation fails if the value is not boolean or does not meet the `nin` and 
`in` requirements.

#### Examples

A boolean validator allowing only "true" would look like:

```json
{
    "type": "Bool",
    "in": true
}
```

### Int

Int types describe an allowed Integer value. They have the following 
optional fields:

- `default`
- `comment`
- `in`: An integer or array of integers the value must be among.
- `nin`: An integer or array of integers the value must not be among.
- `min`: The minimum allowed integer value.
- `max`: The maximum allowed integer value.
- `ex_min`: A boolean that, if true, changes min to not allow equality. If 
    no `min` field is provided, the minimum allowed is the minimum possible 
    64-bit signed integer value plus 1.
- `ex_max`: A boolean that, if true, changes max to not allow equality. If 
    no `max` field is provided, the maximum allowed is the maximum possible 
    64-bit unsigned integer value minus 1.
- `bits_set`: An integer used as a bit field. Any bits set in it must also 
    be set in an allowed value.
- `bits_clr`: An integer used as a bit field. Any bits set in it must be 
    cleared in an allowed value.
- `bit`: Boolean. Allows queries to use `bits_set` and `bits_clr`.
- `ord`: Boolean. Allows queries to use `min`, `max`, `ex_min`, and `ex_max`.
- `query`: Boolean. Allows queries to use `in` and `nin`.

Validation fails if the value is not an integer or does not meet all of the 
optional requirements.

#### Examples

An integer validator allowing any positive value between 0 and 
255, while not allowing bit 6 to be set, would look like:

```json
{
	"type": "Int",
	"min": 0,
	"max": 256,
	"ex_max": true,
	"bits_clr": 64
}
```


### Str

Str types describe an allowed String value. Strings are always valid UTF-8, and 
support counting their length by both the number of raw encoded bytes and the 
number of Unicode scalar values. They have the following optional fields:

- `default`
- `comment`
- `in`: A string or array of strings the value must be among.
- `nin`: A string or array of strings the value must not be among.
- `matches`: A string or array of strings, each of which contains a regex the 
	described field must match. General perl-style regular expressions are 
	supported, but without look around and backreferences.
- `max_len`: The maximum number of bytes allowed for the value. Must be at least 
	0.
- `min_len`: The minimum number of bytes allowed for the value. Must be at least
	0.
- `max_char`: The maximum number of unicode scalar values allowed for the value. 
	Must be at least 0.
- `min_char`: The minimum number of unicode scalar values allowed for the value.
	Must be at least 0.
- `force_nfc`: Boolean. Runs the string to be validated, `in`, `nin`, and 
	`matches` through Unicode normalization to NFC before performing validation.
- `force_nfkc`: Boolean. Runs the string to be validated, `in`, `nin`, and 
	`matches` through Unicode normalization to NFKC before performing validation.
	This overrides `force_nfc` if it is also set to true.
- `query`: Boolean. Allows queries to use `in` and `nin`.
- `regex`: Boolean. Allows queries to use `matches`.
- `size`: Boolean. Allows queries to use `min_len`, `max_len`, `min_char`, and 
	`max_char`.

Validation fails if the value is not a string or does not meet all of the 
optional requirements.

Note that the Unicode NFC and NFKC forms are never enforced when encoding a 
string, but are only used for the purposes of validation. This ensures that 
precise UTF-8 can be preserved while still allowing matching with NFC/NFKC 
forms.

#### Examples

Say we want a string validator that only allows valid Unix file names and 
excludes the special "." and ".." file names. We thus want to only allow strings 
without a forward slash or null character, that are between 1 and 255 bytes in 
size, but aren't "." or "..". Perhaps we also want to allow exact queries with 
`in` and `nin` as well. The resulting validator could be:

```json
{
    "type": "Str",
    "min_len" : 1,
    "max_len": 255,
    "nin": [ ".", ".." ],
    "matches": "^[^/\0]*$",
    "query": true
}
```

### F32 & F64

F32 types describe an allowed IEEE 754 binary32 floating-point value, while F64 
types describe an allowed IEEE 754 binary64 value. They support the following 
optional fields:

- `default`
- `comment`
- `ex_min`: A boolean that, if true, doesn't allow equality for the minimum 
	value. If `min` isn't present, but this is set, the minimum allowed value is 
	anything except for NaN and negative infinity.
- `ex_min`: A boolean that, if true, doesn't allow equality for the minimum 
	value. If `max` isn't present, but this is set, the maxixmum allowed value is 
	anything except for NaN and positive infinity.
- `min`: The minimum allowed value, with NaN not permitted. NaN is not permitted 
	if this is present.
- `max`: The maximum allowed value, with NaN not permitted. NaN is not permitted 
	if this is set.
- `in`: A floating-point value or array of values the value must be among.
- `nin`: A floating-point value or array of values the value must not be among.
- `ord`: Boolean. Allows queries to use `min`, `max`, `ex_min`, and `ex_max`.
- `query`: Boolean. Allows queries to use `in` and `nin`. Default is false.

Validation fails if the value is not the correct type of floating-point value or 
does not meet all of the optional requirements.

#### Examples

Say we want to accept any float64 value that isn't NaN or +/- infinity. The 
validator could be:

```json
{
    "type": "F64",
    "ex_min": true,
    "ex_max": true
}
```

### Bin

Bin types decribe an allowed byte vector. The byte vector can also be 
functionally used as an arbitrary-length, little-endian unsigned value for the 
purposes of ordinal comparisons. They have the following optional fields:

- `default`
- `comment`
- `bits_clr`: A byte vector used as a bit field. Any bits set in it must be 
	cleared in an allowed value.
- `bits_set`: A byte vector used as a bit field. Any bits set in it must be set 
	in an allowed value.
- `ex_max`: A boolean that, if true, doesn't allow equality for the maximum 
	value set by `max`.
- `ex_min`: A boolean that, if true, doesn't allow equality for the minimum 
- value set by `min`. If `min` isn't present, the minimum allowed value is a 1 
	for the first byte.
- `min`: The minimum allowed value, as a little-endian unsigned value. 
- `max`: The maximum allowed value, as a little-endian unsigned value. 
- `min_len`: The minimum number of bytes allowed for the value. Must be at least 
	0.
- `max_len`: The maximum number of bytes allowed for the value. Must be at least 
	0.
- `in`: A byte vector or array of byte vectors the value must be among.
- `nin`: A byte vector or array of byte vectors the value must not be among.
- `bit`: Boolean. Allows queries to use `bit_clr` and `bit_set`.
- `ord`: Boolean. Allows queries to use `min`, `max`, `ex_min`, and `ex_max`.
- `query`: Boolean. Allows queries to use `in` and `nin`.
- `size`: Boolean. Allows queries to use `min_len` and `max_len`.

Validation fails if the value is not a binary value or does not meet all of the 
optional requirements.

#### Examples

Say we want to accept a 32-byte bitfield, but only if bit 31 is set. The 
resulting schema would be:

```json
{
    "type": "Bin",
    "max_len": 32,
    "bits_set": "<Bin([0x00, 0x00, 0x00, 0x80])>"
}
```

Note that we don't need to require a minimum length here - we could implement a
handler that assumes non-present bytes are all 0.

### Array

Array types describe an allowed array value. They support the following optional 
fields:

- `comment`
- `default`
- `contains`: An array of validators. For each validator in the array, the array 
	must contain a value that passes that validator. A single item in the array 
	may pass multiple validators within the `contains` array.
- `items`: An array of validators. Each validator in the `items` array 
	corresponds to a value in the array at the same position. All values in the 
	array must pass their corresponding validator in the `items` array to pass 
	validation.
- `extra_items`: A validator that any array items not covered by the optional 
	`items` array must adhere to. If `items` isn't present, all array items are 
	validated against `extra_items`.
- `in`: An array of arrays that the value must be among.
- `nin`: An array of arrays that the value must not be among.
- `max_len`: The maximum number of items allowed in the array. Must be at least 
	0.
- `min_len`: The minimum number of items allowed in the array. Must be at least 
	0.
- `unique`: Boolean. Requires that every value in the array be unique if true.
- `query`: Boolean. Allows queries to use `in`, and `nin`.
- `size`: Boolean. Allows queries to use `min_len` and `max_len`.
- `contains_ok`: Boolean. Allows queries to use `contains`.
- `unique_ok`: Boolean. Allows queries to use `unique`.
- `array`: Boolean. Allows queries to use `items` and `extra_items`.

Validation fails if the value is not an array or does not meet all of the 
optional requirements.

When checking validity of queries, validators are matched against the ones they 
would be used against. So:

- Each `items` validator is checked against the schema's corresponding one, or 
	against the schema's `extra_items` if the schema has that field. If there is 
	no `extra_items` field in the schema and no corresponding `items` validator, 
	the validator is allowed in the query.
- The `extra_items` validator is checked against the schema's `extra_items` 
	validator, as well as any unmatched `items` validators in the schema's array 
	validator.
- Validators in the query's `contains` are checked against all validators in 
	the schema's `items` array, as well as against the validator in the schema's 
	`extra_items` field.

#### Examples

As an example, say we wanted to accept an array of arrays, where each sub-array 
is exactly three items and contains a string, then two integers. The resulting 
validator could look like:

```json
{
    "type": "Array",
    "extra_items": {
        "type": "Array",
        "min_len": 3,
        "max_len": 3,
        "items": [
            { "type": "Str" },
            { "type": "Int" },
            { "type": "Int" }
        ]
    }
}
```

### Obj

Object types describe an allowed object value. They support the following 
optional fields:

- `comment`
- `default`
- `in`: An object or array of objects that the value must be among.
- `nin`: An object or array of objects that the value must not be among.
- `max_fields`: The maximum number of allowed field-value pairs in the object. 
	Must be at least 0.
- `min_fields`: The minimum number of allowed field-value pairs in the object. 
	Must be at least 0.
- `req`: An object where each field is an optional field for the described 
	object, and each associated value is the validator for that field's value in 
	the described object.
- `opt`: An object where each field is a required field for the described 
	object, and each associated value is the validator for that field's value in 
	the described object.
- `ban`: A string or array of strings that none of the fields are allowed to 
	be among.
- `field_type`: A validator that any field values not covered by `req` or `opt` 
	must adhere to.
- `unknown_ok`: Allows field-value pairs not covered by `req` and `opt`. This 
	must be set for `field_type` to be useful.
- `query`: Boolean. Allows queries to use `in` and `nin`.
- `obj_ok`: Boolean. Allows queries to use `req`, `opt`, `ban`, `field_type`, 
	and `unknown_ok`.

Unlike other validators, Object does not default to "anything goes" if none of 
the optional fields are specified. "Anything goes" can be enabled by setting 
`unknown_ok` to true and not defining anything else. Additionally, anything can 
be allowed with no queries permitted by setting `unknown_ok` to true and setting 
`field_type` to the empty validator.

The validation procedure for Object types is a bit more complex, and proceeds as 
follows:

1. If the number of fields in the object is outside the optional ranges set by 
	`max_fields` and `min_fields`, validation fails.
2. For each field-value pair:
	1. Check the field.
	2. If it is in the `ban` array, validation fails.
	3. If it is in the `req` object, verify against the corresponding validator.
	4. If it is not in `req` but is in `opt`, verify against the corresponding 
		validator.
	5. If it is not in `req` or `opt`, and `unknown_ok` is set to true, check it 
		against the optional `field_type` validator. If `field_type` is not present, 
		it passes regardless.
	6. If it isn't in `req` or `opt` and `unknown_ok` is not set to true, 
		validation fails.
3. Check the object against the objects in the optional `nin` list. If it is in 
	the list, validation fails.
4. If the `in` list exists, check the object agaisnt the objects in that list. 
	If it isn't in the list, validation fails.

When checking validity of queries, validators are matched against the ones they 
would be used against. So:

- Fields in the query's `req` object are matched first against the schema's 
	`req`, then against the schema's `opt` if nothing is in `req`, and finally 
	against `field_type` if nothing is in either. If it finds nothing to match 
	against, the query is invalid if `unknown_ok` is not set to true.
- Fields in the query's `opt` object go through the same procedure as the ones 
	in the query's `req` object.
- If the query has `field_type`, that validator is matched against the schema's 
	`field_type` validator, along with any validators in `req` and `opt` that 
	weren't covered by the query's `req` and `opt` objects.

### Hash

Hash types describe an allowed cryptographic [`Hash`](./struct.hash.html). They 
support the following optional fields:

- `comment`
- `default`
- `in`: A hash or array of hashes the value must be among.
- `nin`: A hash or array of hashes the value must not be among.
- `link`: A validator that the document matching the hash must adhere to. This 
	field is only used when the validator is for an entry; it is ignored for 
	documents.
- `schema`: A hash or array of hashes that match various schemas. The document 
	matching a validated hash must use one of these schemas. This field is only 
	used when the validator is for an entry; it is ignored for documents.
- `query`: Boolean. Allows queries to use `in` and `nin`.
- `link_ok`: Boolean. Allows queries to use `link`.
- `schema_ok`: Boolean. Allows queries to use `schema`.

Validation fails if the value is not a hash or does not meet all of the optional 
requirements. Validation may require fetching additional documents if the tested 
value is in an entry.

### Ident

Identity types describe an allowed [`Identity`](./struct.Identity.html) / 
cryptographic public key. They support the following optional fields:

- `comment`
- `default`
- `in`: An identity or array of identities the value must be among.
- `nin`: An identity or array of identities the value must not be among.
- `query`: Boolean. Allows queries to use `in` and `nin`.

Validation fails if the value is not an identity or does not meet all of the 
optional requirements.

### Lock

Lockbox types describe an allowed [`Lockbox`](./stuct.Lockbox.html), which is an 
encrypted value of some kind. They have the following optional fields:

- `comment`
- `max_len`: The maximum number of bytes allowed in a complete lockbox (not 
	counting the `ext_type` wrapper).
- `size`: Boolean. Allows queries to use `max_len`.

Validation fails if the value is not a lockbox or has a length greater than 
allowed by `max_len`.

`default` is not allowed; No implementation should need or expect a default 
value for an encrypted value.

### Time

Timestamp types describe an allowed [`Timestamp`](./struct.Timestamp.html). They 
have the following optional fields:

- `comment`
- `default`
- `min`: The minimum allowed timestamp.
- `max`: The maximum allowed timestamp.
- `ex_min`: Boolean that, if true, doesn't allow for equality with the minimum 
	value. If no `min` field is present, the minimum possible time is not
	allowed.
- `ex_max`: Boolean that, if true, doesn't allow for equality with the maximum 
	value. If no `max` field is present, the maximum possible time is not
	allowed.
- `in`: A timestamp or array of timestamps that the value must be among.
- `nin`: A timetamp or array of timestamps that the value must not be among.
- `ord`: Boolean. Allows queries to use `min`, `max`, `ex_min`, and `ex_max`.
- `query`: Boolean. Allows queries to use `in` and `nin`.

Validation fails if the value is not a timestamp or does not meet all of the 
optional requirements.

### Multi

A Multi type is not an actual type; instead, it allows any value that can meet 
at least one of the validators in its `any_of` array. If no `any_of` array is 
present, then no value is allowed. It has only two optional fields:

- `comment`
- `any_of`: An array of validators.

When a specified type is queriable, the Multi type is similarly queriable 
when it meets the type specification.
