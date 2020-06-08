/*!

The fog-pack value structure used for Schema.

Schema are a special type of [`Document`](../../struct.Document.html) that describes 
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
or a type that allows several of them (Multi). See the [Validation 
Language](#../validation/index.html) for more info.

At the top level, a schema is an Object but without support for the `in`, `nin`, 
`comment`, `default`, `obj_ok`, or `query` optional fields. Instead, it supports 
a few additional optional fields for documentation, entry validation, and 
compression:

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

## Compression Settings

Compression can be set to a recommended default using `doc_compress` and 
`entries_compress`; if nothing is specified, zstd with the default compression 
level is used.

Compression can be specified using an object that may take on one of three 
forms. The first recommends that no compression be used:

```json
{
    "setting": false
}
```

The second recommends a specific compression level:

```json
{
    "setting": true
    "format": 0,
    "level": 3
}
```

Finally, the third allows for a zstd dictionary to be attached:

```json
{
    "format": 0,
    "level": 3,
    "setting": "[ATTACH BINARY DICTIONARY HERE]"
}
```

This object format can be directly used as the value for the `doc_compress` 
field, and is the value for each field in the `entries_compress` format. For 
example, recommending max compression for a document and none for the entries 
might look like:

```json
{
    "doc_compress": {
        "setting": true,
        "format": 0,
        "level": 22
    },
    "entries_compress": {
        "entry_type0": { "setting": false },
        "entry_type1": { "setting": false },
        "entry_type2": { "setting": false }
    }
}
```
*/
