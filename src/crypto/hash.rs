use constant_time_eq::constant_time_eq;
use std::fmt;
use std::io::Read;
use byteorder::ReadBytesExt;
use std::hash;
use std::cmp;
use std::cmp::Ordering;

use crypto::error::CryptoError;
use crypto::sodium::{HASH_BYTES, blake2b, Blake2BState};

const DEFAULT_HASH_VERSION: u8 = 1;
const MIN_HASH_VERSION: u8 = 1;
const MAX_HASH_VERSION: u8 = 1;

/// Crytographically secure hash of data. Can be signed by a FullKey. It is impractical to generate an 
/// identical hash from different data.
///
/// # Supported Versions
/// - 0: Null hash. Used to refer to hash of parent document
/// - 1: Blake2B hash with 32 bytes of digest
#[derive(Clone)]
pub struct Hash {
    version: u8,
    digest: [u8; HASH_BYTES],
}

#[derive(Clone)]
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
        for i in 0..HASH_BYTES {
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

    pub fn new(data: &[u8]) -> Hash {
        debug_assert!(DEFAULT_HASH_VERSION <= MAX_HASH_VERSION);
        debug_assert!(DEFAULT_HASH_VERSION >= MIN_HASH_VERSION);
        let mut hash = Hash {
            version: DEFAULT_HASH_VERSION,
            digest: [0;HASH_BYTES]
        };
        blake2b(&mut hash.digest, data);
        hash
    }

    pub fn with_version(version: u8, data: &[u8]) -> Result<Hash, CryptoError> {
        if version > MAX_HASH_VERSION || version < MIN_HASH_VERSION {
            return Err(CryptoError::UnsupportedVersion);
        }
        let mut hash = Hash {version, digest: [0;HASH_BYTES]};
        blake2b(&mut hash.digest, data);
        Ok(hash)
    }

    pub fn new_empty() -> Hash {
        Hash { version: 0, digest: [0; HASH_BYTES] }
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
        if version == 0 { return Ok(Hash { version, digest:[0;HASH_BYTES] }); }
        if version != 1 { return Err(CryptoError::UnsupportedVersion); }
        let mut hash = Hash {version, digest:[0;HASH_BYTES]};
        buf.read_exact(&mut hash.digest).map_err(CryptoError::Io)?;
        Ok(hash)
    }
}

impl HashState {
    pub fn new() -> HashState {
        debug_assert!(DEFAULT_HASH_VERSION <= MAX_HASH_VERSION);
        debug_assert!(DEFAULT_HASH_VERSION >= MIN_HASH_VERSION);
        HashState {
            version: DEFAULT_HASH_VERSION,
            state: Blake2BState::new()
        }
    }

    pub fn with_version(version: u8) -> Result<HashState, CryptoError> {
        if version > MAX_HASH_VERSION || version < MIN_HASH_VERSION {
            return Err(CryptoError::UnsupportedVersion);
        }
        Ok(HashState { version, state: Blake2BState::new() })
    }

    pub fn update(&mut self, data: &[u8]) {
        self.state.update(data);
    }

    pub fn get_hash(&self) -> Hash {
        let mut hash = Hash { version: self.version, digest: [0;HASH_BYTES] };
        self.state.get_hash(&mut hash.digest);
        hash
    }

    pub fn finalize(self) -> Hash {
        let mut hash = Hash { version: self.version, digest: [0;HASH_BYTES] };
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
            let h = Hash::new(&ref_input[..]);
            let mut state: HashState = HashState::new();
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
        match Hash::with_version(0, &[1,2]).unwrap_err() {
            CryptoError::UnsupportedVersion => (),
            _ => panic!("New hash should always fail on version 0"),
        };
        match HashState::with_version(0).unwrap_err() {
            CryptoError::UnsupportedVersion => (),
            _ => panic!("HashState should always fail on version 0"),
        };
        let digest = hex::decode(
            "8b57a796a5d07cb04cc1614dfc2acb3f73edc712d7f433619ca3bbe66bb15f49").unwrap();
        let h = Hash::new(&hex::decode("00010203040506070809").unwrap());
        assert_eq!(h.get_version(), 1);
        assert_eq!(h.digest(), &digest[..]);
    }

    #[test]
    fn empty() {
        let h = Hash::new_empty();
        let digest = [0u8; HASH_BYTES];
        assert_eq!(h.get_version(), 0);
        assert_eq!(h.digest(), &digest[..]);
        enc_dec(h);
    }
}
