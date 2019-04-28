use std::io;
use std::io::ErrorKind::{InvalidData, UnexpectedEof};
use std::collections::BTreeMap;
use std::cmp::Ordering;

use byteorder::{ReadBytesExt, BigEndian};

use super::{Value, ValueRef, Hash, Identity, Lockbox, Timestamp};
use Marker;
use ExtType;

fn not_shortest() -> io::Error {
    io::Error::new(InvalidData, "Not shortest possible encoding")
}

fn not_negative() -> io::Error {
    io::Error::new(InvalidData, "Positive value used in Int type")
}

/// Decode a MessagePack value. Decoding will fail if the value isn't in 
/// condense-db canonical form. That is:
/// - All types are encoded in as few bytes as possible
/// - Positive integers are always encoded using UInt types
/// - Map types always have unique strings as keys
/// - Maps are ordered lexicographically
/// - Strings are valid UTF-8
pub fn read_value(buf: &mut &[u8]) -> io::Result<Value> {
    let marker = Marker::from_u8(buf.read_u8()?);
    match marker {

        Marker::PosFixInt(val) => Ok(Value::Integer(val.into())),
        Marker::FixMap(len) => {
            read_to_map(buf, len as usize)
        },
        Marker::FixStr(len) => {
            read_to_str(buf, len as usize)
        },
        Marker::FixArray(len) => {
            let mut v = Vec::new();
            for _i in 0..len {
                v.push(read_value(buf)?);
            }
            Ok(Value::from(v))
        },

        Marker::Nil => Ok(Value::Null),
        Marker::False => Ok(Value::Boolean(false)),
        Marker::True => Ok(Value::Boolean(true)),

        Marker::Bin8 => {
            let len = buf.read_u8()? as usize;
            read_to_bin(buf, len)
        },
        Marker::Bin16 => {
            let len = buf.read_u16::<BigEndian>()? as usize;
            if len <= (std::u8::MAX as usize) { return Err(not_shortest()); }
            read_to_bin(buf, len)
        },
        Marker::Bin32 => {
            let len = buf.read_u32::<BigEndian>()? as usize;
            if len <= (std::u16::MAX as usize) { return Err(not_shortest()); }
            read_to_bin(buf, len)
        },

        Marker::Ext8 => {
            let len = buf.read_u8()? as usize;
            match len {
                1  => Err(not_shortest()),
                2  => Err(not_shortest()),
                4  => Err(not_shortest()),
                8  => Err(not_shortest()),
                16 => Err(not_shortest()),
                _  => {
                    let ty = buf.read_i8()?;
                    read_ext(buf, len, ty)
                }
            }
        },
        Marker::Ext16 => {
            let len = buf.read_u16::<BigEndian>()? as usize;
            if len <= (std::u8::MAX as usize) { return Err(not_shortest()); }
            let ty = buf.read_i8()?;
            read_ext(buf, len, ty)
        },
        Marker::Ext32 => {
            let len = buf.read_u32::<BigEndian>()? as usize;
            if len <= (std::u16::MAX as usize) { return Err(not_shortest()); }
            let ty = buf.read_i8()?;
            read_ext(buf, len, ty)
        },

        Marker::F32 => {
            Ok(Value::F32(buf.read_f32::<BigEndian>()?))
        },

        Marker::F64 => {
            Ok(Value::F64(buf.read_f64::<BigEndian>()?))
        },

        Marker::UInt8 => {
            let val = buf.read_u8()?;
            if val > 127 {
                Ok(Value::Integer(val.into()))
            }
            else {
                Err(not_shortest())
            }
        },
        Marker::UInt16 => {
            let val = buf.read_u16::<BigEndian>()?;
            if val > (std::u8::MAX as u16) {
                Ok(Value::Integer(val.into()))
            }
            else {
                Err(not_shortest())
            }
        },
        Marker::UInt32 => {
            let val = buf.read_u32::<BigEndian>()?;
            if val > (std::u16::MAX as u32) {
                Ok(Value::Integer(val.into()))
            }
            else {
                Err(not_shortest())
            }
        },
        Marker::UInt64 => {
            let val = buf.read_u64::<BigEndian>()?;
            if val > (std::u32::MAX as u64) {
                Ok(Value::Integer(val.into()))
            }
            else {
                Err(not_shortest())
            }
        },

        Marker::Int8 => {
            let val = buf.read_i8()?;
            if val < -32 {
                Ok(Value::Integer(val.into()))
            }
            else if val >= 0 {
                Err(not_negative())
            }
            else {
                Err(not_shortest())
            }
        },
        Marker::Int16 => {
            let val = buf.read_i16::<BigEndian>()?;
            if val < (std::i8::MIN as i16) {
                Ok(Value::Integer(val.into()))
            }
            else if val >= 0 {
                Err(not_negative())
            }
            else {
                Err(not_shortest())
            }
        },
        Marker::Int32 => {
            let val = buf.read_i32::<BigEndian>()?;
            if val < (std::i16::MIN as i32) {
                Ok(Value::Integer(val.into()))
            }
            else if val >= 0 {
                Err(not_negative())
            }
            else {
                Err(not_shortest())
            }
        },
        Marker::Int64 => {
            let val = buf.read_i64::<BigEndian>()?;
            if val < (std::i32::MIN as i64) {
                Ok(Value::Integer(val.into()))
            }
            else if val >= 0 {
                Err(not_negative())
            }
            else {
                Err(not_shortest())
            }
        }

        Marker::FixExt1 => {
            let ty = buf.read_i8()?;
            read_ext(buf, 1, ty)
        },
        Marker::FixExt2 => {
            let ty = buf.read_i8()?;
            read_ext(buf, 2, ty)
        },
        Marker::FixExt4 => {
            let ty = buf.read_i8()?;
            read_ext(buf, 4, ty)
        },
        Marker::FixExt8 => {
            let ty = buf.read_i8()?;
            read_ext(buf, 8, ty)
        },
        Marker::FixExt16 => {
            let ty = buf.read_i8()?;
            read_ext(buf, 16, ty)
        },

        Marker::Str8 => {
            let len = buf.read_u8()? as usize;
            if len <= 31 { return Err(not_shortest()); }
            read_to_str(buf, len)
        }
        Marker::Str16 => {
            let len = buf.read_u16::<BigEndian>()? as usize;
            if len <= (std::u8::MAX as usize) { return Err(not_shortest()); }
            read_to_str(buf, len)
        }
        Marker::Str32 => {
            let len = buf.read_u32::<BigEndian>()? as usize;
            if len <= (std::u16::MAX as usize) { return Err(not_shortest()); }
            read_to_str(buf, len)
        }

        Marker::Array16 => {
            let len = buf.read_u16::<BigEndian>()?;
            if len <= 15 { return Err(not_shortest()); }
            let mut v = Vec::new();
            for _i in 0..len {
                v.push(read_value(buf)?);
            }
            Ok(Value::from(v))
        }
        Marker::Array32 => {
            let len = buf.read_u32::<BigEndian>()?;
            if len <= (std::u16::MAX as u32) { return Err(not_shortest()); }
            let mut v = Vec::new();
            for _i in 0..len {
                v.push(read_value(buf)?);
            }
            Ok(Value::from(v))
        }

        Marker::Map16 => {
            let len = buf.read_u16::<BigEndian>()?;
            if len <= 15 { return Err(not_shortest()); }
            read_to_map(buf, len as usize)
        },
        Marker::Map32 => {
            let len = buf.read_u32::<BigEndian>()?;
            if len <= (std::u16::MAX as u32) { return Err(not_shortest()); }
            read_to_map(buf, len as usize)
        },

        Marker::NegFixInt(val) => {
            Ok(Value::Integer(val.into()))
        }

        Marker::Reserved => Err(io::Error::new(InvalidData, "Unsupported value type")),
    }
}

