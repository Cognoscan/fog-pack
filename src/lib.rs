extern crate rmp;
extern crate rmpv;
use std::collections::HashMap;
use std::collections::BTreeMap;
use rmpv::Value;


// Internal searchable keys:
// - All keys starting with _ are reserved for us
// - _hash stores a list of hashes of the data
// - _publicKey stores the public key that was used to encrypt the data
// - _signedBy stores a list of public keys that signed the data
// - _mutable indicates if the collection's entries can change
// You can sign unencrypted data, then encrypt it and pass it around

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
type Hash = Vec<u8>;
type PublicKey = Vec<u8>;
type Signature = Vec<u8>;

pub struct Query {
}

#[derive(Clone)]
pub struct Entry {
    hash: Hash,
    key: PublicKey,
    signed: Vec<Signature>,
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
    items: HashMap<Hash, Item>,
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
