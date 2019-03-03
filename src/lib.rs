#![allow(dead_code)]

#[cfg(test)]
extern crate serde_json;
#[cfg(test)]
extern crate hex;

extern crate colored;
extern crate num_traits;
extern crate constant_time_eq;
extern crate rmp;
extern crate rmpv;
extern crate byteorder;
extern crate libsodium_sys;
extern crate libc;
//use std::io::Write;

pub mod crypto;

pub use self::crypto::{Value,Integer,Timestamp,Hash,Identity,CryptoError};

#[macro_use]
mod macros;


