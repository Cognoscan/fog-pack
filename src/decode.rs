use super::{Value, Hash, Identity, Lockbox, Timestamp};
use Marker;
use ExtType;

use byteorder::{ReadBytesExt, BigEndian};

use std::io::ErrorKind::{InvalidData, UnexpectedEof};

use std::io;

fn not_shortest() -> io::Error {
    io::Error::new(InvalidData, "Not shortest possible encoding")
}

/// Decode a MessagePack value. Decoding will fail if the value isn't in 
/// canonical shortest form.
pub fn read_value(buf: &mut &[u8]) -> io::Result<Value> {
    let marker = Marker::from_u8(buf.read_u8()?);
    match marker {
        Marker::Nil => Ok(Value::Null),
        Marker::False => Ok(Value::Boolean(false)),
        Marker::True => Ok(Value::Boolean(true)),
        Marker::Bin8 => {
            let len = buf.read_u8()? as usize;
            read_to_bin(buf, len)
        }
        Marker::Bin16 => {
            let len = buf.read_u16::<BigEndian>()? as usize;
            if len <= (std::u8::MAX as usize) { return Err(not_shortest()); }
            read_to_bin(buf, len)
        }
        Marker::Bin32 => {
            let len = buf.read_u32::<BigEndian>()? as usize;
            if len <= (std::u16::MAX as usize) { return Err(not_shortest()); }
            read_to_bin(buf, len)
        }
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
        Marker::Reserved => Err(io::Error::new(InvalidData, "Unsupported value type")),
        _ => Err(io::Error::new(InvalidData, "Unsupported value type")),
    }
}

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





