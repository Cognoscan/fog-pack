use std::io;
use std::io::ErrorKind::{InvalidData, UnexpectedEof};
use std::collections::HashMap;
//use std::cmp::Ordering;

use byteorder::{ReadBytesExt, BigEndian};

use super::{Hash, Integer};
use Marker;
use decode;
//use ExtType;

fn not_shortest() -> io::Error {
    io::Error::new(InvalidData, "Not shortest possible encoding")
}

fn not_negative() -> io::Error {
    io::Error::new(InvalidData, "Positive value used in Int type")
}

pub struct Schema {
    name: String,
    hash: Hash,
    version: Integer,
    required: HashMap<String, Validator>,
    optional: HashMap<String, Validator>,
    entries: HashMap<String, Validator>,
    types: HashMap<String, Validator>,
    unknown_ok: bool
}

impl Schema {
    pub fn from_raw(_raw: &[u8]) -> io::Result<Schema> {
        Ok(Schema {
            name: String::new(),
            hash: Hash::new_empty(),
            version: Integer::from(0),
            required: HashMap::new(),
            optional: HashMap::new(),
            entries: HashMap::new(),
            types: HashMap::new(),
            unknown_ok: false,
        })
    }

    pub fn validate_doc(&self, _doc: &mut &[u8]) -> io::Result<()> {
        Ok(())
    }
}

enum Validator {
    Null(ValidNull),
    Boolean(ValidBool),
    Integer(ValidInt),
    String(ValidStr),
    F32(ValidF32),
    F64(ValidF64),
    //Binary(ValidBin),
    //Array(ValidArray),
    //Object(ValidObj),
    //Hash(ValidHash),
    //Identity(ValidId),
    //Lockbox(ValidLock),
    //Timestamp(ValidTime),
}

struct ValidNull {
    query: bool
}

struct ValidBool {
    default: Option<bool>,
    constant: Option<bool>,
    query: bool,
    ord: bool,
}

struct ValidInt {
    in_vec: Option<Vec<Integer>>,
    nin_vec: Option<Vec<Integer>>,
    constant: Option<Integer>,
    min: Option<Integer>,
    max: Option<Integer>,
    default: Option<Integer>,
    query: bool,
    ord: bool,
    bit: bool,
}

struct ValidStr {
    in_vec: Option<Vec<String>>,
    nin_vec: Option<Vec<String>>,
    constant: Option<String>,
    min_len: Option<usize>,
    max_len: Option<usize>,
    matches: Option<String>,
    default: Option<String>,
    query: bool,
    ord: bool,
    regex: bool,
}

struct ValidF32 {
    in_vec: Option<Vec<f32>>,
    nin_vec: Option<Vec<f32>>,
    constant: Option<f32>,
    min: Option<f32>,
    max: Option<f32>,
    ex_min: bool,
    ex_max: bool,
    default: Option<f32>,
    query: bool,
    ord: bool,
}

struct ValidF64 {
    in_vec: Option<Vec<f64>>,
    nin_vec: Option<Vec<f64>>,
    constant: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
    ex_min: bool,
    ex_max: bool,
    default: Option<f64>,
    query: bool,
    ord: bool,
}

    
pub fn validate_doc(_schema: &Schema, doc: &mut &[u8]) -> io::Result<()> {
    let marker = Marker::from_u8(doc.read_u8()?);
    let len = match marker {
        Marker::FixMap(len) => len as usize,
        Marker::Map16 => {
            let len = doc.read_u16::<BigEndian>()?;
            if len <= 15 { return Err(not_shortest()); }
            len as usize
        },
        Marker::Map32 => {
            let len = doc.read_u32::<BigEndian>()?;
            if len <= (std::u16::MAX as u32) { return Err(not_shortest()); }
            len as usize
        },
        _ => {
            return Err(io::Error::new(InvalidData, "Document must be an Object"));
        }
    };
    if len == 0 { return Err(io::Error::new(InvalidData, "Document cannot have zero fields")); }

    let _old_field = decode::read_string(doc)?;

    Ok(())
}





