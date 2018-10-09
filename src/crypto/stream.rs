use std::ops::Drop;
use std::fmt;
use std::io::{Write,Read};
use byteorder::{ReadBytesExt,WriteBytesExt};
use super::CryptoError;
use libsodium_sys;

const SECRET_KEY_BYTES: usize = libsodium_sys::crypto_aead_xchacha20poly1305_ietf_KEYBYTES as usize;
const NONCE_BYTES: usize = libsodium_sys::crypto_aead_xchacha20poly1305_ietf_NPUBBYTES as usize;
const TAG_BYTES: usize = libsodium_sys::crypto_aead_xchacha20poly1305_ietf_ABYTES as usize;

#[derive(Clone)]
struct SecretKey([u8; SECRET_KEY_BYTES]);

#[derive(Clone,PartialEq,Eq,Hash)]
struct Nonce([u8; NONCE_BYTES]);
#[derive(Clone,PartialEq,Eq,Hash)]
struct Tag([u8; TAG_BYTES]);

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

#[derive(Clone)]
pub struct StreamKey {
    version: u8,
    key: SecretKey,
}

impl StreamKey {
    
    fn blank() -> StreamKey {
        StreamKey {
            version: 0,
            key: SecretKey([0; SECRET_KEY_BYTES]),
        }
    }

    pub fn new() -> StreamKey {
}

fn aead_keygen(key: &mut SecretKey) {
    unsafe { libsodium_sys::crypto_aead_xchacha20poly1305_ietf_keygen(key.0.as_mut_ptr()) };
}

fn aead_encrypt(cipher: &mut [u8], message: &[u8], additional: Option<&[u8]>, n: &Nonce, k: &SecretKey) {
    let (additional_data, additional_len) = additional
        .map(|ad| (ad.as_ptr(), ad.len() as c_ulonglong))
        .unwrap_or((0 as *const _, 0));
    let cipher_len = cipher.len() as c_ulonglong;
    unsafe {
        crypto_aead_xchacha20poly1305_ietf_encrypt(
            cipher.as_mut_ptr(),
            &mut cipher_len,
            m.as_ptr()

fn memzero(x: &mut [u8]) {
    unsafe { libsodium_sys::sodium_memzero(x.as_mut_ptr() as *mut _, x.len()); }
}
