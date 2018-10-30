use std::fmt;
use rmpv::Value;
use byteorder::{self,WriteBytesExt, ReadBytesExt};

use crypto::ext_type::ExtType;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Index (u64,u64);

impl Index {
    pub fn encode(&self) -> Value {
        let mut enc = Vec::new();
        if self.1 == 0 {
            if self.0 < ::std::u8::MAX as u64 {
                enc.write_u8(self.0 as u8).unwrap();
            }
            else if self.0 < ::std::u16::MAX as u64 {
                enc.write_u16::<byteorder::BigEndian>(self.0 as u16).unwrap();
            }
            else if self.0 < ::std::u32::MAX as u64 {
                enc.write_u32::<byteorder::BigEndian>(self.0 as u32).unwrap();
            }
            else {
                enc.write_u64::<byteorder::BigEndian>(self.0).unwrap();
            }
        }
        else {
            enc.write_u64::<byteorder::BigEndian>(self.1).unwrap();
            enc.write_u64::<byteorder::BigEndian>(self.0).unwrap();
        }
        Value::Ext(ExtType::Index.to_i8(), enc)
    }

    pub fn valid_ext(ext: i8, len: usize) -> bool {
        ext == ExtType::Index.to_i8() &&
            (len == 1 || len == 2 || len == 4 || len == 8 || len ==16)
    }


    pub fn decode(v: &Value) -> Option<Index> {
        match v {
            Value::Ext(ext, ref data) => {
                let len = data.len();
                if *ext == ExtType::Index.to_i8() {
                    if len == 1 {
                        let mut r = &data[..];
                        Some(Index (0, r.read_u8().unwrap_or(0u8) as u64))
                    }
                    else if len == 2 {
                        let mut r = &data[..];
                        Some(Index (0, r.read_u16::<byteorder::BigEndian>().unwrap_or(0u16) as u64))
                    }
                    else if len == 4 {
                        let mut r = &data[..];
                        Some(Index (0, r.read_u32::<byteorder::BigEndian>().unwrap_or(0u32) as u64))
                    }
                    else if len == 8 {
                        let mut r = &data[..];
                        Some(Index (0, r.read_u64::<byteorder::BigEndian>().unwrap_or(0u64)))
                    }
                    else if len == 16 {
                        let mut r = &data[..];
                        let v1 = r.read_u64::<byteorder::BigEndian>().unwrap_or(0u64);
                        let v0 = r.read_u64::<byteorder::BigEndian>().unwrap_or(0u64);
                        Some(Index (v1, v0))
                    }
                    else {
                        None
                    }
                }
                else {
                    None
                }
            },
            _ => None,
        }
    }
}

impl fmt::Display for Index {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{},{}", self.1, self.0)
    }
}
