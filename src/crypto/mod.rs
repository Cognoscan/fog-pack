use std::{fmt, io};
use std::error::Error;
use std::collections::HashMap;
use rmpv::Value;
use rmpv;
use super::ExtType;
use std::io::{Write,Read};
use byteorder::{ReadBytesExt,WriteBytesExt};

mod hash;
mod key;
mod stream;
mod lock;
mod sodium;

use self::key::{FullKey,FullIdentity};
use self::stream::FullStreamKey;
use self::lock::{Lock,LockType};

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
    NotInStorage,
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
            CryptoError::NotInStorage         => write!(f, "Provided Key/Identity/StreamKey is not in storage"),
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
            CryptoError::NotInStorage         => "Key/Identity/StreamKey not in storage",
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

pub enum LockBoxContents {
    Key(Key),
    StreamKey(StreamKey),
    Value(Value),
}

enum LockBoxType {
    Key,
    StreamKey,
    Value,
}

impl LockBoxType {
    fn from_u8(i: u8) -> Option<LockBoxType> {
        match self {
            1 => Some(LockBoxType::Key),
            2 => Some(LockBoxType::StreamKey),
            3 => Some(LockBoxType::Value),
            _ => None
        }
    }
    fn to_u8(self) -> u8 {
        match self {
            LockBoxType::Key       => 1,
            LockBoxType::StreamKey => 2,
            LockBoxType::Value     => 3,
        }
    }
}

pub struct Vault {
    perm_keys: HashMap <Key, FullKey>,
    perm_ids: HashMap <Identity, FullIdentity>,
    perm_streams: HashMap <StreamKey, FullStreamKey>,
    temp_keys: HashMap <Key, FullKey>,
    temp_ids: HashMap <Identity, FullIdentity>,
    temp_streams: HashMap <StreamKey, FullStreamKey>,
}

impl Vault {

    /// Create a brand-new empty Vault
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

    /// Create a new key and add to permanent store.
    pub fn new_key(&mut self) -> (Key, Identity) {
        let (k, id) = FullKey::new_pair().unwrap();
        let key_ref = k.get_key_ref();
        let id_ref = id.get_identity_ref();
        self.perm_keys.insert(key_ref.clone(),k);
        self.perm_ids.insert(id_ref.clone(), id);
        (key_ref, id_ref)
    }

    /// Create a new Stream and add to permanent store.
    pub fn new_stream(&mut self) -> StreamKey {
        let k = FullStreamKey::new();
        let k_ref = k.get_stream_ref();
        self.perm_streams.insert(k_ref.clone(), k);
        k_ref
    }

    /// Moves both the Key and Identity to the permanent store.
    pub fn key_to_perm(&mut self, k: &Key) -> bool {
        // Move key and hold onto FullKey if needed to reconstruct identity
        let key = match self.temp_keys.remove(&k) {
            Some(key) => {
                self.perm_keys.insert(k.clone(),key);
                self.perm_keys.get(&k)
            },
            None => self.perm_keys.get(&k),
        };
        // Halt now if we don't actually have the key
        let key = match key {
            Some(v) => v,
            None => { return false; },
        };
        // Since we have the key, we can make sure it ends up in the identity store
        let id_ref = k.get_identity();
        match self.temp_ids.remove(&id_ref) {
            Some(id) => {
                self.perm_ids.insert(id_ref,id);
            },
            None => {
                // The below code shouldn't need to run unless there's a logic error elsewhere
                if !self.perm_ids.contains_key(&id_ref) {
                    // Panic occurs only if a bad key made it into the store. This is panic-worthy.
                    let id = key.get_identity()
                        .expect("Bad Key was unexpectedly found in crypto vault!");
                    self.perm_ids.insert(id_ref,id);
                }
            },
        }
        true
    }

    /// Moves the given Identity to the permanent store.
    pub fn identity_to_perm(&mut self, id: &Identity) -> bool {
        match self.temp_ids.remove(&id) {
            Some(full_id) => {self.perm_ids.insert(id.clone(),full_id); true},
            None => self.perm_ids.contains_key(&id),
        }
    }

    /// Moves the given Stream to the permanent store.
    pub fn stream_to_perm(&mut self, stream: &StreamKey) -> bool {
        match self.temp_streams.remove(&stream) {
            Some(full_stream) => {self.perm_streams.insert(stream.clone(),full_stream); true},
            None => self.perm_streams.contains_key(&stream),
        }
    }

    /// Drops just the given key from every store.
    pub fn drop_key(&mut self, k: Key) {
        self.perm_keys.remove(&k);
        self.temp_keys.remove(&k);
    }

    /// Drops the given identity from every store. Also drops the key if we have it.
    pub fn drop_identity(&mut self, id: Identity) {
        self.perm_ids.remove(&id);
        self.temp_ids.remove(&id);
        self.drop_key(id.get_key());
    }

    /// Drops the given stream from every store.
    pub fn drop_stream(&mut self, stream: StreamKey) {
        self.perm_streams.remove(&stream);
        self.temp_streams.remove(&stream);
    }

