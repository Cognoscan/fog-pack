
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

    /// Convert from assigned extension type to i8.
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
