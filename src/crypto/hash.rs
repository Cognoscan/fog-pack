use constant_time_eq::constant_time_eq;
use std::fmt;
use std::io::{Write,Read};
use byteorder::{ReadBytesExt,WriteBytesExt};
use super::CryptoError;
use std::hash;
use super::sodium::blake2b;

/// Crytographically secure hash of data. Can be signed by a FullKey. It is impractical to generate an 
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
        if data.len() > ::std::u64::MAX as usize { return Err(CryptoError::BadLength); }
        let mut hash = Hash {version, digest: [0;64]};
        blake2b(&mut hash.digest, data);
        Ok(hash)
    }

    pub fn get_version(&self) -> u8 {
        self.version
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.digest
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use serde_json::{self,Value};
    use hex;

    fn enc_dec(h: Hash) {
        let mut v = Vec::new();
        h.write(&mut v).unwrap();
        let hd = Hash::read(&mut &v[..]).unwrap();
        assert_eq!(h, hd);
    }

    #[test]
    fn hash_vectors() {
        let file_ref = fs::File::open("test-resources/blake2b-test-vectors.json").unwrap();
        let json_ref : Value = serde_json::from_reader(file_ref).unwrap();

        for vector in json_ref.as_array().unwrap().iter() {
            let ref_hash = hex::decode(&vector["out"].as_str().unwrap()).unwrap();
            let ref_input = hex::decode(&vector["input"].as_str().unwrap()).unwrap();
            let h = Hash::new(1, &ref_input[..]).unwrap();
            assert_eq!(h.version, 1u8);
            assert_eq!(h.digest[..], ref_hash[..]);
            enc_dec(h)
        }
    }

    #[test]
    fn edge_cases() {
        match Hash::new(0, &[1,2]).unwrap_err() {
            CryptoError::UnsupportedVersion => (),
            _ => panic!("Hash should always fail on version 0"),
        };
        let digest = hex::decode(
            "29102511d749db3cc9b4e335fa1f5e8faca8421d558f6a3f3321d50d044a248b\
             a595cfc3efd3d2adc97334da732413f5cbf4751c362ba1d53862ac1e8dabeee8").unwrap();
        let h = Hash::new(1, &hex::decode("00010203040506070809").unwrap()).unwrap();
        assert_eq!(h.get_version(), 1);
        assert_eq!(h.as_bytes(), &digest[..]);
    }
}
