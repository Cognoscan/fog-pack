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

Validation is simple when adding to the local database. If the relevant schema 
documents are not in the local database, validation fails. Likewise, if a 
document or entry links to a document, and that document should meet specific 
schema, it must be in the local database and aleady has been validated. If not, 
validation fails. If these conditions are all met and the document/entry 
otherwise passes validation, it is added to the database.

Validation is different when retrieving from an external source. If the relevant 
schema documents are not in the local database, they are requested using a query 
with permissions & conditions identical to the query that returned the retrieved 
document/entry. If the schema then specifies that 
