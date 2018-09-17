
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

/// Provides interface for all cryptographic operations
pub struct Crypto {
    version: u32,
    rand: Vec<u8>
}

pub enum Error {
    NotEnough
    WrongVersion,
}

/// Faster key for encrypting streams of data. The key is embedded in a normal lock, and can subsequently 
/// be used to generate additional locks. It is assumed when data locked by this key is received, 
/// the receiver will already know to use the stream key. 
///
/// Backend note: This uses the XChaCha20 stream cipher, where the 128-bit word used to make each
/// lock is the nonce used (upper 64 bits of nonce are always 0)
struct StreamKey(Vec<u8>);

impl Crypto {
    fn init(version: u32) -> Crypto {
        Crypto { version: u32, rand: vec![] }
    }

    // Hash the provided data. Version must be provided
    fn hash(version: u32, data: Vec<u8>) -> Hash {
        panic!();
    }

    // Make a new identity, i.e. a random public & private key pair valid for signing & encrypting
    fn new_identity() -> (Key, Identity) {
        panic!();
    }

    // Make a new identity from a provided password. Identity will contain the public signing & 
    // encrypting keys, and will also contain the salt and hashing parameters
    fn new_identity_from_password(password: String) -> Result<(Key, Identity), Error> {
        panic!();
    }

    // Recover the private key for an identity that was made with the provided password. Returns 
    // nothing if the key couldn't be recovered.
    fn get_key_from_password(password: String, identity: Identity) -> Option<Key> {
        None
    }

    // Generate a lock with the identity visible
    fn new_lock(id: Identity) -> (Lock, StreamKey) {
        panic!();
    }

    // Generate a lock with no visible identity
    fn new_lock_no_id(id: Identity) -> (Lock, StreamKey) {
        panic!();
    }

    fn lock(data: Vec<u8>, lock: Lock) -> LockBox {
        panic!();
    }

    fn lock(data: Vec<u8>, lock: Lock, key: Key) -> LockBox {
        panic!();
    }
    
    fn locked_for_who(data: LockBox) -> Option<Identity> {
        None
    }

    fn unlock(data: LockBox, key: Key) -> (Option<data>, Option<Signature>) {
        None
    }

    fn sign(hash: Hash, key: Key) -> Signature {
        panic!();
    }

    fn signed_by_who(sign: Signature) -> Identity {
        panic!();
    }

    fn verify_sign(hash: Hash, sign: Signature) -> (Identity, bool) {
        panic!();
    }

    fn new_stream(key: StreamKey, stream: (u64,u64)) -> Lock {
        panic!();
    }
}
