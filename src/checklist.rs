/*!
Checklists used for verifying an Entry.
*/
use std::collections::HashMap;

use Hash;
use validator::ValidatorChecklist;

/// A single item within a checklist. Used by [`Checklist`]. Complete 
/// by passing to a schema along with the appropriate Document
///
/// [`Checklist`]: ./struct.Checklist.html
pub struct ChecklistItem {
    items: Vec<usize>,
    done: bool
}

impl ChecklistItem {
    fn new(items: Vec<usize>) -> Self {
        ChecklistItem { items, done: false }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    pub(crate) fn get_list(&self) -> &Vec<usize> {
        &self.items
    }

    pub(crate) fn iter(&self) -> ::std::slice::Iter<usize> {
        self.items.iter()
    }

    pub(crate) fn mark_done(&mut self) {
        self.done = true;
    }
}

/// A Checklist for validating an Entry.
///
/// The checklist can be iterated over, yielding a series of hashes and their associated 
/// [`ChecklistItem`]. Passing an item to a schema along with the document referred to by the hash 
/// allows it to be checked off the list. When all items have been checked off, calling 
/// [`complete`] will return the contained data. This may be a Vec<u8> from encoding an Entry, an 
/// Entry that was decoded, or nothihng if the purpose was purely validation.
///
/// # Examples
///
/// Assuming there is a HashMap containing all documents, a function to encode and verify an 
/// entry could be:
///
/// ```
/// # use fog_pack::{Schema, Entry, Hash, Document};
/// # use std::collections::HashMap;
/// #
/// fn encode_entry(e: Entry, schema: &mut Schema, db: &HashMap<Hash, Document>) -> 
///     Result<Vec<u8>, ()>
/// {
///     let mut checklist = schema.encode_entry(e).or(Err(()))?;
///     // Fetch each document for verification, and fail if we don't have one
///     for (h, item) in checklist.iter_mut() {
///         if let Some(doc) = db.get(h) {
///             schema.check_item(doc, item);
///         }
///         else {
///             return Err(());
///         }
///     }
///     checklist.complete()
/// }
/// ```
///
/// [`ChecklistItem`]: ./struct.ChecklistItem.html
/// [`complete`]: #method.complete
/// [`Entry`]: ../struct.Entry.html
pub struct Checklist<T> {
    list: HashMap<Hash, ChecklistItem>,
    data: T,
}

impl<T> Checklist<T> {
    pub(crate) fn new(checklist: ValidatorChecklist, data: T) -> Self {
        let mut list = HashMap::with_capacity(checklist.len());
        for v in checklist.to_map().drain() {
            list.insert(v.0, ChecklistItem::new(v.1));
        }
        Self {
            list,
            data,
        }
    }

    /// Check to see if the checklist is ready for completion.
    pub fn is_complete(&self) -> bool {
        self.list.values().all(|x| x.done())
    }

    /// Complete the checklist and return the encoded Entry as a byte vector. Fails if the 
    /// checklist was not completed.
    pub fn complete(self) -> Result<T, ()> {
        if self.is_complete() {
            Ok(self.data)
        }
        else {
            Err(())
        }
    }

    /// Iterate over the checklist, yielding tuples of type `(&Hash, &ChecklistItem)`.
    pub fn iter(&self) -> ::std::collections::hash_map::Iter<Hash, ChecklistItem> {
        self.list.iter()
    }

    /// Mutably iterate over the checklist, yielding tuples of type `(&Hash, &mut ChecklistItem)`.
    pub fn iter_mut(&mut self) -> ::std::collections::hash_map::IterMut<Hash, ChecklistItem> {
        self.list.iter_mut()
    }

    /// Fetch a specific checklist item by hash.
    pub fn get(&self, h: &Hash) -> Option<&ChecklistItem> {
        self.list.get(h)
    }

    /// Mutably fetch a specific checklist item by hash.
    pub fn get_mut(&mut self, h: &Hash) -> Option<&mut ChecklistItem> {
        self.list.get_mut(h)
    }
}
