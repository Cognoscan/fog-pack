/// The underlying structure is irrelvent. Here's what we have:
///
/// Database:
/// - I have a database
/// - This database stores tables & entries
/// - Tables contain maps to other tables or contain maps to entries
/// - Tables always have a name and HashMap<String,String>. Find them by hashing name&Hashmap
/// - Entries always have a name, HashMap<String,String>, and data as HashMap<u64,Vec<u8>>. Find 
///     them by hashing name,HashMap, and data
/// - Data is stored unencrypted unless it couldn't be decrypted
/// - Encryption info is next to each table or entry
/// - It *is* a connection handler in its own right
///
/// Queries:
/// - I can query the database & network
/// - My query selects Tables & entries that meet my name & hashmap format
/// - My query also has a network scope to it
/// - My query can optionally store all data to the database
///
/// Permissions:
/// - Others can query my database
/// - Permissions are set for each table & entry.
/// -   "Who's asking? Where are they asking from?"
/// -   Whitelist of acceptable people, and acceptable network scopes
///
/// Apps:
/// - Can create queries, set permissions, and get health info from the manager
///
/// Connection Handler:
/// - Accepts queries for the network scope it supports, and then queries its network in some 
///     fashion.
/// - Rate limiting, line encryption, network maintainence, friend finding...this is all handled by 
///     the connection handler itself.
/// - Can ask the manager to sign or encrypt an arbitrary piece of data, but should have no keys 
///     itself
///
/// Manager:
/// - Talks to apps
/// - Talks to connection handlers
/// - Forwards queries as appropriate
/// - Holds onto keys and does encryption/decryption
///
/// Database management:
/// - By default, throw away the oldest entries based on fetch time
/// - Applications can mark data as "keep" and the database will keep it indefinitely
/// - The "keep" attribute must have an associated name
///
extern crate rmp;
use std::collections::HashMap;
use std::collections::BTreeMap;


/// Immutable parts of tables & entries
/// - A table's name & header never change. Thus, their encryption parameters and hashes never 
/// change
/// - The entries in a table can change
///
/// - An entry is completely immutable. Header, name, and data will never change.

/// Table
/// Tables hold references to Items
/// - Tables contain maps to other tables or contain maps to entries
/// - Tables always have a name and HashMap<String,String>. Find them by hashing name&Hashmap
/// - Data is stored unencrypted unless it couldn't be decrypted
/// - Encryption info is stored in the header with a special name
pub struct Table {
    name: String,
    header: HashMap<String,String>,
    entries: HashMap<u64,Vec<u8>>
}
impl Table {
    pub fn new(name: &str, header: HashMap<String,String>) -> Table {
        Table {
            name: String::from(name),
            header: header,
            entries: HashMap::new(),
        }
    }
    pub fn get_name(&self) -> &String {
        &self.name
    }
    pub fn get_header(self) -> HashMap<String,String> {
        self.header
    }
    pub fn get_header_value(&self,key:&String) -> Option<&String> {
        self.header.get(key)
    }
    pub fn get_entry(self,num:&u64) -> Option<Vec<u8>> {
        match self.entries.get(num) {
            Some(hash) => Some(hash.clone()),
            None => None
        }
    }
}

/// Entry
/// - Entries always have a name, HashMap<String,String>, and data as HashMap<u64,Vec<u8>>. Find 
///     them by hashing name,HashMap, and data
/// - Data is stored unencrypted unless it couldn't be decrypted
/// - Encryption info is stored in the header with a special name
pub struct Entry {
    name: String,
    header: HashMap<String,String>,
    data: BTreeMap<u64,Vec<u8>>,
}
impl Entry {
    pub fn new(name: String, header: HashMap<String,String>, data: BTreeMap<u64,Vec<u8>>) -> Entry {
        Entry {
            name: name,
            header: header,
            data: data,
        }
    }
    pub fn get_name(self) -> String {
        self.name
    }
    pub fn get_header(self) -> HashMap<String,String> {
        self.header
    }
    pub fn get_datum(self, num:&u64) -> Option<Vec<u8>> {
        match self.data.get(num) {
            Some(data) => Some(data.clone()),
            None => None
        }
    }
}

pub struct Database {
    tables: HashMap<Vec<u8>,Table>,
    entries: HashMap<Vec<u8>,Entry>,
}
impl Database {
    pub fn new() -> Database {
        Database {
            tables: HashMap::new(),
            entries: HashMap::new(),
        }
    }
    pub fn add_table(&mut self, hash:Vec<u8>, table: Table) -> Option<Table> {
        self.tables.insert(hash, table)
    }
    pub fn drop_table(&mut self, hash:&Vec<u8>) -> Option<Table> {
        self.tables.remove(hash)
    }
    pub fn add_entry(&mut self, hash:Vec<u8>, entry: Entry) -> Option<Entry> {
        self.entries.insert(hash, entry)
    }
    pub fn drop_entry(&mut self, hash:&Vec<u8>) -> Option<Entry> {
        self.entries.remove(hash)
    }
    pub fn get_entry<'a>(&'a self, hash:&Vec<u8>) -> Option<&Entry> {
        self.entries.get(hash)
    }
    pub fn get_table<'a>(&'a self, hash:&Vec<u8>) -> Option<&Table> {
        self.tables.get(hash)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    fn setup_simple_database() -> Database {
        // Construct the simple database
        let mut db = Database::new();
        let mut header = HashMap::new();
        header.insert(String::from("a"),String::from("A"));
        header.insert(String::from("b"),String::from("B"));
        header.insert(String::from("c"),String::from("C"));
        let table = Table::new("test table",header);
        let hash = vec![0,1];
        db.add_table(hash,table);
        db
    }

    #[test]
    fn test_database_table_contents() {
        let db = setup_simple_database();
        // Test for Table & table name
        let table = db.get_table(&vec![0,1]);
        assert_ne!(None, table);
        assert_eq!(String::from("test table"), *(table.unwrap().get_name()));
        // Test for header
        let header_item = db.get_table(&vec![0,1]).unwrap().get_header_value(&String::from("a"));
        assert_eq!("A",header_item.unwrap());
        // Test for table entries
    }
}
