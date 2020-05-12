Schema
======

A schema is a fog-pack document that may be used to validate & compress other 
fog-pack documents and entries. It may also contain documentation and 
recommended defaults for documents & entries that adhere to the schema.

The primary purpose of a schema document is to specify agreed-upon formats for 
documents and entries. By doing so, a query does not need to include any 
validation criteria. The secondary purpose of a schema document is to specify 
which fields should be queryable, and how they may be queried. This allows an 
implementation to optimize for queries ahead of time, so long as the schema is 
known.

All documents may refer to the schema document they meet using an empty string 
for the field. This unique field, if used, must contain a schema hash. If left 
unspecified, it is assumed that there is no schema. For example, a basic 
schema might look like:

```json
{
	"document(Basic Schema)" : [{
		"": "<Hash(fog-pack Core Schema)>",
		"name": "Simple Schema",
		"required": {
			"title": { "type": "Str", max_len: 255},
			"text": { "type": "Str" }
		}
	}]
}
```

A document that uses this "Basic Schema" would look like:

```json
{
	"document(Example)": [{
		"": "<Hash(Basic Schema)>",
		"title": "Example Document",
		"text": "This is an example document that meets a schema"
	}]
}
```

Schema Document Format
----------------------

The core concept in schema documents is the validator. A validator is an 
object containing the validation rules for a particular part of a document. It 
can directly define the rules, or it can be aliased and used throughout the 
schema document. Validators are always one of the following types: `Null`, 
`Bool`, `Int`, `F32`, `F64`, `Bin`, `Str`, `Obj`, `Array`, `Hash`, `Ident`, 
`Lock`, `Time`, or `Multi`. A Validator may be aliased by using a type name not 
in this list, in which case it will use a validator in the schema's `types` 
object, detailed later.

At the top level, a schema document is an object validator with a few extra 
fields. The object validator is used for the document itself. The additional 
fields are for additional description, for entry validation, and for aliased 
validators.

A schema document must, at minimum, have a name field. This should be a 
descriptive string naming the schema. The other permitted fields are all 
optional, and unspecified fields are not allowed.

The permitted optional fields (besides those of the object validator) are:

- `name`: A name for the schema. Always a string.
- `description`: A string describing the intent of the schema.
- `version`: Version number to differentiate from previously named schema. Not 
	used in validation. This is always a non-negative integer.
- `entries`: Object whose fields are acceptable entry fields. The value for 
	a field is the validator that will be used when an entry with the field is 
	attached to a document.
- `types`: Object whose fields are names for various validators. The value for a 
	field is the validator referred to by the name.

### Aliasing Validators

A Validator alias may be created by using a Validator with a single field, 
`type`, whose value is a string that isn't one of the base types (that is, not 
`Int`, `Bin`, and so on). In this case, the type name will be looked up in the 
schema's `types` object. If it is not present, validation fails. If the 
validator is cyclically or self-referential, validation also fails.

If there is a field in `types` that equals one of the base type names, it will 
never be used.

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

