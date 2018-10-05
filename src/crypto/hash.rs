use libsodium_sys;
use constant_time_eq::constant_time_eq;
use std::fmt;
use std::io::{Write,Read};
use byteorder::{ReadBytesExt,WriteBytesExt};
use super::CryptoError;
use std::hash;

/// Crytographically secure hash of data. Can be signed by a Key. It is impractical to generate an 
/// identical hash from different data.
#[derive(Clone)]
pub struct Hash {
    version: u8,
    digest: [u8; 64],
}

impl Eq for Hash { }

impl PartialEq for Hash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version && constant_time_eq(&self.digest, &other.digest)
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{} {{ version: {:?}, digest: {:?} }}", stringify!(Hash), &self.version, &self.digest[..])
    }
}

impl hash::Hash for Hash {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.version.hash(state);
        self.digest.hash(state);
    }
}

impl Hash {

    pub fn new(version: u8, data: &[u8]) -> Result<Hash, CryptoError> {
        if version != 1 { return Err(CryptoError::UnsupportedVersion); }
        if data.len() as u64 > ::std::u64::MAX { return Err(CryptoError::BadLength); }
        let mut hash = Hash {version, digest: [0;64]};
        blake2b(&mut hash.digest, data);
        Ok(hash)
    }

    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), CryptoError> {
        wr.write_u8(self.version).map_err(CryptoError::Io)?;
        wr.write_all(&self.digest).map_err(CryptoError::Io)?;
        Ok(())
    }

    pub fn read<R: Read>(rd: &mut R) -> Result<Hash, CryptoError> {
        let version = rd.read_u8().map_err(CryptoError::Io)?;
        if version != 1 { return Err(CryptoError::UnsupportedVersion); }
        let mut hash = Hash {version, digest:[0;64]};
        rd.read_exact(&mut hash.digest).map_err(CryptoError::Io)?;
        Ok(hash)
    }
}

fn blake2b( hash: &mut [u8; 64], data: &[u8] ) {
    // The below will only fail if we set up this function wrong.
    unsafe { 
        libsodium_sys::crypto_generichash_blake2b(
            hash.as_mut_ptr(), 64, 
            data.as_ptr(), data.len() as u64,
            ::std::ptr::null(), 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc_dec(h: Hash) {
        let mut v = Vec::new();
        h.write(&mut v).unwrap();
        let hd = Hash::read(&mut &v[..]).unwrap();
        assert_eq!(h, hd);
    }

    #[test]
    fn test_hashes() {
        let h = Hash::new(1, &[]).unwrap();
        enc_dec(h);
        let h = Hash::new(1, &[0]).unwrap();
        enc_dec(h);
    }
}
