use crate::MAX_DOC_SIZE;

/// MessagePack Format Markers. For internal use only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Marker {
    PosFixInt(u8),
    FixMap(u8),
    FixArray(u8),
    FixStr(u8),
    Null,
    Reserved,
    False,
    True,
    Bin8,
    Bin16,
    Bin24,
    Ext8,
    Ext16,
    Ext24,
    F32,
    F64,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    Int8,
    Int16,
    Int32,
    Int64,
    Str8,
    Str16,
    Str24,
    Array8,
    Array16,
    Array24,
    Map8,
    Map16,
    Map24,
    NegFixInt(i8),
}

impl Marker {
    /// Construct a marker from a single byte.
    pub fn from_u8(n: u8) -> Marker {
        match n {
            0x00..=0x7f => Marker::PosFixInt(n),
            0x80..=0x8f => Marker::FixMap(n & 0x0F),
            0x90..=0x9f => Marker::FixArray(n & 0x0F),
            0xa0..=0xbf => Marker::FixStr(n & 0x1F),
            0xc0 => Marker::Null,
            0xc1 => Marker::Reserved,
            0xc2 => Marker::False,
            0xc3 => Marker::True,
            0xc4 => Marker::Bin8,
            0xc5 => Marker::Bin16,
            0xc6 => Marker::Bin24,
            0xc7 => Marker::Ext8,
            0xc8 => Marker::Ext16,
            0xc9 => Marker::Ext24,
            0xca => Marker::F32,
            0xcb => Marker::F64,
            0xcc => Marker::UInt8,
            0xcd => Marker::UInt16,
            0xce => Marker::UInt32,
            0xcf => Marker::UInt64,
            0xd0 => Marker::Int8,
            0xd1 => Marker::Int16,
            0xd2 => Marker::Int32,
            0xd3 => Marker::Int64,
            0xd4 => Marker::Str8,
            0xd5 => Marker::Str16,
            0xd6 => Marker::Str24,
            0xd7 => Marker::Array8,
            0xd8 => Marker::Array16,
            0xd9 => Marker::Array24,
            0xda => Marker::Map8,
            0xdb => Marker::Map16,
            0xdc => Marker::Map24,
            0xdd => Marker::Reserved,
            0xde => Marker::Reserved,
            0xdf => Marker::Reserved,
            0xe0..=0xff => Marker::NegFixInt(n as i8),
        }
    }

    /// Converts a marker object into a single-byte representation.
    /// Assumes the content of the marker is already masked approprately
    pub fn into_u8(self) -> u8 {
        match self {
            Marker::PosFixInt(val) => val,
            Marker::FixMap(len) => 0x80 | len,
            Marker::FixArray(len) => 0x90 | len,
            Marker::FixStr(len) => 0xa0 | len,
            Marker::Null => 0xc0,
            Marker::Reserved => 0xc1,
            Marker::False => 0xc2,
            Marker::True => 0xc3,
            Marker::Bin8 => 0xc4,
            Marker::Bin16 => 0xc5,
            Marker::Bin24 => 0xc6,
            Marker::Ext8 => 0xc7,
            Marker::Ext16 => 0xc8,
            Marker::Ext24 => 0xc9,
            Marker::F32 => 0xca,
            Marker::F64 => 0xcb,
            Marker::UInt8 => 0xcc,
            Marker::UInt16 => 0xcd,
            Marker::UInt32 => 0xce,
            Marker::UInt64 => 0xcf,
            Marker::Int8 => 0xd0,
            Marker::Int16 => 0xd1,
            Marker::Int32 => 0xd2,
            Marker::Int64 => 0xd3,
            Marker::Str8 => 0xd4,
            Marker::Str16 => 0xd5,
            Marker::Str24 => 0xd6,
            Marker::Array8 => 0xd7,
            Marker::Array16 => 0xd8,
            Marker::Array24 => 0xd9,
            Marker::Map8 => 0xda,
            Marker::Map16 => 0xdb,
            Marker::Map24 => 0xdc,
            Marker::NegFixInt(val) => val as u8,
        }
    }

    pub fn encode_ext_marker(buf: &mut Vec<u8>, len: usize) {
        assert!(len <= MAX_DOC_SIZE);
        if len < u8::MAX as usize {
            buf.push(Marker::Ext8.into());
            buf.push(len as u8);
        } else if len < u16::MAX as usize {
            buf.push(Marker::Ext16.into());
            buf.extend_from_slice(&len.to_le_bytes()[..2]);
        } else {
            buf.push(Marker::Ext24.into());
            buf.extend_from_slice(&len.to_le_bytes()[..3]);
        }
    }
}

impl From<u8> for Marker {
    fn from(val: u8) -> Marker {
        Marker::from_u8(val)
    }
}

impl From<Marker> for u8 {
    fn from(val: Marker) -> u8 {
        val.into_u8()
    }
}

/// Defines the Ext Types that this library relies on.
#[derive(Debug, PartialEq, Eq)]
pub enum ExtType {
    Timestamp,
    Hash,
    Identity,
    LockId,
    StreamId,
    DataLockbox,
    IdentityLockbox,
    StreamLockbox,
    LockLockbox,
}

impl ExtType {
    /// Return the assigned extension type.
    pub fn into_u8(self) -> u8 {
        match self {
            ExtType::Timestamp => 0,
            ExtType::Hash => 1,
            ExtType::Identity => 2,
            ExtType::LockId => 3,
            ExtType::StreamId => 4,
            ExtType::DataLockbox => 5,
            ExtType::IdentityLockbox => 6,
            ExtType::StreamLockbox => 7,
            ExtType::LockLockbox => 8,
        }
    }

    /// Convert from assigned extension type to i8. Returns `None` if type isn't recognized.
    pub fn from_u8(v: u8) -> Option<ExtType> {
        match v {
            0 => Some(ExtType::Timestamp),
            1 => Some(ExtType::Hash),
            2 => Some(ExtType::Identity),
            3 => Some(ExtType::LockId),
            4 => Some(ExtType::StreamId),
            5 => Some(ExtType::DataLockbox),
            6 => Some(ExtType::IdentityLockbox),
            7 => Some(ExtType::StreamLockbox),
            8 => Some(ExtType::LockLockbox),
            _ => None,
        }
    }
}

impl From<ExtType> for u8 {
    fn from(val: ExtType) -> u8 {
        val.into_u8()
    }
}
