use std::ops::Drop;
use std::fmt;
use std::io::{Write,Read};
use byteorder::{ReadBytesExt,WriteBytesExt};
use super::CryptoError;
use libsodium_sys;

#[derive(Clone)]
struct SecretKey([u8; SECRET_KEY_BYTES]);

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

fn memzero(x: &mut [u8]) {
    unsafe { libsodium_sys::sodium_memzero(x.as_mut_ptr() as *mut _, x.len()); }
}
