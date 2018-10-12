use std::{fmt, io};
use std::error::Error;

pub mod hash;
pub mod key;

use libsodium_sys;


pub use self::hash::Hash;
pub use self::key::{Key,Identity};

/// Database Crypto Submodule
/// -------------------------
///
/// This contains all cryptographic functions used by the database for the purposes of 
/// creating/verifying documents and entries. Because documents and entries can be freely passed 
/// around without communicating with a recipient, all cryptography used assumes one-way 
/// communication.
///
/// Version 0 algorithms:
/// - There are none! Version 0 means no crytographic primitive is present
///
/// Version 1 algorithms:
/// - BLAKE2s for hashing
/// - Ed25519 for authenticated encryption
/// - Curve25519 for signing
/// - XChaCha20 for symmetric encryption
/// - Argon2 for deriving a secret key from a password
///
/// There is no "shared secret" negotiation. This is like PGP - generate a random symmetric 
/// key, encrypt it using Ed25519, and send that plus the ciphertext.
///
/// This library provides 7 primitives:
/// - Crypto: contains a cryptographically secure random-number generator, which is used as the 
/// source for all other primitives.
/// - Hash: cryptographically secure hash of provided data
/// - Identity: public key, usable for encrypting and for verifying signatures
/// - Signature: detached cryptographic signature that includes the Identity of the signer
/// - Key: private key, usable for decrypting and making signatures
/// - StreamKey: secret key, usable for encrypting/decrypting
/// - LockBox: data that has been encrypted using either a StreamKey or a Identity
/// - Lock: Data used to create a lockbox.
///
/// Notes:
///
/// - Hash uses BLAKE2s to produce 32 bytes of digest
/// - Signature
///     - Consists of a Ed25519 public key and Ed25519 signature
///     - encoded version is exactly the same
/// - LockBox
///     - Starts with a type identifier (one byte)
///     - Begins with necessary data to decrypt - nonce, public key or hash of StreamKey
///     - Has a Poly1305 authentication tag appended to the end

/// Indicates the Key paired with an Identity was used to sign a hash. The signature indicates the 
/// identity used.
#[derive(Debug,Clone,PartialEq,Eq,Hash)]
pub struct Signature (Vec<u8>);

/// Locks are used to put data in a lockbox. They *may* include the identity needed to open the 
/// lockbox.
#[derive(Debug,Clone,PartialEq,Eq,Hash)]
pub struct Lock (Vec<u8>);

/// Data that cannot be seen without the correct key. Signature is optionally embedded inside.
#[derive(Debug,Clone,PartialEq,Eq,Hash)]
pub struct LockBox(Vec<u8>);

/// Faster key for encrypting streams of data. The key is embedded in a normal lock, and can subsequently 
/// be used to generate additional locks. It is assumed when data locked by this key is received, 
/// the receiver will already know to use the stream key. 
///
/// Backend note: This uses the XChaCha20 stream cipher, where the 128-bit word used to make each
/// lock is the nonce used (upper 64 bits of nonce are always 0)
#[derive(Debug,Clone,PartialEq,Eq,Hash)]
pub struct StreamKey(Vec<u8>);

#[derive(Debug)]
pub enum CryptoError {
    WrongVersion,
    UnsupportedVersion,
    DecryptFailed,
    UnsupportedIdentity,
    UnsupportedHash,
    UnsupportedLock,
    UnsupportedKey,
    UnsupportedSignature,
    InvalidFormat,
    BadLength,
    BadKey,
    Io(io::Error),
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CryptoError::WrongVersion         => write!(f, "Incompatible versions for crypto primitives."),
            CryptoError::UnsupportedVersion   => write!(f, "Chosen crypto version not supported."),
            CryptoError::DecryptFailed        => write!(f, "Could not decrypt with key"),
            CryptoError::UnsupportedIdentity  => write!(f, "Version of Identity is not supported"),
            CryptoError::UnsupportedHash      => write!(f, "Version of Hash is not supported"),
            CryptoError::UnsupportedLock      => write!(f, "Version of Lock is not supported"),
            CryptoError::UnsupportedKey       => write!(f, "Version of Key is not supported"),
            CryptoError::UnsupportedSignature => write!(f, "Version of Signature is not supported"),
            CryptoError::InvalidFormat        => write!(f, "Format of crypto object is invalid"),
            CryptoError::BadKey               => write!(f, "Crypto key is weak or invalid"),
            CryptoError::BadLength            => write!(f, "Provided data length is invalid"),
            CryptoError::Io(ref err)          => err.fmt(f),
        }
    }
}

