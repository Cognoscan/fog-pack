#![allow(dead_code)]
#![recursion_limit="500"]

#[cfg(test)]
extern crate serde_json;
#[cfg(test)]
extern crate hex;

extern crate colored;
extern crate num_traits;
extern crate constant_time_eq;
extern crate byteorder;
extern crate libsodium_sys;
extern crate libc;
//use std::io::Write;

pub mod crypto;
mod index;
mod value;
mod timestamp;
mod integer;
mod marker;
pub mod encode;

use marker::{Marker, ExtType};

pub use self::crypto::{Hash, Identity, Lockbox, CryptoError};
pub use self::index::Index;
pub use self::value::Value;
pub use self::integer::Integer;
pub use self::timestamp::Timestamp;

#[macro_use]
mod macros;


