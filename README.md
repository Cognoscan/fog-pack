condense_db
===========

**This README is a work-in-progress and currently documents a far-from-complete library**

Condense DB makes P2P applications easier by abstracting away the details of 
networking, data querying, identity managment, and encryption.

To a developer, it appears as a set of document databases you can query, with 
the ability to store and delete data in one database: your own. You choose who 
sees your queries and who is allowed to query what in your database. Documents 
contain modifiable lists pointing to other documents, allowing you to construct 
any kind of richly structured data graph.

Underneath the hood, there are 4 main components: the local database, the 
connection handler, the encryption ID handler, and the query handler.

- Local database: Your own personal data store, which can be made shareable with 
  other nodes
- Connection handler: Takes care of using the right protocol for the job, and 
  maintains connections with others nodes.
- Encryption ID handler: Handles all cryptography, manages private keys, and 
  keeps track of known public keys and signatures.
- Query handler: Manages all incoming and outgoing queries, and works to provide 
  reasonable management of the node.

**So what can I do with it?**

Pretty much anything. You can create a blog and share it with the world. You can 
create a messaging board or a chat room. Want to stream video or share files 
with friends? That works too. How about a chat room that only includes people 
nearby? Or maybe you thought StreetPass was cool and want to make your own. If 
you can describe your idea as sharing structured data with other people, then 
this is the library for you.

**Is this anonymous? Is it encrypted? Is it...federated?**

That's your call. You can demand documents and queries only be shared with 
friends, or that they always be anonymous (eg. via onion routing). You can share 
with anyone in the world, or only with devices on the LAN. The connection 
handler will use the appropriate protocol and network interfaces that meet your 
requirements.

**Is this another blockchain thing? When's the ICO?**

No blockchain, no cryptocurrency, no mining, full stop. This is fundamentally an 
extension of P2P file sharing systems like BitTorrent and Freenet. If you 
absolutely *need* to have a consistent global database with atomic transactions, 
you're out of luck. It's incredibly hard to do in a P2P fashion, and frankly 
most situations don't require it. Also, people are way more willing to use your 
app if they don't need to put money into it.

**Why are you doing this? Doesn't XXX protocol do the same thing?**

As near as I can tell, every P2P protocol is made with a specific end-goal in 
mind. Maybe it's recreating webpages, sharing files, sending messages, social 
networking, or something else. But they never try to solve *all* of these. And 
they always provide particular privacy or anonymity rules, never providing the 
application with the trade-offs. Even if there are trade-offs, they're made by 
having the user configure something about the protocol, not the app using it.

Really though, I just need to point to the fact that there are a lot of P2P 
projects out there, and no one has yet said: "This solves P2P communications. 
*It just works*." So until that time comes, I might as well make a go at it too.


The Database
------------

The database stores immutable structured documents that follow the messagepack 
encoding scheme. The documents all have a key-value map as the top level, with 
UTF-8 strings as keys. Attached to each document is a set of entries, some of 
which are immutable. A document and the immutable entries together form an Item.

An entry consists of a 128-bit index number, the hash of the Item it points to, 
the hash of the Item it is part of, and any associated signatures. If the entry 
an immutable part of the Item, it substitutes NULL for the hash of the Item it 
is a part of.

Picture each object in the database like this:

- Item
  - Document
    - Key: Value
    - Key: Value
    - ...
    - Key: Value
  - Entry
  - Entry
  - ...
  - Entry
- Entry
- Entry
- ...
- Entry

Together, they form a directed graph of objects pointing to other objects, with 
the Document as the node data and the Entries forming the edges of the graph.

Each Item can be optionally encrypted and signed, and is uniquely identified 
with a hash. To fully encode an Item, we take its messagepack data, prepend a 
lock (discussed in the Cryptography section) and encrypt it. The result is 
hashed and signatures are attached to it. The unique hash is calculated by 
taking the blob hash, concatenating it with all of the raw signature data, and 
taking the hash of that.

```
Encode as array:
+----------+-------+-------+-----+-------+     +------+
| Document | Entry | Entry | ... | Entry | ==> | Item |
+----------+-------+-------+-----+-------+     +------+

Directly prepend Lock & encrypt:
+------+------+     +----------------+
| Lock | Item | ==> | Encrypted Blob |
+------+------+     +----------------+

Append signatures:
+----------------+------------+
| Encrypted Blob | Signatures | ===> Fully encoded Item
+----------------+------------+
```

