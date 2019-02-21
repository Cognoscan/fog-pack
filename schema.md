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

All documents may refer to the schema document they meet using the `$schema` 
reserved field. If left unspecified, the base schema document is used. The 
field is always an array of Hash types, each of which must be the Document Hash 
of a valid schema document.

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
considered a related query (see query specification).

Once the schema document is either in the database or cached, the received 
document is validated against it. The same goes for received entries. If a 
received entry refers to a document not received or in the database, it is held 
until all documents have been received or added to the database.

Schema Document Format
----------------------

```
document(core schema): [
	{
		name: "core",
		required: [
			{
				name: "name",
				type: "Str"
			}
		],
		optional: [
			{
				name: "required",
				type: "FieldArray"
			},
			{
				name: "optional",
				type: "FieldArray"
			},
			{
				name: "entries",
				type: "FieldArray"
			},
			{
				name: "unknown_ok",
				type: "Bool"
			},
			{
				name: "types",
				type: "TypeArray"
			}
		],
		entries: [],
		unknown_ok: false
	}
]
















