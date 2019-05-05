Schema Language
===============

This specifies a schema language for validating documents and their entries. A 
schema document is itself a MessagePack Document, and conforms to a schema 
document.

The primary purpose of a schema document is to specify agreed-upon formats for 
documents and entries. By doing so, a query does not need to include any 
validation criteria. The secondary purpose of a schema document is to specify 
which fields should be queryable, and how they may be queried. This allows a 
database to optimize for queries ahead of time.

All documents may refer to the schema document they meet using an empty string 
for the field. This unique field, if used, must contain a schema hash. If left 
unspecified, it is assumed that there is no schema.

When are Documents Validated?
-----------------------------

Documents & entries are validated against their schema documents at two points: 
when they are received from an external source, and when they are to be added to 
the local database. If a received document/entry is to be immediately added to 
the database, the validation step need not occur twice.

### Validation when adding to the database

Validation is simple when adding to the local database. A document must:

1. Refer to a schema document already in the database
2. Pass validation for the schema document

A entry is similarly simple. It must:

1. Be an entry for a document already in the database
2. Be a valid entry for that document

If the above rules are met for a document/entry, then it passes validation and 
will be added into the database.

### Validating received data

Received data must be temporarily cached in order to support validation. When a 
document is received, if it refers to a schema document not yet in the database, 
a query for that schema document will be generated. This query will have the 
same permissions as the query that resulted in the received document. It will be 
considered a related query - see the [query specification](query.md) for 
details.

Once the schema document is either in the database or cached, the received 
document is validated against it. The same goes for received entries. If a 
received entry refers to a document not received or in the database, it is held 
until all documents have been received or added to the database.

Schema Document Format
----------------------

The core concept in schema documents is the "data type". A data type is an 
object containing the validation rules for a given field. It can directly define 
a field, or it can be aliased and used throughout the schema document. 

A schema document consists of some descriptive fields, 3 array fields specifying 
what fields are permitted in the document, an array of data type definitions 
that may be used in the schema, and a boolean field indicating if the schema 
permits unknown fields in a document.

A schema document must, at minimum, have a name field. This should be a 
descriptive string naming the schema. The other permitted fields are all 
optional, and unspecified fields are not allowed.

The permitted optional fields are:

- `comment`: Description of the schema. Not used in validation.
- `version`: Version number to differentiate from previously named schema. Not 
	used in validation.
- `required`: Array of data types. Each named type is a field that must be 
	present in a document meeting the schema.
- `optional`: Array of data types. Each named type is a field that can be 
	present in a document meeting the schema.
- `entries`: Array of data types. Each named type is a field that can be used in 
	an entry attached to the document. If an entry does not match one of the data 
	types here, it will fail validation.
- `types`: Array of data types. Each named type can be referred to as a type in 
	the `required`, `optional`, and `entries` arrays. It may also be referred to 
	by other types in the `types` array.
- `unknown_ok`: Boolean indicating if unknown fields are allowed in the 
	document.

Names that begin with a "$" character are considered reserved, as are the base 
type names. If a schema has a named type beginning with a "$" or attempts to 
override one of the base type, that type shall be ignored. The exception to this 
is when validating the root schema document or the query schema document, both 
of which define the reserved types.

### Validation Sequence

Validating a document against a schema proceeds as follows. First, read in all 
types in the schema `type` array and store them as type validators. Do likewise 
for the `required`, `optional`, and `entries` field.  Then, for each field in 
the document being validated:

1. If field is named in the `required` array types, validate against the type 
	whose name it has. If it passes, record that the type has been used. If the 
	type has been previously used, validation fails.
2. If field is named in the `optional` array types, validate against the type 
	whose name it has. If it passes, record that the type has been used. If the type 
	has been previously used, validation fails.
3. If the field is named in neither `required` nor `optional`, and `unknown_ok` 
	is not present or is set as false, validation fails.
4. Follow the validation rules given by the type found in steps 1/2.

For entries attached to a document, the field is checked against the `entries` 
array of types. If it isn't named there, and `unknown_ok` is not present or is 
set to false, validation of the entry fails.

### Data Types

All data types have a `name` field that must contain a string. The `name` string 
must be unique amongst the array of types it is in. As an example, one can have 
a `addressUS` and `addressCA` type in the same array, but one cannot have a 
two separate types both named `address`.

Besides the `name` field, a `type` field is also required. This must be a string 
set to one of the base types, or must be set to one of the type names defined in 
the `types` array of the document schema. If the string does not match any of 
these, validation for the field should always fail. String matching is always 
exact and case-sensitive.

All types may also have a `comment` field, which is a descriptive string not 
used for validation.

