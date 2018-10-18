use std::io::{Write,Read};
use byteorder::{ReadBytesExt,WriteBytesExt};
use super::CryptoError;
use super::sodium::*;
use super::{Key,Identity,StreamKey};

#[derive(Clone)]
pub enum LockType {
    Identity((PublicSignKey,PublicCryptKey)), // identity and ephemeral key used to make secret StreamKey
    Stream(StreamId),         // ID of the stream
}
impl LockType {

    fn to_u8(&self) -> u8 {
        match *self {
            LockType::Identity(_) => 1,
            LockType::Stream(_)    => 2,
        }
    }

    pub fn len(&self) -> usize {
        1 + match *self {
            LockType::Identity(ref v) => ((v.0).0.len() + (v.1).0.len()),
            LockType::Stream(ref v)    => v.0.len(),
        }
    }

    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), CryptoError> {
        wr.write_u8(self.to_u8())?;
        match *self {
            LockType::Identity(ref d) => {
                wr.write_all(&(d.0).0).map_err(CryptoError::Io)?;
                wr.write_all(&(d.1).0).map_err(CryptoError::Io)
            },
            LockType::Stream(ref d)    => wr.write_all(&d.0).map_err(CryptoError::Io),
        }
    }

    pub fn read<R: Read>(rd: &mut R) -> Result<LockType, CryptoError> {
        let id = rd.read_u8().map_err(CryptoError::Io)?;

        match id {
            1 => {
                let mut pk: PublicSignKey = Default::default();
                let mut epk: PublicCryptKey = Default::default();
                rd.read_exact(&mut pk.0)?;
                rd.read_exact(&mut epk.0)?;
                Ok(LockType::Identity((pk,epk)))
            },
            2 => {
                let mut id: StreamId = Default::default();
                rd.read_exact(&mut id.0)?;
                Ok(LockType::Stream(id))
            },
            _ => Err(CryptoError::UnsupportedVersion),
        }
    }
}

/// Contains everything needed to encrypt one payload. A lock can be generated from an 
/// [`Identity`], which also will produce an associated [`StreamKey`]. A lock can also be generated 
/// by any valid `StreamKey`.
///
/// To use it for encryption, use `write` to write its identifying information into a byte stream. 
/// Next, write any additional certified data to the byte stream, then use `encrypt` to encrypt and 
/// append all encrypted data to a `Vec`. This wil use XChaCha20 for encryption, and use Poly1305 
/// for the authentication tag. This follows the AEAD construction used by libsodium.
///
/// A lock **must** only be used for a single payload. If encrypting multiple payloads, generate 
/// one lock for each.
///
/// To use it for decryption, use `read` to read it from a byte stream. Next, use `needs` to get 
/// the type of lock and identifying information, and call either `decode_identity` or 
/// `decode_stream` to recover the secret key
///
/// To use it for decryption, use `read` to read it from a byte stream. Next, use `needs` to get 
/// the type of lock and identifying information, and call either `decode_identity` or 
/// `decode_stream` to recover the secret key. Once this is done, call `decrypt` to decode and 
/// verify the encrypted data.
#[derive(Clone)]
pub struct Lock {
    version: u8,
    type_id: LockType,
    key: SecretKey,
    nonce: Nonce,
    decoded: bool,
}

impl Lock {

    pub fn from_stream(k: &StreamKey) -> Result<Lock,CryptoError> {
        let version = k.get_version();
        if version != 1 { return Err(CryptoError::UnsupportedVersion); }
        let mut nonce: Nonce = Default::default();
        randombytes(&mut nonce.0);
        Ok(Lock { 
            version,
            type_id: LockType::Stream(k.get_id().clone()),
            key: k.get_key().clone(),
            nonce,
            decoded: true
        })
    }

