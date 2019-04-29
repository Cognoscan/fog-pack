use std::io;
use std::io::ErrorKind::{InvalidData, UnexpectedEof};
use std::collections::HashMap;
//use std::cmp::Ordering;

use byteorder::{ReadBytesExt, BigEndian};

use super::{Hash, Integer};
use Marker;
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

    let _old_field = get_string(doc)?;

    Ok(())
}

fn get_string<'a>(buf: &mut &'a [u8]) -> io::Result<&'a str> {
    let marker = Marker::from_u8(buf.read_u8()?);
    let len = match marker {
        Marker::FixStr(len) => {
            len as usize
        },
        Marker::Str8 => {
            let len = buf.read_u8()? as usize;
            if len <= 31 { return Err(not_shortest()); }
            len
        }
        Marker::Str16 => {
            let len = buf.read_u16::<BigEndian>()? as usize;
            if len <= (std::u8::MAX as usize) { return Err(not_shortest()); }
            len
        }
        Marker::Str32 => {
            let len = buf.read_u32::<BigEndian>()? as usize;
            if len <= (std::u16::MAX as usize) { return Err(not_shortest()); }
            len
        },
        _ => {
            return Err(io::Error::new(InvalidData, "Expected String, got something else"));
        }
    };
    if buf.len() >= len {
        let (data, rem) = buf.split_at(len);
        *buf = rem;
        let data = ::std::str::from_utf8(data)
            .map_err(|_e| io::Error::new(InvalidData, "String decoded is not valid UTF-8"))?;
        Ok(data)
    }
    else {
        Err(io::Error::new(UnexpectedEof, "String length larger than amount of data"))
    }
}






