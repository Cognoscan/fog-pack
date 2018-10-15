use std::io::{Write,Read};
use byteorder::{ReadBytesExt,WriteBytesExt};
use super::CryptoError;
use super::sodium::{SecretKey,Tag,Nonce,randombytes,aead_encrypt};
use super::StreamKey;

#[derive(Clone)]
pub enum LockType {
    PublicKey([u8; 64]),     // ephemeral key and the identity key used to make the shared secret (stream key)
    Stream([u8; 32]),        // UUID of the stream
    PublicKeyNoId([u8; 32]), // ephemeral key only
    StreamNoId,          // Nothing
}
impl LockType {

    fn to_u8(&self) -> u8 {
        match *self {
            LockType::PublicKey(_)     => 1,
            LockType::Stream(_)        => 2,
            LockType::PublicKeyNoId(_) => 3,
            LockType::StreamNoId       => 4,
        }
    }

    pub fn len(&self) -> usize {
        1 + match *self {
            LockType::PublicKey(v)     => v.len(),
            LockType::Stream(v)        => v.len(),
            LockType::PublicKeyNoId(v) => v.len(),
            LockType::StreamNoId       => 0,
        }
    }

    pub fn write<W: Write>(&self, wr: &mut W) -> Result<(), CryptoError> {
        wr.write_u8(self.to_u8())?;
        match *self {
            LockType::PublicKey(d)     => wr.write_all(&d),
            LockType::Stream(d)        => wr.write_all(&d),
            LockType::PublicKeyNoId(d) => wr.write_all(&d),
            LockType::StreamNoId       => Ok(()),
        }.map_err(CryptoError::Io)
    }

    pub fn read<R: Read>(rd: &mut R) -> Result<LockType, CryptoError> {
        let id = rd.read_u8().map_err(CryptoError::Io)?;
        let lock_type = match id {
            1 => LockType::PublicKey([0; 64]),
            2 => LockType::Stream([0; 32]),
            3 => LockType::PublicKeyNoId([0; 32]),
            4 => LockType::StreamNoId,
            _ => return Err(CryptoError::UnsupportedVersion),
        };
        match lock_type {
            LockType::PublicKey(mut d)     => rd.read_exact(&mut d),
            LockType::Stream(mut d)        => rd.read_exact(&mut d),
            LockType::PublicKeyNoId(mut d) => rd.read_exact(&mut d),
            LockType::StreamNoId           => Ok(()),
        }?;
        Ok(lock_type)
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

    pub fn from_stream(k: &StreamKey, use_id: bool) -> Lock {
        let mut nonce: Nonce = Default::default();
        randombytes(&mut nonce.0);
        if use_id {
            Lock { 
                version: k.get_version(),
                type_id: LockType::Stream(k.get_uuid().clone()),
                key: k.get_key().clone(),
                nonce: nonce,
                decoded: true
            }
        } else {
            Lock { 
                version: k.get_version(),
                type_id: LockType::StreamNoId,
                key: k.get_key().clone(),
                nonce: nonce,
                decoded: true
            }
        }
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
            type_id: LockType::StreamNoId,
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
// - the uuid of the secret key needed
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
