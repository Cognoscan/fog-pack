use std::io::Read;
use byteorder::ReadBytesExt;
use std::fmt;

use crypto::error::CryptoError;
use crypto::sodium::{StreamId, SecretKey, aead_keygen, derive_id};

#[derive(Clone,PartialEq,Eq,Hash)]
pub struct StreamKey {
    version: u8,
    id: StreamId,
}

pub fn stream_from_id(version: u8, id: StreamId) -> StreamKey {
    StreamKey { version, id }
}

impl fmt::Debug for StreamKey {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{} {{ ver={}, {:x?} }}", stringify!(StreamKey), &self.version, &self.id.0[..])
    }
}

/// FullStreamKey: A secret XChaCha20 key, identifiable by its ID
#[derive(Clone)]
pub struct FullStreamKey {
    version: u8,
    id: StreamId,
    key: SecretKey,
}

impl FullStreamKey {
    
    fn blank() -> FullStreamKey {
        FullStreamKey {
            version: 0,
            id: Default::default(),
            key: Default::default(),
        }
    }

    pub fn new() -> FullStreamKey {
        let mut k = FullStreamKey::blank();
        k.version = 1;
        aead_keygen(&mut k.key);
        k.complete();
        k
    }

    pub fn from_secret(k: SecretKey) -> FullStreamKey {
        let mut stream = FullStreamKey::blank();
        stream.version = 1;
        stream.key = k;
        stream.complete();
        stream
    }

    pub fn get_id(&self) -> StreamId {
        (&self.id).clone()
    }

    pub fn get_version(&self) -> u8 {
        self.version
    }

    pub fn get_key(&self) -> &SecretKey {
        &self.key
    }

    pub fn get_stream_ref(&self) -> StreamKey {
        StreamKey {
            version: self.version,
            id: self.get_id()
        }
    }

    pub fn complete(&mut self) {
        derive_id(&self.key, &mut self.id)
    }

    pub fn len(&self) -> usize {
        1 + self.key.0.len()
    }

    pub fn max_len() -> usize {
        1 + SecretKey::len()
    }

    pub fn encode(&self, buf: &mut Vec<u8>) {
        buf.reserve(self.len());
        buf.push(self.version);
        buf.extend_from_slice(&self.key.0);
    }

    pub fn decode(buf: &mut &[u8]) -> Result<FullStreamKey, CryptoError> {
        let mut k = FullStreamKey::blank();
        k.version = buf.read_u8().map_err(CryptoError::Io)?;
        if k.version != 1 { return Err(CryptoError::UnsupportedVersion); }
        buf.read_exact(&mut k.key.0).map_err(CryptoError::Io)?;
        k.complete();
        Ok(k)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn example_key() -> FullStreamKey {
        // Below test vectors come from using the C version of crypto_kdf_derive_from_key with 
        // subkey index of 1 and "condense" as the context.
        let key: [u8; 32] = [0x49, 0x2f, 0xeb, 0x2e, 0x37, 0x5f, 0x2c, 0x70, 
                             0x9f, 0x12, 0xca, 0xf7, 0xad, 0xea, 0xfe, 0x66,
                             0xfe, 0x5a, 0x77, 0x2f, 0x0f, 0x50, 0x83, 0x6d,
                             0x63, 0x4f, 0xf1, 0x8a, 0xa5, 0x43, 0x26, 0x64];
        FullStreamKey::from_secret(SecretKey(key))
    }

    #[test]
    fn id_gen() {
        let key = example_key();
        let subkey: [u8; 32] = [0x79, 0x91, 0xee, 0x19, 0x16, 0x78, 0x60, 0xe3,
                                0x5a, 0xaf, 0x5d, 0x54, 0x5e, 0xf9, 0x80, 0x58,
                                0x72, 0xb9, 0x35, 0x25, 0x61, 0x76, 0x37, 0x21,
                                0xd4, 0xef, 0x15, 0x5e, 0xaa, 0x53, 0xb1, 0x47];
        assert_eq!(key.id.0, subkey);
    }

    fn enc_dec(k: FullStreamKey) {
        let mut v = Vec::new();
        k.encode(&mut v);
        let kd = FullStreamKey::decode(&mut &v[..]).unwrap();
        assert_eq!(k.version, kd.version);
        assert_eq!(k.key.0, kd.key.0);
        assert_eq!(k.id.0, kd.id.0);
    }
    
    #[test]
    fn stream_enc() {
        let k = example_key();
        enc_dec(k);
    }
}