impl Error for CryptoError {
    fn description(&self) -> &str {
        match *self {
            CryptoError::WrongVersion         => "incompatible primitives",
            CryptoError::UnsupportedVersion   => "unsupported version",
            CryptoError::DecryptFailed        => "decryption failed",
            CryptoError::UnsupportedIdentity  => "identity version unsupported",
            CryptoError::UnsupportedHash      => "hash version unsupported",
            CryptoError::UnsupportedLock      => "lock version unsupported",
            CryptoError::UnsupportedKey       => "key version unsupported",
            CryptoError::UnsupportedSignature => "signature version unsupported",
            CryptoError::InvalidFormat        => "invalid object format",
            CryptoError::BadKey               => "weak or invalid key",
            CryptoError::BadLength            => "invalid data length",
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
    if unsafe { libsodium_sys::sodium_init() } >= 0 {
        Ok(())
    } else {
        Err(())
    }
}

/*
impl Crypto {

    // Make a new identity from a provided password. Identity will contain the public signing & 
    // encrypting keys, and will also contain the salt and hashing parameters
    pub fn new_identity_from_password(&mut self, password: &String) -> (Key, Identity) {
        self.rand += password.as_bytes()[0] as u64;
        (Key::new(0), Identity::new())
    }

    // Recover the private key for an identity that was made with the provided password. Returns 
    // nothing if the key couldn't be recovered.
    pub fn get_key_from_password(_password: &String, id: &Identity) -> Result<Key, CryptoError> {
        if !id.is_valid() { return Err(CryptoError::UnsupportedIdentity); }
        Ok(Key::new(0))
    }

    // Generate a lock with the identity visible
    pub fn new_lock(&mut self, id: &Identity) -> Result<(Lock, StreamKey), CryptoError> {
        if !id.is_valid() { return Err(CryptoError::UnsupportedIdentity); }
        self.rand += 1;
        Ok((Lock(vec![0]), StreamKey(vec![0])))
    }

    // Generate a lock with no visible identity
    pub fn new_lock_no_id(&mut self, id: &Identity) -> Result<(Lock, StreamKey), CryptoError> {
        if !id.is_valid() { return Err(CryptoError::UnsupportedIdentity); }
        self.rand += 1;
        Ok((Lock(vec![0]), StreamKey(vec![0])))
    }

    pub fn lock(&mut self, mut data: Vec<u8>, lock: &Lock) -> Result<LockBox, CryptoError> {
        if lock.0[0] != 0 { return Err(CryptoError::UnsupportedLock); }
        self.rand += 1;
        let mut bx = LockBox(lock.0.clone());
        bx.0.append(&mut data);
        Ok(bx)
    }

    pub fn lock_sign(&mut self, mut data: Vec<u8>, lock: &Lock, key: &Key) -> Result<LockBox, CryptoError> {
        if lock.0[0] != 0 { return Err(CryptoError::UnsupportedLock); }
        if !key.is_valid() { return Err(CryptoError::UnsupportedKey); }
        self.rand += 1;
        let mut bx = LockBox(lock.0.clone());
        bx.0.append(&mut data);
        Ok(bx)
    }
    
    pub fn locked_for_who(data: &LockBox) -> Result<Option<Identity>, CryptoError> {
        if data.0[0] != 0 { return Err(CryptoError::UnsupportedLock); }
        Ok(None)
    }

    pub fn unlock(mut data: LockBox, key: &Key) -> Result<(Vec<u8>, StreamKey, Option<Signature>), CryptoError> {
        if data.0[0] != 0 { return Err(CryptoError::UnsupportedLock); }
        if !key.is_valid() { return Err(CryptoError::UnsupportedKey); }
        data.0.remove(0);
        Ok((data.0, StreamKey(vec![0]), None))
    }

    pub fn unlock_stream(mut data: LockBox, stream_key: &StreamKey) -> Result<(Vec<u8>, Option<Signature>), CryptoError> {
        if data.0[0] != 0 { return Err(CryptoError::UnsupportedLock); }
        if stream_key.0[0] != 0 { return Err(CryptoError::UnsupportedKey); }
        data.0.remove(0);
        Ok((data.0, None))
    }

    pub fn sign(&mut self, hash: &Hash, key: &Key) -> Result<Signature, CryptoError> {
        if !hash.is_valid() { return Err(CryptoError::UnsupportedHash); }
        if !key.is_valid() { return Err(CryptoError::UnsupportedKey); }
        self.rand += 1;
        Ok(Signature(vec![0]))
    }

    pub fn signed_by_who(sign: &Signature) -> Result<Identity, CryptoError> {
        if sign.0[0] != 0 { return Err(CryptoError::UnsupportedSignature); }
        Ok(Identity::new())
    }

    pub fn verify_sign(hash: &Hash, sign: &Signature) -> Result<(Identity, bool), CryptoError> {
        if !hash.is_valid() { return Err(CryptoError::UnsupportedHash); }
        if sign.0[0] != 0 { return Err(CryptoError::UnsupportedSignature); }
        Ok((Identity::new(), true))
    }

    pub fn new_stream(&mut self, key: &StreamKey, _stream: (u64,u64)) -> Result<Lock, CryptoError> {
        if key.0[0] != 0 { return Err(CryptoError::UnsupportedKey); }
        self.rand += 1;
        Ok(Lock(vec![0]))

    }
}
*/