/// Decode a MessagePack value. Decoding will fail if the value isn't in 
/// condense-db canonical form. That is:
/// - All types are encoded in as few bytes as possible
/// - Positive integers are always encoded using UInt types
/// - Map types always have unique strings as keys
/// - Maps are ordered lexicographically
/// - Strings are valid UTF-8
pub fn read_value_ref<'a>(buf: &mut &'a [u8]) -> io::Result<ValueRef<'a>> {
    let marker = Marker::from_u8(buf.read_u8()?);
    match marker {

        Marker::PosFixInt(val) => Ok(ValueRef::Integer(val.into())),
        Marker::FixMap(len) => {
            read_to_map_ref(buf, len as usize)
        },
        Marker::FixStr(len) => {
            read_to_str_ref(buf, len as usize)
        },
        Marker::FixArray(len) => {
            let mut v = Vec::new();
            for _i in 0..len {
                v.push(read_value_ref(buf)?);
            }
            Ok(ValueRef::Array(v))
        },

        Marker::Nil => Ok(ValueRef::Null),
        Marker::False => Ok(ValueRef::Boolean(false)),
        Marker::True => Ok(ValueRef::Boolean(true)),

        Marker::Bin8 => {
            let len = buf.read_u8()? as usize;
            read_to_bin_ref(buf, len)
        },
        Marker::Bin16 => {
            let len = buf.read_u16::<BigEndian>()? as usize;
            if len <= (std::u8::MAX as usize) { return Err(not_shortest()); }
            read_to_bin_ref(buf, len)
        },
        Marker::Bin32 => {
            let len = buf.read_u32::<BigEndian>()? as usize;
            if len <= (std::u16::MAX as usize) { return Err(not_shortest()); }
            read_to_bin_ref(buf, len)
        },

        Marker::Ext8 => {
            let len = buf.read_u8()? as usize;
            match len {
                1  => Err(not_shortest()),
                2  => Err(not_shortest()),
                4  => Err(not_shortest()),
                8  => Err(not_shortest()),
                16 => Err(not_shortest()),
                _  => {
                    let ty = buf.read_i8()?;
                    read_ext_ref(buf, len, ty)
                }
            }
        },
        Marker::Ext16 => {
            let len = buf.read_u16::<BigEndian>()? as usize;
            if len <= (std::u8::MAX as usize) { return Err(not_shortest()); }
            let ty = buf.read_i8()?;
            read_ext_ref(buf, len, ty)
        },
        Marker::Ext32 => {
            let len = buf.read_u32::<BigEndian>()? as usize;
            if len <= (std::u16::MAX as usize) { return Err(not_shortest()); }
            let ty = buf.read_i8()?;
            read_ext_ref(buf, len, ty)
        },

        Marker::F32 => {
            Ok(ValueRef::F32(buf.read_f32::<BigEndian>()?))
        },

        Marker::F64 => {
            Ok(ValueRef::F64(buf.read_f64::<BigEndian>()?))
        },

        Marker::UInt8 => {
            let val = buf.read_u8()?;
            if val > 127 {
                Ok(ValueRef::Integer(val.into()))
            }
            else {
                Err(not_shortest())
            }
        },
        Marker::UInt16 => {
            let val = buf.read_u16::<BigEndian>()?;
            if val > (std::u8::MAX as u16) {
                Ok(ValueRef::Integer(val.into()))
            }
            else {
                Err(not_shortest())
            }
        },
        Marker::UInt32 => {
            let val = buf.read_u32::<BigEndian>()?;
            if val > (std::u16::MAX as u32) {
                Ok(ValueRef::Integer(val.into()))
            }
            else {
                Err(not_shortest())
            }
        },
        Marker::UInt64 => {
            let val = buf.read_u64::<BigEndian>()?;
            if val > (std::u32::MAX as u64) {
                Ok(ValueRef::Integer(val.into()))
            }
            else {
                Err(not_shortest())
            }
        },

        Marker::Int8 => {
            let val = buf.read_i8()?;
            if val < -32 {
                Ok(ValueRef::Integer(val.into()))
            }
            else if val >= 0 {
                Err(not_negative())
            }
            else {
                Err(not_shortest())
            }
        },
        Marker::Int16 => {
            let val = buf.read_i16::<BigEndian>()?;
            if val < (std::i8::MIN as i16) {
                Ok(ValueRef::Integer(val.into()))
            }
            else if val >= 0 {
                Err(not_negative())
            }
            else {
                Err(not_shortest())
            }
        },
        Marker::Int32 => {
            let val = buf.read_i32::<BigEndian>()?;
            if val < (std::i16::MIN as i32) {
                Ok(ValueRef::Integer(val.into()))
            }
            else if val >= 0 {
                Err(not_negative())
            }
            else {
                Err(not_shortest())
            }
        },
        Marker::Int64 => {
            let val = buf.read_i64::<BigEndian>()?;
            if val < (std::i32::MIN as i64) {
                Ok(ValueRef::Integer(val.into()))
            }
            else if val >= 0 {
                Err(not_negative())
            }
            else {
                Err(not_shortest())
            }
        }

        Marker::FixExt1 => {
            let ty = buf.read_i8()?;
            read_ext_ref(buf, 1, ty)
        },
        Marker::FixExt2 => {
            let ty = buf.read_i8()?;
            read_ext_ref(buf, 2, ty)
        },
        Marker::FixExt4 => {
            let ty = buf.read_i8()?;
            read_ext_ref(buf, 4, ty)
        },
        Marker::FixExt8 => {
            let ty = buf.read_i8()?;
            read_ext_ref(buf, 8, ty)
        },
        Marker::FixExt16 => {
            let ty = buf.read_i8()?;
            read_ext_ref(buf, 16, ty)
        },

        Marker::Str8 => {
            let len = buf.read_u8()? as usize;
            if len <= 31 { return Err(not_shortest()); }
            read_to_str_ref(buf, len)
        }
        Marker::Str16 => {
            let len = buf.read_u16::<BigEndian>()? as usize;
            if len <= (std::u8::MAX as usize) { return Err(not_shortest()); }
            read_to_str_ref(buf, len)
        }
        Marker::Str32 => {
            let len = buf.read_u32::<BigEndian>()? as usize;
            if len <= (std::u16::MAX as usize) { return Err(not_shortest()); }
            read_to_str_ref(buf, len)
        }

        Marker::Array16 => {
            let len = buf.read_u16::<BigEndian>()?;
            if len <= 15 { return Err(not_shortest()); }
            let mut v = Vec::new();
            for _i in 0..len {
                v.push(read_value_ref(buf)?);
            }
            Ok(ValueRef::Array(v))
        }
        Marker::Array32 => {
            let len = buf.read_u32::<BigEndian>()?;
            if len <= (std::u16::MAX as u32) { return Err(not_shortest()); }
            let mut v = Vec::new();
            for _i in 0..len {
                v.push(read_value_ref(buf)?);
            }
            Ok(ValueRef::Array(v))
        }

        Marker::Map16 => {
            let len = buf.read_u16::<BigEndian>()?;
            if len <= 15 { return Err(not_shortest()); }
            read_to_map_ref(buf, len as usize)
        },
        Marker::Map32 => {
            let len = buf.read_u32::<BigEndian>()?;
            if len <= (std::u16::MAX as u32) { return Err(not_shortest()); }
            read_to_map_ref(buf, len as usize)
        },

        Marker::NegFixInt(val) => {
            Ok(ValueRef::Integer(val.into()))
        }

        Marker::Reserved => Err(io::Error::new(InvalidData, "Unsupported value type")),
    }
}

