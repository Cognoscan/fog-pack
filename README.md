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
- `_crypto_