    /// Encrypt a msgpack Value for a given Identity, returning the StreamKey used and the 
    /// encrypted Value
    pub fn encrypt_for(&mut self, data: Value, id: &Identity) -> Result<(StreamKey, Value), CryptoError> {
        let (lock,full_stream) = {
            let full_id = self.get_id(id)?;
            Lock::from_identity(full_id)?
        };
        let stream_ref = full_stream.get_stream_ref();
        self.temp_streams.insert(stream_ref.clone(), full_stream);
        let mut plaintext: Vec<u8> = Vec::new();
        plaintext.push(LockBoxType::Value.to_u8());
        rmpv::encode::write_value(&mut plaintext, &data).or(Err(CryptoError::BadMsgPack))?;
        let mut crypt: Vec<u8> = Vec::with_capacity(lock.len());
        lock.write(&mut crypt);
        lock.encrypt(&plaintext, &[], &mut crypt)?;
        Ok((stream_ref, Value::Ext(ExtType::LockBox.to_i8(), crypt)))
    }

    /// Encrypt a msgpack Value using a given StreamKey, returning the encrypted Value
    pub fn encrypt_stream(&self, data: Value, stream: &StreamKey) -> Result<Value, CryptoError> {
        let full_stream = self.get_stream(stream)?;
        let lock = Lock::from_stream(full_stream)?;
        let mut plaintext: Vec<u8> = Vec::new();
        plaintext.push(LockBoxType::Value.to_u8());
        rmpv::encode::write_value(&mut plaintext, &data).or(Err(CryptoError::BadMsgPack))?;
        let mut crypt: Vec<u8> = Vec::with_capacity(lock.len());
        lock.write(&mut crypt);
        lock.encrypt(&plaintext, &[], &mut crypt)?;
        Ok(Value::Ext(ExtType::LockBox.to_i8(), crypt))
    }

    /// Decrypt a msgpack Value using stored keys, returning the decrypted Value and keys needed 
    /// for it. If the StreamKey used is new, it will be stored for temporary use
    pub fn decrypt(&mut self, crypt: Value) -> Result<(Option<Key>, StreamKey, LockBoxContents), CryptoError> {
        let (ext_type, crypt_data) = crypt.as_ext().ok_or(CryptoError::BadMsgPack)?;
        let mut crypt_data = &crypt_data[..];
        if ext_type != ExtType::LockBox.to_i8() { return Err(CryptoError::BadMsgPack); }
        let mut lock = Lock::read(&mut crypt_data)?;
        let need = lock.needs().ok_or(CryptoError::BadMsgPack)?.clone(); // Error should never occur
        match need {
            LockType::Identity(id_raw) => {
                let key_ref = self::key::key_from_id(lock.get_version(), id_raw.0.clone());
                let key = self.get_key(&key_ref)?;
                lock.decode_identity(&key)?;
                let stream = lock.get_stream().ok_or(CryptoError::BadKey)?;
                let stream_ref = stream.get_stream_ref();
                let mut plaintext: Vec<u8> = Vec::new();
                lock.decrypt(&crypt_data, &[], &mut plaintext)?;
                let mut plaintext_data = &plaintext[..];
                let content_type = plaintext_data.read_u8().or(Err(CryptoError::BadMsgPack))?;
                let content_type = LockBoxType::from_u8(content_type).ok_or(CryptoError::BadMsgPack)?;
                match content_type {
                    LockBoxType::Value => {
                        let value = rmpv::decode::read_value(&mut plaintext_data).or(Err(CryptoError::BadMsgPack))?;
                        Ok((Some(key_ref), stream_ref, LockBoxContents::Value(value)))
                    },
                    LockBoxType::Key => {
                        let new_key = 
                    },
                    LockBoxType::StreamKey => {
                    },
                }
            },
            LockType::Stream(stream_raw) => {
                let stream_ref = self::stream::stream_from_id(lock.get_version(), stream_raw.clone());
                let stream = self.get_stream(&stream_ref)?;
                let stream_ref = stream.get_stream_ref();
                lock.decode_stream(&stream);
                let mut plaintext: Vec<u8> = Vec::new();
                lock.decrypt(&crypt_data, &[], &mut plaintext)?;
                let value = rmpv::decode::read_value(&mut &plaintext[..]).or(Err(CryptoError::BadMsgPack))?;
                Ok((None, stream_ref, value))
            },
        }
    }

    fn get_key(&self, k: &Key) -> Result<&FullKey, CryptoError> {
        self.perm_keys.get(k).or(self.temp_keys.get(k)).ok_or(CryptoError::NotInStorage)
    }

    fn get_stream(&self, s: &StreamKey) -> Result<&FullStreamKey, CryptoError> {
        self.perm_streams.get(s).or(self.temp_streams.get(s)).ok_or(CryptoError::NotInStorage)
    }

    fn get_id(&self, id: &Identity) -> Result<&FullIdentity, CryptoError> {
        self.perm_ids.get(id).or(self.temp_ids.get(id)).ok_or(CryptoError::NotInStorage)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_encrypt() {
        init().unwrap();
        let mut vault = Vault::new();
        let stream = vault.new_stream();
        let data: Value = Value::from("test");
        let encrypted = vault.encrypt_stream(data.clone(), &stream).unwrap();
        let (key_option, stream_d, data_d)  = vault.decrypt(encrypted).unwrap();
        assert_eq!(stream, stream_d);
        assert_eq!(data, data_d);
        assert_eq!(key_option, None);
    }

    #[test]
    fn identity_encrypt() {
        init().unwrap();
        let mut vault = Vault::new();
        let (key, id) = vault.new_key();
        let data: Value = Value::from("test");
        let (stream, encrypted) = vault.encrypt_for(data.clone(), &id).unwrap();
        let (key_option, stream_d, data_d)  = vault.decrypt(encrypted).unwrap();
        assert_eq!(stream, stream_d);
        assert_eq!(data, data_d);
        assert_eq!(key_option, Some(key));
    }
}
