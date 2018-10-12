use std::ops::Drop;
use std::fmt;
use std::io::{Write,Read};
use byteorder::{ReadBytesExt,WriteBytesExt};
use super::CryptoError;
use libsodium_sys;

const SEED_KEY_BYTES: usize = libsodium_sys::crypto_sign_SEEDBYTES as usize;
const SK_SIGN_KEY_BYTES: usize = libsodium_sys::crypto_sign_ed25519_SECRETKEYBYTES as usize;
const SK_CRYPT_KEY_BYTES: usize = libsodium_sys::crypto_scalarmult_curve25519_BYTES as usize;
const PK_SIGN_KEY_BYTES: usize = libsodium_sys::crypto_sign_ed25519_PUBLICKEYBYTES as usize;
const PK_CRYPT_KEY_BYTES: usize = libsodium_sys::crypto_scalarmult_curve25519_BYTES as usize;

// Secret Structs
#[derive(Clone)]
struct Seed([u8; SEED_KEY_BYTES]);
#[derive(Clone)]
struct SecretSignKey([u8; SK_SIGN_KEY_BYTES]);
#[derive(Clone)]
struct SecretCryptKey([u8; SK_CRYPT_KEY_BYTES]);

// Public Structs
#[derive(Clone,PartialEq,Eq,Hash)]
struct PublicSignKey([u8; PK_SIGN_KEY_BYTES]);
#[derive(Clone,PartialEq,Eq,Hash)]
struct PublicCryptKey([u8; PK_CRYPT_KEY_BYTES]);

impl Drop for Seed {
    fn drop(&mut self) {
        memzero(&mut self.0);
    }
}
impl fmt::Debug for Seed {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}(****)", stringify!(Seed))
    }
}

impl Drop for SecretSignKey {
    fn drop(&mut self) {
        memzero(&mut self.0);
    }
}
impl fmt::Debug for SecretSignKey {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}(****)", stringify!(SecretSignKey))
    }
}

impl Drop for SecretCryptKey {
    fn drop(&mut self) {
        memzero(&mut self.0);
    }
}
impl fmt::Debug for SecretCryptKey {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}(****)", stringify!(SecretCryptKey))
    }
}

/// Keys are the secret data needed to act as a particular Identity.
/// - Consists of a Ed25519 private/public keypair, as well as the Curve25519 private key
/// - When encoded, it is just the "seed", from which the keypair and Curve25519 key can be derived
/// - The seed is actually the Ed25519 private key
#[derive(Clone)]
pub struct Key {
    version: u8,
    signing: SecretSignKey,
    decrypting: SecretCryptKey,
}

impl Key {

    fn blank() -> Key {
        Key {
            version: 0,
            signing: SecretSignKey([0; SK_SIGN_KEY_BYTES]),
            decrypting: SecretCryptKey([0; SK_CRYPT_KEY_BYTES]),
        }
    }

    fn complete(&mut self) {
        ed25519_sk_to_curve25519(&mut self.decrypting, &self.signing)
    }

    pub fn new_pair() -> Result<(Key, Identity), CryptoError> {
        let mut key = Key::blank();
        let mut id = Identity::blank();
        key.version = 1;
        id.version = 1;
        sign_keypair(&mut id.signing, &mut key.signing);
        key.complete();
        id.complete()?;
        Ok((key, id))
    }

    pub fn get_identity(&self) -> Result<Identity, CryptoError> {
        let mut id = Identity::blank();
        id.version = 1;
        ed25519_sk_to_pk(&mut id.signing, &self.signing);
        id.complete()?;
        Ok(id)
    }

    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), CryptoError> {
        wr.write_u8(self.version).map_err(CryptoError::Io)?;
        let mut seed = Seed([0; SEED_KEY_BYTES]);
        ed25519_sk_to_seed(&mut seed, &self.signing);
        wr.write_all(&seed.0).map_err(CryptoError::Io)?;
        Ok(())
    }

    pub fn read<R: Read>(rd: &mut R) -> Result<(Key,Identity), CryptoError> {
        let mut key = Key::blank();
        let mut id = Identity::blank();
        let mut seed = Seed([0; SEED_KEY_BYTES]);
        key.version = rd.read_u8().map_err(CryptoError::Io)?;
        id.version = key.version;
        match key.version {
            1 => {
                rd.read_exact(&mut seed.0).map_err(CryptoError::Io)?;
                sign_seed_keypair(&mut id.signing, &mut key.signing, &seed);
                key.complete();
                id.complete()?;
                Ok((key,id))
            },
            _ => Err(CryptoError::UnsupportedVersion),
        }
    }
}

