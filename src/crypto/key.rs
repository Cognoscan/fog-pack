use std::io::{Write,Read};
use byteorder::{ReadBytesExt,WriteBytesExt};
use super::CryptoError;
use libsodium_sys;

const SEED_KEY_BYTES: usize = libsodium_sys::crypto_sign_SEEDBYTES as usize;
const SK_SIGN_KEY_BYTES: usize = libsodium_sys::crypto_sign_ed25519_SECRETKEYBYTES as usize;
const SK_CRYPT_KEY_BYTES: usize = libsodium_sys::crypto_scalarmult_curve25519_BYTES as usize;
const PK_SIGN_KEY_BYTES: usize = libsodium_sys::crypto_sign_ed25519_PUBLICKEYBYTES as usize;
const PK_CRYPT_KEY_BYTES: usize = libsodium_sys::crypto_scalarmult_curve25519_BYTES as usize;


/// - Key
///     - Consists of a Ed25519 seed and private/public keypair, as well as the Curve25519 private key
///     - When encoded, it is just the seed, from which the keypair and Curve25519 key can be 
///     derived

/// Keys are the secret data needed to act as a particular Identity.
#[derive(Clone)]
pub struct Key {
    version: u8,
    signing: [u8; SK_SIGN_KEY_BYTES],
    decrypting: [u8; SK_CRYPT_KEY_BYTES],
}

impl Key {

    pub fn new_pair(version: u8) -> (Key, Identity) {
        let key = Key::new(version);
        let identity = Identity::new();
        (key, identity)
    }

    pub fn new(version: u8) -> Key {
        Key {
            version,
            signing: [0; SK_SIGN_KEY_BYTES],
            decrypting: [0; SK_CRYPT_KEY_BYTES],
        }
    }

    pub fn from_seed(version: u8, seed: &[u8; SEED_KEY_BYTES]) -> Result<(Key, Identity), CryptoError> {
        let mut key = Key { version, signing: [0; SK_SIGN_KEY_BYTES], decrypting: [0; SK_CRYPT_KEY_BYTES] };
        let mut id = Identity::new();
        sign_seed_keypair(&mut id.signing, &mut key.signing, seed)?;
        Ok((key,id))
    }

    pub fn copy_pk(&self, pk: &mut [u8; PK_SIGN_KEY_BYTES]) -> Result<(), CryptoError> {
        ed25519_sk_to_pk(pk, &self.signing)
    }

    pub fn is_valid(&self) -> bool {
        if self.version == 0 { return true; }
        if self.version == 1 { return true; }
        false
    }

    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), CryptoError> {
        wr.write_u8(self.version).map_err(CryptoError::Io)?;
        let mut seed = [0; SEED_KEY_BYTES];
        ed25519_sk_to_seed(&mut seed, &self.signing)?;
        wr.write_all(&seed).map_err(CryptoError::Io)?;
        Ok(())
    }

    pub fn read<R: Read>(rd: &mut R) -> Result<Identity, CryptoError> {
        let mut id = Identity::new();
        id.version = rd.read_u8().map_err(CryptoError::Io)?;
        match id.version {
            0 => Ok(id),
            1 => {
                rd.read_exact(&mut id.signing).map_err(CryptoError::Io)?;
                ed25519_pk_to_curve25519_pk(&id.signing, &mut id.encrypting)?;
                Ok(id)
            },
            _ => Err(CryptoError::UnsupportedVersion),
        }
    }
}

/// Public identity that can be shared with others. An identity can be used to create locks and 
/// verify signatures
///     - Consists of a Ed25519 public key and the Curve25519 public key.
///     - When encoded, it is just the Ed25519 public key
#[derive(Debug,Clone,PartialEq,Eq,Hash)]
pub struct Identity {
    version: u8,                         // Crypto version
    signing: [u8; PK_SIGN_KEY_BYTES],       // Version 1: Ed25519 public key
    encrypting: [u8; PK_CRYPT_KEY_BYTES], // Version 1: Curve 25519 public key
}