Finally, all types (except Multi) may have `query` and `sign` fields, both of 
which are boolean. If `query` is set to true, the field defined by the type may 
be queried in a document. If `sign` is set to true, the field might be signed 
and can be queried for signature conditions. If it is not true or does not 
exist, signatures are not permitted for that field.  See the 
[query specification](query.md) for more detail.

Base types are: Null, Bool, Int, Str, F32, F64, Bin, Array, Obj, Hash, Ident, 
Lock, Time, and Multi.

As a note: some type descriptions may specify a `const` value or `default` value 
that doesn't meet the other requirements specified. If this is the case, the 
validation should always fail if `const`, or always fail if the `default` value 
is used.

#### Null

Null types describe a field with a Null data. They have no fields besides the 
basic ones.

If the data for the described field is Null, validation passes.

#### Bool

Bool types describe a field with Boolean data. They have the following 
optional fields:

- `const`: a boolean value the described field must be set to.
- `ord`: Allows ordinal comparisons of this field in queries if set to true.
- `default`: Specifies a default that implementations may use if the field is 
	not present.

Validation fails if the described field is not boolean or does not meet any of 
the optional requirements.

#### Int

Int types describe a field with integer data. They have the following optional 
fields:

- `const`: an integer the described field must be set to.
- `in`: an array of unique integers that the described field must be among.
- `nin`: an array of unique integers that the described field must not be among.
- `min`: An integer the described field must be equal to or greater than.
- `max`: An integer the described field must be equal to or less than.
- `ex_min`: A boolean that, if true, changes min to not allow equality.
- `ex_max`: A boolean that, if true, changes max to not allow equality.
- `ord`: Allows ordinal comparisons of this field in queries if set to true.
- `bit`: Allows bitwise comparisons of this field in queries if set to true.
- `default`: Specifies a default that implementations may use if the field is 
	not present.

If `min` and `max` are both present, their behavior is dependent on how they 
compare to each other:

| Condition     | Validation Requirement                    |
| --            | --                                        |
| `min` < `max` | Described field must meet `min` AND `max` |
| `min` > `max` | Described field must meet `min` OR `max`  |
| `min` = `max` | Described field must equal `min`          |

Finally, **if `ex_min` or `ex_max` are true** and the `min` = `max`, validation 
must always fail.

Validation fails if the described field is not an integer or does not meet any 
of the optional requirements.

#### Str

Str types describe a field with string data. They have the following optional 
fields:

- `const`: a string the described field must be set to.
- `in`: an array of unique strings the described field must be among.
- `nin`: an array of unique strings the described field must not be among.
- `min_len`: a non-negative integer that describes the minimum number of allowed 
	bytes in the string. This is *not* the number of characters.
- `max_len`: a non-negative integer that describes the maximum number of allowed 
	bytes in the string. This is *not* the number of characters.
- `matches`: a regular expression the described field must match. See the 
	[regular expression documentation](regex.md) for what is supported here.
- `default`: Specifies a default that implementations may use if the field is 
	not present.
- `ord`: Allows ordinal comparisons of this field in queries if set to true.
- `regex`: Allows regex matching of this field in queries if set to true.

Validation fails if the described field is not a string or does not meet any of 
the optional requirements.

#### F32 & F64

F32 types describe a field with an IEEE 754 binary32 floating-point value, while F64 
describes one with an IEEE 754 binary64 floating-point value. Their optional 
fields are essentially the same, but only accept F32 or F64 values, 
respectively:

- `const`: a floating-point value the described field must be set to.
- `in`: an array of unique floating-point values that the described field must 
	be among.
- `nin`: an array of unique floating-point values that the described field must 
	not be among.
- `min`: A floating-point value the described field must be equal to or greater 
	than.
- `max`: A floating-point value the described field must be equal to or less 
	than.
- `ex_min`: A boolean that, if true, changes min to not allow equality.
- `ex_max`: A boolean that, if true, changes max to not allow equality.
- `ord`: Allows ordinal comparisons of this field in queries if set to true.
- `default`: Specifies a default that implementations may use if the field is 
	not present.

If `min` and `max` are both present, their behavior is dependent on how they 
compare to each other:

| Condition     | Validation Requirement                    |
| --            | --                                        |
| `min` < `max` | Described field must meet `min` AND `max` |
| `min` > `max` | Described field must meet `min` OR `max`  |
| `min` = `max` | Described field must equal `min`          |

Finally, **if `ex_min` or `ex_max` are true** and the `min` = `max`, validation 
must always fail.

Validation fails if the described field is not a floating-point value of the 
appropriate type or does not meet any of the optional requirements.

#### Bin

