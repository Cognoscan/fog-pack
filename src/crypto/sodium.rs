use super::CryptoError;
use std::ops::Drop;
use std::fmt;
use std::ffi::CString;
use libc::c_ulonglong;
use libsodium_sys;

const SECRET_KEY_BYTES: usize = libsodium_sys::crypto_aead_xchacha20poly1305_ietf_KEYBYTES as usize;
const NONCE_BYTES: usize = libsodium_sys::crypto_aead_xchacha20poly1305_ietf_NPUBBYTES as usize;
const TAG_BYTES: usize = libsodium_sys::crypto_aead_xchacha20poly1305_ietf_ABYTES as usize;
const SEED_KEY_BYTES: usize = libsodium_sys::crypto_sign_SEEDBYTES as usize;
const SK_SIGN_KEY_BYTES: usize = libsodium_sys::crypto_sign_ed25519_SECRETKEYBYTES as usize;
const SK_CRYPT_KEY_BYTES: usize = libsodium_sys::crypto_scalarmult_curve25519_BYTES as usize;
const PK_SIGN_KEY_BYTES: usize = libsodium_sys::crypto_sign_ed25519_PUBLICKEYBYTES as usize;
const PK_CRYPT_KEY_BYTES: usize = libsodium_sys::crypto_scalarmult_curve25519_BYTES as usize;

// Secret Structs
#[derive(Clone,Default)]
pub struct Seed(pub [u8; SEED_KEY_BYTES]);
#[derive(Clone)]
pub struct SecretSignKey([u8; SK_SIGN_KEY_BYTES]);
#[derive(Clone,Default)]
pub struct SecretCryptKey([u8; SK_CRYPT_KEY_BYTES]);
#[derive(Clone,Default)]
pub struct SecretKey(pub [u8; SECRET_KEY_BYTES]);

// Public Structs
#[derive(Clone,PartialEq,Eq,Hash,Default)]
pub struct PublicSignKey(pub [u8; PK_SIGN_KEY_BYTES]);
#[derive(Clone,PartialEq,Eq,Hash,Default)]
pub struct PublicCryptKey([u8; PK_CRYPT_KEY_BYTES]);
#[derive(Clone,PartialEq,Eq,Hash,Default)]
pub struct Nonce(pub [u8; NONCE_BYTES]);
#[derive(Clone,PartialEq,Eq,Hash,Default)]
pub struct Tag(pub [u8; TAG_BYTES]);

impl Tag {
    pub fn len() -> usize {
        TAG_BYTES
    }
}

// Seed
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

// SecretSignKey
impl Default for SecretSignKey {
    fn default() -> SecretSignKey {
        SecretSignKey([0; SK_SIGN_KEY_BYTES])
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

// SecretCryptKey
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

// SecretKey
impl Drop for SecretKey {
    fn drop(&mut self) {
        memzero(&mut self.0);
    }
}
impl fmt::Debug for SecretKey {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}(****)", stringify!(SecretKey))
    }
}

pub fn aead_keygen(key: &mut SecretKey) {
    unsafe { libsodium_sys::crypto_aead_xchacha20poly1305_ietf_keygen(key.0.as_mut_ptr()) };
}

// Does in-place encryption of message and returns HMAC Tag
pub fn aead_encrypt(message: &mut [u8], ad: &[u8], n: &Nonce, k: &SecretKey) -> Tag {
    // tag will store the message authentication tag
    let mut tag = Tag([0; TAG_BYTES]);
    // Function call sets mac_len to TAG_BYTES and does nothing else, so we'll do nothing with it
    let mut mac_len = TAG_BYTES as c_ulonglong;
    unsafe {
        libsodium_sys::crypto_aead_xchacha20poly1305_ietf_encrypt_detached(
            message.as_mut_ptr(),
            tag.0.as_mut_ptr(),
            &mut mac_len,
            message.as_ptr(),
            message.len() as c_ulonglong,
            ad.as_ptr(),
            ad.len() as c_ulonglong, 
            0 as *mut _,
            n.0.as_ptr(),
            k.0.as_ptr()
        );
    }
    tag
}

// Shouldn't fail as long as the input parameters are valid
pub fn derive_uuid(k: &SecretKey, uuid: &mut [u8; 32]) {
    unsafe {
        let ctx = CString::from_vec_unchecked(b"condense".to_vec());
        libsodium_sys::crypto_kdf_derive_from_key(uuid.as_mut_ptr(), 32, 1, ctx.as_ptr(), k.0.as_ptr());
    };
}

pub fn blake2b( hash: &mut [u8; 64], data: &[u8] ) {
    // The below will only fail if we set up this function wrong.
    unsafe { 
        libsodium_sys::crypto_generichash_blake2b(
            hash.as_mut_ptr(), 64, 
            data.as_ptr(), data.len() as u64,
            ::std::ptr::null(), 0);
    }
}

pub fn sign_keypair( pk: &mut PublicSignKey, sk: &mut SecretSignKey) {
    unsafe { libsodium_sys::crypto_sign_keypair(pk.0.as_mut_ptr(),sk.0.as_mut_ptr()) };
}

pub fn sign_seed_keypair( pk: &mut PublicSignKey, sk: &mut SecretSignKey, seed: &Seed) {
    unsafe { libsodium_sys::crypto_sign_seed_keypair(pk.0.as_mut_ptr(),sk.0.as_mut_ptr(), seed.0.as_ptr()) };
}

pub fn ed25519_sk_to_pk( pk: &mut PublicSignKey, sk: &SecretSignKey) {
    unsafe { libsodium_sys::crypto_sign_ed25519_sk_to_pk(pk.0.as_mut_ptr(),sk.0.as_ptr()) };
}

pub fn ed25519_sk_to_seed( seed: &mut Seed, ed: &SecretSignKey) {
    unsafe { libsodium_sys::crypto_sign_ed25519_sk_to_seed(seed.0.as_mut_ptr(),ed.0.as_ptr()) };
}

pub fn ed25519_sk_to_curve25519( curve: &mut SecretCryptKey, ed: &SecretSignKey) {
    unsafe { libsodium_sys::crypto_sign_ed25519_sk_to_curve25519(curve.0.as_mut_ptr(),ed.0.as_ptr()) };
}

pub fn ed25519_pk_to_curve25519_pk(
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

pub fn memzero(x: &mut [u8]) {
    unsafe { libsodium_sys::sodium_memzero(x.as_mut_ptr() as *mut _, x.len()); }
}

pub fn randombytes(x: &mut [u8]) {
    unsafe { libsodium_sys::randombytes_buf(x.as_mut_ptr() as *mut _, x.len()); }
}
