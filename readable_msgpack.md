# Human-Readable MessagePack #

When describing MessagePack values, it is useful to display them in a 
human-readable format with options to use short names instead of raw binary 
data.

All value types can be written as `<type>` in lieu of an actual value, when only 
a placeholder is needed. These placeholders may also include a name in 
parentheses immediately after the type, like `<type(name)>`. Names may not 
include parentheses, and may not start with `0x` or `64x`.

For types that store binary data, the name may be replaced with hex-encoded data 
or Base64 encoded data (using the RFC 2045 encoding). In these cases, the 
hex-encoded data is prefixed with `0x` and Base64-encoded data is prefixed with 
`64x`.

Allowed types are nil, boolean, integer, string, floating, binary, array, 
object, hash, identity, signature, lockbox, and timestamp.

If an actual value is to be written, the following rules should be observed:

- Nil is always written as `nil`
- Boolean is always written as either `true` or `false`
- Strings are always encapsulated in quotes. Quotes in the string are escaped 
	with a backslash, backslashes are escaped with a double-backslash, and 
	whitespace for formatting is escaped with a backslash. as an example:
```
"This is a \"sentence\" all on a single \
 line with no newline.
\
 This is on a new line."
```
- Floating point may have an exact representation. This may be done by 
  Base64-encoding or hex-encoding the value and putting it in parentheses next 
	to the human-readable value. As an example: 
	`-234.01234e4(0xC141_DA8D_B333_3333)` shows the value encoded as a 
	double-precision floating-point word.
- Binary, Hash, Identity, Signature, and Lockbox types are never written 
  directly. They must always be written in the `<type(name)>` form, with the 
	option to embed the binary data instead of a name.
- Arrays are displayed by enclosing the array of values in brackets and 
  separating each with commas. Example: `[ value0, value1, value2 ]`.
- For objects, each field-value pair are separated by a colon, with the 
  field-value pairs separated by commas and enclosed in curly brackets. Example:
```
{
	field0: value0,
	field1: value1,
	field2: value2
}
```
- Timestamps are displayed as `<timestamp(name)>`. Name may also be the direct 
  binary representation of the data within the `ext` type, or it may be a 
	human-readable timestamp, preferrably one meeting ISO 8601.

Documents, entries, and queries may be differentiated by prepending the 
MessagePack object with either `document: `, `entry: `, or `query: `, with the 
option to name each in parentheses before the colon.

## Examples ##

Certificate Document, Certificate Entry, and query for an entry:

```
document(ID user cert): [
	{
		$schema: <hash(cert scheme)>,
		id: <identity(ID user)>
	}
]

entry: [
	[
		<hash(ID user cert)>,
		"cert",
		{
			begin: <timestamp>,
			end: <timestamp>,
			name: <string>,
			value: <integer>,
			id: <hash(ID signer cert)>
		}
	],
	<signature(ID signer)>
]

query: {
	root: [ <hash(ID user cert)> ],
	query: {
		cert: {
			begin: { $gte: <timestamp(now)> },
			end: { $lte: <timestamp(now)> },
			name: <string>,
			$signed: { <identity(ID signer)> }
		}
	}
}

```


