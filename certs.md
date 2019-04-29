# Certificates #

Certificates are entries with a special position in the system - that is, they 
have a special meaning in the context of a query, permission-setting, and 
establishing links with other nodes.

A certificate is a entry attached to a specific document containing only two 
fields: the empty string field, set to the hash of the Certificate schema, and 
the `id` field, set to the Identity that is being certified. The entry itself 
has the `cert` field, with an object having the `begin`, `end`, `name`, and 
`value` fields. the entry is then signed by the certifying Identity.

- `begin` specifies the time after which the certificate is valid.
- `end` specifies the time after which the certificate is not valid.
- `name` is a string that may be matched against when using certificates, with a 
	maximum length of 255 bytes.
- `value` is an integer that may be compared against when using certificates.

A certificate is considered valid if the current time falls after `begin` and 
before `end`. It is invalid otherwise.

## Certificate List Schema ##

```json
{
  "document(Condense-db Certificate List Schema)": [{
    "": "<Hash(Condense-db Core Schema)>",
    "name": "Condense-db Certificate List Schema",
    "required": [
      { "name": "id", "type": "Ident" }
    ],
    "entries": [
      {
        "name": "cert",
        "type": "Obj",
        "required": [
          { "name": "name",  "type": "Str", "max_len": 255 },
          { "name": "begin", "type": "Time"  },
          { "name": "end",   "type": "Time"  },
          { "name": "value", "type": "Int"   }
        ]
      }
    ]
  }]
}
```

## Querying or Permissions ##

When asking about signatures in a query, we can require a signature from a 
particular Identity or from a list of Identities. Alternately, we can require a 
signature from any Identity that has a certificate signed by a particular 
Identity or all of a set of identities. Logical operators may be used in place 
of the Identity list to construct more complex Identity requirements (see the 
[query specification](query.md)).

Finally, in place of a specific set of Identities, certificate requirements can 
be repeated, allowing for certificate chains.

## Retrieving Certificate Chains ##

A certificate chain can be incrementally fetched using queries. If a query 
response is received, but fails for just the `$signed` query operator(s), a node 
might choose to retrieve certificates from the responding node. If the 
responding node is unable to produce the needed certificates, the node might be 
treated as untrusted.

As an example, say a query is made that should return any "comment" entry signed 
by a friend of our friends:

```json
{
  "query(0)": [{
    "root": [ "<Hash(example document)>" ],
    "query": {
      "comment": {
        "$signed": {
          "name": "friend",
          "value": { "$gte": 1 },
          "signer": {
            "name": "friend",
            "value": { "$gte": 1 },
            "signer": [ "<Identity(myself)>" ]
          }
        }
      }
    }
  }]
}
```

If any returned entry fails to validate against the query, we can try to get 
certificates that show the entry is actually valid. We start by taking the 
Identities used in the failed signature, and query against each of their 
certificate lists. Say there were two Identities that failed - we calculate the 
hash of their certificate list documents, then run a query against those 
documents:

```json
{
  "query(0)": [{
    "root": [
      "<Hash(Cert List document of Unknown Identity 0)>",
      "<Hash(Cert List document of Unknown Identity 1)>"
    ],
    "query": {
      "cert": {
        "name": "friend",
        "value": { "$gte": 1 },
        "begin": { "$gte": "<Timestamp(now)>" },
        "end":   { "$lte": "<Timestamp(now)>" },
        "$signed": { 
          "name": "friend",
          "value": { "$gte": 1 },
          "signer": [ "<Identity(myself)>" ]
        }
      }
    }
  }]
}
```

If the returned certificates are signed by Identities that also fail validation 
due to signature requirements, we could repeat this process. Given that we are 
the final part of this signer chain in this example, we should probably 
terminate at this point and say validation has failed.









