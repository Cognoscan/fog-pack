
/// Defines the known Ext Types that this library relies on.
///
/// `Timestamp` is defined in the msgpack standard. The remainder are types used by this library 
/// for encoding cryptographic data.
#[derive(Debug,PartialEq,Eq)]
pub enum ExtType {
    Timestamp,
    Index,
    Hash,
    Identity,
    Signature,
    Lockbox,
}

impl ExtType {
    /// Return the assigned extension type.
    pub fn to_i8(self) -> i8 {
        match self {
            ExtType::Timestamp => -1,
            ExtType::Index     => 1,
            ExtType::Hash      => 2,
            ExtType::Identity  => 3,
            ExtType::Signature => 4,
            ExtType::Lockbox   => 5,
        }
    }

    /// Convert from assigned extension type to i8.
    pub fn from_i8(v: i8) -> Option<ExtType> {
        match v {
            -1 => Some(ExtType::Timestamp),
            1  => Some(ExtType::Index),
            2  => Some(ExtType::Hash),
            3  => Some(ExtType::Identity),
            4  => Some(ExtType::Signature),
            5  => Some(ExtType::Lockbox),
            _  => None,
        }
    }
}
