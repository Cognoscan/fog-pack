/*!
fog-pack builds on msgpack with a set of extensions useful for all 
structured data. The ultimate goal is to provide a single coherent approach 
to encoding data, validating it, and compressing it. Any existing data 
format can be replaced with fog-pack, without compromise.

To meet this lofty goal, it builds on msgpack by providing:

- A canonical form for all data. Given a known input, the same fog-pack 
    value will always be generated.
- Cryptographic hashes are a value type, and the hash of a fog-pack value 
    can be calculated
- Encrypted data is a value type, which may contain arbitrary data, a secret 
    key, or a private key
- Support for symmetric-key cryptography
    - Data can be encrypted using a secret key
    - Secret keys may be passed around in encrypted form
- Support for public-key cryptography.
    - Public keys are a value type
    - Data can be signed with a secret key
    - Data can be encrypted with a public key
    - Private keys may be passed around in encrypted form
- A schema format, allowing for validation of fog-pack values
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

# Schema

A [`Schema`](./struct.Schema.html) is a special type of Document that describes the format of 
other documents. It contains additional specifications for what Entries may be attached to a 
Document, and can include recommmended compression settings, even including a compression 
dictionary for zstd. Documents that use a specific schema reference it by placing the schema's hash 
in the value associated with the empty string field, like so:

```json
{
    "": "<Hash(Schema Used)>",
    "text": "Example document"
}
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

pub use self::error::{Error, Result};
pub use self::schema::Schema;
pub use self::crypto::{Hash, Identity, Lockbox, CryptoError};
pub use self::index::Index;
//pub use self::index_ref::IndexRef;
pub use self::value::{Value, ValueRef};
pub use self::integer::Integer;
pub use self::timestamp::Timestamp;
pub use self::document::Document;
pub use self::entry::Entry;
pub use self::no_schema::NoSchema;
pub use document::extract_schema_hash;

/// The maximum allowed size of a raw document, including signatures, is 1 MiB. An encoded, 
/// compressed document may be slightly larger than this, as it includes a short header, and 
/// compression can theoretically result in a slightly larger size too.
pub const MAX_DOC_SIZE: usize = 1usize << 20; // 1 MiB
/// The maximum allowed size of a raw entry, including signatures, is 64 kiB. An encoded, 
/// compressed entry may be slightly larger than this, as it includes a short header, and 
/// compression can theoretically result in a slightly larger size too. This does not include the 
/// size of the parent hash or the field for the entry.
pub const MAX_ENTRY_SIZE: usize = 1usize << 16; // 64 kiB

