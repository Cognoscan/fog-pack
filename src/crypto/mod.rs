use std::{fmt, io};
use std::error::Error;
use std::collections::HashMap;
use rmpv::Value;
use rmpv;
use super::ExtType;

mod hash;
mod key;
mod stream;
mod lock;
mod sodium;

use self::key::{FullKey,FullIdentity,FullSignature};
use self::stream::FullStreamKey;
use self::lock::Lock;

pub use self::hash::Hash;
pub use self::key::{Key, Identity};
pub use self::stream::StreamKey;

#[derive(Debug)]
pub enum CryptoError {
    UnsupportedVersion,
    DecryptFailed,
    BadLength,
    BadKey,
    BadMsgPack,
    Io(io::Error),
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CryptoError::UnsupportedVersion   => write!(f, "Chosen crypto version not supported."),
            CryptoError::DecryptFailed        => write!(f, "Could not decrypt with key"),
            CryptoError::BadKey               => write!(f, "Crypto key is weak or invalid"),
            CryptoError::BadLength            => write!(f, "Provided data length is invalid"),
            CryptoError::BadMsgPack           => write!(f, "MessagePack encode/decode failed"),
            CryptoError::Io(ref err)          => err.fmt(f),
        }
    }
}

impl Error for CryptoError {
    fn description(&self) -> &str {
        match *self {
            CryptoError::UnsupportedVersion   => "unsupported version",
            CryptoError::DecryptFailed        => "decryption failed",
            CryptoError::BadKey               => "weak or invalid key",
            CryptoError::BadLength            => "invalid data length",
            CryptoError::BadMsgPack           => "bad Messagepack value",
            CryptoError::Io(ref err)          => err.description(),
        }
    }
}

impl From<io::Error> for CryptoError {
    fn from(err: io::Error) -> CryptoError {
        CryptoError::Io(err)
    }
}

/// Initializes the underlying crypto library and makes all random number generation functions 
/// thread-safe. *Must* be called successfully before using the rest of this library.
pub fn init() -> Result<(), ()> {
    sodium::init()
}

struct Vault {
    perm_keys: HashMap <Key, FullKey>,
    perm_ids: HashMap <Identity, FullIdentity>,
    perm_streams: HashMap <StreamKey, FullStreamKey>,
    temp_keys: HashMap <Key, FullKey>,
    temp_ids: HashMap <Identity, FullIdentity>,
    temp_streams: HashMap <StreamKey, FullStreamKey>,
}

impl Vault {

    pub fn new() -> Vault {
        Vault {
            perm_keys: Default::default(),
            perm_ids: Default::default(),
            perm_streams: Default::default(),
            temp_keys: Default::default(),
            temp_ids: Default::default(),
            temp_streams: Default::default(),
        }
    }

    /// Create a new key
    pub fn new_key(&mut self) -> (Key, Identity) {
        let (k, id) = FullKey::new_pair().unwrap();
        let key_ref = k.get_key_ref();
        let id_ref = id.get_identity_ref();
        self.temp_keys.insert(key_ref.clone(),k);
        self.temp_ids.insert(id_ref.clone(), id);
        (key_ref, id_ref)
    }

    /// Create a new Stream
    pub fn new_stream(&mut self) -> StreamKey {
        let k = FullStreamKey::new();
        let k_ref = k.get_stream_ref();
        self.temp_streams.insert(k_ref.clone(), k);
        k_ref
    }

    /// Encrypt a msgpack Value for a given Identity, returning the StreamKey used and the 
    /// encrypted Value
    pub fn encrypt_for(&mut self, data: Value, id: &Identity) -> Result<(Value, StreamKey), CryptoError> {
        let full_id = self.perm_ids.get(id)
            .or(self.temp_ids.get(id))
            .ok_or(CryptoError::BadKey)?;
        let (lock,full_stream) = Lock::from_identity(full_id)?;
        let stream_ref = full_stream.get_stream_ref();
        self.temp_streams.insert(stream_ref.clone(), full_stream);
        let mut plaintext: Vec<u8> = Vec::new();
        rmpv::encode::write_value(&mut plaintext, &data).or(Err(CryptoError::BadMsgPack))?;
        let mut crypt: Vec<u8> = Vec::with_capacity(lock.len());
        lock.write(&mut crypt);
        lock.encrypt(&plaintext, &[], &mut crypt)?;
        Ok((Value::Ext(ExtType::LockBox.to_i8(), crypt), stream_ref))
    }

    /// Encrypt a msgpack Value using a given StreamKey, returning the encrypted Value
    pub fn encrypt_stream(&self, data: Value, stream: &StreamKey) -> Result<Value, CryptoError> {
        let full_stream = self.perm_streams.get(stream)
            .or_else(|| self.temp_streams.get(stream))
            .ok_or(CryptoError::BadKey)?;
        let lock = Lock::from_stream(full_stream)?;
        let mut plaintext: Vec<u8> = Vec::new();
        rmpv::encode::write_value(&mut plaintext, &data).or(Err(CryptoError::BadMsgPack))?;
        let mut crypt: Vec<u8> = Vec::with_capacity(lock.len());
        lock.write(&mut crypt);
        lock.encrypt(&plaintext, &[], &mut crypt)?;
        Ok(Value::Ext(ExtType::LockBox.to_i8(), crypt))
    }

    /// Decrypt a msgpack Value using stored keys, returning the decrypted Value and keys needed 
    /// for it. If the StreamKey used is new, it will be stored for temporary use
    pub fn decrypt(&mut self, crypt: Value) -> Result<(Option<Identity>, StreamKey, Value), CryptoError> {
        let (ext_type, crypt_data) = crypt.as_ext().ok_or(CryptoError::BadMsgPack)?;
        let mut crypt_data = &crypt_data[..];
        if ext_type != ExtType::LockBox.to_i8() { return Err(CryptoError::BadMsgPack); }
        let mut lock = Lock::read(&mut crypt_data)?;
        let need = lock.needs().ok_or(CryptoError::BadMsgPack)?; // Error should never occur
        match need {
            LockType::Identity(id_raw) => {
                let id_ref = Identity { version: lock.get_version(), id: id_raw.0 };
            },
            LockType::Stream(stream_raw) => {
                let stream_ref = StreamKey { version: lock.get_version(), id: stream_raw };
            },
        };
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt() {
        init().unwrap();
        let mut vault = Vault::new();
        let stream = vault.new_stream();
        let data: Value = "test";
    }
}
