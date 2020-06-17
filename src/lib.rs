/*!
fog-pack builds on msgpack with a set of extensions useful for all 
structured data. The ultimate goal is to provide a single coherent approach 
to encoding data, validating it, and compressing it. Any existing data 
format can be replaced with fog-pack, without compromise.

To meet this lofty goal, it builds on msgpack by providing:

- A canonical form for all data. Given a known input, the same fog-pack 
    value will always be generated.
- Cryptographic hashes are a value type, and the hash of a fog-pack value 
    can be calculated.
- Encrypted data is a value type, which may contain arbitrary data, a secret 
    key, or a private key.
- Support for symmetric-key cryptography.
    - Data can be encrypted using a secret key
    - Secret keys may be passed around in encrypted form
- Support for public-key cryptography.
    - Public keys are a value type
    - Data can be signed with a secret key
    - Data can be encrypted with a public key
    - Private keys may be passed around in encrypted form
- A schema format, allowing for validation of fog-pack values.
    - Specifies subsets of possible values
    - Schema may be used to filter fog-pack values, allowing them to be used 
        as a query against a database of values
    - Schema are themselves fog-pack objects
- Immutable Documents, consisting of a fog-pack object with an optional 
    schema reference, and identifiable by their cryptographic Hash.
- Entries, consisting of a fog-pack object, a key string, and the hash of a 
    fog-pack Document. These may be used to form mutable links between 
    documents.
- Support for compression. A document or entry may be compressed after 
    encoding & hashing. Dictionary compression of values is supported if a 
    schema is used, allowing for exceptionally fast compression and high ratios. See 
    [`zstd`](https://facebook.github.io/zstd/) for more information on the compression used.

# Documents & Entries

fog-pack defines two core ways of working with encoded values: a 
[`Document`](./struct.Document.html) and an [`Entry`](./struct.Entry.html). Documents contain a 
single encoded Object, and may have an optional associated Schema (see below). Entries have an 
associated parent Document and string, and may contain any type of encoded value. Together, these 
allow for the description of immutable data with small mutable data linking them together. In general:

- Documents contain structured immutable data, like files, content, data records, etc.
- Entries contain small mutable data, like links to Documents, temporary data, small status 
    updates, or small ephemeral records.

Both of these are purposely size-limited; Documents are limited to 1 MiB decompressed, and Entries 
are limited to 54 kiB decompressed. Large files and structures can be easily created by making 
Document trees, using their hashes to connect them.

# Schema

A [`Schema`](./struct.Schema.html) is a special type of Document that describes the format of 
other documents. It contains additional specifications for what Entries may be attached to a 
Document, and can include recommmended compression settings, even including a compression 
dictionary for zstd. Documents that use a specific schema reference it by placing the schema's hash 
in the value associated with the empty string field, like so:

```text
{
    "": "<Hash(Schema Used)>",
    "text": "Example document"
}
```

# Query

A [`Query`] is a special type of Entry that describes a filter for other Entries. They are created 
as an Entry, then are encoded using [`encode_query`]. When decoded with a Schema, they can be used 
to check Entries and determine if each one matches the Query.

[`Query`]: ./struct.Query.html
[`encode_query`]: ./fn.encode_query.html

## Examples

First, include fog-pack in your Cargo.toml: 

```toml
[dependencies]
fog-pack = "0.1.0"
```

Before anything else, we must initialize the underlying crypto library:

```
# use fog_pack::*;
crypto::init();
```

Generally, a schema is the first thing you'll want to make. This specifies the 
format of all our immutable documents, along with the entries attached to them:

```
# use fog_pack::*;
// Create a simple schema for streaming text posts
let schema_doc = Document::new(fogpack!({
    "req": {
        "name": { "type": "Str" },
    },
    "entries": {
        "post": {
            "type": "Obj",
            "req": {
                "text": { "type": "Str" },
                "time": { "type": "Time", "query": true, "ord": true}
            }
        }
    }
})).unwrap();
let mut schema = Schema::from_doc(schema_doc).unwrap();
```

With a schema in place, we can create documents that adhere to them, and entries 
to attach to those documents:

```rust
# use fog_pack::*;
# use std::time::SystemTime;
# // Create a simple schema for streaming text posts
# let schema_doc = Document::new(fogpack!({
#     "req": {
#         "name": { "type": "Str" },
#     },
#     "entries": {
#         "post": {
#             "type": "Obj",
#             "req": {
#                 "text": { "type": "Str" },
#                 "time": { "type": "Time", "query": true, "ord": true}
#             }
#         }
#     }
# })).unwrap();
# let mut schema = Schema::from_doc(schema_doc).unwrap();
#
// Create a new text post document
let mut my_posts = Document::new(fogpack!({
    "": Value::from(schema.hash().clone()),
    "name": "My Text Posts",
})).unwrap();

// Make our first post
let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
let mut first_post = Entry::new(
    my_posts.hash().clone(), 
    "post".to_string(),
    fogpack!({
        "text": "This is my very first post.",
        "time": Timestamp::from_sec(now.as_secs() as i64)
    })
).unwrap();
```

Entries are encoded fog-pack with an associated document and string field. They 
let us attach changing data to an immutable document, including links between 
documents.

Both documents and entries can be crytographically signed. This requires having 
a key vault in place, along with a key:

```rust
# use fog_pack::*;
# use std::time::SystemTime;
# // Create a simple schema for streaming text posts
# let schema_doc = Document::new(fogpack!({
#     "req": {
#         "name": { "type": "Str" },
#     },
#     "entries": {
#         "post": {
#             "type": "Obj",
#             "req": {
#                 "text": { "type": "Str" },
#                 "time": { "type": "Time", "query": true, "ord": true}
#             }
#         }
#     }
# })).unwrap();
# let mut schema = Schema::from_doc(schema_doc).unwrap();
#
# // Create a new text post document
# let mut my_posts = Document::new(fogpack!({
#     "": Value::from(schema.hash().clone()),
#     "name": "My Text Posts",
# })).unwrap();
# 
# // Make our first post
# let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
# let mut first_post = Entry::new(
#     my_posts.hash().clone(), 
#     "post".to_string(),
#     fogpack!({
#         "text": "This is my very first post.",
#         "time": Timestamp::from_sec(now.as_secs() as i64)
#     })
# ).unwrap();
#
// Create a Vault for storing our Identity,
// which we'll use to sign posts.
let mut vault = crypto::Vault::new_from_password(
    crypto::PasswordLevel::Interactive,
    "Not a good password".to_string()
).unwrap();
let my_key = vault.new_key();

my_posts.sign(&vault, &my_key).unwrap();
first_post.sign(&vault, &my_key).unwrap();
```

Both documents and entries go through a schema to be encoded; this 
lets them be validated and optionally compressed:

```rust
# use fog_pack::*;
# use std::time::SystemTime;
# // Create a simple schema for streaming text posts
# let schema_doc = Document::new(fogpack!({
#     "req": {
#         "name": { "type": "Str" },
#     },
#     "entries": {
#         "post": {
#             "type": "Obj",
#             "req": {
#                 "text": { "type": "Str" },
#                 "time": { "type": "Time", "query": true, "ord": true}
#             }
#         }
#     }
# })).unwrap();
# let mut schema = Schema::from_doc(schema_doc).unwrap();
#
# // Create a new text post document
# let mut my_posts = Document::new(fogpack!({
#     "": Value::from(schema.hash().clone()),
#     "name": "My Text Posts",
# })).unwrap();
# 
# // Make our first post
# let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
# let mut first_post = Entry::new(
#     my_posts.hash().clone(), 
#     "post".to_string(),
#     fogpack!({
#         "text": "This is my very first post.",
#         "time": Timestamp::from_sec(now.as_secs() as i64)
#     })
# ).unwrap();
#
let encoded_my_posts = schema.encode_doc(my_posts).unwrap();
let first_post_checklist = schema.encode_entry(first_post).unwrap();
let encoded_first_post = first_post_checklist.complete().unwrap();
```

Entries may require additional validation with documents they link to, but in 
this case, we don't need to do any additional validation and can retrieve the 
encoded entry right away.

Finally, where the schema allows it, we can make queries that will match against 
these entries:

```rust
# use fog_pack::*;
# use std::time::SystemTime;
# // Create a simple schema for streaming text posts
# let schema_doc = Document::new(fogpack!({
#     "req": {
#         "name": { "type": "Str" },
#     },
#     "entries": {
#         "post": {
#             "type": "Obj",
#             "req": {
#                 "text": { "type": "Str" },
#                 "time": { "type": "Time", "query": true, "ord": true}
#             }
#         }
#     }
# })).unwrap();
# let mut schema = Schema::from_doc(schema_doc).unwrap();
#
# // Create a new text post document
# let mut my_posts = Document::new(fogpack!({
#     "": Value::from(schema.hash().clone()),
#     "name": "My Text Posts",
# })).unwrap();
# 
# let encoded_my_posts = schema.encode_doc(my_posts).unwrap();
# let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
#
// We can create a query to use for picking posts within a time window
let my_posts_hash = extract_schema_hash(&encoded_my_posts[..]).unwrap().unwrap();
let query_last_day = Entry::new(
    my_posts_hash,
    "post".to_string(),
    fogpack!({
        "type": "Obj",
        "unknown_ok": true,
        "time": {
            "type": "Time",
            "min": (now.as_secs() as i64) - (24*60*60),
            "max": now.as_secs() as i64,
        }
    })
).unwrap();
let query_last_day = encode_query(query_last_day);
```

*/