Bin types describe a field with an arbitrary binary data sequence. They have the 
following optional fields:

- `const`: a binary sequence the described field must be set to.
- `in`: an array of binary sequences the described field must be among.
- `nin`: an array of binary sequences the described field must not be among.
- `min_len`: a non-negative integer specifying the minimum number of bytes 
	allowed in the field.
- `max_len`: a non-negative integer specifying the maximum number of bytes 
	allowed in the field.
- `ord`: allows ordinal comparisons of this field in queries if set to true.
- `bit`: allows bitwise comparisons of this field in queries if set to true.
- `default`: specifies a default that implementations may use if the field is 
  not present.

Validation fails if the described field is not binary or does not meet any of 
the optional requirements listed.

#### Array

Array types describe a field that contain an array of values. They have the 
following optional fields:

- `const`: an array the described field must be set to.
- `in`: an array of arrays the described field must be among.
- `nin`: an array of arrays the described field must not be among.
- `min_len`: a non-negative integer specifying the minimum number of values 
	allowed in the array.
- `max_len`: a non-negative integer specifying the maximum number of values 
	allowed in the array.
- `items`: a single string or an array of strings that refer to types allowed in 
	the array.
	- If a single string, all values in the array must meet the requirements of 
		the type specified.
	- If an array of strings, the value at each position in the described field's 
		array must validate against the type in the corresponding position of the 
		`items` array.
- `extra_items`: a string specifying a type. Any values in the described field's 
	array that aren't specified by `items` must validate against the type 
	specified here. If `items` is not present, `extra_items` should be ignored.
- `contains`: a single string or an array or strings specifyng types that must 
	be present in the array. All of the specified types must be present at least 
	once in the array. If a value meets multiple types, they are all considered to 
	be met.
- `unique`: A boolean that, if true, requires each value in the array to be 
	unique.
- `array`: allows array queries on this field if set to true.
- `default`: specifies a default that implementations may use if the field is 
	not present.

Validation fails if the described field is not an array or does not meet any of 
the optional requirements listed.

#### Obj

Obj types describe a field that contains an object. They have the following 
optional fields:

- `in`: an array of objects the described field must be among.
- `nin`: an array of objects the described field must not be among.
- `const`: an object the described field must be set to.
- `required`: an object listing unique Data Types that must be present in the 
	object, where each field is the name of a field in the object.
- `optional`: an object listing unique Data Types that may optionally be present 
	in the object, where each type name is the name of a field in the object.
- `min_fields`: a non-negative integer specifying the minimum number of fields 
	allowed in the object.
- `min_fields`: a non-negative integer specifying the maximum number of fields 
	allowed in the object.
- `field_type`: a string specifying what type all fields in an object must be. 
	This is only used if the field has not been previously specified in the 
	`required` or `optional` arrays.
- `default`: specifies a default object that implementations may use if the 
	field is not present.
- `unknown_ok`: specifies if fields not specified in `required` or `optional` 
	are allowed in the object.

Validation fails if the described field is not an object or does not meet any of 
the optional requirements listed.

#### Hash

Hash types describe a field that contains a cryptographic hash. They have the 
following optional fields:

- `in`: an array of hashes the described field must be among.
- `nin`: an array of hashes the described field must not be among.
- `const`: a hash the described field  must be set to.
- `default`: a default hash implementations may use if the field is not present.
- `link`: allows `$link` queries on this field if set to a Hash or array of 
	hashes. If this field is within an entry, the document referred to by the 
	field's hash must be validated against the schema document the `link` Hash 
	refers to, or must have been validated against one of the schema in the array 
	and have that match its specified schema. If the Hash for `link` refers to a 
	non-schema document, validation always fails. This is ignored if used for 
	anything that isn't in an entry, as `$link` queries can't be made within 
	document fields.

Validation fails if the described field is not a hash or does not meet any of 
the optional requirements listed.

#### Ident

Ident types describe a field that contains a cryptographic public key, an 
"Identity". They have no optional fields besides the basic `comment` and `query` 
fields. `default` is not allowed; any implementation must always be able to 
handle if a field normally containing a public key is not present.

#### Lock

Lock types describe a field that contains encrypted data (a "Lockbox"), which 
may be a private key, secret key, or an encrypted data payload. They have the 
following optional fields:

- `max_len`: a non-negative integer specifying the maximum length of the lockbox 
	itself.

`default` is not allowed; any implementation must always be able to handle if a 
field normally containing a lockbox is not present.

#### Time

Time types describe a field that contains a timestamp. They have the following 
optional fields:

- `const`: a timestamp the described field must be set to.
- `in`: an array of unique timestamps that the described field must be among.
- `nin`: an array of unique timestamps that the described field must not be 
	among.
