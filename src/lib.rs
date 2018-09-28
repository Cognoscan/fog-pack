extern crate rmp;
extern crate rmpv;
extern crate byteorder;
use std::collections::HashMap;
use std::collections::BTreeMap;
//use std::io::Write;
use rmpv::Value;
use byteorder::WriteBytesExt;

use std::{fmt, io};
use std::error::Error;

use crypto::{Hash,Signature};

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
    Uuid,
    Hash,
    Identity,
    Signature,
    Lock,
    Key,
    LockBox
}
impl ExtType {
    fn to_i8(&self) -> i8 {
        match *self {
            ExtType::Uuid      => 1,
            ExtType::Hash      => 2,
            ExtType::Identity  => 3,
            ExtType::Signature => 4,
            ExtType::Lock      => 5,
            ExtType::Key       => 6,
            ExtType::LockBox   => 7,
        }
    }
}

pub fn encode_uuid(uuid: (u64, u64)) -> Result<Vec<u8>,EncodeError> {
    let mut enc = Vec::new();
    rmp::encode::write_ext_meta(&mut enc, 16, ExtType::Uuid.to_i8())?;
    enc.write_u64::<byteorder::BigEndian>(uuid.0)?;
    Ok(enc)
}

pub struct Query {
}

#[derive(Clone)]
pub struct Entry {
    hash: Hash,
    signatures: Vec<Signature>,
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
    use super::*;

    #[test]
    fn test_item_new() {
        let test = Item::new();
    }
}
