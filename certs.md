# Certificates #

Certificates are documents with a special position in the system - that is, they 
have a special meaning in the context of a query or in permissions.

## Format ##

```
Document: {
	"$schema": <Hash(cert schema)>,
	"id": <Identity(root)>,
}, signed by <Identity(root)>

Entry: {
	"cert": {
		"id": <Identity(x)>
		"begin": <Timestamp>,
		"end": <Timestamp>,
		"name": <String>,
		"value": <Integer>
	}
}, signed by <Identity(root)>

```

Certificate List Schema is as follows:
```
Document: {
	"name": "Certificate List",
	"required": ["id"],
	"optional": ["name"],
	"properties": {
		"id": {
			"type": "identity"
		},
		"name": {
			"type": "string",
			"max_len": 255
		}
		"cert": {
			"type": "map",
			"required": ["id","begin","end","name","value"],
			"properties": {
				"id": {
					"type": "identity"
				},
				"begin": {
					"type:" "timestamp"
				},
				"end": {
					"type": "timestamp"
				},
				"name": {
					"type": "string",
					"max_len": 255
				},
				"value": {
					"type": "integer"
				},
			}
		}
	}
}
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
	root: [<Hash>],
	query: {
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
			},
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










