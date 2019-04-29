use std::io;
use std::io::Error;
use std::io::ErrorKind::{InvalidData, UnexpectedEof};
use std::collections::BTreeMap;
use std::cmp::Ordering;

use byteorder::{ReadBytesExt, BigEndian};

use super::{Value, Integer, ValueRef, Hash, Identity, Lockbox, Timestamp};
use Marker;
use MarkerType;

fn not_shortest() -> io::Error {
    Error::new(InvalidData, "Not shortest possible encoding")
}

fn not_negative() -> io::Error {
    Error::new(InvalidData, "Positive value used in Int type")
}

/// Decode a MessagePack value. Decoding will fail if the value isn't in 
/// condense-db canonical form. That is:
/// - All types are encoded in as few bytes as possible
/// - Positive integers are always encoded using UInt types
/// - Map types always have unique strings as keys
/// - Maps are ordered lexicographically
/// - Strings are valid UTF-8
pub fn read_value(buf: &mut &[u8]) -> io::Result<Value> {
    let marker = read_marker(buf)?;
    Ok(match marker {
        MarkerType::Null => Value::Null,
        MarkerType::Boolean(v) => Value::Boolean(v),
        MarkerType::NegInt((len, v)) => Value::Integer(read_neg_int(buf, len, v)?),
        MarkerType::PosInt((len, v)) => Value::Integer(read_pos_int(buf, len, v)?),
        MarkerType::String(len) => Value::String(read_str(buf, len)?.to_string()),
        MarkerType::F32 => Value::F32(buf.read_f32::<BigEndian>()?),
        MarkerType::F64 => Value::F64(buf.read_f64::<BigEndian>()?),
        MarkerType::Binary(len) => Value::Binary(read_bin(buf, len)?.to_vec()),
        MarkerType::Array(len) => {
            let mut v = Vec::new();
            for _i in 0..len {
                v.push(read_value(buf)?);
            }
            Value::Array(v)
        },
        MarkerType::Object(len) => {
            read_to_map(buf, len)?
        }
        MarkerType::Hash(len) => Value::Hash(read_hash(buf, len)?),
        MarkerType::Identity(len) => Value::Identity(read_id(buf, len)?),
        MarkerType::Lockbox(len) => Value::Lockbox(read_lockbox(buf, len)?),
        MarkerType::Timestamp(len) => Value::Timestamp(read_time(buf, len)?),
    })
}

/// Decode a MessagePack value without copying binary data or strings. Decoding will fail if the 
/// value isn't in condense-db canonical form. That is:
/// - All types are encoded in as few bytes as possible
/// - Positive integers are always encoded using UInt types
/// - Map types always have unique strings as keys
/// - Maps are ordered lexicographically
/// - Strings are valid UTF-8
pub fn read_value_ref<'a>(buf: &mut &'a [u8]) -> io::Result<ValueRef<'a>> {
    let marker = read_marker(buf)?;
    Ok(match marker {
        MarkerType::Null => ValueRef::Null,
        MarkerType::Boolean(v) => ValueRef::Boolean(v),
        MarkerType::NegInt((len, v)) => ValueRef::Integer(read_neg_int(buf, len, v)?),
        MarkerType::PosInt((len, v)) => ValueRef::Integer(read_pos_int(buf, len, v)?),
        MarkerType::String(len) => ValueRef::String(read_str(buf, len)?),
        MarkerType::F32 => ValueRef::F32(buf.read_f32::<BigEndian>()?),
        MarkerType::F64 => ValueRef::F64(buf.read_f64::<BigEndian>()?),
        MarkerType::Binary(len) => ValueRef::Binary(read_bin(buf, len)?),
        MarkerType::Array(len) => {
            let mut v = Vec::new();
            for _i in 0..len {
                v.push(read_value_ref(buf)?);
            }
            ValueRef::Array(v)
        },
        MarkerType::Object(len) => {
            read_to_map_ref(buf, len)?
        }
        MarkerType::Hash(len) => ValueRef::Hash(read_hash(buf, len)?),
        MarkerType::Identity(len) => ValueRef::Identity(read_id(buf, len)?),
        MarkerType::Lockbox(len) => ValueRef::Lockbox(read_lockbox(buf, len)?),
        MarkerType::Timestamp(len) => ValueRef::Timestamp(read_time(buf, len)?),
    })
}