/// General function for reading binary data from a buffer. Checks for if the 
/// length is greater than remaining bytes in the buffer.
fn read_to_bin(buf: &mut &[u8], len: usize) -> io::Result<Value> {
    if buf.len() >= len {
        let (data, rem) = buf.split_at(len);
        *buf = rem;
        Ok(Value::Binary(data.to_vec()))
    }
    else {
        Err(io::Error::new(UnexpectedEof, "Binary length larger than amount of data"))
    }
}

/// General function for referencing binary data in a buffer. Checks for if the 
/// length is greater than remaining bytes in the buffer.
fn read_to_bin_ref<'a>(buf: &mut &'a [u8], len: usize) -> io::Result<ValueRef<'a>> {
    if buf.len() >= len {
        let (data, rem) = buf.split_at(len);
        *buf = rem;
        Ok(ValueRef::Binary(data))
    }
    else {
        Err(io::Error::new(UnexpectedEof, "Binary length larger than amount of data"))
    }
}

/// General function for reading a UTF-8 string from a buffer. Checks for if the 
/// length is greater than remaining bytes in the buffer, or if the bytes 
/// received are not valid UTF-8.
fn read_to_str(buf: &mut &[u8], len: usize) -> io::Result<Value> {
    if buf.len() >= len {
        let (data, rem) = buf.split_at(len);
        *buf = rem;
        let data = data.to_vec();
        let data = String::from_utf8(data)
            .map_err(|_e| io::Error::new(InvalidData, "String decoded is not valid UTF-8"))?;
        Ok(Value::from(data))
    }
    else {
        Err(io::Error::new(UnexpectedEof, "String length larger than amount of data"))
    }
}

