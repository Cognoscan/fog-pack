# Query Language #

Queries are themselves valid MessagePack Documents, which use reserved names for 
their operators.

A query is made against a specific document or set of documents, all identified 
by their hashes. It can match against the documents themselves or any attached 
entries, and can follow hashes to other documents and match against those 
documents. A match occurs when all statements in the query evaluate to true. The 
matching documents and entries are all returned as a single result.

## Allowable Query Types ##

- query - Allow direct equivalence checking and checking for existance
- ordinal - Allow ordinal comparisons
- bit - Allow bitwise checking of an Integer
- array - Allow checking array contents
- signed - Allow checking signatures

## Types ##

Queries break MessagePack values down into a set of types and assign them 
specific names and numbers, given in the table below. These names should be 
considered case-sensitive.

| Type      | Number      |
| --        | --          |
| null      | 0           |
| boolean   | 1           |
| integer   | 2           |
| string    | 3           |
| floating  | 4           |
| binary    | 5           |
| array     | 6           |
| object    | 7           |
| hash      | 258=256+2   |
| identity  | 259=256+3   |
| signature | 260=256+3   |
| lockbox   | 261=256+3   |
| timestamp | 511=256+255 |

### Equivalence Operators ###

Requires only that the field be queriable:

| Name | Description    |
| --   | --             |
| $eq  | Equiavalence   |
| $ne  | Not equivalent |
| $in  | In set         |
| $nin | Not in set     |

#### $eq ####

`$eq` is valid for all types and looks for exact equivalence. It is identical to 
providing a value for a field directly in a query operator:

```
<field>: { $eq: <value> }

same as

<field>: <value>
```

If the document has a matching field whose value matches exactly, this evaluates 
as true. If the value type differs, this evaluates as false.

#### $ne ####

`$neq` is valid for all types and looks for exact equivalence. If the document 
has a matching field whose value does not match exactly, this evaluates as true. 
If the value does match, or the value type differs, this evaluates as false.

#### $in ####

`$in` is identical to a sequence of `$eq` comparisons logically OR'd together. 
It takes an array as its value, and the values in the array can be of mixed 
type.

```
<field>: { $in: [<value1>, <value2>, ... ] }
```

#### $nin ####

`$nin` is identical to a series of `$neq` comparisons logically AND'd together. 
It takes an array as its value, and the values in the array can be of mixed 
type.

```
<field>: { $nin: [<value1>, <value2>, ... ] }
```

### Comparison Operators ###

Requires that the field be queriable and marked as "ordinal". Operator evaluates 
as true depending on the ordering and the provided value.

All comparison operators evaluate as false if the types do not match.

| Name | Description           |
| --   | --                    |
| $gt  | Greater than          |
| $gte | Greater than or equal |
| $lt  | Less than             |
| $lte | Less than or equal    |

#### Comparisons ####

- Null: always evaluates as true, regardless of comparison, regardless of 
  comparison
- Boolean: true is greater than false.
- Integer: standard integer ordering.
- String: simple binary comparison. Shorter strings are always less than longer 
  strings.
- Floating Point: Should follow the total order specified by IEEE 754-2008. 
  However, it is recommended that an implementation accept when this ordering is 
	violated for NaN entries, or when postive and negative zero are treated as 
	equal.
- Binary data: simple binary comparison. Shorter byte sequences are always less 
  than longer sequences.
- Array, Object, Hash, Identity, Signature, Lockbox: Always evaluate as false.
- Timestamp: standard integer ordering. Assume nanoseconds is zero if it is not 
  present.

### Logical Operators ###

These allow joining of other query operations. They may be used to override the 
default logical ANDing of query clauses.

| Name | Description                      |
| --   | --                               |
| $not | Inverts result of a query clause |
| $and | Logical AND of two query clauses |
| $or  | Logical OR of two query clauses  |
| $nor | Logical NOR of two query clauses |

### Field Query Operators ###

These allow for determining if a field exists, and if so, if it matches a specific 
type.

| Name    | Description                                  |
| --      | --                                           |
| $exists | True if the provided field exists            |
| $type   | True if the field's value is a specific type |

The `$type` operator can have either an integer or a string as its value; see 
the "Types" section for the names and numbers for each type.

**The string value should be considered case-sensitive.**

### Array Operators ###

### Miscellaneous Operators ###

| Name    | Description                                         |
| --      | --                                                  |
| $link   | Follow a Hash to a document and query that document |
| $signed | Evaluate the digital signatures for an object       |

### $signed ###

Requires that the field holding the array be marked as "signed".

`$signed` performs signature verification beyond simple matching. It checks the 
certificate store based on the provided fields:

- `name`: String that should exactly match for a certificate
- `value`: Integer that can be compared against or matched exactly
- `id`: Either a single Identity or an array of Identities
- `$signed`: Chains to another `$signed` query

If both the `id` and `$signed` fields are present, this will evaluate as true if 
either any of the provided Identities was the signer or if the `$signed` 
sub-query evaluates to true.

#### Example ####

Say the data on a node is as below:

```
document: [
	{
		$schema: <Hash - location record-keeping schema>,
		name: "Team Work Locations",
		root: <Identity - root>
	},
	<Signature - ID root>
]

entry: [ <Hash - document>, "loc_status", { team: "Finance",     location: "Home"   }, <Signature - ID user0>]
entry: [ <Hash - document>, "loc_status", { team: "Accounting",  location: "Work"   }, <Signature - ID user1>]
entry: [ <Hash - document>, "loc_status", { team: "Sales", 		   location: "Travel" }, <Signature - ID user2>]
entry: [ <Hash - document>, "loc_status", { team: "Engineering", location: "Work"   }, <Signature - ID user3>]

```

The below will match on any array whose first element is an object with a team 
field set to "Finance" and a location field with a string value. The array must 
also contain a signature, as indicated by `$signed`. Any Identity with a valid 
"user" certificate (name="user" and value>0) from another Identity with a valid 
"admin" certificate (name="admin" and value>0) that was signed by a given 
Identity will be accepted.

```
query: {
	root: [ <Hash - document> ],
	query: {
		loc_status: {
			team: "Finance",
			location: { $type: "string" },
			$signed: {
				value: { $gt: 0 },
				name: "user",
				$signed: {
					value: { $gt: 0 },
					name: "admin"
					id: <Identity>
				}
			}
		}
	}
}
```

This will return a single entry:

```
entry: [ <Hash>, "loc_status", { team: "Finance",     location: "Home"   }, <Signature - ID user0>]
```


## Priority ##

By default, all matched documents are returned in no particular order. If the 
responding node should prioritize certain documents over others, a priority can 
be given by specifiying a field that is both queriable and has the "ordinal" 
property.

```
{
	root: [<Hash>, ...],
	query: { ... },
	priority: { <field>: { <field>: ... { <field>: <target field> } .. } }
}


