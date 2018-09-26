use std::fmt;
use std::error::Error;

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
/// - Identity
///     - Consists of a Ed25519 public key and the Curve25519 public key.
///     - When encoded, it is just the Ed25519 public key
/// - Key
///     - Consists of a Ed25519 seed and private/public keypair, as well as the Curve25519 private key
///     - When encoded, it is just the seed, from which the keypair and Curve25519 key can be 
///     derived
/// - Signature
///     - Consists of a Ed25519 public key and Ed25519 signature
///     - encoded version is exactly the same
/// - StreamKey: A secret XChaCha20 key
/// - Lock
///     - Starts with a type identifier (one byte)
///         - 0 is made with an Identity and LockBox will include the public key
///         - 1 is made with an Ideneity and LockBox will not include public key
///         - 2 is made with StreamKey and Lockbox will include Hash of StreamKey
///         - 3 is made with StreamKey and Lockbox will not include StreamKey hash
///     - When made with a Key, consists of a Curve25519 public key, XChaCha20 key, and random nonce
///     - When made with a StreamKey, consists of the XChaCha20 key and chosen nonce.
/// - LockBox
///     - Starts with a type identifier (one byte)
///     - Begins with necessary data to decrypt - nonce, public key or hash of StreamKey
///     - Has a Poly1305 authentication tag appended to the end

/// Crytographically secure hash of data. Can be signed by a Key. It is impractical to generate an 
/// identical hash from different data.
#[derive(Debug,PartialEq,Hash)]
pub struct Hash (Vec<u8>);

/// Public identity that can be shared with others. An identity can be used to create locks and 
/// verify signatures
#[derive(Debug,PartialEq,Hash)]
pub struct Identity (Vec<u8>);

/// Indicates the Key paired with an Identity was used to sign a hash. The signature indicates the 
/// identity used.
#[derive(Debug,PartialEq,Hash)]
pub struct Signature (Vec<u8>);

/// Locks are used to put data in a lockbox. They *may* include the identity needed to open the 
/// lockbox.
#[derive(Debug,PartialEq,Hash)]
pub struct Lock (Vec<u8>);

/// Keys are the secret data needed to act as a particular Identity.
#[derive(Debug,PartialEq,Hash)]
pub struct Key (Vec<u8>);

/// Data that cannot be seen without the correct key. Signature is optionally embedded inside.
pub struct LockBox(Vec<u8>);

/// Faster key for encrypting streams of data. The key is embedded in a normal lock, and can subsequently 
/// be used to generate additional locks. It is assumed when data locked by this key is received, 
/// the receiver will already know to use the stream key. 
///
/// Backend note: This uses the XChaCha20 stream cipher, where the 128-bit word used to make each
/// lock is the nonce used (upper 64 bits of nonce are always 0)
#[derive(Debug,PartialEq,Hash)]
pub struct StreamKey(Vec<u8>);

/// Provides interface for all cryptographic operations
pub struct Crypto {
    version: u32,
    rand: u64
}

#[derive(Debug)]
pub enum CryptoError {
    WrongVersion,
    UnsupportedVersion,
    DecryptFailed,
    UnsupportedIdentity,
    UnsupportedHash,
    UnsupportedLock,
    UnsupportedKey,
    UnsupportedSignature
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
        }
    }
}

impl Crypto {
    pub fn init(version: u32) -> Result<Crypto, CryptoError> {
        match version {
            0 => Ok(Crypto { version: 0, rand: 0 }),
            _ => Err(CryptoError::UnsupportedVersion),
        }
    }

    // Hash the provided data. Version must be provided
    pub fn hash(version: u32, _data: &Vec<u8>) -> Result<Hash, CryptoError> {
        match version {
            0 => Ok(Hash ( vec![0] )),
            _ => Err(CryptoError::UnsupportedVersion),
        }
    }

    // Make a new identity, i.e. a random public & private key pair valid for signing & encrypting
    pub fn new_identity(&mut self) -> (Key, Identity) {
        self.rand += self.version as u64;
        (Key(vec![0]), Identity(vec![0]))
    }

    // Make a new identity from a provided password. Identity will contain the public signing & 
    // encrypting keys, and will also contain the salt and hashing parameters
    pub fn new_identity_from_password(&mut self, password: &String) -> (Key, Identity) {
        self.rand += password.as_bytes()[0] as u64;
        (Key(vec![0]), Identity(vec![0]))
    }

    // Recover the private key for an identity that was made with the provided password. Returns 
    // nothing if the key couldn't be recovered.
    pub fn get_key_from_password(_password: &String, identity: &Identity) -> Result<Key, CryptoError> {
        if identity.0[0] != 0 { return Err(CryptoError::UnsupportedIdentity); }
        Ok(Key(vec![0]))
    }

    // Generate a lock with the identity visible
    pub fn new_lock(&mut self, id: &Identity) -> Result<(Lock, StreamKey), CryptoError> {
        if id.0[0] != 0 { return Err(CryptoError::UnsupportedIdentity); }
        self.rand += 1;
        Ok((Lock(vec![0]), StreamKey(vec![0])))
    }

    // Generate a lock with no visible identity
    pub fn new_lock_no_id(&mut self, id: &Identity) -> Result<(Lock, StreamKey), CryptoError> {
        if id.0[0] != 0 { return Err(CryptoError::UnsupportedIdentity); }
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
        if key.0[0] != 0 { return Err(CryptoError::UnsupportedKey); }
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
        if key.0[0] != 0 { return Err(CryptoError::UnsupportedKey); }
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
        if hash.0[0] != 0 { return Err(CryptoError::UnsupportedHash); }
        if key.0[0] != 0 { return Err(CryptoError::UnsupportedKey); }
        self.rand += 1;
        Ok(Signature(vec![0]))
    }

    pub fn signed_by_who(sign: &Signature) -> Result<Identity, CryptoError> {
        if sign.0[0] != 0 { return Err(CryptoError::UnsupportedSignature); }
        Ok(Identity(vec![0]))
    }

    pub fn verify_sign(hash: &Hash, sign: &Signature) -> Result<(Identity, bool), CryptoError> {
        if hash.0[0] != 0 { return Err(CryptoError::UnsupportedHash); }
        if sign.0[0] != 0 { return Err(CryptoError::UnsupportedSignature); }
        Ok((Identity(vec![0]), true))
    }

    pub fn new_stream(&mut self, key: &StreamKey, _stream: (u64,u64)) -> Result<Lock, CryptoError> {
        if key.0[0] != 0 { return Err(CryptoError::UnsupportedKey); }
        self.rand += 1;
        Ok(Lock(vec![0]))

    }
}