/// General function for referencing a UTF-8 string in a buffer. Checks for if the 
/// length is greater than remaining bytes in the buffer, or if the bytes 
/// received are not valid UTF-8.
fn read_to_str_ref<'a>(buf: &mut &'a [u8], len: usize) -> io::Result<ValueRef<'a>> {
    if buf.len() >= len {
        let (data, rem) = buf.split_at(len);
        *buf = rem;
        let data = std::str::from_utf8(data)
            .map_err(|_e| io::Error::new(InvalidData, "String decoded is not valid UTF-8"))?;
        Ok(ValueRef::String(data))
    }
    else {
        Err(io::Error::new(UnexpectedEof, "String length larger than amount of data"))
    }
}

fn try_read_str_ref<'a>(buf: &mut &'a [u8]) -> io::Result<&'a str> {
    let marker = Marker::from_u8(buf.read_u8()?);
    let len = match marker {
        Marker::FixStr(len) => len as usize,
        Marker::Str8 => {
            let len = buf.read_u8()? as usize;
            if len <= 31 { return Err(not_shortest()); }
            len
        },
        Marker::Str16 => {
            let len = buf.read_u16::<BigEndian>()? as usize;
            if len <= (std::u8::MAX as usize) { return Err(not_shortest()); }
            len
        },
        Marker::Str32 => {
            let len = buf.read_u32::<BigEndian>()? as usize;
            if len <= (std::u16::MAX as usize) { return Err(not_shortest()); }
            len
        },
        _ => {
            return Err(io::Error::new(InvalidData, "Object field was not a String"));
        },
    };
    if buf.len() >= len {
        let (data, rem) = buf.split_at(len);
        *buf = rem;
        let data = std::str::from_utf8(data)
            .map_err(|_e| io::Error::new(InvalidData, "String decoded is not valid UTF-8"))?;
        Ok(data)
    }
    else {
        Err(io::Error::new(UnexpectedEof, "String length larger than amount of data"))
    }
}

