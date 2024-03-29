use std::{convert::TryFrom, fmt::Debug};

use crate::{depth_tracking::DepthTracker, marker::*, MAX_DOC_SIZE};
use crate::{
    error::{Error, Result},
    get_int_internal, integer, Integer, Timestamp,
};
use fog_crypto::identity::BareIdKey;
use fog_crypto::{
    hash::Hash,
    identity::Identity,
    lock::LockId,
    lockbox::{DataLockboxRef, IdentityLockboxRef, LockLockboxRef, StreamLockboxRef},
    stream::StreamId,
};
use serde::de::Unexpected;

use byteorder::{LittleEndian, ReadBytesExt};

fn base64_encode<T: AsRef<[u8]>>(input: T, output_buf: &mut String) {
    use base64::engine::Engine;
    base64::engine::general_purpose::STANDARD_NO_PAD.encode_string(input, output_buf)
}

#[derive(Clone, Debug)]
pub enum Element<'a> {
    Null,
    Bool(bool),
    Int(Integer),
    Str(&'a str),
    F32(f32),
    F64(f64),
    Bin(&'a [u8]),
    Array(usize),
    Map(usize),
    Timestamp(Timestamp),
    Hash(Hash),
    Identity(Box<Identity>),
    LockId(Box<LockId>),
    StreamId(Box<StreamId>),
    DataLockbox(&'a DataLockboxRef),
    IdentityLockbox(&'a IdentityLockboxRef),
    StreamLockbox(&'a StreamLockboxRef),
    LockLockbox(&'a LockLockboxRef),
    BareIdKey(Box<BareIdKey>),
}

impl<'a> Element<'a> {
    pub fn name(&self) -> &'static str {
        use self::Element::*;
        match self {
            Null => "Null",
            Bool(_) => "Bool",
            Int(_) => "Int",
            Str(_) => "Str",
            F32(_) => "F32",
            F64(_) => "F64",
            Bin(_) => "Bin",
            Array(_) => "Array",
            Map(_) => "Map",
            Timestamp(_) => "Time",
            Hash(_) => "Hash",
            Identity(_) => "Identity",
            LockId(_) => "LockId",
            StreamId(_) => "StreamId",
            DataLockbox(_) => "DataLockbox",
            IdentityLockbox(_) => "IdentityLockbox",
            StreamLockbox(_) => "StreamLockbox",
            LockLockbox(_) => "LockLockbox",
            BareIdKey(_) => "BareIdKey",
        }
    }

    pub fn unexpected(&self) -> Unexpected {
        use self::Element::*;
        match self {
            Null => Unexpected::Unit,
            Bool(v) => Unexpected::Bool(*v),
            Int(v) => match get_int_internal(v) {
                integer::IntPriv::PosInt(v) => Unexpected::Unsigned(v),
                integer::IntPriv::NegInt(v) => Unexpected::Signed(v),
            },
            Str(v) => Unexpected::Str(v),
            F32(v) => Unexpected::Float(*v as f64),
            F64(v) => Unexpected::Float(*v),
            Bin(v) => Unexpected::Bytes(v),
            Array(_) => Unexpected::Seq,
            Map(_) => Unexpected::Map,
            Timestamp(_) => Unexpected::Other("Timestamp"),
            Hash(_) => Unexpected::Other("Hash"),
            Identity(_) => Unexpected::Other("Identity"),
            LockId(_) => Unexpected::Other("LockId"),
            StreamId(_) => Unexpected::Other("StreamId"),
            DataLockbox(_) => Unexpected::Other("DataLockbox"),
            IdentityLockbox(_) => Unexpected::Other("IdentityLockbox"),
            StreamLockbox(_) => Unexpected::Other("StreamLockbox"),
            LockLockbox(_) => Unexpected::Other("LockLockbox"),
            BareIdKey(_) => Unexpected::Other("BareIdKey"),
        }
    }
}

/// Serialize an element onto a byte vector. Doesn't check if Array & Map structures make
/// sense, just writes elements out.
pub fn serialize_elem(buf: &mut Vec<u8>, elem: Element) {
    use self::Element::*;
    match elem {
        Null => buf.push(Marker::Null.into()),
        Bool(v) => buf.push(if v { Marker::True } else { Marker::False }.into()),
        Int(v) => match integer::get_int_internal(&v) {
            integer::IntPriv::PosInt(v) => {
                if v <= 127 {
                    buf.push(Marker::PosFixInt(v as u8).into());
                } else if v <= u8::MAX as u64 {
                    buf.push(Marker::UInt8.into());
                    buf.push(v as u8);
                } else if v <= u16::MAX as u64 {
                    buf.push(Marker::UInt16.into());
                    buf.extend_from_slice(&(v as u16).to_le_bytes());
                } else if v <= u32::MAX as u64 {
                    buf.push(Marker::UInt32.into());
                    buf.extend_from_slice(&(v as u32).to_le_bytes());
                } else {
                    buf.push(Marker::UInt64.into());
                    buf.extend_from_slice(&v.to_le_bytes());
                }
            }
            integer::IntPriv::NegInt(v) => {
                if v >= -32 {
                    buf.push(Marker::NegFixInt(v as i8).into());
                } else if v >= i8::MIN as i64 {
                    buf.push(Marker::Int8.into());
                    buf.push(v as u8);
                } else if v >= i16::MIN as i64 {
                    buf.push(Marker::Int16.into());
                    buf.extend_from_slice(&(v as i16).to_le_bytes());
                } else if v >= i32::MIN as i64 {
                    buf.push(Marker::Int32.into());
                    buf.extend_from_slice(&(v as i32).to_le_bytes());
                } else {
                    buf.push(Marker::Int64.into());
                    buf.extend_from_slice(&v.to_le_bytes());
                }
            }
        },
        Str(v) => {
            let len = v.len();
            assert!(len <= MAX_DOC_SIZE);
            if len <= 31 {
                buf.push(Marker::FixStr(len as u8).into());
            } else if len <= u8::MAX as usize {
                buf.push(Marker::Str8.into());
                buf.push(len as u8);
            } else if len <= u16::MAX as usize {
                buf.push(Marker::Str16.into());
                buf.extend_from_slice(&len.to_le_bytes()[..2]);
            } else {
                buf.push(Marker::Str24.into());
                buf.extend_from_slice(&len.to_le_bytes()[..3]);
            }
            buf.extend_from_slice(v.as_bytes());
        }
        F32(v) => {
            buf.push(Marker::F32.into());
            buf.extend_from_slice(&v.to_bits().to_le_bytes());
        }
        F64(v) => {
            buf.push(Marker::F64.into());
            buf.extend_from_slice(&v.to_bits().to_le_bytes());
        }
        Bin(v) => {
            let len = v.len();
            assert!(len <= MAX_DOC_SIZE);
            if len <= u8::MAX as usize {
                buf.push(Marker::Bin8.into());
                buf.push(len as u8);
            } else if len <= u16::MAX as usize {
                buf.push(Marker::Bin16.into());
                buf.extend_from_slice(&len.to_le_bytes()[..2]);
            } else {
                buf.push(Marker::Bin24.into());
                buf.extend_from_slice(&len.to_le_bytes()[..3]);
            }
            buf.extend_from_slice(v);
        }
        Array(len) => {
            assert!(len <= MAX_DOC_SIZE);
            // Write marker
            if len <= 15 {
                buf.push(Marker::FixArray(len as u8).into());
            } else if len <= u8::MAX as usize {
                buf.push(Marker::Array8.into());
                buf.push(len as u8);
            } else if len <= u16::MAX as usize {
                buf.push(Marker::Array16.into());
                buf.extend_from_slice(&len.to_le_bytes()[..2]);
            } else {
                buf.push(Marker::Array24.into());
                buf.extend_from_slice(&len.to_le_bytes()[..3]);
            }
        }
        Map(len) => {
            assert!(len <= MAX_DOC_SIZE / 2);
            // Write marker
            if len <= 15 {
                buf.push(Marker::FixMap(len as u8).into());
            } else if len <= u8::MAX as usize {
                buf.push(Marker::Map8.into());
                buf.push(len as u8);
            } else if len <= u16::MAX as usize {
                buf.push(Marker::Map16.into());
                buf.extend_from_slice(&len.to_le_bytes()[..2]);
            } else {
                buf.push(Marker::Map24.into());
                buf.extend_from_slice(&len.to_le_bytes()[..3]);
            }
        }
        Timestamp(v) => {
            Marker::encode_ext_marker(buf, v.size());
            buf.push(ExtType::Timestamp.into());
            v.encode_vec(buf);
        }
        Hash(v) => {
            let v = v.as_ref();
            Marker::encode_ext_marker(buf, v.len());
            buf.push(ExtType::Hash.into());
            buf.extend_from_slice(v);
        }
        Identity(v) => {
            Marker::encode_ext_marker(buf, v.size());
            buf.push(ExtType::Identity.into());
            v.encode_vec(buf);
        }
        LockId(v) => {
            Marker::encode_ext_marker(buf, v.size());
            buf.push(ExtType::LockId.into());
            v.encode_vec(buf);
        }
        StreamId(v) => {
            Marker::encode_ext_marker(buf, v.size());
            buf.push(ExtType::StreamId.into());
            v.encode_vec(buf);
        }
        DataLockbox(v) => {
            let v = v.as_bytes();
            Marker::encode_ext_marker(buf, v.len());
            buf.push(ExtType::DataLockbox.into());
            buf.extend_from_slice(v);
        }
        IdentityLockbox(v) => {
            let v = v.as_bytes();
            Marker::encode_ext_marker(buf, v.len());
            buf.push(ExtType::IdentityLockbox.into());
            buf.extend_from_slice(v);
        }
        StreamLockbox(v) => {
            let v = v.as_bytes();
            Marker::encode_ext_marker(buf, v.len());
            buf.push(ExtType::StreamLockbox.into());
            buf.extend_from_slice(v);
        }
        LockLockbox(v) => {
            let v = v.as_bytes();
            Marker::encode_ext_marker(buf, v.len());
            buf.push(ExtType::LockLockbox.into());
            buf.extend_from_slice(v);
        }
        BareIdKey(v) => {
            Marker::encode_ext_marker(buf, v.size());
            buf.push(ExtType::BareIdKey.into());
            v.encode_vec(buf);
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum TrackType {
    FirstArray(usize),
    FirstMap(usize),
    Array(usize),
    Map(usize),
}

#[derive(Clone, Debug)]
struct DebugFormatter {
    debug: String,
    tracker: Vec<TrackType>,
    indent: String,
}

impl DebugFormatter {
    fn new(indent: String) -> Self {
        Self {
            debug: String::new(),
            tracker: Vec::new(),
            indent,
        }
    }

    fn get_str(&self) -> &str {
        &self.debug
    }

    fn indent(&mut self) {
        match self.tracker.last() {
            Some(TrackType::FirstArray(_)) => (),
            Some(TrackType::FirstMap(_)) => (),
            _ => {
                for _ in 0..self.tracker.len() {
                    self.debug.push_str(&self.indent)
                }
            }
        }
    }

    fn update(&mut self, elem: &Element) {
        use std::fmt::Write;

        self.indent();
        match elem {
            Element::Null => self.debug.push_str("null"),
            Element::Bool(v) => self.debug.push_str(if *v { "true" } else { "false" }),
            Element::Int(v) => write!(self.debug, "{}", v).unwrap(),
            Element::Str(v) => write!(self.debug, "\"{}\"", v.escape_debug()).unwrap(),
            Element::F32(v) => write!(self.debug, "{}", v).unwrap(),
            Element::F64(v) => write!(self.debug, "{}", v).unwrap(),
            Element::Bin(v) => {
                self.debug.push_str("\"$fog-Bin:");
                base64_encode(*v, &mut self.debug);
                self.debug.push('"');
            }
            Element::Array(v) => {
                self.tracker.push(TrackType::FirstArray(*v));
                self.debug.push('[');
            }
            Element::Map(v) => {
                self.tracker.push(TrackType::FirstMap(*v * 2));
                self.debug.push('{');
            }
            Element::Timestamp(v) => write!(self.debug, "\"$fog-Time: {}\"", v).unwrap(),
            Element::Hash(v) => write!(self.debug, "\"$fog-Hash:{}\"", v).unwrap(),
            Element::Identity(v) => write!(self.debug, "\"$fog-Identity:{}\"", v).unwrap(),
            Element::LockId(v) => write!(self.debug, "\"$fog-LockId:{}\"", v).unwrap(),
            Element::StreamId(v) => write!(self.debug, "\"$fog-StreamId:{}\"", v).unwrap(),
            Element::DataLockbox(v) => write!(
                self.debug,
                "\"$fog-DataLockbox(len={})\"",
                v.as_bytes().len()
            )
            .unwrap(),
            Element::IdentityLockbox(v) => write!(
                self.debug,
                "\"$fog-IdentityLockbox(len={})\"",
                v.as_bytes().len()
            )
            .unwrap(),
            Element::StreamLockbox(v) => write!(
                self.debug,
                "\"$fog-StreamLockbox(len={})\"",
                v.as_bytes().len()
            )
            .unwrap(),
            Element::LockLockbox(v) => write!(
                self.debug,
                "\"$fog-LockLockbox(len={})\"",
                v.as_bytes().len()
            )
            .unwrap(),
            Element::BareIdKey(v) => {
                write!(self.debug, "\"$fog-BareIdKey:{}\"", v.to_base58()).unwrap()
            }
        }

        while let Some(track) = self.tracker.pop() {
            match track {
                TrackType::FirstArray(size) => {
                    if size == 0 {
                        self.debug.push_str(" ]");
                    } else if size == 1 {
                        self.debug.push(' ');
                        self.tracker.push(TrackType::FirstArray(size - 1));
                        break;
                    } else {
                        self.debug.push('\n');
                        self.tracker.push(TrackType::Array(size - 1));
                        break;
                    }
                }
                TrackType::Array(size) => {
                    if size == 0 {
                        self.debug.push('\n');
                        self.indent();
                        self.debug.push(']');
                    } else {
                        self.debug.push_str(",\n");
                        self.tracker.push(TrackType::Array(size - 1));
                        break;
                    }
                }
                TrackType::FirstMap(size) => {
                    if size == 0 {
                        self.debug.push_str(" }");
                    } else if size == 1 {
                        self.debug.push_str(": ");
                        self.tracker.push(TrackType::FirstMap(size - 1));
                        break;
                    } else if size == 2 {
                        self.debug.push(' ');
                        self.tracker.push(TrackType::FirstMap(size - 1));
                        break;
                    } else {
                        self.debug.push('\n');
                        self.tracker.push(TrackType::Map(size - 1));
                        break;
                    }
                }
                TrackType::Map(size) => {
                    if size == 0 {
                        self.debug.push('\n');
                        self.indent();
                        self.debug.push('}');
                    } else if (size & 0x1) == 1 {
                        self.debug.push_str(": ");
                        self.tracker.push(TrackType::Map(size - 1));
                        break;
                    } else {
                        self.debug.push_str(",\n");
                        self.tracker.push(TrackType::Map(size - 1));
                        break;
                    }
                }
            }
        }
    }
}

/// Fog-pack element parser. Return individual elements of a fog-pack sequence, and checks for
/// nesting depth limits.
///
/// Users of the parser *must* verify that map keys are strings, and that they are in lexicographic
/// order.
#[derive(Clone, Debug)]
pub struct Parser<'a> {
    debug: Option<DebugFormatter>,
    data: &'a [u8],
    depth_tracking: DepthTracker,
    errored: bool,
}

impl<'a> Parser<'a> {
    /// Turn a byte slice into a new parser.
    pub fn new(data: &'a [u8]) -> Parser<'a> {
        Self {
            debug: None,
            data,
            depth_tracking: DepthTracker::new(),
            errored: false,
        }
    }

    /// Turn a byte slice into a new parser, with a debugging pretty-printer that will run as
    /// elements are parsed.
    #[allow(dead_code)]
    pub fn with_debug(data: &'a [u8], indent: impl Into<String>) -> Parser<'a> {
        Self {
            debug: Some(DebugFormatter::new(indent.into())),
            data,
            depth_tracking: DepthTracker::new(),
            errored: false,
        }
    }

    /// Look at what the next marker byte to be parsed will be.
    pub fn peek_marker(&self) -> Option<Marker> {
        self.data.first().map(|n| Marker::from_u8(*n))
    }

    /// Call when parsing is expected to be complete. Fails if there are any bytes left inside the
    /// parser.
    pub fn finish(self) -> Result<()> {
        if self.data.is_empty() {
            Ok(())
        } else {
            Err(Error::BadEncode(format!(
                "Parsing still had {} bytes left",
                self.data.len()
            )))
        }
    }

    #[allow(dead_code)]
    pub fn get_debug(&self) -> Option<&str> {
        match self.debug {
            None => None,
            Some(ref dbg) => Some(dbg.get_str()),
        }
    }

    /// Peek the next element. This doesn't update any of the internal state
    /// tracking, which means it also doesn't track the depth or error
    /// condition.
    pub fn peek(&self) -> Option<Result<Element<'a>>> {
        let data: &[u8] = self.data;
        if self.errored {
            return None;
        }
        let (&marker, mut data) = data.split_first()?;
        Some(Self::get_element(&mut data, Marker::from_u8(marker)))
    }

    fn parse_element(&mut self, marker: Marker) -> Result<Element<'a>> {
        let elem = Self::get_element(&mut self.data, marker)?;
        if let Some(ref mut debug) = self.debug {
            debug.update(&elem);
        }
        self.depth_tracking.update_elem(&elem)?;
        Ok(elem)
    }

    // Given a retrieved marker, try to turn it into the next element, which may move through the
    // indexed data. If we can't, error. This function *does not* set the the errored flag. That's
    // up to the caller.
    fn get_element(data: &mut &'a [u8], marker: Marker) -> Result<Element<'a>> {
        use self::Marker::*;
        Ok(match marker {
            Reserved => return Err(Error::BadEncode(String::from("Reserved marker found"))),
            Null => Element::Null,
            False => Element::Bool(false),
            True => Element::Bool(true),
            PosFixInt(v) => Element::Int(v.into()),
            UInt8 => {
                let v = data.read_u8().map_err(|_| Error::LengthTooShort {
                    step: "decode UInt8",
                    actual: 0,
                    expected: 1,
                })?;
                if v < 128 {
                    return Err(Error::BadEncode(format!(
                        "Got UInt8 with value = {}. This is not the shortest encoding.",
                        v
                    )));
                }
                Element::Int(v.into())
            }
            UInt16 => {
                let v = data
                    .read_u16::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode UInt16",
                        actual: data.len(),
                        expected: 2,
                    })?;
                if v <= u8::MAX as u16 {
                    return Err(Error::BadEncode(format!(
                        "Got UInt16 with value = {}. This is not the shortest encoding.",
                        v
                    )));
                }
                Element::Int(v.into())
            }
            UInt32 => {
                let v = data
                    .read_u32::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode UInt32",
                        actual: data.len(),
                        expected: 4,
                    })?;
                if v <= u16::MAX as u32 {
                    return Err(Error::BadEncode(format!(
                        "Got UInt32 with value = {}. This is not the shortest encoding.",
                        v
                    )));
                }
                Element::Int(v.into())
            }
            UInt64 => {
                let v = data
                    .read_u64::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode UInt64",
                        actual: data.len(),
                        expected: 8,
                    })?;
                if v <= u32::MAX as u64 {
                    return Err(Error::BadEncode(format!(
                        "Got UInt64 with value = {}. This is not the shortest encoding.",
                        v
                    )));
                }
                Element::Int(v.into())
            }
            NegFixInt(v) => Element::Int(v.into()),
            Int8 => {
                let v = data.read_i8().map_err(|_| Error::LengthTooShort {
                    step: "decode UInt8",
                    actual: 0,
                    expected: 1,
                })?;
                if v >= -32 {
                    return Err(Error::BadEncode(format!(
                        "Got Int8 with value = {}. This is not the shortest encoding.",
                        v
                    )));
                }
                Element::Int(v.into())
            }
            Int16 => {
                let v = data
                    .read_i16::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode Int16",
                        actual: data.len(),
                        expected: 2,
                    })?;
                if v >= i8::MIN as i16 {
                    return Err(Error::BadEncode(format!(
                        "Got Int16 with value = {}. This is not the shortest encoding.",
                        v
                    )));
                }
                Element::Int(v.into())
            }
            Int32 => {
                let v = data
                    .read_i32::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode Int32",
                        actual: data.len(),
                        expected: 4,
                    })?;
                if v >= i16::MIN as i32 {
                    return Err(Error::BadEncode(format!(
                        "Got Int32 with value = {}. This is not the shortest encoding.",
                        v
                    )));
                }
                Element::Int(v.into())
            }
            Int64 => {
                let v = data
                    .read_i64::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode Int64",
                        actual: data.len(),
                        expected: 8,
                    })?;
                if v >= i32::MIN as i64 {
                    return Err(Error::BadEncode(format!(
                        "Got Int64 with value = {}. This is not the shortest encoding.",
                        v
                    )));
                }
                Element::Int(v.into())
            }
            Bin8 => {
                let len = data.read_u8().map_err(|_| Error::LengthTooShort {
                    step: "decode Bin8 length",
                    actual: 0,
                    expected: 1,
                })? as usize;
                if len > data.len() {
                    return Err(Error::LengthTooShort {
                        step: "get Bin8 content",
                        actual: data.len(),
                        expected: len,
                    });
                }
                let (bytes, new_data) = data.split_at(len);
                *data = new_data;
                Element::Bin(bytes)
            }
            Bin16 => {
                let len = data
                    .read_u16::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode Bin16 length",
                        actual: data.len(),
                        expected: 2,
                    })? as usize;
                if len <= (u8::MAX as usize) {
                    return Err(Error::BadEncode(format!(
                        "Got Bin16 with length = {}. This is not the shortest encoding.",
                        len
                    )));
                }
                if len > data.len() {
                    return Err(Error::LengthTooShort {
                        step: "get Bin16 content",
                        actual: data.len(),
                        expected: len,
                    });
                }
                let (bytes, new_data) = data.split_at(len);
                *data = new_data;
                Element::Bin(bytes)
            }
            Bin24 => {
                let len = data
                    .read_u24::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode Bin24 length",
                        actual: data.len(),
                        expected: 3,
                    })? as usize;
                if len <= (u16::MAX as usize) {
                    return Err(Error::BadEncode(format!(
                        "Got Bin24 with length = {}. This is not the shortest encoding.",
                        len
                    )));
                }
                if len > data.len() {
                    return Err(Error::LengthTooShort {
                        step: "get Bin24 content",
                        actual: data.len(),
                        expected: len,
                    });
                }
                let (bytes, new_data) = data.split_at(len);
                *data = new_data;
                Element::Bin(bytes)
            }
            F32 => {
                let v = data
                    .read_f32::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode F32",
                        actual: data.len(),
                        expected: 4,
                    })?;
                Element::F32(v)
            }
            F64 => {
                let v = data
                    .read_f64::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode F64",
                        actual: data.len(),
                        expected: 8,
                    })?;
                Element::F64(v)
            }
            FixStr(len) => {
                let len = len as usize;
                if len > data.len() {
                    return Err(Error::LengthTooShort {
                        step: "get FixStr content",
                        actual: data.len(),
                        expected: len,
                    });
                }
                let (string, new_data) = data.split_at(len);
                *data = new_data;
                let string =
                    std::str::from_utf8(string).map_err(|e| Error::BadEncode(format!("{}", e)))?;
                Element::Str(string)
            }
            Str8 => {
                let len = data.read_u8().map_err(|_| Error::LengthTooShort {
                    step: "decode Str8 length",
                    actual: 0,
                    expected: 1,
                })? as usize;
                if len <= 31 {
                    return Err(Error::BadEncode(format!(
                        "Got Str8 with length = {}. This is not the shortest encoding.",
                        len
                    )));
                }
                if len > data.len() {
                    return Err(Error::LengthTooShort {
                        step: "get Str8 content",
                        actual: data.len(),
                        expected: len,
                    });
                }
                let (string, new_data) = data.split_at(len);
                *data = new_data;
                let string =
                    std::str::from_utf8(string).map_err(|e| Error::BadEncode(format!("{}", e)))?;
                Element::Str(string)
            }
            Str16 => {
                let len = data
                    .read_u16::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode Str16 length",
                        actual: data.len(),
                        expected: 2,
                    })? as usize;
                if len <= (u8::MAX as usize) {
                    return Err(Error::BadEncode(format!(
                        "Got Str16 with length = {}. This is not the shortest encoding.",
                        len
                    )));
                }
                if len > data.len() {
                    return Err(Error::LengthTooShort {
                        step: "get Str16 content",
                        actual: data.len(),
                        expected: len,
                    });
                }
                let (string, new_data) = data.split_at(len);
                *data = new_data;
                let string =
                    std::str::from_utf8(string).map_err(|e| Error::BadEncode(format!("{}", e)))?;
                Element::Str(string)
            }
            Str24 => {
                let len = data
                    .read_u24::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode Str24 length",
                        actual: data.len(),
                        expected: 3,
                    })? as usize;
                if len <= (u16::MAX as usize) {
                    return Err(Error::BadEncode(format!(
                        "Got Str24 with length = {}. This is not the shortest encoding.",
                        len
                    )));
                }
                if len > data.len() {
                    return Err(Error::LengthTooShort {
                        step: "get Str24 content",
                        actual: data.len(),
                        expected: len,
                    });
                }
                let (string, new_data) = data.split_at(len);
                *data = new_data;
                let string =
                    std::str::from_utf8(string).map_err(|e| Error::BadEncode(format!("{}", e)))?;
                Element::Str(string)
            }
            FixArray(len) => Element::Array(len as usize),
            Array8 => {
                let len = data.read_u8().map_err(|_| Error::LengthTooShort {
                    step: "decode Array8 length",
                    actual: 0,
                    expected: 1,
                })? as usize;
                if len <= 15 {
                    return Err(Error::BadEncode(format!(
                        "Got Array8 marker with length = {}. This is not the shortest encoding.",
                        len
                    )));
                }
                Element::Array(len)
            }
            Array16 => {
                let len = data
                    .read_u16::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode Array16 length",
                        actual: data.len(),
                        expected: 2,
                    })? as usize;
                if len <= u8::MAX as usize {
                    return Err(Error::BadEncode(format!(
                        "Got Array16 marker with length = {}. This is not the shortest encoding.",
                        len
                    )));
                }
                if len > data.len() {
                    return Err(Error::BadEncode(format!(
                        "Got Array16 marker with length = {}, but there are only {} bytes left.",
                        len,
                        data.len()
                    )));
                }
                Element::Array(len)
            }
            Array24 => {
                let len = data
                    .read_u24::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode Array24 length",
                        actual: data.len(),
                        expected: 3,
                    })? as usize;
                if len <= u16::MAX as usize {
                    return Err(Error::BadEncode(format!(
                        "Got Array24 marker with length = {}. This is not the shortest encoding.",
                        len
                    )));
                }
                if len > data.len() {
                    return Err(Error::BadEncode(format!(
                        "Got Array24 marker with length = {}, but there are only {} bytes left.",
                        len,
                        data.len()
                    )));
                }
                Element::Array(len)
            }
            FixMap(len) => Element::Map(len as usize),
            Map8 => {
                let len = data.read_u8().map_err(|_| Error::LengthTooShort {
                    step: "decode Map8 length",
                    actual: 0,
                    expected: 1,
                })? as usize;
                if len <= 15 {
                    return Err(Error::BadEncode(format!(
                        "Got Map8 marker with length = {}. This is not the shortest encoding.",
                        len
                    )));
                }
                Element::Map(len)
            }
            Map16 => {
                let len = data
                    .read_u16::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode Map16 length",
                        actual: data.len(),
                        expected: 2,
                    })? as usize;
                if len <= u8::MAX as usize {
                    return Err(Error::BadEncode(format!(
                        "Got Map16 marker with length = {}. This is not the shortest encoding.",
                        len
                    )));
                }
                if 2 * len > data.len() {
                    return Err(Error::BadEncode(format!(
                        "Got Map16 marker with length = {}, but there are only {} bytes left.",
                        len,
                        data.len()
                    )));
                }
                Element::Map(len)
            }
            Map24 => {
                let len = data
                    .read_u24::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode Map24 length",
                        actual: data.len(),
                        expected: 3,
                    })? as usize;
                if len <= u16::MAX as usize {
                    return Err(Error::BadEncode(format!(
                        "Got Map24 marker with length = {}. This is not the shortest encoding.",
                        len
                    )));
                }
                if 2 * len > data.len() {
                    return Err(Error::BadEncode(format!(
                        "Got Map24 marker with length = {}, but there are only {} bytes left.",
                        len,
                        data.len()
                    )));
                }
                Element::Map(len)
            }
            Ext8 => {
                let len = data.read_u8().map_err(|_| Error::LengthTooShort {
                    step: "decode Ext8 length",
                    actual: 0,
                    expected: 1,
                })? as usize;
                Self::parse_ext(data, len)?
            }
            Ext16 => {
                let len = data
                    .read_u16::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode Ext16 length",
                        actual: data.len(),
                        expected: 2,
                    })? as usize;
                if len <= u8::MAX as usize {
                    return Err(Error::BadEncode(format!(
                        "Got Ext16 marker with length = {}. This is not the shortest encoding.",
                        len
                    )));
                }
                Self::parse_ext(data, len)?
            }
            Ext24 => {
                let len = data
                    .read_u24::<LittleEndian>()
                    .map_err(|_| Error::LengthTooShort {
                        step: "decode Ext24 length",
                        actual: data.len(),
                        expected: 3,
                    })? as usize;
                if len <= u16::MAX as usize {
                    return Err(Error::BadEncode(format!(
                        "Got Ext24 marker with length = {}. This is not the shortest encoding.",
                        len
                    )));
                }
                Self::parse_ext(data, len)?
            }
        })
    }

    fn parse_ext(data: &mut &'a [u8], len: usize) -> Result<Element<'a>> {
        let ext_type = data.read_u8().map_err(|_| Error::LengthTooShort {
            step: "decode Ext type",
            actual: data.len(),
            expected: 1,
        })?;
        let ext_type = ExtType::from_u8(ext_type)
            .ok_or_else(|| Error::BadEncode(format!("Got unrecognized Ext type {}.", ext_type)))?;
        if len > data.len() {
            return Err(Error::LengthTooShort {
                step: "get Ext content",
                actual: data.len(),
                expected: len,
            });
        }
        let (bytes, new_data) = data.split_at(len);
        *data = new_data;
        Ok(match ext_type {
            ExtType::Timestamp => {
                Element::Timestamp(Timestamp::try_from(bytes).map_err(Error::BadEncode)?)
            }
            ExtType::Hash => Element::Hash(Hash::try_from(bytes)?),
            ExtType::Identity => Element::Identity(Box::new(Identity::try_from(bytes)?)),
            ExtType::LockId => Element::LockId(Box::new(LockId::try_from(bytes)?)),
            ExtType::StreamId => Element::StreamId(Box::new(StreamId::try_from(bytes)?)),
            ExtType::DataLockbox => Element::DataLockbox(DataLockboxRef::from_bytes(bytes)?),
            ExtType::IdentityLockbox => {
                Element::IdentityLockbox(IdentityLockboxRef::from_bytes(bytes)?)
            }
            ExtType::StreamLockbox => Element::StreamLockbox(StreamLockboxRef::from_bytes(bytes)?),
            ExtType::LockLockbox => Element::LockLockbox(LockLockboxRef::from_bytes(bytes)?),
            ExtType::BareIdKey => Element::BareIdKey(Box::new(BareIdKey::try_from(bytes)?)),
        })
    }
}