/// Verify a MessagePack value and return the number of bytes in it. Fails if the value isn't in 
/// condense-db canonical form. That is:
/// - All types are encoded in as few bytes as possible
/// - Positive integers are always encoded using UInt types
/// - Map types always have unique strings as keys
/// - Maps are ordered lexicographically
/// - Strings are valid UTF-8
pub fn verify_value(buf: &mut &[u8]) -> io::Result<usize> {
    let length = buf.len();
    let marker = read_marker(buf)?;
    match marker {
        MarkerType::NegInt((len, v)) => { read_neg_int(buf, len, v)?; },
        MarkerType::PosInt((len, v)) => { read_pos_int(buf, len, v)?; },
        MarkerType::String(len) => { read_str(buf, len)?; },
        MarkerType::F32 => { buf.read_f32::<BigEndian>()?; },
        MarkerType::F64 => { buf.read_f64::<BigEndian>()?; },
        MarkerType::Binary(len) => { read_bin(buf, len)?; },
        MarkerType::Array(len) => {
            for _i in 0..len {
                verify_value(buf)?;
            }
        },
        MarkerType::Object(len) => { verify_map(buf, len)?; },
        MarkerType::Hash(len) => { read_hash(buf, len)?; },
        MarkerType::Identity(len) => { read_id(buf, len)?; },
        MarkerType::Lockbox(len) => { read_lockbox(buf, len)?; },
        MarkerType::Timestamp(len) => { read_time(buf, len)?; },
        _ => (),
    }
    Ok(length - buf.len())
}

/// Read a positive integer straight out of the stream. The size of the integer should be known from the 
/// msgpack marker that was used. If the marker contained the integer, it should be included as `v`.
/// Example Usage:
/// ```
/// let value = if let MarkerType::PosInt((len, v)) = read_marker(buf) {
///     read_pos_int(buf, len, v)?
/// }
/// else {
///     return Err(Error::new(InvalidData, "Expected positive integer"));
/// }
/// ```
pub fn read_pos_int(buf: &mut &[u8], len: usize, v: u8) -> io::Result<Integer> {
    match len {
        0 => Ok(v.into()),
        1 => {
            let v = buf.read_u8()?;
            if v > 127 {
                Ok(v.into())
            }
            else {
                Err(not_shortest())
            }
        },
        2 => {
            let v = buf.read_u16::<BigEndian>()?;
            if v > (std::u8::MAX as u16) {
                Ok(v.into())
            }
            else {
                Err(not_shortest())
            }
        },
        4 => {
            let v = buf.read_u32::<BigEndian>()?;
            if v > (std::u16::MAX as u32) {
                Ok(v.into())
            }
            else {
                Err(not_shortest())
            }
        },
        8 => {
            let v = buf.read_u64::<BigEndian>()?;
            if v > (std::u32::MAX as u64) {
                Ok(v.into())
            }
            else {
                Err(not_shortest())
            }
        },
        _ => Err(Error::new(InvalidData, format!("Length of positive integer is {}, not in 0,1,2,4,8", len))),
    }
}

/// Read a negative integer straight out of the stream. The size of the integer should be known from the 
/// msgpack marker that was used. If the marker contained the integer, it should be included as `v`.
/// Example Usage:
/// ```
/// let value = if let MarkerType::NegInt((len, v)) = read_marker(buf) {
///     read_neg_int(buf, len, v)?
/// }
/// else {
///     return Err(Error::new(InvalidData, "Expected negative integer"));
/// }
/// ```
pub fn read_neg_int(buf: &mut &[u8], len: usize, v: i8) -> io::Result<Integer> {
    match len {
        0 => Ok(v.into()),
        1 => {
            let v = buf.read_i8()?;
            if v < -32 {
                Ok(v.into())
            }
            else if v >= 0 {
                Err(not_negative())
            }
            else {
                Err(not_shortest())
            }
        },
        2 => {
            let v = buf.read_i16::<BigEndian>()?;
            if v < (std::i8::MIN as i16) {
                Ok(v.into())
            }
            else if v >= 0 {
                Err(not_negative())
            }
            else {
                Err(not_shortest())
            }
        },
        4 => {
            let v = buf.read_i32::<BigEndian>()?;
            if v < (std::i16::MIN as i32) {
                Ok(v.into())
            }
            else if v >= 0 {
                Err(not_negative())
            }
            else {
                Err(not_shortest())
            }
        },
        8 => {
            let v = buf.read_i64::<BigEndian>()?;
            if v < (std::i32::MIN as i64) {
                Ok(v.into())
            }
            else if v >= 0 {
                Err(not_negative())
            }
            else {
                Err(not_shortest())
            }
        }
        _ => Err(Error::new(InvalidData, format!("Length of negative integer is {}, not in 0,1,2,4,8", len))),
    }
}