/// General function for reading a field-value map from a buffer. Checks to make 
/// sure the keys are unique, valid UTF-8 Strings in lexicographic order.
fn read_to_map(buf: &mut &[u8], len: usize) -> io::Result<Value> {

    let mut map: BTreeMap<String,Value> = BTreeMap::new();
    if len == 0 { return Ok(Value::from(map)); }

    // Extract the first field-value pair
    let mut old_key = read_value(buf)?.to_string()
        .ok_or(io::Error::new(InvalidData, "Object field was not a String"))?;
    let val = read_value(buf)?;
    map.insert(old_key.clone(), val);
    // Iterate to get remaining field-value pairs
    for _i in 1..len {
        let key = read_value(buf)?.to_string()
            .ok_or(io::Error::new(InvalidData, "Object field was not a String"))?;
        match old_key.cmp(&key) {
            Ordering::Less => {
                // old_key is lower in order. This is correct
                let val = read_value(buf)?;
                map.insert(key.clone(), val);
            },
            Ordering::Equal => {
                return Err(io::Error::new(InvalidData, format!("Found object with non-unique field \"{}\"", key)));
            },
            Ordering::Greater => {
                return Err(io::Error::new(InvalidData, "Fields in object are not in lexicographic order"));
            }
        };
        old_key = key;
    }
    Err(io::Error::new(InvalidData, "Fields in object are not unique or in lexicographic order"))
}

/// General function for referencing a field-value map in a buffer. Checks to make 
/// sure the keys are unique, valid UTF-8 Strings in lexicographic order.
fn read_to_map_ref<'a>(buf: &mut &'a [u8], len: usize) -> io::Result<ValueRef<'a>> {

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
                return Err(io::Error::new(InvalidData, format!("Found object with non-unique field \"{}\"", key)));
            },
            Ordering::Greater => {
                return Err(io::Error::new(InvalidData, "Fields in object are not in lexicographic order"));
            }
        };
        old_key = key;
    }
    Err(io::Error::new(InvalidData, "Fields in object are not unique or in lexicographic order"))
}

