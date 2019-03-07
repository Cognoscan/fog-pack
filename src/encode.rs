use super::Value;
use Marker;
use integer;
use ExtType;

/// Write the MessagePack value out to a Vector. This code assumes that all strings, binary data, 
/// objects, and arrays are less than 2^32 elements in size.
pub fn write_value(buf: &mut Vec<u8>, val: &Value) {
    match *val {

        Value::Null => {
            buf.push(Marker::Nil.into())
        },

        Value::Boolean(val) => {
            if val {
                buf.push(Marker::True.into())
            } else {
                buf.push(Marker::False.into())
            }
        },

        Value::Integer(ref val) => {
            match integer::get_int_internal(val) {
                integer::IntPriv::PosInt(u) => {
                    if u <= 127 {
                        buf.push(Marker::PosFixInt(u as u8).into());
                    }
                    else if u <= std::u8::MAX as u64 {
                        buf.push(Marker::UInt8.into());
                        buf.push(u as u8);
                    }
                    else if u <= std::u16::MAX as u64 {
                        buf.push(Marker::UInt16.into());
                        buf.extend_from_slice(&(u as u16).to_be_bytes());
                    }
                    else if u <= std::u32::MAX as u64 {
                        buf.push(Marker::UInt32.into());
                        buf.extend_from_slice(&(u as u32).to_be_bytes());
                    }
                    else {
                        buf.push(Marker::UInt64.into());
                        buf.extend_from_slice(&u.to_be_bytes());
                    }
                }
                integer::IntPriv::NegInt(u) => {
                    if u >= -32 {
                        buf.push(Marker::NegFixInt(u as i8).into());
                    }
                    else if u >= 255 {
                        buf.push(Marker::Int8.into());
                        buf.push(u as u8);
                    }
                    else if u >= std::i16::MIN as i64 {
                        buf.push(Marker::Int16.into());
                        buf.extend_from_slice(&(u as i16).to_be_bytes());
                    }
                    else if u >= std::i32::MIN as i64 {
                        buf.push(Marker::Int32.into());
                        buf.extend_from_slice(&(u as i32).to_be_bytes());
                    }
                    else {
                        buf.push(Marker::Int64.into());
                        buf.extend_from_slice(&u.to_be_bytes());
                    }
                }
            }
        },

        Value::String(ref val) => {
            write_string(buf, val);
        },

        Value::F32(val) => {
            buf.push(Marker::F32.into());
            buf.extend_from_slice(&val.to_bits().to_be_bytes());
        },

        Value::F64(val) => {
            buf.push(Marker::F64.into());
            buf.extend_from_slice(&val.to_bits().to_be_bytes());
        },

        Value::Binary(ref val) => {
            let len = val.len() as u32;
            if len <= std::u8::MAX as u32 {
                buf.push(Marker::Bin8.into());
                buf.push(len as u8);
            }
            else if len <= std::u16::MAX as u32 {
                buf.push(Marker::Bin16.into());
                buf.extend_from_slice(&(len as u16).to_be_bytes());
            }
            else {
                buf.push(Marker::Bin32.into());
                buf.extend_from_slice(&len.to_be_bytes());
            }
            buf.extend_from_slice(val);
        },

        Value::Array(ref val) => {
            // Write marker
            let len = val.len() as u32;
            if len <= 15 {
                buf.push(Marker::FixArray(len as u8).into());
            }
            else if len <= std::u16::MAX as u32 {
                buf.push(Marker::Array16.into());
                buf.extend_from_slice(&(len as u16).to_be_bytes());
            }
            else {
                buf.push(Marker::Array32.into());
                buf.extend_from_slice(&len.to_be_bytes());
            }
            // Write each value out
            for item in val.iter() {
                write_value(buf, item);
            }
        },

        Value::Object(ref val) => {
            // Write marker
            let len = val.len() as u32;
            if len <= 15 {
                buf.push(Marker::FixMap(len as u8).into());
            }
            else if len <= std::u16::MAX as u32 {
                buf.push(Marker::Map16.into());
                buf.extend_from_slice(&(len as u16).to_be_bytes());
            }
            else {
                buf.push(Marker::Map32.into());
                buf.extend_from_slice(&len.to_be_bytes());
            }
            // Write each value out. Because val is a BTreeMap of strings, the iterator should go 
            // through them in lexicographic order.
            for (key, content) in val.iter() {
                write_string(buf, key);
                write_value(buf, content);
            }
        },

        Value::Hash(ref val) => {
            write_ext_marker(buf, val.len() as u32);
            buf.push(ExtType::Hash.into());
            val.encode(buf);
        }

        Value::Identity(ref val) => {
            write_ext_marker(buf, val.len() as u32);
            buf.push(ExtType::Identity.into());
            val.encode(buf);
        },

        Value::Lockbox(ref val) => {
            write_ext_marker(buf, val.len() as u32);
            buf.push(ExtType::Lockbox.into());
            val.encode(buf);
        },

        Value::Timestamp(val) => {
            let sec = val.timestamp();
            let nano = val.timestamp_subsec_nanos();

            if (sec < (std::u32::MAX as i64)) && (sec >= 0) && (nano == 0) {
                // 32-bit unsigned seconds
                buf.push(Marker::FixExt4.into());
                buf.push(ExtType::Timestamp.into());
                buf.extend_from_slice(&(sec as u32).to_be_bytes());
            }
            else if (sec >= 0) && (sec < ((1i64 << 34)-1)) && (nano < ((1u32 << 30)-1)) {
                // 64-bit unsigned seconds with nanoseconds
                buf.push(Marker::FixExt8.into());
                buf.push(ExtType::Timestamp.into());
                let data: u64 = (sec as u64) | ((nano as u64) << 34);
                buf.extend_from_slice(&data.to_be_bytes());
            }
            else {
                // 96-bit signed seconds with nanoseconds
                buf.push(Marker::Ext8.into());
                buf.push(12u8);
                buf.push(ExtType::Timestamp.into());
                buf.extend_from_slice(&nano.to_be_bytes());
                buf.extend_from_slice(&sec.to_be_bytes());
            }
        }
    }
}

fn write_string(buf: &mut Vec<u8>, val: &String) {
    let len = val.len() as u32;
    if len <= 31 { buf.push(Marker::FixStr(len as u8).into());
    }
    else if len <= std::u8::MAX as u32 {
        buf.push(Marker::Str8.into());
        buf.push(len as u8);
    }
    else if len <= std::u16::MAX as u32 {
        buf.push(Marker::Str16.into());
        buf.extend_from_slice(&(len as u16).to_be_bytes());
    }
    else {
        buf.push(Marker::Str32.into());
        buf.extend_from_slice(&len.to_be_bytes());
    }
    buf.extend_from_slice(val.as_bytes());
}

fn write_ext_marker(buf: &mut Vec<u8>, len: u32) {
    match len {
        1 => buf.push(Marker::FixExt1.into()),
        2 => buf.push(Marker::FixExt2.into()),
        4 => buf.push(Marker::FixExt4.into()),
        8 => buf.push(Marker::FixExt8.into()),
        16 => buf.push(Marker::FixExt16.into()),
        len if len < (std::u8::MAX as u32) => {
            buf.push(Marker::Ext8.into());
            buf.push(len as u8);
        },
        len if len < (std::u16::MAX as u32) => {
            buf.push(Marker::Ext16.into());
            buf.extend_from_slice(&(len as u16).to_be_bytes());
        },
        len => {
            buf.push(Marker::Ext32.into());
            buf.extend_from_slice(&(len as u32).to_be_bytes());
        },
    };
}
