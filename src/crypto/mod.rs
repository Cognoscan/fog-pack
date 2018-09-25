use std::fmt;
use std::io::Write;
use std::error::Error;

/// How to Forward Secrecy
/// ----------------------
///
/// Create a temporary identity to use for a session, then make a relationship called "temp" that 
/// your main identity has signed and give it out to everyone. When your receive data you care 
/// about that has been locked using the temporary identity, revoke the relationship. Hold onto 
/// these temporary identities for a while in case anyone else uses it before seeing your 
/// revocation, but ditch it eventually. Maintain a set of temporary identities
///
/// let (main_key, main_id) = crypto.new_identity();
/// let (temp_key, temp_id) = crypto.new_identity();
/// let relation = Relation(temp_id, "temp");
/// let sign = crypto.sign(relation.encode(), main_key)
/// Add_Relation(relation.encode(), sign)
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
/// There is no "shared secret" negotiation. This is more like PGP - generate a random symmetric 
/// key, encrypt it using Ed25519, and send that plus the ciphertext.
///
/// This library provides 7 primitives:
/// - Crypto: contains a cryptographically secure random-number generator, which is used as the 
/// source for all other primitives.
/// - Hash: a hash of a defined stream of data, constructed with BLAKE2s
/// - 

/// Crytographically secure hash of data. Can be signed by a Key. It is impractical to generate an 
/// identical hash from different data.
pub struct Hash (Vec<u8>);

/// Public identity that can be shared with others. An identity can be used to create locks and 
/// verify signatures
pub struct Identity (Vec<u8>);

/// Indicates the Key paired with an Identity was used to sign a hash. The signature indicates the 
/// identity used.
pub struct Signature (Vec<u8>);

/// Locks are used to put data in a lockbox. They *may* include the identity needed to open the 
/// lockbox.
pub struct Lock (Vec<u8>);

/// Keys are the secret data needed to act as a particular Identity.
pub struct Key (Vec<u8>);

/// Data that cannot be seen without the correct key. Signature is optionally embedded inside.
pub struct LockBox(Vec<u8>);

/// Faster key for encrypting streams of data. The key is embedded in a normal lock, and can subsequently 
/// be used to generate additional locks. It is assumed when data locked by this key is received, 
/// the receiver will already know to use the stream key. 
///
/// Backend note: This uses the XChaCha20 stream cipher, where the 128-bit word used to make each
/// lock is the nonce used (upper 64 bits of nonce are always 0)
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
    pub fn hash(version: u32, data: &Vec<u8>) -> Result<Hash, CryptoError> {
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
        let key = Key(vec![0]);
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

    pub fn lock(data: Vec<u8>, lock: &Lock) -> Result<LockBox, CryptoError> {
        if lock.0[0] != 0 { return Err(CryptoError::UnsupportedLock); }
        let mut bx = LockBox(lock.0.clone());
        bx.0.append(&mut data);
        Ok(bx)
    }

    pub fn lock_sign(data: Vec<u8>, lock: &Lock, key: &Key) -> Result<LockBox, CryptoError> {
        if lock.0[0] != 0 { return Err(CryptoError::UnsupportedLock); }
        let mut bx = LockBox(lock.0.clone());
        bx.0.append(&mut data);
        Ok(bx)
    }
    
    pub fn locked_for_who(data: &LockBox) -> Result<Option<Identity>, CryptoError> {
        if data.0[0] != 0 { return Err(CryptoError::UnsupportedLock); }
        Ok(None)
    }

    pub fn unlock(data: LockBox, key: &Key) -> Result<(Vec<u8>, Option<Signature>), CryptoError> {
        if data.0[0] != 0 { return Err(CryptoError::UnsupportedLock); }
        if key.0[0] != 0 { return Err(CryptoError::UnsupportedKey); }
        data.0.remove(0);
        Ok((data.0, None))
    }

    pub fn sign(hash: &Hash, key: &Key) -> Result<Signature, CryptoError> {
        if hash.0[0] != 0 { return Err(CryptoError::UnsupportedHash); }
        if key.0[0] != 0 { return Err(CryptoError::UnsupportedKey); }
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

    pub fn new_stream(key: &StreamKey, stream: (u64,u64)) -> Result<Lock, CryptoError> {
        if key.0[0] != 0 { return Err(CryptoError::UnsupportedKey); }
        Ok(Lock(vec![0]))

    }
}
