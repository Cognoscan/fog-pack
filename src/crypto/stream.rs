use std::ops::Drop;
use std::fmt;
use std::io::{Write,Read};
use byteorder::{ReadBytesExt,WriteBytesExt};
use super::CryptoError;
use libsodium_sys;
use libc::c_ulonglong;
use std::ffi::CString;

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
    PublicKey,
    Stream,
}
impl LockType {
    fn to_u8(&self) -> u8 {
        match *self {
            LockType::PublicKey  => 1,
            LockType::Stream     => 2,
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

/// StreamKey: A secret XChaCha20 key, identifiable by its UUID
#[derive(Clone)]
pub struct StreamKey {
    version: u8,
    uuid: [u8; 32],
    key: SecretKey,
}

impl StreamKey {
    
    fn blank() -> StreamKey {
        StreamKey {
            version: 0,
            uuid: [0; 32],
            key: SecretKey([0; SECRET_KEY_BYTES]),
        }
    }

    pub fn new() -> StreamKey {
        let mut k = StreamKey::blank();
        k.version = 1;
        aead_keygen(&mut k.key);
        k.complete();
        k
    }

    pub fn get_uuid(&self) -> &[u8; 32] {
        &self.uuid
    }

    pub fn complete(&mut self) {
        derive_uuid(&self.key, &mut self.uuid)
    }

    pub fn new_lock(&self) -> Lock {
        let mut nonce = Nonce([0;24]);
        randombytes(&mut nonce.0);
        let mut pre_data = vec![self.version];
        pre_data.extend_from_slice(&self.uuid);
        Lock { 
            version: 1,
            type_id: LockType::Stream,
            pre_data: pre_data,
            key: self.key.clone(),
            nonce: nonce,
        }
    }

    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), CryptoError> {
        wr.write_u8(self.version).map_err(CryptoError::Io)?;
        wr.write_all(&self.key.0).map_err(CryptoError::Io)?;
        Ok(())
    }

    pub fn read<R: Read>(rd: &mut R) -> Result<StreamKey, CryptoError> {
        let mut k = StreamKey::blank();
        k.version = rd.read_u8().map_err(CryptoError::Io)?;
        if k.version != 1 { return Err(CryptoError::UnsupportedVersion); }
        rd.read_exact(&mut k.key.0).map_err(CryptoError::Io)?;
        k.complete();
        Ok(k)
    }

}

/// Lock
/// - Starts with a type identifier (one byte)
/// - When made with a Key, consists of a Curve25519 public key, XChaCha20 key, and random nonce
/// - When made with a StreamKey, consists of the XChaCha20 key and chosen nonce.
#[derive(Clone)]
pub struct Lock {
    version: u8,
    type_id: LockType,
    pre_data: Vec<u8>,
    key: SecretKey,
    nonce: Nonce,
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
        libsodium_sys::crypto_aead_xchacha20poly1305_ietf_encrypt_detached(
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

// Shouldn't fail as long as the input parameters are valid
fn derive_uuid(k: &SecretKey, uuid: &mut [u8; 32]) {
    unsafe {
        let ctx = CString::from_vec_unchecked(b"condense".to_vec());
        libsodium_sys::crypto_kdf_derive_from_key(uuid.as_mut_ptr(), 32, 1, ctx.as_ptr(), k.0.as_ptr());
    };
}

fn memzero(x: &mut [u8]) {
    unsafe { libsodium_sys::sodium_memzero(x.as_mut_ptr() as *mut _, x.len()); }
}

fn randombytes(x: &mut [u8]) {
    unsafe { libsodium_sys::randombytes_buf(x.as_mut_ptr() as *mut _, x.len()); }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example_key() -> StreamKey {
        // Below test vectors come from using the C version of crypto_kdf_derive_from_key with 
        // subkey index of 1 and "condense" as the context.
        let key: [u8; 32] = [0x49, 0x2f, 0xeb, 0x2e, 0x37, 0x5f, 0x2c, 0x70, 
                             0x9f, 0x12, 0xca, 0xf7, 0xad, 0xea, 0xfe, 0x66,
                             0xfe, 0x5a, 0x77, 0x2f, 0x0f, 0x50, 0x83, 0x6d,
                             0x63, 0x4f, 0xf1, 0x8a, 0xa5, 0x43, 0x26, 0x64];
        let mut key = StreamKey { version: 1, key: SecretKey(key), uuid: [0; 32] };
        derive_uuid(&key.key, &mut key.uuid);
        key
    }

    #[test]
    fn test_uuid_gen() {
        let key = example_key();
        let subkey: [u8; 32] = [0x79, 0x91, 0xee, 0x19, 0x16, 0x78, 0x60, 0xe3,
                                0x5a, 0xaf, 0x5d, 0x54, 0x5e, 0xf9, 0x80, 0x58,
                                0x72, 0xb9, 0x35, 0x25, 0x61, 0x76, 0x37, 0x21,
                                0xd4, 0xef, 0x15, 0x5e, 0xaa, 0x53, 0xb1, 0x47];
        assert_eq!(key.uuid, subkey);
    }

    fn enc_dec(k: StreamKey) {
        let mut v = Vec::new();
        k.write(&mut v).unwrap();
        let kd = StreamKey::read(&mut &v[..]).unwrap();
        assert_eq!(k.version, kd.version);
        assert_eq!(k.key.0, kd.key.0);
        assert_eq!(k.uuid, kd.uuid);
    }
    
    #[test]
    fn test_stream_enc() {
        let k = example_key();
        enc_dec(k);
    }
}
