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