impl<'a> std::iter::Iterator for Parser<'a> {
    type Item = Result<Element<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.errored {
            return None;
        }
        let (&marker, data) = self.data.split_first()?;
        self.data = data;
        let result = self.parse_element(Marker::from_u8(marker));
        if result.is_err() {
            self.errored = true;
        }
        Some(result)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn reserved() {
        let data = [0xc1, 0x00, 0xdd, 0x00, 0xde, 0x00, 0xdf, 0x00];
        for i in 0..3 {
            let mut parser = Parser::new(&data[2 * i..2 * i + 1]);
            let result = parser.next().unwrap();
            assert!(
                result.is_err(),
                "0x{:x} should fail because it is a reserved marker byte",
                data[2 * i]
            );
            assert!(parser.next().is_none());
        }
    }

    mod null {
        use super::*;

        #[test]
        fn roundtrip() {
            // Make element
            let elem = Element::Null;
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem.clone());

            // Parse element
            let mut parser = Parser::new(enc.as_ref());
            let result = parser.next().unwrap();
            let val = result.unwrap();
            assert!(parser.next().is_none());
            if let Element::Null = val {
            } else {
                panic!("Element wasn't Null");
            }
        }

        #[test]
        fn spec() {
            let elem = Element::Null;
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            assert_eq!(enc, &[0xc0]);
        }
    }

    mod bool {
        use super::*;

        #[test]
        fn roundtrip_true() {
            // Make element
            let elem = Element::Bool(true);
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem.clone());

            // Parse element
            let mut parser = Parser::new(enc.as_ref());
            let result = parser.next().unwrap();
            let val = result.unwrap();
            assert!(parser.next().is_none());
            if let Element::Bool(val) = val {
                assert!(val);
            } else {
                panic!("Element wasn't an Integer");
            }
        }

        #[test]
        fn roundtrip_false() {
            // Make element
            let elem = Element::Bool(false);
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem.clone());

            // Parse element
            let mut parser = Parser::new(enc.as_ref());
            let result = parser.next().unwrap();
            let val = result.unwrap();
            assert!(parser.next().is_none());
            if let Element::Bool(val) = val {
                assert!(!val);
            } else {
                panic!("Element wasn't an Integer");
            }
        }

        #[test]
        fn spec() {
            let elem = Element::Bool(false);
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            assert_eq!(enc, &[0xc2]);

            let elem = Element::Bool(true);
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            assert_eq!(enc, &[0xc3]);
        }
    }

    mod integer {
        use super::*;

        #[test]
        fn roundtrip_pos_int() {
            // Run through all the boundary cases
            let mut test_cases: Vec<u64> = vec![0, 1];
            for i in 0..5 {
                test_cases.push(127 - 2 + i)
            }
            for i in 0..5 {
                test_cases.push(u8::MAX as u64 - 2 + i)
            }
            for i in 0..5 {
                test_cases.push(u16::MAX as u64 - 2 + i)
            }
            for i in 0..5 {
                test_cases.push(u32::MAX as u64 - 2 + i)
            }
            for i in 0..3 {
                test_cases.push(u64::MAX - 2 + i)
            }

            for case in test_cases {
                // Make element
                let elem = Element::Int((case).into());
                let mut enc = Vec::new();
                serialize_elem(&mut enc, elem.clone());
                println!("{:x?}", enc);

                // Parse element
                let mut parser = Parser::new(enc.as_ref());
                let result = parser.next().unwrap();
                let val = result.unwrap();
                assert!(parser.next().is_none());
                if let Element::Int(val) = val {
                    assert_eq!(val.as_u64().unwrap(), case);
                } else {
                    panic!("Element wasn't an Integer");
                }
            }
        }

        #[test]
        fn roundtrip_neg_int() {
            // Run through all the boundary cases
            let mut test_cases: Vec<i64> = vec![-1];
            for i in -2..3 {
                test_cases.push(-32 - i)
            }
            for i in -2..3 {
                test_cases.push(i8::MIN as i64 - i)
            }
            for i in -2..3 {
                test_cases.push(i16::MIN as i64 - i)
            }
            for i in -2..3 {
                test_cases.push(i32::MIN as i64 - i)
            }
            for i in -2..0 {
                test_cases.push(i64::MIN - i)
            }

            for case in test_cases {
                // Make element
                let elem = Element::Int((case).into());
                let mut enc = Vec::new();
                serialize_elem(&mut enc, elem.clone());
                println!("{:x?}", enc);

                // Parse element
                let mut parser = Parser::new(enc.as_ref());
                let result = parser.next().unwrap();
                let val = result.unwrap();
                assert!(parser.next().is_none());
                if let Element::Int(val) = val {
                    assert_eq!(val.as_i64().unwrap(), case);
                } else {
                    panic!("Element wasn't an Integer");
                }
            }
        }

        #[test]
        fn spec_pos_int() {
            // Check against a list of spec-conforming values
            let mut test_cases: Vec<(u64, Vec<u8>)> = Vec::new();
            test_cases.push((0, vec![0x00]));
            test_cases.push((1, vec![0x01]));
            test_cases.push((127, vec![0x7f]));
            test_cases.push((128, vec![0xcc, 0x80]));
            test_cases.push((u8::MAX as u64, vec![0xcc, 0xff]));
            test_cases.push((u8::MAX as u64 + 1, vec![0xcd, 0x00, 0x01]));
            test_cases.push((u16::MAX as u64, vec![0xcd, 0xff, 0xff]));
            test_cases.push((u16::MAX as u64 + 1, vec![0xce, 0x00, 0x00, 0x01, 0x00]));
            test_cases.push((u32::MAX as u64, vec![0xce, 0xff, 0xff, 0xff, 0xff]));
            test_cases.push((
                u32::MAX as u64 + 1,
                vec![0xcf, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00],
            ));
            test_cases.push((
                u64::MAX,
                vec![0xcf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
            ));

            for case in test_cases {
                let elem = Element::Int(case.0.into());
                let mut enc = Vec::new();
                serialize_elem(&mut enc, elem.clone());
                assert_eq!(enc, case.1);
            }
        }

        #[test]
        fn spec_neg_int() {
            // Check against a list of spec-conforming values
            let mut test_cases: Vec<(i64, Vec<u8>)> = Vec::new();
            test_cases.push((-1, vec![0xff]));
            test_cases.push((-2, vec![0xfe]));
            test_cases.push((-32, vec![0xe0]));
            test_cases.push((-33, vec![0xd0, 0xdf]));
            test_cases.push((i8::MIN as i64, vec![0xd0, 0x80]));
            test_cases.push((i8::MIN as i64 - 1, vec![0xd1, 0x7f, 0xff]));
            test_cases.push((i16::MIN as i64, vec![0xd1, 0x00, 0x80]));
            test_cases.push((i16::MIN as i64 - 1, vec![0xd2, 0xff, 0x7f, 0xff, 0xff]));
            test_cases.push((i32::MIN as i64, vec![0xd2, 0x00, 0x00, 0x00, 0x80]));
            test_cases.push((
                i32::MIN as i64 - 1,
                vec![0xd3, 0xff, 0xff, 0xff, 0x7f, 0xff, 0xff, 0xff, 0xff],
            ));
            test_cases.push((
                i64::MIN,
                vec![0xd3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80],
            ));

            for (index, case) in test_cases.iter().enumerate() {
                let elem = Element::Int(case.0.into());
                let mut enc = Vec::new();
                serialize_elem(&mut enc, elem.clone());
                assert_eq!(enc, case.1, "Failed test #{}", index);
            }
        }

        #[test]
        fn not_enough_bytes() {
            let mut test_cases: Vec<Vec<u8>> = Vec::new();
            test_cases.push(vec![0xcc]);
            test_cases.push(vec![0xcd, 0xff]);
            test_cases.push(vec![0xce, 0xff, 0xff, 0xff]);
            test_cases.push(vec![0xcf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
            test_cases.push(vec![0xd0]);
            test_cases.push(vec![0xd1, 0xff]);
            test_cases.push(vec![0xd2, 0xff, 0xff, 0xff]);
            test_cases.push(vec![0xd3, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
            for (index, case) in test_cases.iter().enumerate() {
                println!("Test #{}: {:x?}", index, case);
                let mut parser = Parser::new(case);
                let result = parser
                    .next()
                    .expect("Should have returned a result on parsing");
                assert!(
                    result.is_err(),
                    "Didn't error when there were too few bytes to parse"
                );
                assert!(parser.next().is_none(), "Parser should stop after error");
            }
        }

        #[test]
        fn non_canonical_pos_int() {
            let mut test_cases: Vec<Vec<u8>> = Vec::new();
            // Each test case has a 0 after it, as the next marker. The parser should never parse
            // that byte, as it's supposed to yield None after an error.
            test_cases.push(vec![0xcc, 0x00, 0x00]);
            test_cases.push(vec![0xcc, 0x7f, 0x00]);
            test_cases.push(vec![0xcd, 0x00, 0x00, 0x00]);
            test_cases.push(vec![0xcd, 0xff, 0x00, 0x00]);
            test_cases.push(vec![0xce, 0x00, 0x00, 0x00, 0x00, 0x00]);
            test_cases.push(vec![0xce, 0xff, 0xff, 0x00, 0x00, 0x00]);
            test_cases.push(vec![
                0xcf, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00,
            ]);
            for (index, case) in test_cases.iter().enumerate() {
                println!("Test #{}: {:x?}", index, case);
                let mut parser = Parser::new(case);
                let result = parser
                    .next()
                    .expect("Should have returned a result on parsing");
                assert!(result.is_err(), "Didn't error on non-canonical value");
                assert!(parser.next().is_none(), "Parser should stop after error");
            }
        }

        #[test]
        fn non_canonical_neg_int() {
            let mut test_cases: Vec<Vec<u8>> = Vec::new();
            // Each test case has a 0 after it, as the next marker. The parser should never parse
            // that byte, as it's supposed to yield None after an error.
            // Run through the positive cases first. Positive values should all fail
            test_cases.push(vec![0xd0, 0x00, 0x00]);
            test_cases.push(vec![0xd0, 0x7f, 0x00]);
            test_cases.push(vec![0xd1, 0x00, 0x00, 0x00]);
            test_cases.push(vec![0xd1, 0xff, 0x7f, 0x00]);
            test_cases.push(vec![0xd2, 0x00, 0x00, 0x00, 0x00, 0x00]);
            test_cases.push(vec![0xd2, 0xff, 0xff, 0xff, 0x7f, 0x00]);
            test_cases.push(vec![
                0xd3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ]);
            test_cases.push(vec![
                0xd3, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f,
            ]);
            // Negative cases that aren't shortest encoding
            test_cases.push(vec![0xd0, 0xff, 0x00]);
            test_cases.push(vec![0xd0, 0xe0, 0x00]);
            test_cases.push(vec![0xd1, 0xe0, 0xff, 0x00]);
            test_cases.push(vec![0xd1, 0x80, 0xff, 0x00]);
            test_cases.push(vec![0xd2, 0xff, 0xff, 0xff, 0xff, 0x00]);
            test_cases.push(vec![0xd2, 0x00, 0x80, 0xff, 0xff, 0x00]);
            test_cases.push(vec![
                0xd3, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00,
            ]);
            test_cases.push(vec![
                0xd3, 0x00, 0x00, 0x00, 0x80, 0xff, 0xff, 0xff, 0xff, 0x00,
            ]);
            for (index, case) in test_cases.iter().enumerate() {
                println!("Test #{}: {:x?}", index, case);
                let mut parser = Parser::new(case);
                let result = parser
                    .next()
                    .expect("Should have returned a result on parsing");
                assert!(result.is_err(), "Didn't error on non-canonical value");
                assert!(parser.next().is_none(), "Parser should stop after error");
            }
        }
    }

    mod f32 {
        use super::*;

        #[test]
        fn roundtrip() {
            let mut test_cases: Vec<f32> = Vec::new();
            test_cases.push(0.0);
            test_cases.push(1.0);
            test_cases.push(-1.0);
            test_cases.push(f32::MIN);
            test_cases.push(f32::MAX);
            test_cases.push(f32::NEG_INFINITY);
            test_cases.push(f32::INFINITY);
            test_cases.push(f32::MIN_POSITIVE);

            for (index, case) in test_cases.iter().enumerate() {
                println!("Test #{}: {}", index, case);
                // Make element
                let elem = Element::F32(*case);
                let mut enc = Vec::new();
                serialize_elem(&mut enc, elem.clone());

                // Parse element
                let mut parser = Parser::new(enc.as_ref());
                let result = parser.next().unwrap();
                let val = result.unwrap();
                assert!(parser.next().is_none());
                if let Element::F32(val) = val {
                    assert_eq!(val.to_ne_bytes(), case.to_ne_bytes());
                } else {
                    panic!("Element wasn't F32");
                }
            }
        }

        #[test]
        fn not_enough_bytes() {
            let enc = vec![0xca, 0x00, 0x00, 0x00];
            let mut parser = Parser::new(enc.as_ref());
            let result = parser
                .next()
                .expect("Should have returned result on parsing");
            assert!(
                result.is_err(),
                "Result should have errored due to not enough bytes"
            );
            assert!(
                parser.next().is_none(),
                "Post-error parser should return None"
            );
        }

        #[test]
        fn spec() {
            let mut test_cases: Vec<(f32, Vec<u8>)> = Vec::new();
            test_cases.push((0.0, vec![0xca, 0x00, 0x00, 0x00, 0x00]));
            test_cases.push((1.0, vec![0xca, 0x00, 0x00, 0x80, 0x3f]));
            test_cases.push((-1.0, vec![0xca, 0x00, 0x00, 0x80, 0xbf]));
            test_cases.push((f32::NEG_INFINITY, vec![0xca, 0x00, 0x00, 0x80, 0xff]));
            test_cases.push((f32::INFINITY, vec![0xca, 0x00, 0x00, 0x80, 0x7f]));

            for (index, case) in test_cases.iter().enumerate() {
                println!("Test #{}: {}", index, case.0);
                // Make element
                let elem = Element::F32(case.0);
                let mut enc = Vec::new();
                serialize_elem(&mut enc, elem);
                assert_eq!(enc, case.1);
            }
        }
    }

    mod f64 {
        use super::*;

        #[test]
        fn roundtrip() {
            let mut test_cases: Vec<f64> = Vec::new();
            test_cases.push(0.0);
            test_cases.push(1.0);
            test_cases.push(-1.0);
            test_cases.push(f64::MIN);
            test_cases.push(f64::MAX);
            test_cases.push(f64::NEG_INFINITY);
            test_cases.push(f64::INFINITY);
            test_cases.push(f64::MIN_POSITIVE);

            for (index, case) in test_cases.iter().enumerate() {
                println!("Test #{}: {}", index, case);
                // Make element
                let elem = Element::F64(*case);
                let mut enc = Vec::new();
                serialize_elem(&mut enc, elem.clone());

                // Parse element
                let mut parser = Parser::new(enc.as_ref());
                let result = parser.next().unwrap();
                let val = result.unwrap();
                assert!(parser.next().is_none());
                if let Element::F64(val) = val {
                    assert_eq!(val.to_ne_bytes(), case.to_ne_bytes());
                } else {
                    panic!("Element wasn't F64");
                }
            }
        }

        #[test]
        fn not_enough_bytes() {
            let enc = vec![0xcb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
            let mut parser = Parser::new(enc.as_ref());
            let result = parser
                .next()
                .expect("Should have returned result on parsing");
            assert!(
                result.is_err(),
                "Result should have errored due to not enough bytes"
            );
            assert!(
                parser.next().is_none(),
                "Post-error parser should return None"
            );
        }

        #[test]
        fn spec() {
            let mut test_cases: Vec<(f64, Vec<u8>)> = Vec::new();
            test_cases.push((
                0.0,
                vec![0xcb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            ));
            test_cases.push((
                1.0,
                vec![0xcb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x3f],
            ));
            test_cases.push((
                -1.0,
                vec![0xcb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0xbf],
            ));
            test_cases.push((
                f64::NEG_INFINITY,
                vec![0xcb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0xff],
            ));
            test_cases.push((
                f64::INFINITY,
                vec![0xcb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x7f],
            ));

            for (index, case) in test_cases.iter().enumerate() {
                println!("Test #{}: {}", index, case.0);
                // Make element
                let elem = Element::F64(case.0);
                let mut enc = Vec::new();
                serialize_elem(&mut enc, elem);
                assert_eq!(enc, case.1);
            }
        }
    }

    mod bin {
        use super::*;
        use rand::prelude::*;

        #[test]
        fn roundtrip() {
            // Run through the boundary cases
            let mut test_cases: Vec<usize> = vec![0, 1];
            for i in 0..5 {
                test_cases.push(u8::MAX as usize - 2 + i);
                test_cases.push(u16::MAX as usize - 2 + i);
            }

            let mut rng = thread_rng();
            for case in test_cases {
                let mut test = vec![0; case];
                rng.fill_bytes(test.as_mut());
                let elem = Element::Bin(test.as_ref());
                let mut enc = Vec::new();
                serialize_elem(&mut enc, elem);

                // Parse element
                let mut parser = Parser::new(enc.as_ref());
                let result = parser.next().unwrap();
                let val = result.unwrap();
                assert!(parser.next().is_none());
                if let Element::Bin(val) = val {
                    assert_eq!(val, &test[..]);
                } else {
                    panic!("Element wasn't Bin");
                }
            }
        }

        #[test]
        fn not_enough_bytes() {
            // Run through the boundary cases
            let mut test_cases: Vec<Vec<u8>> = Vec::new();
            test_cases.push(vec![0xc4, 0x01]);
            let mut case = vec![0xc4, 0xff];
            case.resize(256, 0u8);
            test_cases.push(case);
            let mut case = vec![0xc5, 0xff, 0xff];
            case.resize(65537, 0u8);
            test_cases.push(case);
            let mut case = vec![0xc6, 0xff, 0xff, 0xff];
            case.resize(80000, 0u8);
            test_cases.push(case);

            for case in test_cases {
                let mut parser = Parser::new(case.as_ref());
                let result = parser
                    .next()
                    .expect("Should have returned result on parsing");
                assert!(
                    result.is_err(),
                    "Result should have errored due to not enough bytes"
                );
                assert!(
                    parser.next().is_none(),
                    "Post-error parser should return None"
                );
            }
        }

        #[test]
        fn spec() {
            // Run through spec cases
            let mut test_cases: Vec<(usize, Vec<u8>)> = Vec::new();
            test_cases.push((0, vec![0xc4, 0x00]));
            test_cases.push((1, vec![0xc4, 0x01, 0x00]));
            let mut case = vec![0xc4, 0xff];
            case.resize(255 + 2, 0u8);
            test_cases.push((255, case));
            let mut case = vec![0xc5, 0xff, 0xff];
            case.resize(65535 + 3, 0u8);
            test_cases.push((65535, case));
            let mut case = vec![0xc6, 0x00, 0x00, 0x01];
            case.resize(65536 + 4, 0u8);
            test_cases.push((65536, case));

            for (index, case) in test_cases.iter().enumerate() {
                println!("Test #{}: {}", index, case.0);
                // Make element
                let test_vec = vec![0; case.0];
                let elem = Element::Bin(&test_vec[..]);
                let mut enc = Vec::new();
                serialize_elem(&mut enc, elem);
                assert_eq!(enc, case.1);
            }
        }
    }

    mod str {
        use super::*;
        use rand::prelude::*;

        #[test]
        fn roundtrip() {
            // Run through the boundary cases
            let mut test_cases: Vec<usize> = vec![0, 1, 30, 31, 32, 33];
            for i in 0..5 {
                test_cases.push(u8::MAX as usize - 2 + i);
                test_cases.push(u16::MAX as usize - 2 + i);
            }

            let mut rng = thread_rng();
            for case in test_cases {
                let test: String = rand::distributions::Alphanumeric
                    .sample_iter(&mut rng)
                    .take(case)
                    .map(char::from)
                    .collect();
                let elem = Element::Str(test.as_ref());
                let mut enc = Vec::new();
                serialize_elem(&mut enc, elem);

                println!("Encoded starts with {:x?}, is size {}. Test String starts with {:x?}, is size {}",
                    &enc[0..enc.len().min(6)], enc.len(), &test[0..test.len().min(6)], test.len()
                );

                // Parse element
                let mut parser = Parser::new(enc.as_ref());
                let result = parser.next().unwrap();
                let val = result.unwrap();
                assert!(parser.next().is_none());
                if let Element::Str(val) = val {
                    assert_eq!(val, &test[..]);
                } else {
                    panic!("Element wasn't Str");
                }
            }
        }

        #[test]
        fn not_enough_bytes() {
            // Run through the boundary cases
            let mut test_cases: Vec<Vec<u8>> = Vec::new();
            test_cases.push(vec![0xa1]);
            let mut case = vec![0xbf];
            case.resize(31, 0u8);
            test_cases.push(case);
            let mut case = vec![0xd4, 0xff];
            case.resize(256, 0u8);
            test_cases.push(case);
            let mut case = vec![0xd5, 0xff, 0xff];
            case.resize(65537, 0u8);
            test_cases.push(case);
            let mut case = vec![0xd6, 0xff, 0xff, 0xff];
            case.resize(80000, 0u8);
            test_cases.push(case);

            for case in test_cases {
                let mut parser = Parser::new(case.as_ref());
                let result = parser
                    .next()
                    .expect("Should have returned result on parsing");
                assert!(
                    result.is_err(),
                    "Result should have errored due to not enough bytes"
                );
                assert!(
                    parser.next().is_none(),
                    "Post-error parser should return None"
                );
            }
        }

        #[test]
        fn spec() {
            // Run through spec cases
            let mut test_cases: Vec<(usize, Vec<u8>)> = Vec::new();
            test_cases.push((0, vec![0xa0]));
            test_cases.push((1, vec![0xa1, 0x00]));
            let mut case = vec![0xbf];
            case.resize(32, 0u8);
            test_cases.push((31, case));
            let mut case = vec![0xd4, 0xff];
            case.resize(255 + 2, 0u8);
            test_cases.push((255, case));
            let mut case = vec![0xd5, 0xff, 0xff];
            case.resize(65535 + 3, 0u8);
            test_cases.push((65535, case));
            let mut case = vec![0xd6, 0x00, 0x00, 0x01];
            case.resize(65536 + 4, 0u8);
            test_cases.push((65536, case));

            for (index, case) in test_cases.iter().enumerate() {
                println!("Test #{}: {}", index, case.0);
                // Make element
                let test_vec = vec![0; case.0];
                let test_vec = String::from_utf8(test_vec).unwrap();
                println!("String raw len is {}", test_vec.len());
                let elem = Element::Str(&test_vec[..]);
                let mut enc = Vec::new();
                serialize_elem(&mut enc, elem);
                println!("Encoded len is {}", enc.len());
                assert_eq!(
                    enc,
                    case.1,
                    "Encoded starts with {:x?}, is size {}. Expected starts with {:x?}, is size {}",
                    &enc[0..6],
                    enc.len(),
                    &case.1[0..6],
                    case.1.len()
                );
            }
        }
    }

    mod array {
        use super::*;

        fn edge_cases() -> Vec<usize> {
            let mut test_cases: Vec<usize> = vec![0, 1, 14, 15, 16];
            for i in 0..5 {
                test_cases.push(u8::MAX as usize - 2 + i);
                test_cases.push(u16::MAX as usize - 2 + i);
            }
            test_cases
        }

        fn spec_examples() -> Vec<(usize, Vec<u8>)> {
            vec![
                (0x000000, vec![0x90]),
                (0x000001, vec![0x91]),
                (0x00000f, vec![0x9f]),
                (0x000010, vec![0xd7, 0x10]),
                (0x0000ff, vec![0xd7, 0xff]),
                (0x000100, vec![0xd8, 0x00, 0x01]),
                (0x00ffff, vec![0xd8, 0xff, 0xff]),
                (0x010000, vec![0xd9, 0x00, 0x00, 0x01]),
                (0x020000, vec![0xd9, 0x00, 0x00, 0x02]),
            ]
        }

        #[test]
        fn roundtrip() {
            for case in edge_cases() {
                println!("Test with size = {}", case);
                let elem = Element::Array(case);
                let mut enc = Vec::new();
                serialize_elem(&mut enc, elem);
                enc.resize(enc.len() + case, 0xa0);
                let mut parser = Parser::new(&enc);
                let val = parser
                    .next()
                    .expect("Should have gotten a result")
                    .expect("Should have gotten an Ok result");
                if let Element::Array(val) = val {
                    assert_eq!(val, case);
                } else {
                    panic!("Element wasn't an Array type");
                }
                if case > 0 {
                    let val_next = parser
                        .next()
                        .expect("Should have gotten the next element")
                        .expect("Should have gotten an Ok result");
                    if let Element::Str(val) = val_next {
                        assert_eq!(val, "");
                    } else {
                        panic!("Next element wasn't the empty string");
                    }
                } else {
                    assert!(parser.next().is_none());
                }
            }
        }

        #[test]
        fn not_enough_bytes() {
            for case in spec_examples() {
                println!("Test with spec, size = {}", case.0);
                if case.1.len() == 1 {
                    continue;
                }
                let mut enc = case.1.clone();
                enc.pop();
                let mut parser = Parser::new(&enc);
                let result = parser.next().unwrap();
                assert!(result.is_err());
                assert!(parser.next().is_none());
            }
        }

        #[test]
        fn not_shortest() {
            let mut test_cases: Vec<(usize, Vec<u8>)> = vec![
                (0x0000, vec![0xd7, 0x00]),
                (0x0001, vec![0xd7, 0x01]),
                (0x000f, vec![0xd7, 0x0f]),
                (0x000f, vec![0xd8, 0x0f, 0x00]),
                (0x0010, vec![0xd8, 0x10, 0x00]),
                (0x00ff, vec![0xd8, 0xff, 0x00]),
                (0x000f, vec![0xd9, 0x0f, 0x00, 0x00]),
                (0x0010, vec![0xd9, 0x10, 0x00, 0x00]),
                (0x00ff, vec![0xd9, 0xff, 0x00, 0x00]),
                (0x0100, vec![0xd9, 0x00, 0x01, 0x00]),
                (0x1000, vec![0xd9, 0x00, 0x10, 0x00]),
                (0xffff, vec![0xd9, 0xff, 0xff, 0x00]),
            ];

            for (len, enc) in test_cases.iter_mut() {
                enc.resize(enc.len() + *len, 0xa0);
            }
            for (len, enc) in test_cases {
                println!(
                    "Test with vec {:x?}... (array len={})",
                    &enc[0..(enc.len().min(5))],
                    len
                );
                let mut parser = Parser::new(&enc);
                assert!(
                    parser.next().unwrap().is_err(),
                    "Not shortest should cause failure"
                );
                assert!(
                    parser.next().is_none(),
                    "Should always return None after failure"
                );
            }
        }

        #[test]
        fn spec() {
            for case in spec_examples() {
                println!("Test with spec, size = {}", case.0);
                let elem = Element::Array(case.0);
                let mut enc = Vec::new();
                serialize_elem(&mut enc, elem);
                assert_eq!(enc, case.1);
                enc.resize(enc.len() + case.0, 0xa0);
                let mut parser = Parser::new(&enc);
                let val = parser
                    .next()
                    .expect("Should have gotten a result")
                    .expect("Should have gotten an Ok result");
                if let Element::Array(val) = val {
                    assert_eq!(val, case.0);
                } else {
                    panic!("Element wasn't an Array type");
                }
                if case.0 > 0 {
                    let val_next = parser
                        .next()
                        .expect("Should have gotten the next element")
                        .expect("Should have gotten an Ok result");
                    if let Element::Str(val) = val_next {
                        assert_eq!(val, "");
                    } else {
                        panic!("Next element wasn't the empty string");
                    }
                } else {
                    assert!(parser.next().is_none());
                }
            }
        }
    }

    mod map {
        use super::*;

        fn edge_cases() -> Vec<usize> {
            let mut test_cases: Vec<usize> = vec![0, 1, 14, 15, 16];
            for i in 0..5 {
                test_cases.push(u8::MAX as usize - 2 + i);
                test_cases.push(u16::MAX as usize - 2 + i);
            }
            test_cases
        }

        fn spec_examples() -> Vec<(usize, Vec<u8>)> {
            vec![
                (0x000000, vec![0x80]),
                (0x000001, vec![0x81]),
                (0x00000f, vec![0x8f]),
                (0x000010, vec![0xda, 0x10]),
                (0x0000ff, vec![0xda, 0xff]),
                (0x000100, vec![0xdb, 0x00, 0x01]),
                (0x00ffff, vec![0xdb, 0xff, 0xff]),
                (0x010000, vec![0xdc, 0x00, 0x00, 0x01]),
                (0x020000, vec![0xdc, 0x00, 0x00, 0x02]),
            ]
        }

        #[test]
        fn roundtrip() {
            for case in edge_cases() {
                println!("Test with size = {}", case);
                let elem = Element::Map(case);
                let mut enc = Vec::new();
                serialize_elem(&mut enc, elem);
                println!("{:x?}", &enc);
                enc.resize(enc.len() + 2 * case, 0xa0);
                let mut parser = Parser::new(&enc);
                let val = parser
                    .next()
                    .expect("Should have gotten a result")
                    .expect("Should have gotten an Ok result");
                if let Element::Map(val) = val {
                    assert_eq!(val, case);
                } else {
                    panic!("Element wasn't a Map type");
                }
                if case > 0 {
                    let val_next = parser
                        .next()
                        .expect("Should have gotten the next element")
                        .expect("Should have gotten an Ok result");
                    if let Element::Str(val) = val_next {
                        assert_eq!(val, "");
                    } else {
                        panic!("Next element wasn't the empty string");
                    }
                } else {
                    assert!(parser.next().is_none());
                }
            }
        }

        #[test]
        fn not_enough_bytes() {
            for case in spec_examples() {
                println!("Test with spec, size = {}", case.0);
                if case.1.len() == 1 {
                    continue;
                }
                let mut enc = case.1.clone();
                enc.pop();
                let mut parser = Parser::new(&enc);
                let result = parser.next().unwrap();
                assert!(result.is_err());
                assert!(parser.next().is_none());
            }
        }

        #[test]
        fn not_shortest() {
            let mut test_cases = vec![
                (0x0000, vec![0xda, 0x00]),
                (0x0001, vec![0xda, 0x01]),
                (0x000f, vec![0xda, 0x0f]),
                (0x000f, vec![0xdb, 0x0f, 0x00]),
                (0x0010, vec![0xdb, 0x10, 0x00]),
                (0x00ff, vec![0xdb, 0xff, 0x00]),
                (0x000f, vec![0xdc, 0x0f, 0x00, 0x00]),
                (0x0010, vec![0xdc, 0x10, 0x00, 0x00]),
                (0x00ff, vec![0xdc, 0xff, 0x00, 0x00]),
                (0x0100, vec![0xdc, 0x00, 0x01, 0x00]),
                (0x1000, vec![0xdc, 0x00, 0x10, 0x00]),
                (0xffff, vec![0xdc, 0xff, 0xff, 0x00]),
            ];
            for (len, enc) in test_cases.iter_mut() {
                enc.resize(enc.len() + (*len * 2), 0xa0);
            }
            for (len, enc) in test_cases {
                println!(
                    "Test with vec {:x?}... (map len={})",
                    &enc[0..(enc.len().min(5))],
                    len
                );
                let mut parser = Parser::new(&enc);
                assert!(
                    parser.next().unwrap().is_err(),
                    "Not shortest should cause failure"
                );
                assert!(
                    parser.next().is_none(),
                    "Should always return None after failure"
                );
            }
        }

        #[test]
        fn spec() {
            for case in spec_examples() {
                println!("Test with spec, size = {}", case.0);
                let elem = Element::Map(case.0);
                let mut enc = Vec::new();
                serialize_elem(&mut enc, elem);
                assert_eq!(enc, case.1);
                enc.resize(enc.len() + 2 * case.0, 0xa0);
                let mut parser = Parser::new(&enc);
                let val = parser
                    .next()
                    .expect("Should have gotten a result")
                    .expect("Should have gotten an Ok result");
                if let Element::Map(val) = val {
                    assert_eq!(val, case.0);
                } else {
                    panic!("Element wasn't a Map type");
                }
                if case.0 > 0 {
                    let val_next = parser
                        .next()
                        .expect("Should have gotten the next element")
                        .expect("Should have gotten an Ok result");
                    if let Element::Str(val) = val_next {
                        assert_eq!(val, "");
                    } else {
                        panic!("Next element wasn't the empty string");
                    }
                } else {
                    assert!(parser.next().is_none());
                }
            }
        }
    }

    mod ext {
        use super::*;

        #[test]
        fn unrecognized_ext() {
            let test_cases = vec![
                vec![0xc7, 0x01, 0x00, 0x00],
                vec![0xc7, 0x01, 0xfe, 0x00],
                vec![0xc7, 0x01, 0x09, 0x00],
            ];
            for case in test_cases {
                let mut parser = Parser::new(&case);
                assert!(parser.next().unwrap().is_err());
                assert!(parser.next().is_none());
            }
        }

        #[test]
        fn not_shortest() {
            let timestamp = Timestamp::from_utc(0, 0).unwrap();
            let time_len = timestamp.size();
            let mut test_cases = Vec::new();
            let mut case = vec![0xc8];
            case.extend_from_slice(&time_len.to_le_bytes()[..2]);
            case.push(0xff);
            timestamp.encode_vec(&mut case);
            test_cases.push(case);
            let mut case = vec![0xc9];
            case.extend_from_slice(&time_len.to_le_bytes()[..3]);
            case.push(0xff);
            timestamp.encode_vec(&mut case);
            test_cases.push(case);

            for case in test_cases {
                let mut parser = Parser::new(&case);
                let err = parser.next().unwrap().unwrap_err();
                if let Error::BadEncode(_) = err {
                } else {
                    panic!("Was expecting a different error class for this");
                }
                assert!(parser.next().is_none());
            }
        }
    }

    mod timestamp {
        use super::*;

        fn edge_cases() -> Vec<(usize, Timestamp)> {
            vec![
                (5, Timestamp::from_utc(0, 0).unwrap()),
                (5, Timestamp::from_utc(1, 0).unwrap()),
                (13, Timestamp::from_utc(1, 1).unwrap()),
                (5, Timestamp::from_utc(u32::MAX as i64 - 1, 0).unwrap()),
                (5, Timestamp::from_utc(u32::MAX as i64, 0).unwrap()),
                (9, Timestamp::from_utc(u32::MAX as i64 + 1, 0).unwrap()),
                (9, Timestamp::from_utc(i64::MIN, 0).unwrap()),
                (13, Timestamp::from_utc(i64::MIN, 1).unwrap()),
            ]
        }

        #[test]
        fn roundtrip() {
            for (index, case) in edge_cases().iter().enumerate() {
                println!(
                    "Test #{}: '{}' with expected length = {}",
                    index, case.1, case.0
                );
                let mut enc = Vec::new();
                let elem = Element::Timestamp(case.1);
                serialize_elem(&mut enc, elem);
                println!("{:x?}", &enc);
                let mut parser = Parser::new(&enc);
                let val = parser
                    .next()
                    .expect("Should have gotten a result")
                    .expect("Should have gotten an Ok result");
                assert!(parser.next().is_none());
                if let Element::Timestamp(val) = val {
                    assert_eq!(val, case.1);
                } else {
                    panic!("Element wasn't a Timestamp type");
                }
            }
        }
    }

    mod hash {
        use super::*;

        #[test]
        fn roundtrip() {
            let hash = Hash::new(b"I am about to get hashed");
            let elem = Element::Hash(hash.clone());
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            let mut parser = Parser::new(&enc);
            let val = parser
                .next()
                .expect("Should have gotten a result")
                .expect("Should have gotten an Ok result");
            assert!(parser.next().is_none());
            if let Element::Hash(val) = val {
                assert_eq!(val, hash);
            } else {
                panic!("Element wasn't a Hash type");
            }
        }

        #[test]
        fn too_long() {
            let hash = Hash::new(b"I am about to get hashed");
            let elem = Element::Hash(hash.clone());
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            assert!(enc[1] as usize == hash.as_ref().len());
            enc[1] += 1;
            enc.push(0u8);
            let mut parser = Parser::new(&enc);
            let val = parser.next().expect("Should have gotten a result");
            assert!(val.is_err());
            assert!(parser.next().is_none());
        }

        #[test]
        fn too_short() {
            let hash = Hash::new(b"I am about to get hashed");
            let elem = Element::Hash(hash.clone());
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            assert!(enc[1] as usize == hash.as_ref().len());
            enc[1] -= 1;
            enc.pop();
            let mut parser = Parser::new(&enc);
            let val = parser.next().expect("Should have gotten a result");
            assert!(val.is_err());
            assert!(parser.next().is_none());
        }
    }

    mod identity {
        use super::*;

        #[test]
        fn roundtrip() {
            let identity = fog_crypto::identity::IdentityKey::new().id().to_owned();
            let elem = Element::Identity(Box::new(identity.clone()));
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            let mut parser = Parser::new(&enc);
            let val = parser
                .next()
                .expect("Should have gotten a result")
                .expect("Should have gotten an Ok result");
            assert!(parser.next().is_none());
            if let Element::Identity(val) = val {
                assert_eq!(val.as_ref(), &identity);
            } else {
                panic!("Element wasn't a Identity type");
            }
        }

        #[test]
        fn too_long() {
            let identity = fog_crypto::identity::IdentityKey::new().id().to_owned();
            let elem = Element::Identity(Box::new(identity.clone()));
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            assert!(enc[1] as usize == identity.size());
            enc[1] += 1;
            enc.push(0u8);
            let mut parser = Parser::new(&enc);
            let val = parser.next().expect("Should have gotten a result");
            assert!(val.is_err());
            assert!(parser.next().is_none());
        }

        #[test]
        fn too_short() {
            let identity = fog_crypto::identity::IdentityKey::new().id().to_owned();
            let elem = Element::Identity(Box::new(identity.clone()));
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            assert!(enc[1] as usize == identity.size());
            enc[1] -= 1;
            enc.pop();
            let mut parser = Parser::new(&enc);
            let val = parser.next().expect("Should have gotten a result");
            assert!(val.is_err());
            assert!(parser.next().is_none());
        }
    }

    mod lock_id {
        use super::*;

        #[test]
        fn roundtrip() {
            let id = fog_crypto::lock::LockKey::new().id().to_owned();
            let elem = Element::LockId(Box::new(id.clone()));
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            let mut parser = Parser::new(&enc);
            let val = parser
                .next()
                .expect("Should have gotten a result")
                .expect("Should have gotten an Ok result");
            assert!(parser.next().is_none());
            if let Element::LockId(val) = val {
                assert_eq!(val.as_ref(), &id);
            } else {
                panic!("Element wasn't a LockId type");
            }
        }

        #[test]
        fn too_long() {
            let id = fog_crypto::lock::LockKey::new().id().to_owned();
            let elem = Element::LockId(Box::new(id.clone()));
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            assert!(enc[1] as usize == id.size());
            enc[1] += 1;
            enc.push(0u8);
            let mut parser = Parser::new(&enc);
            let val = parser.next().expect("Should have gotten a result");
            assert!(val.is_err());
            assert!(parser.next().is_none());
        }

        #[test]
        fn too_short() {
            let id = fog_crypto::lock::LockKey::new().id().to_owned();
            let elem = Element::LockId(Box::new(id.clone()));
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            assert!(enc[1] as usize == id.size());
            enc[1] -= 1;
            enc.pop();
            let mut parser = Parser::new(&enc);
            let val = parser.next().expect("Should have gotten a result");
            assert!(val.is_err());
            assert!(parser.next().is_none());
        }
    }

    mod stream_id {
        use super::*;

        #[test]
        fn roundtrip() {
            let id = fog_crypto::stream::StreamKey::new().id().to_owned();
            let elem = Element::StreamId(Box::new(id.clone()));
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            let mut parser = Parser::new(&enc);
            let val = parser
                .next()
                .expect("Should have gotten a result")
                .expect("Should have gotten an Ok result");
            assert!(parser.next().is_none());
            if let Element::StreamId(val) = val {
                assert_eq!(val.as_ref(), &id);
            } else {
                panic!("Element wasn't a LockId type");
            }
        }

        #[test]
        fn too_long() {
            let id = fog_crypto::stream::StreamKey::new().id().to_owned();
            let elem = Element::StreamId(Box::new(id.clone()));
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            assert!(enc[1] as usize == id.size());
            enc[1] += 1;
            enc.push(0u8);
            let mut parser = Parser::new(&enc);
            let val = parser.next().expect("Should have gotten a result");
            assert!(val.is_err());
            assert!(parser.next().is_none());
        }

        #[test]
        fn too_short() {
            let id = fog_crypto::stream::StreamKey::new().id().to_owned();
            let elem = Element::StreamId(Box::new(id.clone()));
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            assert!(enc[1] as usize == id.size());
            enc[1] -= 1;
            enc.pop();
            let mut parser = Parser::new(&enc);
            let val = parser.next().expect("Should have gotten a result");
            assert!(val.is_err());
            assert!(parser.next().is_none());
        }
    }

    mod lockbox {
        use super::*;

        fn roundtrip_data_lockbox_len(len: usize) {
            let key = fog_crypto::stream::StreamKey::new();
            let data = vec![0u8; len];
            let lockbox = key.encrypt_data(&data);
            let elem = Element::DataLockbox(&lockbox);
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            let mut parser = Parser::new(&enc);
            let val = parser
                .next()
                .expect("Should have gotten a result")
                .expect("Should have gotten an Ok result");
            assert!(parser.next().is_none());
            if let Element::DataLockbox(val) = val {
                let dec = key.decrypt_data(val).unwrap();
                assert_eq!(dec, data);
            } else {
                panic!("Element wasn't a DataLockbox type");
            }
        }

        #[test]
        fn roundtrip_data_lockbox() {
            roundtrip_data_lockbox_len(0);
            roundtrip_data_lockbox_len(1);
            roundtrip_data_lockbox_len(255);
            roundtrip_data_lockbox_len(256);
            roundtrip_data_lockbox_len(65535);
            roundtrip_data_lockbox_len(65536);
            roundtrip_data_lockbox_len(80000);
        }

        #[test]
        fn roundtrip_identity_lockbox() {
            let key = fog_crypto::stream::StreamKey::new();
            let to_send = fog_crypto::identity::IdentityKey::new();
            let lockbox = to_send.export_for_stream(&key).unwrap();
            let elem = Element::IdentityLockbox(&lockbox);
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            let mut parser = Parser::new(&enc);
            let val = parser
                .next()
                .expect("Should have gotten a result")
                .expect("Should have gotten an Ok result");
            assert!(parser.next().is_none());
            if let Element::IdentityLockbox(val) = val {
                let dec = key.decrypt_identity_key(val).unwrap();
                assert_eq!(dec.id(), to_send.id());
            } else {
                panic!("Element wasn't a IdentityLockbox type");
            }
        }

        #[test]
        fn roundtrip_stream_lockbox() {
            let key = fog_crypto::stream::StreamKey::new();
            let to_send = fog_crypto::stream::StreamKey::new();
            let lockbox = to_send.export_for_stream(&key).unwrap();
            let elem = Element::StreamLockbox(&lockbox);
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            let mut parser = Parser::new(&enc);
            let val = parser
                .next()
                .expect("Should have gotten a result")
                .expect("Should have gotten an Ok result");
            assert!(parser.next().is_none());
            if let Element::StreamLockbox(val) = val {
                let dec = key.decrypt_stream_key(val).unwrap();
                assert_eq!(dec.id(), to_send.id());
            } else {
                panic!("Element wasn't a StreamLockbox type");
            }
        }

        #[test]
        fn roundtrip_lock_lockbox() {
            let key = fog_crypto::stream::StreamKey::new();
            let to_send = fog_crypto::lock::LockKey::new();
            let lockbox = to_send.export_for_stream(&key).unwrap();
            let elem = Element::LockLockbox(&lockbox);
            let mut enc = Vec::new();
            serialize_elem(&mut enc, elem);
            let mut parser = Parser::new(&enc);
            let val = parser
                .next()
                .expect("Should have gotten a result")
                .expect("Should have gotten an Ok result");
            assert!(parser.next().is_none());
            if let Element::LockLockbox(val) = val {
                let dec = key.decrypt_lock_key(val).unwrap();
                assert_eq!(dec.id(), to_send.id());
            } else {
                panic!("Element wasn't a LockLockbox type");
            }
        }
    }
}
