use {Hash, Entry, Value};
use encode;

#[derive(Clone, Copy)]
pub enum QueryOrder {
    Random,
    LowestFirst,
    HighestFirst,
}

#[derive(Clone)]
pub struct Query {
   reference: Option<Hash>,
   root: Vec<Hash>,
   priority: Option<Vec<String>>,
   order: QueryOrder
}

impl Query {
    pub fn new() -> Query {
        Query {
            reference: None,
            root: Vec::new(),
            priority: Some(Vec::new()),
            order: QueryOrder::Random
        }
    }

    pub fn set_ref(&mut self, hash: &Hash) {
        self.reference = Some(hash.clone());
    }

    pub fn add_root(&mut self, root: &Hash) {
        self.root.push(root.clone());
    }

    pub fn set_priority(&mut self, priority: Vec<String>) {
        self.priority = Some(priority);
    }

    pub fn get_reference(&self) -> Option<Hash> {
        self.reference.clone()
    }

    pub fn root_iter(&self) -> std::slice::Iter<Hash> {
        self.root.iter()
    }
}

/// Encode an Entry for later decoding into a query.
///
/// An [`Entry`] can be used to describe a query to be made against a [`Document`], where the 
/// Entry's parent document is the document to be queried, and the field is the specific entry type 
/// to be queried for.
///
/// ['Entry`]: ./struct.Entry.html
/// ['Document`]: ./struct.Document.html
pub fn encode_query(entry: Entry) -> Vec<u8> {
    let mut buf = Vec::new();
    entry.doc_hash().encode(&mut buf);
    encode::write_value(&mut buf, &Value::from(entry.field()));
    buf.extend_from_slice(entry.raw_entry());
    buf
}

