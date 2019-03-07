/// MessagePack Format Markers. For internal use only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Marker {
    PosFixInt(u8),
    FixMap(u8),
    FixArray(u8),
    FixStr(u8),
    Nil,
    Reserved,
    False,
    True,
    Bin8,
    Bin16,
    Bin32,
    Ext8,
    Ext16,
    Ext32,
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
    FixExt1,
    FixExt2,
    FixExt4,
    FixExt8,
    FixExt16,
    Str8,
    Str16
    Str32,
    Array16,
    Array32,
    Map16,
    Map32,
    NegFixInt(i8),
}

impl Marker {
    /// Construct a marker from a single byte.
    pub fn from_u8(n: u8) -> Marker {
        match n {
            0x00 ... 0x7f => Marker::PosFixInt(n),
            0x80 ... 0x8f => Marker::FixMap(n & 0x0F),
            0x90 ... 0x9f => Marker::FixArray(n & 0x0F),
            0xa0 ... 0xbf => Marker::FixStr(n & 0x1F),
            0xc0 => Marker::Nil,
            // Marked in MessagePack spec as never used.
            0xc1 => Marker::Reserved,
            0xc2 => Marker::False,
            0xc3 => Marker::True,
            0xc4 => Marker::Bin8,
            0xc5 => Marker::Bin16,
            0xc6 => Marker::Bin32,
            0xc7 => Marker::Ext8,
            0xc8 => Marker::Ext16,
            0xc9 => Marker::Ext32,
            0xca => Marker::F32,
            0xcb => Marker::F64,
            0xcc => Marker::U8,
            0xcd => Marker::U16,
            0xce => Marker::U32,
            0xcf => Marker::U64,
            0xd0 => Marker::I8,
            0xd1 => Marker::I16,
            0xd2 => Marker::I32,
            0xd3 => Marker::I64,
            0xd4 => Marker::FixExt1,
            0xd5 => Marker::FixExt2,
            0xd6 => Marker::FixExt4,
            0xd7 => Marker::FixExt8,
            0xd8 => Marker::FixExt16,
            0xd9 => Marker::Str8,
            0xda => Marker::Str16,
            0xdb => Marker::Str32,
            0xdc => Marker::Array16,
            0xdd => Marker::Array32,
            0xde => Marker::Map16,
            0xdf => Marker::Map32,
            0xe0 ... 0xff => Marker::NegFixInt(n as i8),
        }
    }

    /// Converts a marker object into a single-byte representation.
    /// Assumes the content of the marker is already masked approprately
    pub fn to_u8(&self) -> u8 {
        match *self {
            Marker::PosFixInt(val) => val,
            Marker::FixMap(len)    => 0x80 | len,
            Marker::FixArray(len)  => 0x90 | len,
            Marker::FixStr(len)    => 0xa0 | len,
            Marker::Null           => 0xc0,
            Marker::Reserved       => 0xc1,
            Marker::False          => 0xc2,
            Marker::True           => 0xc3,
            Marker::Bin8           => 0xc4,
            Marker::Bin16          => 0xc5,
            Marker::Bin32          => 0xc6,
            Marker::Ext8           => 0xc7,
            Marker::Ext16          => 0xc8,
            Marker::Ext32          => 0xc9,
            Marker::F32            => 0xca,
            Marker::F64            => 0xcb,
            Marker::U8             => 0xcc,
            Marker::U16            => 0xcd,
            Marker::U32            => 0xce,
            Marker::U64            => 0xcf,
            Marker::I8             => 0xd0,
            Marker::I16            => 0xd1,
            Marker::I32            => 0xd2,
            Marker::I64            => 0xd3,
            Marker::FixExt1        => 0xd4,
            Marker::FixExt2        => 0xd5,
            Marker::FixExt4        => 0xd6,
            Marker::FixExt8        => 0xd7,
            Marker::FixExt16       => 0xd8,
            Marker::Str8           => 0xd9,
            Marker::Str16          => 0xda,
            Marker::Str32          => 0xdb,
            Marker::Array16        => 0xdc,
            Marker::Array32        => 0xdd,
            Marker::Map16          => 0xde,
            Marker::Map32          => 0xdf,
            Marker::NegFixInt(val) => val as u8,
        }
    }
}

impl From<u8> for Marker {
    fn from(val: u8) -> Marker {
        Marker::from_u8(val)
    }
}

impl Into<u8> for Marker {
    fn into(self) -> u8 {
        self.to_u8()
    }
}
/// Defines the known Ext Types that this library relies on.
///
/// `Timestamp` is defined in the msgpack standard. The remainder are types used by this library 
/// for encoding cryptographic data.
#[derive(Debug,PartialEq,Eq)]
pub enum ExtType {
    Timestamp,
    Hash,
    Identity,
    Lockbox,
}

impl ExtType {
    /// Return the assigned extension type.
    pub fn to_i8(self) -> i8 {
        match self {
            ExtType::Timestamp => -1,
            ExtType::Hash      => 1,
            ExtType::Identity  => 2,
            ExtType::Lockbox   => 3,
        }
    }

    /// Convert from assigned extension type to i8. Returns `None` if type isn't recognized.
    pub fn from_i8(v: i8) -> Option<ExtType> {
        match v {
            -1 => Some(ExtType::Timestamp),
            1  => Some(ExtType::Hash),
            2  => Some(ExtType::Identity),
            3  => Some(ExtType::Lockbox),
            _  => None,
        }
    }
}

impl Into<i8> for ExtType {
    fn into(self) -> i8 {
        self.to_i8()
    }
}