#![allow(dead_code)]
#![recursion_limit="500"]

#[cfg(test)]
extern crate serde_json;
#[cfg(test)]
extern crate hex;
#[cfg(test)]
extern crate rand;
#[cfg(test)]
extern crate colored;

extern crate zstd_safe;
extern crate num_traits;
extern crate constant_time_eq;
extern crate byteorder;
extern crate libsodium_sys;
extern crate libc;
extern crate regex;
extern crate ieee754;
extern crate bytecount;
extern crate unicode_normalization;
//use std::io::Write;

#[macro_use]
mod macros;

mod index;
//mod index_ref;
mod value;
mod timestamp;
mod integer;
mod marker;
mod document;
mod entry;
mod schema;
mod validator;
mod query;
mod compress_type;
mod no_schema;
mod zstd_help;
mod error;
mod encode;
mod decode;

pub mod crypto;
pub mod checklist;
pub mod spec;

use marker::{Marker, ExtType, MarkerType};
use compress_type::CompressType;

pub use error::{Error, Result};
pub use crypto::{Hash, Identity, Lockbox, CryptoError};
pub use index::Index;
//pub use self::index_ref::IndexRef;
pub use value::{Value, ValueRef};
pub use integer::Integer;
pub use timestamp::Timestamp;
pub use document::Document;
pub use entry::Entry;
pub use entry::train_entry_dict;
pub use no_schema::NoSchema;
pub use schema::Schema;
pub use query::Query;
pub use document::{extract_schema_hash, train_doc_dict};
pub use query::encode_query;

/// The maximum allowed size of a raw document, including signatures, is 1 MiB. No encoded document 
/// will ever be equal to or larger than this size.
pub const MAX_DOC_SIZE: usize = 1usize << 20; // 1 MiB
/// The maximum allowed size of a raw entry, including signatures, is 64 kiB. No encoded entry will 
/// ever be equal to or larger than this size. This does not include the size of the parent hash or 
/// the field for the entry.
pub const MAX_ENTRY_SIZE: usize = 1usize << 16; // 64 kiB

