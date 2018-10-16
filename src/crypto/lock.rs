use std::io::{Write,Read};
use byteorder::{ReadBytesExt,WriteBytesExt};
use super::CryptoError;
use super::sodium::*;
use super::{Identity,StreamKey};

#[derive(Clone)]
pub enum LockType {
    PublicKey((PublicSignKey,PublicCryptKey)), // identity and ephemeral key used to make secret StreamKey
    Stream(StreamId),         // ID of the stream
}
impl LockType {

    fn to_u8(&self) -> u8 {
        match *self {
            LockType::PublicKey(_) => 1,
            LockType::Stream(_)    => 2,
        }
    }

    pub fn len(&self) -> usize {
        1 + match *self {
            LockType::PublicKey(ref v) => ((v.0).0.len() + (v.1).0.len()),
            LockType::Stream(ref v)    => v.0.len(),
        }
    }

    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), CryptoError> {
        wr.write_u8(self.to_u8())?;
        match *self {
            LockType::PublicKey(ref d) => {
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
                Ok(LockType::PublicKey((pk,epk)))
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

/// Lock
/// - Starts with a type identifier (one byte)
/// - When made with a Key, consists of a Curve25519 public key, XChaCha20 key, and random nonce
/// - When made with a StreamKey, consists of the XChaCha20 key and chosen nonce.
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
        let k = calc_secret(id.get_crypt(), &esk)?;
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

    pub fn encrypt(&self, message: &[u8], ad: &[u8], out: &mut Vec<u8>) {
        out.reserve(self.encrypt_len(message.len())); // Prepare the vector
        let crypt_start = out.len(); // Store for later when we do the in-place encryption
        out.extend_from_slice(message);
        // Iterate over the copied message and append the tag
        let tag = aead_encrypt(&mut out[crypt_start..], ad, &self.nonce, &self.key);
        out.extend_from_slice(&tag.0);
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

}

// OH SHIT MF now what
// ok so like we want to figure out what's encoded given the raw data at the front
// And then we want to figure out what's 
//
// wait no
// we want to figure out what the key is somehow
//
// so maybe there's a clue in the front
// So that clue can be either
// - the identity needed
// - the id of the secret key needed
// and we should be able to look it up from those. does that mean we want to be able to 
// auto-complete public keys? Or just look them up, more likely.
//
// - Ok so that means we want to be able to hash the secret key and be able to hash the public 
// keys, and look them up in the same manner
//
// so i need to override the default hashing functions to let me look up keys and secret keys
// I also need to override the hashing for looking up streamkeys 
// I guess I feed the hasher the version number as well for all of these
//
// final goal is to re-derive the lock from what I figure out given the pre-pended stuff
// then decode the entire stream
// 
// so maybe I want to have two separate lock types? encoded lock and decoded lock
// and the decoded lock lets me encode or decode a giant string of stuff
