use std::collections::BTreeMap;

use crate::*;
use crate::{compress::Compress, validator::Validator};

pub struct InnerSchema {
    description: String,
    doc: Validator,
    doc_compress: Compress,
    entries: BTreeMap<String, Validator>,
    entries_compress: BTreeMap<String, Compress>,
    name: String,
    types: BTreeMap<String, Validator>,
    version: Integer,
}
