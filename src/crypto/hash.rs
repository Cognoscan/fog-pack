use constant_time_eq::constant_time_eq;
use std::io::{self,Write,Read};
use byteorder::{ReadBytesExt,WriteBytesExt};
use blake2_rfc::blake2s::blake2s;
use super::CryptoError;

/// Crytographically secure hash of data. Can be signed by a Key. It is impractical to generate an 
/// identical hash from different data.
#[derive(Debug,Clone,Hash)]
pub struct Hash {
    version: u8,
    digest: [u8; 32],
}

impl Eq for Hash { }

impl PartialEq for Hash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version && constant_time_eq(&self.digest, &other.digest)
    }
}

impl Hash {

    pub fn new(version: u8, m: &[u8]) -> Result<Hash, CryptoError> {
        if version != 1 { return Err(CryptoError::UnsupportedVersion); }
        let mut h = Hash {version, digest: [0;32]};
        let blake = blake2s(32, &[], m);
        h.digest.copy_from_slice(blake.as_bytes());
        Ok(h)
    }

    pub fn is_valid(&self) -> bool {
        if self.version != 1 { return false; }
        if self.digest.len() != 32 { return false; }
        true
    }

    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), io::Error> {
        wr.write_u8(self.version)?;
        wr.write_all(&self.digest)?;
        Ok(())
    }

    pub fn read<R: Read>(rd: &mut R) -> Result<Hash, CryptoError> {
        let version = rd.read_u8().map_err(CryptoError::Io)?;
        if version != 1 { return Err(CryptoError::UnsupportedVersion); }
        let mut h = Hash {version, digest:[0;32]};
        rd.read_exact(&mut h.digest).map_err(CryptoError::Io)?;
        Ok(h)
    }
}
