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
#[derive(Clone,PartialEq,Eq,Hash)]
enum LockType {
    PublicRecipient,
    PrivateRecipient,
    StreamType,
}
impl LockType {
    fn to_u8(&self) -> u8 {
        match *self {
            ExtType::PublicRecipient  => 1,
            ExtType::PrivateRecipient => 2,
            ExtType::StreamType       => 3,
        }
    }
}

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

/// StreamKey: A secret XChaCha20 key
#[derive(Clone)]
pub struct StreamKey {
    version: u8,
    key: SecretKey,
}

/// Lock
/// - Starts with a type identifier (one byte)
/// - When made with a Key, consists of a Curve25519 public key, XChaCha20 key, and random nonce
/// - When made with a StreamKey, consists of the XChaCha20 key and chosen nonce.
#[derive(Clone)]
pub struct Lock {
    version: u8,
    id: Option<Identity>,
    key: SecretKey,
    nonce: Nonce,
}

impl StreamKey {
    
    fn blank() -> StreamKey {
        StreamKey {
            version: 0,
            key: SecretKey([0; SECRET_KEY_BYTES]),
        }
    }

    pub fn new() -> StreamKey {
        let mut k = blank();
        version = 1;
        aead_keygen(&mut k.key);
        k
    }

}

fn aead_keygen(key: &mut SecretKey) {
    unsafe { libsodium_sys::crypto_aead_xchacha20poly1305_ietf_keygen(key.0.as_mut_ptr()) };
}

// Does in-place encryption of message and returns HMAC Tag
fn aead_encrypt(message: &mut [u8], additional: Option<&[u8]>, n: &Nonce, k: &SecretKey) -> Tag {
    let (additional_data, additional_len) = additional
        .map(|ad| (ad.as_ptr(), ad.len() as c_ulonglong))
        .unwrap_or((0 as *const _, 0));
    let mut tag = Tag([0; TAG_BYTES]);
    let mut mac_len = TAG_BYTES as c_ulonglong; // Function call sets this to TAG_BYTES and does nothing else
    unsafe {
        crypto_aead_xchacha20poly1305_ietf_encrypt_detached(
            message.as_mut_ptr(),
            tag.0.as_mut_ptr(),
            &mut mac_len,
            message.as_ptr(),
            message.len() as c_ulonglong,
            additional_data,
            additional_len,
            0 as *mut _,
            n.0.as_ptr(),
            k.0.as_ptr()
        );
    }
    tag
}

fn memzero(x: &mut [u8]) {
    unsafe { libsodium_sys::sodium_memzero(x.as_mut_ptr() as *mut _, x.len()); }
}