pub fn try_read_integer(buf: &mut &[u8]) -> io::Result<Integer> {
    let marker = read_marker(buf)?;
    match marker {
        MarkerType::PosInt((len, v)) => read_pos_int(buf, len, v),
        MarkerType::NegInt((len, v)) => read_neg_int(buf, len, v),
        _ => Err(Error::new(InvalidData, format!("Expected Integer, got {:?}", marker))),
    }
}

/// General function for referencing binary data in a buffer. Checks for if the 
/// length is greater than remaining bytes in the buffer.
pub fn read_bin<'a>(buf: &mut &'a [u8], len: usize) -> io::Result<&'a [u8]> {
    if buf.len() >= len {
        let (data, rem) = buf.split_at(len);
        *buf = rem;
        Ok(data)
    }
    else {
        Err(Error::new(UnexpectedEof, "Binary length larger than amount of data"))
    }
}

/// General function for referencing a UTF-8 string in a buffer. Checks for if the 
/// length is greater than remaining bytes in the buffer, or if the bytes 
/// received are not valid UTF-8.
pub fn read_str<'a>(buf: &mut &'a [u8], len: usize) -> io::Result<&'a str> {
    if buf.len() >= len {
        let (data, rem) = buf.split_at(len);
        *buf = rem;
        let data = std::str::from_utf8(data)
            .map_err(|_e| Error::new(InvalidData, "String decoded is not valid UTF-8"))?;
        Ok(data)
    }
    else {
        Err(Error::new(UnexpectedEof, "String length larger than amount of data"))
    }
}

/// General function for reading a field-value map from a buffer. Checks to make 
/// sure the keys are unique, valid UTF-8 Strings in lexicographic order.
pub fn read_to_map(buf: &mut &[u8], len: usize) -> io::Result<Value> {

    let mut map: BTreeMap<String,Value> = BTreeMap::new();
    if len == 0 { return Ok(Value::from(map)); }

    // Extract the first field-value pair
    let mut old_key = read_value(buf)?.to_string()
        .ok_or(Error::new(InvalidData, "Object field was not a String"))?;
    let val = read_value(buf)?;
    map.insert(old_key.clone(), val);
    // Iterate to get remaining field-value pairs
    for _i in 1..len {
        let key = read_value(buf)?.to_string()
            .ok_or(Error::new(InvalidData, "Object field was not a String"))?;
        match old_key.cmp(&key) {
            Ordering::Less => {
                // old_key is lower in order. This is correct
                let val = read_value(buf)?;
                map.insert(key.clone(), val);
            },
            Ordering::Equal => {
                return Err(Error::new(InvalidData, format!("Found object with non-unique field \"{}\"", key)));
            },
            Ordering::Greater => {
                return Err(Error::new(InvalidData,
                    format!("Object fields not in lexicographic order. Last = '{}', Current = '{}'", old_key, key)));
            }
        };
        old_key = key;
    }
    Ok(Value::Object(map))
}

fn try_read_str_ref<'a>(buf: &mut &'a [u8]) -> io::Result<&'a str> {
    if let MarkerType::String(len) = read_marker(buf)? {
        read_str(buf, len)
    }
    else {
        Err(Error::new(InvalidData, "Object field was not a String"))
    }
}

/// General function for referencing a field-value map in a buffer. Checks to make 
/// sure the keys are unique, valid UTF-8 Strings in lexicographic order.
pub fn read_to_map_ref<'a>(buf: &mut &'a [u8], len: usize) -> io::Result<ValueRef<'a>> {

    let mut map: BTreeMap<&'a str,ValueRef<'a>> = BTreeMap::new();
    if len == 0 { return Ok(ValueRef::Object(map)); }

    // Extract the first field-value pair
    let mut old_key = try_read_str_ref(buf)?;
    let val = read_value_ref(buf)?;
    map.insert(old_key.clone(), val);
    // Iterate to get remaining field-value pairs
    for _i in 1..len {
        let key = try_read_str_ref(buf)?;
        match old_key.cmp(&key) {
            Ordering::Less => {
                // old_key is lower in order. This is correct
                let val = read_value_ref(buf)?;
                map.insert(key.clone(), val);
            },
            Ordering::Equal => {
                return Err(Error::new(InvalidData, format!("Found object with non-unique field \"{}\"", key)));
            },
            Ordering::Greater => {
                return Err(Error::new(InvalidData,
                    format!("Object fields not in lexicographic order. Last = '{}', Current = '{}'", old_key, key)));
            }
        };
        old_key = key;
    }
    Ok(ValueRef::Object(map))
}