- `min`: A timestamp the described field must be equal to or greater than.
- `max`: A timestamp the described field must be equal to or less than.
- `ex_min`: A boolean that, if true, changes min to not allow equality.
- `ex_max`: A boolean that, if true, changes max to not allow equality.
- `ord`: Allows ordinal comparisons of this field in queries if set to true.
- `default`: Specifies a default that implementations may use if the field is 
	not present.

If `min` and `max` are both present, their behavior is dependent on how they 
compare to each other:

| Condition     | Validation Requirement                    |
| --            | --                                        |
| `min` < `max` | Described field must meet `min` AND `max` |
| `min` > `max` | Described field must meet `min` OR `max`  |
| `min` = `max` | Described field must equal `min`          |

Finally, **if `ex_min` or `ex_max` are true** and the `min` = `max`, validation 
must always fail.

Validation fails if the described field is not a timestamp or does not meet any 
of the optional requirements.

#### Multi

A Multi type is not an actual type; instead, it allows for something to validate 
against one of several types. Unlike all other types, it has no support for the 
`query` or `sign` field. It has a single additional required field besides 
`name` and `type`: `any_of`.

`any_of` is an array of unique strings that specify types from the document 
schema's `type` array. If a field validates against any of the types named, it 
passes validation. The exception is if the named type is itself a Multi type; 
validation automatically fails for that specific type. This is to prevent 
infinite recursion when doing type validation.

When a specified type is queriable, the Multi type is similarly queriable when 
it meets the type specification. As such, an implementation must, at minimum, 
check against all queriable types when adding a document/entry to the database.


### Self-Validation

The schema is self-validating: that is, there exists a schema document that can 
be used to validate itself and all other schema documents.


## Schema Fields

### Miscellaneous

| Field       | Content | Usage                                               |
| --          | --      | --                                                  |
| version     | Integer | Schema documentation only                           |
| name        | String  | Schema documentation only                           |
| description | String  | Schema documentation only                           |
| comment     | String  | Validator documentation only                        |
| default     | Any     | Recommended default value for a validator           |
| entries     | Object  | Accepted fields for entries (like `req` and `opt`)  |
| types       | Object  | Named Validators, which may be referenced elsewhere |

### Validator Fields

| Field       | Content                      |
| --          | --                           |
| type        | String                       |
| any         | Array of Validators          |
| in          | Array of type                |
| nin         | Array of type                |
| min         | Numeric type                 |
| max         | Numeric type                 |
| ex_min      | Boolean                      |
| ex_max      | Boolean                      |
| bits_set    | Integer/Binary               |
| bits_clr    | Integer/Binary               |
| min_len     | Non-negative Integer         |
| max_len     | Non-negative Integer         |
| match       | String Array                 |
| req         | Object with Validator Values |
| opt         | Object with Validator Values |
| min_fields  | Non-negative Integer         |
| max_fields  | Non-negative Integer         |
| field_type  | Validator                    |
| unknown_ok  | Boolean                      |
| items       | Array of Validators          |
| extra_items | Validator                    |
| contains    | Array of Validators          |
| unique      | Boolean                      |
| schema      | Array of Hashes              |
| link        | Validator                    |

### Query Specifier Fields

| Field        | Content              |
| --           | --                   |
| query        | bool                 |
| set          | non-negative integer |
| ord          | bool                 |
| bit          | non-negative integer |
| link_ok      | bool                 |
| regex        | non-negative integer |
| array        | non-negative integer |
| contains_num | non-negative integer |


### Integer Type

Integer validators have the following validation fields:

- `type`: Must be `Int` or not present.
- `in`: A single integer or array of integers that are allowed by the validator. 
	If not present, any integer meeting the other criteria is allowed.
- `nin`: A single integer or array of integers that are not allowed by the 
	validator.
- `min`: Indicate the minimum value allowed by the validator. Defaults to the 
	minimum possible integer value if not present.
- `max`: Indicate the maximum value allowed by the validator. Defaults to 
	maximum possible integer value if not present.
- `ex_min`: Optional boolean. Changes `min` to be exclusive instead of inclusive.
- `ex_max`: Optional boolean. Changes `max` to be exclusive instead of inclusive.
- `bits_set`: Optional Integer value. Requires that all bits set in it also be 
  set in the checked value. Negative values are always sign-extended.
- `bits_clr`: Optional Integer value. Requires that all bits set in it be 
	cleared in the checked value. Negative values are always sign-extended.

In addition: integer validators have the query-relevant optional `query`, `ord`, 
and `bit` boolean fields. Finally, they may also have `default` and `comment` 
fields.
