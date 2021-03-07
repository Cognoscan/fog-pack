use std::collections::HashMap;

use super::*;
use crate::error::{Error, Result};
use crate::Hash;

#[derive(Clone, Debug)]
pub struct ListItem<'a> {
    schema: Vec<&'a [Option<Hash>]>,
    link: Vec<&'a Validator>,
}

impl<'a> ListItem<'a> {
    pub fn new() -> Self {
        Self {
            schema: Vec::new(),
            link: Vec::new(),
        }
    }

    pub fn check(&self, _parent_hash: &Hash, _data: Vec<u8>) -> Result<()> {
        todo!()
    }
}

#[derive(Clone, Debug)]
pub struct DataChecklist<'a, T> {
    list: Checklist<'a>,
    data: T,
}

impl<'a, T> DataChecklist<'a, T> {
    pub fn new(data: T) -> Self {
        Self {
            list: Checklist::new(),
            data,
        }
    }

    pub fn from_checklist(list: Checklist<'a>, data: T) -> Self {
        Self {
            list,
            data,
        }
    }

    pub fn insert(&mut self, hash: Hash, schema: Option<&'a [Option<Hash>]>, link: Option<&'a Validator>) {
        self.list.insert(hash, schema, link)
    }

    pub fn complete(self) -> Result<T> {
        self.list.complete()?;
        Ok(self.data)
    }
    
}

#[derive(Clone, Debug)]
pub struct Checklist<'a> {
    list: HashMap<Hash, ListItem<'a>>
}

impl<'a> Checklist<'a> {
    pub fn new() -> Self {
        Self {
            list: HashMap::new()
        }
    }

    pub fn insert(&mut self, hash: Hash, schema: Option<&'a [Option<Hash>]>, link: Option<&'a Validator>) {
        let entry = self.list.entry(hash).or_insert_with(|| ListItem::new());
        if let Some(schema) = schema { entry.schema.push(schema) }
        if let Some(link) = link { entry.link.push(link) }
    }

    pub fn complete(self) -> Result<()> {
        if self.list.is_empty() {
            Ok(())
        }
        else {
            Err(Error::FailValidate("Not all verification checklist items were completed".into()))
        }
    }

}
