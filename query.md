Query Language
==============

Queries are themselves valid MessagePack Documents, which use reserved names for 
their operators.

A query is made against a specific document or set of documents, all identified 
by their hashes. It can match against the documents themselves or any attached 
entries, and can follow hashes to other documents and match against those 
documents. A match occurs when all statements in the query evaluate to true. The 
matching documents and entries are all returned as a single result.

By default, a query has an unlimited lifetime. As long as it is active, new 
results may be returned, either because a new node with results was discovered 
or because a known node updated its database.

Given that they have unlimited lifetimes, queries can alternately be viewed as a 
sort of publish-subscribe system, where the query is a way of subscribing to a 
document and narrowing the scope of the subscription.

Allowable Query Types
---------------------

Document schema allow for specifying which fields should be queryable, and what 
types of queries are allowed. This serves as a guideline for the query maker and 
the database - the database is encouraged to plan for certain query types, and 
the query maker is encouraged to only use queries that meet the schema.

- query - Allow direct equivalence checking and checking for existance
- ord - Allow ordinal comparisons
- bit - Allow bitwise checking of an Integer or Binary type.
- array - Allow checking array contents
- signed - Allow checking signatures
- link - Allow following a hash and evaluating the matching document

Types
-----

Queries break MessagePack values down into a set of types and assign them 
specific names and numbers, given in the table below. These names should be 
considered case-sensitive. In addition, a "Short Name" may be used for the type. 
All types can be queried with equivalence operaters if the "query" property is 
set to true in the document schema. Other query operators can be enabled by 
setting the appropriate property in the document schema.

| Type      | Number      | Short Name | Possible Query Types |
| --        | --          | --         | --                   |
| Nil       | 0           | Nil        |                      |
| Boolean   | 1           | Bool       | ord                  |
| Integer   | 2           | Int        | ord, bit             |
| String    | 3           | Str        | ord                  |
| F32       | 4           | F32        | ord                  |
| F64       | 5           | F64        | ord                  |
| Binary    | 6           | Bin        | ord, bit             |
| Array     | 7           | Array      | array                |
| Object    | 8           | Obj        |                      |
| Hash      | 258=256+2   | Hash       | link                 |
| Identity  | 259=256+3   | Ident      |                      |
| Signature | 260=256+3   | Sign       | sign                 |
| Lockbox   | 261=256+3   | Lock       |                      |
| Timestamp | 511=256+255 | Time       | ord                  |

### Equivalence Operators ###

Requires that the field have the "query" property set to true in the document 
schema, or that the "ord" property be set to true.

| Name | Description    |
| --   | --             |
| $eq  | Equivalence    |
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

Requires that the field have the "ord" property set to true in the document 
schema. Operator evaluates as true depending on the ordering and the provided 
value.

All comparison operators evaluate as false if the types do not match.

| Name | Description           |
| --   | --                    |
| $gt  | Greater than          |
| $gte | Greater than or equal |
| $lt  | Less than             |
| $lte | Less than or equal    |

#### Comparisons ####

- Boolean: true is greater than false.
- Integer: standard integer ordering.
- String: simple binary comparison. Shorter strings are always less than longer 
  strings.
- Floating Point: Shall follow the total order specified by IEEE 754-2008. 
	However, it is recommended that an implementation accept when this ordering is 
	violated for NaN entries, or when postive and negative zero are treated as 
	equal, as actual implementations will likely fail to follow this.
- Binary data: simple binary comparison. Shorter byte sequences are always less 
  than longer sequences.
- Timestamp: standard integer ordering. Assume nanoseconds is zero if it is not 
  present.

### Bit Operators ###

These allow for checking specific bits, treating the field as a binary array. 
They are only capable of operating on the "Binary" annd "Integer" types.

Requires that the field have the "bit" property set to true in the document 
schema.

| Name          | Description                                                    |
| --            | --                                                             |
| $bitsAllClear | Selects bits from mask and is true if all bits are zero        |
| $bitsAllSet   | Selects bits from mask and is true if all bits are non-zero    |
| $bitsAnyClear | Selects bits from mask and is true if any bits are zero        |
| $bitsAnySet   | Selects bits from mask and is true if any bits are non-zero    |

### Logical Operators ###

These allow joining of other query operations. They may be used to override the 
default logical ANDing of query clauses.

No specific requirements are needed for these operators, as they do not operate 
directly on any fields.

| Name | Description                      |
| --   | --                               |
| $not | Inverts result of a query clause |
| $and | Logical AND of two query clauses |
| $or  | Logical OR of two query clauses  |
| $nor | Logical NOR of two query clauses |

### Field Query Operators ###

These allow for determining if a field exists, and if so, if it matches a specific 
type.

These operators require that the field have the "query" property set to true in 
the document schema.

| Name    | Description                                  |
| --      | --                                           |
| $exists | True if the provided field exists            |
| $type   | True if the field's value is a specific type |

The `$type` operator can have either an integer or a string as its value; see 
the "Types" section for the names and numbers for each type.

**The string value should be considered case-sensitive.**

### Array Operators ###

These allow for querying the properties of an array.

These operators require that the field have the "array" property set to true in 
the document schema.

| Name  | Description                                           |
| --    | --                                                    |
| $all  | True if the array contains all the elements specified |
| $size | True if the field's array is of a specific size       |

#### $all ####

Checks an array an makes sure all values listed are present in it.

```
<field>: { $all: [<value1>, <value2>, ... ] }
```

### Miscellaneous Operators ###

The below operators are special. The `$signed` operator requires that the field 
have the "sign" property set to true in the document schema. The `$link` 
operator requires that the field have the "link" property set to an array of 
Hash values.

| Name    | Description                                         |
| --      | --                                                  |
| $signed | Evaluate the digital signatures for an object       |
| $link   | Follow a Hash to a document and evaluate its fields |

#### $signed ####

Requires that the field holding the array has the "sign" property set to true in 
the document schema.

`$signed` performs signature verification beyond simple matching. It checks the 
certificate store based on the provided fields:

- `name`: String that should exactly match for a certificate
- `value`: Integer that can be compared against or matched exactly
- `id`: Either a single Identity or an array of Identities
- `$signed`: Chains to another `$signed` query

If both the `id` and `$signed` fields are present, this will evaluate as true if 
either any of the provided Identities was the signer or if the `$signed` 
sub-query evaluates to true.

As an example, say the data on a node is as below:

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

#### $link ####

Requires that the field be a Hash and have the "link" property set to true in 
the document schema. The contents of a `$link` operator is an object with fields 
to be queried in each sub-document. 

Priority
--------

By default, all matched documents are returned in no particular order. If the 
responding node should prioritize certain documents over others, a priority can 
be given by specifiying a field that is both queriable and has the "ord" 
property set to true.

```
{
	root: [<Hash>, ...],
	query: { ... },
	priority: { <field>: { <field>: ... { <field>: <target field> } .. } }
}


