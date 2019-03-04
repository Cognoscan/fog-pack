use std::io::Read;
use byteorder::{ReadBytesExt,WriteBytesExt};

use crypto::error::CryptoError;
use crypto::sodium::*;
use crypto::key::{FullKey, FullIdentity, Identity};
use crypto::stream::{FullStreamKey, StreamKey};

use crypto;


#[derive(Clone,PartialEq,Debug)]
enum LockType {
    /// Identity and ephemeral key used to make secret FullStreamKey
    Identity((PublicSignKey,PublicCryptKey)),
    /// ID of the Streamkey used
    Stream(StreamId),
}

impl LockType {

    fn to_u8(&self) -> u8 {
        match *self {
            LockType::Identity(_) => 1,
            LockType::Stream(_)   => 2,
        }
    }

    pub fn len(&self) -> usize {
        1 + match *self {
            LockType::Identity(ref v) => ((v.0).0.len() + (v.1).0.len()),
            LockType::Stream(ref v)    => v.0.len(),
        }
    }

    pub fn encode(&self, buf: &mut Vec<u8>) {
        buf.push(self.to_u8());
        match *self {
            LockType::Identity(ref d) => {
                buf.extend_from_slice(&(d.0).0);
                buf.extend_from_slice(&(d.1).0);
            },
            LockType::Stream(ref d) => buf.extend_from_slice(&d.0),
        }
    }

    pub fn decode(buf: &mut &[u8]) -> Result<LockType, CryptoError> {
        let id = buf.read_u8().map_err(CryptoError::Io)?;
        match id {
            1 => {
                let mut pk: PublicSignKey = Default::default();
                let mut epk: PublicCryptKey = Default::default();
                buf.read_exact(&mut pk.0)?;
                buf.read_exact(&mut epk.0)?;
                Ok(LockType::Identity((pk,epk)))
            },
            2 => {
                let mut id: StreamId = Default::default();
                buf.read_exact(&mut id.0)?;
                Ok(LockType::Stream(id))
            },
            _ => Err(CryptoError::UnsupportedVersion),
        }
    }
}


/// A container for either a StreamKey, Key, or Value. Can be encrypted for a 
/// particular Identity, or for a particular StreamKey
#[derive(Clone, PartialEq, Debug)]
pub struct Lockbox {
    version: u8,
    type_id: LockType,
    nonce: Nonce,
    ciphertext: Vec<u8>,
}


impl Lockbox {
    /// Get what version of Key/StreamKey was used in encrypting the Lockbox
    pub fn get_version(&self) -> u8 {
        self.version
    }

    /// Returns true if the Lockbox was encrypted with a StreamKey
    pub fn uses_stream(&self) -> bool {
        if let LockType::Stream(_) = self.type_id {
            true
        } else {
            false
        }
    }

    /// Returns true if the Lockbox was encrypted with an Identity
    pub fn uses_identity(&self) -> bool {
        if let LockType::Identity(_) = self.type_id {
            true
        } else {
            false
        }
    }

    /// Get the StreamKey used for encrypting the Lockbox. Returns None if an 
    /// Identity was used instead.
    pub fn get_stream(&self) -> Option<StreamKey> {
        if let LockType::Stream(ref stream) = self.type_id {
            Some(crypto::stream::stream_from_id(self.version, stream.clone()))
        } else {
            None
        }
    }

    /// Get the Identity used for encrypting the Lockbox. Returns None if a 
    /// StreamKey was used instead.
    pub fn get_id(&self) -> Option<Identity> {
        if let LockType::Identity(ref id) = self.type_id {
            Some(crypto::key::identity_from_id(self.version, id.0.clone()))
        } else {
            None
        }
    }

    /// Get the length of the Lockbox when it is binary-encoded
    pub fn len(&self) -> usize {
        1 + self.type_id.len() + self.nonce.0.len() + self.ciphertext.len()
    }

    /// Determine the length of the encrypted data, given the length of the message
    pub fn encrypt_len(&self, message_len: usize) -> usize {
        message_len + Tag::len()
    }

    pub fn encode(&self, buf: &mut Vec<u8>) {
        buf.reserve(self.len());
        buf.push(self.version);
        self.type_id.encode(buf);
        buf.extend_from_slice(&self.nonce.0);
        buf.extend_from_slice(&self.ciphertext[..]);
    }

    /*
    pub fn decode(buf: &mut [u8]) -> Result<Lockbox, CryptoError> {
        let version = buf.read_u8().map_err(CryptoError::Io)?;
        Lock
    }
    */
}

pub fn lockbox_from_stream(k: &FullStreamKey, mut message: Vec<u8>) -> Result<Lockbox, CryptoError> {
    let version = k.get_version();
    if version != 1 { return Err(CryptoError::UnsupportedVersion); }
    let mut nonce: Nonce = Default::default();
    randombytes(&mut nonce.0);
    let raw_key = k.get_key();
    let type_id = LockType::Stream(k.get_id());

    message.reserve_exact(Tag::len()); // Need exactly enough to append the tag
    let tag = aead_encrypt(&mut message[..], &[], &nonce, &raw_key);
    message.extend_from_slice(&tag.0);
    Ok(Lockbox {
        version,
        type_id,
        nonce,
        ciphertext: message
    })
}