/// Consists of a Ed25519 public key and corresponding Curve25519 public key.
/// When encoded, it is just the Ed25519 public key
impl Identity {

    pub fn new() -> Identity {
        Identity {
            version: 0,
            signing: [0; PK_SIGN_KEY_BYTES],
            encrypting: [0; PK_CRYPT_KEY_BYTES],
        }
    }

    pub fn from_key(key: super::Key) -> Identity {
        unimplemented!();
    }

    pub fn is_valid(&self) -> bool {
        if self.version != 0 { return false; }
        true
    }

    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), CryptoError> {
        wr.write_u8(self.version).map_err(CryptoError::Io)?;
        wr.write_all(&self.signing).map_err(CryptoError::Io)?;
        Ok(())
    }

    pub fn read<R: Read>(rd: &mut R) -> Result<Identity, CryptoError> {
        let mut id = Identity::new();
        id.version = rd.read_u8().map_err(CryptoError::Io)?;
        match id.version {
            0 => Ok(id),
            1 => {
                rd.read_exact(&mut id.signing).map_err(CryptoError::Io)?;
                ed25519_pk_to_curve25519_pk(&id.signing, &mut id.encrypting)?;
                Ok(id)
            },
            _ => Err(CryptoError::UnsupportedVersion),
        }
    }
}

fn sign_seed_keypair(
    pk: &mut[u8; PK_SIGN_KEY_BYTES],
    sk: &mut[u8; SK_SIGN_KEY_BYTES],
    seed: &[u8; SEED_KEY_BYTES]
) -> Result<(), CryptoError>
{
    if unsafe {
        libsodium_sys::crypto_sign_seed_keypair(pk.as_mut_ptr(),sk.as_mut_ptr(), seed.as_ptr())
    } >= 0 {
        Ok(())
    }
    else {
        Err(CryptoError::BadKey)
    }
}

fn ed25519_sk_to_pk(
    pk: &mut[u8; PK_SIGN_KEY_BYTES],
    sk: &[u8; SK_SIGN_KEY_BYTES]
) -> Result<(), CryptoError>
{
    if unsafe {
        libsodium_sys::crypto_sign_ed25519_sk_to_pk(pk.as_mut_ptr(),sk.as_ptr())
    } >= 0 {
        Ok(())
    }
    else {
        Err(CryptoError::BadKey)
    }
}

fn ed25519_sk_to_seed(
    seed: &mut[u8; SEED_KEY_BYTES],
    ed: &[u8; SK_SIGN_KEY_BYTES]
) -> Result<(), CryptoError>
{
    if unsafe {
        libsodium_sys::crypto_sign_ed25519_sk_to_seed(seed.as_mut_ptr(),ed.as_ptr())
    } >= 0 {
        Ok(())
    }
    else {
        Err(CryptoError::BadKey)
    }
}

fn ed25519_sk_to_curve25519(
    curve: &mut [u8; SK_CRYPT_KEY_BYTES],
    ed: &[u8; SK_SIGN_KEY_BYTES]
) -> Result<(), CryptoError>
{
    if unsafe {
        libsodium_sys::crypto_sign_ed25519_sk_to_curve25519(curve.as_mut_ptr(),ed.as_ptr())
    } >= 0 {
        Ok(())
    }
    else {
        Err(CryptoError::BadKey)
    }
}

fn ed25519_pk_to_curve25519_pk(
    ed: &[u8; PK_SIGN_KEY_BYTES], 
    curve: &mut [u8; PK_CRYPT_KEY_BYTES]
) -> Result<(), CryptoError>
{
    if unsafe {
        libsodium_sys::crypto_sign_ed25519_pk_to_curve25519(curve.as_mut_ptr(),ed.as_ptr())
    } >= 0 {
        Ok(())
    }
    else {
        Err(CryptoError::BadKey)
    }
}