/// Public identity that can be shared with others. An identity can be used to create locks and 
/// verify signatures
///     - Consists of a Ed25519 public key and the Curve25519 public key.
///     - When encoded, it is just the Ed25519 public key
#[derive(Clone,PartialEq,Eq,Hash)]
pub struct Identity {
    version: u8,                         // Crypto version
    signing: PublicSignKey,       // Version 1: Ed25519 public key
    encrypting: PublicCryptKey, // Version 1: Curve 25519 public key
}

/// Consists of a Ed25519 public key and corresponding Curve25519 public key.
/// When encoded, it is just the Ed25519 public key
impl Identity {

    fn blank() -> Identity {
        Identity {
            version: 0,
            signing: PublicSignKey([0; PK_SIGN_KEY_BYTES]),
            encrypting: PublicCryptKey([0; PK_CRYPT_KEY_BYTES]),
        }
    }

    fn complete(&mut self) -> Result<(), CryptoError> {
        ed25519_pk_to_curve25519_pk(&mut self.encrypting, &self.signing)
    }

    pub fn encrypt

    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), CryptoError> {
        wr.write_u8(self.version).map_err(CryptoError::Io)?;
        wr.write_all(&self.signing.0).map_err(CryptoError::Io)?;
        Ok(())
    }

    pub fn read<R: Read>(rd: &mut R) -> Result<Identity, CryptoError> {
        let mut id = Identity::blank();
        id.version = rd.read_u8().map_err(CryptoError::Io)?;
        match id.version {
            1 => {
                rd.read_exact(&mut id.signing.0).map_err(CryptoError::Io)?;
                id.complete()?;
                Ok(id)
            },
            _ => Err(CryptoError::UnsupportedVersion),
        }
    }
}



fn sign_keypair( pk: &mut PublicSignKey, sk: &mut SecretSignKey) {
    unsafe { libsodium_sys::crypto_sign_keypair(pk.0.as_mut_ptr(),sk.0.as_mut_ptr()) };
}

fn sign_seed_keypair( pk: &mut PublicSignKey, sk: &mut SecretSignKey, seed: &Seed) {
    unsafe { libsodium_sys::crypto_sign_seed_keypair(pk.0.as_mut_ptr(),sk.0.as_mut_ptr(), seed.0.as_ptr()) };
}

fn ed25519_sk_to_pk( pk: &mut PublicSignKey, sk: &SecretSignKey) {
    unsafe { libsodium_sys::crypto_sign_ed25519_sk_to_pk(pk.0.as_mut_ptr(),sk.0.as_ptr()) };
}

fn ed25519_sk_to_seed( seed: &mut Seed, ed: &SecretSignKey) {
    unsafe { libsodium_sys::crypto_sign_ed25519_sk_to_seed(seed.0.as_mut_ptr(),ed.0.as_ptr()) };
}

fn ed25519_sk_to_curve25519( curve: &mut SecretCryptKey, ed: &SecretSignKey) {
    unsafe { libsodium_sys::crypto_sign_ed25519_sk_to_curve25519(curve.0.as_mut_ptr(),ed.0.as_ptr()) };
}

fn ed25519_pk_to_curve25519_pk(
    curve: &mut PublicCryptKey,
    ed: &PublicSignKey,
) -> Result<(), CryptoError>
{
    // Can actually fail with a bad key, so check it
    if unsafe {
        libsodium_sys::crypto_sign_ed25519_pk_to_curve25519(curve.0.as_mut_ptr(),ed.0.as_ptr())
    } >= 0 {
        Ok(())
    }
    else {
        Err(CryptoError::BadKey)
    }
}

fn memzero(x: &mut [u8]) {
    unsafe { libsodium_sys::sodium_memzero(x.as_mut_ptr() as *mut _, x.len()); }
}
