use std::collections::HashMap;
use std::fs::File;
use std::io::{Write,BufReader, Read};

mod sodium;
mod error;
mod hash;
mod key;
mod stream;
mod lock;

use self::key::{FullKey, FullIdentity};
use self::stream::FullStreamKey;

pub use self::error::CryptoError;
pub use self::hash::{Hash, HashState};
pub use self::key::{Key, Identity};
pub use self::stream::StreamKey;
pub use self::lock::Lockbox;

use self::sodium::{Nonce, PasswordConfig, SecretKey};

/// Initializes the underlying crypto library and makes all random number generation functions 
/// thread-safe. *Must* be called successfully before using the rest of this library.
pub fn init() -> Result<(), ()> {
    sodium::init()
}

/// Contains either the Key, StreamKey or data that was in the Lockbox
#[derive(Debug)]
pub enum LockboxContent {
    Key(Key),
    StreamKey(StreamKey),
    Data(Vec<u8>),
}

enum LockboxType {
    Key,
    StreamKey,
    Data,
}

impl LockboxType {
    fn from_u8(i: u8) -> Option<LockboxType> {
        match i {
            1 => Some(LockboxType::Key),
            2 => Some(LockboxType::StreamKey),
            3 => Some(LockboxType::Data),
            _ => None
        }
    }
    fn to_u8(self) -> u8 {
        match self {
            LockboxType::Key       => 1,
            LockboxType::StreamKey => 2,
            LockboxType::Data      => 3,
        }
    }
}

/// The level of security to be provided by a password hashing function.
pub enum PasswordLevel {
    /// For online, reasonably fast password unlock. Requires 64 MiB of RAM.
    Interactive,
    /// Longer password unlock. Requires 256 MiB of RAM.
    Moderate,
    /// For sensitive, non-interactive operations. Requires 1024 MiB of RAM.
    Sensitive,
}


pub struct Vault {
    config: PasswordConfig,
    root_key: SecretKey,
    perm_keys: HashMap <Key, FullKey>,
    perm_streams: HashMap <StreamKey, FullStreamKey>,
    temp_keys: HashMap <Key, FullKey>,
    temp_streams: HashMap <StreamKey, FullStreamKey>,
}

impl Vault {

    /// Create a brand-new empty Vault. Can fail if the OS doesn't let us allocate enough memory 
    /// for the password hashing algorithm, or if the password is too short or too long.
    /// 
    /// Consumes the password string in the process and zeroes it out before dropping it.
    pub fn new_from_password(security: PasswordLevel, password: String) -> Result<Vault, ()> {
        let config = match security {
            PasswordLevel::Interactive => PasswordConfig::interactive(),
            PasswordLevel::Moderate => PasswordConfig::moderate(),
            PasswordLevel::Sensitive => PasswordConfig::sensitive(),
        };
        let root_key = sodium::password_to_key(password, &config)?;
        Ok(Vault {
            config,
            root_key,
            perm_keys: Default::default(),
            perm_streams: Default::default(),
            temp_keys: Default::default(),
            temp_streams: Default::default(),
        })
    }

    /// Write the entire keystore out to a file.
    pub fn write_to_file(&self, f: &mut File) -> std::io::Result<()> {
        // Write the PasswordConfig, then all perm_keys, then all perm_streams
        let mut data = Vec::with_capacity(
            PasswordConfig::max_len() +
            self.perm_keys.len() * (1+FullKey::max_len()) +
            self.perm_streams.len() * (1+FullStreamKey::max_len())
            );
        self.config.encode(&mut data);
        let start_of_keys = data.len();
        for key in self.perm_keys.values() {
            data.push(1u8);
            key.encode(&mut data);
        }
        for key in self.perm_streams.values() {
            data.push(2u8);
            key.encode(&mut data);
        }
        let nonce = Nonce::new();
        let tag = {
            let (pre_data, mut key_data) = data.split_at_mut(start_of_keys);
            sodium::aead_encrypt(&mut key_data, &pre_data, &nonce, &self.root_key)
        };
        data.extend_from_slice(&tag.0);
        f.write_all(&data[..])?;
        f.sync_data()?;
        Ok(())
    }

