/// The underlying structure is irrelvent. Here's what we have:
///
/// Database:
/// - I have a database
/// - This database stores tables & entries
/// - Tables contain maps to other tables or contain maps to entries
/// - Tables always have a and HashMap<String,rmpv::Value>. Find them by hashing Hashmap
/// - Entries always have a HashMap<String,rmpv::Value>, and data as HashMap<u64,Vec<u8>>. Find 
///     them by hashing HashMap, and data
/// - Data is stored unencrypted unless it couldn't be decrypted
/// - Encryption info is next to each table or entry
/// - It *is* a connection handler in its own right
///
/// Queries:
/// - I can query the database & network
/// - My query selects Tables & entries that meet my hashmap format
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
/// - The "keep" attribute must have an associated name - i.e. each application must claim 
///   ownership of data it cares about.
///
extern crate rmp;
extern crate rmpv;
use std::collections::HashMap;
use std::collections::BTreeMap;

pub enum Comparison {
    ExactEq           =  0, // Is valid msgPack and encoded data match exactly
    NotExactEq        =  1, // Is valid msgPack and encoded data do not match
    Equal             =  2, // Meet PartialEq and match
    NotEqual          =  3, // Meet PartialEq and do not match
    GreaterThan       =  4, // Meet PartialOrd and provided value is greater
    LessEqual         =  5, // Meet PartialOrd and provided value is less or equal
    LessThan          =  6, // Meet PartialOrd and provided value is less
    GreaterEqual      =  7, // Meet PartialOrd and provided value is greater or equal
    Contains          =  8, // Is string and contains provided string, case-insensitive
    ContainsNot       =  9, // Is string and does not contain provided string, case-insensitive
    ContainsCase      = 10, // Is string and contains provided string, case-sensitive
    ContainsCaseNot   = 11, // Is string and does not contain provided string, case-sensitive
    StartsWith        = 12, // Is string and starts with provided string, case-insensitive
    StartsWithNot     = 13, // Is string and does not start with provided string, case-insensitive
    StartsWithCase    = 14, // Is string and starts with provided string, case-sensitive
    StartsWithCaseNot = 15, // Is string and does not start with provided string, case-sensitive
    EndsWith          = 16, // Is string and ends with provided string, case-insensitive
    EndsWithNot       = 17, // Is string and does not end with provided string, case-insensitive
    EndsWithCase      = 18, // Is string and ends with provided string, case-sensitive
    EndsWithCaseNot   = 19, // Is string and does not end with provided string, case-sensitive
}

/// Immutable parts of tables & entries
/// - A table's header never change. Thus, their encryption parameters and hashes never 
/// change
/// - The entries in a table can change
///
/// - An entry is completely immutable. Header, and data will never change.

/// Table
/// Tables hold references to Items
/// - Tables contain maps to other tables or contain maps to entries
/// - Data is stored unencrypted unless it couldn't be decrypted
/// - Encryption info is stored in the header with a special key name
pub struct Table {
    header: HashMap<String,rmpv::Value>,
    entries: HashMap<u64,Vec<u8>>
}
impl Table {
    pub fn new(header: HashMap<String,rmpv::Value>) -> Table {
        Table {
            header: header,
            entries: HashMap::new(),
        }
    }
    pub fn get_header(self) -> HashMap<String,rmpv::Value> {
        self.header
    }
    pub fn get_header_value(&self,key:&String) -> Option<&rmpv::Value> {
        self.header.get(key)
    }
    pub fn add_entry(&mut self, num:u64, hash:Vec<u8>) -> Option<Vec<u8>> {
        self.entries.insert(num, hash)
    }
    pub fn drop_entry(&mut self, num:&u64) -> Option<Vec<u8>> {
        self.entries.remove(num)
    }
    pub fn get_entry(self,num:&u64) -> Option<Vec<u8>> {
        match self.entries.get(num) {
            Some(hash) => Some(hash.clone()),
            None => None
        }
    }
}

/// Entry
/// - Entries always have a HashMap<String,rmpv::Value>, and data as HashMap<u64,Vec<u8>>. Find 
///     them by hashing HashMap, and data
/// - Data is stored unencrypted unless it couldn't be decrypted
/// - Encryption info is stored in the header with a special key name
pub struct Entry {
    header: HashMap<String,rmpv::Value>,
    data: BTreeMap<u64,Vec<u8>>,
}
impl Entry {
    pub fn new(header: HashMap<String,rmpv::Value>, data: BTreeMap<u64,Vec<u8>>) -> Entry {
        Entry {
            header: header,
            data: data,
        }
    }
    pub fn get_header(self) -> HashMap<String,rmpv::Value> {
        self.header
    }
    pub fn get_header_value(&self,key:&String) -> Option<&rmpv::Value> {
        self.header.get(key)
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
        header.insert(String::from("a"),rmpv::Value::from("A"));
        header.insert(String::from("b"),rmpv::Value::from("B"));
        header.insert(String::from("c"),rmpv::Value::from("C"));
        let table = Table::new(header);
        let hash = vec![0,1];
        db.add_table(hash,table);
        db
    }

    #[test]
    fn test_entry() {
        let mut header = HashMap::new();
        header.insert(String::from("name"), rmpv::Value::from("TestHeader"));
        let mut data = BTreeMap::new();
        data.insert(0, vec![1,2,3]);
        data.insert(1, vec![4,5,6]);
        let entry = Entry::new(header, data);
        
        let header_item = entry.get_header_value(&String::from("name")).unwrap();
        assert_eq!(header_item,  &rmpv::Value::from("TestHeader"));
        let data_item = entry.get_datum(&0).unwrap();
        assert_eq!(data_item, vec![1,2,3]);
    }

    #[test]
    fn test_database_table_contents() {
        let db = setup_simple_database();
        // Test for Table & table name
        let table = db.get_table(&vec![0,1]);
        assert!(!table.is_none());
        // Test for header
        let header_item = db.get_table(&vec![0,1]).unwrap().get_header_value(&String::from("a"));
        assert_eq!(&rmpv::Value::from("A"),header_item.unwrap());
        // Test for table entries
    }
}
