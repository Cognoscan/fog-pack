# Human-Readable MessagePack #

When describing MessagePack values, it is useful to display them in a 
human-readable format with options to use short names instead of raw binary 
data. For the sake of making syntax highlighting work, this format will be 
JSON-compatible.

All value types can be written as as string containing `<Type>` in lieu of an 
actual value, when only a placeholder is needed. These placeholders may also 
include a name in parentheses immediately after the type, like `<Type(name)>`. 
Names may not include parentheses, and may not start with `0x` or `64x`.

For types that store binary data, the name may be replaced with hex-encoded data 
or Base64 encoded data (using the RFC 2045 encoding). In these cases, the 
hex-encoded data is prefixed with `0x` and Base64-encoded data is prefixed with 
`64x`.

Allowed types are Null, Boolean, Integer, String, Floating, Binary, Array, 
Object, Hash, Identity, Lockbox, and Timestamp. The short name for each type may 
also be used.

If an actual value is to be written, the following rules should be observed:

- Null is always written lowercase as `null` for JSON-compatiblity
- Boolean is always written as either `true` or `false`
- Strings are always encapsulated in quotes. Quotes in the string are escaped 
	with a backslash, backslashes are escaped with a double-backslash, and 
	newlines/carriage returns/backspace/solidus/formfeeds/tabs are replaced with 
	the appropriate backslash-excaped code. If a string starts with `<`, a second 
	`<` is prepended to let a reader recognize this is not one of the custom types 
	not supported in JSON.
- Floating point may have an exact representation. Instead of writing a JSON 
	number, it can be written as a string with brackets at either end, with the 
	value given in parentheses after the human-readable version. As an example:
	`"<-234.01234e4(0xC141_DA8D_B333_3333)>"` shows the value encoded as a 
	double-precision floating-point word.
- Binary, Hash, Identity, and Lockbox types are never written 
	directly. They must always be written in the `<Type(name)>` form, with the 
	option to embed the binary data instead of a name.
- Arrays are displayed by enclosing the array of values in brackets and 
	separating each with commas. Example: `[ "value0", "value1", "value2" ]`.
- For objects, each field-value pair are separated by a colon, with the 
	field-value pairs separated by commas and enclosed in curly brackets, just 
	like regular JSON objects. Example:
```json
{
	"field0": "value0",
	"field1": "value1",
	"field2": "value2"
}
```
- Timestamps are displayed as `<Timestamp(name)>`. Name may also be the direct 
	binary representation of the data within the `ext` type, or it may be a 
	human-readable timestamp, preferrably one meeting ISO 8601.

Documents, entries, and queries may be shown by creating a JSON object whose 
keys are of the form `document(id)`, `entry(id)`, or `query(id)`, with a unique 
identifier for each. The content of each is an array whose first element is the 
document/entry/query, and whose remaining elements are signatures, signified by 
using `<Signature(id)>` for each.

## Examples ##

Certificate Document, Certificate Entry, and query for an entry:

```json
{ 
  "document(ID user cert)": [{
    "$schema": "<Hash(cert scheme)>",
    "id": "<Identity(ID user)>"
  }],

  "entry(0)": [
    [
      "<hash(ID user cert)>",
      "cert",
      {
        "begin": "<Timestamp>",
        "end": "<Timestamp>",
        "name": "<String>",
        "value": "<Integer>",
        "id": "<Hash(ID signer cert)>"
      }
    ],
    "<Signature(ID signer)>"
  ],

  "query(example)": [{
    "root": [ "<Hash(ID user cert)>" ],
    "query": {
      "cert": {
        "begin": { "$gte": "<Timestamp(now)>" },
        "end":   { "$lte": "<Timestamp(now)>" },
        "name": "<String>",
        "$signed": { "signer": "<Identity(ID signer)>" }
      }
    }
  }]
}
```


