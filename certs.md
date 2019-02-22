# Certificates #

Certificates are entries with a special position in the system - that is, they 
have a special meaning in the context of a query, permission-setting, and 
establishing links with other nodes.

A certificate is a entry attached to a specific document containing only two 
fields: the `$schema` field, set to the hash of the Certificate schema, and the 
`id` field, set to the Identity that is being certified. The entry itself has 
the `cert` field, with an object having the `begin`, `end`, `name`, and `value` 
fields. the entry is then signed by the certifying Identity.

- `begin` specifies the time after which the certificate is valid.
- `end` specifies the time after which the certificate is not valid.
- `name` is a string that may be matched against when using certificates.
- `value` is an integer that may be compared against when using certificates.

A certificate is considered valid if the current time falls after `begin` and 
before `end`. It is invalid otherwise.

## Format ##

```
document(ID user cert): [
  {
    $schema: <hash(Schema - Certificate List)>,
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
      value: <integer>
    }
  ],
  <signature(ID signer 0)>
]


```

Certificate List Schema is as follows:
```
document(Condense-DB Certificate List Schema): [
  {
    $schema: <Hash(Condense-DB Core Schema)>,
    name: "Condense-DB Certificate List Schema",
    required: [
      { name: "id", type: "Ident" }
    ],
    optional: [
      { name: "name", type: "Str", max_len: 255 }
    ],
    entries: [
      {
        name: "cert",
        type: "Obj",
        required: [
          { name: "name",  type: "Str", max_len: 255 },
          { name: "begin", type: "Time"  },
          { name: "end",   type: "Time"  },
          { name: "value", type: "Int"   }
        ]
      }
    ]
  }
]
```

## Querying or Permissions ##

When asking about signatures in a query, we can ask:

1. Logical and/or/not of signatures

We may also accept signature whose Identity has been certified by some other 
Identity that meets our requirements. Namely, it matches a string. 


### Retrieving Certificate Chains ###

A certificate chain can be retrieved using an ordinary query. If a query 
response is received, but fails identity validation, a node might generate a 
query to retrieve a certificate chain from the responding node. It can then 
decide if the responding node has lied or not.

```
{
  root: [ <hash(ID user cert> ],
  query: {
    cert: {
      begin: { $gte: <timestamp(now)> },
      end: { $lte: <timestamp(now)> },
      name: "friend",
      link: {
        $link: {
          $schema: <Hash - cert schema>,
          cert: {
            begin: {
              $gte: <Timestamp - now>
            },
            end: {
              $lte: <Timestamp - now>
            },
            name: "friend",
            value: {
              $gt: 0
            }
            link: {
              $link: {
                $schema: <Hash - cert schema>,
                id: <Identity - received Identity>
              }
            }
          }
        }
      }
    }
  }
```


```
Document: {
  "$schema": <Hash(cert schema)>,
  "id": <Identity(child)>,
}

Entry: {
  "cert": {
    "id": <Identity(child)>
    "begin": <Timestamp>,
    "end": <Timestamp>,
    "name": <String>,
    "value": <Integer>
  }
}, signed by <Identity(root)>
```