Each entry can be encrypted and signed as well. To fully encode an entry, we 
take both the hash it points to and the hash of its Item, prepend a lock, and 
encrypt it. The result is hashed and signatures are attached. The index is *not* 
included.

List entries can be attached to each Item.


Encoding Scheme
---------------

Documents, list entries, queries, and cryptography objects can all be encoded 
using messagepack. We use the `ext` format to encode some of these:

| ext type   | Description             |
| --         | --                      |
| -128 to -1 | Reserved by messagepack |
| 0          | Reserved                |
| 1          | 128-bit index           |
| 2          | Hash                    |
| 3          | Identity                |
| 4          | Signature               |
| 5          | Lock                    |
| 6          | Key                     |
| 7          | LockBox                 |

Cryptography in condense_db
---------------------------

We don't specify an exact encryption scheme on purpose. Crypto is hard, best 
practices change, and the details can make or break an entire security scheme. 
Instead, we pretend there are only a few primitives: hashes, locks, keys, 
identities, and signatures. Implementation details are left to a separate 
module: condense_crypto.

- A hash is a short piece of data that can be generated from a data stream. It 
  shall be effectively impossible to make two different data streams that 
  produce the same hash.
- A lock is data that can be prepended to a data stream, and the result can then 
  be encrypted. The result shall be impossible to read without a lock's 
  corresponding key. The result may provide a clue about what key is needed.
- A key is data that must be provided to the encryption module to decrypt locked 
  data. Keys are considered private and generally stored separate from the 
  database.
- An identity is data about a key that can be shared. This data is used to let 
  others create locks for the key, and to let others know if the key was used to 
  make a particular signature.
- A signature is data that can be appended to a data stream to indicate the 
  key's owner has seen and approved it. It shall be impossible to create a 
  signature without the hash of the data stream and a particular key. A 
  signature shall indicate what key was used to make it.

### What if the hashing scheme changes?

When a document or entry is created, its hashing scheme is determined. The 
document/entry then always uses that hashing scheme. If a different hashing 
scheme is used with the same underlying data, the result will be different.





## Meta-fields

- `_hash` - The hash of an Item/Entry
- `_crypto_version` - Version number of the crypto scheme used
- `_crypto_signed` - Array of Signatures that signed an Item/Entry
- `_crypto_locked` - Indicates if the Item/Entry was encrypted
- `_crypto_lock_signed` - Indicates if the Item/Entry has a signature inside 
  the locked section (only one internal signature allowed)


Query Language
--------------
Queries are made against a database and return Item hashes and entries in a 
structure conforming to the query. From the query result, a user can then go and 
fetch each Item.

Queries are a sequence of nested sub-queries:
1. Check for a Item that matches the sub-query. Return hashes of all such items
2. On that document, return all entries that match the selection list
3. Follow those entries and run the next sub-query on all Items they lead to
4. Etc.

Internally, a query consists of:
- The set of nested sub-queries
- The permission list for what nodes are allowed to see this query
- What identities should be used to sign this query
- A priority for the query

By default, the progression of querying looks like this:

1. User creates a query
2. Query is run against local database and sent out according to its permission 
	 list
3. A query will return arrays for each level of the query - Item hashes, then 
entries, Item hashes, then entries, etc.
4. The user's node fetches the matched Items as required and verifies they match
5. The user's node then verifies the entries beneath
6. And so on
7. This process continues indefinitely as long as a query remains open. In the 
process, both nodes keep track of what they have provided to avoid repeats. This 
list may be stored long-term and re-opened between nodes, but that's not the 
user's concern.

Network Concept
---------------
The network system is largely abstracted from the perspective of the developer. 
When making queries or adding to the database, they only need to know:

- Who can I share this with?
	- Specific identities, or ones that meet certain requirements on the identity 
	  graph.
- When can I share this?
	- Share only during specific times
- Where can I share this?
	- Specific geo-location based on longitude/latitude/altitude
	- Specific range:
		- direct radio/wired contact
		- P2P direct network
		- Local network
		- Global network

Identity Management
-------------------
A node can hold multiple identities and their corresponding keys. You can choose 
to sign an identity with a key and include a single key-value pair with it. This 
signature will include the time of signing and an optional expiration point. 
Multiple signatures can be made at the same time. All signatures you make should 
be considered knowledge that can be spread to anyone who knows your identity.

Put another way, you own certain public/private key pairs. You can sign the 
public keys of others and include a bit of data with it. This collectively forms 
a directed graph that you can use to specify who to share things with. It could 
also be used to specify who can make queries.














