// Can fail due to bad public key
pub fn lockbox_from_identity(id: &FullIdentity, mut message: Vec<u8>) -> Result<(Lockbox, FullStreamKey),CryptoError> {
    let version = id.get_version();
    if version != 1 { return Err(CryptoError::UnsupportedVersion); }
    let mut nonce: Nonce = Default::default();
    randombytes(&mut nonce.0);
    let mut esk: SecretCryptKey = Default::default();
    let mut epk: PublicCryptKey = Default::default();
    crypt_keypair(&mut epk, &mut esk);
    let k = id.calc_stream_key(&esk)?;
    let k = FullStreamKey::from_secret(k);
    let type_id = LockType::Identity((id.get_id(),epk));

    message.reserve_exact(Tag::len()); // Need exactly enough to append the tag
    let tag = aead_encrypt(&mut message[..], &[], &nonce, &k.get_key());
    message.extend_from_slice(&tag.0);
    Ok((Lockbox {
        version,
        type_id,
        nonce,
        ciphertext: message
    }, k))
}

/*
impl Lock {

    pub fn decrypt(&self, crypt: &[u8], ad: &[u8], out: &mut Vec<u8>) -> Result<(), CryptoError> {
        if !self.decoded { return Err(CryptoError::BadKey); }
        let m_len = crypt.len() - Tag::len();
        out.reserve(m_len); // Prepare the output vector
        let message_start = out.len(); // Store for later when we do in-place decryption
        out.extend_from_slice(&crypt[..m_len]);
        // Iterate over copied ciphertext and verify the tag
        let success = aead_decrypt(
            &mut out[message_start..],
            ad,
            &crypt[m_len..],
            &self.nonce,
            &self.key);
        if success {
            Ok(())
        } else {
            Err(CryptoError::DecryptFailed)
        }
    }

    pub fn get_stream(&self) -> Option<FullStreamKey> {
        if !self.decoded { return None; };
        Some(FullStreamKey::from_secret(self.key.clone()))
    }

    pub fn decode_stream(&mut self, k: &FullStreamKey) -> Result<(), CryptoError> {
        match self.type_id {
            LockType::Identity(_) => Err(CryptoError::BadKey),
            LockType::Stream(ref v) => {
                if *v != k.get_id() || self.version != k.get_version() {
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

    pub fn decode_identity(&mut self, k: &FullKey) -> Result<(), CryptoError> {
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
*/

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::init;

    fn enc_dec_identity(lk: Lock, k: &FullKey) {
        let mut v = Vec::new();
        lk.write(&mut v).unwrap();
        let mut lkd = Lock::read(&mut &v[..]).unwrap();
        match lkd.needs().unwrap() {
            LockType::Identity(v) => assert_eq!(v.0, k.get_id()),
            LockType::Stream(_) => panic!("Shouldn't be a stream lock"),
        };
        lkd.decode_identity(k).unwrap();
        assert_eq!(lk, lkd);
    }

    fn enc_dec_stream(lk: Lock, stream: &FullStreamKey) {
        let mut v = Vec::new();
        lk.write(&mut v).unwrap();
        let mut lkd = Lock::read(&mut &v[..]).unwrap();
        match lkd.needs().unwrap() {
            LockType::Identity(_) => panic!("Shouldn't be a identity lock"),
            LockType::Stream(i) => assert_eq!(*i, stream.get_id()),
        };
        lkd.decode_stream(stream).unwrap();
        assert_eq!(lk, lkd);
    }

    #[test]
    fn lock_types() {
        init().unwrap();
        let (k, id) = FullKey::new_pair().unwrap();
        let (lock, stream) = Lock::from_identity(&id).unwrap();
        enc_dec_identity(lock, &k);
        let lock = Lock::from_stream(&stream).unwrap();
        enc_dec_stream(lock, &stream);
    }

    #[test]
    fn stream_encrypt() {
        init().unwrap();
        let stream = FullStreamKey::new();
        let lock = Lock::from_stream(&stream).unwrap();
        let (data, a_data) = (vec![], vec![]);
        encrypt_decrypt(&lock, data, a_data);
        let (data, a_data) = (vec![0], vec![]);
        encrypt_decrypt(&lock, data, a_data);
        let (data, a_data) = (vec![], vec![0]);
        encrypt_decrypt(&lock, data, a_data);
        let (data, a_data) = (vec![0,1,2], vec![0,1,2]);
        encrypt_decrypt(&lock, data, a_data);
    }

    #[test]
    fn identity_encrypt() {
        init().unwrap();
        let (_, id) = FullKey::new_pair().unwrap();
        let (lock, _) = Lock::from_identity(&id).unwrap();
        let (data, a_data) = (vec![], vec![]);
        encrypt_decrypt(&lock, data, a_data);
        let (data, a_data) = (vec![0], vec![]);
        encrypt_decrypt(&lock, data, a_data);
        let (data, a_data) = (vec![], vec![0]);
        encrypt_decrypt(&lock, data, a_data);
        let (data, a_data) = (vec![0,1,2], vec![0,1,2]);
        encrypt_decrypt(&lock, data, a_data);
    }

    fn encrypt_decrypt(lk: &Lock, d: Vec<u8>, ad: Vec<u8>) {
        let mut ciphertext: Vec<u8> = Vec::new();
        let mut plaintext: Vec<u8> = Vec::new();
        lk.encrypt(&d[..], &ad[..], &mut ciphertext).unwrap();
        assert_eq!(ciphertext.len(), lk.encrypt_len(d.len()));
        if d.len() > 0 {
            assert_ne!(ciphertext[..d.len()], d[..]);
        }
        lk.decrypt(&ciphertext[..], &ad[..], &mut plaintext).unwrap();
        assert_eq!(d, plaintext);
    }
}
