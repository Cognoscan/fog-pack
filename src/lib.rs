//! fog-pack builds on msgpack with a set of extensions useful for all structured 
//! data. The ultimate goal is to provide a single coherent approach to encoding 
//! data, validating it, and compressing it. Any existing data format should be 
//! replaceable with fog-pack, without compromise.
//! 
//! To meet this lofty goal, it extends msg-pack by providing:
//! 
//! - A canonical form for all data. Given a known input, the same fog-pack value 
//! 	will always be generated
//! - Cryptographic hashes are a value type, and the hash of a fog-pack value can 
//! 	be calculated
//! - Encrypted data is a value type, which may contain arbitrary data, a secret 
//! 	key, or a private key
//! - Support for symmetric-key cryptography
//! 	- Data can be encrypted using a secret key
//! 	- Secret keys may be passed around in encrypted form
//! - Support for public-key cryptography.
//! 	- Public keys are a value type
//! 	- Data can be signed with a secret key
//! 	- Data can be encrypted with a public key
//! 	- Private keys may be passed around in encrypted form
//! - A schema format, allowing for validation of fog-pack values
//! 	- Specifies subsets of possible values
//! 	- Schema may be used to filter fog-pack values, allowing them to be used as a 
//! 		query against a database of values
//! 	- Schema are themselves fog-pack objects
//! - Immutable Documents, consisting of a fog-pack object with an optional schema 
//! 	reference.
//! - Entries, consisting of a fog-pack object, a key string, and the hash of a 
//! 	fog-pack Document. These may be used to form mutable links between documents.
//! - Support for compression. A document or entry may be compressed after encoding 
//! 	& hashing. Dictionary compression of values is supported if a schema is used.

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

extern crate zstd;
extern crate num_traits;
extern crate constant_time_eq;
extern crate byteorder;
extern crate libsodium_sys;
extern crate libc;
extern crate regex;
extern crate ieee754;
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
mod query;
mod varint;

pub mod crypto;
pub mod encode;
pub mod decode;

use marker::{Marker, ExtType, MarkerType};

pub use self::schema::Schema;
pub use self::crypto::{Hash, Identity, Lockbox, CryptoError};
pub use self::index::Index;
//pub use self::index_ref::IndexRef;
pub use self::value::{Value, ValueRef};
pub use self::integer::Integer;
pub use self::timestamp::Timestamp;
pub use self::document::Document;
pub use self::entry::Entry;