/// Reads any of the supported extension types out. Fails if extension type 
/// isn't recognized, or the extension type decoding fails. The latter can 
/// happen if the crypto version number is not used by this library.
fn read_ext(buf: &mut &[u8], len: usize, ty: i8) -> io::Result<Value> {
    let ext_type = ExtType::from_i8(ty).ok_or(io::Error::new(InvalidData, "Unsupported Extension type"))?;
    match ext_type {
        ExtType::Timestamp => {
            match len {
                4 => {
                    let sec = buf.read_u32::<BigEndian>()?;
                    Ok(Value::from(Timestamp::from_sec(sec as i64)))
                },
                8 => {
                    let raw_time = buf.read_u64::<BigEndian>()?;
                    let sec = (raw_time & 0x3FFFF_FFFFu64) as i64;
                    let nano = (raw_time >> 34) as u32;
                    Ok(Value::from(
                        Timestamp::from_raw(sec,nano).ok_or(io::Error::new(InvalidData, "Timestamp nanoseconds is too big"))?))
                },
                12 => {
                    let nano = buf.read_u32::<BigEndian>()?;
                    let sec = buf.read_i64::<BigEndian>()?;
                    Ok(Value::from(
                        Timestamp::from_raw(sec,nano).ok_or(io::Error::new(InvalidData, "Timestamp nanoseconds is too big"))?))
                },
                _ => Err(io::Error::new(InvalidData, "Timestamp type has invalid size"))
            }
        },
        ExtType::Hash => {
            let hash = Hash::decode(buf).map_err(|_e| io::Error::new(InvalidData, "Hash not recognized"))?;
            if hash.len() != len {
                Err(io::Error::new(InvalidData, "Hash type has invalid size"))
            }
            else {
                Ok(Value::from(hash))
            }
        }
        ExtType::Identity => {
            let id = Identity::decode(buf).map_err(|_e| io::Error::new(InvalidData, "Identity not recognized"))?;
            if id.len() != len {
                Err(io::Error::new(InvalidData, "Identity type has invalid size"))
            }
            else {
                Ok(Value::from(id))
            }
        }
        ExtType::Lockbox => {
            let lock = Lockbox::decode(len, buf).map_err(|_e| io::Error::new(InvalidData, "Lockbox not recognized"))?;
            Ok(Value::from(lock))
        }
    }
}

/// Reads any of the supported extension types out. Fails if extension type 
/// isn't recognized, or the extension type decoding fails. The latter can 
/// happen if the crypto version number is not used by this library.
fn read_ext_ref<'a>(buf: &mut &'a [u8], len: usize, ty: i8) -> io::Result<ValueRef<'a>> {
    let ext_type = ExtType::from_i8(ty).ok_or(io::Error::new(InvalidData, "Unsupported Extension type"))?;
    match ext_type {
        ExtType::Timestamp => {
            match len {
                4 => {
                    let sec = buf.read_u32::<BigEndian>()?;
                    Ok(ValueRef::Timestamp(Timestamp::from_sec(sec as i64)))
                },
                8 => {
                    let raw_time = buf.read_u64::<BigEndian>()?;
                    let sec = (raw_time & 0x3FFFF_FFFFu64) as i64;
                    let nano = (raw_time >> 34) as u32;
                    Ok(ValueRef::Timestamp(
                        Timestamp::from_raw(sec,nano).ok_or(io::Error::new(InvalidData, "Timestamp nanoseconds is too big"))?))
                },
                12 => {
                    let nano = buf.read_u32::<BigEndian>()?;
                    let sec = buf.read_i64::<BigEndian>()?;
                    Ok(ValueRef::Timestamp(
                        Timestamp::from_raw(sec,nano).ok_or(io::Error::new(InvalidData, "Timestamp nanoseconds is too big"))?))
                },
                _ => Err(io::Error::new(InvalidData, "Timestamp type has invalid size"))
            }
        },
        ExtType::Hash => {
            let hash = Hash::decode(buf).map_err(|_e| io::Error::new(InvalidData, "Hash not recognized"))?;
            if hash.len() != len {
                Err(io::Error::new(InvalidData, "Hash type has invalid size"))
            }
            else {
                Ok(ValueRef::Hash(hash))
            }
        }
        ExtType::Identity => {
            let id = Identity::decode(buf).map_err(|_e| io::Error::new(InvalidData, "Identity not recognized"))?;
            if id.len() != len {
                Err(io::Error::new(InvalidData, "Identity type has invalid size"))
            }
            else {
                Ok(ValueRef::Identity(id))
            }
        }
        ExtType::Lockbox => {
            let lock = Lockbox::decode(len, buf).map_err(|_e| io::Error::new(InvalidData, "Lockbox not recognized"))?;
            Ok(ValueRef::Lockbox(lock))
        }
    }
}
