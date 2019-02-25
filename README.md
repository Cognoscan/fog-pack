condense-db
===========

**This README is a work-in-progress and currently documents a far-from-complete library**

Condense DB makes P2P applications easier by abstracting away the details of 
networking, data querying, identity managment, and encryption.

[Read the specification.](spec.md)

To a developer, condense-db appears as a set of document databases you can 
query, with the ability to store and delete data in one database: your own. You 
choose who sees your queries and who is allowed to query what in your database. 
Each immutable document can have have additional data attached to it, and all 
documents can be uniquely identified by their cryptographic hash. The attached 
data, or "entries", are the mutable portion of the database.

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
many situations don't require it. Also, people are way more willing to use your 
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

