use std::io;
use std::io::ErrorKind::InvalidData;
use byteorder::ReadBytesExt;

/// Defines the compression types supported by documents & entries. Format when encoded is a single 
/// byte, with the lowest two bits indicating the actual compression type. The upper 6 bits are 
/// reserved for possible future compression formats. For now, the only allowed compression is 
/// zstd, where the upper 6 bits are 0.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum CompressType {
    /// No compression applied to the document/entry.
    Uncompressed,
    /// Document is compressed, but has no schema. May be used for compressed entries.
    CompressedNoSchema,
    /// Document is compressed and has associated schema. May be used for compressed entries.
    Compressed,
    /// Document/Entry is compressed using a schema's dictionary.
    DictCompressed,
}

impl CompressType {
    /// Convert CompressType to a single byte.
    pub fn to_u8(self) -> u8 {
        match self {
            CompressType::Uncompressed       => 0,
            CompressType::CompressedNoSchema => 1,
            CompressType::Compressed         => 2,
            CompressType::DictCompressed     => 3,
        }
    }

    /// Try to read a byte as a CompressType. Fails if it's not a recognized type.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(CompressType::Uncompressed),
            1 => Some(CompressType::CompressedNoSchema),
            2 => Some(CompressType::Compressed),
            3 => Some(CompressType::DictCompressed),
            _ => None,
        }
    }

    pub fn encode(&self, buf: &mut Vec<u8>) {
        buf.push(self.to_u8());
    }

    pub fn decode(buf: &mut &[u8]) -> io::Result<Self> {
        Self::from_u8(buf.read_u8()?)
            .ok_or(io::Error::new(InvalidData, "Compression type not recognized"))
    }

}
