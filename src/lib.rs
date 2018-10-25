#[cfg(test)]
extern crate serde_json;
#[cfg(test)]
extern crate hex;

extern crate constant_time_eq;
extern crate rmp;
extern crate rmpv;
extern crate byteorder;
extern crate libsodium_sys;
extern crate libc;
use std::collections::HashMap;
use std::collections::BTreeMap;
//use std::io::Write;
use rmpv::Value;
use byteorder::{WriteBytesExt, ReadBytesExt};

use std::{fmt, io};
use std::error::Error;

use crypto::Hash;

pub mod crypto;

#[derive(Debug)]
pub enum EncodeError {
    ValueWriteError(rmp::encode::ValueWriteError),
    Io(io::Error),
    Crypto(crypto::CryptoError),
}
impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            EncodeError::ValueWriteError(ref err) => err.fmt(f),
            EncodeError::Io(ref err) => err.fmt(f),
            EncodeError::Crypto(ref err) => err.fmt(f),
        }
    }
}

impl Error for EncodeError {
    fn description(&self) -> &str {
        match *self {
            EncodeError::ValueWriteError(ref err) => err.description(),
            EncodeError::Io(ref err) => err.description(),
            EncodeError::Crypto(ref err) => err.description(),
        }
    }
}

impl From<rmp::encode::ValueWriteError> for EncodeError {
    fn from(err: rmp::encode::ValueWriteError) -> EncodeError {
        EncodeError::ValueWriteError(err)
    }
}
impl From<io::Error> for EncodeError {
    fn from(err: io::Error) -> EncodeError {
        EncodeError::Io(err)
    }
}
impl From<crypto::CryptoError> for EncodeError {
    fn from(err: crypto::CryptoError) -> EncodeError {
        EncodeError::Crypto(err)
    }
}

// An Item contains a document and a list. The document is a key-value store where the keys are 
// strings and the values are arbitrary msgpack objects. The list is a key-value store where the 
// keys are 128-bit unsigned integers and the values are hashes pointing to other Items.
//
// 
// Two formats: mutable and immutable:
// - Immutable
//      - Hash is from encrypting the entire item, hashing it, attaching signatures, and 
//        hashing the concatenation of the original hash and the attached signatures.
// - Mutable
//      - Hash is from encrypting the document, hashing it, attaching signatures, and hashing 
//        the concatenation of the original hash and the attached signatures.
//      - List entries are provided separately. They consist of the Item hash, the key number, and 
//        the hash value it stores. The Item hash is encrypted, then the entire entry is hashed, 
//        and signatures are attached. 

pub enum ExtType {
    Index,
    Hash,
    Identity,
    Signature,
    LockBox,
}
impl ExtType {
    fn to_i8(&self) -> i8 {
        match *self {
            ExtType::Index     => 1,
            ExtType::Hash      => 2,
            ExtType::Identity  => 3,
            ExtType::Signature => 4,
            ExtType::LockBox   => 5,
        }
    }
}

pub struct Index (u64,u64);

impl Index {
    pub fn encode(&self) -> Value {
        let mut enc = Vec::new();
        if self.1 == 0 {
            if self.0 < 256 {
                enc.write_u8(self.0 as u8).unwrap();
            }
            else if self.0 < 65536 {
                enc.write_u16::<byteorder::BigEndian>(self.0 as u16).unwrap();
            }
            else if self.0 < 4294967295 {
                enc.write_u32::<byteorder::BigEndian>(self.0 as u32).unwrap();
            }
            else {
                enc.write_u64::<byteorder::BigEndian>(self.0).unwrap();
            }
        }
        else {
            enc.write_u64::<byteorder::BigEndian>(self.1).unwrap();
            enc.write_u64::<byteorder::BigEndian>(self.0).unwrap();
        }
        Value::Ext(ExtType::Index.to_i8(), enc)
    }

    pub fn valid_ext(ext: i8, len: usize) -> bool {
        ext == ExtType::Index.to_i8() &&
            (len == 1 || len == 2 || len == 4 || len == 8 || len ==16)
    }


    pub fn decode(v: &Value) -> Option<Index> {
        match v {
            Value::Ext(ext, ref data) => {
                let len = data.len();
                if *ext == ExtType::Index.to_i8() {
                    if len == 1 {
                        let mut r = &data[..];
                        Some(Index (0, r.read_u8().unwrap_or(0u8) as u64))
                    }
                    else if len == 2 {
                        let mut r = &data[..];
                        Some(Index (0, r.read_u16::<byteorder::BigEndian>().unwrap_or(0u16) as u64))
                    }
                    else if len == 4 {
                        let mut r = &data[..];
                        Some(Index (0, r.read_u32::<byteorder::BigEndian>().unwrap_or(0u32) as u64))
                    }
                    else if len == 8 {
                        let mut r = &data[..];
                        Some(Index (0, r.read_u64::<byteorder::BigEndian>().unwrap_or(0u64)))
                    }
                    else if len == 16 {
                        let mut r = &data[..];
                        let v1 = r.read_u64::<byteorder::BigEndian>().unwrap_or(0u64);
                        let v0 = r.read_u64::<byteorder::BigEndian>().unwrap_or(0u64);
                        Some(Index (v1, v0))
                    }
                    else {
                        None
                    }
                }
                else {
                    None
                }
            },
            _ => None,
        }
    }
}

pub struct Query {
}

#[derive(Clone)]
pub struct Entry {
    hash: Hash,
}
impl Entry {
    pub fn encode(self, _key: (u64, u64)) -> Vec<u8> {
        unimplemented!();
        /*
        let mut enc = Vec::<u8>::new();
        // Format: fixarray [keypair, Identity, hash, fixarray [signatues]]
        rmp::encode::write_array_len(&mut enc, 3);
        rmp::encode::write_array_len(&mut enc, self.signatures.len() as u32);
        enc
        */
    }
}

#[derive(Clone)]
pub struct Item {
    document: HashMap<String, Value>,
    list: BTreeMap<(u64, u64), Entry>,
}
impl Item {
    pub fn new() -> Item {
        Item {
            document: HashMap::new(), 
            list: BTreeMap::new()
        }
    }
    pub fn add_entry(&mut self, key: (u64,u64), entry: Entry) -> Option<Entry> {
        self.list.insert(key, entry)
    }
}

// There is only one database, which holds every Item we know about
pub struct Database {
    items: HashMap<crypto::Hash, Item>,
}
impl Database {
    /// True on successful add, false if the item is already present
    pub fn add_item(&mut self, hash: Hash, item: Item) -> bool {
        if self.items.contains_key(&hash) {
            false
        } else {
            self.items.insert(hash, item);
            true
        }
    }

    pub fn del_item(&mut self, hash: &Hash) -> bool {
        if self.items.contains_key(hash) {
            self.items.remove(hash);
            true
        } else {
            false
        }
    }

    pub fn get_item(&self, hash: &Hash) -> Option<Item> {
        match self.items.get(hash) {
            Some(item) => Some(item.clone()),
            None => None
        }
    }
    
    pub fn add_entry(&mut self, item_hash: &Hash, key: (u64, u64), entry: Entry) -> Option<Entry> {
        match self.items.get_mut(item_hash) {
            Some(item) => item.add_entry(key, entry),
            None => None
        }
    }

    pub fn run_item_query(self, _query: Query) -> Vec<Item> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    //use super::*;

    #[test]
    fn test_item_new() {
        //let test = Item::new();
    }
}