    /// Read the entire keystore from a file, returning a Vault.
    /// 
    /// Consumes the password string in the process and zeroes it out before dropping it.
    pub fn read_from_file(f: &mut File, password: String) -> std::io::Result<Vault> {
        let mut buf_reader = BufReader::new(f);
        let mut content = Vec::new();
        buf_reader.read_to_end(&mut content)?;
        let mut rd: &[u8] = &content[..];
        let config = sodium::PasswordConfig::decode(&mut rd)?;
        let root_key = sodium::password_to_key(password, &config)
            .map_err(|_e| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Password hashing failed"))?;
        let mut vault = Vault {
            config,
            root_key,
            perm_keys: Default::default(),
            perm_streams: Default::default(),
            temp_keys: Default::default(),
            temp_streams: Default::default(),
        };
        let mut bool done = false;
        while !done {
            record_type = rd.read_u8(
        }
        Ok(vault)
    }

    /// Create a new key and add to permanent store.
    pub fn new_key(&mut self) -> Key {
        let (k, _id) = FullKey::new_pair().unwrap();
        let key_ref = k.get_key_ref();
        self.perm_keys.insert(key_ref.clone(),k);
        key_ref
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
        match key {
            Some(_) => true,
            None => false,
        }
    }

    /// Moves the given Stream to the permanent store.
    pub fn stream_to_perm(&mut self, stream: &StreamKey) -> bool {
        match self.temp_streams.remove(&stream) {
            Some(full_stream) => {self.perm_streams.insert(stream.clone(),full_stream); true},
            None => self.perm_streams.contains_key(&stream),
        }
    }

    /// Drops the given key from every store.
    pub fn drop_key(&mut self, k: Key) {
        self.perm_keys.remove(&k);
        self.temp_keys.remove(&k);
    }

    /// Drops the given stream from every store.
    pub fn drop_stream(&mut self, stream: StreamKey) {
        self.perm_streams.remove(&stream);
        self.temp_streams.remove(&stream);
    }

    fn data_from_lockbox_content(&self, data: LockboxContent) -> Result<Vec<u8>, CryptoError> {
        let m = match data {
            LockboxContent::Key(k) => {
                let mut m = vec![LockboxType::Key.to_u8()];
                self.get_key(&k)?.encode(&mut m);
                m
            },
            LockboxContent::StreamKey(sk) => {
                let mut m = vec![LockboxType::StreamKey.to_u8()];
                self.get_stream(&sk)?.encode(&mut m);
                m
            },
            LockboxContent::Data(mut d) => {
                d.insert(0, LockboxType::Data.to_u8());
                d
            },
        };
        Ok(m)
    }

    /// Create a Lockbox using a StreamKey.
    pub fn encrypt_using_stream(&self, data: LockboxContent, stream: &StreamKey) -> Result<Lockbox, CryptoError> {
        let message = self.data_from_lockbox_content(data)?;
        let stream_key = self.get_stream(stream)?;
        lock::lockbox_from_stream(stream_key, message)
    }

    /// Create a Lockbox using an Identity.
    pub fn encrypt_using_identity(&self, data: LockboxContent, id: &Identity) -> Result<Lockbox, CryptoError> {
        let message = self.data_from_lockbox_content(data)?;
        let full_id = FullIdentity::from_identity(id)?;
        let (lock, _) = lock::lockbox_from_identity(&full_id, message)?;
        Ok(lock)
    }

    /// Create a Lockbox using an Identity and keep the StreamKey used for it.
    pub fn encrypt_using_identity_keep_key(&mut self, data: LockboxContent, id: &Identity) -> Result<(Lockbox, StreamKey), CryptoError> {
        let message = self.data_from_lockbox_content(data)?;
        let full_id = FullIdentity::from_identity(id)?;
        let (lock, stream_key) = lock::lockbox_from_identity(&full_id, message)?;
        let stream_key_ref = stream_key.get_stream_ref();
        self.perm_streams.insert(stream_key_ref.clone(), stream_key);
        Ok((lock, stream_key_ref))
    }

    /// Attempt to open a Lockbox and return the contents
    pub fn decrypt(&mut self, lock: Lockbox) -> Result<LockboxContent, CryptoError> {
        let mut data = if lock.uses_stream() {
            let stream = lock.get_stream().expect("Lockbox claimed to have stream, but didn't");
            let key = self.get_stream(&stream)?;
            lock::decrypt_lockbox(key, lock)?
        }
        else {
            let key = lock::get_key(&lock).expect("Lockbox claimed to have identity, but didn't");
            let key = self.get_key(&key)?;
            let key = lock::stream_key_from_lockbox(&key, &lock)?;
            lock::decrypt_lockbox(&key, lock)?
        };
        if data.len() < 2 { return Err(CryptoError::BadFormat); }
        match LockboxType::from_u8(data[0]) {
            Some(LockboxType::Key) => {
                let (full_key, _) = FullKey::decode(&mut &data[1..])?;
                let key = full_key.get_key_ref();
                self.temp_keys.insert(key.clone(), full_key);
                Ok(LockboxContent::Key(key))
            }
            Some(LockboxType::StreamKey) => {
                let full_stream = FullStreamKey::decode(&mut &data[1..])?;
                let stream = full_stream.get_stream_ref();
                self.temp_streams.insert(stream.clone(), full_stream);
                Ok(LockboxContent::StreamKey(stream))
            }
            Some(LockboxType::Data) => {
                data.remove(0);
                Ok(LockboxContent::Data(data))
            }
            None => Err(CryptoError::BadFormat)
        }
    }

    /*
    /// Encrypt something for a given Identity, returning the StreamKey used and an encrypted Value
    pub fn encrypt_for(&mut self, data: LockboxContents, id: &Identity) -> Result<(StreamKey, Value), CryptoError> {
        // Construct lock
        let (lock, full_stream) = {
            let full_id = FullIdentity::from_identity(id)?;
            Lock::from_identity(full_id)?
        };
        let stream_ref = full_stream.get_stream_ref();
        self.temp_streams.insert(stream_ref.clone(), full_stream);
        // Encode & encrypt data
        let mut plaintext = self.encode_data(data)?;
        let crypt = Vault::encrypt_raw(&plaintext, lock)?;
        // Zero out plaintext
        sodium::memzero(&mut plaintext);
        Ok((stream_ref, crypt))
    }

    /// Encrypt something using a given StreamKey, returning an encrypted Value
    pub fn encrypt_stream(&self, data: LockboxContents, stream: &StreamKey) -> Result<Value, CryptoError> {
        // Construct lock
        let full_stream = self.get_stream(stream)?;
        let lock = Lock::from_stream(full_stream)?;
        // Encode & encrypt data
        let mut plaintext = self.encode_data(data)?;
        let crypt = Vault::encrypt_raw(&plaintext, lock)?;
        // Zero out plaintext
        sodium::memzero(&mut plaintext);
        Ok(crypt)
    }

    fn encode_data(&self, data: LockboxContents) -> Result<Vec<u8>, CryptoError> {
        let mut plaintext: Vec<u8> = Vec::new();
        match data {
            LockboxContents::Value(v) => {
                plaintext.push(LockboxType::Value.to_u8());
                //rmpv::encode::write_value(&mut plaintext, &v).or(Err(CryptoError::BadFormat))?;
            },
            LockboxContents::Key(k) => {
                plaintext.push(LockboxType::Key.to_u8());
                let full_key = self.get_key(&k)?;
                full_key.write(&mut plaintext)?;
            },
            LockboxContents::StreamKey(s) => {
                plaintext.push(LockboxType::StreamKey.to_u8());
                let full_stream = self.get_stream(&s)?;
                full_stream.write(&mut plaintext)?;
            },
        };
        Ok(plaintext)
    }

    fn encrypt_raw(data: &[u8], lock: Lock) -> Result<Value, CryptoError> {
        let mut crypt: Vec<u8> = Vec::with_capacity(lock.len()+lock.encrypt_len(data.len()));
        lock.write(&mut crypt)?;
        lock.encrypt(data, &[], &mut crypt)?;
        // FIXME: Should actually encode as a Lockbox
        Ok(Value::Null)
        //Ok(Value::Ext(ExtType::Lockbox.to_i8(), crypt))
    }

    /// Decrypt a msgpack Value using stored keys, returning the decrypted Value and keys needed 
    /// for it. If the StreamKey used is new, it will be stored for temporary use
    pub fn decrypt(&mut self, crypt: Value) -> Result<(Option<Key>, StreamKey, LockboxContents), CryptoError> {
        Err(CryptoError::BadFormat)
        /*
        // Unpack the Value and check it
        let (ext_type, crypt_data) = crypt.as_ext().ok_or(CryptoError::BadFormat)?;
        let mut crypt_data = &crypt_data[..];
        if ext_type != ExtType::Lockbox.to_i8() { return Err(CryptoError::BadFormat); }

        // Extract the Lock used for encryption and determine what is needed for decryption
        let mut lock = Lock::read(&mut crypt_data)?;
        let need = lock.needs().ok_or(CryptoError::BadFormat)?.clone(); // Error should never occur

        // Decrypt depending on lock type
        let (key_ref, stream_ref) = match need {
            LockType::Identity(id_raw) => {
                // Fetch key and prepare lock for decoding
                let key_ref = self::key::key_from_id(lock.get_version(), id_raw.0.clone());
                lock.decode_identity(
                    self.get_key(&key_ref)?
                )?;
                let stream = lock.get_stream().ok_or(CryptoError::BadKey)?;
                let stream_ref = stream.get_stream_ref();
                self.temp_streams.insert(stream_ref.clone(),stream);
                (Some(key_ref), stream_ref)
            },
            LockType::Stream(stream_raw) => {
                let stream_ref = self::stream::stream_from_id(lock.get_version(), stream_raw.clone());
                let stream = self.get_stream(&stream_ref)?;
                let stream_ref = stream.get_stream_ref();
                lock.decode_stream(&stream)?;
                (None, stream_ref)
            },
        };
        // Decode raw data
        let mut plaintext: Vec<u8> = Vec::new();
        lock.decrypt(&crypt_data, &[], &mut plaintext)?;
        let mut plaintext_data = &plaintext[..];
        let content_type = plaintext_data.read_u8().or(Err(CryptoError::BadFormat))?;
        let content_type = LockboxType::from_u8(content_type).ok_or(CryptoError::BadFormat)?;
        let content = match content_type {
            LockboxType::Value => {
                let value = rmpv::decode::read_value(&mut plaintext_data).or(Err(CryptoError::BadFormat))?;
                LockboxContents::Value(value)
            },
            LockboxType::Key => {
                // Read out the key and put it in the temp store
                let (key, id) = FullKey::read(&mut plaintext_data)?;
                let key_ref = key.get_key_ref();
                self.temp_keys.insert(key_ref.clone(), key);
                self.temp_ids.insert(id.get_identity_ref(), id);
                LockboxContents::Key(key_ref)
            },
            LockboxType::StreamKey => {
                let stream = FullStreamKey::read(&mut plaintext_data)?;
                let stream_ref = stream.get_stream_ref();
                self.temp_streams.insert(stream_ref.clone(), stream);
                LockboxContents::StreamKey(stream_ref)
            },
        };
        Ok((key_ref, stream_ref, content))
            */
    }
    */

    fn get_key(&self, k: &Key) -> Result<&FullKey, CryptoError> {
        self.perm_keys.get(k).or(self.temp_keys.get(k)).ok_or(CryptoError::NotInStorage)
    }

    fn get_stream(&self, s: &StreamKey) -> Result<&FullStreamKey, CryptoError> {
        self.perm_streams.get(s).or(self.temp_streams.get(s)).ok_or(CryptoError::NotInStorage)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_encrypt_value() {
        // Setup keys
        init().unwrap();
        let mut vault = Vault::new();
        let stream = vault.new_stream();
        // Run test on data
        let data = Value::from("test");
        let encrypted = vault.encrypt_stream(LockboxContents::Value(data.clone()), &stream).unwrap();
        let (key_option, stream_d, data_d)  = vault.decrypt(encrypted).unwrap();
        let data_d = match data_d {
            LockboxContents::Value(v) => v,
            _ => panic!("Lockbox should contain a value!"),
        };
        assert_eq!(stream, stream_d);
        assert_eq!(data, data_d);
        assert_eq!(key_option, None);
    }

    #[test]
    fn identity_encrypt_value() {
        // Setup keys
        init().unwrap();
        let mut vault = Vault::new();
        let key = vault.new_key();
        let id = key.get_identity();
        // Run test on data
        let data = Value::from("test");
        let (stream, encrypted) = vault.encrypt_for(LockboxContents::Value(data.clone()), &id).unwrap();
        let (key_option, stream_d, data_d)  = vault.decrypt(encrypted).unwrap();
        let data_d = match data_d {
            LockboxContents::Value(v) => v,
            _ => panic!("Lockbox should contain a value!"),
        };
        assert_eq!(stream, stream_d);
        assert_eq!(data, data_d);
        assert_eq!(key_option, Some(key));
    }

    #[test]
    fn stream_encrypt_key() {
        // Setup keys
        init().unwrap();
        let mut vault = Vault::new();
        let key = vault.new_key();
        let stream = vault.new_stream();
        // Run test on data
        let data = vault.new_key();
        let encrypted = vault.encrypt_stream(LockboxContents::Key(data.clone()), &stream).unwrap();
        let (key_option, stream_d, data_d)  = vault.decrypt(encrypted).unwrap();
        let data_d = match data_d {
            LockboxContents::Key(k) => k,
            _ => panic!("Lockbox should contain a value!"),
        };
        assert_eq!(stream, stream_d);
        assert_eq!(data, data_d);
        assert_eq!(key_option, None);
    }

    #[test]
    fn identity_encrypt_key() {
        // Setup keys
        init().unwrap();
        let mut vault = Vault::new();
        let key = vault.new_key();
        let id = key.get_identity();
        let data = Value::from("test");
        // Run test on data
        let data = vault.new_key();
        let (stream, encrypted) = vault.encrypt_for(LockboxContents::Key(data.clone()), &id).unwrap();
        let (key_option, stream_d, data_d)  = vault.decrypt(encrypted).unwrap();
        let data_d = match data_d {
            LockboxContents::Key(k) => k,
            _ => panic!("Lockbox should contain a value!"),
        };
        assert_eq!(stream, stream_d);
        assert_eq!(data, data_d);
        assert_eq!(key_option, Some(key));
    }

    #[test]
    fn stream_encrypt_stream() {
        // Setup keys
        init().unwrap();
        let mut vault = Vault::new();
        let stream = vault.new_stream();
        // Run test on data
        let data = vault.new_stream();
        let encrypted = vault.encrypt_stream(LockboxContents::StreamKey(data.clone()), &stream).unwrap();
        let (key_option, stream_d, data_d)  = vault.decrypt(encrypted).unwrap();
        let data_d = match data_d {
            LockboxContents::StreamKey(s) => s,
            _ => panic!("Lockbox should contain a value!"),
        };
        assert_eq!(stream, stream_d);
        assert_eq!(data, data_d);
        assert_eq!(key_option, None);
    }

    #[test]
    fn identity_encrypt() {
        // Setup keys
        init().unwrap();
        let mut vault = Vault::new();
        let key = vault.new_key();
        let id = key.get_identity();
        // Run test on data
        let data = vault.new_stream();
        let (stream, encrypted) = vault.encrypt_for(LockboxContents::StreamKey(data.clone()), &id).unwrap();
        let (key_option, stream_d, data_d)  = vault.decrypt(encrypted).unwrap();
        let data_d = match data_d {
            LockboxContents::StreamKey(s) => s,
            _ => panic!("Lockbox should contain a value!"),
        };
        assert_eq!(stream, stream_d);
        assert_eq!(data, data_d);
        assert_eq!(key_option, Some(key));
    }
}
