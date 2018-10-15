use std::io::{Write,Read};
use byteorder::{ReadBytesExt,WriteBytesExt};
use super::CryptoError;
use super::sodium::{SecretKey, aead_keygen, derive_uuid};

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
            key: Default::default(),
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

    pub fn get_version(&self) -> u8 {
        self.version
    }

    pub fn get_key(&self) -> &SecretKey {
        &self.key
    }

    pub fn complete(&mut self) {
        derive_uuid(&self.key, &mut self.uuid)
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

    //TODO: Make sure to test encrypting a message of length 0
}
