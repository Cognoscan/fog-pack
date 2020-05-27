/*!
The fog-pack value structure used for Queries.

Queries are nearly identical to Entries: They have a parent document, a field 
string, and contain a fog-pack Value. The main difference is that they do not 
attach to that parent document, but instead are used to query its attached 
Entries. A query uses the same [Validation Language](../validation/index.html) 
that a Schema does.

Queries are limited in what they can look like based on the Schema used by a 
document. The fields they may use for each validator are determined by what 
fields are allowed by the matching validator in a document's schema. For 
example, say a schema looks like:

```json
{
    "req": {
        "name": { "type": "Str" },
        "owner": { "type": "Ident" }
    },
    "entries": {
        "post": {
            "type": "Obj",
            "obj_ok": true,
            "req": {
                "text": { "type": "Str" },
                "time": { "type": "Time", "ord": true, "query": true }
            }
        }
    }
}
```

A query can only be used to check entries with the "post" field string. 
Furthermore, they can only non-trivially check the "time" field in each entry's 
object. So a query checking for any "post" entries dated between January 1 and 2 
could look like:

```json
{
    "type": "Obj",
    "req": {
        "time": {
            "type": "Time",
            "min": "<Time(2020-01-01 00:00:00)>,
            "max": "<Time(2020-01-02 00:00:00)>
        },
        "text": { "type": "Str" }
    }
}
```

But checking for a specific string for text would not be allowed by the schema:

```json
{
    "type": "Obj",
    "req": {
        "time": { "type": "Time" },
        "text": { "type": "Str", "in": "test" }
    }
}
```

The above would fail, as the `query` field is not set for "text".
*/
