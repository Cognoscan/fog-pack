Design Decisions
================

One can come up with all kinds of data-sharing methods, so how did Condense-DB 
end up looking the way it does, and what goals informed its design decisions? 
This document is a collection of some of the guiding decisions that led to 
Condense-DB's current specification.

Broad Goals
-----------

Condense-DB aims to fit nearly all network data-sharing and data storage 
applications. However, it aims to do so while assuming all network nodes are 
homogenous; that is, there is no inherent concept of client, server, or trusted 
authority in the system. It also aims to allow multiple levels of security and 
networking scale.


Maximum Sizes for Documents/Entries/Queries
-------------------------------------------

There will be no actual maximum size. Instead, we'll pick a default maximum size 
of 4 MiB for documents, 64 KiB for entries and queries. It's expected that most 
nodes on the network will enforce these sizes. You should think long and hard 
about why you'd want to push this limit up. If your data is being stored as a 
raw binary type in msgpack, it can be easily sharded and should be. If your data 
is structured msgpack, you are likely better off splitting it up along some 
smaller boundaries. Remember, your user is frequently on a mobile phone and 
cares very much about resource utilization; don't expect they can keep a 100 MiB
file in RAM without running into trouble.

The question then is: why 4 MiB and why 64 KiB? We went looking for examples of 
large structured data, something that would be difficult to chunk into separate 
documents. We also looked at a number of document databases, what their maximum 
document size is, and what led to that maximum size. Those notes are below.

In general, all structured data we could find was amenable to chunking. Any 
large data was part of a mutable structure, which in Condense-DB means it is 
best expressed as entries in a document. Sometimes large immutable data sets 
would show up, but again, these generally consisted of long lists of objects, or 
other structures that are easy to chunk.

### MongoDB

MongoDB started with a 4 MiB document limit and moved up to 16 MiB after 
[complaints][mongodb-complaints]. All of these complaints seem to be variations 
on one of two problems.

The first is where the user is storing large binary data blobs and is hitting 
the limit. They then complain that they need to use both GridFS and their 
original document mechanism. Instead, they should've used GridFS from the start. 
Storing binary blobs as large individual documents gives no benefit over 
sharding the blob with GridFS.

The second is where the user has very large structured data files. These seem 
more valid at first glance: one user wanted to store large web pages, and
another wanted to store large XML files. If they only wanted them in the 
original encoding, they could've used GridFS and been done with it. But if they 
switched to BSON encoding they might have a problem - how could they shard these 
large documents? Is the any obvious way to shard them at all? As long as the 
document is an immutable piece of structured data (not, say, comments or other 
mutable things), this could be an issue.

The XML document user was using something called NIEM formatted data. This 
appears to grow large when there are many records of a same type - essentially a 
large number of "embedded documents", in MongoDB parlance. This is is a 
non-issue for Condense-DB, as we'd split them into many smaller documents.

The HTML document user is probably storing everything in a webpage and running 
up against issues with storing the whole page, images and all. 90th percentile 
of HTML pages is [around 168 kB in 2019](httpsL//httparchive.org/reports/page-weight).

So, there's no real issue here. It seems that it's largely due to users building 
large single documents, when what they should have is slightly more complex code 
using GridFS or collections of smaller documents.

[mongodb-complaints]: <https://jira.mongodb.org/browse/server-431&usg=AOvVaw1pr1vBdbaO_K0d8bh64N2n>

### DocumentDB

Uses the same limits as MongoDB because emulating MongoDB is a primary use case.

### CouchDB

CouchDB has a configurable size limit for document size. Some places seem to 
limit it to 1 MB, but the database limit is 4 GB, gated by maximum RAM in the 
system. There's heresay about large files becoming a major problem, but nothing 
concrete.

Overhead for Binary Data
------------------------

Maximum sizes are desireable in order to limit the amount of poisoned data that 
can be received from any individual node. On the other hand, the smaller the 
maximum size, the more overhead must be spent on splitting up large documents. 
It also means that data ideally expressed as a single large document must be 
divided up somehow, which can be especially inconvenient for chunks. There's no 
ideal maximum size; instead, one must balance how much junk data is acceptable 
vs. how much overhead is acceptable.

First: Each Hash in msgpack is 65+3 = 68 bytes.

Maximum size effectively sets a minimum percentage of overhead. Applications 
concerned about overhead for raw data transfer generally come down to either 
being a raw file, or are a data stream. Of the two, data streams will be the 
worser, as they require an entry and a data file. 

We wanted to target an overhead of 0.1% for binary documents linked in an entry. 
Assuming the document contained a schema and a single letter field for the data, 
it would have an overhead of:

| Name             | Size |
| --               | --   |
| Object header    | 1    |
| Schema           | 69   |
| Data field = 'd' | 2    |
| *Total*          | *72* |

Worst case for an entry containing the hash and a simple 32-bit sequence number 
is:

| Name             | Size  |
| --               | --    |
| Parent Hash      | 68    |
| Field = 'd'      | 2     |
| Object           | 1     |
| Seq field = 't'  | 2     |
| Sequence #       | 5     |
| Link field = 'd' | 2     |
| Link Hash        | 68    |
| *Total*          | *148* |

The total overhead is thus 220 bytes, and at 0.1% that comes to 220 kB. So as 
long as documents can be larger than 220 kB, overhead on binary data isn't bad 
at all.


