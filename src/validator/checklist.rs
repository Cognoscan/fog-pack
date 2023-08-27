use std::collections::HashMap;

use super::*;
use crate::Hash;
use crate::{
    document::Document,
    error::{Error, Result},
};

/// An item in a Checklist. To complete it, find a document whose hash matches the one that was
/// provided alongside this item, then feed that document to the [`check`][ListItem::check]
/// function of this item. If the check fails, checking should be halted and the checklist should
/// be discarded.
#[derive(Clone, Debug)]
pub struct ListItem<'a> {
    inner: InnerListItem<'a>,
    schema: &'a Hash,
    types: &'a BTreeMap<String, Validator>,
}

impl<'a> ListItem<'a> {
    /// Check an item in the checklist using the provided document. If the
    /// document passes, it returns a `Ok(())`. On failure, future checks should
    /// be halted and the checklist discarded.
    pub fn check(self, doc: &Document) -> Result<()> {
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
    fn new() -> Self {
        Self {
            schema: Vec::new(),
            link: Vec::new(),
        }
    }
}

/// A Checklist of documents that must be verified before the contained data is
/// yielded.
///
/// A checklist can be completed by performing the following steps:
/// 1. Call [`iter`][DataChecklist::iter] to get an iterator over the list.
/// 2. For each [item][ListItem], fetch the Document matching the hash that was
///     provided alongside the item, then provide it to the item by calling
///     [`check`][ListItem::check].
/// 3. When all the items have been completed successfully, call
///     [`complete`][DataChecklist::complete] to get the contained data.
///
#[derive(Clone, Debug)]
pub struct DataChecklist<'a, T> {
    list: Checklist<'a>,
    data: T,
}

impl<'a, T> DataChecklist<'a, T> {
    pub(crate) fn from_checklist(list: Checklist<'a>, data: T) -> Self {
        Self { list, data }
    }

    /// Iterate through the whole checklist, going through one Hash and list item at a time. For
    /// each item, look up a Document with the same hash and check it with the [`ListItem`]'s
    /// [`check`][ListItem::check] function.
    pub fn iter(&mut self) -> impl Iterator<Item = (Hash, ListItem)> {
        self.list.iter()
    }

    /// Complete a checklist, yielding the inner data value if the checklist was
    /// successfully iterated over.
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
    pub(crate) fn new(schema: &'a Hash, types: &'a BTreeMap<String, Validator>) -> Self {
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
    pub(crate) fn iter(&mut self) -> impl Iterator<Item = (Hash, ListItem)> {
        let schema = self.schema;
        let types = self.types;
        self.list.drain().map(move |(doc, inner)| {
            (
                doc,
                ListItem {
                    inner,
                    types,
                    schema,
                },
            )
        })
    }

    /// Complete the checklsit
    fn complete(self) -> Result<()> {
        if self.list.is_empty() {
            Ok(())
        } else {
            Err(Error::FailValidate(
                "Not all verification checklist items were completed".into(),
            ))
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{document::NewDocument, schema::*, types::Integer};

    use super::*;

    #[test]
    fn runthrough() {
        // Set up the schemas
        let schema1 = SchemaBuilder::new(IntValidator::default().build())
            .build()
            .unwrap();
        let schema1 = Schema::from_doc(&schema1).unwrap();
        let validator = IntValidator {
            min: Integer::from(0),
            ..IntValidator::default()
        }
        .build();
        let schema2 = SchemaBuilder::new(validator).build().unwrap();
        let schema2 = Schema::from_doc(&schema2).unwrap();

        let doc1 = NoSchema::validate_new_doc(NewDocument::new(None, 0u8).unwrap()).unwrap();
        let doc2 = NoSchema::validate_new_doc(NewDocument::new(None, 1u8).unwrap()).unwrap();
        let doc3 = schema1
            .validate_new_doc(NewDocument::new(Some(schema1.hash()), 0u8).unwrap())
            .unwrap();
        let doc4 = schema2
            .validate_new_doc(NewDocument::new(Some(schema2.hash()), 0u8).unwrap())
            .unwrap();

        let types = BTreeMap::new();
        let mut checklist = Checklist::new(schema1.hash(), &types);
        let validator = IntValidator {
            min: Integer::from(0u32),
            ..IntValidator::default()
        }
        .build();
        let schema2_schema = [Some(schema2.hash().clone())];
        checklist.insert(doc1.hash().clone(), None, Some(&validator));
        checklist.insert(doc2.hash().clone(), None, Some(&validator));
        checklist.insert(doc3.hash().clone(), Some(&[None]), None);
        checklist.insert(doc4.hash().clone(), Some(&schema2_schema), Some(&validator));
        let mut checklist = DataChecklist::from_checklist(checklist, ());

        let mut map = HashMap::new();
        map.insert(doc1.hash().clone(), doc1);
        map.insert(doc2.hash().clone(), doc2);
        map.insert(doc3.hash().clone(), doc3);
        map.insert(doc4.hash().clone(), doc4);

        checklist
            .iter()
            .try_for_each(|(hash, item)| {
                let doc = map
                    .get(&hash)
                    .ok_or_else(|| Error::FailValidate("".into()))?;
                item.check(doc)
            })
            .unwrap();
        checklist.complete().unwrap();
    }
}