/// General function for verifying a field-value map in a buffer. Makes sure the keys are unique, 
/// valid UTF-8 Strings in lexicographic order.
pub fn verify_map(buf: &mut &[u8], len: usize) -> io::Result<usize> {

    if len == 0 { return Ok(0); }
    let length = buf.len();

    // Extract the first field-value pair
    let mut old_key = try_read_str_ref(buf)?;
    verify_value(buf)?;
    // Iterate to get remaining field-value pairs
    for _i in 1..len {
        let key = try_read_str_ref(buf)?;
        match old_key.cmp(&key) {
            Ordering::Less => {
                // old_key is lower in order. This is correct
                verify_value(buf)?;
            },
            Ordering::Equal => {
                return Err(Error::new(InvalidData, format!("Found object with non-unique field \"{}\"", key)));
            },
            Ordering::Greater => {
                return Err(Error::new(InvalidData,
                    format!("Object fields not in lexicographic order. Last = '{}', Current = '{}'", old_key, key)));
            }
        };
        old_key = key;
    }
    Ok(length - buf.len())
}

/// Read raw Timestamp out from a buffer
pub fn read_time(buf: &mut &[u8], len: usize) -> io::Result<Timestamp> {
    match len {
        4 => {
            let sec = buf.read_u32::<BigEndian>()?;
            Ok(Timestamp::from_sec(sec as i64))
        },
        8 => {
            let raw_time = buf.read_u64::<BigEndian>()?;
            let sec = (raw_time & 0x3FFFF_FFFFu64) as i64;
            let nano = (raw_time >> 34) as u32;
            Ok(Timestamp::from_raw(sec,nano).ok_or(Error::new(InvalidData, "Timestamp nanoseconds is too big"))?)
        },
        12 => {
            let nano = buf.read_u32::<BigEndian>()?;
            let sec = buf.read_i64::<BigEndian>()?;
            Ok(Timestamp::from_raw(sec,nano).ok_or(Error::new(InvalidData, "Timestamp nanoseconds is too big"))?)
        },
        _ => Err(Error::new(InvalidData, "Timestamp type has invalid size"))
    }
}

/// Read raw Hash out from a buffer
pub fn read_hash(buf: &mut &[u8], len: usize) -> io::Result<Hash> {
    let hash = Hash::decode(buf).map_err(|_e| Error::new(InvalidData, "Hash not recognized"))?;
    if hash.len() != len {
        Err(Error::new(InvalidData, "Hash type has invalid size"))
    }
    else {
        Ok(hash)
    }
}

/// Read raw Identity out from a buffer
pub fn read_id(buf: &mut &[u8], len: usize) -> io::Result<Identity> {
    let id = Identity::decode(buf).map_err(|_e| Error::new(InvalidData, "Identity not recognized"))?;
    if id.len() != len {
        Err(Error::new(InvalidData, "Identity type has invalid size"))
    }
    else {
        Ok(id)
    }
}

/// Read raw lockbox data out from a buffer
pub fn read_lockbox(buf: &mut &[u8], len: usize) -> io::Result<Lockbox> {
    Ok(Lockbox::decode(len, buf).map_err(|_e| Error::new(InvalidData, "Lockbox not recognized"))?)
}