    // Can fail due to bad public key
    pub fn from_identity(id: &Identity) -> Result<(Lock,StreamKey),CryptoError> {
        let version = id.get_version();
        if version != 1 { return Err(CryptoError::UnsupportedVersion); }
        let mut nonce: Nonce = Default::default();
        randombytes(&mut nonce.0);
        let mut esk: SecretCryptKey = Default::default();
        let mut epk: PublicCryptKey = Default::default();
        crypt_keypair(&mut epk, &mut esk);
        let k = id.calc_stream_key(&esk)?;
        let k = StreamKey::from_secret(k);
        Ok((Lock {
            version,
            type_id: LockType::Stream(k.get_id().clone()),
            key: k.get_key().clone(),
            nonce,
            decoded: true
        }, k))
    }

    // Determine the length of the encrypted data, given the length of the message
    pub fn encrypt_len(&self, message_len: usize) -> usize {
        message_len + Tag::len()
    }

    pub fn encrypt(&self, message: &[u8], ad: &[u8], out: &mut Vec<u8>) -> Result<(), CryptoError> {
        if !self.decoded { return Err(CryptoError::BadKey); }
        out.reserve(self.encrypt_len(message.len())); // Prepare the vector
        let crypt_start = out.len(); // Store for later when we do the in-place encryption
        out.extend_from_slice(message);
        // Iterate over the copied message and append the tag
        let tag = aead_encrypt(&mut out[crypt_start..], ad, &self.nonce, &self.key);
        out.extend_from_slice(&tag.0);
        Ok(())
    }

    pub fn decrypt(&self, crypt: &[u8], ad: &[u8], out: &mut Vec<u8>) -> Result<(), CryptoError> {
        if !self.decoded { return Err(CryptoError::BadKey); }
        let m_len = crypt.len() - Tag::len();
        out.reserve(m_len); // Prepare the output vector
        let message_start = out.len(); // Store for later when we do in-place decryption
        out.extend_from_slice(&crypt[..m_len]);
        // Iterate over copied ciphertext and verify the tag
        let success = aead_decrypt(&mut out[message_start..], ad, &self.nonce, &self.key);
        if success {
            Ok(())
        } else {
            Err(CryptoError::DecryptFailed)
        }
    }

    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), CryptoError> {
        wr.write_u8(self.version)?;
        &self.type_id.write(wr)?;
        wr.write_all(&self.nonce.0)?;
        Ok(())
    }

    pub fn read<R: Read>(rd: &mut R) -> Result<Lock, CryptoError> {
        let mut lock = Lock {
            version: 0,
            type_id: LockType::Stream(Default::default()),
            key: Default::default(),
            nonce: Default::default(),
            decoded:false,
        };
        lock.version = rd.read_u8()?;
        lock.type_id = LockType::read(rd)?;
        rd.read_exact(&mut lock.nonce.0)?;
        Ok(lock)
    }

    pub fn needs(&self) -> Option<&LockType> {
        if self.decoded { None } else { Some(&self.type_id) }
    }

    pub fn decode_stream(&mut self, k: &StreamKey) -> Result<(), CryptoError> {
        match self.type_id {
            LockType::Identity(_) => Err(CryptoError::BadKey),
            LockType::Stream(ref v) => {
                if *v != *k.get_id() || self.version != k.get_version() {
                    Err(CryptoError::BadKey)
                }
                else {
                    self.key = k.get_key().clone();
                    self.decoded = true;
                    Ok(())
                }
            },
        }
    }

    pub fn decode_identity(&mut self, k: &Key) -> Result<(), CryptoError> {
        match self.type_id {
            LockType::Identity(ref v) => {
                if v.0 != k.get_id() || self.version != k.get_version() {
                    Err(CryptoError::BadKey)
                }
                else {
                    self.key = k.calc_stream_key(&v.1)?;
                    self.decoded = true;
                    Ok(())
                }
            },
            LockType::Stream(_) => Err(CryptoError::BadKey),
        }
    }

}

#[cfg(test)]
mod tests {
    //use super::*;

}
