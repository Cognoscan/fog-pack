use constant_time_eq::constant_time_eq;
use std::fmt;
use std::io::Read;
use byteorder::ReadBytesExt;
use std::hash;
use std::cmp;
use std::cmp::Ordering;

use crypto::error::CryptoError;
use crypto::sodium::{blake2b, Blake2BState};

/// Crytographically secure hash of data. Can be signed by a FullKey. It is impractical to generate an 
/// identical hash from different data.
///
/// # Supported Versions
/// - 0: Null hash. Used to refer to hash of parent document
/// - 1: Blake2B hash with 64 bytes of digest
#[derive(Clone)]
pub struct Hash {
    version: u8,
    digest: [u8; 64],
}

pub struct HashState {
    version: u8,
    state: Blake2BState,
}

impl Eq for Hash { }

impl PartialEq for Hash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version && constant_time_eq(&self.digest, &other.digest)
    }
}

// Not constant time, as no cryptographic operation requires Ord. This is solely for ordering in a 
// BTree
impl cmp::Ord for Hash {
    fn cmp(&self, other: &Hash) -> Ordering {
        if self.version > other.version {
            return Ordering::Greater;
        }
        else if self.version < other.version {
            return Ordering::Less;
        }
        for i in 0..64 {
            if self.digest[i] > other.digest[i] {
                return Ordering::Greater;
            }
            else if self.digest[i] < other.digest[i] {
                return Ordering::Less;
            }
        }
        Ordering::Equal
    }
}

impl cmp::PartialOrd for Hash {
    fn partial_cmp(&self, other: &Hash) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{} {{ version: {:?}, digest: {:x?} }}", stringify!(Hash), &self.version, &self.digest[..])
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

    pub fn new_empty() -> Hash {
        Hash { version: 0, digest: [0; 64] }
    }

    pub fn get_version(&self) -> u8 {
        self.version
    }

    pub fn digest(&self) -> &[u8] {
        &self.digest
    }

    pub fn len(&self) -> usize {
        if self.version == 0 {
            1
        }
        else {
            65
        }
    }

    pub fn encode(&self, buf: &mut Vec<u8>) {
        buf.reserve(self.len());
        buf.push(self.version);
        if self.version != 0 {
            buf.extend_from_slice(&self.digest);
        }
    }

    pub fn decode(buf: &mut &[u8]) -> Result<Hash, CryptoError> {
        let version = buf.read_u8().map_err(CryptoError::Io)?;
        if version == 0 { return Ok(Hash { version, digest:[0;64] }); }
        if version != 1 { return Err(CryptoError::UnsupportedVersion); }
        let mut hash = Hash {version, digest:[0;64]};
        buf.read_exact(&mut hash.digest).map_err(CryptoError::Io)?;
        Ok(hash)
    }
}

impl HashState {
    pub fn new(version: u8) -> Result<HashState, CryptoError> {
        if version != 1 { return Err(CryptoError::UnsupportedVersion); }
        Ok(HashState { version, state: Blake2BState::new() })
    }

    pub fn update(&mut self, data: &[u8]) {
        self.state.update(data);
    }

    pub fn get_hash(&self) -> Hash {
        let mut hash = Hash { version: self.version, digest: [0;64] };
        self.state.get_hash(&mut hash.digest);
        hash
    }

    pub fn finalize(self) -> Hash {
        let mut hash = Hash { version: self.version, digest: [0;64] };
        self.state.finalize(&mut hash.digest);
        hash
    }
}

impl fmt::Debug for HashState {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{} {{ version: {:?} }}", stringify!(HashState), &self.version)
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
        h.encode(&mut v);
        let hd = Hash::decode(&mut &v[..]).unwrap();
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
            let mut state: HashState = HashState::new(1).unwrap();
            state.update(&ref_input[..]);
            let h2 = state.get_hash();
            let h3 = state.finalize();
            assert_eq!(h.version, 1u8);
            assert_eq!(h.digest[..], ref_hash[..]);
            assert_eq!(h2.version, 1u8);
            assert_eq!(h2.digest[..], ref_hash[..]);
            assert_eq!(h3.version, 1u8);
            assert_eq!(h3.digest[..], ref_hash[..]);
            enc_dec(h)
        }
    }

    #[test]
    fn edge_cases() {
        match Hash::new(0, &[1,2]).unwrap_err() {
            CryptoError::UnsupportedVersion => (),
            _ => panic!("New hash should always fail on version 0"),
        };
        match HashState::new(0).unwrap_err() {
            CryptoError::UnsupportedVersion => (),
            _ => panic!("HashState should always fail on version 0"),
        };
        let digest = hex::decode(
            "29102511d749db3cc9b4e335fa1f5e8faca8421d558f6a3f3321d50d044a248b\
             a595cfc3efd3d2adc97334da732413f5cbf4751c362ba1d53862ac1e8dabeee8").unwrap();
        let h = Hash::new(1, &hex::decode("00010203040506070809").unwrap()).unwrap();
        assert_eq!(h.get_version(), 1);
        assert_eq!(h.digest(), &digest[..]);
    }

    #[test]
    fn empty() {
        let h = Hash::new_empty();
        let digest = [0u8; 64];
        assert_eq!(h.get_version(), 0);
        assert_eq!(h.digest(), &digest[..]);
        enc_dec(h);
    }
}
