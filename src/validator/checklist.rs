use std::collections::HashMap;

use super::*;
use crate::Hash;
use crate::{
    error::{Error, Result},
    Document,
};

/// An item in a Checklist. To complete it, find a document whose hash matches the one returned by
/// this item's [`doc`] function, then feed that document to the [`check`] function of this item.
/// If the check fails, checking should be halted and the checklist should be discarded.
#[derive(Clone, Debug)]
pub struct ListItem<'a> {
    inner: InnerListItem<'a>,
    schema: &'a Hash,
    types: &'a BTreeMap<String, Validator>,
    doc: Hash,
}

impl<'a> ListItem<'a> {
    pub fn doc(&self) -> &Hash {
        &self.doc
    }

    pub fn check(self, doc: &Document) -> Result<()> {
        // Check to make sure we have the right Document
        let actual_hash = doc.hash();
        if actual_hash != self.doc {
            return Err(Error::FailValidate(format!(
                "Got wrong document for checklist item. Expected {}, got {}",
                self.doc, actual_hash
            )));
        }

        // Check that the Document meets all the `schema` requirements from each Hash validator
        if !self.inner.schema.is_empty() {
            let doc_schema = match doc.schema_hash() {
                Some(schema) => schema,
                None => {
                    return Err(Error::FailValidate(
                        "Document has no schema, but must pass `schema` validation".into(),
                    ))
                }
            };
            let all_schema_pass = self.inner.schema.iter().all(|list| {
                list.iter().any(|schema| match schema {
                    None => self.schema == doc_schema,
                    Some(schema) => schema == doc_schema,
                })
            });
            if !all_schema_pass {
                return Err(Error::FailValidate(
                    "Document schema didn't satisfy all `schema` requirements".into(),
                ));
            }
        }

        // Check that the Document meets all the `link` validators from each Hash validator
        // Note: we have no new checklist, because this is a document.
        // We don't need to `finish()` the parser after each validation because that's to
        // catch sitautions where the inner data contains more than one fog-pack value in sequence.
        // Because we already have a Document, that check was already performed.
        let parser = Parser::new(doc.data());
        let all_link_pass = self
            .inner
            .link
            .iter()
            .all(|validator| validator.validate(self.types, parser.clone(), None).is_ok());
        if !all_link_pass {
            return Err(Error::FailValidate(
                "Document schema didn't satisfy all `link` requirements".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct InnerListItem<'a> {
    schema: Vec<&'a [Option<Hash>]>,
    link: Vec<&'a Validator>,
}

impl<'a> InnerListItem<'a> {
    pub fn new() -> Self {
        Self {
            schema: Vec::new(),
            link: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DataChecklist<'a, T> {
    list: Checklist<'a>,
    data: T,
}

impl<'a, T> DataChecklist<'a, T> {
    pub(crate) fn from_checklist(list: Checklist<'a>, data: T) -> Self {
        Self { list, data }
    }

    /// Iterate through the whole checklist, going through one item at a time. Each item should be
    /// checked; see [`ListItem`] for details.
    pub fn iter(&mut self) -> impl Iterator<Item = ListItem> {
        self.list.iter()
    }

    pub fn complete(self) -> Result<T> {
        self.list.complete()?;
        Ok(self.data)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Checklist<'a> {
    list: HashMap<Hash, InnerListItem<'a>>,
    types: &'a BTreeMap<String, Validator>,
    schema: &'a Hash,
}

impl<'a> Checklist<'a> {
    pub fn new(schema: &'a Hash, types: &'a BTreeMap<String, Validator>) -> Self {
        Self {
            list: HashMap::new(),
            types,
            schema,
        }
    }

    pub(crate) fn insert(
        &mut self,
        hash: Hash,
        schema: Option<&'a [Option<Hash>]>,
        link: Option<&'a Validator>,
    ) {
        let entry = self.list.entry(hash).or_insert_with(InnerListItem::new);
        if let Some(schema) = schema {
            entry.schema.push(schema)
        }
        if let Some(link) = link {
            entry.link.push(link)
        }
    }

    /// Iterate through the whole checklist, going through one item at a time. Each item should be
    /// checked; see [`ListItem`] for details.
    pub fn iter(&mut self) -> impl Iterator<Item = ListItem> {
        let schema = self.schema;
        let types = self.types;
        self.list.drain().map(move |(doc, inner)| ListItem {
            inner,
            doc,
            types,
            schema,
        })
    }

    pub fn complete(self) -> Result<()> {
        if self.list.is_empty() {
            Ok(())
        } else {
            Err(Error::FailValidate(
                "Not all verification checklist items were completed".into(),
            ))
        }
    }
}
