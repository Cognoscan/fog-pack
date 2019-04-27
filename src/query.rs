use super::Hash;

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
}