/// Read a msgpack marker, length, and/or extension type from a buffer.
pub fn read_marker(buf: &mut &[u8]) -> io::Result<MarkerType> {
    let marker = Marker::from_u8(buf.read_u8()?);
    Ok(match marker {
        Marker::PosFixInt(val) => MarkerType::PosInt((0,val)),
        Marker::FixMap(len) => MarkerType::Object(len as usize),
        Marker::FixStr(len) => MarkerType::String(len as usize),
        Marker::FixArray(len) => MarkerType::Array(len as usize),
        Marker::Nil => MarkerType::Null,
        Marker::False => MarkerType::Boolean(false),
        Marker::True => MarkerType::Boolean(true),
        Marker::Bin8 => {
            let len = buf.read_u8()? as usize;
            MarkerType::Binary(len)
        },
        Marker::Bin16 => {
            let len = buf.read_u16::<BigEndian>()? as usize;
            if len <= (std::u8::MAX as usize) { return Err(not_shortest()); }
            MarkerType::Binary(len)
        },
        Marker::Bin32 => {
            let len = buf.read_u32::<BigEndian>()? as usize;
            if len <= (std::u16::MAX as usize) { return Err(not_shortest()); }
            MarkerType::Binary(len)
        },
        Marker::Ext8 => {
            let len = buf.read_u8()? as usize;
            match len {
                1  => { return Err(not_shortest()); },
                2  => { return Err(not_shortest()); },
                4  => { return Err(not_shortest()); },
                8  => { return Err(not_shortest()); },
                16 => { return Err(not_shortest()); },
                _  => {
                    let ty = buf.read_i8()?;
                    MarkerType::from_ext_i8(len, ty)
                        .ok_or(Error::new(InvalidData, format!("Unsupported Extension type 0x{:X}", ty)))?
                }
            }
        },
        Marker::Ext16 => {
            let len = buf.read_u16::<BigEndian>()? as usize;
            if len <= (std::u8::MAX as usize) { return Err(not_shortest()); }
            let ty = buf.read_i8()?;
            MarkerType::from_ext_i8(len, ty)
                .ok_or(Error::new(InvalidData, format!("Unsupported Extension type 0x{:X}", ty)))?
        },
        Marker::Ext32 => {
            let len = buf.read_u32::<BigEndian>()? as usize;
            if len <= (std::u16::MAX as usize) { return Err(not_shortest()); }
            let ty = buf.read_i8()?;
            MarkerType::from_ext_i8(len, ty)
                .ok_or(Error::new(InvalidData, format!("Unsupported Extension type 0x{:X}", ty)))?
        },
        Marker::F32 => MarkerType::F32,
        Marker::F64 => MarkerType::F64,
        Marker::UInt8 => MarkerType::PosInt((1,0)),
        Marker::UInt16 => MarkerType::PosInt((2,0)),
        Marker::UInt32 => MarkerType::PosInt((4,0)),
        Marker::UInt64 => MarkerType::PosInt((8,0)),
        Marker::Int8 => MarkerType::NegInt((1,0)),
        Marker::Int16 => MarkerType::NegInt((2,0)),
        Marker::Int32 => MarkerType::NegInt((4,0)),
        Marker::Int64 => MarkerType::NegInt((8,0)),
        Marker::FixExt1 => {
            let ty = buf.read_i8()?;
            MarkerType::from_ext_i8(1, ty)
                .ok_or(Error::new(InvalidData, format!("Unsupported Extension type 0x{:X}", ty)))?
        },
        Marker::FixExt2 => {
            let ty = buf.read_i8()?;
            MarkerType::from_ext_i8(2, ty)
                .ok_or(Error::new(InvalidData, format!("Unsupported Extension type 0x{:X}", ty)))?
        },
        Marker::FixExt4 => {
            let ty = buf.read_i8()?;
            MarkerType::from_ext_i8(4, ty)
                .ok_or(Error::new(InvalidData, format!("Unsupported Extension type 0x{:X}", ty)))?
        },
        Marker::FixExt8 => {
            let ty = buf.read_i8()?;
            MarkerType::from_ext_i8(8, ty)
                .ok_or(Error::new(InvalidData, format!("Unsupported Extension type 0x{:X}", ty)))?
        },
        Marker::FixExt16 => {
            let ty = buf.read_i8()?;
            MarkerType::from_ext_i8(16, ty)
                .ok_or(Error::new(InvalidData, format!("Unsupported Extension type 0x{:X}", ty)))?
        },
        Marker::Str8 => {
            let len = buf.read_u8()? as usize;
            if len <= 31 { return Err(not_shortest()); }
            MarkerType::String(len)
        }
        Marker::Str16 => {
            let len = buf.read_u16::<BigEndian>()? as usize;
            if len <= (std::u8::MAX as usize) { return Err(not_shortest()); }
            MarkerType::String(len)
        }
        Marker::Str32 => {
            let len = buf.read_u32::<BigEndian>()? as usize;
            if len <= (std::u16::MAX as usize) { return Err(not_shortest()); }
            MarkerType::String(len)
        }
        Marker::Array16 => {
            let len = buf.read_u16::<BigEndian>()?;
            if len <= 15 { return Err(not_shortest()); }
            MarkerType::Array(len as usize)
        }
        Marker::Array32 => {
            let len = buf.read_u32::<BigEndian>()?;
            if len <= (std::u16::MAX as u32) { return Err(not_shortest()); }
            MarkerType::Array(len as usize)
        }
        Marker::Map16 => {
            let len = buf.read_u16::<BigEndian>()?;
            if len <= 15 { return Err(not_shortest()); }
            MarkerType::Object(len as usize)
        },
        Marker::Map32 => {
            let len = buf.read_u32::<BigEndian>()?;
            if len <= (std::u16::MAX as u32) { return Err(not_shortest()); }
            MarkerType::Object(len as usize)
        },
        Marker::NegFixInt(val) => MarkerType::NegInt((0,val)),
        Marker::Reserved => { return Err(Error::new(InvalidData, "Unsupported value type")) },
    })
}








